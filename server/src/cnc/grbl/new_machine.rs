use std::{str::from_utf8_unchecked, mem::MaybeUninit};

use async_trait::async_trait;
use chrono::Local;
use futures::{Future};
use tokio::join;
use tokio::io::{AsyncBufRead, Lines};
use crate::cnc::machine_writer::MachineWriter;
use {
    super::{
        messages::{GrblMessage, GrblPosition, GrblStateInfo, ProbeEvent},
        parser::parse_grbl_line,
    },
    crate::util::history_broadcast,
    futures::{
        future::{Fuse, FusedFuture},
        FutureExt,
    },
    ndarray::Array1,
    std::{collections::VecDeque, pin::Pin, time::Duration},
    tokio::{
        io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
        select, spawn,
        sync::{mpsc, oneshot},
        time::{sleep, Sleep},
    },
};
use super::handler::Handler;
pub use super::handler::{LineError, ProbeError, WriteRequest, ImmediateRequest};

struct MachineThread<'a, Write: MachineWriter, H: Handler> {
    writer: Write,
    handler: &'a H,
    waiting_ok: VecDeque<oneshot::Sender<Result<(), LineError>>>,
    waiting_probe: VecDeque<oneshot::Sender<Result<ProbeEvent, ProbeError>>>,
    waiting_status: VecDeque<oneshot::Sender<GrblStateInfo>>,
    work_coordinate_offset: Option<Array1<f64>>,
}
impl<'a, Write: MachineWriter, H: Handler> MachineThread<'a, Write, H> {
    fn log_send(&mut self, bytes: Vec<u8>) {
        self.handler.after_send(bytes);
    }
    async fn send_immediate(&mut self, bytes: Vec<u8>) {
        let bytes = self.writer.write_immediate(bytes).await.unwrap();
        self.log_send(bytes);
    }
    async fn receive_line(&mut self, line: String) {
        self.handler.after_receive(line.clone());
        let parsed = parse_grbl_line(&line);
        match parsed {
            GrblMessage::ProbeEvent(probe_event) => {
                let next_result = self.waiting_probe.pop_front();
                match next_result {
                    Some(channel) => drop(channel.send(Ok(probe_event))),
                    None => self.handler.warn(
                        "received probe info without listener".to_string(),
                    ),
                }
            }
            GrblMessage::StatusEvent(status_event) => {
                if let Some(wco) = status_event.work_coordinate_offset {
                    self.work_coordinate_offset = Some(wco);
                }
                // self.work_coordinate_offset must be set according to protocol!
                let machine_position = match status_event.machine_position {
                    GrblPosition::Machine(pos) => pos,
                    GrblPosition::Work(pos) => pos + self.work_coordinate_offset.as_ref().unwrap(),
                };
                let state = GrblStateInfo {
                    state: status_event.state,
                    machine_position,
                    work_coordinate_offset: self.work_coordinate_offset.as_ref().unwrap().clone(),
                };
                for waiting in self.waiting_status.drain(..) {
                    drop(waiting.send(state.clone())); // Don't worry about if it actually sent;
                }
            }
            GrblMessage::GrblError(index) => {
                self.writer.pop_received_line().await.unwrap().map(|v| self.log_send(v));
                self.handler.warn(
                    format!("Error received: {}!", GrblMessage::get_error_text(index)),
                );
                let next_result = self.waiting_ok.pop_front();
                match next_result {
                    Some(channel) => drop(channel.send(Err(LineError::Grbl(index)))),
                    None => self.handler.warn(
                        "received error without listener".to_string(),
                    ),
                }
            }
            GrblMessage::GrblAlarm(index) => {
                self.handler.on_alarm(index).await;
                self.handler.warn(
                    format!("Alarm received: {}!", GrblMessage::get_alarm_text(index)),
                );
            },
            GrblMessage::GrblOk => {
                self.writer.pop_received_line().await.unwrap().map(|v| self.log_send(v));
                let next_result = self.waiting_ok.pop_front();
                match next_result {
                    Some(channel) => drop(channel.send(Ok(()))),
                    None => self.handler.warn(
                        "received ok without listener".to_string(),
                    ),
                }
            }
            GrblMessage::GrblGreeting => self.handler.warn(
                "received unexpected greeting!".to_string(),
            ),
            GrblMessage::Unrecognized(line) => {
                self.handler.warn(format!("Unrecognized line: {:?}", line))
            }
        }
    }
    async fn plain_send(&mut self, request: WriteRequest) {
        match request {
            WriteRequest::Plain { data, result } => {
                self.writer.enqueue_line(data).await.unwrap().map(|v| self.log_send(v));
                self.waiting_ok.push_back(result);
            }
            WriteRequest::Probe {
                data,
                result_line,
                result,
            } => {
                self.writer.enqueue_line(data).await.unwrap().map(|v| self.log_send(v));
                self.waiting_ok.push_back(result_line);
                self.waiting_probe.push_back(result);
            }
        }
    }
    async fn immediate_send(&mut self, request: ImmediateRequest) {
        match request {
            ImmediateRequest::Status { result } => {
                self.send_immediate(vec![b'?']).await;
                self.waiting_status.push_back(result);
            },
            ImmediateRequest::FeedHold => {
                self.send_immediate(vec![b'!']).await;
            },
            ImmediateRequest::FeedResume => {
                self.send_immediate(vec![b'~']).await;
            },
            ImmediateRequest::Reset => {
                self.send_immediate(vec![0x18]).await;
            },
        }
    }
    async fn reset(&mut self) {
        self.writer.clear_waiting();
        // Clear out all expected results. They're not coming.
        for waiting in self.waiting_ok.drain(..) {
            drop(waiting.send(Err(LineError::Reset)));
        }
        for waiting in self.waiting_probe.drain(..) {
            drop(waiting.send(Err(ProbeError::Reset)));
        }
        // If there is still a waiting status, it may have been cleared. Re-send it.
        if !self.waiting_status.is_empty() {
            self.send_immediate(vec![b'?']).await;
        }
    }
    async fn wait_for_greeting<Read: AsyncBufRead + Unpin>(&mut self, lines_reader: &mut Lines<Read>) -> Result<(), std::io::Error> {
        loop {
            match lines_reader.next_line().await {
                Ok(Some(line)) => {
                    self.handler.after_receive(line.clone());
                    if let GrblMessage::GrblGreeting = parse_grbl_line(&line) {
                        return Ok(())
                    }
                }
                Ok(None) => return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "unexpected eof!")),
                Err(e) => return Err(e),
            }
        }
    }    
}

pub async fn run_machine_with_handler<H, W, R>(handler: H, writer: W, reader: R)
where
    R: AsyncBufRead + Unpin,
    H: Handler,
    W: MachineWriter
{
    let mut lines_reader = BufReader::new(reader).lines();
    let mut machine_thread = MachineThread {
        writer,
        handler: &handler,
        waiting_ok: Default::default(),
        waiting_probe: Default::default(),
        waiting_status: Default::default(),
        work_coordinate_offset: None,
    };
    join!(
        async {
            // Main loop for the machine
            'outer: loop {
                machine_thread.wait_for_greeting(&mut lines_reader).await.unwrap();
                machine_thread.reset().await;  // Reset here: we now know that no more messages from the prior world will arrive.
                loop {
                    select! {
                        biased;
                        line = lines_reader.next_line() => {
                            if let Ok(Some(line)) = line {
                                machine_thread.receive_line(line).await
                            }
                        },
                        immediate_request = machine_thread.handler.next_immediate_request() => {
                            let must_reset = if let ImmediateRequest::Reset = immediate_request { true } else { false };
                            machine_thread.immediate_send(immediate_request).await;
                            if must_reset {
                                continue 'outer  //Expect another greeting.
                            }
                        },
                        write_request = machine_thread.handler.next_write_request(), if machine_thread.writer.can_enqueue_line() => {
                            machine_thread.plain_send(write_request).await
                        },
                    }
                }
            }
        },
        handler.run()
    );
}
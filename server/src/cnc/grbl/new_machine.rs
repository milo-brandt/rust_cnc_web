use std::{str::from_utf8_unchecked, mem::MaybeUninit};

use async_trait::async_trait;
use chrono::Local;
use futures::Future;
use ringbuf::LocalRb;
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

#[derive(Clone, Debug)]
pub enum MachineDebugEvent {
    Sent(Vec<u8>),
    Received(String),
    Warning(String),
    Comment(String),
}

#[derive(Debug)]
pub enum WriteRequest {
    Plain {
        data: Vec<u8>,                            // Should include newline
        result: oneshot::Sender<Result<(), u64>>, // gives error code on failure
    },
    Probe {
        data: Vec<u8>,
        result_ok: oneshot::Sender<Result<(), u64>>, // gives error code on failure
        result: oneshot::Sender<Result<ProbeEvent, u64>>, // gives error code on failure
    },
    Comment(String),
}
#[derive(Debug)]
pub enum ImmediateRequest {
    Status {
        result: oneshot::Sender<GrblStateInfo>,
    },
    FeedHold,
    FeedResume,
    Reset,
}
pub struct MachineThreadInput {
    debug_stream: history_broadcast::Sender<MachineDebugEvent>,
    write_stream: mpsc::Receiver<WriteRequest>,
    immediate_write_stream: mpsc::Receiver<ImmediateRequest>,
}

struct MachineThread<Write: MachineWriter> {
    writer: Write,
    debug_stream: history_broadcast::Sender<MachineDebugEvent>,
    waiting_ok: VecDeque<oneshot::Sender<Result<(), u64>>>,
    waiting_probe: VecDeque<oneshot::Sender<Result<ProbeEvent, u64>>>,
    waiting_status: VecDeque<oneshot::Sender<GrblStateInfo>>,
    status_refresh: Pin<Box<Fuse<Sleep>>>,
    work_coordinate_offset: Array1<f64>,
}
pub struct MachineInterface {
    pub debug_stream: history_broadcast::Receiver<MachineDebugEvent>,
    pub write_stream: mpsc::Sender<WriteRequest>,
    pub immediate_write_stream: mpsc::Sender<ImmediateRequest>,
}
impl<Write: MachineWriter> MachineThread<Write> {
    fn log_send(&mut self, bytes: Vec<u8>) {
        self.debug_stream.send(MachineDebugEvent::Sent(bytes));
    }
    async fn send_immediate(&mut self, bytes: Vec<u8>) {
        let bytes = self.writer.write_immediate(bytes).await.unwrap();
        self.log_send(bytes);
    }
    async fn receive_line(&mut self, line: String) {
        self.debug_stream
            .send(MachineDebugEvent::Received(line.clone()));
        let parsed = parse_grbl_line(&line);
        match parsed {
            GrblMessage::ProbeEvent(probe_event) => {
                let next_result = self.waiting_probe.pop_front();
                match next_result {
                    Some(channel) => drop(channel.send(Ok(probe_event))),
                    None => self.debug_stream.send(MachineDebugEvent::Warning(
                        "received probe info without listener".to_string(),
                    )),
                }
            }
            GrblMessage::StatusEvent(status_event) => {
                if let Some(wco) = status_event.work_coordinate_offset {
                    self.work_coordinate_offset = wco;
                }
                let machine_position = match status_event.machine_position {
                    GrblPosition::Machine(pos) => pos,
                    GrblPosition::Work(pos) => pos + &self.work_coordinate_offset,
                };
                let state = GrblStateInfo {
                    state: status_event.state,
                    machine_position,
                    work_coordinate_offset: self.work_coordinate_offset.clone(),
                };
                for waiting in self.waiting_status.drain(..) {
                    drop(waiting.send(state.clone())); // Don't worry about if it actually sent;
                }
                self.status_refresh = Box::pin(Fuse::terminated());
            }
            GrblMessage::GrblError(index) => {
                self.writer.pop_received_line().await.unwrap().map(|v| self.log_send(v));
                self.debug_stream.send(MachineDebugEvent::Warning(
                    format!("Error received: {}!", GrblMessage::get_error_text(index)),
                ));
                let next_result = self.waiting_ok.pop_front();
                match next_result {
                    Some(channel) => drop(channel.send(Err(index))),
                    None => self.debug_stream.send(MachineDebugEvent::Warning(
                        "received error without listener".to_string(),
                    )),
                }
            }
            GrblMessage::GrblAlarm(index) => self.debug_stream.send(MachineDebugEvent::Warning(
                format!("Alarm received: {}!", GrblMessage::get_alarm_text(index)),
            )),
            GrblMessage::GrblOk => {
                self.writer.pop_received_line().await.unwrap().map(|v| self.log_send(v));
                let next_result = self.waiting_ok.pop_front();
                match next_result {
                    Some(channel) => drop(channel.send(Ok(()))),
                    None => self.debug_stream.send(MachineDebugEvent::Warning(
                        "received ok without listener".to_string(),
                    )),
                }
            }
            GrblMessage::GrblGreeting => self.debug_stream.send(MachineDebugEvent::Warning(
                "received unexpected greeting!".to_string(),
            )),
            GrblMessage::Unrecognized(_) => {} // ignore
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
                result_ok,
                result,
            } => {
                self.writer.enqueue_line(data).await.unwrap().map(|v| self.log_send(v));
                self.waiting_ok.push_back(result_ok);
                self.waiting_probe.push_back(result);
            }
            WriteRequest::Comment(comment) => {
                self.debug_stream.send(MachineDebugEvent::Comment(comment))
            }
        }
    }
    async fn rerequest_status(&mut self) {
        self.send_immediate(vec![b'?']).await;
        self.status_refresh = Box::pin(sleep(Duration::from_millis(1000)).fuse());
        self.debug_stream.send(MachineDebugEvent::Warning(
            "Needed to resend status query!".to_string(),
        ));
    }
    async fn immediate_send(&mut self, request: ImmediateRequest) {
        match request {
            ImmediateRequest::Status { result } => {
                self.waiting_status.push_back(result);
                if self.status_refresh.is_terminated() {
                    // Nominally required for grbl interface - can get cancelled time to time.
                    // Waits 1000 ms for response to ? before resending; note that faster polling is allowed if the response has come back.
                    self.send_immediate(vec![b'?']).await;
                }
            }
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
}
pub async fn start_machine<
    Read: AsyncRead + Unpin + Send + 'static,
    Write: MachineWriter + Unpin + Send + 'static,
>(
    reader: Read,
    mut writer: Write,
) -> Option<(MachineInterface, impl Future<Output=()>)> {
    let mut lines_reader = BufReader::new(reader).lines();
    // First: loop until we get a greeting.
    println!("Waiting for greeting...");
    let mut debug_stream = history_broadcast::Sender::new(512);
    let (write_stream_send, mut write_stream_receive) = mpsc::channel(32);
    let (immediate_write_stream_send, mut immediate_write_stream_receive) = mpsc::channel(32);
    let debug_stream_receiver = debug_stream.subscribe_with_history_count(0);
    loop {
        match lines_reader.next_line().await {
            Ok(Some(line)) => {
                debug_stream.send(MachineDebugEvent::Received(line.clone()));
                if let GrblMessage::GrblGreeting = parse_grbl_line(&line) {
                    break;
                }
            }
            _ => return None,
        }
    }
    println!("Greeted...");
    // Then, ask for status and store the WCO!
    debug_stream.send(MachineDebugEvent::Sent(vec![b'?']));
    match writer.write_immediate(b"?".to_vec()).await {
        Ok(_) => {}
        Err(_) => return None,
    }
    println!("Sent '?'...");
    let work_coordinate_offset = loop {
        match lines_reader.next_line().await {
            Ok(Some(line)) => {
                debug_stream.send(MachineDebugEvent::Received(line.clone()));
                if let GrblMessage::StatusEvent(status) = parse_grbl_line(&line) {
                    break status.work_coordinate_offset.unwrap();
                }
            }
            _ => return None,
        }
    };
    println!("Status received... spawning machine thread");
    let machine_future = async move {
        let mut machine_thread = MachineThread {
            writer,
            debug_stream,
            waiting_ok: Default::default(),
            waiting_probe: Default::default(),
            waiting_status: Default::default(),
            status_refresh: Box::pin(Fuse::terminated()),
            work_coordinate_offset,
        };
        loop {
            select! {
                biased;
                line = lines_reader.next_line() => {
                    if let Ok(Some(line)) = line {
                        machine_thread.receive_line(line).await
                    }
                },
                write_request = write_stream_receive.recv(), if machine_thread.writer.can_enqueue_line() => {
                    if let Some(request) = write_request {
                        machine_thread.plain_send(request).await
                    }
                },
                immediate_write_request = immediate_write_stream_receive.recv() => {
                    if let Some(request) = immediate_write_request {
                        machine_thread.immediate_send(request).await
                    }
                },
                _ = &mut machine_thread.status_refresh => {
                    machine_thread.rerequest_status().await
                }
            }
        }
    };
    Some((MachineInterface {
        debug_stream: debug_stream_receiver,
        write_stream: write_stream_send,
        immediate_write_stream: immediate_write_stream_send,
    }, machine_future))
}

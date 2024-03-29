/*
    This code is not good. Hard to improve. :/
*/

use core::ascii;
use std::{sync::Arc, future::IntoFuture, cell::{RefCell, Cell}, pin::Pin, time::{Duration, Instant}};

use crate::{cnc::{gcode::{GCodeLine, GCodeFormatSpecification}}, util::{local_generation_counter::LocalGenerationCounter, fixed_rb::{FixedRb}, history_broadcast, format_bytes::format_byte_string, future_or_pending::FutureOrPending}};

use super::{handler::{Handler, SpeedOverride}, new_machine::{LineError, WriteRequest, ProbeError, ImmediateRequest}, messages::{ProbeEvent, GrblStateInfo}};
use async_trait::async_trait;
use chrono::{DateTime, Local, Utc};
use common::grbl::GrblState;
use futures::{Future, io::Write, FutureExt, future::OptionFuture, pin_mut};
use serde::Serialize;
use tokio::{sync::{mpsc, oneshot, watch}, select, spawn, runtime::Handle, time::{sleep, timeout}};
use common::api::JobStatus;

#[derive(Debug)]
pub enum Message {
    GetState(oneshot::Sender<GrblStateInfo>),
    Write(WriteRequest),
    Comment(String),
    SetStatus(JobStatus),
}
#[derive(Debug)]
pub enum ImmediateMessage {
    GetState(oneshot::Sender<GrblStateInfo>),
    GetJobStatus(oneshot::Sender<watch::Receiver<Option<JobStatus>>>),
    Pause,
    Resume,
    Stop,
    Reset,
    OverrideSpeed(SpeedOverride),
    InitiateJob(oneshot::Sender<Option<JobHandle>>),
}
// ... if we wanted, we could go further and refactor out this logging functionality ...
#[derive(Clone, Debug)]
pub enum MachineDebugEvent {
    Sent(DateTime<Local>, Vec<u8>),
    Received(DateTime<Local>, String),
    Warning(DateTime<Local>, String),
    Comment(DateTime<Local>, String),
}


#[derive(Debug)]
pub struct JobFail;


#[derive(Debug)]
pub struct JobHandle {
    format_specification: Arc<GCodeFormatSpecification>,
    sender: mpsc::Sender<Message>,
    start_time: chrono::DateTime<Utc>,
}
impl JobHandle {
    pub async fn send_gcode(&self, gcode: GCodeLine) -> Result<impl Future<Output=Result<(), LineError>>, JobFail> {
        let bytes = format!("{}\n", self.format_specification.format_line(&gcode)).into_bytes();
        unsafe {  // Safe because we just formatted it.
            self.send_gcode_raw(bytes).await
        }
    }
    pub async fn send_probe_gcode(&self, gcode: GCodeLine) -> Result<(impl Future<Output=Result<(), LineError>>, impl Future<Output=Result<ProbeEvent, ProbeError>>), JobFail>  {
        let bytes = format!("{}\n", self.format_specification.format_line(&gcode)).into_bytes();
        let (line_tx, line_rx) = oneshot::channel();
        let (probe_tx, probe_rx) = oneshot::channel();
        self.sender.send(Message::Write(WriteRequest::Probe { data: bytes, result_line: line_tx, result: probe_tx })).await.map_err(|_| JobFail)?;
        Ok((line_rx.map(Result::unwrap), probe_rx.map(Result::unwrap)))
    }
    pub async fn send_comment(&self, message: String) -> Result<(), JobFail> {
        self.sender.send(Message::Comment(message)).await.map_err(|_| JobFail)?;
        Ok(())
    }
    pub async fn request_state(&self) -> Result<impl Future<Output=GrblStateInfo>, JobFail> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(Message::GetState(tx)).await.map_err(|_| JobFail)?;
        Ok(rx.map(Result::unwrap))
    } 
    pub async fn get_state(&self) -> Result<GrblStateInfo, JobFail> {
        Ok(self.request_state().await?.await)
    }
    pub async fn set_status(&self, status: String) -> Result<(), JobFail> {
        self.sender.send(Message::SetStatus(JobStatus { start_time: self.start_time, message: status })).await.map_err(|_| JobFail)?;
        Ok(())
    }
    /*
        Lower level functions (for debugging!)
    */
    pub async unsafe fn send_gcode_raw(&self, bytes: Vec<u8>) -> Result<impl Future<Output=Result<(), LineError>>, JobFail> {
        // Should have a \n after it!
        let (tx, rx) = oneshot::channel();
        self.sender.send(Message::Write(WriteRequest::Plain { data: bytes, result: tx })).await.map_err(|_| JobFail)?;

        Ok(rx.map(Result::unwrap))
    }

}
#[derive(Clone)]
pub struct ImmediateHandle {
    sender: mpsc::Sender<ImmediateMessage>
}
impl ImmediateHandle {
    pub async fn request_state(&self) -> impl Future<Output=GrblStateInfo>{
        let (tx, rx) = oneshot::channel();
        self.sender.send(ImmediateMessage::GetState(tx)).await.unwrap();
        rx.map(Result::unwrap)
    } 
    pub async fn get_state(&self) -> GrblStateInfo {
        self.request_state().await.await
    }
    pub async fn pause(&self) {
        self.sender.send(ImmediateMessage::Pause).await.unwrap()
    }
    pub async fn resume(&self) {
        self.sender.send(ImmediateMessage::Resume).await.unwrap()
    }
    pub async fn stop(&self) {
        self.sender.send(ImmediateMessage::Stop).await.unwrap()
    }
    pub async fn reset(&self) {
        self.sender.send(ImmediateMessage::Reset).await.unwrap()
    }
    pub async fn override_speed(&self, speed_override: SpeedOverride) {
        self.sender.send(ImmediateMessage::OverrideSpeed(speed_override)).await.unwrap()
    }
    pub async fn get_job_handle(&self) -> Option<JobHandle> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(ImmediateMessage::InitiateJob(tx)).await.unwrap();
        rx.await.unwrap()
    }
    pub async fn try_send_job<F, Fut>(&self, f: F) -> Result<(), F>
    where
        F: FnOnce(JobHandle) -> Fut,
        Fut: Future<Output=()> + Send + 'static    
    {
        match self.get_job_handle().await {
            Some(job_handle) => {
                spawn(f(job_handle));
                Ok(())
            }
            None => Err(f)
        }
    }
    pub async fn subscribe_job_status(&self) -> watch::Receiver<Option<JobStatus>> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(ImmediateMessage::GetJobStatus(tx)).await.unwrap();
        rx.await.unwrap()
    }
}

struct HandlerPrivateState {
    job_receiver: Option<mpsc::Receiver<Message>>,
    immediate_receiver: mpsc::Receiver<ImmediateMessage>,
}
struct HandlerSharedState {
    waiting_writes: FixedRb<WriteRequest, 4>,
    waiting_immediate: FixedRb<ImmediateRequest, 4>,
}
pub struct StandardHandler {
    format: Arc<GCodeFormatSpecification>,
    private_state: RefCell<HandlerPrivateState>,
    shared_state: RefCell<HandlerSharedState>,
    generation_counter: LocalGenerationCounter,

    debug_stream: history_broadcast::Sender<MachineDebugEvent>,
    job_status: watch::Sender<Option<JobStatus>>,
}
pub struct StandardHandlerParts {
    pub handler: StandardHandler,
    pub immediate_handle: ImmediateHandle,
    pub debug_rx: history_broadcast::Receiver<MachineDebugEvent>
}
/*
    Utilities for working with a fixed ring buffer...
*/


impl StandardHandler {
    pub fn create(format: GCodeFormatSpecification) -> StandardHandlerParts {
        let debug_tx = history_broadcast::Sender::new(256);
        let debug_rx = debug_tx.subscribe_with_history_count(0);
        let (immediate_tx, immediate_rx) = mpsc::channel(16);
        StandardHandlerParts {
            handler: StandardHandler {
                format: Arc::new(format),
                private_state: RefCell::new(HandlerPrivateState {
                    job_receiver: None,
                    immediate_receiver: immediate_rx,
                }),
                shared_state: RefCell::new(HandlerSharedState {
                    waiting_writes: FixedRb::new(),
                    waiting_immediate: FixedRb::new(),
                }),
                generation_counter: LocalGenerationCounter::new(),

                debug_stream: debug_tx,
                job_status: watch::channel(None).0,
            },
            immediate_handle: ImmediateHandle { sender: immediate_tx },
            debug_rx
        }
    }

    fn mutate<F: FnOnce(&mut HandlerSharedState) -> T, T>(&self, f: F) -> T {
        f(&mut *self.shared_state.borrow_mut())
    }
    fn mutate_and_advance<F: FnOnce(&mut HandlerSharedState) -> T, T>(&self, f: F) -> T {
        self.generation_counter.advance();
        self.mutate(f)
    }
    fn stop_job(&self, private: &mut HandlerPrivateState) {
        self.mutate(|inner| inner.waiting_writes.clear());
        drop(self.job_status.send(None));
        private.job_receiver = None;
    }

    async fn send_immediate_request(&self, mut request: ImmediateRequest) {
        loop {
            let result = self.mutate_and_advance(move |inner|
                inner.waiting_immediate.push(request)
            );
            match result {
                Ok(()) => return,
                Err(r) => request = r
            }
            (&self.generation_counter).await;
        }
    }
    async fn get_status(&self) -> Result<GrblStateInfo, oneshot::error::RecvError> {
        let (tx, rx) = oneshot::channel();
        self.send_immediate_request(ImmediateRequest::Status { result: tx }).await;
        rx.await
    }
    async fn graceful_halt(&self) {
        // Should be called only after a feed hold command has been issued!
        const TIME_TO_CONFIM_HALTING: Duration = Duration::from_millis(50);
        const TIME_TO_HALT: Duration = Duration::from_millis(600);

        // Shared state; used to check that the microcontroller has changed its state to the
        // halting for hold state. If not, we just send the reset anyways after TIME_TO_CONFIRM_HALTING.
        let has_confirmed_halting = Cell::new(false);
        // Future that gives us a chance to hold...
        let has_held = async {
            // Poll once and ignore to be sure to get past any batching of immediate commands
            let status = self.get_status().await;
            let status = match status {
                Ok(status) => status,
                Err(_) => return, // exit early, causing reset to be sent.
            };
            if status.state == (GrblState::Hold { code: 1 }) {
                has_confirmed_halting.set(true);
            }
            // Now, while Hold(1) is the state, loop to give the machine time to stop.
            // Typically takes ~8-9 ms per loop with FluidNC.
            loop {
                let status = self.get_status().await;
                match status {
                    Ok(state_info) => {
                        match state_info.state {
                            GrblState::Hold { code: 1 } => {
                                has_confirmed_halting.set(true);
                            },
                            _ => return
                        }
                    }
                    _ => return
                }
            }
        };
        pin_mut!(has_held);
        let final_deadline = sleep(TIME_TO_HALT);
        // For an initial period, check that the message was actually received; if we're doing something like
        // homing, we won't get confirmation and should 
        let wait_for_halt = select! {
            biased;
            _ = &mut has_held => false,
            _ = sleep(TIME_TO_CONFIM_HALTING) => has_confirmed_halting.get(),
        };
        // If allowed, we continue waiting either until the hard timeout or until has_held finishes.
        if wait_for_halt {
            select! {
                biased;
                _ = has_held => (),
                _ = final_deadline => (),
            }
        }
        self.send_immediate_request(ImmediateRequest::Reset).await;
    }
}

#[async_trait(?Send)]
impl Handler for StandardHandler {
    /*
        Logging...
    */
    async fn next_write_request(&self) -> WriteRequest {
        loop {
            if let Some(request) = self.mutate(|inner| inner.waiting_writes.pop()) {
                self.generation_counter.advance();  // Restart run loop!
                return request;
            }
            (&self.generation_counter).await;
        }
    }
    async fn next_immediate_request(&self) -> ImmediateRequest {
        loop {
            if let Some(request) = self.mutate(|inner| inner.waiting_immediate.pop()) {
                self.generation_counter.advance();  // Restart run loop!
                return request;
            }
            (&self.generation_counter).await;
        }
    }
    async fn run(&self) {
        let mut private = self.private_state.borrow_mut();
        let private = &mut *private;
        let halt_future = FutureOrPending::new(None);
        pin_mut!(halt_future);
        loop {
            select! {
                biased;
                immediate = private.immediate_receiver.recv(), if !self.mutate(|inner| inner.waiting_immediate.is_full()) => {
                    match immediate {
                        Some(ImmediateMessage::GetState(tx)) => {
                            self.mutate_and_advance(|inner|
                                inner.waiting_immediate.push(ImmediateRequest::Status{ result: tx }).unwrap()
                            );
                        }
                        Some(ImmediateMessage::GetJobStatus(tx)) => {
                            drop(tx.send(self.job_status.subscribe()))
                        }
                        Some(ImmediateMessage::Pause) => {
                            // TODO: Should perhaps discriminate based on current state & check that we really do stop (e.g. while homing!)
                            self.mutate_and_advance(|inner|
                                inner.waiting_immediate.push(ImmediateRequest::FeedHold).unwrap()
                            );
                        }
                        Some(ImmediateMessage::Resume) => {
                            if halt_future.is_none() {  // Disallow resumption if we're trying to halt.
                                self.mutate_and_advance(|inner|
                                    inner.waiting_immediate.push(ImmediateRequest::FeedResume).unwrap()
                                );
                            }
                        }
                        Some(ImmediateMessage::Stop) => {
                            self.stop_job(private);
                            // TODO: Also reset when ready!
                            self.mutate_and_advance(|inner|
                                inner.waiting_immediate.push(ImmediateRequest::FeedHold).unwrap()
                            );
                            halt_future.set(Some(self.graceful_halt()).into());
                        }
                        Some(ImmediateMessage::Reset) => {
                            // Remove the current job!
                            self.stop_job(private);
                            halt_future.set(None.into());
                            self.mutate_and_advance(|inner|
                                inner.waiting_immediate.push(ImmediateRequest::Reset).unwrap()
                            );
                        }
                        Some(ImmediateMessage::InitiateJob(tx)) => {
                            if private.job_receiver.is_some() || halt_future.is_some() {
                                drop(tx.send(None))
                            } else {
                                let (job_tx, job_rx) = mpsc::channel(16);
                                drop(tx.send(Some(JobHandle{
                                    format_specification: self.format.clone(),
                                    sender: job_tx,
                                    start_time: Utc::now()
                                })));
                                private.job_receiver = Some(job_rx);
                            }
                        },
                        Some(ImmediateMessage::OverrideSpeed(speed_override)) => {
                            self.mutate_and_advance(|inner|
                                inner.waiting_immediate.push(ImmediateRequest::OverrideSpeed(speed_override)).unwrap()
                            );
                        }
                        None => ()
                    }
                }
                line = FutureOrPending::from(private.job_receiver.as_mut().map(mpsc::Receiver::recv)), if self.mutate(|inner| !inner.waiting_writes.is_full() && !inner.waiting_immediate.is_full()) => {
                    match line {
                        Some(Message::GetState(tx)) => {
                            self.mutate_and_advance(|inner|
                                inner.waiting_immediate.push(ImmediateRequest::Status{ result: tx }).unwrap()
                            );
                        },
                        Some(Message::Write(write_request)) => {
                            self.mutate_and_advance(|inner|
                                inner.waiting_writes.push(write_request).unwrap()
                            );
                        },
                        Some(Message::Comment(message)) => {
                            self.debug_stream.send(MachineDebugEvent::Comment(Local::now(), message));
                        },
                        Some(Message::SetStatus(message)) => {
                            drop(self.job_status.send(Some(message)));
                        }
                        None => {
                            drop(self.job_status.send(None));
                            private.job_receiver = None;  // job hung up - must be done
                        }
                    }
                },
                _ = &mut halt_future => continue,
                _ = self.generation_counter.into_future() => continue  // If we get a signal to go on...
            }
        }
    }

    fn after_send(&self, bytes: Vec<u8>) {
        self.debug_stream.send(MachineDebugEvent::Sent(Local::now(), bytes))
    }
    fn after_receive(&self, line: String) {
        self.debug_stream.send(MachineDebugEvent::Received(Local::now(), line))
    }
    fn warn(&self, message: String) {
        self.debug_stream.send(MachineDebugEvent::Warning(Local::now(), message))
    }
}
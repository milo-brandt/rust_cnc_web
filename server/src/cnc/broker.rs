use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use async_trait::async_trait;
use axum::extract::ws::Message;
use futures::{Future, future::Fuse, FutureExt, Stream, StreamExt, pin_mut};
use tokio::{sync::{mpsc, watch}, spawn, task::JoinHandle, select};

use super::{grbl::new_machine::{ImmediateRequest, MachineInterface, MachineDebugEvent, WriteRequest}, gcode::parser::GeneralizedLineOwned};

pub struct MachineHandle {
    pub write_stream: mpsc::Sender<WriteRequest>,
    pub immediate_write_stream: mpsc::Sender<ImmediateRequest>
}
pub struct JobInnerHandle {
    pub command_stream: mpsc::Receiver<MessageToJob>,
    pub return_stream: mpsc::Sender<MessageFromJob>
}
pub struct JobOuterHandle {
    pub command_stream: mpsc::Sender<MessageToJob>,
    pub return_stream: mpsc::Receiver<MessageFromJob>
}

pub trait Job: Sized {
    fn run(self, handle: MachineHandle, job_handle: JobInnerHandle);
    fn begin(self, handle: MachineHandle) -> JobOuterHandle {
        let (command_stream_sender, command_stream_receiver) = mpsc::channel(128);
        let (return_stream_send, return_stream_receive) = mpsc::channel(128);
        self.run(handle, JobInnerHandle { command_stream: command_stream_receiver, return_stream: return_stream_send });
        JobOuterHandle { command_stream: command_stream_sender, return_stream: return_stream_receive }
    }
}

impl<F, JobFuture> Job for F
where
    F: FnOnce(MachineHandle, JobInnerHandle) -> JobFuture,
    JobFuture: Future<Output=()> + Send + 'static
{
    fn run(self, handle: MachineHandle, job_handle: JobInnerHandle) {
        let sender_copy = job_handle.return_stream.clone();
        let future = self(handle, job_handle);
        spawn(async move {
            future.await;
            drop(sender_copy.send(MessageFromJob::Complete).await);
        });
    }
}
struct StreamJob<S>(S);
impl<S> StreamJob<S>
where
    S: Stream<Item=GeneralizedLineOwned> + Send + 'static
{
    fn new(stream: S) -> Self {
        StreamJob(stream)
    }
}
impl<S> Job for StreamJob<S>
where
    S: Stream<Item=GeneralizedLineOwned> + Send + 'static
{
    fn run(self, handle: MachineHandle, mut job_handle: JobInnerHandle) {
        let sender_copy = job_handle.return_stream.clone();
        let stream = self.0;
        spawn(async move {
            pin_mut!(stream);
            let mut next_stream_future = stream.next();
            select! {
                next_value = &mut next_stream_future => {
                    match next_value {
                        Some(v) => {
                            next_stream_future = stream.next();
                        }
                        None => { return; }
                    }
                },
                message = job_handle.command_stream.recv() => {
                    match message {
                        Some(MessageToJob::RequestStop) => {
                            return;
                        }
                        _ => {},
                    }
                }
            };
        });
    }
}
#[derive(Debug)]
pub enum MessageToJob {
    FeedHeld, // Job is not responsible for holding feed; merely notified of it.
    FeedResumed, // Also not responsible for resuming feed.
    RequestStop
}
#[derive(Debug)]
pub enum MessageFromJob {
    Status(String),
    Complete
}

pub struct Broker {
    is_busy: Arc<AtomicBool>,
    last_status: watch::Receiver<String>,
    message_sender: Option<mpsc::Sender<MessageToJob>>,
    new_job_sender: mpsc::Sender<mpsc::Receiver<MessageFromJob>>,
    broker_task: JoinHandle<()>,
}
impl Broker {
    pub fn new() -> Self {
        let (new_job_sender, mut new_job_receiver) = mpsc::channel(8);
        let (watch_sender, watch_receiver) = watch::channel("Idle".to_string());
        let is_busy = Arc::new(AtomicBool::new(false));
        let is_busy_clone = is_busy.clone();
        let broker_task = spawn(async move {
            let mut current_job: Option<mpsc::Receiver<MessageFromJob>> = None;
            loop {
                select! {
                    new_job = new_job_receiver.recv() => {
                        if let Some(new_job) = new_job {
                            watch_sender.send_replace("Beginning new job...".to_string());
                            current_job = Some(new_job);
                        }
                    }
                    job_message = current_job.as_mut().map_or(Fuse::terminated(), |receiver| receiver.recv().fuse()) => {
                        match job_message {
                            Some(MessageFromJob::Complete) => {
                                is_busy_clone.store(false, Ordering::SeqCst);
                                watch_sender.send_replace("Idle".to_string());
                            },
                            Some(MessageFromJob::Status(status)) => {
                                watch_sender.send_replace(status);
                            },
                            _ => {}
                        };
                    }
                }
            }
        });
        Broker {
            is_busy,
            last_status: watch_receiver,
            message_sender: None,
            new_job_sender,
            broker_task
        }
    }
    pub fn try_send_job<J: Job>(&self, job: J, handle: MachineHandle) -> Result<(), J> {
        // Set self to busy; proceed only if previous value was false!
        if !self.is_busy.fetch_or(true, Ordering::SeqCst) {
            let job_handle = job.begin(handle);
            self.new_job_sender.try_send(job_handle.return_stream).expect("Sending should work!");
            Ok(())
        } else {
            Err(job)
        }
    }
    pub fn watch_status(&self) -> watch::Receiver<String> {
        self.last_status.clone()
    }
}

impl Default for Broker {
    fn default() -> Self {
        Self::new()
    }
}
unsafe impl Sync for Broker {}
impl Drop for Broker {
    fn drop(&mut self) {
        self.broker_task.abort();
    }
}
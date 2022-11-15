use {
    super::{
        gcode::parser::GeneralizedLineOwned,
        grbl::new_machine::{ImmediateRequest, WriteRequest},
    },
    crate::default_settings,
    futures::{future::Fuse, pin_mut, Future, FutureExt, Stream, StreamExt},
    std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    tokio::{
        select, spawn,
        sync::{mpsc, oneshot, watch},
        task::JoinHandle,
    },
};

pub struct MachineHandle {
    pub write_stream: mpsc::Sender<WriteRequest>,
    pub immediate_write_stream: mpsc::Sender<ImmediateRequest>,
}
pub struct JobInnerHandle {
    pub command_stream: mpsc::Receiver<MessageToJob>,
    pub return_stream: mpsc::Sender<MessageFromJob>,
}
pub struct JobOuterHandle {
    pub command_stream: mpsc::Sender<MessageToJob>,
    pub return_stream: mpsc::Receiver<MessageFromJob>,
}

pub trait Job: Sized {
    fn run(self, handle: MachineHandle, job_handle: JobInnerHandle);
    fn begin(self, handle: MachineHandle) -> JobOuterHandle {
        let (command_stream_sender, command_stream_receiver) = mpsc::channel(128);
        let (return_stream_send, return_stream_receive) = mpsc::channel(128);
        self.run(
            handle,
            JobInnerHandle {
                command_stream: command_stream_receiver,
                return_stream: return_stream_send,
            },
        );
        JobOuterHandle {
            command_stream: command_stream_sender,
            return_stream: return_stream_receive,
        }
    }
}

impl<F, JobFuture> Job for F
where
    F: FnOnce(MachineHandle, JobInnerHandle) -> JobFuture,
    JobFuture: Future<Output = ()> + Send + 'static,
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
pub struct StreamJob<S>(S, usize);
impl<S> StreamJob<S>
where
    S: Stream<Item = GeneralizedLineOwned> + Send + 'static,
{
    pub fn new(stream: S, size: usize) -> Self {
        StreamJob(stream, size)
    }
}
impl<S> Job for StreamJob<S>
where
    S: Stream<Item = GeneralizedLineOwned> + Send + 'static,
{
    fn run(self, handle: MachineHandle, mut job_handle: JobInnerHandle) {
        let sender_copy = job_handle.return_stream.clone();
        let stream = self.0;
        let total_lines = self.1;
        let spec = default_settings();
        spawn(async move {
            pin_mut!(stream);
            let mut next_stream_future = stream.next();
            let mut commands_done = false;
            let mut line_num = 0;
            loop {
                select! {
                    next_value = &mut next_stream_future => {
                        match next_value {
                            Some(v) => {
                                job_handle.return_stream.send(MessageFromJob::Status(format!("At line {}/{}", line_num, total_lines))).await.unwrap();
                                line_num += 1;
                                next_stream_future = stream.next();
                                match v {
                                    GeneralizedLineOwned::Line(line) => {
                                        let (sender, _receiver) = oneshot::channel();
                                        let data = format!("{}\n", spec.format_line(&line)).into_bytes();
                                        handle.write_stream.send(WriteRequest::Plain { data , result: sender }).await.unwrap();
                                        job_handle.return_stream.send(MessageFromJob::Status(format!("At line {}/{}", line_num, total_lines))).await.unwrap();
                                        /* if let Err(e) = receiver.await.unwrap() {
                                            handle.write_stream.send(WriteRequest::Comment(format!("Error in job! Line: {}\n{:?}\n", line_num + 1, e))).await.unwrap();
                                            break;
                                        } */ //don't wait for ok
                                    },
                                    GeneralizedLineOwned::Comment(comment) => handle.write_stream.send(WriteRequest::Comment(comment.to_string())).await.unwrap(),
                                    GeneralizedLineOwned::Empty => {},
                                }
                            }
                            None => { break }
                        }
                    },
                    message = job_handle.command_stream.recv(), if !commands_done => {
                        match message {
                            Some(MessageToJob::RequestStop) => {
                                break
                            }
                            Some(_) => {},
                            None => {
                                commands_done = true;
                            }
                        }
                    }
                }
            }
            drop(sender_copy.send(MessageFromJob::Complete).await);
        });
    }
}
#[derive(Debug)]
pub enum MessageToJob {
    FeedHeld,    // Job is not responsible for holding feed; merely notified of it.
    FeedResumed, // Also not responsible for resuming feed.
    RequestStop,
}
#[derive(Debug)]
pub enum MessageFromJob {
    Status(String),
    Complete,
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
            broker_task,
        }
    }
    pub fn try_send_job<J: Job>(&self, job: J, handle: MachineHandle) -> Result<(), J> {
        // Set self to busy; proceed only if previous value was false!
        if !self.is_busy.fetch_or(true, Ordering::SeqCst) {
            let job_handle = job.begin(handle);
            self.new_job_sender
                .try_send(job_handle.return_stream)
                .expect("Sending should work!");
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

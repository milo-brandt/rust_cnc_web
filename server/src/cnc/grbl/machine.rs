use {
    crate::util::history_broadcast,
    tokio::{
        io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
        select, spawn,
        sync::mpsc,
        task::JoinHandle,
    },
};

use std::str::from_utf8;

#[derive(Clone)]
pub enum MachineDebugEvent {
    Sent(String),
    Received(String),
}
pub struct Machine {
    debug_stream_receiver: history_broadcast::Receiver<MachineDebugEvent>,
    write_sender: mpsc::Sender<Vec<u8>>,
    handler_task: JoinHandle<()>,
}
impl Machine {
    pub fn new<
        Reader: AsyncRead + Unpin + Send + 'static,
        Writer: AsyncWrite + Unpin + Send + 'static,
    >(
        reader: Reader,
        mut writer: Writer,
    ) -> Self {
        let mut debug_stream = history_broadcast::Sender::<MachineDebugEvent>::new(64);
        let debug_stream_receiver = debug_stream.subscribe_with_history_count(0);
        let (write_sender, mut write_reader) = mpsc::channel::<Vec<u8>>(1024);
        let handler_task = {
            spawn(async move {
                let buffer = BufReader::new(reader);
                let mut lines = buffer.lines();
                loop {
                    select! {
                        next_line = lines.next_line() => {
                            if let Ok(Some(line)) = next_line {
                                debug_stream.send(MachineDebugEvent::Received(line.to_string()));
                            }
                        }
                        to_send = write_reader.recv() => {
                            if let Some(mut line) = to_send {
                                if line[0] == b'?' {
                                    writer.write_all(b"?").await.unwrap();
                                } else {
                                    writer.write_all(&line[..]).await.unwrap();
                                }
                                line.pop();
                                debug_stream.send(MachineDebugEvent::Sent(String::from(from_utf8(&line).unwrap())));
                            }
                        }
                    }
                }
            })
        };
        Machine {
            debug_stream_receiver,
            write_sender,
            handler_task,
        }
    }
    pub fn debug_stream_subscribe(&self) -> history_broadcast::Receiver<MachineDebugEvent> {
        self.debug_stream_receiver.subscribe_with_history_count(60)
    }
    pub fn get_write_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.write_sender.clone()
    }
}
impl Drop for Machine {
    fn drop(&mut self) {
        self.handler_task.abort();
    }
}

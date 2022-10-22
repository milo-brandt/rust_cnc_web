use tokio::{
    sync::{
        broadcast,
        mpsc,
    },
    io::{
        AsyncWrite,
        AsyncRead,
        AsyncWriteExt,
        AsyncReadExt,
        BufReader,
        AsyncBufReadExt,
    },
    task::JoinHandle,
    spawn
};
use crate::util::history_broadcast;
use super::messages::{
    GrblState, GrblPosition, GrblStatus, GrblMessage
};
use std::str::from_utf8;

pub struct Machine {
    raw_input: history_broadcast::Receiver<String>,
    raw_output: broadcast::Sender<String>,
    parsed_input: broadcast::Sender<GrblMessage>,
    write_sender: mpsc::Sender<Vec<u8>>,
    read_task: JoinHandle<()>,
    write_task: JoinHandle<()>,
}
impl Machine {
    pub fn new<Reader: AsyncRead + Unpin + Send + 'static, Writer: AsyncWrite + Unpin + Send + 'static>(reader: Reader, mut writer: Writer) -> Self {
        let mut raw_input = history_broadcast::Sender::<String>::new(256);
        let raw_input_receiver = raw_input.subscribe_with_history_count(0);
        let (raw_output, _) = broadcast::channel(1024);
        let (parsed_input, _) = broadcast::channel(1024);
        let read_task = {
            let mut parsed_input = parsed_input.clone();
            spawn(async move {
                let buffer = BufReader::new(reader);
                let mut lines = buffer.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    raw_input.send(line.to_string());
                    let line = crate::cnc::grbl::parser::parse_grbl_line(&line);
                    parsed_input.send(line);
                }
            })
        };
        let (write_sender, mut write_reader) = mpsc::channel::<Vec<u8>>(1024);
        let write_task = {
            let mut raw_output = raw_output.clone();
            spawn(async move {
                while let Some(mut line) = write_reader.recv().await {
                    writer.write(&line[..]).await.unwrap();
                    line.pop();
                    raw_output.send(String::from(from_utf8(&line).unwrap()));
                }
            })
        };
        Machine {
            raw_input: raw_input_receiver, raw_output, parsed_input, write_sender, read_task, write_task
        }
    }
    pub fn raw_input_subscribe(&self) -> history_broadcast::Receiver<String> {
        self.raw_input.subscribe_with_history_count(128)
    }
    pub fn raw_output_subscribe(&self) -> broadcast::Receiver<String> {
        self.raw_output.subscribe()
    }
    pub fn parsed_subscribe(&self) -> broadcast::Receiver<GrblMessage> {
        self.parsed_input.subscribe()
    }
    pub fn get_write_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.write_sender.clone()
    }
}
impl Drop for Machine {
    fn drop(&mut self) {
        self.read_task.abort();
        self.write_task.abort();
    }
}
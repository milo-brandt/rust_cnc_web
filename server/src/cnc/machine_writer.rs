use std::mem::MaybeUninit;

use async_trait::async_trait;
use ringbuf::LocalRb;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::util::fixed_rb::{FixedRb};

/*
    Struct for writing to Grbl, such that never more than a fixed amount of line-oriented
commands can be pending at once. Also provides a way for immediate commands to pass through.
*/
#[async_trait]
pub trait MachineWriter {
    async fn write_immediate(&mut self, bytes: Vec<u8>) -> Result<Vec<u8>, std::io::Error>;
    fn clear_unsent(&mut self);  // Clear unsent lines
    fn clear_waiting(&mut self);  // Clear unsent lines + any memory of sent ones
    fn can_enqueue_line(&mut self) -> bool;
    async fn flush(&mut self) -> Result<(), std::io::Error>;
    async fn enqueue_line(&mut self, bytes: Vec<u8>) -> Result<Option<Vec<u8>>, std::io::Error>;
    async fn pop_received_line(&mut self) -> Result<Option<Vec<u8>>, std::io::Error>;
}


pub struct BufferCountingWriter<Write> {
    write: Write,
    max_waiting_size: u16,
    waiting_size: u16,
    waiting_lines: FixedRb<u16, 8>,
    next_line: Option<Vec<u8>>,
}
#[async_trait]
impl<Write: AsyncWrite + Unpin + Send> MachineWriter for BufferCountingWriter<Write> {
    /*
        Public methods
    */
    async fn write_immediate(&mut self, bytes: Vec<u8>) -> Result<Vec<u8>, std::io::Error> { 
        // Write immediately with no checks. Should be used externally only for immediate commands.
        self.write.write_all(&bytes).await?;
        Ok(bytes)
    }
    fn clear_unsent(&mut self) {
        self.next_line = None;
    }
    fn clear_waiting(&mut self) {
        // Clear any pending writes and forget them.
        self.waiting_lines.clear();
        self.next_line = None;
        self.waiting_size = 0;
    }
    fn can_enqueue_line(&mut self) -> bool {
        // Precondition for enqueue_line.
        self.next_line.is_none()
    }
    async fn flush(&mut self) -> Result<(), std::io::Error> {
        self.write.flush().await
    }
    async fn enqueue_line(&mut self, bytes: Vec<u8>) -> Result<Option<Vec<u8>>, std::io::Error> {
        // Write a line if we can
        assert!(self.can_enqueue_line());  // Precondition. Will misbehave otherwise.
        let length = bytes.len() as u16;
        if self.can_write_line_immediate_with_length(length) {
            self.write_line_immediate(bytes, length).await.map(Some)
        } else {
            self.next_line = Some(bytes);
            Ok(None)
        }
    }
    async fn pop_received_line(&mut self) -> Result<Option<Vec<u8>>, std::io::Error> {
        // Signal that a line has been processed and its buffer space free for writing.
        // Should be called only after at least as many calls to enqueue_line; may panic otherwise.
        let last_length = self.waiting_lines.pop();
        let received_length = match last_length {
            Some(length) => length,
            None => {
                println!("Unexpected line popped! This should never happen!");
                return Ok(None)
            }
        };
        self.waiting_size -= received_length;
        if let Some(next_line) = &self.next_line {
            let length = next_line.len() as u16;
            if self.can_write_line_immediate_with_length(length) {
                let line = self.next_line.take();
                return self.write_line_immediate(line.unwrap(), length).await.map(Some);
            }
        }
        Ok(None)
    }
}
impl<Write> BufferCountingWriter<Write>
where
    Write: AsyncWrite + Unpin + Send
{
    pub fn new(write: Write, max_waiting_size: u16) -> Self {
        BufferCountingWriter {
            write,
            max_waiting_size,
            waiting_size: 0,
            waiting_lines: FixedRb::new(),
            next_line: None,
        }
    }
    /*
        Private internals
    */
    fn can_write_line_immediate_with_length(&mut self, length: u16) -> bool {
        return self.waiting_size + length <= self.max_waiting_size && !self.waiting_lines.is_full();
    }
    async fn write_line_immediate(&mut self, bytes: Vec<u8>, length: u16) -> Result<Vec<u8>, std::io::Error> {
        self.waiting_lines.push(length).unwrap();
        self.waiting_size += length;
        self.write_immediate(bytes).await
    }
}
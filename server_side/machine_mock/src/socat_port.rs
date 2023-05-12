use std::{time::Duration, future::{Future}, convert::Infallible, path::{Path, PathBuf}, sync::Mutex, borrow::BorrowMut, pin::Pin, task::Poll};

use notify::{Watcher, RecommendedWatcher, EventKind, event::CreateKind};
use tempdir::TempDir;
use tokio::{process::{Command, Child}, io::{AsyncRead, AsyncWrite, split, ReadHalf, WriteHalf}, sync::oneshot, join};
use tokio_serial::{
    self, DataBits, FlowControl, Parity, SerialPort, SerialPortBuilderExt, StopBits, SerialStream,
};
use anyhow::{Result, anyhow};

struct RetryFuture<F, Fut> {
    future_factory: F,
    future: Fut,
    retries_left: u64,
}
impl<F: FnMut() -> Fut, Fut: Future<Output=Result<Ok, Err>>, Ok, Err> Future for RetryFuture<F, Fut> {
    type Output = Result<Ok, Err>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let unpinned_self = unsafe { self.get_unchecked_mut() };
        loop {
            let fut = unsafe { Pin::new_unchecked(&mut unpinned_self.future) };
            match fut.poll(cx) {
                Poll::Ready(Ok(value)) => return Poll::Ready(Ok(value)),
                Poll::Ready(Err(_)) if unpinned_self.retries_left > 0 => {
                    println!("RETRYING!");
                    unpinned_self.retries_left -= 1;
                    unpinned_self.future = (unpinned_self.future_factory)()
                },
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

fn retry<F: FnMut() -> Result<Ok, Err>, Ok, Err>(mut result_factory: F, retries: u64) -> Result<Ok, Err> {
    loop {
        match result_factory() {
            Ok(value) => return Ok(value),
            Err(_) if retries > 0 => continue,
            Err(err) => return Err(err),
        }
    }
}

fn retry_future<F: FnMut() -> Fut, Fut: Future<Output=Result<Ok, Err>>, Ok, Err>(mut future_factory: F, retries: u64) -> RetryFuture<F, Fut> {
    let future = future_factory();
    RetryFuture { future_factory, future, retries_left: retries }
}

fn wait_for_file_creation_impl(path: PathBuf) -> Result<impl Future<Output=Result<()>>> {
    let end_of_path = path.file_name().ok_or(anyhow!("Invalid path: {:?}", path))?.to_owned();
    let directory = path.parent().ok_or(anyhow!("Invalid path: {:?}", path))?;
    let (sender, receiver) = oneshot::channel();
    let mut sender = Some(sender);
    let mut watcher = RecommendedWatcher::new(move |event: Result<notify::Event, notify::Error>| {
        match event {
            Ok(event) => {
                if let EventKind::Create(_) = event.kind {
                    if event.paths[0].ends_with(&end_of_path) {
                        if let Some(sender) = sender.take() {
                            drop(sender.send(()));
                        }
                    }
                }
            },
            Err(_) => todo!(),
        }
    }, notify::Config::default())?;
    watcher.watch(directory, notify::RecursiveMode::NonRecursive)?;
    Ok(async move {
        // Everything is set up by now; check if the file exists before doing anything
        let result = if path.exists() {
            Ok(())
        } else {
            receiver.await
        };
        drop(watcher);
        match result {
            Ok(()) => Ok(()),
            Err(_) => Err(anyhow!("Watched dropped unexpectedly while watching path: {:?}", path))
        }
    })
}

async fn wait_for_file_creation_no_retry(path: PathBuf) -> Result<()> {
    wait_for_file_creation_impl(path)?.await
}

async fn wait_for_file_creation(path: PathBuf) -> Result<()> {
    retry_future(|| wait_for_file_creation_no_retry(path.clone()), 10).await
}

async fn interrupt_child(mut child: Child) -> Result<()> {
    if let Some(pid) = child.id() {
        Command::new("kill").arg("-2").arg(pid.to_string()).spawn()?.wait().await?;
        child.wait().await?;
        Ok(())
    } else {
        Ok(())
    }
}

struct InterruptibleChild(Option<Child>);
impl InterruptibleChild {
    pub async fn interrupt(mut self) -> Result<()> {
        if let Some(child) = self.0.take() {
            interrupt_child(child).await
        } else {
            Ok(())
        }
    }
}
impl Drop for InterruptibleChild {
    fn drop(&mut self) {
        if let Some(child) = self.0.take() {
            tokio::spawn(interrupt_child(child));
        }
    }
}

async fn create_port_pair(first_path: &Path, second_path: &Path) -> Result<InterruptibleChild> {
    let child = Command::new("socat")
        .arg(format!("pty,raw,echo=0,link={}", first_path.to_string_lossy()))
        .arg(format!("pty,raw,echo=0,link={}", second_path.to_string_lossy()))
        .kill_on_drop(true) // fallback in case we fail to execute the graceful stop.
        .spawn()?;
    // Set the child up to interrupt on drop; it's okay if it doesn't, be undesirable.
    let child = InterruptibleChild(Some(child));
    let (first_file, second_file) = join!(
        wait_for_file_creation(first_path.to_owned()),
        wait_for_file_creation(second_path.to_owned()),
    );
    first_file?;
    second_file?;
    Ok(child)
}

fn open_port(path: &Path) -> Result<(ReadHalf<SerialStream>, WriteHalf<SerialStream>)> {
    let port = tokio_serial::new(path.to_string_lossy(), 115200)
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .timeout(Duration::from_millis(30))
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .open_native_async()?;
    Ok(split(port))
}

pub struct CommandPort {
    path: PathBuf,
    // For its lifetime only; never read. When dropped, sends a signal to kill the port.
    halt_sender: oneshot::Sender<Infallible>
}
impl CommandPort {
    pub fn get_path(&self) -> &Path {
        &self.path
    }
}

// Should only be called from within a tokio runtime.
pub async fn port_to_command<F: Future<Output=()> + Send + 'static>(f: impl FnOnce(ReadHalf<SerialStream>, WriteHalf<SerialStream>) -> F) -> Result<CommandPort> {
    let directory = TempDir::new("testing-port")?;
    let host_port_path = directory.path().join("host_port");
    let machine_port_path = directory.path().join("machine_port");
    let socat_process = create_port_pair(
        &host_port_path,
        &machine_port_path
    ).await?;
    let (machine_input, machine_output) = open_port(&machine_port_path)?;
    // start the process for reading the machine half of the port.
    let machine_task = tokio::spawn(f(machine_input, machine_output));
    // set up a process that, when halt_sender is dropped, kills socat and the child process.
    let (halt_sender, halt_receiver) = oneshot::channel();
    tokio::spawn(async move {
        drop(halt_receiver.await);
        // Stop the machine task and ensure it's exitted.
        machine_task.abort();
        drop(machine_task.await);
        // Then interrupt socat explicitly, which will also delete the temporary serial port.
        // Note that killing it won't close the files, so we need to interrupt instead.
        drop(socat_process.interrupt().await);
        // Finally, drop the directory; this is mostly to extend its lifetime until all is done.
        drop(directory);
    });
    
    Ok(CommandPort {
        path: host_port_path,
        halt_sender
    })
}

#[cfg(test)]
mod test {
    use std::pin::pin;

    use tokio::{io::{AsyncReadExt, AsyncWriteExt, BufReader, AsyncBufReadExt}, time::sleep};

    use super::*;

    #[tokio::test]
    async fn basic_port_to_command_test() {
        for i in 0..100000 {
            if i % 100 == 0 {
                println!("COUNT: {}", i);
            }
            // Simple echo port
            let port = port_to_command(|input, output| async move {
                let mut input = pin!(input);
                let mut output = pin!(output);
                loop {
                    let value = match input.read_u8().await {
                        Ok(value) => value,
                        Err(_) => return (),
                    };
                    if value >= b'a' && value <= b'y' {
                        output.write_u8(value + 1).await.unwrap();
                    } else {
                        output.write_u8(value).await.unwrap();
                    }
                }
            }).await.unwrap();
            let (input, mut output) = open_port(port.get_path()).unwrap();
            let mut input_lines = BufReader::new(input).lines();
            output.write_all(b"abc\n").await.unwrap();
            let result = input_lines.next_line().await.unwrap();
            assert_eq!(result, Some("bcd".into()));
        }
    }
}
/*    

}*/
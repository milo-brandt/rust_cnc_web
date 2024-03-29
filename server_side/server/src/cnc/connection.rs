use {
    std::time::Duration,
    tokio::{
        io::{split, stdin, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
        join,
        time::sleep,
    },
    tokio_serial::{
        self, DataBits, FlowControl, Parity, SerialPort, SerialPortBuilderExt, StopBits,
    },
};

// Seems difficult to actually get a serial port; probably better to allow a normal file/tty.
pub async fn open_and_reset_arduino_like_serial(path: &str) -> (impl AsyncRead, impl AsyncWrite) {
    let mut port = tokio_serial::new(path, 115200)
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .timeout(Duration::from_millis(30))
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .open_native_async()
        .expect("failed to open serial port :(");
    // Try to do some bit twiddling to signal an arduino-like device to reset.
    if port.write_data_terminal_ready(false).is_ok() {
        sleep(Duration::from_millis(2)).await;
        port.write_data_terminal_ready(true).expect("re-reset things");
    } else {
        // Report the error if not; probably not a big deal - happens in development environments.
        println!("DTR manipulation to reset arduino failed.");
    }
    split(port)
}

pub async fn as_terminal<Reader: AsyncRead + Unpin, Writer: AsyncWrite + Unpin>(
    reader: Reader,
    mut writer: Writer,
) {
    join!(
        async move {
            let buffer = BufReader::new(reader);
            let mut lines = buffer.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                println!("line = {:?}", line)
            }
        },
        async move {
            let buffer = BufReader::new(stdin());
            let mut lines = buffer.lines();
            while let Ok(Some(mut line)) = lines.next_line().await {
                println!("sending = {}", line);
                line.push('\n');
                writer.write_all(line.as_bytes()).await.unwrap();
            }
        }
    );
}

/*
pub async fn as_fake_terminal<Reader: AsyncRead + Unpin + Send + 'static, Writer: AsyncWrite + Unpin + Send + 'static>(reader: Reader, mut writer: Writer) {
    let mut machine = crate::cnc::grbl::machine::Machine::new(reader, writer);
    let mut parsed_receiver = machine.parsed_subscribe();
    let mut write_sender = machine.get_write_sender();
    join!(
        async move {
            while let Ok(line) = parsed_receiver.recv().await {
                println!("line = {:?}", line);
            }
        },
        async move {
            let buffer = BufReader::new(stdin());
            let mut lines = buffer.lines();
            while let Ok(Some(mut line)) = lines.next_line().await {
                println!("sending = {}", line);
                line.push('\n');
                write_sender.send(line.as_bytes().to_vec()).await.unwrap();
            }
        }
    );
}
*/

//     let (mut reader, mut writer) = split(port);
//     join!(
//         async move {
//             let mut buffer = BufReader::new(reader);
//             let mut lines = buffer.lines();
//             while let Ok(Some(line)) = lines.next_line().await {
//                 println!("line = {}", line)
//             }
//         },
//         async move {
//             let mut buffer = BufReader::new(stdin());
//             let mut lines = buffer.lines();
//             while let Ok(Some(mut line)) = lines.next_line().await {
//                 println!("sending = {}", line);
//                 line.push('\n');
//                 writer.write(line.as_bytes()).await.unwrap();
//             }
//         }
//     );
// }

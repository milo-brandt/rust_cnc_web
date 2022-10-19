use futures::future::select;
use tokio_serial::{self, FlowControl, DataBits, Parity, StopBits, SerialPortBuilderExt, SerialPort};
use std::time::Duration;
use tokio::{
    time::sleep,
    io::{
        AsyncWrite,
        AsyncRead,
        split,
    },
    io::stdin,
    io::BufReader,
    io::AsyncBufReadExt,
    io::AsyncWriteExt,
    task::JoinHandle,
    join
};

pub async fn open_and_reset_arduino_like_serial(path: &str) -> (impl AsyncRead, impl AsyncWrite) {
    let mut port = tokio_serial::new(path, 115200)
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .timeout(Duration::from_millis(30))
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .open_native_async().expect("failed to open serial port :(");
    port.write_data_terminal_ready(false).expect("reset things");
    sleep(Duration::from_millis(2)).await;
    port.write_data_terminal_ready(true).expect("re-reset things");
    split(port)
}
pub async fn as_terminal<Reader: AsyncRead + Unpin, Writer: AsyncWrite + Unpin>(reader: Reader, mut writer: Writer) {
    join!(
        async move {
            let buffer = BufReader::new(reader);
            let mut lines = buffer.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = crate::cnc::grbl::parser::parse_grbl_line(&line);
                println!("line = {:?}", line)
            }
        },
        async move {
            let buffer = BufReader::new(stdin());
            let mut lines = buffer.lines();
            while let Ok(Some(mut line)) = lines.next_line().await {
                println!("sending = {}", line);
                line.push('\n');
                writer.write(line.as_bytes()).await.unwrap();
            }
        }
    );
}


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

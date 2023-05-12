use std::{future::Future, pin::pin};

use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};

pub fn trivial_machine(input: impl AsyncRead, output: impl AsyncWrite) -> impl Future<Output=()> {
    async move {
        let mut input = pin!(input);
        let mut output = pin!(output);
        drop(output.write_all(b"Grbl v0.0.0 -- Fake\n").await);
        loop {
            let value = match input.read_u8().await {
                Ok(value) => value,
                Err(_) => return (),
            };
            match value {
                b'\n' => drop(output.write_all(b"ok\n").await),
                b'?' => drop(output.write_all(b"<Idle|MPos:0.00,1.00,3.00|Pn:XY|WCO:5.00,-5.25,17|FS:100,500|Bf:15,128|Ov:25,50,200|A:SM>\n").await),
                0x18 => drop(output.write_all(b"Grbl v0.0.0 -- Fake\n")),
                _ => (),
            }
        }
    }
}
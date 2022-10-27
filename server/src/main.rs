#![allow(dead_code)]

mod cnc;
mod util;

use std::str::from_utf8_unchecked;

use cnc::grbl::new_machine::{ImmediateRequest, WriteRequest};

use {
    axum::{
        extract::{
            ws::{Message, WebSocket, WebSocketUpgrade},
            RawBody,
        },
        response::Response,
        routing::{get, post},
        Extension, Router,
    },
    tower_http::cors::{Any, CorsLayer},
};

use {
    futures::{sink::SinkExt, stream::StreamExt},
    tokio::{join, sync::oneshot},
};

use {
    cnc::grbl::new_machine::{start_machine, MachineDebugEvent, MachineInterface},
    futures::stream::SplitStream,
    std::sync::Arc,
    tokio::select,
};

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods(Any)
        // allow requests from any origin... should maybe read in config
        .allow_origin(Any);
    // We should probably add some other authentication?
    // Maybe a header-to-cookie sort of deal?
    // Or double submit cookie?
    let (reader, writer) =
        cnc::connection::open_and_reset_arduino_like_serial("/dev/ttyUSB0").await;
    let machine = start_machine(reader, writer).await.unwrap();
    // build our application with a single route
    let app = Router::new()
        .route("/debug/send", post(index))
        .route("/debug/listen_raw", get(listen_raw))
        //.route("/ws", get(websocket_upgrade))
        .layer(cors)
        .layer(Extension(Arc::new(machine)));

    // run it with hyper on localhost:3000
    println!("Listening on port 3000...");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

/* async fn listen_raw(machine: Extension<Arc<Machine>>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    println!("Request to listen!");
    let mut input_receiver = machine.raw_input_subscribe();
    let mut output_receiver = machine.raw_output_subscribe();
    let result = stream! {
        loop {
            let (is_output, value) = select! {
                input = input_receiver.recv() => (false, input),
                output = output_receiver.recv() => (true, output)
            };
            if let Ok(string) = value {
                let prefix = if is_output { "> " } else { "< " };
                yield Event::default().data(format!("{}{}", prefix, string));
            }
        }
    }.map(Ok);

    Sse::new(result).keep_alive(KeepAlive::default())
} */

async fn listen_raw(ws: WebSocketUpgrade, machine: Extension<Arc<MachineInterface>>) -> Response {
    let mut debug_receiver = machine.debug_stream.subscribe_with_history_count(100);
    ws.on_upgrade(move |socket| async move {
        let (mut writer, mut reader) = socket.split();
        let (closer, mut close_listen) = oneshot::channel::<()>();
        let (writer, reader) = join! {
            async move {
                loop {
                    select! {
                        event = debug_receiver.recv() => {
                            let event = event.unwrap();
                            let message = match event {
                                MachineDebugEvent::Sent(str) => format!("> {}", unsafe { from_utf8_unchecked(&str) }),
                                MachineDebugEvent::Received(str) => format!("< {}", str),
                                MachineDebugEvent::Warning(str) => format!("! {}", str)
                            };
                            if writer.send(Message::Text(message)).await.is_err() {
                                break
                            }
                        }
                        _ = &mut close_listen => break
                    }
                }
                writer
            },
            async move {
                //ensure that this is actually getting read - so we can handle close frame!
                loop {
                    let response = <SplitStream<WebSocket> as StreamExt>::next(&mut reader).await;
                    if let Some(Ok(Message::Close(_))) = response {
                        closer.send(()).unwrap();
                        break
                    }
                    if response.is_none() {
                        break
                    }
                }
                reader
            }
        };
        let together = reader.reunite(writer).unwrap();
        drop(together.close().await);
    })
}

async fn index(message: RawBody, machine: Extension<Arc<MachineInterface>>) -> String {
    let mut body_bytes = hyper::body::to_bytes(message.0).await.unwrap().to_vec();
    if body_bytes.len() == 1 && body_bytes[0] == b'?' {
        let (sender, receiver) = oneshot::channel();
        machine
            .immediate_write_stream
            .send(ImmediateRequest::Status { result: sender })
            .await
            .unwrap();
        match receiver.await {
            Ok(result) => format!("{:?}", result),
            Err(_) => "Internal error immediate?".to_string(),
        }
    } else {
        body_bytes.push(b'\n');
        //let result = format!("Sent message: {}", from_utf8(&body_bytes).unwrap());
        let (sender, receiver) = oneshot::channel();
        machine
            .write_stream
            .send(WriteRequest::Plain {
                data: body_bytes,
                result: sender,
            })
            .await
            .unwrap();
        match receiver.await {
            Ok(Ok(())) => "Success!".to_string(),
            Ok(Err(id)) => format!("Failed: {}", id),
            Err(_) => "Internal error?".to_string(),
        }
    }
}

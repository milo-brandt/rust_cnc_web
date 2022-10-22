mod cnc;
mod util;
use axum::{
    routing::{get, post},
    Router,
    response::Response,
    Extension,
    response::sse::{Event, KeepAlive, Sse},
    extract::Path,
    extract::Json,
    extract::RawBody,
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
};
use tower_http::cors::{Any, CorsLayer};
use async_stream::stream;
use std::{time::Duration, convert::Infallible};
use tokio_stream::StreamExt as _ ;
use futures::{stream::{self, Stream, StreamExt}};
use tokio::join;
use tokio::sync::oneshot;
use futures::sink::SinkExt;
use tokio::sync::broadcast;
use futures::future::FutureExt;
use cnc::grbl::machine::{Machine, MachineDebugEvent};
use std::sync::Arc;
use std::str::from_utf8;
use tokio::select;
use futures::stream::SplitStream;

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
    let (reader, writer) = cnc::connection::open_and_reset_arduino_like_serial("/dev/ttyUSB0").await;
    let machine = Machine::new(reader, writer);
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

async fn listen_raw(ws: WebSocketUpgrade, machine: Extension<Arc<Machine>>) -> Response {
    let mut debug_receiver = machine.debug_stream_subscribe();
    ws.on_upgrade(move |mut socket| async move {
        let (mut writer, mut reader) = socket.split();
        let (mut closer, mut close_listen) = oneshot::channel::<()>();
        let (writer, reader) = join! {
            async move {
                loop {
                    select! {
                        event = debug_receiver.recv() => {
                            let event = event.unwrap();
                            let message = match event {
                                MachineDebugEvent::Sent(str) => format!("> {}", str),
                                MachineDebugEvent::Received(str) => format!("< {}", str),
                            };
                            if let Err(_) = writer.send(Message::Text(message)).await {
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
                        closer.send(());
                        break
                    }
                    if let None = response {
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

async fn index(message: RawBody, machine: Extension<Arc<Machine>>) -> String {
    let mut body_bytes = hyper::body::to_bytes(message.0).await.unwrap().to_vec();
    println!("Writing!");
    body_bytes.push(b'\n');
    let result = format!("Sent message: {}", from_utf8(&body_bytes).unwrap());
    drop(machine.get_write_sender().send(body_bytes).await);
    result
}

//use cnc::connection::SerialConnection;

/*
#[tokio::main]
async fn main() {
    let (reader, writer) = cnc::connection::open_and_reset_arduino_like_serial("/dev/ttyUSB0").await;
    cnc::connection::as_fake_terminal(reader, writer).await;
}
*/
mod cnc;

use axum::{
    routing::get,
    Router,
    response::Response,
    Extension,
    response::sse::{Event, KeepAlive, Sse},
    extract::Path,
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
};
use tower_http::cors::{Any, CorsLayer};
use async_stream::stream;
use std::{time::Duration, convert::Infallible};
use tokio_stream::StreamExt as _ ;
use futures::stream::{self, Stream};
use tokio::sync::broadcast;


// #[tokio::main]
// async fn main() {
//     let cors = CorsLayer::new()
//         // allow `GET` and `POST` when accessing the resource
//         .allow_methods(Any)
//         // allow requests from any origin... should maybe read in config
//         .allow_origin(Any);
//         // We should probably add some other authentication?
//         // Maybe a header-to-cookie sort of deal?
//         // Or double submit cookie?

//     let (tx, _rx) = broadcast::channel::<String>(16);
//     drop(_rx);
//     // build our application with a single route
//     let app = Router::new()
//         .route("/:message", get(index))
//         .route("/friend", get(sse_handler))
//         .route("/ws", get(websocket_upgrade))
//         .layer(cors)
//         .layer(Extension(tx));

//     // run it with hyper on localhost:3000
//     axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
//         .serve(app.into_make_service())
//         .await
//         .unwrap();
// }

// async fn sse_handler(sender: Extension<broadcast::Sender<String> >) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
//     let mut receiver = sender.subscribe();
//     let result = stream! {
//         for _x in 0..10 {
//             yield Event::default().data(receiver.recv().await.unwrap());
//         }
//     }.map(Ok);

//     Sse::new(result).keep_alive(KeepAlive::default())
// }

// async fn websocket_upgrade(ws: WebSocketUpgrade, sender: Extension<broadcast::Sender<String> >) -> Response {
//     let mut receiver = sender.subscribe();
//     ws.on_upgrade(move |socket| websocket_handler(socket, receiver))
// }

// async fn websocket_handler(mut ws: WebSocket, mut receiver: broadcast::Receiver<String>) {
//     loop {
//         let next_message = receiver.recv().await.unwrap();
//         drop(ws.send(Message::Text(next_message)).await);
//     }
// }

// async fn index(message: Path<String>, sender: Extension<broadcast::Sender<String> >) -> String {
//     drop(sender.send(message.clone())); // Ignore error if there are no listeners
//     format!("Sent message: \"{}\"\n", *message)
// }

//use cnc::connection::SerialConnection;

#[tokio::main]
async fn main() {
    let (reader, writer) = cnc::connection::open_and_reset_arduino_like_serial("/dev/ttyUSB0").await;
    cnc::connection::as_terminal(reader, writer).await;
}

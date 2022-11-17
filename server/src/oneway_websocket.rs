use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Json, RawBody, Multipart
    },
    response::Response
};
use futures::{Stream, StreamExt, SinkExt, pin_mut};
use tokio::{sync::oneshot, join, select};

pub fn send_stream<T: Stream<Item=Message> + Send + 'static>(ws: WebSocketUpgrade, stream: T) -> Response {
    ws.on_upgrade(move |socket| async move {
        let (mut writer, mut reader) = socket.split();
        let (closer, mut close_listen) = oneshot::channel::<()>();
        let (writer, reader) = join! {
            // Sending half of the loop.
            async move {
                pin_mut!(stream);
                let mut stream_next_future = stream.next();
                loop {
                    select! {
                        next_value = &mut stream_next_future => {
                            match next_value {
                                Some(message) => {
                                    if writer.send(message).await.is_err() {
                                        break;
                                    }
                                    stream_next_future = stream.next();
                                },
                                None => break,
                            }
                        }
                        _ = &mut close_listen => break
                    }
                }
                writer
            },
            // Read the websocket - mostly to ensure we actually handle the close frame.
            async move {
                loop {
                    let response = reader.next().await;
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
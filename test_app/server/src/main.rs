use axum::{Router, routing::{post, get}, Json, Server, extract::{WebSocketUpgrade, ws::{self}, State}, response::Response};
use futures::{join, StreamExt, SinkExt, future::ready, channel::oneshot, FutureExt, Stream, Sink, sink};
use protocol_types::{Message, RoomTx};
use protocol_util::{sender::Sender, receiver::{create_listener_full, FullListenerCreation}, communication_context::Context, channel_allocator::{ChannelAllocator, TypedChannelAllocator}, base::{ChannelCoStream, ChannelStream}, generic::{Receivable, Infallible, DefaultSendable}, spawner::Spawner};
use serde::{Serialize, Deserialize};
use tokio::{time::sleep, sync::broadcast};
use tokio_stream::wrappers::BroadcastStream;
use tower_http::{cors::{CorsLayer, Any}, catch_panic::CatchPanicLayer};
use std::{net::{Ipv4Addr, SocketAddr, IpAddr}, task::{self, Waker}, cell::{UnsafeCell, RefCell}, sync::{Mutex, Arc}, time::Duration, collections::HashMap, thread::JoinHandle};
use futures::channel::mpsc;
use std::future::Future;

// TODO: Probably want some way to close a room. Oh well - just a prototype.
pub struct Room {
    messages: broadcast::Sender<Message>,
}
pub struct SubscribeResult<Stream, Receive> {
    stream: Stream,
    receive: Receive,
}
impl Room {
    pub fn new() -> Self {
        Self {
            messages: broadcast::channel(16).0,
        }
    }
    pub fn subscribe(&self, username: String) -> SubscribeResult<impl Stream<Item=Message> + Send + 'static, impl FnMut(Option<String>) + Send + 'static> {
        let subscriber = BroadcastStream::new(self.messages.subscribe());
        let (stop_tx, stop_rx) = oneshot::channel::<std::convert::Infallible>();
        let bounded_subscribe = subscriber.filter_map(|input| ready(input.ok())).take_until(stop_rx);
        let sender = self.messages.clone();
        let mut stop_tx = Some(stop_tx);
        SubscribeResult { stream: bounded_subscribe, receive: move |message| {
            match message {
                Some(message) => drop(sender.send(Message { username: username.clone(), message })),
                None => {
                    stop_tx = None
                },
            }
        }}
    }
}

#[tokio::main]
async fn main() {
   let rooms = Arc::new(Mutex::new(HashMap::<String, Room>::new()));

   let cors = CorsLayer::new()
      .allow_methods(Any)
      .allow_origin(Any)
      .allow_headers(Any);

   let app = Router::new()
      .route("/ws", get(websocket))
      .with_state(rooms)
      .layer(CatchPanicLayer::new())
      .layer(cors);
   
   Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 3000))
      .serve(app.into_make_service())
      .await
      .unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
struct X {
   value: u64
}

fn fn_mut_to_sink<T, F: FnMut(T) + Send + 'static>(mut f: F) -> impl Sink<T> + Send + 'static {
    sink::unfold((), move |(), value| ready(Ok::<_, std::convert::Infallible>(f(value))))
}
pub async fn run_service(context: Context, state: Arc<Mutex<HashMap<String, Room>>>) {
    let echo_channel = ChannelStream::<protocol_types::JoinRoom>(context.channel_allocator.incoming());
    let mut echo_stream = echo_channel.receive_in_context(&context);

    context.spawner.spawn(
        async move {
            while let Some(next) = echo_stream.next().await {
                let SubscribeResult { stream, receive: mut send_message } = {
                    let mut lock = state.lock().unwrap();
                    let room = lock.entry(next.room_name).or_insert_with(|| Room::new());
                    room.subscribe(next.username)
                };
                next.result.channel_send(Some(RoomTx {
                    send_message: DefaultSendable(fn_mut_to_sink(move |m| send_message(Some(m)))),
                    messages: DefaultSendable(stream),
                }))
            }
        }
    );
}

async fn websocket(ws: WebSocketUpgrade, State(state): State<Arc<Mutex<HashMap<String, Room>>>>) -> Response {
    ws.on_upgrade(|ws| async move {
        // to share between the loops
        let (out_sender, mut out_receiver) = mpsc::unbounded();
        let sender = Sender::new(out_sender);
        let (mut ws_send, mut ws_receive) = ws.split();
        let FullListenerCreation {
            future,
            controller,
            sender: receiver,
        } = create_listener_full();
        let handles = Arc::new(Mutex::new(Some(Vec::new())));
        let context = Context {
            channel_allocator: Arc::new(TypedChannelAllocator::new()),
            controller,
            sender,
            spawner: Spawner::new({
                let handles = handles.clone();
                move |future| {
                    let join_handle = tokio::spawn(future);
                    if let Some(handles) = &mut *handles.lock().unwrap() {
                        handles.push(join_handle);
                    } else {
                        join_handle.abort();
                    }
                }
            })
        };
        context.spawner.spawn(run_service(context.clone(), state));
        context.spawner.spawn(future);
        // TODO: Fix shutdown here...
        let (ws_send, ws_receive) = join! {
            async move {
                loop {
                    match ws_receive.next().await {
                        Some(Ok(ws::Message::Text(text))) => {
                            if let Some((channel, message)) = text.split_once(':') {
                                if let Ok(channel_id) = channel.parse() {
                                    receiver.send(channel_id, message.into());
                                }
                            }
                        },
                        // TODO: Handle close messages correctly
                        Some(_) => (),
                        None => break
                    }
                }
                ws_receive
            },
            async move {
                while let Some(next_message) = out_receiver.next().await {
                    drop(ws_send.send(ws::Message::Text(format!("{}:{}", next_message.0, next_message.1))).await);
                }
                ws_send
            },
        };
        // Stop all tasks from this and prevent more from spawning.
        let tasks = handles.lock().unwrap().take();
        for task in tasks.unwrap() {
            task.abort();
        }
        let ws = ws_send.reunite(ws_receive).unwrap();
        // drop the websocket
        drop(ws.close().await);
    })
}



/*
Need to be able to spawn futures (possibly at the end of some window)...
...listeners can be attached immediately - don't need to be run...
*/
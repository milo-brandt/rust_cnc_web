use std::sync::Arc;

use futures::{StreamExt, join, SinkExt, FutureExt};
use futures::channel::{mpsc, oneshot};
use protocol_types::{JoinRoom, JoinRoomTx};
//use protocol_types::{EchoMessage, EchoMessageTx, EchoResponseRx};
use protocol_util::base::{ChannelCoStream, Channel};
use protocol_util::channel_allocator::TypedChannelAllocator;
use protocol_util::communication_context::Context;
use protocol_util::generic::{Receivable, DefaultSendable};
use protocol_util::receiver::{FullListenerCreation, create_listener_full};
use protocol_util::sender::Sender;
use protocol_util::spawner::Spawner;
use reqwasm::websocket;
use sycamore::prelude::*;
use sycamore::futures::{create_resource, spawn_local, spawn_local_scoped};
use reqwasm::websocket::futures::WebSocket;
use web_sys::{Event, KeyboardEvent};
use wasm_bindgen::JsCast;

fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    println!("HELLO!");
    let ws = WebSocket::open("ws://localhost:3000/ws").unwrap();

    let (out_sender, mut out_receiver) = mpsc::unbounded();
    let sender = Sender::new(out_sender);
    let (mut ws_send, mut ws_receive) = ws.split();
    let FullListenerCreation {
        future: service_future,
        controller,
        sender: receiver,
    } = create_listener_full();
    let context = Context {
        channel_allocator: Arc::new(TypedChannelAllocator::new()),
        controller,
        sender,
        spawner: Spawner::new(spawn_local)
    };
    spawn_local(async move {
        let (ws_send, ws_receive, _) = join! {
            async move {
                loop {
                    match ws_receive.next().await {
                        Some(Ok(websocket::Message::Text(text))) => {
                            if let Some((channel, message)) = text.split_once(':') {
                                if let Ok(channel_id) = channel.parse() {
                                    log::debug!("RECEIVING: {:?}", (channel_id, message));
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
                    log::debug!("SENDING: {:?}", next_message);
                    drop(ws_send.send(websocket::Message::Text(format!("{}:{}", next_message.0, next_message.1))).await);
                }
                ws_send
            },
            // run the listener
            service_future,
        };
    });
    let echo_co_channel = ChannelCoStream::<JoinRoom>(Channel(0, Default::default()));

    let echo_send = echo_co_channel.receive_in_context(&context);



    sycamore::render(|cx| {
        /* spawn_local_scoped(cx, async move {
            loop {
                sleep(Duration::from_secs(1)).await;
                log::debug!("Hello world! {:?}", ws.state());
            }
        }); */

        let input_value = create_signal(cx, String::new());
        let messages = create_signal(cx, Vec::<(String, String)>::new());
        let room = create_signal(cx, None);

        let echo_send = create_ref(cx, echo_send);

        let keydown_handler = {
            move |event: Event| {
                let keyboard_event: KeyboardEvent = event.unchecked_into();
                if keyboard_event.key() == "Enter" {
                    let line = &*input_value.get();
                    if room.get().is_none() {
                        let split = line.split_once("@");
                        if let Some((name, channel)) = split {
                            let (result_tx, result_rx) = oneshot::channel();
                            echo_send.channel_send(JoinRoomTx {
                                username: name.to_string(),
                                room_name: channel.to_string(),
                                result: result_tx,
                            });
                            spawn_local_scoped(cx, async move {
                                let room_result = result_rx.await.ok().flatten();
                                if let Some(room_result) = room_result {
                                    room.set(Some(room_result.send_message));
                                    messages.set(Vec::new());
                                    let mut message_channel = room_result.messages;
                                    let messages = messages.clone();
                                    while let Some(value) = message_channel.next().await {
                                        messages.modify().push((value.username, value.message));
                                    }
                                }
                            });
                        }
                    } else {
                        let room_value = room.get();
                        let room = room_value.as_ref().as_ref().unwrap();
                        room.channel_send(line.clone());
                    }
                    input_value.set("".to_string());                    
                }
            }
        };
    
        view! { cx,
            Indexed(
                iterable=messages,
                view=|cx, (user, message)| {
                    view! { cx, 
                        (user) ": " (message) br{} 
                    }
                }
            )
            div(class="input") {
                input(class="input_box", type="text", on:keydown=keydown_handler, bind:value=input_value) {

                }
            }
        }
    });
}

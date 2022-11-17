use std::mem::forget;
use std::sync::Arc;

use futures::future::Fuse;
use reqwasm::websocket::{futures::WebSocket, Message};
use reqwasm::http::Request;
use sycamore::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use futures::stream::StreamExt;
use futures::channel::oneshot;
use futures::{select, FutureExt};
use stylist::style;
use web_sys::{KeyboardEvent, Event};
use gloo_timers::future::sleep;
use std::time::Duration;
use sycamore::futures::spawn_local_scoped;

pub struct GlobalInfo<'a> {
    pub status: &'a ReadSignal<String>,
    pub is_idle: &'a ReadSignal<bool>
}

pub fn global_info<'a>(cx: Scope<'a>) -> &'a GlobalInfo<'a> {
    let message_list = create_signal(cx, "Waiting for connection...".to_string());
    let position = create_signal(cx, "".to_string());
    {
        spawn_local_scoped(cx, async move {
            let mut ws = WebSocket::open("ws://cnc:3000/debug/listen_status").unwrap();
            let mut ws2 = WebSocket::open("ws://cnc:3000/debug/listen_position").unwrap();
            let mut ws_next = ws.next().fuse();
            let mut ws2_next = ws2.next().fuse();
            loop {
                select! {
                    next_message = &mut ws_next => {
                        ws_next = ws.next().fuse();
                        match next_message {
                            Some(Ok(Message::Text(ws_message))) => {
                                log::debug!("Received status: {:?}", ws_message);
                                message_list.set(ws_message);
                            }
                            Some(_) => {
                                log::debug!("Received status: ???");
                            }
                            None => break
                        }
                    },
                    next_message = &mut ws2_next => {
                        ws2_next = ws2.next().fuse();
                        match next_message {
                            Some(Ok(Message::Text(ws_message))) => {
                                log::debug!("Received position status: {:?}", ws_message);
                                position.set(ws_message);
                            }
                            Some(_) => {
                                log::debug!("Received position status: ???");
                            }
                            None => break
                        }
                    },
                }
            }
            log::debug!("Closing status!");
            //drop(ws);
            let result = ws.close(None, None);
            log::debug!("Closed with result: {:?}", result);
            //let (forever_send, forever) = oneshot::channel::<()>();
            //forever.await.unwrap();
            //log::debug!("Oooh, bye!");
        });
    }
    create_ref(cx, GlobalInfo {
        status: create_memo(cx, || format!("{}\n{}", *message_list.get(), *position.get())),
        is_idle: create_memo(cx, move || *message_list.get() == "Idle")
    })
}


#[component]
pub fn StatusHeader(cx: Scope) -> View<DomNode> {
    let css_style = style! { r#"
        display: flex;
        flex-direction: column;
        height: 5vh;
        width: 100vw;
        align-items: stretch;
        background-color: gray;
    "#
    }.expect("CSS should work");
    log::debug!("CSS class: {}", css_style.get_class_name());
    let global_info: &GlobalInfo = use_context(cx);
    view! { cx,
        div(class=css_style.get_class_name()) {
            (global_info.status.get())
        }
    }
}
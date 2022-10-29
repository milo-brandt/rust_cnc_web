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
use crate::utils::async_sycamore;

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
    let (mut message_list_sender, message_list) = async_sycamore::create_channel(cx, "Waiting for connection...".to_string());
    {
        async_sycamore::spawn_local_drop_with_context(cx, async move {
            let mut ws = WebSocket::open("ws://cnc:3000/debug/listen_status").unwrap();
            let mut ws_next = ws.next().fuse();
            loop {
                select! {
                    next_message = &mut ws_next => {
                        ws_next = ws.next().fuse();
                        match next_message {
                            Some(Ok(Message::Text(ws_message))) => {
                                log::debug!("Received status: {:?}", ws_message);
                                message_list_sender.set(ws_message);
                            }
                            Some(_) => {
                                log::debug!("Received status: ???");
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
    view! { cx,
        div(class=css_style.get_class_name()) {
            (message_list.get())
        }
    }
}
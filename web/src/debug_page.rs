use std::io::Read;
use std::mem::forget;
use std::sync::Arc;

use futures::future::{Fuse, FusedFuture};
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

#[derive(Prop)]
pub struct DebugLineProps {
    #[builder(default)]
    text: String,
    class: String
}

#[component]
pub fn DebugLine(cx: Scope, props: DebugLineProps) -> View<DomNode> {
    view! { cx,
        div(class=props.class) {
            (props.text)
            br{}
        }
    }
}

#[component]
pub fn DebugPage(cx: Scope) -> View<DomNode> {
    let css_style = style! { r#"
        display: flex;
        flex-direction: column;
        height: 70vh;
        align-items: stretch;
        background-color: red;

        .history {
            background-color: black;
            font-weight: bold;
            padding-top: 0.5em;
            padding-left: 2em;
            padding-right: 2em;
            padding-bottom: 0.5em;
            flex-grow: 1;
            flex-shrink: 1;
            overflow: scroll;

            display: flex;
            flex-direction: column-reverse;
        }
        .history .outgoing {
            color: fuchsia;
        }
        .history .incoming {
            color: cyan;
        }
        .history .warning {
            color: red;
        }
        .history .comment {
            color: lime;
        }
        .input {
            flex-grow: 0;
            flex-shrink: 0;
            padding-top: 0em;
            padding-left: 0em;
            padding-right: 0em;
            padding-bottom: 0em;
            background-color: gray;
        }
        .input .input_box {
            padding-top: 0.5em;
            padding-left: 2em;
            padding-right: 2em;
            padding-bottom: 0.5em;
            color: white;
            font-family: inherit;
            font-size: inherit;
            width: 100%;
            background-color: transparent;
        }
    "#
    }.expect("CSS should work");
    log::debug!("CSS class: {}", css_style.get_class_name());
    let (mut message_list_sender, message_list) = async_sycamore::create_channel(cx, vec![]);
    {
        async_sycamore::spawn_local_drop_with_context(cx, async move {
            let mut ws = WebSocket::open("ws://cnc:3000/debug/listen_raw").unwrap();
            let mut ws_next = ws.next().fuse();
            let mut values = vec![];
            let mut next_update = Fuse::terminated(); //sleep(Duration::from_millis(0)).fuse();
            loop {
                select! {
                    next_message = &mut ws_next => {
                        ws_next = ws.next().fuse();
                        match next_message {
                            Some(Ok(Message::Text(ws_message))) => {
                                values.push(ws_message);
                                if next_update.is_terminated() {
                                    next_update = sleep(Duration::from_millis(10)).fuse();
                                }
                            }
                            None => break
                        }
                    },
                    _ = &mut next_update => {
                        message_list_sender.set(values.clone());
                    }
                }
            }
            log::debug!("Closing!");
            //drop(ws);
            let result = ws.close(None, None);
            log::debug!("Closed with result: {:?}", result);
            //let (forever_send, forever) = oneshot::channel::<()>();
            //forever.await.unwrap();
            //log::debug!("Oooh, bye!");
        });
    }
    let message_list_inner = create_memo(cx, move || {
        let mut v = (*message_list.get()).clone();
        v.reverse();
        v
    });
    let input_value = create_signal(cx, String::new());
    let is_checked = create_signal(cx, true);
    let keydown_handler = |event: Event| {
        let keyboard_event: KeyboardEvent = event.unchecked_into();
        if keyboard_event.key() == "Enter" {
            let line = &*input_value.get();
            let request = Request::post("http://cnc:3000/debug/send")
                .body(line)
                .send();
            log::debug!("Sending!");
            spawn_local(async move {
                log::debug!("Inside!");
                let result = request.await.expect("Request should go through!");
                log::debug!("Sent! {:?}", result);
            });
            input_value.set("".to_string());
        }
    };
    let list = create_memo(cx, || if *is_checked.get() {
        message_list_inner.get().iter().filter(|x| !x.starts_with("< <") && !x.starts_with("> ?")).map(String::from).collect()
    } else {
        (*message_list_inner.get()).clone()
    });
    view! { cx,
        div(class=css_style.get_class_name()) {
            div(class="history") {
                Indexed(
                    iterable=list,
                    view=|cx, x| {
                        let class_name = if x.starts_with("> ") {
                            "outgoing"
                        } else if x.starts_with("! ") {
                            "warning"
                        } else if x.starts_with("~ ") {
                            "comment"
                        } else {
                            "incoming"
                        };
                        view! { cx,
                        DebugLine(text=x, class=class_name.to_string())
                    }
                    }
                )
            }
            div(class="input") {
                input(class="input_box", type="text", on:keydown=keydown_handler, bind:value=input_value) {

                }
            }
        }
        div {
            input(type="checkbox", bind:checked=is_checked)
            label { "Hide status queries." }
        }
        a(href="/") { "Go home!" }
    }
}
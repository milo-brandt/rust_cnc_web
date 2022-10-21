use std::mem::forget;
use std::sync::Arc;

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
        height: 80vh;
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
    let (stop_send, mut stop_receive) = oneshot::channel::<()>();
    let message_list = create_rc_signal(vec![]);
    {
        let message_list = message_list.clone();
        spawn_local(async move {
            let mut ws = WebSocket::open("ws://cnc:3000/debug/listen_raw").unwrap();
            let mut ws_next = ws.next().fuse();
            loop {
                select! {
                    next_message = ws_next => {
                        ws_next = ws.next().fuse();
                        match next_message {
                            Some(Ok(Message::Text(ws_message))) => {
                                log::debug!("Received: {:?}", ws_message);
                                let mut v = (*message_list.get()).clone();
                                v.push(ws_message);
                                message_list.set(v);
                            }
                            Some(_) => {
                                log::debug!("Received: ???");
                            }
                            None => break
                        }
                    }
                    _ = stop_receive => break
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
    on_cleanup(cx, move || drop(stop_send.send(())));
    let input_value = create_signal(cx, String::new());
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

    view! { cx,
        div(class=css_style.get_class_name()) {
            div(class="history") {
                Indexed(
                    iterable=message_list_inner,
                    view=|cx, x| {
                        let class_name = if x.starts_with("> ") {
                            "outgoing"
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
        a(href="/") { "Go home!" }
    }
}
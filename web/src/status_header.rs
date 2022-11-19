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
use common::grbl::GrblState;

pub struct GlobalInfo<'a> {
    pub grbl_info: &'a ReadSignal<Option<common::grbl::GrblFullInfo>>,
    pub job_info: &'a ReadSignal<String>,
    pub is_idle: &'a ReadSignal<bool>
}

pub fn global_info<'a>(cx: Scope<'a>) -> &'a GlobalInfo<'a> {
    let job_info = create_signal(cx, "Waiting for connection...".to_string());
    let grbl_info = create_signal(cx, None);
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
                            job_info.set(ws_message);
                        }
                        _ => break
                    }
                },
                next_message = &mut ws2_next => {
                    ws2_next = ws2.next().fuse();
                    match next_message {
                        Some(Ok(Message::Text(ws_message))) => {
                            let value: common::grbl::GrblFullInfo = serde_json::from_str(&ws_message).unwrap();
                            grbl_info.set(Some(value));
                        }
                        _ => break
                    }
                },
            }
        }
        //drop(ws);
        ws.close(None, None).unwrap();
        ws2.close(None, None).unwrap();
        //let (forever_send, forever) = oneshot::channel::<()>();
        //forever.await.unwrap();
        //log::debug!("Oooh, bye!");
    });
    create_ref(cx, GlobalInfo {
        grbl_info,
        job_info,
        is_idle: create_memo(cx, move || *job_info.get() == "Idle")
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
    let in_motion = create_selector(cx, || {
        (&*global_info.grbl_info.get()).as_ref().map_or(true, |v| if let GrblState::Hold(_) = v.state { false } else { true })
    });
    let on_click = create_ref(cx, |_event| {
        let url = if *in_motion.get() {
            "http://cnc:3000/command/feed_hold"
        } else {
            "http://cnc:3000/command/feed_resume"
        };
        spawn_local(async{
            let result = Request::post(url).send().await;
            log::debug!("Result: {:?}", result);
        })
    });
    view! { cx,
        div(class=css_style.get_class_name()) {
            ({
                let value = &*global_info.grbl_info.get();
                let x: Option<&common::grbl::GrblFullInfo> = value.as_ref();
                x.map_or("No!".to_string(), |v| format!("{:?} {:?} {}", v.state, v.work_position(), (*global_info.job_info.get())))
            }) br {}
            button(on:click=on_click) {
                (if *in_motion.get() {
                    "Stop"
                } else {
                    "Start"
                })
            }
        }
    }
}
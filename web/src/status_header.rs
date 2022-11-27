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

use crate::mdc::IconButton;

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
pub fn OverrideController(cx: Scope) -> View<DomNode> {
    let global_info: &GlobalInfo = use_context(cx);
    let callback_for_url = move |url: String| {
        create_ref(cx, move || {
            let url = url.clone();
            spawn_local(async move {
                let result = Request::post(&url).send().await;
                log::debug!("Result: {:?}", result);
            })    
        })
    };
    let feed_reset = callback_for_url("http://cnc:3000/command/override/feed/reset".into());
    let feed_increase_10 = callback_for_url("http://cnc:3000/command/override/feed/plus10".into());
    let feed_increase_1 = callback_for_url("http://cnc:3000/command/override/feed/plus1".into());
    let feed_decrease_1 = callback_for_url("http://cnc:3000/command/override/feed/minus1".into());
    let feed_decrease_10 = callback_for_url("http://cnc:3000/command/override/feed/minus10".into());

    let status = create_selector(cx, move || {
        //let value = &*global_info.grbl_info.get();
        //let x: Option<&common::grbl::GrblFullInfo> = value.as_ref();
        //x.map_or("???".to_string(), |v| format!("{}", v.feed_override))
        "Hello".to_string()
    });

    view! { cx,
        div {
            IconButton(icon_name=create_signal(cx, "keyboard_double_arrow_down".into()), on_click=feed_decrease_10)
            IconButton(icon_name=create_signal(cx, "keyboard_arrow_down".into()), on_click=feed_decrease_1)
            ({
                let value = &*global_info.grbl_info.get();
                let x: Option<&common::grbl::GrblFullInfo> = value.as_ref();
                x.map_or("???".to_string(), |v| v.feed_override.to_string())
            })
            IconButton(icon_name=create_signal(cx, "restart_alt".into()), on_click=feed_reset)
            IconButton(icon_name=create_signal(cx, "keyboard_arrow_up".into()), on_click=feed_increase_1)
            IconButton(icon_name=create_signal(cx, "keyboard_double_arrow_up".into()), on_click=feed_increase_10)

        }
    }
}

#[component]
pub fn StatusHeader(cx: Scope) -> View<DomNode> {
    let css_style = style! { r#"
        display: flex;
        flex-direction: column;
        height: 15vh;
        width: 100vw;
        align-items: stretch;
        background-color: gray;
        div button {
            border: none;
            background: none;
            cursor: pointer;
        }
        div button img {
            height: 2rem;
        }
    "#
    }.expect("CSS should work");
    log::debug!("CSS class: {}", css_style.get_class_name());
    let global_info: &GlobalInfo = use_context(cx);
    let in_motion = create_selector(cx, || {
        (&*global_info.grbl_info.get()).as_ref().map_or(true, |v| if let GrblState::Hold(_) = v.state { false } else { true })
    });
    let on_click = create_ref(cx, || {
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
    let stop = create_ref(cx, || {
        spawn_local(async{
            let result = Request::post("http://cnc:3000/command/stop").send().await;
            log::debug!("Result: {:?}", result);
        })
    });
    let reset = create_ref(cx, || {
        spawn_local(async{
            let result = Request::post("http://cnc:3000/command/reset").send().await;
            log::debug!("Result: {:?}", result);
        })
    });
    let unlock = create_ref(cx, || {
        let line = "$X";
        let request = Request::post("http://cnc:3000/debug/send")
            .body(line)
            .send();
        log::debug!("Sending!");
        spawn_local(async move {
            log::debug!("Inside unlock!");
            let result = request.await.expect("Request should go through!");
            log::debug!("Sent unlock! {:?}", result);
        });
    });
    let home_disabled = create_selector(cx, || {
        (&*global_info.grbl_info.get()).as_ref().map_or(true, |v| {
            match v.state {
                GrblState::Idle => false,
                GrblState::Run => false,
                GrblState::Hold(_) => true,
                GrblState::Jog => false,
                GrblState::Alarm => false,
                GrblState::Door(_) => true,
                GrblState::Check => true,
                GrblState::Home => true,
                GrblState::Sleep => true,
            }            
        })
    });
    let home = create_ref(cx, || {
        let line = "\\$H";
        let request = Request::post("http://cnc:3000/debug/send")
            .body(line)
            .send();
        log::debug!("Sending!");
        spawn_local(async move {
            log::debug!("Inside home!");
            let result = request.await.expect("Request should go through!");
            log::debug!("Sent home! {:?}", result);
        });
    });
    let button_kind = create_selector(cx, || {
        if *in_motion.get() {
            "stop"
        } else {
            "play_arrow"
        }.to_string()
    });
    let button_disabled = create_selector(cx, || {
        (&*global_info.grbl_info.get()).as_ref().map_or(true, |v| {
            match v.state {
                GrblState::Idle => false,
                GrblState::Run => false,
                GrblState::Hold(_) => false,
                GrblState::Jog => false,
                GrblState::Alarm => true,
                GrblState::Door(_) => true,
                GrblState::Check => true,
                GrblState::Home => true,
                GrblState::Sleep => true,
            }            
        })
    });
    let unlock_disabled = create_selector(cx, || {
        (&*global_info.grbl_info.get()).as_ref().map_or(true, |v| {
            match v.state {
                GrblState::Alarm => false,
                _ => true,
            }            
        })
    });
    view! { cx,
        div(class=css_style.get_class_name()) {
            ({
                let value = &*global_info.grbl_info.get();
                let x: Option<&common::grbl::GrblFullInfo> = value.as_ref();
                x.map_or("No!".to_string(), |v| format!("{:?} {:?} {}", v.state, v.work_position(), (*global_info.job_info.get())))
            }) br {}
            div {
                IconButton(icon_name=button_kind, on_click=on_click, disabled=button_disabled)
                IconButton(icon_name=create_signal(cx, "lock_open".to_string()), on_click=unlock, disabled=unlock_disabled)
                IconButton(icon_name=create_signal(cx, "restart_alt".to_string()), on_click=stop)
                IconButton(icon_name=create_signal(cx, "home".to_string()), on_click=home, disabled=home_disabled)
                IconButton(icon_name=create_signal(cx, "cancel".to_string()), on_click=reset)
            } br {}
            OverrideController
        }
    }
}
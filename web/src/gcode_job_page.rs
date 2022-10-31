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
use sycamore::futures::create_resource;
use crate::utils::async_sycamore;


#[derive(Prop)]
pub struct GcodeFileProps {
    name: String,
}

#[component]
pub fn GcodeFile(cx: Scope, props: GcodeFileProps) -> View<DomNode> {
    let name = create_ref(cx, props.name);
    let run_callback = create_ref(cx, |_| {
        let name = name.clone();
        spawn_local(async move {
            log::debug!("Sending job!");
            let result = Request::post("http://cnc:3000/job/run_file")
            .body(format!("{{\"path\":\"{}\"}}", name))
            .header("Content-Type", "application/json")
            .send()
            .await;
            log::debug!("Result: {:?}", result);
        });
    });
    view! { cx,
        div(class="gcode_line") {
            (name.clone()) " "
            button(on:click=run_callback) { "Run!" }
        }
    }
}

#[component]
pub fn GCodePage(cx: Scope) -> View<DomNode> {
    let list = create_resource(cx, async {
        let result = Request::get("http://cnc:3000/list_files")
        .send()
        .await
        .unwrap();
        let names: Vec<String> = result.json().await.unwrap();
        names
    });
    let list = create_memo(cx, move || (*list.get()).clone().unwrap_or(Vec::new()));
    view! { cx,
        div(class="debug_page") {
            Indexed(
                iterable=list,
                view=|cx, x| view! { cx,
                    GcodeFile(name=x)
                }
            )
        }
        a(href="/") { "Go home!" }
    }
}

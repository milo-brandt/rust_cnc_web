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
use crate::status_header::GlobalInfo;
use crate::utils::async_sycamore;


#[derive(Prop)]
pub struct GcodeFileProps<'a> {
    name: String,
    can_send_job: &'a ReadSignal<bool>
}

#[component]
pub fn GcodeFile<'a>(cx: Scope<'a>, props: GcodeFileProps<'a>) -> View<DomNode> {
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
            button(on:click=run_callback, disabled=!*props.can_send_job.get()) { "Run!" }
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
    let global_info: &GlobalInfo = use_context(cx);
    let list = create_memo(cx, move || (*list.get()).clone().unwrap_or(Vec::new()));
    view! { cx,
        div(class="debug_page") {
            Indexed(
                iterable=list,
                view=move |cx, x| view! { cx,
                    GcodeFile(name=x, can_send_job=global_info.is_idle)
                }
            )
        }
        a(href="/") { "Go home!" }
    }
}

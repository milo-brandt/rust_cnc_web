use std::io::Read;
use std::mem::forget;
use std::sync::Arc;

use futures::future::{Fuse, FusedFuture};
use reqwasm::websocket::{futures::WebSocket, Message};
use reqwasm::http::{FormData, Request};
use sycamore::prelude::*;
use sycamore::web::html::{input, form};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use futures::stream::StreamExt;
use futures::channel::oneshot;
use futures::{select, FutureExt};
use stylist::style;
use web_sys::{KeyboardEvent, Event, HtmlInputElement};
use gloo_timers::future::sleep;
use std::time::Duration;
use sycamore::futures::{create_resource, spawn_local_scoped};
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


#[derive(Prop)]
pub struct GCodeUploadProps<F: Fn() -> ()> {
    on_upload: F,
}

#[component]
pub fn GCodeUpload<'a, F: Fn() -> () + 'a>(cx: Scope<'a>, props: GCodeUploadProps<F>) -> View<DomNode> {
    let node_ref = create_node_ref(cx);
    let on_upload = create_ref(cx, props.on_upload);
    let run_callback = create_ref(cx, move |_| {
        spawn_local_scoped(cx, async move {
            log::debug!("uploading!");
            log::debug!("{:?}", node_ref);
            let node: DomNode = node_ref.get();
            let input_node: HtmlInputElement = node.unchecked_into();
            if let Some(file) = input_node.files().and_then(|files| files.item(0)) {
                log::debug!("{:?} {}", file, file.name());
                let form_data = FormData::new().unwrap();
                form_data.append_with_str("filename", &file.name()).unwrap();
                form_data.append_with_blob_and_filename("file", &file, "filename.nc").unwrap();
                let result = Request::post("http://cnc:3000/job/upload_file")
                .body(form_data)
                .send()
                .await;
                log::debug!("Result: {:?}", result);
                on_upload()
            }
            /*let result = Request::post("http://cnc:3000/job/run_file")
            .body(format!("{{\"path\":\"{}\"}}", name))
            .header("Content-Type", "application/json")
            .send()
            .await;*/
            log::debug!("welp");
        });
    });
    view! { cx,
        input(ref=node_ref, type="file") {} br{}
        button(on:click=run_callback) { "Upload" }
    }
}


#[component]
pub fn GCodePage(cx: Scope) -> View<DomNode> {
    let list = create_signal(cx, None);
    let get_list = || async {
        // TODO: Probably want some sort of debounce here?
        let result = Request::get("http://cnc:3000/list_files")
        .send()
        .await
        .unwrap();
        let names: Vec<String> = result.json().await.unwrap();
        list.set(Some(names));
    };
    spawn_local_scoped(cx, get_list());
    let global_info: &GlobalInfo = use_context(cx);
    //let list = create_memo(cx, move || (*list.get()).clone().unwrap_or(Vec::new()));
    view! { cx,
        (if list.get().is_none() {
            view! { cx, 
                "Loading..."
            }
        } else {
            let list = list.clone();
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
            }
        })
        GCodeUpload(on_upload=move || spawn_local_scoped(cx, get_list())) br{}
        a(href="/") { "Go home!" }
    }
}

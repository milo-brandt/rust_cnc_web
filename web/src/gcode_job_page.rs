use std::io::Read;
use std::mem::forget;
use std::sync::Arc;

use common::api::{self, RunGcodeFile, DeleteGcodeFile};
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
use crate::request::{self, HttpMethod};
use crate::status_header::GlobalInfo;
use crate::utils::async_sycamore;


#[derive(Prop)]
pub struct GcodeFileProps<'a, F: Fn() -> ()> {
    name: String,
    can_send_job: &'a ReadSignal<bool>,
    on_delete: F,
}

#[component]
pub fn GcodeFile<'a, F: Fn() -> () + 'a>(cx: Scope<'a>, props: GcodeFileProps<'a, F>) -> View<DomNode> {
    let name = create_ref(cx, props.name);
    let run_callback = create_ref(cx, |_| {
        request::request_detached_with_json(
            HttpMethod::Post,
            api::RUN_GCODE_FILE,
            &RunGcodeFile { path: name.clone() }
        );
    });
    let on_delete = create_ref(cx, props.on_delete);
    let delete_callback = create_ref(cx, move |_| {
        spawn_local_scoped(cx, async {
            request::request_with_json(
                HttpMethod::Delete,  // Should really have method bundled in...
                api::DELETE_GCODE_FILE,
                &DeleteGcodeFile { path: name.clone() }
            ).await.unwrap();
            on_delete();
        });
    });
    view! { cx,
        div(class="gcode_line") {
            (name.clone()) " "
            button(on:click=run_callback, disabled=!*props.can_send_job.get()) { "Run!" }
            button(on:click=delete_callback, disabled=!*props.can_send_job.get()) { "Delete!" }
            a(href=format!("/view/{}", name)) { "View!" }
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
                let result = request::request_with_body(
                    HttpMethod::Post, 
                    api::UPLOAD_GCODE_FILE, 
                    form_data,
                ).await;
                log::debug!("Result: {:?}", result);
                on_upload()
            }
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
        let result = request::request(
            HttpMethod::Get,
            api::LIST_GCODE_FILES,
        ).await.unwrap();
        // TODO: Would be nice to wrap the un-jsoning in the request somehow...
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
                            GcodeFile(name=x, can_send_job=global_info.is_idle, on_delete=move || spawn_local_scoped(cx, get_list()))
                        }
                    )
                }
            }
        })
        GCodeUpload(on_upload=move || spawn_local_scoped(cx, get_list())) br{}
        a(href="/") { "Go home!" }
    }
}

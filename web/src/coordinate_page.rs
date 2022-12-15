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

#[component]
pub fn CoordinatePage(cx: Scope) -> View<DomNode> {
    let list = create_signal(cx, None);
    let get_list = || async {
        // TODO: Probably want some sort of debounce here?
        let result = request::request(
            HttpMethod::Get,
            api::LIST_COORDINATE_OFFSETS,
        ).await.unwrap();
        // TODO: Would be nice to wrap the un-jsoning in the request somehow...
        let names: Vec<String> = result.json().await.unwrap();
        list.set(Some(names));
    };
    spawn_local_scoped(cx, get_list());
    let global_info: &GlobalInfo = use_context(cx);
    //let list = create_memo(cx, move || (*list.get()).clone().unwrap_or(Vec::new()));
    let new_name = create_signal(cx, "".to_string());
    let on_click = move |_| {
        let result = request::request_with_json(
            HttpMethod::Post,
            api::SAVE_COORDINATE_OFFSET,
            &api::SaveCoordinateOffset {
                name: (*new_name.get()).clone()
            }
        );
        spawn_local_scoped(cx, async move {
            drop(result.await);
            new_name.set("".into());
            get_list().await;
        });
    };
    let delete_callback = move |name: String| move |_| {
        let req = request::request_with_json(
            HttpMethod::Delete,
            api::DELETE_COORDINATE_OFFSET,
            &api::DeleteCoordinateOffset{ name: name.clone() }
        );
        spawn_local_scoped(cx, async move {
            drop(req.await);
            get_list().await;
        })
    };
    let restore_callback = move |name: String| move |_| {
        request::request_detached_with_json(
            HttpMethod::Post,
            api::RESTORE_COORDINATE_OFFSET,
            &api::RestoreCoordinateOffset{ name: name.clone() }
        );
    };

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
                    table {
                        Indexed(
                            iterable=list,
                            view=move |cx, x| view! { cx,
                                tr {
                                    td {
                                        (x)
                                    }
                                    td {
                                        button(on:click = restore_callback(x.clone())) { "Restore" }
                                    }
                                    td {
                                        button(on:click = delete_callback(x.clone())) { "Delete" }
                                    }
                                }
                            }
                        )
                    }
                }
            }
        })
        br{}
        input(type="text", bind:value=new_name) {}
        button(on:click=on_click) { "Save coordinate system" }
        br{}
        // add save functionality!
        a(href="/") { "Go home!" }
    }
}

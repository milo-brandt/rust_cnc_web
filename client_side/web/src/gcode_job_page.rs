use std::io::Read;
use std::mem::forget;
use std::path::PathBuf;
use std::sync::Arc;

use common::api::{self, RunGcodeFile, DeleteGcodeFile};
use futures::future::{Fuse, FusedFuture};
use itertools::Itertools;
use reqwasm::websocket::{futures::WebSocket, Message};
use reqwasm::http::{FormData, Request};
use sycamore::prelude::*;
use sycamore::web::html::{input, form};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use futures::stream::StreamExt;
use futures::channel::oneshot;
use futures::{select, FutureExt, Future};
use stylist::style;
use web_sys::{KeyboardEvent, Event, HtmlInputElement};
use gloo_timers::future::sleep;
use std::time::Duration;
use sycamore::futures::{create_resource, spawn_local_scoped};
use crate::components::modal_wrapper::use_modal_handler;
use crate::components::upload_modal::UploadModal;
use crate::request::{self, HttpMethod};
use crate::status_header::GlobalInfo;
use crate::utils::async_sycamore;
use crate::components::folder_create_modal::FolderCreateModal;

#[derive(Prop)]
pub struct GcodeFileProps<'a, F: Fn() -> ()> {
    name: String,
    can_send_job: &'a ReadSignal<bool>,
    on_delete: F,
    path: String,
}

#[component]
pub fn GcodeFile<'a, F: Fn() -> () + 'a>(cx: Scope<'a>, props: GcodeFileProps<'a, F>) -> View<DomNode> {
    let name = create_ref(cx, props.name);
    let path = create_ref(cx, props.path);
    let run_callback = create_ref(cx, |_| {
        request::request_detached_with_json(
            HttpMethod::Post,
            api::RUN_GCODE_FILE,
            &RunGcodeFile { path: path.clone() }
        );
    });
    let on_delete = create_ref(cx, props.on_delete);
    view! { cx,
        tr(class="gcode_line") {
            td {
                (name.clone()) " "
            }
            td {
                button(on:click=run_callback, disabled=!*props.can_send_job.get()) { "Run!" }
            }
            td {
                button(on:click=|_| on_delete()) { "Delete!" }
            }
            td {
                a(href=format!("/view/{}", path)) { "View!" }
            }
        }
    }
}

#[derive(Prop)]
pub struct GcodeDirectoryProps<F> {
    name: String,
    link: String,
    on_delete: F,
}


#[component]
pub fn GcodeDirectory<'a, F: Fn() -> () + 'a>(cx: Scope<'a>, props: GcodeDirectoryProps<F>) -> View<DomNode> {
    view! { cx,
        tr(class="gcode_line") {
            td {
                a(href=props.link) {
                    (props.name) " "
                }
            }
            td {}
            td {
                button(on:click=move |_| (props.on_delete)()) { "Delete directory!" }
            }
            td {}
        }
    }
}


#[derive(Prop)]
pub struct GCodeUploadProps<F: Fn() -> ()> {
    on_upload: F,
}

#[component]
pub fn GCodePage<'a>(cx: Scope<'a>, path: Vec<String>) -> View<DomNode> {
    let modal = use_modal_handler(cx);

    let list = create_signal(cx, None);
    let directory = create_ref(cx, path.iter().map(|component| format!("{}/", component)).join(""));
    let parent_directory = create_ref(cx, path[..if path.is_empty() { 0 } else { path.len() - 1}].join("/"));
    let get_list = create_ref(cx, || async {
        // TODO: Probably want some sort of debounce here?
        let result = request::request_with_json(
            HttpMethod::Post,
            api::LIST_GCODE_FILES,
            &api::ListGcodeFiles {
                prefix: directory.clone()
            },
        ).await.unwrap();
        // TODO: Would be nice to wrap the un-jsoning in the request somehow...
        let names: Vec<api::GcodeFile> = result.json().await.unwrap();
        list.set(Some(names));
    });
    let on_upload = create_ref(cx, Box::new(move |files: Vec<web_sys::File>| Box::new(async move {
        for file in files {
            let form_data = FormData::new().unwrap();
            form_data.append_with_str("filename", &format!("{}{}", directory, file.name())).unwrap();
            form_data.append_with_blob_and_filename("file", &file, "filename.nc").unwrap();
            let result = request::request_with_body(
                HttpMethod::Post, 
                api::UPLOAD_GCODE_FILE, 
                form_data,
            ).await;
        }
        get_list().await;
        Ok(())
    }) as Box<dyn Future<Output=Result<(), String>> + 'a>));
    let on_close = create_ref(cx, Box::new(|| modal.clear_modal()));
    let open_upload_modal = create_ref(cx, move |_| {
        modal.set_modal(cx, move || {
            view! { cx,
                UploadModal(on_upload=on_upload.clone(), on_close=on_close.clone())
            }
        });
    });
    let open_folder_modal = create_ref(cx, move |_| {
        modal.set_modal(cx, move || {
            let on_upload = create_ref(cx, move |dirname| async move {
                request::request_with_json(
                    HttpMethod::Post,
                    api::CREATE_GCODE_DIRECTORY,
                    &api::CreateGcodeDirectory {
                        directory: format!("{}{}", directory, dirname)
                    }
                ).await.unwrap();
                get_list().await;
                Ok(())
            });
            view! { cx,
                FolderCreateModal(on_upload=on_upload, on_close=on_close.clone())
            }
        });
    });
    let on_delete_factory = create_ref(cx, move |name: String, is_directory: bool| {
        let name = format!("{}{}", directory, name);
        move || {
            let name = name.clone();
            spawn_local_scoped(cx, async move {
                request::request_with_json(
                    HttpMethod::Delete,  // Should really have method bundled in...
                    api::DELETE_GCODE_FILE,
                    &DeleteGcodeFile { path: name, is_directory }
                ).await.unwrap();
                get_list().await;
            });
        }
    });
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
                    table {
                        Indexed(
                            iterable=list,
                            view=move |cx, x|
                                if x.is_file {
                                    view! { cx,
                                        GcodeFile(
                                            name=x.name.clone(),
                                            can_send_job=global_info.is_idle,
                                            on_delete=on_delete_factory(x.name.clone(), false),
                                            path=format!("{}{}", directory, x.name),
                                        )
                                    }
                                } else {
                                    let link = format!("/send_gcode/{}{}", directory, x.name);
                                    log::debug!("LINK: {}", link);
                                    view! { cx,
                                        GcodeDirectory(name=x.name.clone(), link=link, on_delete=on_delete_factory(x.name, true))
                                    }
                                }
                        )
                        (
                            if directory.is_empty() {
                                view! { cx, }      
                            } else {
                                view! { cx,
                                    a(href=format!("/send_gcode/{}", parent_directory)) { "Parent directory" }
                                }
                            }
                        )
                    }
                }
            }
        })
        button(on:click=open_upload_modal) { "Upload" } br{}
        button(on:click=open_folder_modal) { "Add Folder" } br{} 
        a(href="/") { "Go home!" }
    }
}

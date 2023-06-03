use std::collections::HashMap;
use std::io::Read;
use std::mem::forget;
use std::sync::Arc;

use common::api::{self, RunGcodeFile, DeleteGcodeFile, SavedPosition, Vec3};
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
use futures::{select, FutureExt, try_join};
use stylist::style;
use web_sys::{KeyboardEvent, Event, HtmlInputElement};
use gloo_timers::future::sleep;
use std::time::Duration;
use sycamore::futures::{create_resource, spawn_local_scoped};
use crate::models::offsets::OffsetModel;
use crate::models::positions::PositionModel;
use crate::request::{self, HttpMethod};
use crate::status_header::GlobalInfo;
use crate::utils::async_sycamore::{self, loading_view, try_loading_view};

#[derive(Prop)]
pub struct LabelledAdderProps<'a, F> {
    on_add: &'a F,
    disabled: &'a ReadSignal<bool>,
    action_text: &'static str,
}
#[component]
pub fn LabelledAdder<'a, F: Fn(String) -> ()> (cx: Scope<'a>, props: LabelledAdderProps<'a, F>) -> View<DomNode> {
    let label = create_signal(cx, "".to_string());
    let on_add = props.on_add;
    let disabled = props.disabled;
    let action_text = props.action_text;
    view! { cx,
        input(type="text", bind:value=label)
        button(disabled=*disabled.get(), on:click=move |_| {
            let result = label.get().as_ref().clone();
            label.set("".to_string());
            on_add(result);
        }) {
            (action_text)
        }
    }
}
#[derive(Prop)]
pub struct PositionTableProps<'a> {
    data: &'a ReadSignal<Vec<SavedPosition>>
}
#[component]
pub fn PositionTable<'a>(cx: Scope<'a>, props: PositionTableProps<'a>) -> View<DomNode> {
    view! { cx,
        table(class="position_table") {
            ({
                View::new_fragment(
                    props.data.get().iter().rev().cloned().map(|item| {
                        view! { cx,
                            tr {
                                td {
                                    (item.label)
                                }
                                td {
                                    (format!("{:?}", item.position.0))
                                }
                            }
                        }
                    }).collect::<Vec<_>>()
                )
            })
        }
    }
}
#[derive(Prop)]
pub struct OffsetTableProps<'a> {
    data: &'a ReadSignal<HashMap<String, Vec3>>
}
#[component]
pub fn OffsetTable<'a>(cx: Scope<'a>, props: OffsetTableProps<'a>) -> View<DomNode> {
    let entries = create_memo(cx, || props.data.get().iter().map(|(key, value)| (key.clone(), value.clone())).collect_vec());
    view! { cx,
        table(class="position_table") {
            Keyed(
                iterable=entries,
                key=|item| item.0.clone(),
                view=|cx, item| view! { cx,
                    tr {
                        td {
                            (item.0)
                        }
                        td {
                            (format!("{:?}", item.1.0))
                        }
                    }
                }
            )
        }
    }
}

#[component]
pub fn CoordinatePage(cx: Scope) -> View<DomNode> {
    try_loading_view(
        cx,
        view! { cx, "Loading..." },
        move |err: anyhow::Error| view! { cx, (format!("Error: {:?}", err)) },
        async move {
            // Load the data...
            let global_info: &GlobalInfo = use_context(cx);
            let has_global_info = create_memo(cx, || global_info.grbl_info.get().is_some());
            let get_current_position = move || {
                let global_info = global_info.grbl_info.get();
                let position = &global_info.as_ref().as_ref().unwrap().machine_position;
                api::Vec3([position[0], position[1], position[2]])
            };
            let (offset_model, position_model) = try_join!(
                OffsetModel::new(cx),
                PositionModel::new(cx)
            )?;
            /*
                Position management...
            */
            let adding_position = create_signal(cx, false);
            let adding_disabled = create_memo(cx, || *adding_position.get() || !*has_global_info.get());
            let add_position = create_ref(cx, move |label| {
                spawn_local_scoped(cx, async move {
                    adding_position.set(true);
                    if let Err(e) = position_model.add(label, get_current_position()).await {
                        log::debug!("ERROR: {:?}", e);
                    }
                    adding_position.set(false);
                });
            });
            /*
                Offset management...
            */
            let tool_offsets = create_memo(cx, || offset_model.get().as_ref().tools.clone());
            /*
            Styling
             */
            let css = style! { r#"
                .position_table {
                    overflow: scroll;
                    height: 10rem;
                    display: block;
                }
            "#}.unwrap();
            Ok(view! { cx, 
                div(class=css.get_class_name()) {
                    LabelledAdder(on_add=add_position, disabled=adding_disabled, action_text="Record Position")
                    PositionTable(data=position_model.signal())
                }
            })
        }
    )
}

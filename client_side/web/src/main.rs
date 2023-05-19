mod utils;
mod mdc;
mod debug_page;
mod status_header;
mod gcode_job_page;
mod jog_page;
mod display_page;
mod request;
mod coordinate_page;
mod components;
pub mod render;

use common::api;
use coordinate_page::CoordinatePage;
use display_page::DisplayPage;
use gloo_timers::future::sleep;
use jog_page::JogPage;
use request::HttpMethod;
use status_header::GlobalInfo;
use status_header::global_info;
use sycamore::futures::spawn_local_scoped;
use sycamore::prelude::*;

use reqwasm::websocket::{futures::WebSocket, Message};
use sycamore_router::HistoryIntegration;
use wasm_bindgen_futures::spawn_local;
use futures::stream::StreamExt;
use futures::channel::oneshot;
use futures::channel::mpsc;
use futures::select;
use web_sys::window;
use std::cell::RefCell;
use std::mem;
use std::rc::Rc;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use mdc::CircularProgress;
use gcode_job_page::GCodePage;
use status_header::StatusHeader;

use std::time::Duration;

use sycamore::easing;
use sycamore::motion::create_tweened_signal;
use sycamore_router::{Route, Router, RouterProps};

use debug_page::DebugPage;

use crate::components::modal_wrapper::ModalWrapper;
use crate::components::modal_wrapper::install_modal_handler;
use crate::components::modal_wrapper::use_modal_handler;

#[derive(Route)]
enum AppRoutes {
    #[to("/")]
    Index,
    #[to("/debug")]
    Debug,
    #[to("/send_gcode/<path..>")]
    SendGcode {
        path: Vec<String>
    },
    #[to("/coordinates")]
    Coordinates,
    #[to("/jog")]
    Jog,
    #[to("/view/<path..>")]
    DisplayGCode {
        path: Vec<String>
    },
    #[not_found]
    NotFound,
}
impl AppRoutes {
    pub fn title(&self) -> String {
        match self {
            AppRoutes::Index => "Home".to_string(),
            AppRoutes::Debug => "Debug".to_string(),
            AppRoutes::SendGcode { .. } => "Send GCode".to_string(),
            AppRoutes::Coordinates => "Coordinates".to_string(),
            AppRoutes::Jog => "Jog".to_string(),
            AppRoutes::DisplayGCode { path } => format!("View - {}", path.last().map_or("??", |s| &s)),
            AppRoutes::NotFound => "404".to_string(),
        }
    }
}

#[component]
fn IndexPage(cx: Scope) -> View<DomNode> {
    view! { cx, 
        div {
            "Hello! "
            //DebugLine(text="hello!".to_string())
            a(href="/debug") {
                "Debug page"
            }       
            br {}
            a(href="/send_gcode") {
                "Send gcode"
            }      
            br {}
            a(href="/coordinates") {
                "Manage coordinates"
            }      
            br {}
            a(href="/jog") {
                "Jog"
            }      
        }
    }
}
#[component]
fn Page1(cx: Scope) -> View<DomNode> {
    view! { cx, 
        div {
            "Page1!"
            a(href="/") {
                "Go home?"
            }       
        }
    }
}
#[component]
fn NotFound(cx: Scope) -> View<DomNode> {
    view! { cx, 
        div {
            "Not found!"
            a(href="/") {
                "Go home?"
            }       
        }
    }
}

fn component_with_effect<'a>(cx: Scope<'a>) -> (View<DomNode>, impl Fn(u64) + Clone) {
    let signal = create_rc_signal(5);
    let local_signal = create_ref(cx, signal.clone());
    let result = view! {
        cx,
        p {
            (local_signal.get())
        }
    };
    let setter = move |value| signal.set(value);
    return (result, setter);
}

fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    let set_title = {
        let document = window().unwrap().document().unwrap();
        move |title: String| document.set_title(&title)
    };


    sycamore::render(|cx| {
        provide_context_ref(cx,  unsafe { mem::transmute::<_, &GlobalInfo<'static>>(global_info(cx)) });
        let modal_wrapper = install_modal_handler(cx);
        view! { cx,
            (modal_wrapper)
            StatusHeader
            Router(
                integration=HistoryIntegration::new(),
                view=move |cx, route: &ReadSignal<AppRoutes>| {
                    create_effect(cx, move || set_title(route.get().title()));
                    view! { cx,
                        div(class="app") {
                            (match route.get().as_ref() {
                                AppRoutes::Index => view! { cx,
                                    IndexPage
                                },
                                AppRoutes::Debug => view! { cx,
                                    DebugPage
                                },
                                AppRoutes::SendGcode { path } => view! { cx,
                                    GCodePage(path.clone())
                                },
                                AppRoutes::Jog => view! { cx,
                                    JogPage
                                },
                                AppRoutes::NotFound => view! { cx,
                                    NotFound
                                },
                                AppRoutes::DisplayGCode { path } => view! { cx,
                                    DisplayPage(path=path.clone())
                                },
                                AppRoutes::Coordinates => view! { cx, 
                                    CoordinatePage
                                }
                            })
                        }
                    }
                }
            )
        
        }
    });
}
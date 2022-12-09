use std::io::Read;
use std::mem::forget;
use std::sync::Arc;

use common::api;
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
use crate::request::{HttpMethod, self};
use crate::status_header::GlobalInfo;
use crate::utils::async_sycamore;

async fn jog(x: f64, y: f64, z: f64) {
    request::request_with_body(
        HttpMethod::Post,
        api::SEND_RAW_GCODE,
        format!("$J=G21 G91 X{:.3} Y{:.3} Z{:.3} F6000.000", x, y, z)
    ).await.unwrap();
}

#[derive(Prop)]
pub struct JogButtonProps<'a> {
    name: String,
    unit_vector: (f64, f64, f64),
    scale: &'a ReadSignal<f64>,
}

#[component]
pub fn JogButton<'a>(cx: Scope<'a>, props: JogButtonProps<'a>) -> View<DomNode> {
    let (x, y, z) = props.unit_vector;
    let scale = props.scale;
    view! { cx,
        button(on:click=move |_| {
            let scale = *scale.get();
            spawn_local(jog(x*scale,y*scale,z*scale))
        }) { (props.name) }
    }
}

pub fn parse_f64_signal<'a>(cx: Scope<'a>, input: &'a ReadSignal<String>) -> &'a ReadSignal<f64> {
    let result = create_signal(cx, 0.0);
    create_effect(cx, move || {
        let x = input.get().parse();
        match x {
            Ok(v) => result.set(v),
            Err(_) => {}
        }
    });
    result
}

#[component]
pub fn JogPage(cx: Scope) -> View<DomNode> {
    let css_style = style! { r#"
        display: grid;
        grid-template-columns: 10vw 10vw 10vw 10vw;
        
        div {
            aspect-ratio: 1;
            display: flex;
            justify-content: center;
            align-items: center;
        }
    "#
    }.expect("CSS should work");
    log::debug!("Jog CSS class: {}", css_style.get_class_name());
    let value = create_signal(cx, "100.00".to_string());
    let amt = parse_f64_signal(cx, value);
    let valuez = create_signal(cx, "10.00".to_string());
    let amtz = parse_f64_signal(cx, valuez);
    view! { cx, 
        div(class=css_style.get_class_name()) {
            div {}
            div { JogButton(name="Y+".to_string(), unit_vector=(0.0, 1.0, 0.0), scale=amt) }
            div {}
            div { JogButton(name="Z+".to_string(), unit_vector=(0.0, 0.0, 1.0), scale=amtz) }

            div { JogButton(name="X-".to_string(), unit_vector=(-1.0, 0.0, 0.0), scale=amt) }
            div {
                input(type="text", bind:value=value)
            }
            div { JogButton(name="X+".to_string(), unit_vector=(1.0, 0.0, 0.0), scale=amt) }
            div {
                input(type="text", bind:value=valuez)
            }

            div {}
            div { JogButton(name="Y-".to_string(), unit_vector=(0.0, -1.0, 0.0), scale=amt) }
            div {}
            div { JogButton(name="Z-".to_string(), unit_vector=(0.0, 0.0, -1.0), scale=amtz) }
        }
        a(href="/") { "Go home!" }
    }
}
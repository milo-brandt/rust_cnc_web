use std::sync::Arc;

use reqwasm::websocket::{futures::WebSocket, Message};
use sycamore::prelude::*;
use wasm_bindgen_futures::spawn_local;
use futures::stream::StreamExt;
use futures::channel::oneshot;
use futures::{select, FutureExt};

#[derive(Prop)]
pub struct DebugLineProps {
    #[builder(default)]
    text: String
}

#[component]
pub fn DebugLine(cx: Scope, props: DebugLineProps) -> View<DomNode> {
    view! { cx,
        div {
            (props.text)
            br{}
        }
    }
}

#[component]
pub fn DebugPage(cx: Scope) -> View<DomNode> {
    let (stop_send, mut stop_receive) = oneshot::channel::<()>();
    spawn_local(async move {
        let mut ws = WebSocket::open("ws://cnc:3000/debug/listen_raw").unwrap();
        let mut ws_next = ws.next().fuse();
        loop {
            select! {
                next_message = ws_next => {
                    ws_next = ws.next().fuse();
                    match next_message {
                        Some(Ok(Message::Text(ws_message))) => {
                            log::debug!("Received: {:?}", ws_message);
                        }
                        Some(_) => {
                            log::debug!("Received: ???");
                        }
                        None => break
                    }
                }
                _ = stop_receive => break
            }
        }
        log::debug!("Closing!");
        //drop(ws);
        let result = ws.close(None, None);
        log::debug!("Closed with result: {:?}", result);
        //let (forever_send, forever) = oneshot::channel::<()>();
        //forever.await.unwrap();
        //log::debug!("Oooh, bye!");
    });
    on_cleanup(cx, move || drop(stop_send.send(())));

    view! { cx,
        DebugLine(text="THIS IS THE DEBUG PAGE!!!!".to_string())
        a(href="/") { "Go home!" }
    }
}
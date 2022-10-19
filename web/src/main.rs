mod mdc;
use sycamore::prelude::*;

use reqwasm::websocket::{futures::WebSocket, Message};
use wasm_bindgen_futures::spawn_local;
use futures::stream::StreamExt;
use futures::channel::oneshot;
use futures::channel::mpsc;
use futures::select;
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use mdc::CircularProgress;

use std::time::Duration;

use sycamore::easing;
use sycamore::motion::create_tweened_signal;

#[derive(Debug)]
enum HandlerMessage {
    Stop,
    Send(String)
}

struct WebSocketImpl {
    messages: RcSignal<Vec<String>>,
    message_sender: mpsc::Sender<HandlerMessage>,
}

#[derive(Clone)]
struct WebSocketHandle {
    inner: Rc<RefCell<WebSocketImpl>>
}

impl PartialEq for WebSocketHandle {
    fn eq(&self, other: &Self) -> bool {
        return Rc::ptr_eq(&self.inner, &other.inner);
    }
}

impl WebSocketHandle {
    fn start(messages: RcSignal<Vec<String>>) -> Self {
        let (sender, receiver) = mpsc::channel(1024);
        let shared_state = Rc::new(RefCell::new(WebSocketImpl{
            messages: messages,
            message_sender: sender
        }));
        {
            let shared_state = shared_state.clone();
            spawn_local(async move {
                let ws = WebSocket::open("ws://127.0.0.1:3000/ws").unwrap();
                let (mut write, mut read) = ws.split();
                let mut read = read.fuse();
                let mut receiver = receiver.fuse();
                loop {
                    select! {
                        ws_message = read.next() => {
                            if let Some(Ok(Message::Text(data))) = ws_message {
                                log::debug!("Got websocket data: {}", data);
                                let rc_signal = &mut shared_state.borrow_mut().messages;
                                let mut old = (*rc_signal.get()).clone();
                                old.push(data);
                                rc_signal.set(old);
                                //shared_state.borrow_mut().on_change.fire();
                            } else {
                                log::debug!("Idk");
                            }
                        },
                        ext_message = receiver.next() => {
                            log::debug!("Got other message: {:?}", ext_message);
                        }
                    }
                }
            })
        }
        return WebSocketHandle{ inner: shared_state };
    }
}

#[derive(Prop)]
struct MyProps<'a> {
    values: &'a ReadSignal<Vec<String>>,
}

#[component]
fn App<'a, G: Html>(cx: Scope<'a>, props: MyProps<'a>) -> View<G> {
    let state = create_signal(cx, 0i32);
    let increment = |_| state.set(*state.get() + 1);
    let decrement = |_| state.set(*state.get() - 1);
    let reset = |_| state.set(0);
    let message_count = create_memo(cx, || (*props.values.get()).len());
    view! { cx,
        div {
            p { "Value: " (state.get()) }
            p { "Awkward! " (message_count.get()) }
            Indexed(
                iterable=props.values.map(cx, |messages| messages.iter().cloned().enumerate().map(|(index, value)| (index, format!("{}: {}", messages.len(), value))).collect()),
                view=|cx, x| {
                    log::debug!("Creating view for message {}", x.0);
                    view! { cx,
                        li { (x.1) }
                    }
                }//,
                //key=|x| x.0 + 200 * x.1.len()
            )
            button(on:click=increment) { "+" }
            button(on:click=decrement) { "-" }
            button(on:click=reset) { "Reset" }
        }
    }
}

/*
<label class="mdc-text-field mdc-text-field--filled">
  <span class="mdc-text-field__ripple"></span>
  <span class="mdc-floating-label" id="my-label">Label</span>
  <input type="text" class="mdc-text-field__input" aria-labelledby="my-label">
  <span class="mdc-line-ripple"></span>
</label>


      <input id="{{$ctrl.id}}" type="text" ng-model="$ctrl.ngModel" 
             class="mdc-textfield__input">
      <label for="{{$ctrl.id}}" class="mdc-textfield__label">
        {{$ctrl.label}}
      </label>
*/

#[wasm_bindgen(module = "/js/text_field.js")]
extern "C" {
    fn register_text_field(node: web_sys::Node) -> wasm_bindgen::JsValue;
    fn deregister_text_field(mdc_text_field: wasm_bindgen::JsValue);
}

#[wasm_bindgen(module = "/js/ripple.js")]
extern "C" {
    fn register_ripple(node: web_sys::Node) -> wasm_bindgen::JsValue;
    fn deregister_ripple(mdc_text_field: wasm_bindgen::JsValue);
}



#[derive(Prop)]
pub struct TextInputProps {
    #[builder(default)]
    label: String
}

#[component]
pub fn TextInput(cx: Scope, props: TextInputProps) -> View<DomNode> {
    
    let cool_button: DomNode = node! { cx,
        label(class="mdc-text-field mdc-text-field--filled") {
            span(class="mdc-text-field__ripple") {}
            span(class="mdc-floating-label", id="my-label") { (props.label) }
            input(type="text", class="mdc-text-field__input", aria-labelledby="my-label")
            span(class="mdc-line-ripple") {}
        }
    };

    let mdc_text_field = register_text_field(cool_button.inner_element());
    on_cleanup(cx, move || deregister_text_field(mdc_text_field));
    

//    cool_button.set_property("myProperty", &"Epic!".into());

    View::new_node(cool_button)
}

#[derive(Prop)]
pub struct ButtonProps<'a> {
    children: Children<'a, DomNode>
}


#[component]
pub fn Button<'a>(cx: Scope<'a>, props: ButtonProps<'a>) -> View<DomNode> {
    let children = props.children.call(cx);
    let base: DomNode = node! { cx, 
        button(class="mdc-button") {
            span(class="mdc-button__ripple") {}
            span(class="mdc-button__label", style="display:flex;align-items:center;") { (children) }
        }
    };

    let mdc_ripple = register_ripple(base.inner_element());
    on_cleanup(cx, move || deregister_ripple(mdc_ripple));


    View::new_node(base)
}


fn swoop_signal<'a>(cx: Scope<'a>) -> &'a ReadSignal<f32> {
    let signal = create_tweened_signal(cx, 0.0f32, Duration::from_millis(2500), easing::quad_out);
    signal.set(1.0);
    create_memo(cx, || *signal.get())
}

#[component]
fn Appy<'a>(cx: Scope<'a>) -> View<DomNode> {
    let state = create_signal(cx, false);
    let toggle = |_| state.set(!*state.get());

    view! { cx,
        (if *state.get() {
            view! { cx,
                TextInput(label="Name here!".to_string())
                Button { "heyasdfasdfasdfadsfasdfsadf" CircularProgress(density=-6.0, determinate=create_signal(cx, true), progress=swoop_signal(cx)) }
            }
        } else {
            view! { cx,
                div {
                    "Nothing here!"
                }
            }
        })
        button(on:click=toggle) { "toggle!" }
    }
}





fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    //let messages = create_rc_signal(vec![]);
    //let handle = WebSocketHandle::start(messages.clone());
    sycamore::render(|cx| {
        view! { cx,
            //App(values=messages)
            Appy
        }
    });
}
use std::io::Read;

use sycamore::prelude::*;
use wasm_bindgen::prelude::*;

use super::ripple::{register_ripple, deregister_ripple};

#[wasm_bindgen(module = "/src/mdc/icon_button.js")]
extern "C" {
    pub fn deselect(node: &web_sys::Node);
}

#[derive(Prop)]
pub struct IconButtonProps<'a, F: Fn() -> () + 'a>
{
    #[builder(default, setter(strip_option))]
    disabled: Option<&'a ReadSignal<bool>>,
    icon_name: &'a ReadSignal<String>,
    on_click: F
}

#[component]
pub fn IconButton<'a, F: Fn() -> () + 'a>(cx: Scope<'a>, props: IconButtonProps<'a, F>) -> View<DomNode> {
    let on_click = props.on_click;
    let disabled = props.disabled.unwrap_or_else(|| create_signal(cx, false));
    let base: DomNode = node! { cx, 
        button(class="mdc-icon-button", on:click=move |_event| on_click(), disabled=*disabled.get()) {
            div(class="mdc-icon-button__ripple") {}
            span(class="mdc-icon-button__focus-ring") {}
            i(class="material-icons"){ (props.icon_name.get()) }
        }
    };
    let mdc_ripple = create_ref(cx, register_ripple(base.inner_element()));
    let node = base.inner_element();
    create_effect(cx, move || if *disabled.get() {
        deselect(&node)
    });
    on_cleanup(cx, || deregister_ripple(mdc_ripple.clone()));

    View::new_node(base)

}
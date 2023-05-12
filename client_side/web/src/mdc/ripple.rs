use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/mdc/ripple.js")]
extern "C" {
    pub fn register_ripple(node: web_sys::Node) -> wasm_bindgen::JsValue;
    pub fn deregister_ripple(ripple: wasm_bindgen::JsValue);
}

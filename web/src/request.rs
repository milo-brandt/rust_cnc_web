use reqwasm::{websocket::futures::WebSocket, http::{Request, Response}, Error};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use futures::{FutureExt, Future};
use serde::Serialize;

const HOST_NAME: &str = "localhost:3000";

pub enum HttpMethod {
    Get,
    Post,
    Delete,
}
impl HttpMethod {
    fn request_url(&self, path: &str) -> Request {
        let true_path = format!("http://{}{}", HOST_NAME, path);
        match self {
            HttpMethod::Get => Request::get(&true_path),
            HttpMethod::Post => Request::post(&true_path),
            HttpMethod::Delete => Request::delete(&true_path),
        }
    }
}

pub fn open_websocket(path: &str) -> WebSocket {
    WebSocket::open(&format!("ws://{}{}", HOST_NAME, path)).unwrap()
}

pub fn request(method: HttpMethod, path: &str) -> impl Future<Output=Result<Response, Error>> + 'static {
    method.request_url(path).send()
}
pub fn request_with_json(method: HttpMethod, path: &str, body: &impl Serialize) -> impl Future<Output=Result<Response, Error>> + 'static {
    method.request_url(path)
    .body(serde_json::to_string(&body).unwrap())
    .header("Content-Type", "application/json")
    .send()
}
pub fn request_with_body(method: HttpMethod, path: &str, body: impl Into<JsValue>) -> impl Future<Output=Result<Response, Error>> + 'static {
    method.request_url(path)
    .body(body)
    .send()
}
pub fn request_detached(method: HttpMethod, path: &str) {
    spawn_local(request(method, path).map(|_| ()));
}
pub fn request_detached_with_json(method: HttpMethod, path: &str, body: &impl Serialize) {
    spawn_local(request_with_json(method, path, body).map(|_| ()));
}
pub fn request_detached_with_body(method: HttpMethod, path: &str, body: impl Into<JsValue>) {
    spawn_local(request_with_body(method, path, body).map(|_| ()));
}
use futures::channel::mpsc;
use serde::Serialize;

use crate::base::Channel;

#[derive(Clone)]
pub struct Sender {
    inner: mpsc::UnboundedSender<(u64, String)>
}
impl Sender {
    pub fn new(inner: mpsc::UnboundedSender<(u64, String)>) -> Self {
        Self { inner }
    }
    pub fn send<T: Serialize>(&self, channel: &Channel<T>, value: T) {
        drop(self.inner.unbounded_send((channel.0, serde_json::to_string(&value).unwrap())))
    }
}
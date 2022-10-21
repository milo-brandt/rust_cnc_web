use tokio::sync::broadcast::{self, error::RecvError};
use std::sync::{Arc, Mutex}; // Use synchronous mutex, but don't hold over awaits

struct Shared<T> {
    history: Mutex<Vec<T>>,
    sender: broadcast::Sender<T>
}

#[derive(Clone)]
pub struct HistoricalBroadcast<T> {
    state: Arc<Shared<T>>
}

pub struct HistoricalBroadcastReceiver<T> {
    ready: Vec<T>,
    listener: broadcast::Receiver<T>,
}

impl<T: Clone> HistoricalBroadcast<T> {
    pub fn new() -> Self {
        HistoricalBroadcast {
            state: Arc::new(Shared {
                history: Mutex::new(vec![]),
                sender: broadcast::channel(128).0
            })
        }
    }
    pub fn send(&self, value: T) {
        let mut state = self.state.history.lock().unwrap();
        state.push(value.clone());
        drop(self.state.sender.send(value));
    }
    pub fn subscribe(&self) -> HistoricalBroadcastReceiver<T> {
        let state = self.state.history.lock().unwrap();
        let mut ready = state.clone();
        ready.reverse();
        HistoricalBroadcastReceiver {
            ready,
            listener: self.state.sender.subscribe()
        }
    }
}
impl<T: Clone> HistoricalBroadcastReceiver<T> {
    pub async fn recv(&mut self) -> Result<T, RecvError> {
        match self.ready.pop() {
            Some(value) => Ok(value),
            None => {
                self.listener.recv().await
            },
        }
    }
}
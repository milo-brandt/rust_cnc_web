// some sort of executor thing; supports defering

use std::{sync::{Arc, Mutex}, pin::Pin};

use futures::Future;

pub type BoxedFuture = Pin<Box<dyn Future<Output=()> + Send + 'static>>;
type BoxedFutureSpanwer = Box<dyn FnMut(BoxedFuture) + Send + 'static>;

#[derive(Clone)]
pub struct Spawner {
    inner: Arc<Mutex<BoxedFutureSpanwer>>,
}

impl Spawner {
    pub fn new(f: impl FnMut(BoxedFuture) + Send + 'static) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Box::new(f)))
        }
    }
    pub fn spawn<Fut: Future<Output=()> + Send + 'static>(&self, future: Fut) {
        self.spawn_boxed(Box::pin(future));
    }
    pub fn spawn_boxed(&self, boxed_future: BoxedFuture) {
        self.inner.lock().unwrap()(boxed_future);
    }
    pub fn spawn_boxeds<I: IntoIterator<Item=BoxedFuture>>(&self, boxed_futures: I) {
        for future in boxed_futures {
            self.spawn_boxed(future);
        }
    }
}
use std::{cell::{Cell, RefCell}, collections::HashMap, task::{Waker, Context, Poll}, future::IntoFuture};

use futures::Future;

pub struct LocalGenerationCounter {
    generation: Cell<u64>,
    wakers: RefCell<HashMap<u64, Waker>>,
    next_index: Cell<u64>,
}
pub struct LocalGenerationCounterWaiter<'a> {
    parent: &'a LocalGenerationCounter,
    generation: u64,
    index: u64,
}
impl LocalGenerationCounter {
    pub fn new() -> Self {
        LocalGenerationCounter { generation: Cell::new(0), wakers: RefCell::new(HashMap::default()), next_index: Cell::new(0) }
    }
    pub fn advance(&self) {
        self.generation.set(self.generation.get() + 1);
        for (_, waker) in self.wakers.borrow_mut().drain() {
            waker.wake();
        }
    }
}
impl<'a> IntoFuture for &'a LocalGenerationCounter {
    type Output = ();
    type IntoFuture = LocalGenerationCounterWaiter<'a>;

    fn into_future(self) -> Self::IntoFuture {
        let index = self.next_index.get();
        self.next_index.set(index + 1);
        LocalGenerationCounterWaiter { parent: &self, generation: self.generation.get(), index }
    }
}
impl<'a> Future for LocalGenerationCounterWaiter<'a> {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        if self.parent.generation.get() > self.generation {
            Poll::Ready(())
        } else {
            self.parent.wakers.borrow_mut().insert(self.index, cx.waker().clone());
            Poll::Pending
        }
    }
}
impl<'a> Drop for LocalGenerationCounterWaiter<'a> {
    fn drop(&mut self) {
        self.parent.wakers.borrow_mut().remove(&self.index);
    }
}
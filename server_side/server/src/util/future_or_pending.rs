use std::{task::Poll, pin::Pin};

use futures::{Future, future::FusedFuture};
use pin_project::pin_project;

#[pin_project]
pub struct FutureOrPending<F> {
    future: Option<F>
}
impl<F: Future> FutureOrPending<F> {
    pub fn new(future: Option<F>) -> Self {
        FutureOrPending{ future }
    }
    pub fn is_none(&self) -> bool {
        self.future.is_none()
    }
    pub fn is_some(&self) -> bool {
        self.future.is_some()
    }
}
impl<F: Future> Future for FutureOrPending<F> {
    type Output = F::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let projection = self.project();
        match projection.future {
            Some(future) => {
                let future = unsafe {
                    Pin::new_unchecked(future)
                };
                let result = future.poll(cx);
                if result.is_ready() {
                    *projection.future = None;
                }
                result
            }
            None => Poll::Pending
        }
    }
}
impl<F: Future> FusedFuture for FutureOrPending<F> {
    fn is_terminated(&self) -> bool {
        self.future.is_none()
    }
}
impl<F: Future> From<Option<F>> for FutureOrPending<F> {
    fn from(value: Option<F>) -> Self {
        FutureOrPending::new(value)
    }
}

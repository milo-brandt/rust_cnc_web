use sycamore::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::Event;
use std::future::Future;
use std::marker::PhantomData;
use std::rc::Rc;
use std::pin::Pin;
use std::task::Poll;
use std::cell::UnsafeCell;

// I wonder if there's a way to spawn futures with right lifetime...?

struct LocalFutureSharedState<F> {
    future: UnsafeCell<Option<F>>
}
struct LocalFuture<F> {
    shared: Rc<LocalFutureSharedState<F>>
}
impl<F: Future<Output=()>> Future for LocalFuture<F> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        unsafe {
            if let Some(f) = &mut *self.shared.future.get() {
                    // Ok because no one ever moves f.
                    Future::poll(Pin::new_unchecked(f), cx)
            } else {
                Poll::Ready(())
            }
        }
    }

}
pub fn spawn_local_drop_with_context<'a, F: Future<Output=()> + 'static>(cx: Scope<'a>, future: F) {
    let shared = Rc::new(LocalFutureSharedState{ future: UnsafeCell::new(Some(future)) });
    let shared_clone = shared.clone();
    on_cleanup(cx, move || unsafe { *shared_clone.future.get() = None; });
    LocalFuture{ shared };
}

pub struct RcSender<T>(RcSignal<T>);
impl<T> RcSender<T> {
    pub fn set(&mut self, value: T) {
        self.0.set(value);
    }
    pub fn set_rc(&mut self, value: Rc<T>) {
        self.0.set_rc(value);
    }
}
pub fn create_channel<'a, T>(cx: Scope<'a>, value: T) -> (RcSender<T>, &'a ReadSignal<T>) {
    let rc = create_rc_signal(value);
    let read = create_signal_from_rc(cx, rc.get());
    let rc_clone = rc.clone();
    create_effect(cx, move || read.set_rc(rc.get()));
    (RcSender(rc_clone), read)
}

/*
trait Reducer<T> {
    type Event;
    fn reduce(&self, value: T, event: Event) -> T;
}
pub struct RcReducer<T, R: Reducer<T>>(RcSender<Vec<R::Event>>);
pub fn create_reducer<'a, T: Clone, R: Reducer<T> + 'a>(cx: Scope<'a>, value: T, reducer: R) -> (RcReducer<T, R>, &'a ReadSignal<T>) {
    let event_rc = create_rc_signal(vec![]);
    let value = create_signal(cx, value.clone());
    create_effect(cx, move || {
        let current_value = (*value.get_untracked()).clone();
        for event in *event_rc.get() {
            current_value = reducer.reduce(current_value, event);
        }
        value.set(current_value);
    });
}
*/
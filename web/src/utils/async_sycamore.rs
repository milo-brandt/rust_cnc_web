use sycamore::prelude::*;
use wasm_bindgen_futures::spawn_local;
use std::future::Future;
use std::marker::PhantomData;
use std::rc::Rc;
use std::pin::Pin;
use std::task::Poll;
use std::cell::{Cell, RefCell};

// I wonder if there's a way to spawn futures with right lifetime...?

struct LocalFutureSharedState<F> {
    future: RefCell<Option<F>> // If we get error here, context was probably dropped while future was running; use RefCell to at least panic! instead of UnsafeCell
}
struct LocalFuture<F> {
    shared: Rc<LocalFutureSharedState<F>>
}
impl<F: Future<Output=()>> Future for LocalFuture<F> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        if let Some(f) = &mut *self.shared.future.borrow_mut() {
            unsafe {
                // Ok because no one ever moves f.
                Future::poll(Pin::new_unchecked(f), cx)
            }
        } else {
            Poll::Ready(())
        }
    }

}
pub fn spawn_local_drop_with_context<'a, F: Future<Output=()> + 'static>(cx: Scope<'a>, future: F) {
    //NOTE: This assumes the context can't drop *while* running the future.
    let shared = Rc::new(LocalFutureSharedState{ future: RefCell::new(Some(future)) });
    let shared_clone = shared.clone();
    on_cleanup(cx, move || unsafe { *shared_clone.future.borrow_mut() = None; });
    spawn_local(LocalFuture{ shared });
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
/*pub trait Reducer<T> {
    fn reduce(&self, value: T, event: Self::Event) -> T;
}
pub struct RcReducer<T, R: Reducer<T>>(RcSignal<Cell<Vec<R::Event>>>);
pub fn create_reducer<'a, T: Clone, R: Reducer<T> + 'a>(cx: Scope<'a>, value: T, reducer: R) -> (RcReducer<T, R>, &'a ReadSignal<T>) {
    let event_rc = create_rc_signal(Cell::new(vec![]));
    let event_rc_clone = event_rc.clone();
    let value = create_signal(cx, value.clone());
    create_effect(cx, move || {
        let mut current_value = (*value.get_untracked()).clone();
        for event in event_rc.get().take() {
            current_value = reducer.reduce(current_value, event);
        }
        value.set(current_value);
    });
    (RcReducer(event_rc_clone), value)
}*/
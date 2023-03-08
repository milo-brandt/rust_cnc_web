use std::cell::RefCell;
use std::sync::Arc;

use futures::Future;
use futures::channel::mpsc;
use futures::future::BoxFuture;
use serde::Serialize;

use crate::base::Channel;
use crate::generic::SendableAs;
use crate::receiver::ListenerController;
use crate::channel_allocator::TypedChannelAllocator;
use crate::sender::Sender;
use crate::spawner::{Spawner, BoxedFuture};

#[derive(Clone)]
pub struct Context {
    pub channel_allocator: Arc<TypedChannelAllocator>,
    pub controller: ListenerController,
    pub sender: Sender,
    pub spawner: Spawner,
}

// A context with a list of futures to spawn (after some message is sent)
pub struct DeferingContext {
    pub channel_allocator: Arc<TypedChannelAllocator>,
    pub controller: ListenerController,
    pub sender: Sender,
    pub spawner: Spawner,
    waiting: RefCell<Vec<BoxedFuture>>,
}

impl Context {
    pub fn defering(self) -> DeferingContext {
        DeferingContext {
            channel_allocator: self.channel_allocator,
            controller: self.controller,
            sender: self.sender,
            spawner: self.spawner,
            waiting: RefCell::new(Vec::new()),
        }
    }
    pub fn send_in_context<T: Serialize, U: SendableAs<T>>(&self, channel: &Channel<T>, value: U) {
        let defering = self.clone().defering();
        self.sender.send(channel, value.prepare_in_context(&defering));
        let (_, futures) = defering.destructure();
        self.spawner.spawn_boxeds(futures);
    }
}
impl DeferingContext {
    pub fn destructure(self) -> (Context, Vec<BoxedFuture>) {
        (Context {
            channel_allocator: self.channel_allocator,
            controller: self.controller,
            sender: self.sender,
            spawner: self.spawner,
        }, self.waiting.into_inner())
    }
    pub fn context_clone(&self) -> Context {
        Context {
            channel_allocator: self.channel_allocator.clone(),
            controller: self.controller.clone(),
            sender: self.sender.clone(),
            spawner: self.spawner.clone(),
        }
    }
    pub fn defer_future<F: Future<Output=()> + Send + 'static>(&self, future: F) {
        self.defer_future_boxed(Box::pin(future));
    }
    pub fn defer_future_boxed(&self, boxed_future: BoxedFuture) {
        self.waiting.borrow_mut().push(boxed_future);
    }
    pub fn defer_futures_boxed<I: IntoIterator<Item=BoxedFuture>>(&self, boxed_futures: I) {
        self.waiting.borrow_mut().extend(boxed_futures);
    }
}
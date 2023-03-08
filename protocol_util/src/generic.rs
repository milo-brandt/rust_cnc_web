use serde::{Serialize, de::DeserializeOwned};

use crate::{communication_context::{Context, DeferingContext}, spawner::BoxedFuture};

pub trait Receivable: DeserializeOwned {
    type ReceivedAs: Send;
    fn receive_in_context(self, context: &Context) -> Self::ReceivedAs;
}

pub struct SendableWithFutures<T> {
    pub value: T,
    pub futures: Vec<BoxedFuture>,
}
impl<T: Serialize> SendableWithFutures<T> {
    pub fn prepare<U: SendableAs<T>>(context: &Context, value: U) -> Self {
        let defering = context.clone().defering();
        let value = value.prepare_in_context(&defering);
        let (_, futures) = defering.destructure();
        Self {
            value,
            futures
        }
    }
}

pub trait SendableAs<T: Serialize> {
    fn prepare_in_context(self, context: &DeferingContext) -> T;
}
impl<T: Serialize> SendableAs<T> for T {
    fn prepare_in_context(self, _: &DeferingContext) -> T {
        self
    }
}
impl<T: Serialize> SendableAs<T> for SendableWithFutures<T> {
    fn prepare_in_context(self, context: &DeferingContext) -> T {
        context.defer_futures_boxed(self.futures);
        self.value
    }
}
// Empty enum to use as default for generics with known arms of a variant.
pub enum Infallible {}
impl<T: Serialize> SendableAs<T> for Infallible {
    fn prepare_in_context(self, context: &DeferingContext) -> T {
        match self { }
    }
}
// Wrapper for send/receive implementations based on a trait.
pub struct DefaultSendable<U>(pub U);
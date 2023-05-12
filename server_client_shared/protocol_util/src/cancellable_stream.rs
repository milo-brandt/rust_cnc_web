use futures::{Stream, StreamExt};
use futures::channel::{oneshot, mpsc};
use pin_project::pin_project;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::base::{ChannelStream, ChannelCoFutureSender};
use crate::base::ChannelCoFuture;
use crate::generic::{DefaultSendable, Receivable};
use crate::generic::SendableAs;
use crate::{protocol_type, types};
use crate::types::Tuple;

#[derive(Serialize, Deserialize)]
pub struct CancellableStream<T> {
    stream: ChannelStream<T>,
    // Should only ever receive a canceled message; never a value.
    cancel: ChannelCoFuture<types::Infallible>,
}
impl<T: Serialize + Send + 'static, U: Stream + Unpin + Send + 'static> SendableAs<CancellableStream<T>> for DefaultSendable<U>
where U::Item: SendableAs<T> {
    fn prepare_in_context(self, context: &crate::communication_context::DeferingContext) -> CancellableStream<T> {
        let (tx, rx) = oneshot::channel::<std::convert::Infallible>();
        let reduced_future = self.0.take_until(rx);
        CancellableStream {
            stream: DefaultSendable(reduced_future).prepare_in_context(context),
            cancel: tx.prepare_in_context(context),
        }
    }
}

pub struct CancellableReceiver<T> {
    receiver: mpsc::UnboundedReceiver<T>,
    cancel: ChannelCoFutureSender<types::Infallible>,
}
impl<T: DeserializeOwned + Receivable> Receivable for CancellableStream<T>
where T::ReceivedAs: Send + 'static {
    type ReceivedAs = CancellableReceiver<T::ReceivedAs>;

    fn receive_in_context(self, context: &crate::communication_context::Context) -> Self::ReceivedAs {
        return CancellableReceiver {
            receiver: self.stream.receive_in_context(context),
            cancel: self.cancel.receive_in_context(context),
        }
    }
}
impl<T> CancellableReceiver<T> {
    pub fn close(self) {
        self.cancel.close();
    }
}
impl<T> Stream for CancellableReceiver<T> {
    type Item = T;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        self.receiver.poll_next_unpin(cx)
    }
}
use futures::future::ready;
use futures::{StreamExt, Sink, SinkExt, Stream, Future, sink, FutureExt};
use futures::channel::{mpsc, oneshot};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::sync::{Mutex, Arc};
use std::task::{self, Poll};
use crate::communication_context::{Context, DeferingContext};

use crate::generic::{Receivable, SendableAs, DefaultSendable, Infallible};
use crate::types;

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct Channel<T>(pub u64, pub PhantomData<T>);

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), Default::default())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChannelStream<T>(pub Channel<types::Option<T>>);
// represent a stream antiparallel to the message
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChannelCoStream<T>(pub Channel<types::Option<T>>);
// represent a oneshot channel parallel to the message
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChannelFuture<T>(pub Channel<types::Option<T>>);
// represent a oneshot cannel antiparallel to the message
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChannelCoFuture<T>(pub Channel<types::Option<T>>);

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct Callback<I, O>(pub ChannelCoStream<types::Tuple<(I, ChannelCoFuture<O>)>>);
/*
    Trait implementations
*/

/*
CoStreams

Can send a Sink as a CoStream
Can receive a CoStream as a Sink
 */
pub struct ChannelCoStreamSender<T: Serialize> {
    context: Context,
    channel: Channel<types::Option<T>>,
    closed: bool,
}
impl<T: Serialize> ChannelCoStreamSender<T> {
    // TODO: Handle errors?
    pub fn channel_send(&self, value: impl SendableAs<T>) {
        self.context.send_in_context(&self.channel, Some(value))
    }
}
impl<T: Serialize> Drop for ChannelCoStreamSender<T> {
    fn drop(&mut self) {
        if !self.closed {
            self.context.send_in_context(&self.channel, None::<Infallible>);
        }
    }
}
impl<T: Serialize, U: SendableAs<T>> Sink<U> for ChannelCoStreamSender<T> {
    type Error = ();

    fn poll_ready(self: std::pin::Pin<&mut Self>, _: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: U) -> Result<(), Self::Error> {
        self.channel_send(item);
        Ok(())
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.context.send_in_context(&self.channel, None::<Infallible>);
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
impl<T: DeserializeOwned + Serialize + Send + 'static> Receivable for ChannelCoStream<T> {
    type ReceivedAs = ChannelCoStreamSender<T>;
    fn receive_in_context(self, context: &Context) -> Self::ReceivedAs {
        ChannelCoStreamSender {
            context: context.clone(),
            channel: self.0,
            closed: false,
        }
    }
}

// We don't need inherently need Unpin here, but it's easier to write...
impl<T: Serialize + Receivable + DeserializeOwned + Send + 'static, U: Sink<T::ReceivedAs> + Send + Unpin + 'static> SendableAs<ChannelCoStream<T>> for DefaultSendable<U> {
    fn prepare_in_context(self, context: &DeferingContext) -> ChannelCoStream<T> {
        let channel = context.channel_allocator.incoming::<types::Option<T>>();
        context.controller.listen_with_handle(channel.clone(), {
            let context = context.context_clone();
            let sink = Arc::new(async_mutex::Mutex::new(self));
            move |message, handle| {
                let sink = sink.clone();
                let context = context.clone();
                async move {
                    // Safe to lock as these futures must execute sequentially.
                    // Could probably do try_lock
                    let sink = &mut sink.lock().await.0;
                    let message = message.receive_in_context(&context);
                    if let Some(message) = message {
                        drop(sink.send(message).await);
                    } else {
                        drop(sink.close().await);
                        handle.disconnect();
                    }
                }
            }
        });
        ChannelCoStream(channel)
    }
}
/*
Streams

Can send a Stream as a Stream
Can receive a Stream as a Stream
*/
impl<T: DeserializeOwned + Receivable> Receivable for ChannelStream<T>
where T::ReceivedAs: Send + 'static
{
    type ReceivedAs = mpsc::UnboundedReceiver<T::ReceivedAs>;

    fn receive_in_context(self, context: &Context) -> Self::ReceivedAs {
        let (sender, receiver) = mpsc::unbounded();
        context.controller.listen_with_handle(self.0, {
            let context = context.clone();
            move |message, handle| {
                match message.receive_in_context(&context) {
                    Some(message) => {
                        drop(sender.unbounded_send(message))
                    },
                    None => {
                        drop(sender.close_channel());
                        handle.disconnect();
                    }
                }
                ready(())
            }
        });
        receiver
    }
}

impl<T: Serialize + Send + 'static, U: Stream + Unpin + Send + 'static> SendableAs<ChannelStream<T>> for DefaultSendable<U>
where U::Item: SendableAs<T>
{
    fn prepare_in_context(mut self, context: &DeferingContext) -> ChannelStream<T> {
        let channel = context.channel_allocator.outgoing::<types::Option<T>>();
        context.defer_future({
            let context = context.context_clone();
            let channel = channel.clone();
            async move {
                loop {
                    let item = self.0.next().await;
                    let is_none = item.is_none();
                    context.send_in_context(&channel, item);
                    if is_none {
                        break;
                    }
                }
            }
        });
        ChannelStream(channel)
    }
}
/*
CoFuture

Can send a oneshot sender as a co-future
Can receive a co-future as an object with a send method.
 */
pub struct ChannelCoFutureSender<T: Serialize> {
    context: Context,
    channel: Channel<types::Option<T>>,
    closed: bool,
}
impl<T: Serialize> ChannelCoFutureSender<T> {
    pub fn channel_send(mut self, value: impl SendableAs<T>) {
        self.context.send_in_context(&self.channel, Some(value));
        self.closed = true;
    }
}
impl<T: Serialize> Drop for ChannelCoFutureSender<T> {
    fn drop(&mut self) {
        if !self.closed {
            self.context.send_in_context(&self.channel, None::<Infallible>);
        }
    }
}
impl<T: DeserializeOwned + Serialize + Send + 'static> Receivable for ChannelCoFuture<T> {
    type ReceivedAs = ChannelCoFutureSender<T>;
    fn receive_in_context(self, context: &Context) -> Self::ReceivedAs {
        ChannelCoFutureSender {
            context: context.clone(),
            channel: self.0,
            closed: false,
        }
    }
}
// TODO - could support async functions here. Dunno if it's worth it.
impl<T: Serialize + Receivable + DeserializeOwned + Send + 'static, U: FnOnce(Option<T::ReceivedAs>) + Send + Unpin + 'static> SendableAs<ChannelCoFuture<T>> for DefaultSendable<U> {
    fn prepare_in_context(self, context: &DeferingContext) -> ChannelCoFuture<T> {
        let channel = context.channel_allocator.incoming::<types::Option<T>>();
        context.controller.listen_with_handle(channel.clone(), {
            let context = context.context_clone();
            let callback = Arc::new(Mutex::new(Some(self.0)));
            move |message, handle| {
                if let Some(function) = callback.lock().unwrap().take() {
                    function(message.receive_in_context(&context));
                    handle.disconnect();
                }
                ready(())
            }
        });
        ChannelCoFuture(channel)
    }
}
impl<T: Serialize + Receivable + DeserializeOwned + Send + 'static> SendableAs<ChannelCoFuture<T>> for oneshot::Sender<T::ReceivedAs> {
    fn prepare_in_context(self, context: &DeferingContext) -> ChannelCoFuture<T> {
        let channel = context.channel_allocator.incoming::<types::Option<T>>();
        context.controller.listen_with_handle(channel.clone(), {
            let context = context.context_clone();
            let callback = Arc::new(Mutex::new(Some(self)));
            move |message, handle| {
                if let Some(function) = callback.lock().unwrap().take() {
                    if let Some(message) = message.receive_in_context(&context) { 
                        drop(function.send(message));
                    }
                    handle.disconnect();
                }
                ready(())
            }
        });
        ChannelCoFuture(channel)
    }
}
/*
Future

Can send a Future as a Future
Can receive a Future as a oneshot channel
 */
impl<T: DeserializeOwned + Receivable> Receivable for ChannelFuture<T>
where T::ReceivedAs: Send + 'static
{
    type ReceivedAs = oneshot::Receiver<T::ReceivedAs>;

    fn receive_in_context(self, context: &Context) -> Self::ReceivedAs {
        let (sender, receiver) = oneshot::channel();
        context.controller.listen_with_handle(self.0, {
            let context = context.clone();
            let mut sender = Some(sender);
            move |message, handle| {
                if let Some(sender) = sender.take() {
                    // Send a message if one is received! Otherwise, we'll just drop the sender.
                    if let Some(message) = message.receive_in_context(&context) {
                        drop(sender.send(message));
                    }
                    handle.disconnect();
                }
                ready(())
            }
        });
        receiver
    }
}

impl<T: Serialize + Send + 'static, U: Future + Unpin + Send + 'static> SendableAs<ChannelFuture<T>> for DefaultSendable<U>
where U::Output: SendableAs<types::Option<T>>
{
    fn prepare_in_context(self, context: &DeferingContext) -> ChannelFuture<T> {
        let channel = context.channel_allocator.outgoing::<types::Option<T>>();
        context.defer_future({
            let context = context.context_clone();
            let channel = channel.clone();
            async move {
                let result = self.0.await;
                context.send_in_context(&channel, result);
            }
        });
        ChannelFuture(channel)
    }
}
// Extra impl to get around the Result<T, Cancelled> issue...
impl<T: Serialize + Send + 'static, U: SendableAs<T> + Send + 'static> SendableAs<ChannelFuture<T>> for oneshot::Receiver<U> {
    fn prepare_in_context(self, context: &DeferingContext) -> ChannelFuture<T> {
        let channel = context.channel_allocator.outgoing::<types::Option<T>>();
        context.defer_future({
            let context = context.context_clone();
            let channel = channel.clone();
            async move {
                let result = self.await;
                context.send_in_context(&channel, result.ok());
            }
        });
        ChannelFuture(channel)
    }
}

/*
Callback: can set an asynchronous callback...
*/
pub struct CallbackRx<I: Serialize, O: Serialize> {
    sender: ChannelCoStreamSender<types::Tuple<(I, ChannelCoFuture<O>)>>
}
impl<I: Receivable + Serialize + Send + 'static, O: Serialize + Send + DeserializeOwned + 'static, Output: SendableAs<O>, Fut: Future<Output = Output> + Send + 'static, F: FnMut(I::ReceivedAs) -> Fut + Send + 'static> SendableAs<Callback<I, O>> for F {
    fn prepare_in_context(mut self, context: &DeferingContext) -> Callback<I, O> {
        let sink = sink::unfold((), move |_, (input, return_slot): (I::ReceivedAs, ChannelCoFutureSender<O>)| {
            Box::pin(self(input)).map(move |result| {
                return_slot.channel_send(result);
                Ok::<_, Infallible>(())
            })
        });
        Callback(DefaultSendable(sink).prepare_in_context(context))
    }
}
impl<I: Serialize + 'static + DeserializeOwned + Send, O: Receivable + Serialize + Send + 'static> Receivable for Callback<I, O> {
    type ReceivedAs = CallbackRx<I, O>;

    fn receive_in_context(self, context: &Context) -> Self::ReceivedAs {
        CallbackRx {
            sender: self.0.receive_in_context(context)
        }
    }
}
impl<I: Serialize, O: Serialize + Receivable + Send + 'static> CallbackRx<I, O> {
    pub fn call<Input: SendableAs<I>>(&self, input: Input) -> impl Future<Output=Result<O::ReceivedAs, oneshot::Canceled>> {
        let (tx, rx) = oneshot::channel();
        self.sender.channel_send((input, tx));
        rx
    }
}
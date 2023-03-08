use std::{future::Future, collections::HashMap, cell::UnsafeCell, task::{Context, Poll}, pin::Pin, sync::{Mutex, Arc}};

use futures::{FutureExt, stream::{FusedStream, Fuse}, Stream, StreamExt, channel::mpsc::{self, UnboundedReceiver}};
use pin_project::pin_project;
use serde::de::DeserializeOwned;
use serde_json::from_str;
use crate::base::Channel;

/*
    A simple structure that models a series of channels on which one can (once) assign a listener
which will be called for all messages received on the channel. Listeners may disconnect whenever
and messages will be ignored if not on a listened-to channel.

Callbacks may return futures which are polled to completion before proceeding.
*/

// Safe to do things like: a function that has some internal state, passed to each future


type ListenerCallback = Box<dyn FnMut(String) -> Pin<Box<dyn Future<Output=()> + Send>> + Send>;
pub struct Listener {
    running: Option<Pin<Box<dyn Future<Output=()> + Send>>>,
    callbacks: HashMap<u64, UnsafeCell<ListenerCallback>>,
}
impl Listener {
    pub fn new() -> Self {
        Self {
            running: None,
            callbacks: HashMap::new(),
        }
    }
    pub fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match self.running.as_mut() {
            Some(future) => match future.poll_unpin(cx) {
                Poll::Ready(()) => {
                    self.running = None;
                    Poll::Ready(())
                },
                Poll::Pending => Poll::Pending,
            },
            None => Poll::Ready(()),
        }
    }
    pub fn listen(&mut self, channel: u64, listener: ListenerCallback) {
        let old_value = self.callbacks.insert(channel, UnsafeCell::new(listener));
        if old_value.is_some() {
            panic!("A listener was attached to a channel that already has a listener!")
        }
    }
    pub fn stop_listening(&mut self, channel: u64) {
        let old_value = self.callbacks.remove(&channel);
        if old_value.is_none() {
            panic!("Listening on a channel was stopped on a channel without a listener!")
        }
    }
    pub fn receive_message(&mut self, channel: u64, message: String) {
        if self.running.is_some() {
            panic!("Received a message while another was processing!");
        }
        let receiver = match self.callbacks.get(&channel) {
            None => { return } // ignore message; no listener -- should maybe log?
            Some(receiver) => receiver
        };
        let receiver = unsafe { &mut *receiver.get() };
        self.running = Some(receiver(message));
    }
}
/*
    Wrapper for a task around a listener which requires two channels to control - one which feeds in the messages
and one which handles changes in the listener assignments.

Changes in listener assignment are guarunteed to be made immediately before receiving any message.
*/
pub enum ListenerUpdate {
    Listen(u64, ListenerCallback),
    StopListening(u64),
}
#[pin_project(project=ListenerTaskProjection)]
pub struct ListenerTask<Input, ListenerInput> {
    listener: Listener,
    #[pin]
    input: Input,
    #[pin]
    listener_input: Fuse<ListenerInput>
}
impl<Input: Stream<Item=(u64, String)>, ListenerInput: Stream<Item=ListenerUpdate>> ListenerTask<Input, ListenerInput> {
    pub fn new(input: Input, listener_input: ListenerInput) -> Self {
        Self {
            listener: Listener::new(),
            input,
            listener_input: listener_input.fuse(),
        }
    }
}
impl<'pin, Input, ListenerInput: Stream<Item=ListenerUpdate>> ListenerTaskProjection<'pin, Input, ListenerInput> {
    fn update_listeners(&mut self, cx: &mut Context<'_>) {
        // Flush the configuration changes into the listener.
        while let Poll::Ready(Some(value)) = self.listener_input.as_mut().poll_next(cx) {
            match value {
                ListenerUpdate::Listen(channel, listener) => self.listener.listen(channel, listener),
                ListenerUpdate::StopListening(channel) => self.listener.stop_listening(channel),
            }
        }
    }
}
impl<Input: Stream<Item=(u64, String)>, ListenerInput: Stream<Item=ListenerUpdate>> Future for ListenerTask<Input, ListenerInput> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut proj_self = self.project();
        // Loop as long as we are ready and receiving inputs.
        let result = loop {
            // Process any ready updates to the listener before doing any updates.
            // See if we are ready for more input...
            match proj_self.listener.poll_ready(cx) {
                // ...if we are ready for more input, try to get some.
                Poll::Ready(()) => {
                    match proj_self.input.as_mut().poll_next(cx) {
                        // ...if we've exhausted the input, this task is done.
                        Poll::Ready(None) => break Poll::Ready(()),
                        // ...if there's an input, receive it (and continue the loop)
                        Poll::Ready(Some((channel, message))) => {
                            // Update the listeners immediately before receiving a message
                            proj_self.update_listeners(cx);
                            // Then receive the message
                            proj_self.listener.receive_message(channel, message);
                        }
                        // ...if the channel is still working, return pending.
                        Poll::Pending => break Poll::Pending
                    }
                },
                // ...if we are still processing the last input, return pending
                Poll::Pending => {
                    break Poll::Pending
                },
            }
        };
        // Flush any desired updates before returning to avoid anything piling up in 
        // the update queue.
        proj_self.update_listeners(cx);
        return result;
    }
}
/*
    Convenience constructor for a ListenerTask
*/
#[derive(Clone)]
pub struct ListenerController {
    sender: mpsc::UnboundedSender<ListenerUpdate>
}
#[must_use]
pub struct ListenHandle {
    channel: u64,
    sender: Option<mpsc::UnboundedSender<ListenerUpdate>>
}
#[derive(Clone)]
pub struct ArcListenHandle {
    inner: Arc<Mutex<Option<ListenHandle>>>
}
impl ListenerController {
    pub fn listen_raw<F, Fut>(&self, channel: u64, mut listener: F) -> ListenHandle
    where
        F: FnMut(String) -> Fut + Send + 'static,
        Fut: Future<Output=()> + Send + 'static
    {
        self.listen_boxed(channel, Box::new(move |input| Box::pin(listener(input))))
    }
    pub fn listen_boxed(&self, channel: u64, listener: ListenerCallback) -> ListenHandle {
        drop(self.sender.unbounded_send(ListenerUpdate::Listen(channel, listener)));
        ListenHandle { channel, sender: Some(self.sender.clone()) }
    }
    pub fn listen<T, F, Fut>(&self, channel: Channel<T>, mut listener: F) -> ListenHandle
    where
        T: DeserializeOwned,
        F: FnMut(T) -> Fut + Send + 'static,
        Fut: Future<Output=()> + Send + 'static
    {
        // TODO: The .unwrap() here has nowhere to go; should probably kill the listener somehow or alert or something...
        self.listen_raw(channel.0, move |input| listener(from_str(&input).unwrap()))
    }
    pub fn listen_raw_with_handle<F, Fut>(&self, channel: u64, mut listener: F) -> ArcListenHandle
    where
        F: FnMut(String, ArcListenHandle) -> Fut + Send + 'static,
        Fut: Future<Output=()> + Send + 'static
    {
        let arc_handle = ArcListenHandle {
            inner: Arc::new(Mutex::new(None::<ListenHandle>))
        };
        // Initialize data; grab the lock to ensure that nothing funny can happen in the time between when the
        // listener is connected and when inner value is initialized.
        {
            let mut lock = arc_handle.inner.lock().unwrap();
            *lock = Some(self.listen_raw(channel, {
                let arc_handle = arc_handle.clone();
                move |message| listener(message, arc_handle.clone())
            }));
        }
        arc_handle

    }
    pub fn listen_with_handle<T, F, Fut>(&self, channel: Channel<T>, mut listener: F) -> ArcListenHandle
    where
        T: DeserializeOwned,
        F: FnMut(T, ArcListenHandle) -> Fut + Send + 'static,
        Fut: Future<Output=()> + Send + 'static
    {
        let arc_handle = ArcListenHandle {
            inner: Arc::new(Mutex::new(None::<ListenHandle>))
        };
        // Initialize data; grab the lock to ensure that nothing funny can happen in the time between when the
        // listener is connected and when inner value is initialized.
        {
            let mut lock = arc_handle.inner.lock().unwrap();
            *lock = Some(self.listen(channel, {
                let arc_handle = arc_handle.clone();
                move |message| listener(message, arc_handle.clone())
            }));
        }
        arc_handle
    }
}
impl ListenHandle {
    pub fn disconnect_mut(&mut self) {
        if let Some(sender) = &self.sender {
            drop(sender.unbounded_send(ListenerUpdate::StopListening(self.channel)));
        }
        self.sender = None;
    }
    pub fn disconnect(mut self) {
        self.disconnect_mut()
    }
    pub fn detach_mut(&mut self) {
        self.sender = None;
    }
    pub fn detach(mut self) {
        self.detach_mut();
    }
}
impl Drop for ListenHandle {
    fn drop(&mut self) {
        self.disconnect_mut()
    }
}
impl ArcListenHandle {
    pub fn disconnect(self) {
        self.inner.lock().unwrap().as_mut().unwrap().disconnect_mut()
    }
}

#[derive(Clone)]
pub struct ListenerSender {
    sender: mpsc::UnboundedSender<(u64, String)>
}
impl ListenerSender {
    pub fn send(&self, channel: u64, message: String) {
        drop(self.sender.unbounded_send((channel, message)));
    }
}
pub struct ListenerCreation<F> {
    pub future: F,
    pub controller: ListenerController,
}
pub fn create_listener<Input: Stream<Item=(u64, String)>>(input: Input) -> ListenerCreation<ListenerTask<Input, UnboundedReceiver<ListenerUpdate>>> {
    let (listener_sender, listener_input) = mpsc::unbounded();
    let future = ListenerTask::new(input, listener_input);
    ListenerCreation {
        future,
        controller: ListenerController { sender: listener_sender }
    }
}
pub struct FullListenerCreation<F> {
    pub future: F,
    pub controller: ListenerController,
    pub sender: ListenerSender,
}
pub fn create_listener_full() -> FullListenerCreation<impl Future<Output=()> + Unpin + Send> {
    let (sender, receiver) = mpsc::unbounded();
    let ListenerCreation { future, controller } = create_listener(receiver);
    FullListenerCreation { future, controller, sender: ListenerSender { sender } }
}





#[cfg(test)]
mod tests {
    use std::sync::{Arc, self};

    use futures::{poll, SinkExt, channel::mpsc::UnboundedSender};

    use super::*;

    // Could potentially add an executor that crashes when a thread parks. 

    // Test that the future stops when the stream closes.
    #[tokio::test]
    async fn test_stop() {
        let FullListenerCreation { future, controller: _, sender } = create_listener_full();
        drop(sender);
        future.await;
    }

    fn setup_echo(channel: u64, controller: &ListenerController, send_result: UnboundedSender<String>) {
        controller.listen_raw_with_handle(channel, move |message, handle| {                
            let mut send_result = send_result.clone();
            async move {
                drop(send_result.send(message.clone()).await);
                if message == "Bye" {
                    handle.disconnect()
                }
            }
        });
    }

    #[tokio::test]
    async fn test_echo() {
        let FullListenerCreation { mut future, controller, sender } = create_listener_full();
        let (send_result, mut results) = mpsc::unbounded();
        // Make sure that first poll does nothing.
        assert_eq!(poll!(&mut future), Poll::Pending);
        // Send a message & ensure it is ignored; must poll before calling listen.
        sender.send(0, "Hello".into());
        assert_eq!(poll!(&mut future), Poll::Pending);
        // Attach a listener on channel 0 that echos its input to results and disconnects if it
        // receives the message "Bye"
        setup_echo(0, &controller, send_result);
        // Make sure nothing's yet sent
        assert_eq!(poll!(results.next()), Poll::Pending);
        // Now, send "Hey" and make sure it gets echoed.
        sender.send(0, "Hey".into());
        assert_eq!(poll!(&mut future), Poll::Pending);
        assert_eq!(poll!(results.next()), Poll::Ready(Some("Hey".into())));
        // Check that the above works multiple times.
        sender.send(0, "Hey_2".into());
        assert_eq!(poll!(&mut future), Poll::Pending);
        assert_eq!(poll!(results.next()), Poll::Ready(Some("Hey_2".into())));
        // Check that we are differentiating based on channel.
        sender.send(1, "Wrong channel".into());
        assert_eq!(poll!(&mut future), Poll::Pending);
        assert_eq!(poll!(results.next()), Poll::Pending);
        // Check that we can trigger the disconnection - deleting a function and also the stream
        sender.send(0, "Bye".into());
        assert_eq!(poll!(&mut future), Poll::Pending);
        assert_eq!(poll!(results.next()), Poll::Ready(Some("Bye".into())));
        assert_eq!(poll!(results.next()), Poll::Ready(None));
        // Now, send a message and ensure it is ignored.
        sender.send(0, "Hey_3".into());
        assert_eq!(poll!(&mut future), Poll::Pending);
        // Now close the input and make sure the future completes.
        drop(sender);
        future.await;
    }

    // Even if the results are ready, updates to the listener state are processed before continuing.
    #[tokio::test]
    async fn test_echo_batch() {
        let FullListenerCreation { future, controller, sender } = create_listener_full();
        let (send_result, results) = mpsc::unbounded();
        setup_echo(0, &controller, send_result);
        
        sender.send(0, "Hey".into());
        sender.send(0, "Hey_2".into());
        sender.send(0, "Bye".into());
        sender.send(0, "Hey_3".into());
        drop(sender);
        future.await;
        let results: Vec<String> = results.collect().await;
        assert_eq!(results, vec!["Hey", "Hey_2", "Bye"]);
    }
    
    // Ensure that returned futures are polled to completion.
    #[tokio::test]
    async fn test_strict_order() {
        let FullListenerCreation { mut future, controller, sender } = create_listener_full();
        let (send_result, mut results) = mpsc::unbounded();
        let (continue_token, continue_token_receiver) = mpsc::unbounded::<()>();
        controller.listen_raw(0, {
            let continue_token_receiver = Arc::new(async_mutex::Mutex::new(continue_token_receiver));
            move |message| {                
                let mut send_result = send_result.clone();
                let continue_token_receiver = continue_token_receiver.clone();
                async move {
                    drop(send_result.send(message).await);
                    continue_token_receiver.lock().await.next().await;
                }
            }
        }).detach();
        // Check that the first future processes immediately.
        sender.send(0, "Hey".into());
        assert_eq!(poll!(&mut future), Poll::Pending);
        assert_eq!(poll!(results.next()), Poll::Ready(Some("Hey".into())));
        // Check that the second future does not process as long as the first one is processing.
        sender.send(0, "Hey_2".into());
        assert_eq!(poll!(&mut future), Poll::Pending);
        assert_eq!(poll!(results.next()), Poll::Pending);
        // Check that resolving the first future lets the second resolve.
        drop(continue_token.unbounded_send(()));
        assert_eq!(poll!(&mut future), Poll::Pending);
        assert_eq!(poll!(results.next()), Poll::Ready(Some("Hey_2".into())));
        // Check that the future overall does not complete until its last task does.
        drop(sender);
        assert_eq!(poll!(&mut future), Poll::Pending);
        assert_eq!(poll!(results.next()), Poll::Pending);
        // Check that it does complete once this last message is sent.
        drop(continue_token.unbounded_send(()));
        assert_eq!(poll!(&mut future), Poll::Ready(()));
    }
}

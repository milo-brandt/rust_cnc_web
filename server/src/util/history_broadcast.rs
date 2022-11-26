use {
    std::{
        alloc::{alloc, Layout},
        cmp::min,
        mem::MaybeUninit,
        sync::{Arc, RwLock},
    },
    tokio::sync::Notify,
};

fn make_vec_uninit<T>(size: usize) -> Vec<MaybeUninit<T>> {
    let layout = Layout::array::<T>(size).unwrap();
    unsafe { Vec::from_raw_parts(alloc(layout) as *mut MaybeUninit<T>, size, size) }
}

struct HistoryRingBufferInner<T> {
    storage: Vec<MaybeUninit<T>>,
    begin_count: usize,
    end_count: usize,
    closed: bool,
}
impl<T> Drop for HistoryRingBufferInner<T> {
    fn drop(&mut self) {
        for i in self.begin_count..self.end_count {
            let index = i % self.storage.capacity();
            unsafe {
                self.storage[index].assume_init_drop();
            }
        }
    }
}
struct HistoryRingBuffer<T> {
    inner: RwLock<HistoryRingBufferInner<T>>,
    capacity: usize,
    on_next: Notify,
}
pub struct Sender<T> {
    //could copy end_count out here; this controls state
    state: Arc<HistoryRingBuffer<T>>,
}
#[derive(Clone)]
pub struct Receiver<T> {
    next_count: usize,
    state: Arc<HistoryRingBuffer<T>>,
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ReceiverError {
    Lagged(usize),
    Closed,
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TryReceiverError {
    Empty,
    Lagged(usize),
    Closed,
}

fn subscribe_with_history_count_impl<T>(
    history: Arc<HistoryRingBuffer<T>>,
    size: usize,
) -> Receiver<T> {
    let end_count = history.inner.read().unwrap().end_count;
    let true_size = min(end_count, size);
    let next_count = end_count - true_size;
    Receiver {
        next_count,
        state: history,
    }
}

impl<T: Clone + Send> Sender<T> {
    pub fn new(size: usize) -> Self {
        Sender {
            state: Arc::new(HistoryRingBuffer {
                inner: RwLock::new(HistoryRingBufferInner {
                    storage: make_vec_uninit(size),
                    begin_count: 0,
                    end_count: 0,
                    closed: false,
                }),
                capacity: size,
                on_next: Notify::new(),
            }),
        }
    }
    pub fn send(&self, value: T) {
        let history = &*self.state;
        let mut lock = history.inner.write().unwrap();
        unsafe {
            if lock.end_count == lock.begin_count + history.capacity {
                let begin_index = lock.begin_count % history.capacity;
                lock.storage[begin_index].assume_init_drop();
                lock.storage[begin_index].write(value);
                lock.begin_count += 1;
                lock.end_count += 1;
            } else {
                let end_index = lock.end_count; // Same because no wrapping yet!
                lock.storage[end_index].write(value);
                lock.end_count += 1;
            }
            history.on_next.notify_waiters();
        }
    }
    pub fn close(self) {
        drop(self);
    }
    pub fn subscribe_with_history_count(&self, size: usize) -> Receiver<T> {
        subscribe_with_history_count_impl(self.state.clone(), size)
    }
}
impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut lock = self.state.inner.write().unwrap();
        lock.closed = true;
    }
}
impl<T: Clone + Send> Receiver<T> {
    pub fn try_recv(&mut self) -> Result<T, TryReceiverError> {
        let history = &*self.state;
        let lock = history.inner.read().unwrap();
        if lock.begin_count > self.next_count {
            let difference = lock.begin_count - self.next_count;
            self.next_count = lock.begin_count;
            Err(TryReceiverError::Lagged(difference))
        } else if self.next_count >= lock.end_count {
            if lock.closed {
                Err(TryReceiverError::Closed)
            } else {
                Err(TryReceiverError::Empty)
            }
        } else {
            let index = self.next_count % history.capacity;
            self.next_count += 1;
            unsafe { Ok(lock.storage[index].assume_init_ref().clone()) }
        }
    }
    pub async fn recv(&mut self) -> Result<T, ReceiverError> {
        loop {
            match self.try_recv() {
                Ok(value) => return Ok(value),
                Err(TryReceiverError::Closed) => return Err(ReceiverError::Closed),
                Err(TryReceiverError::Lagged(amt)) => return Err(ReceiverError::Lagged(amt)),
                Err(TryReceiverError::Empty) => {
                    let notified = {
                        let history = &*self.state;
                        let lock = history.inner.read().unwrap();
                        if self.next_count >= lock.end_count {
                            Some(history.on_next.notified())
                        } else {
                            None
                        }
                    };
                    if let Some(notified) = notified {
                        notified.await;
                    }
                }
            }
        }
    }
    pub fn subscribe_with_history_count(&self, size: usize) -> Receiver<T> {
        subscribe_with_history_count_impl(self.state.clone(), size)
    }
}
#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    struct FunnyTuple(u64, u8); // Weird alignment

    #[test]
    fn test_maybe_unit_vec() {
        let mut v: Vec<MaybeUninit<FunnyTuple>> = make_vec_uninit(8);
        v[7].write(FunnyTuple(23, 5));
        unsafe {
            assert_eq!(*v[7].assume_init_ref(), FunnyTuple(23, 5));
        }
    }

    #[test]
    fn test_history_buffer() {
        let sender = Sender::<u64>::new(4);
        let mut receiver = sender.subscribe_with_history_count(10);
        let mut receiver_2 = sender.subscribe_with_history_count(10);
        assert_eq!(receiver.try_recv(), Err(TryReceiverError::Empty));
        assert_eq!(receiver_2.try_recv(), Err(TryReceiverError::Empty));
        for i in 0..16 {
            sender.send(i);
            assert_eq!(receiver.try_recv(), Ok(i));
            assert_eq!(receiver.try_recv(), Err(TryReceiverError::Empty));
        }
        assert_eq!(receiver_2.try_recv(), Err(TryReceiverError::Lagged(12)));
        for i in 12..16 {
            assert_eq!(receiver_2.try_recv(), Ok(i));
        }

        let mut receiver_3 = sender.subscribe_with_history_count(100);
        assert_eq!(receiver_3.try_recv(), Err(TryReceiverError::Lagged(12)));
        for i in 12..16 {
            assert_eq!(receiver_3.try_recv(), Ok(i));
        }

        let mut receiver_4 = sender.subscribe_with_history_count(4);
        for i in 12..16 {
            assert_eq!(receiver_4.try_recv(), Ok(i));
        }

        let mut receiver_5 = sender.subscribe_with_history_count(2);
        for i in 14..16 {
            assert_eq!(receiver_5.try_recv(), Ok(i));
        }
    }

    #[test]
    fn test_send() {
        fn is_send<T: Send>() {}
        is_send::<u8>(); // compiles only if true
        is_send::<Receiver<u8>>(); // won't compile; see compile_fail rustdoc feature
        is_send::<Sender<u8>>(); // won't compile; see compile_fail rustdoc feature
    }
}

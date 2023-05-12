use crate::base::Channel;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ChannelAllocator {
    incoming: AtomicU64,
    outgoing: AtomicU64,
}
impl ChannelAllocator {
    pub fn new() -> Self {
        Self {
            incoming: AtomicU64::new(0),
            outgoing: AtomicU64::new(u64::MAX)
        }
    }
    pub fn next_incoming(&self) -> u64 {
        self.incoming.fetch_add(1, Ordering::SeqCst)
    }
    pub fn next_outgoing(&self) -> u64 {
        self.outgoing.fetch_sub(1, Ordering::SeqCst)
    }
}
pub struct TypedChannelAllocator {
    allocator: ChannelAllocator
}
impl TypedChannelAllocator {
    pub fn new() -> Self {
        Self {
            allocator: ChannelAllocator::new()
        }
    }
    pub fn incoming<T>(&self) -> Channel<T> {
        Channel(self.allocator.next_incoming(), Default::default())
    }
    pub fn outgoing<T>(&self) -> Channel<T> {
        Channel(self.allocator.next_outgoing(), Default::default())
    }
}
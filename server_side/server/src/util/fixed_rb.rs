use std::mem::MaybeUninit;
use chrono::Local;
use ringbuf::LocalRb;

pub struct FixedRb<T, const N: usize> {
    storage: LocalRb<T, [MaybeUninit<T>; N]>
}
impl<T, const N: usize> FixedRb<T, N> {
    pub fn new() -> Self {
        FixedRb {
            storage: unsafe {
                LocalRb::from_raw_parts(
                    MaybeUninit::uninit().assume_init(),
                    0,
                    0
                )
            }
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        self.storage.split_ref().1.pop()
    }
    pub fn push(&mut self, element: T) -> Result<(), T> {
        self.storage.split_ref().0.push(element)
    }
    pub fn is_empty(&mut self) -> bool {
        self.storage.split_ref().1.is_empty()
    }
    pub fn is_full(&mut self) -> bool {
        self.storage.split_ref().0.is_full()
    }
    pub fn clear(&mut self) -> usize {
        self.storage.split_ref().1.clear()
    }
}
use std::mem::MaybeUninit;
use ringbuf::LocalRb;

pub type FixedRb<T, const N: usize> = LocalRb<T, [MaybeUninit<T>; N]>; 
pub fn make_fixed_rb<T, const N: usize>() -> LocalRb<T, [MaybeUninit<T>; N]> {
    unsafe {
        LocalRb::from_raw_parts(MaybeUninit::uninit().assume_init(), 0, 0)
    }
}

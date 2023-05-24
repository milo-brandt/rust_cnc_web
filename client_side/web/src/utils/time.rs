use std::{pin::pin, cell::{Cell, RefCell}};

use chrono::{Duration, DateTime, Utc};
use futures::StreamExt;
use gloo_timers::future::IntervalStream;
use sycamore::{prelude::*, futures::spawn_local_scoped};

pub fn format_seconds(seconds: i64) -> String {
    format!("{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
}
pub fn format_duration(duration: Duration) -> String {
    format_seconds(duration.num_seconds())
}
// Produce a signal that ticks every second; cheap way to keep things updated. Could have weird behavior if
// you have both this pulse and another thing that can update.
pub fn second_pulse<'a>(cx: Scope<'a>) -> &'a ReadSignal<()> {
    let signal = create_signal(cx, ());
    spawn_local_scoped(cx, async {
        let mut intervals = pin!(IntervalStream::new(1000));
        loop {
            while let Some(()) = intervals.next().await {
                signal.trigger_subscribers();
            }
        }
    });
    signal
}
pub fn elapsed_seconds_since<'a>(cx: Scope<'a>, time: &'a ReadSignal<Option<DateTime<Utc>>>) -> &'a ReadSignal<i64> {
    // Copy time, but only notify on changes.
    let time = create_selector(cx, || time.get().as_ref().clone());
    let signal = create_signal(cx, 0);
    let set_signal = || signal.set(match time.get().as_ref() {
        Some(time) => (Utc::now() - time.clone()).num_seconds(),
        None => 0,
    }); 
    // Compute time each second
    spawn_local_scoped(cx, async move {
        let mut intervals = pin!(IntervalStream::new(1000));
        loop {
            while let Some(()) = intervals.next().await {
                set_signal();
            }
        }
    });
    // Upon changing the time, immediately recompute time; will track the time signal.
    create_effect(cx, set_signal);
    signal
}
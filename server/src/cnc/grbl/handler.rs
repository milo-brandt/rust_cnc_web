use async_trait::async_trait;
use tokio::sync::oneshot;

use super::messages::{ProbeEvent, GrblStateInfo};


#[derive(Clone, Debug)]
pub enum LineError {
    Grbl(u64),
    Reset
}
#[derive(Clone, Debug)]
pub enum ProbeError {
    Grbl(u64),
    Reset,
}
#[derive(Debug)]
pub enum WriteRequest {
    Plain {
        data: Vec<u8>,                            // Should include newline
        result: oneshot::Sender<Result<(), LineError>>, // gives error code on failure
    },
    Probe {
        data: Vec<u8>,
        result_line: oneshot::Sender<Result<(), LineError>>, // gives error code on failure
        result: oneshot::Sender<Result<ProbeEvent, ProbeError>>, // gives error code on failure
    },
}
#[derive(Debug)]
pub enum SpeedOverride {
    FeedReset,
    FeedIncrease10,
    FeedDecrease10,
    FeedIncrease1,
    FeedDecrease1,

    RapidReset,
    RapidHalf,
    RapidQuarter,

    SpindleReset,
    SpindleIncrease10,
    SpindleDecrease10,
    SpindleIncrease1,
    SpindleDecrease1,
}
#[derive(Debug)]
pub enum ImmediateRequest {
    Status {
        result: oneshot::Sender<GrblStateInfo>,
    },
    FeedHold,
    FeedResume,
    Reset,
    OverrideSpeed(SpeedOverride),
}

#[async_trait(?Send)]
pub trait Handler {
    // Callbacks; should only include minimal logic here!
    fn after_send(&self, bytes: Vec<u8>) {}
    fn after_receive(&self, line: String) {}
    fn warn(&self, message: String) {}
    async fn on_alarm(&self, index: u64) {}
    async fn after_reset(&self) {}

    // Futures for getting lines; should be cancellation safe.
    async fn next_write_request(&self) -> WriteRequest;
    async fn next_immediate_request(&self) -> ImmediateRequest;

    // Main loop
    async fn run(&self);
}
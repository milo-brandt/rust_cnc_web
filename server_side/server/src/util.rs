use futures::Future;

pub mod history_broadcast;
pub mod local_generation_counter;
pub mod fixed_rb;
pub mod format_bytes;
pub mod future_or_pending;
pub mod file_backed_json;
pub mod exclusive_extension;

pub fn force_output_type<T>(future: impl Future<Output=T>) -> impl Future<Output=T> {
    future
}
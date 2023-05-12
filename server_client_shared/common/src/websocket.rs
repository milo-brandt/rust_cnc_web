use protocol_util::{protocol_type, types::Primitive, base::{Callback, ChannelStream, ChannelCoFuture}};

#[protocol_type]
struct Watchable<T> {
    value: T,
    update_stream: ChannelStream<T>,
    stop: ChannelCoFuture<()>,
}

pub enum EntryType {
    File, Directory
}

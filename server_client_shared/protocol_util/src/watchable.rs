use protocol_util::{protocol_type, types::Primitive, base::{Callback, ChannelStream, ChannelCoFuture}};



struct WatchableInner<T> {
    value: T,
    update_stream: ChannelStream<T>,
    stop: ChannelCoFuture<()>,
}

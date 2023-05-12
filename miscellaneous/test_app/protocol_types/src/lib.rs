use futures::{channel::oneshot, Future, FutureExt};
use protocol_util::{communication_context::{Context, DeferingContext}, base, generic::{Receivable, SendableAs}, types};
use protocol_util::protocol_type;
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub username: String,
    pub message: String,
}

#[protocol_type]
pub struct Room {
    pub send_message: base::ChannelCoStream<types::Primitive<String>>,
    pub messages: base::ChannelStream<types::Primitive<Message>>
}

#[protocol_type]
pub struct JoinRoom {
    pub username: types::Primitive<String>,
    pub room_name: types::Primitive<String>,
    pub result: base::ChannelCoFuture<types::Option<Room>>
}
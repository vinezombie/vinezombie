//! Useful handler implementations.

mod ping;

pub use ping::*;

use super::{Handler, SelfMadeHandler};
use crate::ircmsg::ServerMsg;

/// [`Handler`] that yields every message it receives until the channel closes.
#[derive(Clone, Copy, Debug, Default)]
pub struct YieldAll;

impl Handler for YieldAll {
    type Value = ServerMsg<'static>;

    fn handle(
        &mut self,
        msg: &crate::ircmsg::ServerMsg<'_>,
        _: super::QueueEditGuard<'_>,
        mut channel: super::channel::SenderRef<'_, Self::Value>,
    ) -> bool {
        channel.send(msg.clone().owning());
        !channel.can_send()
    }

    fn wants_owning(&self) -> bool {
        true
    }
}

impl SelfMadeHandler for YieldAll {
    type Receiver<Spec: super::channel::ChannelSpec> = Spec::Queue<Self::Value>;

    fn queue_msgs(&self, _: super::QueueEditGuard<'_>) {}

    fn make_channel<Spec: super::channel::ChannelSpec>(
        spec: &Spec,
    ) -> (std::sync::Arc<dyn super::channel::Sender<Value = Self::Value>>, Self::Receiver<Spec>)
    {
        spec.new_queue()
    }
}

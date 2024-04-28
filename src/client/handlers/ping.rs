use super::{Handler, SelfMadeHandler};
use crate::client::ClientState;
use crate::names::cmd::{PING, PONG};
use crate::{
    client::{
        channel::{ChannelSpec, Sender, SenderRef},
        queue::QueueEditGuard,
    },
    ircmsg::{ClientMsg, ServerMsg},
    string::Arg,
};
use std::time::Instant;

/// [`Handler`] that pings the server and yields the duration it took.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Ping(pub std::time::Instant);

impl Default for Ping {
    fn default() -> Self {
        Ping(std::time::Instant::now())
    }
}

impl Handler for Ping {
    type Value = (Option<crate::ircmsg::Source<'static>>, std::time::Duration);

    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        _: &mut ClientState,
        _: QueueEditGuard<'_>,
        mut channel: SenderRef<'_, Self::Value>,
    ) -> bool {
        if msg.kind == PONG {
            if let Some(last) = msg.args.split_last().1 {
                let hash = crate::util::mangle(&self.0);
                let mut value: u32 = 0;
                for byte in last.as_bytes().iter().cloned() {
                    if !(b'0'..=b'7').contains(&byte) {
                        return false;
                    }
                    value <<= 3;
                    value |= (byte - b'0') as u32;
                }
                if hash == value {
                    let duration = Instant::now().saturating_duration_since(self.0);
                    let source = msg.source.clone().map(crate::ircmsg::SharedSource::owning_merged);
                    channel.send((source, duration));
                    return true;
                }
            }
        }
        false
    }
}

impl SelfMadeHandler for Ping {
    type Receiver<Spec: ChannelSpec> = Spec::Oneshot<Self::Value>;

    fn queue_msgs(&self, _: &ClientState, mut queue: QueueEditGuard<'_>) {
        let mut msg = ClientMsg::new(PING);
        let hash = crate::util::mangle(&self.0);
        let hash: Arg<'static> = format!("{hash:o}").try_into().unwrap();
        msg.args.edit().add_word(hash);
        queue.push(msg);
    }

    fn make_channel<Spec: ChannelSpec>(
        spec: &Spec,
    ) -> (Box<dyn Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>) {
        spec.new_oneshot()
    }
}

pub(crate) fn pong(
    msg: &ServerMsg<'_>,
    mut queue: impl crate::client::ClientMsgSink<'static>,
) -> bool {
    let retval = msg.kind == PING;
    if retval {
        let mut reply = ClientMsg::new(PONG);
        if let Some(last) = msg.args.split_last().1 {
            reply.args.edit().add(last.clone().owning());
        }
        queue.send(reply);
    }
    retval
}

/// Auto-replier to PING messages.
///
/// This is generally necessary on every connection to avoid being disconnected by the server.
/// Note that the included registration handler automatically responds to pings on its own,
/// as some IRCds require this to successfully register.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct AutoPong;

impl Handler for AutoPong {
    type Value = ();

    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        _: &mut ClientState,
        mut queue: QueueEditGuard<'_>,
        _: SenderRef<'_, Self::Value>,
    ) -> bool {
        pong(msg, &mut queue);
        false
    }
}

impl SelfMadeHandler for AutoPong {
    type Receiver<Spec: ChannelSpec> = ();

    fn queue_msgs(&self, _: &ClientState, _: QueueEditGuard<'_>) {}

    fn make_channel<Spec: ChannelSpec>(
        _: &Spec,
    ) -> (Box<dyn Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>) {
        (Box::<crate::client::channel::ClosedSender<_>>::default(), ())
    }
}

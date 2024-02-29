use super::{Handler, SelfMadeHandler};
use crate::consts::cmd::{PING, PONG};
use crate::{
    client::{
        channel::{ChannelSpec, Sender, SenderRef},
        QueueEditGuard,
    },
    ircmsg::{ClientMsg, ServerMsg},
    string::Arg,
};
use std::sync::Arc;
use std::{hash::Hasher, time::Instant};

/// [`Handler`] that pings the server and yields the duration it took.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Ping(pub std::time::Instant);

impl Default for Ping {
    fn default() -> Self {
        Ping(std::time::Instant::now())
    }
}

impl Ping {
    fn make_hash(&self) -> u32 {
        use std::hash::Hash;
        /// Slightly weird FNV-0a hash.
        struct Hasher(pub u64);
        impl std::hash::Hasher for Hasher {
            fn finish(&self) -> u64 {
                self.0
            }

            fn write(&mut self, bytes: &[u8]) {
                let mut bytes = bytes.iter();
                loop {
                    // We're not supposed to know much about std::time::Instant.
                    // Let's assume bytes is an integer of unknown endianness.
                    // It is important that the low bits diffuse into the hash as much as possible,
                    // so let's slighly favor little-endian but otherwise
                    // take bytes from both ends, add 1 wrapping because FNV hates 0x00 bytes
                    // (which are very likely to exist in the more significant bytes).
                    let Some(byte) = bytes.next() else {
                        return;
                    };
                    self.0 ^= byte.wrapping_add(1) as u64;
                    self.0 = self.0.wrapping_mul(1099511628211);
                    let Some(byte) = bytes.next_back() else {
                        return;
                    };
                    self.0 ^= byte.wrapping_add(1) as u64;
                    self.0 = self.0.wrapping_mul(1099511628211);
                }
            }
        }
        let mut hasher = Hasher(0);
        // 0 is possibly the worst offset basis. Let's fix this by hashing the OS name.
        std::env::consts::OS.hash(&mut hasher);
        self.0.hash(&mut hasher);
        let result = hasher.finish();
        // XOR-fold to make the original info even more unrecoverable.
        ((result >> 32) | (result & 0xFFFFFFFF)) as u32
    }
}

impl Handler for Ping {
    type Value = (Option<crate::ircmsg::Source<'static>>, std::time::Duration);

    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        _: QueueEditGuard<'_>,
        mut channel: SenderRef<'_, Self::Value>,
    ) -> bool {
        if msg.kind == PONG {
            if let Some(last) = msg.args.split_last().1 {
                let hash = self.make_hash();
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

    fn queue_msgs(&self, mut queue: QueueEditGuard<'_>) {
        let mut msg = ClientMsg::new_cmd(PING);
        let hash = self.make_hash();
        let hash: Arg<'static> = format!("{hash:o}").try_into().unwrap();
        msg.args.edit().add_word(hash);
        queue.push(msg);
    }

    fn make_channel<Spec: ChannelSpec>(
        spec: &Spec,
    ) -> (Arc<dyn Sender<Value = Self::Value>>, Self::Receiver<Spec>) {
        spec.new_oneshot()
    }
}

pub(crate) fn pong(
    msg: &ServerMsg<'_>,
    mut queue: impl crate::client::ClientMsgSink<'static>,
) -> bool {
    let retval = msg.kind == PING;
    if retval {
        let mut reply = ClientMsg::new_cmd(PONG);
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
        mut queue: QueueEditGuard<'_>,
        _: SenderRef<'_, Self::Value>,
    ) -> bool {
        pong(msg, &mut queue);
        false
    }
}

impl SelfMadeHandler for AutoPong {
    type Receiver<Spec: ChannelSpec> = ();

    fn queue_msgs(&self, _: QueueEditGuard<'_>) {}

    fn make_channel<Spec: ChannelSpec>(
        _: &Spec,
    ) -> (Arc<dyn Sender<Value = Self::Value>>, Self::Receiver<Spec>) {
        (Arc::new(crate::client::channel::ClosedSender::default()), ())
    }
}

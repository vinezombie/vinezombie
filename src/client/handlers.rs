//! Useful handler implementations.

mod autoreply;
mod ping;

pub use autoreply::*;
pub use ping::*;

use super::{queue::QueueEditGuard, Handler, SelfMadeHandler};
use crate::{
    client::ClientState,
    ircmsg::{ServerMsg, ServerMsgKindRaw},
    names::{NameValued, ServerMsgKind},
    util::FlatMap,
};

/// [`Handler`] that yields every message it receives until the channel closes.
#[derive(Clone, Copy, Debug, Default)]
pub struct YieldAll;

impl Handler for YieldAll {
    type Value = ServerMsg<'static>;

    fn handle(
        &mut self,
        msg: &crate::ircmsg::ServerMsg<'_>,
        _: &mut ClientState,
        _: QueueEditGuard<'_>,
        mut channel: super::channel::SenderRef<'_, Self::Value>,
    ) -> bool {
        channel.send(msg.clone().owning());
        !channel.may_send()
    }

    fn wants_owning(&self) -> bool {
        true
    }
}

impl SelfMadeHandler for YieldAll {
    type Receiver<Spec: super::channel::ChannelSpec> = Spec::Queue<Self::Value>;

    fn queue_msgs(&self, _: &ClientState, _: QueueEditGuard<'_>) {}

    fn make_channel<Spec: super::channel::ChannelSpec>(
        spec: &Spec,
    ) -> (Box<dyn super::channel::Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>) {
        spec.new_queue()
    }
}

type Parser<T> = dyn FnMut(ServerMsg<'static>) -> Option<T> + Send;

/// [`Handler`] that yields every message that successfully parses into `T`.
#[derive(Default)]
pub struct YieldParsed<T>(FlatMap<(ServerMsgKindRaw<'static>, Box<Parser<T>>)>);

impl<T: 'static + Send> YieldParsed<T> {
    /// Creates a new instance that parses no messages.
    pub const fn new() -> Self {
        YieldParsed(FlatMap::new())
    }
    /// Creates an instance that parses messages of the provided `kind`.
    pub fn just<N>(kind: N) -> Self
    where
        N: NameValued<ServerMsgKind, Value<'static> = T>,
    {
        YieldParsed(FlatMap::singleton((
            kind.as_raw().clone(),
            Box::new(|raw| N::from_union(&raw).ok()),
        )))
    }
    /// Creates an instance that parses messages of the provided `kind` and maps them into `T`.
    pub fn just_map<U, N, F>(kind: N, mut f: F) -> Self
    where
        N: NameValued<ServerMsgKind, Value<'static> = U>,
        F: FnMut(U) -> Option<T> + 'static + Send,
    {
        YieldParsed(FlatMap::singleton((
            kind.as_raw().clone(),
            Box::new(move |raw| N::from_union(&raw).ok().and_then(&mut f)),
        )))
    }
    /// Extends `self` to also parse messages of another kind.
    pub fn with<N>(mut self, kind: N) -> Self
    where
        N: NameValued<ServerMsgKind, Value<'static> = T>,
    {
        self.0.edit().insert((kind.as_raw().clone(), Box::new(|raw| N::from_union(&raw).ok())));
        self
    }
    /// Extends `self` to also parse messages of another kind and map them into `T`.
    pub fn with_map<U, N, F>(mut self, kind: N, mut f: F) -> Self
    where
        N: NameValued<ServerMsgKind, Value<'static> = U>,
        F: FnMut(U) -> Option<T> + 'static + Send,
    {
        self.0.edit().insert((
            kind.as_raw().clone(),
            Box::new(move |raw| N::from_union(&raw).ok().and_then(&mut f)),
        ));
        self
    }
}

impl<T: 'static + Send> Handler for YieldParsed<T> {
    type Value = T;

    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        _: &mut ClientState,
        _: QueueEditGuard<'_>,
        mut channel: super::channel::SenderRef<'_, Self::Value>,
    ) -> bool {
        let msg = msg.clone().owning();
        let Some((_, parser)) = self.0.get_mut(&msg.kind) else {
            return false;
        };
        let Some(parsed) = parser(msg) else {
            return false;
        };
        !channel.send(parsed)
    }

    fn wants_owning(&self) -> bool {
        true
    }
}

impl<T: 'static + Send> SelfMadeHandler for YieldParsed<T> {
    type Receiver<Spec: super::channel::ChannelSpec> = Spec::Queue<T>;

    fn queue_msgs(&self, _: &ClientState, _: QueueEditGuard<'_>) {}

    fn make_channel<Spec: super::channel::ChannelSpec>(
        spec: &Spec,
    ) -> (Box<dyn super::channel::Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>) {
        spec.new_queue()
    }
}

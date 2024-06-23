//! Useful handler implementations.

mod autoreply;
mod ping;
mod track;

use std::ops::ControlFlow;

pub use {autoreply::*, ping::*, track::*};

use super::{cf_discard, channel::SenderRef, queue::QueueEditGuard, Handler, SelfMadeHandler};
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
    ) -> ControlFlow<()> {
        crate::client::cf_discard(channel.send(msg.clone().owning()))
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

type Parser<T> = dyn FnMut(ServerMsg<'static>, SenderRef<T>) -> ControlFlow<()> + Send;

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
            Box::new(|raw, mut sender| {
                if let Ok(parsed) = N::from_union(&raw) {
                    cf_discard(sender.send(parsed))
                } else {
                    ControlFlow::Continue(())
                }
            }),
        )))
    }
    #[deprecated = "Function parameter changing in 0.4. Use `YieldAll::just_flat_map` instead."]
    /// Creates an instance that parses messages of the provided `kind`
    /// and optionally maps them into `T`.
    pub fn just_map<U, N, F>(kind: N, f: F) -> Self
    where
        N: NameValued<ServerMsgKind, Value<'static> = U>,
        F: FnMut(U) -> Option<T> + 'static + Send,
    {
        Self::just_flat_map(kind, f)
    }
    /// Creates an instance that parses messages of the provided `kind`
    /// and maps them into a sequence of `T`s.
    ///
    /// The returned iterable must be finite and should be relatively short,
    /// as the handler will attempt to send everything over the channel before returning.
    /// The iterator may not be fully consumed if the channel closes.
    pub fn just_flat_map<U, N, I, F>(kind: N, mut f: F) -> Self
    where
        N: NameValued<ServerMsgKind, Value<'static> = U>,
        I: IntoIterator<Item = T>,
        F: FnMut(U) -> I + 'static + Send,
    {
        YieldParsed(FlatMap::singleton((
            kind.as_raw().clone(),
            Box::new(move |raw, mut sender| {
                if let Ok(parsed) = N::from_union(&raw) {
                    for value in f(parsed) {
                        cf_discard(sender.send(value))?;
                    }
                }
                ControlFlow::Continue(())
            }),
        )))
    }
    /// Extends `self` to also parse messages of another kind.
    pub fn with<N>(mut self, kind: N) -> Self
    where
        N: NameValued<ServerMsgKind, Value<'static> = T>,
    {
        self.0.edit().insert((
            kind.as_raw().clone(),
            Box::new(|raw, mut sender| {
                if let Ok(parsed) = N::from_union(&raw) {
                    cf_discard(sender.send(parsed))
                } else {
                    ControlFlow::Continue(())
                }
            }),
        ));
        self
    }
    #[deprecated = "Function parameter changing in 0.4. Use `YieldAll::with_flat_map` instead."]
    /// Extends `self` to also parse messages of another kind and map them into `T`.
    pub fn with_map<U, N, F>(self, kind: N, f: F) -> Self
    where
        N: NameValued<ServerMsgKind, Value<'static> = U>,
        F: FnMut(U) -> Option<T> + 'static + Send,
    {
        self.with_flat_map(kind, f)
    }
    /// Extends `self` to also parse messages of another kind and
    /// map them into a sequence of `T`s.
    ///
    /// The returned iterable must be finite and should be relatively short,
    /// as the handler will attempt to send everything over the channel before returning.
    /// The iterator may not be fully consumed if the channel closes.
    pub fn with_flat_map<U, N, I, F>(mut self, kind: N, mut f: F) -> Self
    where
        N: NameValued<ServerMsgKind, Value<'static> = U>,
        I: IntoIterator<Item = T>,
        F: FnMut(U) -> I + 'static + Send,
    {
        self.0.edit().insert((
            kind.as_raw().clone(),
            Box::new(move |raw, mut sender| {
                if let Ok(parsed) = N::from_union(&raw) {
                    for value in f(parsed) {
                        cf_discard(sender.send(value))?;
                    }
                }
                ControlFlow::Continue(())
            }),
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
        channel: super::channel::SenderRef<'_, Self::Value>,
    ) -> ControlFlow<()> {
        let msg = msg.clone().owning();
        if let Some((_, parser)) = self.0.get_mut(&msg.kind) {
            parser(msg, channel)?;
        };
        ControlFlow::Continue(())
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

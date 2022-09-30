//! Common handlers for writing IRC applications.

pub mod init;

use std::{collections::VecDeque, iter::FusedIterator, time::Duration};

use crate::msg::{
    data::Ping, ClientMsg, DefaultMsgWriter, MsgWriter, NewMsgWriter, RawClientMsg, RawServerMsg,
};
use graveseed::{
    handler::{
        inline::{InlineHandler, RateLimited},
        Action, Handler,
    },
    time::rate_limiters::LeakyBucket,
};

/// A collection of [`MsgWriter`]s usable as an iterator or a [`Handler`].
#[derive(Default)]
pub struct Send<'a>(VecDeque<Box<dyn MsgWriter<'a> + 'a>>);

impl<'a> Send<'a> {
    /// Creates a new empty message burst.
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds the provided [`ClientMsg`] to the end of this burst.
    ///
    /// The data of the message must have a default message writer
    /// whose `Options` implement `Default`.
    pub fn with_msg<W>(self, what: ClientMsg<'a, W>) -> Self
    where
        W: DefaultMsgWriter<'a> + 'a,
        <<W as DefaultMsgWriter<'a>>::Writer as NewMsgWriter<'a>>::Options: Default,
    {
        self.with_writer(W::Writer::new_msg_writer(what, Default::default()))
    }
    /// Adds the provided [`ClientMsg`] to the end of this burst using `opts`
    /// for writer options.
    ///
    /// The data of the message must have a default message writer.
    pub fn with_msg_and_opts<W>(
        self,
        what: ClientMsg<'a, W>,
        opts: <<W as DefaultMsgWriter<'a>>::Writer as NewMsgWriter<'a>>::Options,
    ) -> Self
    where
        W: DefaultMsgWriter<'a> + 'a,
    {
        self.with_writer(W::Writer::new_msg_writer(what, opts))
    }

    /// Adds the provided boxed [`MsgWriter`] to the end of this burst.
    pub fn with_writer(mut self, what: Box<dyn MsgWriter<'a> + 'a>) -> Self {
        self.0.push_back(what);
        self
    }
}

impl<'a, T: Default> Handler<T, RawServerMsg<'a>, RawClientMsg<'static>> for Send<'static> {
    fn handle(
        self: Box<Self>,
        _: Option<&RawServerMsg<'a>>,
    ) -> Action<T, RawServerMsg<'a>, RawClientMsg<'static>> {
        Action::done(T::default()).with_send(self)
    }

    fn timeout(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::ZERO)
    }
}

impl<'a> Iterator for Send<'a> {
    type Item = RawClientMsg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let back = self.0.pop_front()?;
        let (msg, cont) = back.write_msg();
        if let Some(cont) = cont {
            self.0.push_front(cont);
        }
        Some(msg)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.0.len(), self.0.is_empty().then_some(0))
    }
}

/// Creates a rate-limiting inline handler.
///
/// RFC 1459 recommends a burst of up to 5 messages,
/// followed by one message every 2 seconds.
/// By specifying `None` for both arguments,
/// these recommendations will be used.
pub fn rate_limited_queue<S>(
    rate: Option<Duration>,
    burst: Option<u32>,
) -> RateLimited<VecDeque<S>, LeakyBucket> {
    let rate = rate.unwrap_or_else(|| Duration::from_secs(2));
    let max = rate * burst.unwrap_or(5);
    RateLimited::new::<S>(LeakyBucket::new(rate, max))
}

/// Inline handler that logs messages at the debug level.
///
/// This should NOT be used in production,
/// as it can and will log messages containing sensitive information unredacted,
/// such as AUTHENTICATE messages.
#[cfg(feature = "log")]
#[derive(Clone, Copy, Debug, Default)]
pub struct DebugLog;

#[cfg(feature = "log")]
impl<'a> InlineHandler<RawServerMsg<'a>, RawClientMsg<'static>> for DebugLog {
    fn handle_recv(&mut self, msg: Option<RawServerMsg<'a>>) -> Option<RawServerMsg<'a>> {
        if let Some(ref msg) = msg {
            log::debug!("recv: {msg}");
        }
        msg
    }

    fn handle_send(
        &mut self,
        msg: Result<RawClientMsg<'static>, graveseed::time::Deadline>,
    ) -> Result<RawClientMsg<'static>, graveseed::time::Deadline> {
        if let Ok(ref msg) = msg {
            log::debug!("send: {msg}");
        }
        msg
    }
}

impl<'a> FusedIterator for Send<'a> {}

// Do NOT impl DoubleEndedIterator.

/// Handler that auto-replies to pings.
///
/// You almost always want to have this handler in use
/// in order to avoid being auto-disconnected.
#[derive(Clone, Copy, Debug, Default)]
pub struct AutoPong;

impl<'a> Handler<(), RawServerMsg<'a>, RawClientMsg<'static>> for AutoPong {
    fn handle(
        self: Box<Self>,
        msg: Option<&RawServerMsg<'a>>,
    ) -> graveseed::handler::Action<(), RawServerMsg<'a>, RawClientMsg<'static>> {
        let retval = Action::next(self);
        let Some(msg) = msg.and_then(|m| m.to_parsed::<Ping<'a>>(())) else {
            return retval;
        };
        let reply = ClientMsg::new(msg.data.pong());
        retval.with_send(Send::new().with_msg(reply))
    }
}

//! Common handlers for writing IRC applications.

// pub mod init;

use crate::ircmsg::{ClientMsg, ServerMsg};
use graveseed::{
    handler::{
        inline::{InlineHandler, RateLimited},
        Action, Handler,
    },
    time::rate_limiters::LeakyBucket,
};
use std::{collections::VecDeque, time::Duration};

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
impl<'a> InlineHandler<ServerMsg<'a>, ClientMsg<'static>> for DebugLog {
    fn handle_recv(&mut self, msg: Option<ServerMsg<'a>>) -> Option<ServerMsg<'a>> {
        if let Some(ref msg) = msg {
            log::debug!("recv: {msg}");
        }
        msg
    }

    fn handle_send(
        &mut self,
        msg: Result<ClientMsg<'static>, graveseed::time::Deadline>,
    ) -> Result<ClientMsg<'static>, graveseed::time::Deadline> {
        if let Ok(ref msg) = msg {
            log::debug!("send: {msg}");
        }
        msg
    }
}

/// Handler that auto-replies to pings.
///
/// You almost always want to have this handler in use
/// in order to avoid being auto-disconnected.
#[derive(Clone, Copy, Debug, Default)]
pub struct AutoPong;

impl<'a> Handler<(), ServerMsg<'a>, ClientMsg<'static>> for AutoPong {
    fn handle(
        self: Box<Self>,
        msg: Option<&ServerMsg<'a>>,
    ) -> graveseed::handler::Action<(), ServerMsg<'a>, ClientMsg<'static>> {
        use crate::known::cmd::{PING, PONG};
        let retval = Action::next(self);
        let Some(msg) = msg else {
            return retval;
        };
        if msg.kind != PING {
            return retval;
        }
        let mut reply = ClientMsg::new_cmd(PONG);
        reply.args = msg.args.clone().owning();
        retval.with_send(std::iter::once(reply))
    }
}

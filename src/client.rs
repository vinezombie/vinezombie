//! Utilities for building IRC clients or bots.

pub mod auth;
pub mod conn;
pub mod nick;
mod queue;
pub mod register;

pub use queue::*;

use crate::ircmsg::{ClientMsg, ServerMsg};
use crate::known::cmd::{PING, PONG};

/// Returns a message in reply to a server ping.
pub fn pong(msg: &ServerMsg<'_>) -> Option<ClientMsg<'static>> {
    (msg.kind == PING).then(|| {
        let mut ret = ClientMsg::new_cmd(PONG);
        ret.args = msg.args.clone().owning();
        ret
    })
}

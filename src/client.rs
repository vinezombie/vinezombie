#![doc = include_str!("../doc/rustdoc/client.md")]

pub mod auth;
pub mod cap;
pub mod conn;
mod handler;
pub mod nick;
mod queue;
pub mod register;
mod sink;
#[cfg(feature = "tls")]
pub mod tls;

pub use {handler::*, queue::*, sink::*};

use crate::consts::cmd::{PING, PONG};
use crate::ircmsg::{ClientMsg, ServerMsg};

/// Returns a message in reply to a server ping.
pub fn pong(msg: &ServerMsg<'_>) -> Option<ClientMsg<'static>> {
    (msg.kind == PING).then(|| {
        let mut ret = ClientMsg::new_cmd(PONG);
        ret.args = msg.args.clone().owning();
        ret
    })
}

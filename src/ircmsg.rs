#![doc = include_str!("../doc/rustdoc/ircmsg.md")]

// Placing this here for macros.
#[macro_use]
mod common;

mod args;
mod client;
mod numeric;
mod server;
mod servermsgkind;
mod source;
mod tags;
mod targeted;
#[cfg(test)]
mod tests;

pub use self::{
    args::*, client::*, numeric::*, server::*, servermsgkind::*, source::*, tags::*, targeted::*,
};
use common::*;

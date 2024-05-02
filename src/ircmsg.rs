#![doc = include_str!("../doc/rustdoc/ircmsg.md")]

mod args;
mod client;
mod codec;
mod ctcp;
mod numeric;
mod server;
mod servermsgkind;
mod source;
mod tags;
mod targeted;
#[cfg(test)]
mod tests;

pub use self::{
    args::*, client::*, codec::*, ctcp::*, numeric::*, server::*, servermsgkind::*, source::*,
    tags::*, targeted::*,
};

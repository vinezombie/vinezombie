//! Minimally-processed IRC messages.

// Placing this here for macros.
#[macro_use]
mod common;

mod args;
mod client;
mod numeric;
mod server;
mod servermsgkind;
mod tags;
#[cfg(test)]
mod tests;

pub use self::{args::*, client::*, numeric::*, server::*, servermsgkind::*, tags::*};
pub(self) use common::*;

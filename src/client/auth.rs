//! Traits and types related to authentication
//!
//! Specific SASL authenciation methods can be found in `sasl`.

#[cfg(feature = "base64")]
mod handler;
pub mod sasl;
mod secret;

#[cfg(feature = "base64")]
pub use handler::*;
pub use secret::*;

use crate::{ircmsg::ClientMsg, string::Arg};

/// Returns the [`ClientMsg`] for aborting authentication.
pub fn msg_abort() -> ClientMsg<'static> {
    use crate::consts::cmd::AUTHENTICATE;
    let mut msg = ClientMsg::new_cmd(AUTHENTICATE);
    msg.args.edit().add_word(crate::consts::STAR);
    msg
}

/// The logic of a SASL mechanism.
pub trait SaslLogic {
    /// Handles data sent by the server.
    fn reply<'a>(
        &'a mut self,
        data: &[u8],
    ) -> Result<&'a [u8], Box<dyn std::error::Error + Send + Sync>>;
}

/// SASL mechanisms.
pub trait Sasl {
    /// The name of this mechanism, as the client requests it.
    fn name(&self) -> Arg<'static>;
    /// Returns the logic for this mechanism as a [`SaslLogic]`.
    fn logic(&self) -> std::io::Result<Box<dyn SaslLogic>>;
}

/// Enum of included SASL mechanisms and options for them.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum AnySasl<S: Secret> {
    External(sasl::External),
    Plain(sasl::Plain<S>),
}

impl<S: Secret + std::fmt::Debug> std::fmt::Debug for AnySasl<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnySasl::External(s) => s.fmt(f),
            AnySasl::Plain(s) => s.fmt(f),
        }
    }
}

impl<S: Secret + 'static> Sasl for AnySasl<S> {
    fn name(&self) -> Arg<'static> {
        match self {
            AnySasl::External(s) => s.name(),
            AnySasl::Plain(s) => s.name(),
        }
    }

    fn logic(&self) -> std::io::Result<Box<dyn SaslLogic>> {
        match self {
            AnySasl::External(s) => s.logic(),
            AnySasl::Plain(s) => s.logic(),
        }
    }
}

impl<S: Secret> From<sasl::External> for AnySasl<S> {
    fn from(value: sasl::External) -> Self {
        AnySasl::External(value)
    }
}

impl<S: Secret> From<sasl::Plain<S>> for AnySasl<S> {
    fn from(value: sasl::Plain<S>) -> Self {
        AnySasl::Plain(value)
    }
}

//! Traits and types related to authentication
//!
//! Specific SASL authenciation methods can be found in `sasl`.

use crate::{ircmsg::ClientMsg, known::cmd::AUTHENTICATE, string::Arg};

#[cfg(feature = "base64")]
mod handler;
pub mod sasl;

#[cfg(feature = "base64")]
pub use handler::*;

type BoxedErr = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Returns the initial [`ClientMsg`] to begin authentication.
pub fn msg_auth(sasl: &(impl Sasl + ?Sized)) -> ClientMsg<'static> {
    let mut msg = ClientMsg::new_cmd(AUTHENTICATE);
    msg.args.add_word(sasl.name());
    msg
}

/// Returns the [`ClientMsg`] for aborting authentication.
pub fn msg_abort() -> ClientMsg<'static> {
    let mut msg = ClientMsg::new_cmd(AUTHENTICATE);
    msg.args.add_word(crate::known::STAR);
    msg
}

/// Trait for types that can store sensitive byte strings.
pub trait Secret {
    /// Appends the secret's bytes to the provided buffer.
    fn load(&self, data: &mut Vec<u8>) -> Result<(), BoxedErr>;
    /// Sets this secret's value.
    fn store(&mut self, data: Vec<u8>) -> Result<(), BoxedErr>;
    /// Irreversibly destroys any sensitive data owned by self.
    ///
    /// The actions performed by this method should ideally be performed by [`Drop`] impl,
    /// however this method provides a means of doing so when
    /// either the object will not be dropped outright, or when `Secret`
    /// is being implemented on a foreign type whose `Drop` impl isn't secure.
    fn destroy(&mut self) {}
}

/// Zero-added-security implementation of [`Secret`].
impl Secret for Vec<u8> {
    fn load(&self, data: &mut Vec<u8>) -> Result<(), BoxedErr> {
        let lens = data.len() + self.len();
        if data.capacity() < lens {
            let mut vec = Vec::with_capacity(lens);
            vec.extend_from_slice(data.as_slice());
            #[cfg(feature = "zeroize")]
            zeroize::Zeroize::zeroize(data);
            *data = vec;
        }
        data.extend_from_slice(self.as_slice());
        Ok(())
    }

    fn store(&mut self, data: Vec<u8>) -> Result<(), BoxedErr> {
        self.destroy();
        *self = data;
        Ok(())
    }

    fn destroy(&mut self) {
        #[cfg(feature = "zeroize")]
        zeroize::Zeroize::zeroize(self);
    }
}

/// The logic of a SASL mechanism.
pub trait SaslLogic {
    /// Handles data sent by the server.
    fn reply<'a>(&'a mut self, data: &[u8]) -> Result<&'a [u8], BoxedErr>;
}

/// SASL mechanisms.
pub trait Sasl: std::fmt::Debug {
    /// The name of this mechanism, as the client requests it.
    fn name(&self) -> Arg<'static>;
    /// Returns the logic for this mechanism as a [`SaslLogic]`.
    fn logic(&self) -> Result<Box<dyn SaslLogic>, BoxedErr>;
}

/// Enum of included SASL mechanisms and options for them.
#[derive(PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum AnySasl<S: Secret> {
    External(sasl::External),
    Plain(sasl::Plain<S>),
}

impl<S: Secret> std::fmt::Debug for AnySasl<S> {
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

    fn logic(&self) -> Result<Box<dyn SaslLogic>, BoxedErr> {
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

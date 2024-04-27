//! Traits and types related to authentication
//!
//! Specific SASL authenciation methods can be found in `sasl`.

#[cfg(feature = "base64")]
mod handler;
pub mod sasl;
mod secret;
#[cfg(test)]
mod tests;

#[cfg(feature = "base64")]
pub use handler::*;
pub use secret::*;

use crate::{
    ircmsg::ClientMsg,
    string::{Arg, SecretBuf},
};

/// Returns the [`ClientMsg`] for aborting authentication.
pub fn msg_abort() -> ClientMsg<'static> {
    use crate::names::cmd::AUTHENTICATE;
    let mut msg = ClientMsg::new(AUTHENTICATE);
    msg.args.edit().add_word(crate::names::STAR);
    msg
}

/// The logic of one SASL mechanism.
pub trait SaslLogic: Send + 'static {
    /// The name of the mechanism.
    fn name(&self) -> Arg<'static>;

    /// Handles data sent by the server.
    ///
    /// Errors to indicate that the server's implementation of a given mechanism is broken.
    /// This type is not responsible for validating the server; that should be handled by
    /// TLS or other secure connection system.
    fn reply(
        &mut self,
        input: &[u8],
        output: &mut SecretBuf,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Returns a hint for how large the output buffer should be.
    fn size_hint(&self) -> usize {
        0
    }
}

/// Configuration for doing SASL authentication.
///
/// This is separate from [`SaslLogic`] with the idea that these types may be
/// (de)serializeable, where the actual state for implementing these mechanisms may be
/// significantly more complex.
/// Additionally, the construction of these types is meant to be responsible for
/// loading the necessary secrets into memory, so that the actual authenticator logic
/// doesn't have to worry about it.
pub trait Sasl {
    /// Returns the logic for this mechanism as a [`SaslLogic`].
    ///
    /// Some `Sasl` implementations represent configuration for a collection of mechanisms,
    fn logic(&self) -> Vec<Box<dyn SaslLogic>>;
}

/// A queue of SASL authenticators to try in order.
#[derive(Default)]
pub struct SaslQueue {
    queue: std::collections::VecDeque<Box<dyn SaslLogic>>,
}

impl SaslQueue {
    /// Creates a new, empty list of SASL authenticators.
    pub const fn new() -> Self {
        SaslQueue { queue: std::collections::VecDeque::new() }
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns the number of SASL authenticators in `self.`
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Adds a SASL authenticator to the end of the list.
    pub fn push(&mut self, sasl: &(impl Sasl + ?Sized)) {
        let mut vec_deque: std::collections::VecDeque<_> = sasl.logic().into();
        self.queue.append(&mut vec_deque);
    }

    /// Returns the next SASL authenticator to attempt.
    pub fn pop(&mut self) -> Option<Box<dyn SaslLogic>> {
        self.queue.pop_front()
    }

    /// Retains only SASL authenticators for which
    /// the provided function returns `true` when passed their names.
    pub fn retain(&mut self, supported: &(impl Fn(&Arg<'_>) -> bool + ?Sized)) {
        self.queue.retain(|l| supported(&l.name()));
    }

    /// Cleares the queue.
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

impl<'a, S: Sasl + ?Sized> std::iter::FromIterator<&'a S> for SaslQueue {
    fn from_iter<T: IntoIterator<Item = &'a S>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let mut retval = Self::new();
        retval.queue.reserve(iter.size_hint().0);
        for sasl in iter {
            retval.push(sasl);
        }
        retval
    }
}

impl From<Vec<Box<dyn SaslLogic>>> for SaslQueue {
    fn from(value: Vec<Box<dyn SaslLogic>>) -> Self {
        SaslQueue { queue: value.into() }
    }
}

/// Enum of included SASL mechanisms and options for them.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde_derive::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound(deserialize = "'de: 'static, S: LoadSecret + serde::Deserialize<'de>"))
)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum AnySasl<S: LoadSecret> {
    External(sasl::External),
    Password(sasl::Password<S>),
}

impl<S: LoadSecret + std::fmt::Debug> std::fmt::Debug for AnySasl<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnySasl::External(s) => s.fmt(f),
            AnySasl::Password(s) => s.fmt(f),
        }
    }
}

impl<S: LoadSecret + 'static> Sasl for AnySasl<S> {
    fn logic(&self) -> Vec<Box<dyn SaslLogic>> {
        match self {
            AnySasl::External(s) => s.logic(),
            AnySasl::Password(s) => s.logic(),
        }
    }
}

impl<S: LoadSecret> From<sasl::External> for AnySasl<S> {
    fn from(value: sasl::External) -> Self {
        AnySasl::External(value)
    }
}

impl<S: LoadSecret> From<sasl::Password<S>> for AnySasl<S> {
    fn from(value: sasl::Password<S>) -> Self {
        AnySasl::Password(value)
    }
}

//! Traits and types related to authentication
//!
//! Specific SASL authenciation methods can be found in `sasl`.

#[cfg(feature = "base64")]
mod handler;
pub mod sasl;
mod secret;

use std::borrow::Borrow;

#[cfg(feature = "base64")]
pub use handler::*;
pub use secret::*;

use crate::{ircmsg::ClientMsg, string::Arg};

/// Returns the [`ClientMsg`] for aborting authentication.
pub fn msg_abort() -> ClientMsg<'static> {
    use crate::names::cmd::AUTHENTICATE;
    let mut msg = ClientMsg::new(AUTHENTICATE);
    msg.args.edit().add_word(crate::names::STAR);
    msg
}

/// The logic of a SASL mechanism.
pub trait SaslLogic: Send {
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

/// A queue of SASL authenticators to try in order.
#[derive(Default)]
pub struct SaslQueue {
    seq: std::collections::VecDeque<(Arg<'static>, Box<dyn SaslLogic>)>,
    had_values: bool,
}

impl SaslQueue {
    /// Creates a new, empty list of SASL authenticators.
    pub const fn new() -> Self {
        SaslQueue { seq: std::collections::VecDeque::new(), had_values: false }
    }

    /// Returns `true` if this queue has or had values in it.
    ///
    /// This is also `true` if this queue was constructed from a non-empty iterator.
    pub const fn had_values(&self) -> bool {
        self.had_values
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.seq.is_empty()
    }

    /// Returns the number of SASL authenticators in `self.`
    pub fn len(&self) -> usize {
        self.seq.len()
    }

    /// Adds a SASL authenticator to the end of the list.
    pub fn push(&mut self, sasl: &(impl Sasl + ?Sized)) -> std::io::Result<()> {
        let logic = sasl.logic()?;
        let name = sasl.name();
        self.seq.push_back((name, logic));
        self.had_values = true;
        Ok(())
    }

    /// Returns a pair containing the name and logic of a SASL authenticator.
    pub fn pop(&mut self) -> Option<(Arg<'static>, Box<dyn SaslLogic>)> {
        self.seq.pop_front()
    }

    /// Removes all SASL authenticators with a protocol name matching `name`.
    pub fn remove(&mut self, name: &Arg<'_>) {
        self.seq.retain(|(k, _)| *k != *name);
    }

    /// Removes all SASL authenticators except those with a protocol name in `names`.
    pub fn retain(&mut self, names: &std::collections::BTreeSet<impl Borrow<[u8]> + Ord>) {
        self.seq.retain(|(k, _)| names.contains(k.borrow()));
    }

    /// Cleares the queue.
    pub fn clear(&mut self) {
        self.seq.clear();
    }
}

impl<'a, S: Sasl + ?Sized> std::iter::FromIterator<&'a S> for SaslQueue {
    fn from_iter<T: IntoIterator<Item = &'a S>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let mut seq = std::collections::VecDeque::with_capacity(iter.size_hint().0);
        let mut had_values = false;
        for sasl in iter {
            had_values = true;
            let Ok(logic) = sasl.logic() else {
                continue;
            };
            let name = sasl.name();
            seq.push_back((name, logic));
        }
        SaslQueue { seq, had_values }
    }
}

impl std::fmt::Debug for SaslQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_list();
        for (arg, _) in self.seq.iter() {
            f.entry(arg);
        }
        f.finish()
    }
}

/// Enum of included SASL mechanisms and options for them.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
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

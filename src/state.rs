//! Representations of IRC network and connection state, including users and channels.
#![allow(missing_docs)]

use crate::string::{Arg, Key, Nick, Word};
use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

/// The observed state of an entire IRC network.
#[derive(Clone, Default, Debug)]
pub struct Network {
    // TODO: Channel lists, user lists.
    /// The capabilities that are available on this network.
    pub caps: BTreeSet<Arg<'static>>,
    /// The values of some capabilities that are available on this network.
    ///
    /// Presence of a key in this map does not imply that a capability is available,
    /// that it has the specified value if it is available.
    /// You should ideally ensure that the set of keys is a subset of [Network::caps].
    pub caps_values: BTreeMap<Arg<'static>, Word<'static>>,
}

impl Network {
    /// Adds a capability.
    pub fn cap_add(&mut self, cap: Arg<'static>, value: Word<'static>) {
        if !value.is_empty() {
            self.caps_values.insert(cap.clone(), value);
        }
        self.caps.insert(cap);
    }
    /// Looks up a capability, returning its value if it exists.
    pub fn cap_get<K>(&mut self, cap: &K) -> Option<Word<'static>>
    where
        K: Ord + ?Sized,
        Arg<'static>: Borrow<K>,
    {
        if !self.caps.contains(cap) {
            return None;
        }
        Some(self.caps_values.get(cap).cloned().unwrap_or_default())
    }
    /// Removes a capability.
    pub fn cap_del<K>(&mut self, cap: &K)
    where
        K: Ord + ?Sized,
        Arg<'static>: Borrow<K>,
    {
        self.caps_values.remove(cap);
        self.caps.remove(cap);
    }
}

/// The state of an active client connection to an IRC network.
#[derive(Clone, Debug)]
pub struct Connection {
    /// The nickname currently in use.
    pub nick: Nick<'static>,
    /// The capabilities that have been enabled for this connection.
    pub caps: BTreeSet<Key<'static>>,
    /// The state of the network.
    pub net: Arc<RwLock<Network>>,
}

impl Connection {
    /// Creates a new connection state tracker using the given nick and shared network state.
    pub fn new(nick: Nick<'static>, net: Option<Arc<RwLock<Network>>>) -> Self {
        Connection { nick, caps: BTreeSet::new(), net: net.unwrap_or_default() }
    }
}

/// Information on who set channel metadata.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ChanMetaSetter {
    user: Option<crate::source::Source<'static>>,
    when: Option<std::time::SystemTime>,
}

/// User metadata.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum UserMeta {
    UnknownMode(u8),
    Invisible,
    GetsWallops,
    Bot,
}

/// User metadata keys.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum UserMetaKey {
    UnknownMode(u8),
    UserHost,
    Realname,
    Account,
    Away,
}

/// Channel metadata.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ChanMeta {
    UnknownMode(u8),
    InviteOnly,
    Moderated,
    Secret,
    TopicLock,
    NoExternalSend,
    NoFormat,
    Ban(Word<'static>),
    Quiet(Word<'static>),
    Invex(Word<'static>),
    Exempt(Word<'static>),
}

/// Channel metadata keys.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ChanMetaKey {
    UnknownMode(u8),
    Topic,
    Key,
    Forward,
    Limit,
    UserPrefix(Nick<'static>),
}

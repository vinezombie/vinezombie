//! Representations of IRC network and connection state, including users and channels.

use crate::known::{Cap, MaybeKnown};
use crate::msg::data::CapStatus;
use crate::IrcWord;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

/// The observed state of an entire IRC network.
#[derive(Clone, Default, Debug)]
pub struct Network {
    // TODO: Channel lists, user lists.
    /// The capabilities that are available on this network.
    pub caps: BTreeSet<MaybeKnown<'static, Cap>>,
    /// The values of some capabilities that are available on this network.
    ///
    /// Presence of a key in this map does not imply that a capability is available,
    /// that it has the specified value if it is available.
    /// You should ideally ensure that the set of keys is a subset of [Network::caps].
    pub caps_values: BTreeMap<MaybeKnown<'static, Cap>, IrcWord<'static>>,
}

impl Network {
    /// Adds a capability.
    pub fn cap_add(
        &mut self,
        cap: impl Into<MaybeKnown<'static, Cap>>,
        value: Option<IrcWord<'static>>,
    ) {
        let cap = cap.into();
        if let Some(value) = value {
            self.caps_values.insert(cap.clone(), value);
        }
        self.caps.insert(cap);
    }
    /// Removes a capability.
    pub fn cap_del(&mut self, cap: impl Into<MaybeKnown<'static, Cap>>) {
        let cap = cap.into();
        self.caps_values.remove(&cap);
        self.caps.remove(&cap);
    }
}

/// The state of an active connection to an IRC network.
#[derive(Clone, Debug)]
pub struct Connection {
    /// The nickname currently in use.
    pub nick: IrcWord<'static>,
    /// The capabilities that have been enabled for this connection.
    pub caps: BTreeSet<MaybeKnown<'static, Cap>>,
    /// The state of the network.
    pub net: Arc<RwLock<Network>>,
}

impl Connection {
    /// Creates a new connection state tracker using the given nick and shared network state.
    pub fn new(nick: IrcWord<'static>, net: Option<Arc<RwLock<Network>>>) -> Self {
        Connection { nick, caps: BTreeSet::new(), net: net.unwrap_or_default() }
    }
    /// Updates the statuses of capabilities.
    pub fn update_caps(
        &mut self,
        iter: impl IntoIterator<Item = (MaybeKnown<'static, Cap>, CapStatus)>,
    ) {
        // TODO: Use LazyCell whenever that stabilizes to acquire the net lock lazily.
        let mut net = self.net.write().unwrap();
        for (cap, status) in iter {
            match status {
                CapStatus::Available(value) => net.cap_add(cap, value),
                CapStatus::Enabled => {
                    net.caps.insert(cap.clone());
                    self.caps.insert(cap);
                }
                CapStatus::Unchanged => (),
                CapStatus::Disabled(avail) => {
                    self.caps.remove(&cap);
                    if !avail {
                        net.cap_del(cap);
                    }
                }
            }
        }
    }
}

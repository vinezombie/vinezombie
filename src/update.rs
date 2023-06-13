//! Serializable network events and state updates.
#![allow(missing_docs)]

use crate::{
    ircmsg::{ServerMsg, Source},
    state::*,
    string::{Arg, Bytes, Line, Nick, Word},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Header {
    update: Option<u32>,
    cmd: Option<(u16, u16)>,
}

/// Message from server to client describing an IRC state update.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Update<'a> {
    /// An unknown server message.
    Unknown(ServerMsg<'a>),
    /// Synchronization of user sets for either a channel or the network.
    SyncUsers { chan: Option<Arg<'a>>, nicks: BTreeSet<Nick<'a>> },
    /// A user has changed their nickname.
    ChgNick { old: Nick<'a>, new: Nick<'a> },
    /// A new user has joined either a channel or the network.
    Joined { chan: Option<Arg<'a>>, nick: Nick<'a> },
    /// A user has left either a channel or the network.
    Left { who: Nick<'a>, chan: Option<Arg<'a>>, msg: Line<'a> },
    /// A message has been sent.
    Msg {
        who: Option<Nick<'a>>,
        kind: MsgKind,
        target: Arg<'a>,
        msg: Line<'a>, // TODO: Formatting.
    },
    /// A CTCP message.
    ///
    /// The ACTION command is not included in this message.
    Ctcp {
        who: Option<Nick<'a>>,
        target: Arg<'a>,
        is_reply: bool,
        command: Word<'a>,
        args: Line<'a>,
    },
    UserMeta {
        target: Option<Nick<'a>>,
        meta: BTreeMap<UserMeta, UserMetaChange<()>>,
        meta_valued: BTreeMap<UserMetaKey, UserMetaChange<Bytes<'a>>>,
    },
    ChanMeta {
        target: Arg<'a>,
        meta: BTreeMap<ChanMeta, ChanMetaChange<'a, ()>>,
        meta_valued: BTreeMap<ChanMetaKey, ChanMetaChange<'a, Bytes<'a>>>,
    },
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum UserMetaChange<V> {
    Sync(V),
    Set(Option<V>),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ChanMetaChange<'a, V> {
    Sync(ChanMetaSetter, V),
    Set(Source<'a>, Option<V>),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum MsgKind {
    /// PRIVMSG.
    #[default]
    Normal,
    /// NOTICE.
    ///
    /// Ideally used for bot output.
    Notice,
    /// CTCP ACTION.
    ///
    /// Commonly used via /me.
    Action,
}

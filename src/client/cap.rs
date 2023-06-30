//! Utilities for working with capability negotiation.

use crate::{
    consts::cmd::CAP,
    error::ParseError,
    ircmsg::{Args, ClientMsg, Source},
    string::{Arg, Cmd, Key, Line, Nick, Word},
};
use std::collections::{BTreeMap, VecDeque};

use super::ClientMsgSink;

/// Requests capabilities to be enabled.
///
/// `client` and `server` are used to evaluate the maximum message length for
/// the purpose of ensuring replies will fit on a single line.
///
/// This function makes a best effort to remain within the 512 byte limit.
/// Absurd lengths may cause it to emit an over-long message.
pub fn req<'a>(
    caps: impl IntoIterator<Item = Key<'a>>,
    client: Option<Arg<'a>>,
    server: Option<&Source>,
    mut sink: impl ClientMsgSink<'static>,
) -> Result<(), std::io::Error> {
    let mut msg = ClientMsg::new_cmd(CAP);
    msg.args.add_literal("REQ");
    // " clientname :" plus one space to simplify length calcs.
    let len_mod = 4 + client.map(|c| c.len()).unwrap_or(1) as isize;
    // This should never be negative, but just in case.
    let base_len = (msg.bytes_left(server) - len_mod).try_into().unwrap_or_default();
    let mut cap_string = Vec::new();
    for cap in caps {
        if cap_string.len() + cap.len() < base_len {
            cap_string.extend_from_slice(cap.as_bytes());
            cap_string.push(b' ');
        } else {
            if !cap_string.is_empty() {
                // Remove the last space.
                cap_string.pop();
                let msg_clone = msg.clone();
                // TODO: Need a LineBuilder to avoid having to do this.
                msg.args.add_last(unsafe { Line::from_unchecked(cap_string.into()) });
                sink.send(msg)?;
                msg = msg_clone;
            }
            cap_string = cap.as_bytes().to_vec();
        }
    }
    msg.args.add_last(unsafe { Line::from_unchecked(cap_string.into()) });
    sink.send(msg)
}

/// The CAP subcommand type.
#[allow(missing_docs)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SubCmd {
    Ls,
    List,
    Ack,
    Nak,
    New,
    Del,
}

impl SubCmd {
    /// Case-sensitively matches `bytes`.
    pub const fn from_bytes(bytes: &[u8]) -> Option<SubCmd> {
        match bytes {
            b"LS" => Some(Self::Ls),
            b"LiST" => Some(Self::List),
            b"ACK" => Some(Self::Ack),
            b"NAK" => Some(Self::Nak),
            b"NEW" => Some(Self::New),
            b"DEL" => Some(Self::Del),
            _ => None,
        }
    }
    /// Returns `self` as a byte string.
    pub const fn into_arg(self) -> Arg<'static> {
        self.into_cmd().into_super()
    }
    /// Returns `self` as a byte string.
    ///
    /// There are relatively few cases when this is neccessary.
    /// Consider using [`into_arg`][SubCmd::into_arg] instead.
    pub const fn into_cmd(self) -> Cmd<'static> {
        match self {
            SubCmd::Ls => Cmd::from_str("LS"),
            SubCmd::List => Cmd::from_str("LIST"),
            SubCmd::Ack => Cmd::from_str("ACK"),
            SubCmd::Nak => Cmd::from_str("NAK"),
            SubCmd::New => Cmd::from_str("NEW"),
            SubCmd::Del => Cmd::from_str("DEL"),
        }
    }
}

/// The arguments of a server-originated CAP message.
pub struct ServerMsgArgs<'a> {
    /// The nick of the user this message was sent to.
    pub nick: Nick<'a>,
    /// This message's subcommand.
    pub subcmd: SubCmd,
    /// Whether this is the last message in a multiline reply.
    pub is_last: bool,
    /// The map of capabilities to values.
    pub caps: BTreeMap<Key<'a>, Word<'a>>,
}

impl<'a> ServerMsgArgs<'a> {
    /// Parses the argument list of a server-originated CAP message.
    pub fn parse(args: &Args<'a>) -> Result<Self, ParseError> {
        let (args, Some(last)) = args.split_last() else {
            return Err(ParseError::MissingField("caps"));
        };
        let (nick, args) = args.split_first().ok_or(ParseError::MissingField("nick"))?;
        let nick = Nick::from_super(nick.clone()).map_err(ParseError::InvalidNick)?;
        let (subcmd, args) = args.split_first().ok_or(ParseError::MissingField("subcmd"))?;
        let mut subcmd = subcmd.clone();
        // Does the spec actually mandate that this match be case-insensitive?
        subcmd.transform(crate::string::tf::AsciiCasemap::<true>);
        let subcmd = SubCmd::from_bytes(subcmd.as_bytes())
            .ok_or_else(|| ParseError::InvalidField("subcmd", subcmd.owning().into()))?;
        let is_last = if let Some((last_arg, _)) = args.split_first() {
            if last_arg == "*" {
                false
            } else {
                return Err(ParseError::InvalidField("is_last", last_arg.clone().owning().into()));
            }
        } else {
            true
        };
        let mut caps = BTreeMap::new();
        let mut last = last.clone();
        while !last.is_empty() {
            let mut word = last.transform(crate::string::tf::SplitWord);
            let (Ok(key), sep) = word.transform(crate::string::tf::SplitKey) else {
                continue;
            };
            let value = match sep {
                Some(b'=') => word,
                None => Word::default(),
                // We've hit a capability name that vinezombie can't represent.
                // Skip it.
                _ => continue,
            };
            caps.insert(key, value);
        }
        Ok(Self { nick, subcmd, is_last, caps })
    }
    /// Combines a newer [`ServerMsgArgs`] into `self`.
    /// Returns `Some(newer)` if it cannot be combined into `self`.
    ///
    /// This can be used for processing multi-line replies as a single reply.
    #[must_use]
    pub fn combine(&mut self, newer: ServerMsgArgs<'a>) -> Option<ServerMsgArgs<'a>> {
        if self.is_last || self.subcmd != newer.subcmd || self.nick != newer.nick {
            return Some(newer);
        }
        self.is_last = newer.is_last;
        for (key, value) in newer.caps {
            self.caps.insert(key, value);
        }
        None
    }
    /// Returns `true` if `self.caps` contains a capability.
    pub fn contains(&self, cap: impl AsRef<[u8]>) -> bool {
        return self.caps.get(cap.as_ref()).is_some();
    }
}

/// Parses the value of the advertised "sasl" capability
/// and retains only the elements in `auths`
/// whose names are included in `value`.
///
/// If `value` is empty, `auths` is not filtered.
pub(crate) fn filter_sasl<V>(auths: &mut VecDeque<(Arg<'static>, V)>, mut value: Word<'_>) {
    use crate::string::tf::{AsciiCasemap, Split, SplitFirst};
    use std::collections::BTreeSet;
    let mut names = BTreeSet::new();
    while !value.is_empty() {
        let mut name = value.transform(Split(|c: &u8| *c == b','));
        if !name.is_empty() {
            name.transform(AsciiCasemap::<true>);
            names.insert(name);
        }
        value.transform(SplitFirst);
    }
    if !names.is_empty() {
        auths.retain(|s| names.contains(s.0.as_bytes()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ircmsg::ServerMsg;

    #[test]
    fn ls_reply() {
        let msg = ServerMsg::parse("CAP * LS * :foo=bar").unwrap();
        let mut args1 = ServerMsgArgs::parse(&msg.args).unwrap();
        assert!(!args1.is_last);
        assert_eq!(args1.caps["foo".as_bytes()], "bar");
        assert!(args1.caps.get("bar".as_bytes()).is_none());
        let msg = ServerMsg::parse("CAP * LS :bar baz").unwrap();
        let args2 = ServerMsgArgs::parse(&msg.args).unwrap();
        assert!(args2.is_last);
        assert_eq!(args2.caps["bar".as_bytes()], "");
        assert_eq!(args2.caps["baz".as_bytes()], "");
        assert!(args1.combine(args2).is_none());
        assert_eq!(args1.caps.len(), 3);
    }
}

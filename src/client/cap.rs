//! Utilities for working with capability negotiation.

use super::ClientMsgSink;
use crate::{
    consts::cmd::CAP,
    error::ParseError,
    ircmsg::{Args, ClientMsg, Source},
    string::{Arg, Builder, Cmd, Key, Line, Nick, Splitter, Word},
};
use std::collections::BTreeMap;

type LineBuilder = Builder<Line<'static>>;

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
) {
    let mut msg = ClientMsg::new(CAP);
    msg.args.edit().add_literal("REQ");
    // " clientname :" plus one space to simplify length calcs.
    let len_mod = 4 + client.map(|c| c.len()).unwrap_or(1) as isize;
    // This should never be negative, but just in case.
    let base_len = (msg.bytes_left(server) - len_mod).try_into().unwrap_or_default();
    let mut cap_string = LineBuilder::default();
    for cap in caps {
        if cap_string.len() + cap.len() < base_len {
            if !cap_string.is_empty() {
                let _ = cap_string.try_push_char(' ');
            }
            cap_string.append(cap);
        } else {
            if !cap_string.is_empty() {
                let mut msg_clone = msg.clone();
                msg_clone.args.edit().add(cap_string.build());
                sink.send(msg_clone);
            }
            cap_string = LineBuilder::new(cap.into());
        }
    }
    msg.args.edit().add(cap_string.build());
    sink.send(msg);
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
            b"LIST" => Some(Self::List),
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
            return Err(ParseError::MissingField("caps".into()));
        };
        let (nick, args) = args.split_first().ok_or(ParseError::MissingField("nick".into()))?;
        let nick = Nick::from_super(nick.clone()).map_err(ParseError::InvalidNick)?;
        let (subcmd, args) = args.split_first().ok_or(ParseError::MissingField("subcmd".into()))?;
        let mut subcmd = subcmd.clone();
        // Does the spec actually mandate that this match be case-insensitive?
        subcmd.transform(crate::string::tf::AsciiCasemap::<true>);
        let subcmd = SubCmd::from_bytes(subcmd.as_bytes()).ok_or_else(|| {
            ParseError::InvalidField(
                "subcmd".into(),
                format!("unknown CAP subcommand: {}", subcmd.to_utf8_lossy()).into(),
            )
        })?;
        let is_last = if let Some((last_arg, _)) = args.split_first() {
            if last_arg == "*" {
                false
            } else {
                return Err(ParseError::InvalidField(
                    "is_last".into(),
                    format!("expected * as first CAP argument, got {}", last_arg.to_utf8_lossy())
                        .into(),
                ));
            }
        } else {
            true
        };
        let mut caps = BTreeMap::new();
        let mut last = Splitter::new(last.clone());
        while !last.is_empty() {
            last.consume_whitespace();
            let word = last.string_or_default::<Word>(false);
            let mut word = Splitter::new(word);
            let Ok(key) = word.string::<Key>(false) else {
                continue;
            };
            let sep = word.next_byte();
            let value = match sep {
                Some(b'=') => word.rest().unwrap(),
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

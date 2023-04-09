use super::Args;
use crate::string::{Kind, Word};

/// Representation of the source of a server message.
pub type Source<'a> = Option<Word<'a>>;

/// Error type when parsing a [`ServerMsg`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MsgParseError {
    /// Message exceeds permissible length limits.
    ///
    /// [`ServerMsg::parse`] does not return this, but an I/O step may.
    TooLong,
    /// Expected tags but none were provided.
    NoTags,
    /// Expected a source but none was provided.
    NoSource,
    /// There was no message kind in the provided message.
    NoKind,
    /// The message kind is not ASCII alphanumeric.
    InvalidKind,
}

impl std::fmt::Display for MsgParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MsgParseError::TooLong => write!(f, "invalid msg: length limits exceeded"),
            MsgParseError::NoTags => write!(f, "invalid msg: no tags after @"),
            MsgParseError::NoSource => write!(f, "invalid msg: no source after :"),
            MsgParseError::NoKind => write!(f, "invalid msg: missing kind/command"),
            MsgParseError::InvalidKind => write!(f, "invalid msg: non-alphanumeric kind/command"),
        }
    }
}

impl std::error::Error for MsgParseError {}

/// Message sent by an IRC server.
#[derive(Clone, Debug)]
pub struct ServerMsg<'a> {
    /// The sender of this message.
    pub source: Source<'a>,
    /// What kind of message this is, usually a command or numeric reply.
    pub kind: Kind<'a>,
    /// This message's arguments.
    pub args: Args<'a>,
}

impl<'a> ServerMsg<'a> {
    /// Creates a new `ServerMsg` with no source or arguments.
    pub const fn new(kind: Kind<'a>) -> Self {
        ServerMsg { source: None, kind, args: Args::new() }
    }
    /*
    /// Parses a message from a string.
    pub fn parse(msg: impl Into<IrcStr<'a>>) -> Result<ServerMsg<'a, RawData<'a>>, MsgParseError> {
        let mut msg = msg.into();
        let mut source = None;
        msg.slice(str::trim);
        if msg.lex_char(|c| *c == '@').is_some() {
            // TODO: Tags. Specifically, actually parse them.
            let _ = msg.lex_word().ok_or(MsgParseError::NoTags)?;
            msg.slice(str::trim_start);
        }
        if msg.lex_char(|c| *c == ':').is_some() {
            source = Some(msg.lex_word().ok_or(MsgParseError::NoSource)?);
            msg.slice(str::trim_start);
        }
        let kind = msg.lex_word().ok_or(MsgParseError::NoKind)?.into();
        let args = args::Args::parse(msg);
        let data = RawData { args };
        Ok(ServerMsg { source, kind, data })
    }
    */
}

impl std::fmt::Display for ServerMsg<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Tags.
        if let Some(ref src) = self.source {
            write!(f, ":{} ", src)?;
        }
        write!(f, "{}", self.kind)?;
        if !self.args.is_empty() {
            write!(f, " {}", self.args)?;
        }
        Ok(())
    }
}

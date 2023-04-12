//! Minimally-processed IRC messages.

mod args;
mod source;
pub mod tags;
//#[cfg(test)]
//mod tests;

use crate::string::Kind;

pub use self::{args::*, source::*, tags::Tags};

/// Error type when parsing a [`ServerMsg`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ParseError {
    /// Message exceeds permissible length limits.
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

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::TooLong => write!(f, "invalid msg: length limits exceeded"),
            ParseError::NoTags => write!(f, "invalid msg: no tags after @"),
            ParseError::NoSource => write!(f, "invalid msg: no source after :"),
            ParseError::NoKind => write!(f, "invalid msg: missing kind/command"),
            ParseError::InvalidKind => write!(f, "invalid msg: non-alphanumeric kind/command"),
        }
    }
}

impl std::error::Error for ParseError {}

/// An IRC message.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct IrcMsg<'a> {
    /// This message's tags, if any.
    pub tags: Tags<'a>,
    /// The sender of this message.
    pub source: Option<Source<'a>>,
    /// What kind of message this is, usually a command or numeric reply.
    pub kind: Kind<'a>,
    /// This message's arguments.
    pub args: Args<'a>,
}

impl<'a> IrcMsg<'a> {
    /// Creates a new `ServerMsg` with no source or arguments.
    pub const fn new(kind: Kind<'a>) -> Self {
        IrcMsg { tags: Tags::new(), source: None, kind, args: Args::new() }
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
    /// The number of bytes of space remaining in this message excluding tags.
    ///
    /// When calculating this, it is strongly recommended to have [`source`][Msg::source]
    /// set.
    ///
    /// If either of the returned values are negative, this message is too long
    /// to guarantee that it will be delivered in whole.
    pub fn bytes_left(&self) -> isize {
        let mut size = self.kind.len() + 2; // Newline.
        if let Some(ref src) = self.source {
            size += 2 + src.len();
        }
        for arg in self.args.all() {
            size += arg.len() + 1; // Space.
        }
        if self.args.is_last_long() {
            size += 1; // Colon.
        }
        let size: isize = size.try_into().unwrap_or(isize::MAX);
        512 - size
    }
}

impl std::fmt::Display for IrcMsg<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.tags.is_empty() {
            // Tags' Display impl includes the leading @.
            write!(f, "{} ", self.tags)?;
        }
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

//! Minimally-processed IRC messages.

mod args;
mod source;
pub mod tags;
#[cfg(test)]
mod tests;

use crate::string::{InvalidByte, Kind, Line};

pub use self::{args::*, source::*, tags::Tags};

/// Error type when parsing an [`IrcMsg`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParseError {
    /// Message exceeds permissible length limits.
    ///
    /// This will never be returned by [`IrcMsg::parse()`],
    /// but may be returned during I/O buffering.
    TooLong(usize),
    /// The source fragment of the message contains an invalid nickname.
    InvalidNick(InvalidByte),
    /// The source fragment of the message contains an invalid username.
    InvalidUser(InvalidByte),
    /// The message's kind is invalid.
    InvalidKind(InvalidByte),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::TooLong(len) => write!(f, "message is too long ({len}+ bytes)"),
            ParseError::InvalidNick(e) => write!(f, "invalid source nickname: {e}"),
            ParseError::InvalidUser(e) => write!(f, "invalid source username: {e}"),
            ParseError::InvalidKind(e) => write!(f, "invalid message kind: {e}"),
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
    /// Parses a message from a string.
    pub fn parse(msg: impl Into<Line<'a>>) -> Result<IrcMsg<'a>, ParseError> {
        use crate::string::tf::{SplitFirst, SplitWord};
        let mut msg = msg.into();
        let mut tags = Tags::new();
        let mut source = None;
        let mut expect_tags = true;
        let mut expect_source = true;
        let kind = loop {
            let mut word = msg.transform(&SplitWord);
            if word.is_empty() {
                return Err(ParseError::InvalidKind(InvalidByte::new_empty()));
            }
            match word.first() {
                Some(b'@') if expect_tags => {
                    expect_tags = false;
                    word.transform(&SplitFirst);
                    tags = Tags::parse(word);
                }
                Some(b':') if expect_source => {
                    expect_tags = false;
                    expect_source = false;
                    word.transform(&SplitFirst);
                    // Maybe not quiet failure here?
                    // Non-parsed sources can sometimes still be useful.
                    source = Some(Source::parse(word)?);
                }
                Some(_) => break Kind::from_word(word).map_err(ParseError::InvalidKind)?,
                None => return Err(ParseError::InvalidKind(InvalidByte::new_empty())),
            }
        };
        let args = Args::parse(msg);
        Ok(IrcMsg { tags, source, kind, args })
    }
    /// The number of bytes of space remaining in this message excluding tags.
    ///
    /// When calculating this, it is strongly recommended to have [`source`][IrcMsg::source]
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

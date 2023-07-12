//! Error types.

// All lovingly made without thiserror!

use std::num::NonZeroUsize;

/// Errors from parsing an IRC message..
#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// The message exceeds permissible length limits.
    TooLong,
    /// An expected field is missing.
    MissingField(&'static str),
    /// A field has an invalid value.
    InvalidField(&'static str, crate::string::Line<'static>),
    /// The string provided to a parse function is not a Line.
    InvalidLine(InvalidByte),
    /// The source fragment of the message contains an invalid nickname.
    InvalidNick(InvalidByte),
    /// The source fragment of the message contains an invalid username.
    InvalidUser(InvalidByte),
    /// The source fragment of the message contains an invalid hostname.
    InvalidHost(InvalidByte),
    /// The message's kind is invalid.
    InvalidKind(InvalidByte),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::TooLong => write!(f, "message is too long"),
            ParseError::MissingField(e) => write!(f, "missing field {e}"),
            ParseError::InvalidField(e, a) => write!(f, "invalid field {e}: got \"{a}\""),
            ParseError::InvalidLine(e) => write!(f, "invalid line: {e}"),
            ParseError::InvalidNick(e) => write!(f, "invalid source nickname: {e}"),
            ParseError::InvalidUser(e) => write!(f, "invalid source username: {e}"),
            ParseError::InvalidHost(e) => write!(f, "invalid source hostname: {e}"),
            ParseError::InvalidKind(e) => write!(f, "invalid message kind: {e}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<ParseError> for std::io::Error {
    fn from(value: ParseError) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, value)
    }
}
/// Error indicating that the invariant of a [`Bytes`][crate::string::Bytes] newtype
/// has been violated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InvalidByte(u8, Option<NonZeroUsize>);

impl InvalidByte {
    /// Creates an `InvalidByte` representing a violation of a "non-empty string" invariant.
    pub const fn new_empty() -> InvalidByte {
        InvalidByte(0u8, None)
    }
    /// Creates an `InvalidBytes` for an invalid byte in a slice.
    pub const fn new_at(bytes: &[u8], idx: usize) -> InvalidByte {
        Self::new(bytes[idx], idx)
    }
    /// Creates an `InvalidBytes` out of a byte and index.
    pub const fn new(byte: u8, idx: usize) -> InvalidByte {
        // Assume that it's impossible to ever have an array where `usize::MAX` is a valid index.
        InvalidByte(byte, Some(unsafe { NonZeroUsize::new_unchecked(idx + 1) }))
    }
    /// Returns `true` if `self` is an error representing an invalid byte at some position.
    pub fn has_index(&self) -> bool {
        self.1.is_some()
    }
    /// Returns the invalid byte, which will be `0u8` for non-empty string invariant violations.
    pub fn byte(&self) -> u8 {
        self.0
    }
    /// Returns the index at which the invalid byte was found.
    pub fn index(&self) -> Option<usize> {
        self.1.map(|v| v.get() - 1usize)
    }
}

impl std::fmt::Display for InvalidByte {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(idx) = self.index() {
            write!(f, "invalid byte {} @ index {idx}", self.0.escape_ascii())
        } else {
            write!(f, "empty byte string")
        }
    }
}

impl std::error::Error for InvalidByte {}

impl From<std::convert::Infallible> for InvalidByte {
    fn from(value: std::convert::Infallible) -> Self {
        // Forward compat idiom, also used by std.
        match value {}
    }
}

impl From<InvalidByte> for std::io::Error {
    fn from(value: InvalidByte) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, value)
    }
}

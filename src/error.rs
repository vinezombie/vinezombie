//! Error types.

// All lovingly made without thiserror!

/// Errors from parsing an IRC message..
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// The message exceeds permissible length limits.
    TooLong,
    /// An expected field is missing.
    MissingField(std::borrow::Cow<'static, str>),
    /// A field has an invalid value.
    InvalidField(std::borrow::Cow<'static, str>, Box<dyn std::error::Error + Send + Sync>),
    /// The string provided to a parse function is not a Line.
    InvalidLine(InvalidString),
    /// The source fragment of the message contains an invalid nickname.
    InvalidNick(InvalidString),
    /// The source fragment of the message contains an invalid username.
    InvalidUser(InvalidString),
    /// The source fragment of the message contains an invalid hostname.
    InvalidHost(InvalidString),
    /// The message's kind is invalid.
    InvalidKind(InvalidString),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::TooLong => write!(fmt, "message is too long"),
            ParseError::MissingField(f) => write!(fmt, "missing field {f}"),
            ParseError::InvalidField(f, e) => write!(fmt, "invalid field {f}: {e}"),
            ParseError::InvalidLine(e) => write!(fmt, "invalid line: {e}"),
            ParseError::InvalidNick(e) => write!(fmt, "invalid source nickname: {e}"),
            ParseError::InvalidUser(e) => write!(fmt, "invalid source username: {e}"),
            ParseError::InvalidHost(e) => write!(fmt, "invalid source hostname: {e}"),
            ParseError::InvalidKind(e) => write!(fmt, "invalid message kind: {e}"),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseError::InvalidField(_, e) => Some(e.as_ref()),
            ParseError::InvalidLine(ref e) => Some(e),
            ParseError::InvalidNick(ref e) => Some(e),
            ParseError::InvalidUser(ref e) => Some(e),
            ParseError::InvalidHost(ref e) => Some(e),
            ParseError::InvalidKind(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<ParseError> for std::io::Error {
    fn from(value: ParseError) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, value)
    }
}
/// Error indicating that the invariant of a [`Bytes`][crate::string::Bytes] newtype
/// has been violated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InvalidString {
    /// The string is empty.
    Empty,
    /// The string begins with a colon, which is not allowed for this type.
    Colon,
    /// The string contains an invalid byte.
    Byte(u8),
}

impl std::fmt::Display for InvalidString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvalidString::Empty => write!(f, "empty substring"),
            InvalidString::Colon => write!(f, "substring begins with colon"),
            InvalidString::Byte(b) => write!(f, "invalid byte '{}'", b.escape_ascii()),
        }
    }
}

impl std::error::Error for InvalidString {}

impl From<std::convert::Infallible> for InvalidString {
    fn from(value: std::convert::Infallible) -> Self {
        // Forward compat idiom, also used by std.
        match value {}
    }
}

impl From<InvalidString> for std::io::Error {
    fn from(value: InvalidString) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, value)
    }
}

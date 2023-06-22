use super::Numeric;
use crate::string::{Arg, Cmd};

/// Either an alphabetic command or a numeric reply.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ServerMsgKind<'a> {
    #[allow(missing_docs)]
    Numeric(Numeric),
    #[allow(missing_docs)]
    Cmd(Cmd<'a>),
}

impl<'a> From<Cmd<'a>> for ServerMsgKind<'a> {
    fn from(value: Cmd<'a>) -> Self {
        ServerMsgKind::Cmd(value)
    }
}

impl<'a> From<Numeric> for ServerMsgKind<'a> {
    fn from(value: Numeric) -> Self {
        ServerMsgKind::Numeric(value)
    }
}

impl<'a> PartialEq<str> for ServerMsgKind<'a> {
    fn eq(&self, other: &str) -> bool {
        self.as_arg() == other
    }
}

impl<'a> PartialEq<[u8]> for ServerMsgKind<'a> {
    fn eq(&self, other: &[u8]) -> bool {
        self.as_arg() == other
    }
}

impl<'a> PartialEq<&str> for ServerMsgKind<'a> {
    fn eq(&self, other: &&str) -> bool {
        self.as_arg() == *other
    }
}

impl<'a> PartialEq<&[u8]> for ServerMsgKind<'a> {
    fn eq(&self, other: &&[u8]) -> bool {
        self.as_arg() == *other
    }
}

impl<'a> PartialEq<Cmd<'_>> for ServerMsgKind<'a> {
    fn eq(&self, other: &Cmd<'_>) -> bool {
        if let Self::Cmd(cmd) = self {
            cmd == other
        } else {
            false
        }
    }
}

impl<'a> PartialEq<Numeric> for ServerMsgKind<'a> {
    fn eq(&self, other: &Numeric) -> bool {
        if let Self::Numeric(num) = self {
            num == other
        } else {
            false
        }
    }
}

#[allow(clippy::len_without_is_empty)]
impl<'a> ServerMsgKind<'a> {
    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> ServerMsgKind<'static> {
        match self {
            ServerMsgKind::Numeric(num) => ServerMsgKind::Numeric(num),
            ServerMsgKind::Cmd(c) => ServerMsgKind::Cmd(c.owning()),
        }
    }
    /// Returns `self`'s value as an [`Arg`].
    pub const fn as_arg(&self) -> Arg<'_> {
        use crate::string::Bytes;
        unsafe { Arg::from_unchecked(Bytes::from_str(self.as_str())) }
    }
    /// Returns a reference to `self`'s value as a [`str`].
    pub const fn as_str(&self) -> &str {
        match self {
            ServerMsgKind::Numeric(num) => num.as_str(),
            ServerMsgKind::Cmd(c) => c.as_str(),
        }
    }
    /// The length of the server message kind, in bytes.
    ///
    /// This value is guaranteed to be non-zero.
    pub const fn len(&self) -> usize {
        match self {
            ServerMsgKind::Numeric(_) => 3,
            ServerMsgKind::Cmd(c) => c.len(),
        }
    }
    /// Returns `Some(true)` if `self` represents an error,
    /// `Some(false)` if it does not, or `None` if it's unknown.
    pub const fn is_error(&self) -> Option<bool> {
        match self {
            ServerMsgKind::Numeric(n) => n.is_error(),
            ServerMsgKind::Cmd(c) => match c.as_str().as_bytes() {
                b"FAIL" => Some(true),
                b"ERROR" => Some(true),
                _ => Some(false),
            },
        }
    }
}

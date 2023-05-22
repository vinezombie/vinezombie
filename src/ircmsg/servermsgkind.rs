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
    /// Returns a textual representation of `self`'s value.
    pub fn as_arg(&self) -> Arg<'_> {
        match self {
            ServerMsgKind::Numeric(num) => unsafe { Arg::from_unchecked(num.as_str().into()) },
            ServerMsgKind::Cmd(c) => unsafe { Arg::from_unchecked(c.as_ref().into()) },
        }
    }
    /// The length of the server message kind, in bytes.
    ///
    /// This value is guaranteed to be non-zero.
    pub fn len(&self) -> usize {
        match self {
            ServerMsgKind::Numeric(_) => 3,
            ServerMsgKind::Cmd(c) => c.len(),
        }
    }
}

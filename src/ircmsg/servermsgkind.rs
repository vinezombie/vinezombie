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

impl<'a> ServerMsgKind<'a> {
    /// Returns a textual representation of `self`'s value.
    pub fn as_arg<'b>(&'b self) -> Arg<'b> {
        match self {
            ServerMsgKind::Numeric(num) => unsafe { Arg::from_unchecked(num.as_str().into()) },
            ServerMsgKind::Cmd(c) => unsafe { Arg::from_unchecked(c.as_ref().into()) },
        }
    }
    /// The length of the server message kind, in bytes.
    pub fn len(&self) -> usize {
        match self {
            ServerMsgKind::Numeric(_) => 3,
            ServerMsgKind::Cmd(c) => c.len(),
        }
    }
}

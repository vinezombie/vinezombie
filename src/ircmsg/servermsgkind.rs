use super::Numeric;
use crate::string::{Arg, Cmd};

/// Either an alphabetic command or a numeric reply.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum ServerMsgKindRaw<'a> {
    #[allow(missing_docs)]
    Numeric(Numeric),
    #[allow(missing_docs)]
    Cmd(Cmd<'a>),
}

impl<'a> std::borrow::Borrow<[u8]> for ServerMsgKindRaw<'a> {
    fn borrow(&self) -> &[u8] {
        match self {
            ServerMsgKindRaw::Numeric(n) => n.as_ref(),
            ServerMsgKindRaw::Cmd(c) => c,
        }
    }
}

impl<'a> std::hash::Hash for ServerMsgKindRaw<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ServerMsgKindRaw::Numeric(n) => n.hash(state),
            ServerMsgKindRaw::Cmd(c) => c.hash(state),
        }
    }
}

impl<'a> From<Cmd<'a>> for ServerMsgKindRaw<'a> {
    fn from(value: Cmd<'a>) -> Self {
        ServerMsgKindRaw::Cmd(value)
    }
}

impl<'a> From<Numeric> for ServerMsgKindRaw<'a> {
    fn from(value: Numeric) -> Self {
        ServerMsgKindRaw::Numeric(value)
    }
}

impl<'a> PartialEq<str> for ServerMsgKindRaw<'a> {
    fn eq(&self, other: &str) -> bool {
        self.as_arg() == other
    }
}

impl<'a> PartialEq<[u8]> for ServerMsgKindRaw<'a> {
    fn eq(&self, other: &[u8]) -> bool {
        self.as_arg() == other
    }
}

impl<'a> PartialEq<&str> for ServerMsgKindRaw<'a> {
    fn eq(&self, other: &&str) -> bool {
        self.as_arg() == *other
    }
}

impl<'a> PartialEq<&[u8]> for ServerMsgKindRaw<'a> {
    fn eq(&self, other: &&[u8]) -> bool {
        self.as_arg() == *other
    }
}

impl<'a> PartialEq<Cmd<'_>> for ServerMsgKindRaw<'a> {
    fn eq(&self, other: &Cmd<'_>) -> bool {
        if let Self::Cmd(cmd) = self {
            cmd == other
        } else {
            false
        }
    }
}

impl<'a> PartialEq<Numeric> for ServerMsgKindRaw<'a> {
    fn eq(&self, other: &Numeric) -> bool {
        if let Self::Numeric(num) = self {
            num == other
        } else {
            false
        }
    }
}

#[allow(clippy::len_without_is_empty)]
impl<'a> ServerMsgKindRaw<'a> {
    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> ServerMsgKindRaw<'static> {
        match self {
            ServerMsgKindRaw::Numeric(num) => ServerMsgKindRaw::Numeric(num),
            ServerMsgKindRaw::Cmd(c) => ServerMsgKindRaw::Cmd(c.owning()),
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
            ServerMsgKindRaw::Numeric(num) => num.as_str(),
            ServerMsgKindRaw::Cmd(c) => c.as_str(),
        }
    }
    /// The length of the server message kind, in bytes.
    ///
    /// This value is guaranteed to be non-zero.
    pub const fn len(&self) -> usize {
        match self {
            ServerMsgKindRaw::Numeric(_) => 3,
            ServerMsgKindRaw::Cmd(c) => c.len(),
        }
    }
    /// Returns `Some(true)` if `self` represents an error,
    /// `Some(false)` if it does not, or `None` if it's unknown.
    pub const fn is_error(&self) -> Option<bool> {
        match self {
            ServerMsgKindRaw::Numeric(n) => n.is_error(),
            ServerMsgKindRaw::Cmd(c) => match c.as_str().as_bytes() {
                b"FAIL" => Some(true),
                b"ERROR" => Some(true),
                _ => Some(false),
            },
        }
    }
}

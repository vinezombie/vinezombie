use crate::string::{Nick, User, Word};
use std::io::Write;

/// The sender of a message, also known as a message's "prefix".
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Source<'a> {
    /// The name of the source, usually a nickname but also sometimes a server name.
    pub nick: Nick<'a>,
    /// The address of the sender, if the sender is NOT a server.
    pub address: Option<Address<'a>>,
}

/// The `username@hostname` fragment of a [`Source`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Address<'a> {
    /// The hostname (or vhost) of the sender.
    pub host: Word<'a>,
    /// The username of the sender.
    pub user: Option<User<'a>>,
}

#[allow(clippy::len_without_is_empty)]
impl Source<'_> {
    /// Returns the length of `self`'s textual representaiton in bytes.
    pub fn len(&self) -> usize {
        if let Some(address) = self.address.as_ref() {
            self.nick.len() + 1 + address.len()
        } else {
            self.nick.len()
        }
    }
    /// Writes `self` to the provided [`Write`].
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, w: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        if let Some(address) = self.address.as_ref() {
            w.write_all(self.nick.as_ref())?;
            if let Some(user) = &address.user {
                w.write_all(b"!")?;
                w.write_all(user.as_ref())?;
            }
            w.write_all(b"@")?;
            w.write_all(address.host.as_ref())
        } else {
            w.write_all(self.nick.as_ref())
        }
    }
}

#[allow(clippy::len_without_is_empty)]
impl Address<'_> {
    /// Returns `false` if [`user`][Address:user] exists and starts with a tilde.
    ///
    /// Many IRC networks use a leading `~` to indicate a lack of ident response.
    pub fn has_ident(&self) -> bool {
        !matches!(self.user.as_ref().and_then(|user| user.first()), Some(b'~'))
    }
    /// Returns the length of `self`'s textual representaiton in bytes.
    pub fn len(&self) -> usize {
        if let Some(user) = self.user.as_ref() {
            user.len() + 1 + self.host.len()
        } else {
            self.host.len()
        }
    }
    /// Writes `self` to the provided [`Write`].
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, w: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        if let Some(user) = self.user.as_ref() {
            w.write_all(user.as_ref())?;
            w.write_all(b"@")?;
        }
        w.write_all(self.host.as_ref())
    }
}

impl std::fmt::Display for Source<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let nick = &self.nick;
        if let Some(address) = self.address.as_ref() {
            let host = &address.host;
            if let Some(user) = &address.user {
                write!(f, "{nick}!{user}@{host}")
            } else {
                write!(f, "{nick}@{host}")
            }
        } else {
            write!(f, "{nick}")
        }
    }
}

impl std::fmt::Display for Address<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let host = &self.host;
        if let Some(user) = self.user.as_ref() {
            write!(f, "{user}@{host}")
        } else {
            write!(f, "{host}")
        }
    }
}

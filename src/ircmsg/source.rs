//! Message sources, also used by some parts of IRC state
// such as ban setters.

use crate::{
    error::ParseError,
    string::{
        tf::{Split, SplitFirst},
        Host, Nick, User, Word,
    },
};
use std::io::Write;

/// The sender of a message, also known as a message's "prefix".
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Source<'a> {
    /// The name of the source, usually a nickname but also sometimes a server name.
    pub nick: Nick<'a>,
    /// The user@host of the sender, if the sender is NOT a server.
    pub userhost: Option<UserHost<'a>>,
}

/// The `username@hostname` fragment of a [`Source`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UserHost<'a> {
    /// The hostname (or vhost) of the sender.
    pub host: Host<'a>,
    /// The username of the sender.
    pub user: Option<User<'a>>,
}

#[allow(clippy::len_without_is_empty)]
impl<'a> Source<'a> {
    /// Creates a new source representing a server.
    pub const fn new_server(server_name: Nick<'a>) -> Self {
        Source { nick: server_name, userhost: None }
    }
    /// Creates a new source representing a user.
    pub const fn new_user(nick: Nick<'a>, user: User<'a>, host: Host<'a>) -> Self {
        Source { nick, userhost: Some(UserHost { user: Some(user), host }) }
    }
    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> Source<'static> {
        Source { nick: self.nick.owning(), userhost: self.userhost.map(UserHost::owning) }
    }
    /// Returns the length of `self`'s textual representaiton in bytes.
    pub fn len(&self) -> usize {
        if let Some(address) = self.userhost.as_ref() {
            self.nick.len() + 1 + address.len()
        } else {
            self.nick.len()
        }
    }
    /// Writes `self` to the provided [`Write`].
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, w: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        if let Some(address) = self.userhost.as_ref() {
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
    /// Parses the provided source string.
    ///
    /// The provided word should NOT contain the leading ':'.
    ///
    /// # Errors
    /// This function can return only either
    /// [`InvalidNick`][crate::error::ParseError::InvalidNick] or
    /// [`InvalidUser`][crate::error::ParseError::InvalidUser].
    pub fn parse(word: impl Into<Word<'a>>) -> Result<Self, ParseError> {
        let mut word = word.into();
        let nick = word.transform(Split(Nick::is_invalid));
        // TODO: We know things that make the full from_bytes check here redundant,
        // but we still need to check Args's conditions for Host (non-empty, no leading colon).
        let nick = Nick::from_bytes(nick).map_err(ParseError::InvalidNick)?;
        let user = match word.transform(SplitFirst) {
            Some(b'!') => {
                let user = word.transform(Split(|b: &u8| *b == b'@'));
                word.transform(SplitFirst);
                user
            }
            Some(b'@') => {
                let host = word.try_into().map_err(ParseError::InvalidHost)?;
                let address = UserHost { user: None, host };
                return Ok(Source { nick, userhost: Some(address) });
            }
            _ => return Ok(Source { nick, userhost: None }),
        };
        let user = User::from_bytes(user).map_err(ParseError::InvalidUser)?;
        let host = word.try_into().map_err(ParseError::InvalidHost)?;
        Ok(Source { nick, userhost: Some(UserHost { user: Some(user), host }) })
    }
}

#[allow(clippy::len_without_is_empty)]
impl UserHost<'_> {
    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> UserHost<'static> {
        UserHost { host: self.host.owning(), user: self.user.map(User::owning) }
    }
    /// Returns `false` if `self.user` is `Some` and starts with a tilde.
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
        if let Some(address) = self.userhost.as_ref() {
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

impl std::fmt::Display for UserHost<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let host = &self.host;
        if let Some(user) = self.user.as_ref() {
            write!(f, "{user}@{host}")
        } else {
            write!(f, "{host}")
        }
    }
}

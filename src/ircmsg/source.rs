use crate::{
    error::ParseError,
    string::{Builder, Nick, Splitter, User, Word},
};
use std::{io::Write, num::NonZeroUsize};

/// The sender of a message, also known as a message's "prefix".
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
pub struct Source<'a> {
    /// The name of the source, usually a nickname but also sometimes a server name.
    pub nick: Nick<'a>,
    /// The user@host of the sender, if the sender is NOT a server.
    pub userhost: Option<UserHost<'a>>,
}

/// The `username@hostname` fragment of a [`Source`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
pub struct UserHost<'a> {
    /// The username of the sender.
    pub user: Option<User<'a>>,
    /// The hostname (or vhost) of the sender.
    ///
    /// Because vhosts can theoretically include any word-legal character,
    /// this has to be a `Word` despite often being a `Host` in practice.
    pub host: Word<'a>,
}

#[allow(clippy::len_without_is_empty)]
impl<'a> Source<'a> {
    /// Creates a new source representing a server.
    pub const fn new_server(server_name: Nick<'a>) -> Self {
        Source { nick: server_name, userhost: None }
    }
    /// Creates a new source representing a user.
    pub const fn new_user(nick: Nick<'a>, user: User<'a>, host: Word<'a>) -> Self {
        Source { nick, userhost: Some(UserHost { user: Some(user), host }) }
    }
    /// Returns a reference to the username of the sender, if any.
    pub const fn user(&self) -> Option<&User<'a>> {
        if let Some(uh) = &self.userhost {
            if let Some(u) = &uh.user {
                return Some(u);
            }
        }
        None
    }
    /// Returns a reference to the hostname of the sender, if any.
    pub const fn host(&self) -> Option<&Word<'a>> {
        if let Some(uh) = &self.userhost {
            Some(&uh.host)
        } else {
            None
        }
    }
    /// Returns the length of `self`'s textual representaiton in bytes.
    pub fn len(&self) -> usize {
        let mut len = self.nick.len();
        if let Some(address) = self.userhost.as_ref() {
            len = len
                .saturating_add(1) // '@'
                .saturating_add(address.len())
        };
        len
    }
    /// As [`len`][Self::len] but returns a [`NonZeroUsize`].
    pub fn len_nonzero(&self) -> NonZeroUsize {
        unsafe { NonZeroUsize::new_unchecked(self.len()) }
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
        let mut word = Splitter::new(word.into());
        let nick = word.string::<Nick>(false).map_err(ParseError::InvalidNick)?;
        match word.next_byte() {
            Some(b'!') => {
                let user = word
                    .save_end()
                    .until_byte_eq(b'@')
                    .string::<User>(true)
                    .map_err(ParseError::InvalidUser)?;
                word.next_byte();
                let host: Word = word.rest_or_default();
                if host.is_empty() {
                    Err(ParseError::InvalidHost(crate::error::InvalidString::Empty))
                } else {
                    Ok(Source { nick, userhost: Some(UserHost { user: Some(user), host }) })
                }
            }
            Some(b'@') => {
                let host: Word = word.rest_or_default();
                if host.is_empty() {
                    Err(ParseError::InvalidHost(crate::error::InvalidString::Empty))
                } else {
                    let address = UserHost { user: None, host };
                    Ok(Source { nick, userhost: Some(address) })
                }
            }
            _ => Ok(Source { nick, userhost: None }),
        }
    }
    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> Source<'static> {
        Source { nick: self.nick.owning(), userhost: self.userhost.map(UserHost::owning) }
    }
    /// Merges the values of `self` into one buffer,
    /// then creates a new `Source` from the shared buffer.
    ///
    /// Parsed `Source`s are usually constructed out of single buffers containing the rest of
    /// a message. If you want to keep `self` around after the lifetime of the message,
    /// this can be used to retain the minimum amount of memory necessary for the source.
    pub fn owning_merged(self) -> Source<'static> {
        let len_nick = self.nick.len();
        let mut len_user = None;
        let mut len_host = None;
        let mut concat = Builder::<Word>::new(self.nick.clone().into());
        if let Some(uh) = self.userhost {
            len_host = Some(uh.host.len());
            if let Some(u) = uh.user {
                len_user = Some(u.len());
                concat.append(u);
            }
            concat.append(uh.host);
        }
        let mut splitter = Splitter::new(concat.build());
        let nick = splitter.save_end().until_count(len_nick).string(true).unwrap();
        if let Some(lh) = len_host {
            let user = len_user.map(|lu| {
                // This really shouldn't fail.
                splitter.save_end().until_count(lu).string(true).unwrap()
            });
            let host = splitter.save_end().until_count(lh).string(true).unwrap();
            let userhost = UserHost { user, host };
            Source { nick, userhost: Some(userhost) }
        } else {
            Source { nick, userhost: None }
        }
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

/// An atomic reference-counted [`Source`].
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct SharedSource<'a>(std::sync::Arc<Source<'a>>);

impl<'a> SharedSource<'a> {
    /// Wraps the provided source in a [`SharedSource`].
    pub fn new(source: Source<'a>) -> Self {
        Self(std::sync::Arc::new(source))
    }
    /// As [`Source::owning`].
    pub fn owning(self) -> Source<'static> {
        // TODO: Check if everything in Source is owning,
        // and if so just return self.
        match std::sync::Arc::try_unwrap(self.0) {
            Ok(src) => src.owning(),
            Err(arc) => (*arc).clone().owning(),
        }
    }
    /// As [`Source::owning_merged`].
    pub fn owning_merged(self) -> Source<'static> {
        match std::sync::Arc::try_unwrap(self.0) {
            Ok(src) => src.owning_merged(),
            Err(arc) => (*arc).clone().owning_merged(),
        }
    }
}

impl<'a> std::ops::Deref for SharedSource<'a> {
    type Target = Source<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for SharedSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

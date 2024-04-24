use super::*;

impl Line<'static> {
    /// Returns the realname of the local user running this program.
    ///
    /// This function returns `None` if either the `whoami` feature is not enabled
    /// or if the realname is not a valid `Line`.
    pub fn new_realname() -> Option<Self> {
        #[cfg(feature = "whoami")]
        if let Ok(val) = Line::from_bytes(whoami::realname()) {
            return Some(val);
        }
        None
    }
}

impl Key<'_> {
    /// Returns `true` if this string could be a client tag.
    pub fn is_client_tag(&self) -> bool {
        // SAFE: TagKey is non-empty.
        unsafe { *self.0.get_unchecked(0) == b'+' }
    }
}

impl User<'static> {
    /// Returns the username of the local user running this program.
    ///
    /// This function returns `None` if either the `whoami` feature is not enabled
    /// or if the username is not a valid `User`.
    pub fn new_username() -> Option<Self> {
        #[cfg(feature = "whoami")]
        if let Ok(val) = User::from_bytes(whoami::username()) {
            return Some(val);
        }
        None
    }

    /// Creates a new [`User`] from a `u32` ID.
    pub fn from_id(id: u32) -> Self {
        let retval = format!("i{id:08x}");
        User::from_bytes(retval).unwrap()
    }

    /// Creates a new [`User`] from a `u16` ID.
    pub fn from_id_short(id: u16) -> Self {
        let retval = format!("i{id:05}");
        User::from_bytes(retval).unwrap()
    }
}

impl<'a> Cmd<'a> {
    /// Tries to convert `word` into an instance of this type, uppercasing where necessary.
    pub fn from_word(word: impl Into<Word<'a>>) -> Result<Self, InvalidString> {
        let mut word = word.into();
        if let Some(inval) = word.iter().find(|b| !b.is_ascii_alphabetic()) {
            return Err(InvalidString::Byte(*inval));
        };
        word.transform(AsciiCasemap::<true>);
        Ok(unsafe { Cmd::from_unchecked(word.into()) })
    }
    /// Returns a reference to `self`'s value as a `str`.
    pub const fn as_str(&self) -> &str {
        // Safety: This should only contain ASCII characters.
        unsafe { std::str::from_utf8_unchecked(self.0.as_bytes()) }
    }
}

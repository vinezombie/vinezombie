use super::*;

impl<'a> Default for NoNul<'a> {
    fn default() -> Self {
        NoNul(Bytes::default())
    }
}

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

impl<'a> Default for Line<'a> {
    fn default() -> Self {
        Line(Bytes::default())
    }
}

impl<'a> Default for Word<'a> {
    fn default() -> Self {
        Word(Bytes::default())
    }
}

impl<'a> Host<'a> {
    /// Returns a reference to `self`'s value as a `str`.
    pub const fn as_str(&self) -> &str {
        // Safety: This should only contain ASCII characters.
        unsafe { std::str::from_utf8_unchecked(self.0.as_bytes()) }
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
}

impl<'a> Cmd<'a> {
    /// Tries to convert `word` into an instance of this type, uppercasing where necessary.
    pub fn from_word(word: impl Into<Word<'a>>) -> Result<Self, InvalidByte> {
        let mut word = word.into();
        if let Some(idx) = word.iter().position(|b| !b.is_ascii_alphabetic()) {
            return Err(InvalidByte::new_at(word.as_ref(), idx));
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

#![allow(clippy::derivable_impls)]

use crate::string::tf::AsciiCasemap;

use super::{Bytes, Transform};
use std::{borrow::Borrow, num::NonZeroUsize};

/// # Safety
/// Here be transmutes. $ssuper must be either Bytes
/// or a an $sname from a previous use of this macro.
macro_rules! impl_subtype {
    (
        $doc:literal
        $sname:ident: $ssuper:ident
        $tname:ident: $tsuper:ident
        |$targ:ident| $tbody:block
        |$uarg:ident| $ubody:block
    ) => {
        impl_subtype! {
            $doc
            $sname: $ssuper
            $tname: $tsuper
            |$targ| $tbody
        }
        impl<'a> $sname<'a> {
            /// Tries to convert `sup` into an instance of this type.
            /// Errors if `sup` does not uphold this type's guarantees.
            pub fn from_super(sup: impl Into<$ssuper<'a>>) -> Result<Self, InvalidByte> {
                let sup = sup.into();
                #[inline]
                fn check($uarg: &[u8]) -> Option<InvalidByte> {
                    $ubody
                }
                if let Some(e) = check(sup.as_ref()) {
                    Err(e)
                } else {
                    Ok(unsafe { std::mem::transmute(sup) })
                }
            }
            /// Cheaply converts `self` into the next more-general type in the string hierarchy.
            pub const fn into_super(self) -> $ssuper<'a> {
                // Can't use `self.0` for non-const destructor reasons.
                unsafe { std::mem::transmute(self) }
            }
        }
    };
    (
        $doc:literal
        $sname: ident: $ssuper:ident
        $tname:ident: $tsuper:ident
        |$targ:ident| $tbody:block
    ) => {
        #[doc = $doc]
        #[repr(transparent)]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub struct $sname<'a>(Bytes<'a>);

        #[doc = concat!("Marker for [`", stringify!($sname), "`]-safe [`Transform`]s.")]
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = "[`Transform::transform()`]' must return a byte string that maintains"]
        #[doc = concat!("[`",stringify!($sname),"`]'s invariants.")]
        #[doc = "See its struct-level documentation for more info."]
        pub unsafe trait $tname: $tsuper {}

        impl<'a> $sname<'a> {
            /// Returns the first byte and its index that violate this type's guarantees.
            #[inline]
            pub const fn find_invalid(bytes: &[u8]) -> Option<InvalidByte> {
                // TODO: It seems like this could be made const for some cases.
                // Optimization: the block here can also do a test for ASCII-validity
                // and use that to infer UTF-8 validity.
                let $targ = bytes;
                $tbody
            }
            /// Tries to convert `bytes` into an instance of this type.
            /// Errors if `bytes` does not uphold this type's guarantees.
            pub fn from_bytes(bytes: impl Into<Bytes<'a>>) -> Result<Self, InvalidByte> {
                let bytes = bytes.into();
                if let Some(e) = Self::find_invalid(bytes.as_ref()) {
                    Err(e)
                } else {
                    Ok($sname(bytes))
                }
            }
            /// Tries to convert the provided [`str`] into an instance of this type.
            /// Panics if `string` does not uphold this type's gurarantees.
            pub const fn from_str(string: &'a str) -> Self {
                if Self::find_invalid(string.as_bytes()).is_some() {
                    // Can't emit the error here because of the const context.
                    panic!("invalid string")
                } else {
                    unsafe { Self::from_unchecked(Bytes::from_str(string)) }
                }
            }
            /// Performs an unchecked conversion from `bytes`.
            ///
            /// # Safety
            /// This function assumes that this type's guarantees are upheld by `bytes`.
            pub const unsafe fn from_unchecked(bytes: Bytes<'a>) -> Self {
                $sname(bytes)
            }
            /// Transforms `self` using the provided [`Transform`]
            /// that upholds `self`'s invariant.
            pub fn transform<T: $tname>(&mut self, tf: T) -> T::Value<'a> {
                self.0.transform(tf)
            }
            /// Cheaply converts `self` into the underlying byte string.
            pub const fn into_bytes(self) -> Bytes<'a> {
                // Can't use `self.0` for non-const destructor reasons.
                unsafe { std::mem::transmute(self) }
            }
            /// Returns an owning version of this string.
            ///
            /// If this string already owns its data, this method only extends its lifetime.
            pub fn owning(self) -> $sname<'static> {
                $sname(self.0.owning())
            }
        }
        impl<'a> From<$sname<'a>> for Bytes<'a> {
            fn from(value: $sname<'a>) -> Bytes<'a> {
                value.into_bytes()
            }
        }
        impl<'a> TryFrom<Bytes<'a>> for $sname<'a> {
            type Error = InvalidByte;
            fn try_from(value: Bytes<'a>) -> Result<$sname<'a>, InvalidByte> {
                $sname::from_bytes(value)
            }
        }
        impl AsRef<[u8]> for $sname<'_> {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }
        impl Borrow<[u8]> for $sname<'_> {
            fn borrow(&self) -> &[u8] {
                self.0.borrow()
            }
        }
        impl<'a> std::ops::Deref for $sname<'a> {
            type Target = $ssuper<'a>;

            fn deref(&self) -> &Self::Target {
                unsafe { &*(self as *const Self as *const Self::Target) }
            }
        }
        impl std::fmt::Display for $sname<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
        impl<const N: usize> PartialEq<[u8; N]> for $sname<'_> {
            fn eq(&self, other: &[u8; N]) -> bool {
                self.0 == *other
            }
        }
        impl<const N: usize> PartialEq<&[u8; N]> for $sname<'_> {
            fn eq(&self, other: &&[u8; N]) -> bool {
                self.0 == *other
            }
        }
        impl PartialEq<[u8]> for $sname<'_> {
            fn eq(&self, other: &[u8]) -> bool {
                self.0 == *other
            }
        }
        impl PartialEq<&[u8]> for $sname<'_> {
            fn eq(&self, other: &&[u8]) -> bool {
                self.0 == *other
            }
        }
        impl PartialEq<str> for $sname<'_> {
            fn eq(&self, other: &str) -> bool {
                self.0 == *other
            }
        }
        impl PartialEq<&str> for $sname<'_> {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }
        impl<'a> TryFrom<&'a [u8]> for $sname<'a> {
            type Error = InvalidByte;
            fn try_from(value: &'a [u8]) -> Result<$sname<'a>, Self::Error> {
                Bytes::from(value).try_into()
            }
        }
        impl<'a> TryFrom<&'a str> for $sname<'a> {
            type Error = InvalidByte;
            fn try_from(value: &'a str) -> Result<$sname<'a>, Self::Error> {
                Bytes::from(value).try_into()
            }
        }
    };
}

macro_rules! conversions {
    ($sname: ident: $ssuper: ident) => {
        // TODO: Downcasting TryFrom impls?
        impl<'a> From<$sname<'a>> for $ssuper<'a> {
            fn from(value: $sname<'a>) -> $ssuper<'a> {
                unsafe { std::mem::transmute(value) }
            }
        }
        impl<'a> TryFrom<$ssuper<'a>> for $sname<'a> {
            type Error = InvalidByte;
            fn try_from(value: $ssuper<'a>) -> Result<$sname<'a>, InvalidByte> {
                $sname::from_bytes(value.into_bytes())
            }
        }
    };
}

macro_rules! check_bytes {
    ($bytes:ident, $f:expr) => {{
        let mut i = 0usize;
        while i < $bytes.len() {
            if $f(&$bytes[i]) {
                return Some(InvalidByte::new_at($bytes, i))
            }
            i += 1;
        }
        None
    }}
}

#[inline]
pub(crate) const fn is_invalid_for_line(byte: &u8) -> bool {
    matches!(*byte, b'\0' | b'\r' | b'\n')
}

impl_subtype! {
    "A [`Bytes`] that does not contain NUL, CR, or LF."
    Line: Bytes
    LineSafe: Transform
    |bytes| {
        check_bytes!(bytes, is_invalid_for_line)
    }
}

impl<'a> Default for Line<'a> {
    fn default() -> Self {
        Line(Bytes::default())
    }
}

#[inline]
pub(crate) const fn is_invalid_for_word<const CHAIN: bool>(byte: &u8) -> bool {
    *byte == b' ' || if CHAIN { is_invalid_for_line(byte) } else { false }
}

impl_subtype! {
    "A [`Line`] that does not contain ASCII spaces."
    Word: Line
    WordSafe: LineSafe
    |bytes| {
        check_bytes!(bytes, is_invalid_for_word::<true>)
    }
    |bytes| {
        check_bytes!(bytes, is_invalid_for_word::<false>)
    }
}
conversions!(Word: Line);

impl<'a> Default for Word<'a> {
    fn default() -> Self {
        Word(Bytes::default())
    }
}

#[inline]
const fn arg_first_check(bytes: &[u8]) -> Option<InvalidByte> {
    match bytes.first() {
        None => Some(InvalidByte::new_empty()),
        Some(b':') => Some(InvalidByte::new_at(bytes, 0)),
        _ => None,
    }
}

impl_subtype! {
    "A non-empty [`Word`] that does not begin with `:`."
    Arg: Word
    ArgSafe: WordSafe
    |bytes| {
        if let Some(e) = arg_first_check(bytes) {
            Some(e)
        } else {
            check_bytes!(bytes, is_invalid_for_word::<true>)
        }
    }
    |bytes| {
        arg_first_check(bytes)
    }
}
conversions!(Arg: Line);
conversions!(Arg: Word);

#[inline]
pub(crate) const fn is_invalid_for_tagkey<const CHAIN: bool>(byte: &u8) -> bool {
    matches!(*byte, b'=' | b';') || if CHAIN { is_invalid_for_word::<true>(byte) } else { false }
}

impl_subtype! {
    "A non-empty [`Word`] that does not contain `=` or `;`."
    Key: Word
    KeySafe: WordSafe
    |bytes| {
        if bytes.is_empty() {
            Some(InvalidByte::new_empty())
        } else {
            check_bytes!(bytes, is_invalid_for_tagkey::<true>)
        }
    }
    |bytes| {
        if bytes.is_empty() {
            Some(InvalidByte::new_empty())
        } else {
            check_bytes!(bytes, is_invalid_for_tagkey::<false>)
        }
    }
}
conversions!(Key: Line);
conversions!(Key: Word);

impl Key<'_> {
    /// Returns `true` if this is a client tag.
    pub fn is_client_tag(&self) -> bool {
        // SAFE: TagKey is non-empty.
        unsafe { *self.0.get_unchecked(0) == b'+' }
    }
}

#[inline]
pub(crate) const fn is_invalid_for_nick<const CHAIN: bool>(byte: &u8) -> bool {
    matches!(*byte, b'!' | b'@') || if CHAIN { is_invalid_for_word::<true>(byte) } else { false }
}

#[inline]
pub(crate) const fn is_invalid_for_user<const CHAIN: bool>(byte: &u8) -> bool {
    matches!(*byte, b'@' | b'%') || if CHAIN { is_invalid_for_word::<true>(byte) } else { false }
}

impl_subtype! {
    "An [`Arg`] that does not contain `!` or `@`.\nIntended for use with nicknames."
    Nick: Arg
    NickSafe: ArgSafe
    |bytes| {
        if let Some(e) = arg_first_check(bytes) {
            Some(e)
        } else {
            check_bytes!(bytes, is_invalid_for_nick::<true>)
        }
    }
    |bytes| {
        check_bytes!(bytes, is_invalid_for_nick::<false>)
    }
}
conversions!(Nick: Line);
conversions!(Nick: Word);
conversions!(Nick: Arg);

impl_subtype! {
    "An [`Arg`] that does not contain `@` or `%`.\nIntended for use with usernames."
    User: Arg
    UserSafe: ArgSafe
    |bytes| {
        if let Some(e) = arg_first_check(bytes) {
            Some(e)
        } else {
            check_bytes!(bytes, is_invalid_for_user::<true>)
        }
    }
    |bytes| {
        check_bytes!(bytes, is_invalid_for_user::<false>)
    }
}
conversions!(User: Line);
conversions!(User: Word);
conversions!(User: Arg);

#[inline]
const fn cmd_byte_check(byte: &u8) -> bool {
    !byte.is_ascii_uppercase()
}

impl_subtype! {
    "An [`Arg`] that only contains ASCII uppercase characters."
    Cmd: Arg
    CmdSafe: ArgSafe
    |bytes| {
        if bytes.is_empty() {
            Some(InvalidByte::new_empty())
        } else {
            check_bytes!(bytes, cmd_byte_check)
        }
    }
    |bytes| {
        check_bytes!(bytes, cmd_byte_check)
    }
}
conversions!(Cmd: Line);
conversions!(Cmd: Word);
conversions!(Cmd: Arg);

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
}

/// Error indicating that the invariant of a [`Bytes`] newtype has been violated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InvalidByte(u8, Option<NonZeroUsize>);

impl InvalidByte {
    /// Creates an `InvalidByte` representing a violation of a "non-empty string" invariant.
    pub const fn new_empty() -> InvalidByte {
        InvalidByte(0u8, None)
    }
    /// Creates an `InvalidBytes` for an invalid bytes.
    pub const fn new_at(bytes: &[u8], idx: usize) -> InvalidByte {
        // Assuming that it's impossible to ever have an array where `usize::MAX` is a valid index.
        InvalidByte(bytes[idx], Some(unsafe { NonZeroUsize::new_unchecked(idx + 1) }))
    }
    /// Returns the invalid byte, which will be `0u8` for non-empty string invariant violations.
    pub fn byte(&self) -> u8 {
        self.0
    }
    /// Returns the index at which the invalid byte was found.
    pub fn index(&self) -> Option<usize> {
        self.1.map(|v| v.get() - 1usize)
    }
}

impl std::fmt::Display for InvalidByte {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(idx) = self.index() {
            write!(f, "invalid byte {} @ index {idx}", self.0.escape_ascii())
        } else {
            write!(f, "empty byte string")
        }
    }
}

impl std::error::Error for InvalidByte {}

impl From<std::convert::Infallible> for InvalidByte {
    fn from(value: std::convert::Infallible) -> Self {
        // Forward compat idiom, also used by std.
        match value {}
    }
}

impl From<InvalidByte> for std::io::Error {
    fn from(value: InvalidByte) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, value)
    }
}

#[cfg(test)]
mod tests {
    use super::{Arg, Line, Word};

    #[test]
    pub fn line() {
        assert!(Line::from_bytes("foo bar").is_ok());
        assert!(Line::from_bytes("").is_ok());
        assert!(Line::from_bytes("foobar\n").is_err());
        assert!(Line::from_bytes("foo\nbar").is_err());
        assert!(Line::from_bytes("foobar\r\n").is_err());
        assert!(Line::from_bytes("foobar\r").is_err());
        assert!(Line::from_bytes("foo\rbar").is_err());
    }

    #[test]
    pub fn word() {
        assert!(Word::from_bytes("foobar").is_ok());
        assert!(Word::from_bytes("").is_ok());
        assert!(Word::from_bytes("foo\nbar").is_err());
        assert!(Word::from_bytes("foo bar").is_err());
        assert!(Word::from_bytes("foobar ").is_err());
        assert!(Word::from_bytes(" foobar").is_err());
    }

    #[test]
    pub fn arg() {
        assert!(Arg::from_bytes("foobar").is_ok());
        assert!(Arg::from_bytes("foo:bar").is_ok());
        assert!(Arg::from_bytes("").is_err());
        assert!(Arg::from_bytes(":foo").is_err());
    }
}

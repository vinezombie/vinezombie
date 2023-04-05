#![allow(clippy::derivable_impls)]

use std::{borrow::Borrow, num::NonZeroUsize};
use super::{Bytes, Transform};

macro_rules! impl_subtype {
    (
        $doc:literal
        $sname:ident: $ssuper:ident
        $tname:ident: $tsuper:ident
        |$targ:ident| $tbody:block
    ) => {
        impl_subtype! {
            $doc
            $sname: $ssuper
            $tname: $tsuper
            |$targ| $tbody
            |$targ| $tbody
        }
    };
    (
        $doc:literal
        $sname: ident: $ssuper: ident
        $tname:ident: $tsuper:ident
        |$targ:ident| $tbody:block
        |$uarg:ident| $ubody:block
    ) => {
        #[doc = $doc]
        #[repr(transparent)]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub struct $sname<'a>(Bytes<'a>);

        #[doc = concat!("Marker for [`", stringify!($sname), "`]-safe [`Transform`]s'.")]
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = "[`Transform::transform()`]' must return a byte string that maintains"]
        #[doc = concat!("[`",stringify!($sname),"`]'s invariants.")]
        #[doc = "See its struct-level documentation for more info."]
        pub unsafe trait $tname: $tsuper {}

        impl<'a> $sname<'a> {
            /// Returns the first byte and its index that violate this type's guarantees.
            pub fn find_invalid(bytes: impl AsRef<[u8]>) -> Option<InvalidByte> {
                // Optimization: the block here can also do a test for ASCII-validity
                // and use that to infer UTF-8 validity.
                let $targ = bytes.as_ref();
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
            /// Performs an unchecked conversion from `bytes`.
            ///
            /// # Safety
            /// This function assumes that this type's guarantees are upheld by `bytes`.
            pub unsafe fn from_bytes_unchecked(bytes: impl Into<Bytes<'a>>) -> Self {
                $sname(bytes.into())
            }
            /// Transforms `self` using the provided [`Transform`]
            /// that upholds `self`'s invariant.
            pub fn transform<T: $tname + ?Sized>(&mut self, tf: &T) -> T::Value {
                self.0.transform(tf)
            }
            /// Cheaply converts `self` into the next more-general type in the string hierarchy.
            pub fn into_super(self) -> $ssuper<'a> {
                unsafe { std::mem::transmute(self) }
            }
            /// Cheaply converts `self` into the underlying byte string.
            pub fn into_bytes(self) -> Bytes<'a> {
                self.0
            }
        }
        // TODO: More TryFrom/From impls for downcasting/upcasting?
        // The into_super one is probably the most generally-useful
        // due to Nick/User/Kind needing upcasts to Arg.
        impl<'a> From<$sname<'a>> for $ssuper<'a> {
            fn from(value: $sname<'a>) -> $ssuper<'a> {
                value.into_super()
            }
        }
        impl<'a> AsRef<[u8]> for $sname<'a> {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }
        impl<'a> Borrow<[u8]> for $sname<'a> {
            fn borrow(&self) -> &[u8] {
                self.0.borrow()
            }
        }
        impl<'a> std::ops::Deref for $sname<'a> {
            type Target = $ssuper<'a>;

            fn deref(&self) -> &Self::Target {
                unsafe { std::mem::transmute(self) }
            }
        }
    };
}

#[inline(always)]
fn check_bytes(bytes: &[u8], f: impl FnMut(&u8) -> bool) -> Option<InvalidByte> {
    let idx = bytes.iter().position(f)?;
    Some(InvalidByte::new_at(bytes, idx))
}

#[inline]
fn line_byte_check(byte: &u8) -> bool {
    matches!(*byte, b'\0' | b'\r' | b'\n')
}

impl_subtype! {
    "A [`Bytes`] that does not contain NUL, CR, or LF."
    Line: Bytes
    LineSafe: Transform
    |bytes| {
        check_bytes(bytes, line_byte_check)
    }
}

impl<'a> Default for Line<'a> {
    fn default() -> Self {
        Line(Bytes::default())
    }
}

#[inline]
fn word_byte_check(byte: &u8) -> bool {
    *byte == b' ' || line_byte_check(byte)
}

impl_subtype! {
    "A [`Line`] that does not contain ASCII spaces."
    Word: Line
    WordSafe: LineSafe
    |bytes| {
        check_bytes(bytes, word_byte_check)
    }
}

impl<'a> Default for Word<'a> {
    fn default() -> Self {
        Word(Bytes::default())
    }
}

#[inline]
fn arg_first_check(bytes: &[u8]) -> Option<InvalidByte> {
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
        arg_first_check(bytes).or_else(|| check_bytes(bytes, word_byte_check))
    }
    |bytes| {
        arg_first_check(bytes)
    }
}

#[inline]
fn tagkey_byte_check(byte: &u8) -> bool {
    matches!(*byte, b'+' | b'=' | b'/' | b';') || word_byte_check(byte)
}

impl_subtype! {
    "A non-empty [`Word`] that does not contain any of `+`, `=`, `/`, or `;`."
    TagKey: Word
    TagKeySafe: WordSafe
    |bytes| {
        if bytes.is_empty() {
            Some(InvalidByte::new_empty())
        } else {
            check_bytes(bytes, tagkey_byte_check)
        }
    }
}

#[inline]
fn nick_byte_check(byte: &u8) -> bool {
    matches!(*byte, b'!' | b'@') || word_byte_check(byte)
}

#[inline]
fn user_byte_check(byte: &u8) -> bool {
    matches!(*byte, b'@' | b'%') || word_byte_check(byte)
}

impl_subtype! {
    "An [`Arg`] that does not contain any of `!` or `@`.\nIntended for use with nicknames."
    Nick: Arg
    NickSafe: ArgSafe
    |bytes| {
        arg_first_check(bytes).or_else(|| check_bytes(bytes, nick_byte_check))
    }
    |bytes| {
        check_bytes(bytes, nick_byte_check)
    }
}

impl_subtype! {
    "An [`Arg`] that does not contain any of `@` or `%`.\nIntended for use with usernames."
    User: Arg
    UserSafe: ArgSafe
    |bytes| {
        arg_first_check(bytes).or_else(|| check_bytes(bytes, user_byte_check))
    }
    |bytes| {
        check_bytes(bytes, user_byte_check)
    }
}

#[inline]
fn kind_byte_check(byte: &u8) -> bool {
    !byte.is_ascii_digit() && !byte.is_ascii_uppercase()
}

impl_subtype! {
    "An [`Arg`] that only contains ASCII digits and uppercase characters."
    Kind: Arg
    KindSafe: ArgSafe
    |bytes| {
        if bytes.is_empty() {
            Some(InvalidByte::new_empty())
        } else {
            check_bytes(bytes, kind_byte_check)
        }
    }
    |bytes| {
        check_bytes(bytes, kind_byte_check)
    }
}

/// Error indicating that the invariant of a [`Bytes`] newtype has been violated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InvalidByte(u8, Option<NonZeroUsize>);

impl InvalidByte {
    /// Creates an `InvalidByte` representing a violation of a "non-empty string" invariant.
    pub fn new_empty() -> InvalidByte {
        InvalidByte(0u8, None)
    }
    /// Creates an `InvalidBytes` for an invalid bytes.
    pub fn new_at(bytes: &[u8], idx: usize) -> InvalidByte {
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

impl From<InvalidByte> for std::io::Error {
    fn from(value: InvalidByte) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, value)
    }
}

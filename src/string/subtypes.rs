#![allow(clippy::derivable_impls)]

#[macro_use]
mod macros;

mod impls;
#[cfg(test)]
mod tests;

use super::{Bytes, Transform};
use crate::{error::InvalidString, owning::MakeOwning, string::tf::AsciiCasemap};
use std::borrow::Borrow;

/// [`Bytes`] newtypes that uphold some invariant.
///
/// # Safety
/// This trait is not meant to be implemented by foreign types and is NOT stable.
///
/// It is assumed that is_invalid will either reject no non-ASCII bytes or all non-ASCII bytes,
/// in effect ensuring that byte invalidity checks on UTF-8 strings will only result in
/// invalidity on character boundaries.
#[allow(missing_docs)]
pub unsafe trait BytesNewtype<'a>: AsRef<[u8]> + MakeOwning {
    #[doc(hidden)]
    unsafe fn as_bytes_unsafe(&self) -> &'a [u8];
    #[doc(hidden)]
    fn check_others(bytes: &[u8]) -> Option<InvalidString>;
    #[doc(hidden)]
    unsafe fn from_unchecked(bytes: Bytes<'a>) -> Self;
    #[doc(hidden)]
    fn into_bytes(self) -> Bytes<'a>;
    #[doc(hidden)]
    fn into_vec(this: <Self as MakeOwning>::This<'_>) -> Vec<u8>;
    #[doc(hidden)]
    fn is_invalid(byte: &u8) -> bool;
    #[doc(hidden)]
    fn is_utf8_lazy(&self) -> bool;
    #[doc(hidden)]
    unsafe fn using_value(&self, bytes: &'a [u8], utf8: bool) -> Self;
    #[doc(hidden)]
    fn is_secret(&self) -> bool;
}

/// This implementation allows [`Bytes`] to be used wherever any bytes newtype is expected.
unsafe impl<'a> BytesNewtype<'a> for Bytes<'a> {
    unsafe fn as_bytes_unsafe(&self) -> &'a [u8] {
        self.as_bytes_unsafe()
    }
    fn check_others(_: &[u8]) -> Option<InvalidString> {
        None
    }
    unsafe fn from_unchecked(bytes: Bytes<'a>) -> Self {
        bytes
    }
    fn into_bytes(self) -> Bytes<'a> {
        self
    }
    fn into_vec(this: <Self as MakeOwning>::This<'_>) -> Vec<u8> {
        this.into()
    }
    fn is_invalid(_: &u8) -> bool {
        false
    }
    fn is_utf8_lazy(&self) -> bool {
        self.is_utf8_lazy().unwrap_or_default()
    }
    unsafe fn using_value(&self, bytes: &'a [u8], utf8: bool) -> Self {
        use super::Utf8Policy;
        self.using_value(bytes, if utf8 { Utf8Policy::Valid } else { Utf8Policy::Recheck })
    }
    fn is_secret(&self) -> bool {
        self.is_secret()
    }
}

#[inline(always)]
const fn return_none(_: &[u8]) -> Option<InvalidString> {
    None
}

#[inline(always)]
const fn is_invalid_for_nonul(byte: &u8) -> bool {
    *byte == b'\0'
}

impl_subtype! {
    "A [`Bytes`] that does not contain NUL."
    NoNul: Bytes
    NoNulSafe: Transform
    is_invalid_for_nonul;
    return_none;
}

#[inline(always)]
const fn is_invalid_for_line<const CHAIN: bool>(byte: &u8) -> bool {
    matches!(byte, b'\r' | b'\n') || if CHAIN { is_invalid_for_nonul(byte) } else { false }
}

impl_subtype! {
    "A [`NoNul`] that does not contain CR or LF."
    Line: NoNul
    LineSafe: NoNulSafe
    is_invalid_for_line::<true>;
    return_none;
    |bytes| {
        check_bytes!(bytes, is_invalid_for_line::<false>)
    }
}
conversions!(Line: NoNul);

#[inline(always)]
const fn is_invalid_for_word<const CHAIN: bool>(byte: &u8) -> bool {
    *byte == b' ' || if CHAIN { is_invalid_for_line::<true>(byte) } else { false }
}

impl_subtype! {
    "A [`Line`] that does not contain ASCII spaces."
    Word: Line
    WordSafe: LineSafe
    is_invalid_for_word::<true>;
    return_none;
    |bytes| {
        check_bytes!(bytes, is_invalid_for_word::<false>)
    }
}
conversions!(Word: NoNul);
conversions!(Word: Line);

#[inline(always)]
const fn arg_first_check(bytes: &[u8]) -> Option<InvalidString> {
    match bytes.first() {
        None => Some(InvalidString::Empty),
        Some(b':') => Some(InvalidString::Colon),
        _ => None,
    }
}

impl_subtype! {
    "A non-empty [`Word`] that does not begin with `:`."
    Arg: Word
    ArgSafe: WordSafe
    is_invalid_for_word::<true>;
    arg_first_check;
    |bytes| {
        arg_first_check(bytes)
    }
}
conversions!(Arg: NoNul);
conversions!(Arg: Line);
conversions!(Arg: Word);

#[inline(always)]
const fn is_invalid_for_key<const CHAIN: bool>(byte: &u8) -> bool {
    matches!(*byte, b'=' | b';') || if CHAIN { is_invalid_for_word::<true>(byte) } else { false }
}

impl_subtype! {
    "An [`Arg`] that does not contain `=` or `;`.\n"
    Key: Arg
    KeySafe: ArgSafe
    is_invalid_for_key::<true>;
    arg_first_check;
    |bytes| {
        check_bytes!(bytes, is_invalid_for_key::<false>)
    }
}
conversions!(Key: NoNul);
conversions!(Key: Line);
conversions!(Key: Word);
conversions!(Key: Arg);

#[inline(always)]
const fn is_invalid_for_nick<const CHAIN: bool>(byte: &u8) -> bool {
    matches!(*byte, b'!' | b'@') || if CHAIN { is_invalid_for_word::<true>(byte) } else { false }
}

#[inline(always)]
const fn is_invalid_for_user<const CHAIN: bool>(byte: &u8) -> bool {
    matches!(*byte, b'@' | b'%') || if CHAIN { is_invalid_for_word::<true>(byte) } else { false }
}

impl_subtype! {
    "An [`Arg`] that does not contain `!` or `@`.\nIntended for use with nicknames."
    Nick: Arg
    NickSafe: ArgSafe
    is_invalid_for_nick::<true>;
    arg_first_check;
    |bytes| {
        check_bytes!(bytes, is_invalid_for_nick::<false>)
    }
}
conversions!(Nick: NoNul);
conversions!(Nick: Line);
conversions!(Nick: Word);
conversions!(Nick: Arg);

impl_subtype! {
    "An [`Arg`] that does not contain `@` or `%`.\nIntended for use with usernames."
    User: Arg
    UserSafe: ArgSafe
    is_invalid_for_user::<true>;
    arg_first_check;
    |bytes| {
        check_bytes!(bytes, is_invalid_for_user::<false>)
    }
}
conversions!(User: NoNul);
conversions!(User: Line);
conversions!(User: Word);
conversions!(User: Arg);

#[inline(always)]
const fn cmd_byte_check(byte: &u8) -> bool {
    !byte.is_ascii_uppercase()
}

impl_subtype! {
    "An [`Arg`] that only contains ASCII uppercase characters."
    Cmd: Arg
    CmdSafe: ArgSafe
    cmd_byte_check;
    arg_first_check;
    |bytes| {
        check_bytes!(bytes, cmd_byte_check)
    }
}
conversions!(Cmd: NoNul);
conversions!(Cmd: Line);
conversions!(Cmd: Word);
conversions!(Cmd: Arg);

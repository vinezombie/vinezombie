#![allow(clippy::derivable_impls)]

#[macro_use]
mod macros;

mod impls;
#[cfg(test)]
mod tests;

use super::{Bytes, Transform};
use crate::{error::InvalidByte, string::tf::AsciiCasemap};
use std::borrow::Borrow;

#[inline]
pub(crate) const fn is_invalid_for_nonul(byte: &u8) -> bool {
    *byte == b'\0'
}

impl_subtype! {
    "A [`Bytes`] that does not contain NUL."
    NoNul: Bytes
    NoNulSafe: Transform
    |bytes| {
        check_bytes!(bytes, is_invalid_for_nonul)
    }
}

#[inline]
pub(crate) const fn is_invalid_for_line<const CHAIN: bool>(byte: &u8) -> bool {
    matches!(byte, b'\r' | b'\n') || if CHAIN { is_invalid_for_nonul(byte) } else { false }
}

impl_subtype! {
    "A [`NoNul`] that does not contain CR or LF."
    Line: NoNul
    LineSafe: NoNulSafe
    |bytes| {
        check_bytes!(bytes, is_invalid_for_line::<true>)
    }
    |bytes| {
        check_bytes!(bytes, is_invalid_for_line::<false>)
    }
}
conversions!(Line: NoNul);

#[inline]
pub(crate) const fn is_invalid_for_word<const CHAIN: bool>(byte: &u8) -> bool {
    *byte == b' ' || if CHAIN { is_invalid_for_line::<true>(byte) } else { false }
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
conversions!(Word: NoNul);
conversions!(Word: Line);

#[inline]
pub(crate) const fn is_not_ascii(byte: &u8) -> bool {
    !byte.is_ascii()
}

impl_subtype! {
    "A non-empty [`Word`] that contains only ASCII."
    Host: Word
    HostSafe: WordSafe
    |bytes| {
        if bytes.is_empty() {
            Some(InvalidByte::new_empty())
        } else {
            check_bytes!(bytes, is_not_ascii)
        }
    }
    |bytes| {
        if bytes.is_empty() {
            Some(InvalidByte::new_empty())
        } else {
            check_bytes!(bytes, is_not_ascii)
        }
    }
}
conversions!(Host: NoNul);
conversions!(Host: Line);
conversions!(Host: Word);

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
conversions!(Arg: NoNul);
conversions!(Arg: Line);
conversions!(Arg: Word);

#[inline]
pub(crate) const fn is_invalid_for_key<const CHAIN: bool>(byte: &u8) -> bool {
    matches!(*byte, b'=' | b';') || if CHAIN { is_invalid_for_word::<true>(byte) } else { false }
}

impl_subtype! {
    "An [`Arg`] that does not contain `=` or `;`.\n"
    Key: Arg
    KeySafe: ArgSafe
    |bytes| {
        if let Some(e) = arg_first_check(bytes) {
            Some(e)
        } else {
            check_bytes!(bytes, is_invalid_for_key::<true>)
        }
    }
    |bytes| {
        check_bytes!(bytes, is_invalid_for_key::<false>)
    }
}
conversions!(Key: NoNul);
conversions!(Key: Line);
conversions!(Key: Word);
conversions!(Key: Arg);

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
conversions!(Nick: NoNul);
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
conversions!(User: NoNul);
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
conversions!(Cmd: NoNul);
conversions!(Cmd: Line);
conversions!(Cmd: Word);
conversions!(Cmd: Arg);

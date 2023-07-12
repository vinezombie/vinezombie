#![allow(clippy::derivable_impls)]

#[macro_use]
mod macros;

mod builders;
mod impls;
#[cfg(test)]
mod tests;

use super::{Bytes, Transform};
use crate::{error::InvalidByte, string::tf::AsciiCasemap};
use std::borrow::Borrow;

#[inline(always)]
const fn is_invalid_for_nonul(byte: &u8) -> bool {
    *byte == b'\0'
}

impl_subtype! {
    "A [`Bytes`] that does not contain NUL."
    NoNul: Bytes
    NoNulSafe: Transform
    is_invalid_for_nonul;
    |bytes| {
        check_bytes!(bytes, is_invalid_for_nonul)
    }
}
impl_builder!(NoNulBuilder from NoNul with NoNulSafe for NoNul default);

#[inline(always)]
const fn is_invalid_for_line<const CHAIN: bool>(byte: &u8) -> bool {
    matches!(byte, b'\r' | b'\n') || if CHAIN { is_invalid_for_nonul(byte) } else { false }
}

impl_subtype! {
    "A [`NoNul`] that does not contain CR or LF."
    Line: NoNul
    LineSafe: NoNulSafe
    is_invalid_for_line::<true>;
    |bytes| {
        check_bytes!(bytes, is_invalid_for_line::<true>)
    }
    |bytes| {
        check_bytes!(bytes, is_invalid_for_line::<false>)
    }
}
impl_builder!(LineBuilder from Line with LineSafe for Line default);
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
    |bytes| {
        check_bytes!(bytes, is_invalid_for_word::<true>)
    }
    |bytes| {
        check_bytes!(bytes, is_invalid_for_word::<false>)
    }
}
impl_builder!(WordBuilder from Word with WordSafe for Word default);
conversions!(Word: NoNul);
conversions!(Word: Line);

#[inline(always)]
const fn is_not_ascii(byte: &u8) -> bool {
    !byte.is_ascii()
}

impl_subtype! {
    "A non-empty [`Word`] that contains only ASCII."
    Host: Word
    HostSafe: WordSafe
    is_not_ascii;
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
impl_builder!(HostBuilder from Host with HostSafe for Host);
conversions!(Host: NoNul);
conversions!(Host: Line);
conversions!(Host: Word);

#[inline(always)]
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
    is_invalid_for_word::<true>;
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
impl_builder!(ArgBuilder from Word with ArgSafe for Arg);
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
impl_builder!(KeyBuilder from Key with KeySafe for Key);
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
impl_builder!(NickBuilder from Nick with NickSafe for Nick);
conversions!(Nick: NoNul);
conversions!(Nick: Line);
conversions!(Nick: Word);
conversions!(Nick: Arg);

impl_subtype! {
    "An [`Arg`] that does not contain `@` or `%`.\nIntended for use with usernames."
    User: Arg
    UserSafe: ArgSafe
    is_invalid_for_user::<true>;
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
impl_builder!(UserBuilder from User with UserSafe for User);
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

//! Base64 encoding and decoding.
//!
//! This module contains two functions,
//! one for encoding octet strings as standard Base64,
//! and one for decoding the same.

// TODO: Homebrew implementations here are possible.

use crate::{IrcStr, IrcWord};

static CONFIG: base64::Config =
    base64::Config::new(base64::CharacterSet::Standard, true).decode_allow_trailing_bits(true);

/// Encodes bytes as a Base64 string.
pub fn encode(b256: &[u8]) -> IrcWord<'static> {
    let string: IrcStr<'static> = base64::encode_config(b256, CONFIG).into();
    unsafe { IrcWord::new_unchecked(string) }
}

/// Decodes a Base64 string.
pub fn decode(b64: &str) -> Option<Vec<u8>> {
    base64::decode_config(b64, CONFIG).ok()
}

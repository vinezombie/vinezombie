use crate::string::{Bytes, Word};
use std::num::NonZeroU8;

/// Returns the unescaped value for an tag value escape code.
pub fn unescape_byte(byte: &u8) -> u8 {
    match byte {
        b':' => b';',
        b's' => b' ',
        b'r' => b'\r',
        b'n' => b'\n',
        b => *b,
    }
}

/// Returns the escape code for a particular byte in a tag value,
/// or `None` if no escaping is necessary.
pub fn escape_byte(byte: &u8) -> Option<NonZeroU8> {
    match byte {
        b';' => Some(unsafe { NonZeroU8::new_unchecked(b':') }),
        b' ' => Some(unsafe { NonZeroU8::new_unchecked(b's') }),
        b'\r' => Some(unsafe { NonZeroU8::new_unchecked(b'r') }),
        b'\n' => Some(unsafe { NonZeroU8::new_unchecked(b'n') }),
        b'\\' => Some(unsafe { NonZeroU8::new_unchecked(b'\\') }),
        _ => None,
    }
}

/// Returns an escaped form of the provided tag value.
pub fn escape<'a>(tag_value: impl Into<Bytes<'a>>) -> Word<'a> {
    let tag_value = tag_value.into();
    let Some(first_idx) = tag_value.iter().position(|c| escape_byte(c).is_some()) else {
        return unsafe {
            Word::from_unchecked(tag_value)
        }
    };
    let (mut new_bytes, rest) = unsafe {
        let (no_escape, rest) = tag_value.as_bytes_unsafe().split_at(first_idx);
        // rest contains at least one byte because first_idx is a valid index.
        let (first, rest) = rest.split_first().unwrap_unchecked();
        let count = 1 + rest.iter().filter(|c| escape_byte(c).is_some()).count();
        let mut new_bytes = Vec::with_capacity(tag_value.len() + count);
        new_bytes.extend_from_slice(no_escape);
        new_bytes.push(b'\\');
        new_bytes.push(escape_byte(first).unwrap().get());
        (new_bytes, rest)
    };
    for byte in rest {
        if let Some(esc) = escape_byte(byte) {
            new_bytes.push(b'\\');
            new_bytes.push(esc.get());
        } else {
            new_bytes.push(*byte);
        }
    }
    unsafe { Word::from_unchecked(new_bytes.into()) }
}

/// Returns an unescaped form of the provided tag value.
pub fn unescape<'a>(tag_value: impl Into<Bytes<'a>>) -> Bytes<'a> {
    let tag_value = tag_value.into();
    let Some(first_idx) = tag_value.iter().position(|c| *c == b'\\') else {
        return tag_value;
    };
    let (mut new_bytes, rest) = unsafe {
        let (no_escape, rest) = tag_value.as_bytes_unsafe().split_at(first_idx);
        // rest contains at least one byte because first_idx is a valid index.
        let (first, rest) = rest.split_first().unwrap_unchecked();
        let mut new_bytes = Vec::with_capacity(tag_value.len() - 1);
        new_bytes.extend_from_slice(no_escape);
        new_bytes.push(unescape_byte(first));
        (new_bytes, rest)
    };
    let mut esc = false;
    for byte in rest {
        if esc {
            new_bytes.push(unescape_byte(byte));
            esc = false;
        } else if *byte == b'\\' {
            esc = true;
        } else {
            new_bytes.push(*byte);
        }
    }
    new_bytes.into()
}

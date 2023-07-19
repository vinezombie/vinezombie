//! String transformations, including parsers and casemaps.

mod casemap;
mod escape;

pub use {casemap::*, escape::*};

use super::{Transformation, Utf8Policy};

pub(super) unsafe fn map_bytes(
    bytes: &[u8],
    utf8: Utf8Policy,
    mut f: impl FnMut(&u8) -> u8,
) -> Transformation<'_, ()> {
    let mut replace = 0u8;
    let mut idx = 0usize;
    let mut ascii_only = true;
    for byte in bytes {
        let mapped = f(byte);
        ascii_only &= mapped < 128;
        if mapped != *byte {
            replace = mapped;
            break;
        }
        idx += 1;
    }
    let transformed = if idx < bytes.len() {
        let mut new_bytes = Vec::with_capacity(bytes.len());
        new_bytes.extend_from_slice(&bytes[..idx]);
        new_bytes.push(replace);
        idx += 1;
        for byte in &bytes[idx..] {
            let mapped = f(byte);
            ascii_only &= mapped < 128;
            new_bytes.push(mapped);
        }
        new_bytes.into()
    } else {
        bytes.into()
    };
    let utf8 = if ascii_only { Utf8Policy::Valid } else { utf8 };
    Transformation { value: (), transformed, utf8 }
}

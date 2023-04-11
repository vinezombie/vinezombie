//! String transformations, including parsers and casemaps.

mod casemap;
#[cfg(test)]
mod tests;

pub use casemap::*;

use super::{Transformation, Utf8Policy};

pub(self) unsafe fn map_bytes(
    bytes: &[u8],
    utf8: Utf8Policy,
    mut f: impl FnMut(&u8) -> u8,
) -> Transformation<'_, ()> {
    let mut replace = 0u8;
    let mut idx = 0usize;
    let mut utf8_valid = true;
    for byte in bytes {
        let mapped = f(byte);
        utf8_valid &= mapped < 128;
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
            utf8_valid &= mapped < 128;
            new_bytes.push(mapped);
        }
        new_bytes.into()
    } else {
        bytes.into()
    };
    let utf8 = if utf8_valid { Utf8Policy::Valid } else { utf8 };
    Transformation { value: (), transformed, utf8 }
}

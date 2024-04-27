use super::StringSide;
use crate::string::{LineSafe, NoNulSafe, Transform, Transformation, Utf8Policy};
use std::borrow::Cow;

/// Removes ASCII whitespace from a provided string.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrimAscii(pub StringSide);

impl Default for TrimAscii {
    fn default() -> Self {
        TrimAscii(StringSide::End)
    }
}

unsafe impl Transform for TrimAscii {
    type Value = ();

    fn transform<'a>(self, bytes: &crate::string::Bytes<'a>) -> Transformation<'a, Self::Value> {
        let mut slice = unsafe { bytes.as_bytes_unsafe() };
        if self.0 != StringSide::Start {
            while let Some((last, rest)) = slice.split_last() {
                if last.is_ascii_whitespace() {
                    slice = rest;
                } else {
                    break;
                }
            }
        }
        if self.0 != StringSide::End {
            while let Some((last, rest)) = slice.split_first() {
                if last.is_ascii_whitespace() {
                    slice = rest;
                } else {
                    break;
                }
            }
        }
        Transformation {
            value: (),
            transformed: Cow::Borrowed(slice),
            utf8: Utf8Policy::PreserveStrict,
        }
    }
}
unsafe impl NoNulSafe for TrimAscii {}
unsafe impl LineSafe for TrimAscii {}

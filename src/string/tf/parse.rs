use crate::string::{Transform, Transformation, Utf8Policy};
use crate::{
    error::InvalidByte,
    string::{
        is_invalid_for_key,
        subtypes::{is_invalid_for_line, is_invalid_for_word},
        Bytes, Key, Line, LineSafe, Word, WordSafe,
    },
};

// TODO: Dedup SplitLine and SplitWord.
// In theory we can make this generic over any newtype of Bytes that only
// enforces a constraint on the individual bytes it contains,
// but there's probably no point in doing that.

/// Parser that splits [`Line`]s from the front of a byte string.
///
/// This parser discards any bytes that are `Line`-invalid,
/// then extracts bytes until either another `Line`-invalid byte is found or the string ends.
/// The returned `Line` will be empty if the input string contains no `Line`-valid bytes.
#[derive(Clone, Copy, Debug)]
pub struct SplitLine;

unsafe impl Transform for SplitLine {
    type Value<'a> = Line<'a>;
    fn transform<'a>(self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>> {
        unsafe {
            let slice = bytes.as_bytes_unsafe();
            let Some(first_valid_idx) = slice.iter().position(
                |b| !is_invalid_for_line(b)
            ) else {
                return Transformation::empty(Line::default())
            };
            let slice = slice.split_at(first_valid_idx).1;
            let end_idx = slice.iter().position(is_invalid_for_line).unwrap_or(slice.len());
            let (line, rest) = slice.split_at(end_idx);
            Transformation {
                value: Line::from_unchecked(bytes.using_value(line, Utf8Policy::Preserve)),
                transformed: rest.into(),
                utf8: Utf8Policy::Preserve,
            }
        }
    }
}
unsafe impl LineSafe for SplitLine {}
unsafe impl WordSafe for SplitLine {}

/// Parser that splits [`Word`]s from the front of a byte string.
///
/// This parser discards any bytes that are `Word`-invalid,
/// then extracts bytes until either another `Word`-invalid byte is found or the string ends.
/// The returned `Word` will be empty if the input string contains no `Word`-valid bytes.
#[derive(Clone, Copy, Debug)]
pub struct SplitWord;

unsafe impl Transform for SplitWord {
    type Value<'a> = Word<'a>;

    fn transform<'a>(self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>> {
        unsafe {
            let slice = bytes.as_bytes_unsafe();
            let Some(first_valid_idx) = slice.iter().position(
                |b| !is_invalid_for_word::<true>(b)
            ) else {
                return Transformation::empty(Word::default())
            };
            let slice = slice.split_at(first_valid_idx).1;
            let end_idx = slice.iter().position(is_invalid_for_word::<true>).unwrap_or(slice.len());
            let (word, rest) = slice.split_at(end_idx);
            Transformation {
                value: Word::from_unchecked(bytes.using_value(word, Utf8Policy::Preserve)),
                transformed: rest.into(),
                utf8: Utf8Policy::Preserve,
            }
        }
    }
}
unsafe impl LineSafe for SplitWord {}
unsafe impl WordSafe for SplitWord {}

/// Parser that splits [`Key`]s from the front of a byte string.
///
/// This parser extracts the longest continuous range of `Key`-valid bytes
/// from the front of the byte string. If a `Key`-invalid byte is found,
/// extracts and returns it seperately.
/// It attempts to convert the range of `Key`-valid bytes into a `Key`;
/// this can fail if the range is not `Arg`-valid (i.e. empty or beginning with `':'`).
#[derive(Clone, Copy, Debug)]
pub struct SplitKey;

unsafe impl Transform for SplitKey {
    type Value<'a> = (Result<Key<'a>, InvalidByte>, Option<u8>);

    fn transform<'a>(self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>> {
        unsafe {
            let slice = bytes.as_bytes_unsafe();
            let first_invalid_idx = slice.iter().position(is_invalid_for_key::<true>);
            let (key, rest, inval) = if let Some(first_invalid_idx) = first_invalid_idx {
                let (key, rest) = slice.split_at(first_invalid_idx);
                if let Some((first, rest)) = rest.split_first() {
                    (key, rest, Some(*first))
                } else {
                    (key, Default::default(), None)
                }
            } else {
                (slice, Default::default(), None)
            };
            let key = if let Some(inval) = Key::find_invalid(key) {
                Err(inval)
            } else {
                // Split shouldn't happen in the middle of a UTF-8 character.
                let bytes = bytes.using_value(key, Utf8Policy::Preserve);
                Ok(Key::from_unchecked(bytes))
            };
            // An additional byte gets munched, but none of the Key-invalid bytes
            // are part of multi-byte UTF-8 characters.
            Transformation {
                value: (key, inval),
                transformed: rest.into(),
                utf8: Utf8Policy::Preserve,
            }
        }
    }
}
unsafe impl LineSafe for SplitKey {}
unsafe impl WordSafe for SplitKey {}

/// Parser that extracts one byte from the front of a byte string.
#[derive(Clone, Copy, Debug)]
pub struct SplitFirst;

unsafe impl Transform for SplitFirst {
    type Value<'a> = Option<u8>;

    fn transform<'a>(self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>> {
        unsafe {
            let slice = bytes.as_bytes_unsafe();
            if let Some((first, rest)) = slice.split_first() {
                // This transform can easily make or break UTF-8 validity.
                // However, if the split-off byte is ASCII, it will neither
                // make an invalid string valid, nor invalidate a previously valid string.
                let utf8 =
                    if first.is_ascii() { Utf8Policy::PreserveStrict } else { Utf8Policy::Recheck };
                Transformation { value: Some(*first), transformed: rest.into(), utf8 }
            } else {
                Transformation::empty(None)
            }
        }
    }
}
unsafe impl LineSafe for SplitFirst {}
unsafe impl WordSafe for SplitFirst {}

/// Transform that splits at the first byte for which the provided function returns `true`.
#[derive(Clone, Copy, Debug)]
pub struct Split<F>(pub F);

unsafe impl<F: FnMut(&u8) -> bool> Transform for Split<F> {
    type Value<'a> = Bytes<'a>;

    fn transform<'a>(mut self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>> {
        unsafe {
            let slice = bytes.as_bytes_unsafe();
            if let Some(idx) = slice.iter().position(&mut self.0) {
                let (ret, rest) = slice.split_at(idx);
                Transformation {
                    value: bytes.using_value(ret, Utf8Policy::Recheck),
                    transformed: rest.into(),
                    // Could be more optimal.
                    utf8: Utf8Policy::Recheck,
                }
            } else {
                Transformation::empty(bytes.using_value(slice, Utf8Policy::PreserveStrict))
            }
        }
    }
}
unsafe impl<F: FnMut(&u8) -> bool> LineSafe for Split<F> {}
unsafe impl<F: FnMut(&u8) -> bool> WordSafe for Split<F> {}

/// Transform that discards leading bytes while the provided function returns `true`.
#[derive(Clone, Copy, Debug)]
pub struct TrimStart<F>(pub F);

unsafe impl<F: FnMut(&u8) -> bool> Transform for TrimStart<F> {
    type Value<'a> = ();

    fn transform<'a>(mut self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>> {
        unsafe {
            let slice = bytes.as_bytes_unsafe();
            if let Some(idx) = slice.iter().position(|b| !self.0(b)) {
                let transformed = slice.split_at(idx).1.into();
                Transformation {
                    value: (),
                    transformed,
                    // Could be more optimal.
                    utf8: Utf8Policy::Recheck,
                }
            } else {
                Transformation::empty(())
            }
        }
    }
}
unsafe impl<F: FnMut(&u8) -> bool> LineSafe for TrimStart<F> {}
unsafe impl<F: FnMut(&u8) -> bool> WordSafe for TrimStart<F> {}

/// Transform that takes a chunk from the start that is exactly some number of bytes long.
#[derive(Clone, Copy, Debug)]
pub struct SplitAt(pub usize);

unsafe impl Transform for SplitAt {
    type Value<'a> = Option<Bytes<'a>>;

    fn transform<'a>(self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>> {
        unsafe {
            let slice = bytes.as_bytes_unsafe();
            if self.0 > slice.len() {
                Transformation {
                    value: None,
                    transformed: slice.into(),
                    utf8: Utf8Policy::PreserveStrict,
                }
            } else {
                let (chunk, rest) = slice.split_at(self.0);
                Transformation {
                    value: Some(bytes.using_value(chunk, Utf8Policy::Recheck)),
                    transformed: rest.into(),
                    utf8: Utf8Policy::Recheck,
                }
            }
        }
    }
}
unsafe impl LineSafe for SplitAt {}
unsafe impl WordSafe for SplitAt {}

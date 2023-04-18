use crate::string::{subtypes::is_invalid_for_word, Bytes, LineSafe, Word, WordSafe};
use crate::string::{Transform, Transformation, Utf8Policy};

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
            let slice = bytes.as_slice_unsafe();
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

/// Parser that extracts one byte from the front of a byte string.
#[derive(Clone, Copy, Debug)]
pub struct SplitFirst;

unsafe impl Transform for SplitFirst {
    type Value<'a> = Option<u8>;

    fn transform<'a>(self, bytes: &Bytes<'a>) -> Transformation<'a, Self::Value<'a>> {
        unsafe {
            let slice = bytes.as_slice_unsafe();
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
            let slice = bytes.as_slice_unsafe();
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
            let slice = bytes.as_slice_unsafe();
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

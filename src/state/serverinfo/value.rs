use crate::{state::Mode, string::Word};
use std::{error::Error, num::*};

/// Values that can be used as ISUPPORT tokens.
pub trait ISupportValue: std::any::Any + PartialEq + Eq + Clone + Send + Sync {
    /// Parses a value out of a [`Word`].
    ///
    /// This function should be idempotent.
    fn try_from_word(value: Word<'_>) -> Result<Self, Box<dyn Error + Send + Sync>>;
    /// Creates a [`Word`] out of self.
    ///
    /// Equality between generated words must agree with equality between the original values.
    /// That is, `a.to_word() == b.to_word()` must always equal `a == b`.
    fn to_word(&self) -> Option<Word<'static>>;
}

impl ISupportValue for () {
    fn try_from_word(_: Word<'_>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Err("unexpected ISUPPORT token value".into())
    }

    fn to_word(&self) -> Option<Word<'static>> {
        None
    }
}

impl<T: ISupportValue> ISupportValue for Option<T> {
    fn try_from_word(value: Word<'_>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        T::try_from_word(value).map(Some)
    }

    fn to_word(&self) -> Option<Word<'static>> {
        self.as_ref().and_then(T::to_word)
    }
}

impl ISupportValue for Word<'static> {
    fn try_from_word(value: Word<'_>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(value.owning())
    }
    fn to_word(&self) -> Option<Word<'static>> {
        Some(self.clone())
    }
}

macro_rules! impl_isv_with_strings
    {
    ($($name:ty),+) => {
         $(impl ISupportValue for $name {
            fn try_from_word(value: Word<'_>) -> Result<Self, Box<dyn Error + Send + Sync>> {
                let Some(this) = value.to_utf8() else {
                    let _ = std::str::from_utf8(value.as_bytes())?;
                    panic!("spurious failure of Bytes::to_utf8");
                };
                let value = this.parse()?;
                Ok(value)
            }
            fn to_word(&self) -> Option<Word<'static>> {
                Some(unsafe { Word::from_unchecked(self.to_string().into()) })
            }
         })+
    };
}

impl ISupportValue for Mode {
    fn try_from_word(value: Word<'_>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        value.first().copied().and_then(Mode::new).ok_or_else(|| "invalid mode char".into())
    }

    fn to_word(&self) -> Option<Word<'static>> {
        let arg: crate::string::Arg<'static> = (*self).into();
        Some(arg.into())
    }
}

// Due to semantic ambiguity involving u8,
// we shouldn't implement ISupportValue for it here.

impl_isv_with_strings! {
    u16, u32, u64, u128, i8, i16, i32, i64, i128, char,
    NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128,
    NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128
}

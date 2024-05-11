//! ISUPPORT tokens.

// TODO: Very incomplete. https://defs.ircdocs.horse/defs/isupport

use std::num::{NonZeroU16, NonZeroU32};

use super::{ISupport, Name, NameValued};
use crate::state::{Mode, ModeTypes, StatusModes};
use crate::{
    error::ParseError,
    string::{Bytes, Key, Word},
};

macro_rules! defn_isupport {
    ($key:ident: $value:ty = |$arg:ident| $parse:expr $(, $doc:literal)*) => {
        #[doc = concat!("The `", stringify!($key), "` ISUPPORT token.")]
        $(#[doc = $doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
        pub struct $key;
        impl $key {
            /// The ISUPPORT token `self` stands in for as a [`Key`].
            #[allow(clippy::declare_interior_mutable_const)]
            pub const NAME: Key<'static> =
                unsafe { Key::from_unchecked(Bytes::from_str(stringify!($key))) };
            /// Returns a reference to a static [`Key`] representing `self`'s name.
            pub fn as_key<'a>(&self) -> &'static Key<'a> {
                static VALUE: Key<'static> = $key::NAME;
                &VALUE
            }
        }
        impl std::fmt::Display for $key {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                stringify!($key).fmt(f)
            }
        }
        impl std::hash::Hash for $key {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.as_key().hash(state)
            }
        }
        impl<'a> From<$key> for Key<'a> {
            fn from(v: $key) -> Key<'a> {
                v.as_key().clone()
            }
        }
        impl<'a> PartialEq<Key<'a>> for $key {
            fn eq(&self, other: &Key<'a>) -> bool {
                *self.as_key() == *other
            }
        }
        impl<'a> PartialEq<$key> for Key<'a> {
            fn eq(&self, other: &$key) -> bool {
                *other == *self
            }
        }
        impl<'a> std::borrow::Borrow<Key<'a>> for $key {
            fn borrow(&self) -> &Key<'a> {
                self.as_key()
            }
        }
        impl Name<ISupport> for $key {
            fn as_raw(&self) -> &'static <ISupport as super::NameClass>::Raw<'static> {
                self.as_key()
            }
        }
        impl NameValued<ISupport> for $key {
            type Value<'a> = $value;

            fn from_union<'a>(
                input: &<ISupport as super::NameClass>::Union<'a>,
            ) -> Result<Self::Value<'a>, crate::error::ParseError> {
                use std::error::Error;
                let (_, raw) = input;
                #[inline(always)]
                fn do_parse($arg: &Word<'_>) -> Result<$value, Box<dyn Error + Send + Sync>> {
                    $parse
                }
                match do_parse(raw) {
                    Ok(rv) => Ok(rv),
                    Err(e) => Err(ParseError::InvalidField(
                        format!("{} value", stringify!($name)).into(),
                        e,
                    )),
                }
            }
        }
    };
}

macro_rules! isupport_unitary {
    ($($name:ident)+) => {
        $(
            defn_isupport!($name: () = |_arg| Ok(()));
        )+
    }
}

macro_rules! isupport_strparse {
    ($($name:ident: $value:ty)+) => {
        $(
            defn_isupport!($name: $value = |arg| {
                let Some(this) = arg.to_utf8() else {
                    let _ = std::str::from_utf8(arg.as_bytes())?;
                    panic!("spurious failure of Bytes::to_utf8");
                };
                Ok(this.parse()?)
            });
        )+
    }
}

macro_rules! isupport_strparse_option {
    ($($name:ident: $value:ty)+) => {
        $(
            defn_isupport!($name: Option<$value> = |arg| {
                if arg.is_empty() {
                    return Ok(None);
                }
                let Some(this) = arg.to_utf8() else {
                    let _ = std::str::from_utf8(arg.as_bytes())?;
                    panic!("spurious failure of Bytes::to_utf8");
                };
                Ok(Some(this.parse()?))
            });
        )+
    }
}

macro_rules! isupport_mode {
    ($($name:ident $(= $default:literal)?)+) => {
        $(
            defn_isupport!($name: Mode = |arg| {
                if let Some(ml) = arg.first().copied() {
                    Mode::new(ml).ok_or_else(|| "invalid mode letter".into())
                } else {
                    $(return Ok(unsafe {Mode::new_unchecked($default)});)?
                    #[allow(unreachable_code)]
                    Err("missing mode letter".into())
                }
            });
        )+
    }
}

isupport_unitary! {
    ETRACE
    KNOCK
    SAFELIST
    UTF8ONLY
    WHOX
}

isupport_strparse! {
    AWAYLEN: NonZeroU16
    CHANNELLEN: NonZeroU16
    HOSTLEN: NonZeroU16
    KICKLEN: NonZeroU16
    MODES: NonZeroU16
    NICKLEN: NonZeroU16
    TOPICLEN: NonZeroU16
    USERLEN: NonZeroU16
}

isupport_strparse_option! {
    MONITOR: NonZeroU32
    SILENCE: NonZeroU32
}

isupport_mode! {
    BOT
    CALLERID = b'g'
    EXCEPTS = b'e'
    INVEX = b'I'
}

defn_isupport!(NETWORK: Word<'static> = |arg| Ok(arg.clone().owning()));
defn_isupport!(CHANMODES: ModeTypes = |arg| Ok(ModeTypes::parse(arg.as_bytes()).0));
defn_isupport!(PREFIX: StatusModes = |arg| Ok(StatusModes::parse(arg.as_bytes())?));
defn_isupport!(
    USERMODES: ModeTypes = |arg| Ok(ModeTypes::parse(arg.as_bytes()).0),
    "",
    "This ISUPPORT token is not standard nor generally used.",
    "The information associated with this token is generally obtained from the 004 message.",
    "The name was chosen as the most probable name for this token should it exist in the future."
);

//! Structured server information

#[cfg(test)]
mod tests;
mod value;

pub use value::*;

use crate::{
    error::ParseError,
    string::{Arg, Key, Word},
};
use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::num::{NonZeroU16, NonZeroU32};

use super::Mode;

/// ISUPPORT tokens and value parsers.
pub trait ISupport: Any + Send + Sync {
    /// The type of value associated with this token.
    type Value: ISupportValue;
    /// Returns the name of this token as a [`Key`].
    ///
    /// This should be unique across every value of every type
    /// for which [`ISupport`] is implemented. Parsing conflicts may occur otherwise.
    fn key(&self) -> Key<'static>;

    /// Returns the default value for this token.
    fn default_value() -> Result<Self::Value, ParseError>;
    /// Implementation detail. Do not implement yourself.
    #[doc(hidden)]
    fn _serverinfo_get<'a>(&self, si: &'a ServerInfo) -> Option<&'a Self::Value> {
        si._misc().get(&self.type_id()).and_then(|v| v.downcast_ref::<Self::Value>())
    }
    /// Implementation detail. Do not implement yourself.
    #[doc(hidden)]
    fn _severinfo_set(&self, si: &mut ServerInfo, value: Option<Self::Value>) -> bool {
        if let Some(value) = value {
            si._misc_mut().insert(self.type_id(), Box::new(value)).is_some()
        } else {
            si._misc_mut().remove(&self.type_id()).is_some()
        }
    }
}

type AnyBox = Box<dyn Any + Send + Sync>;
type MiscMap = BTreeMap<TypeId, AnyBox>;
type ValueParser = Box<dyn Fn(&mut ServerInfo, Word<'_>) -> Result<(), ParseError> + Send + Sync>;

/// Matcher and parser for ISUPPORT tokens.
pub struct ISupportParser {
    map: BTreeMap<Key<'static>, ValueParser>,
}

static GLOBAL_ISP: std::sync::OnceLock<std::sync::Arc<ISupportParser>> = std::sync::OnceLock::new();

impl ISupportParser {
    /// Creates an empty [`ISupportParser`].
    ///
    /// This is unlikely to be what you want, as it will recognize no ISUPPORT tokens.
    /// See [`ISupportParser::global()`] or [`ISupportParser::full()`].
    pub fn empty() -> ISupportParser {
        ISupportParser { map: BTreeMap::default() }
    }
    /// Returns a lazily-intialized parser initialized with [`ISupportParser::full()`].
    pub fn global() -> std::sync::Arc<ISupportParser> {
        GLOBAL_ISP.get_or_init(|| std::sync::Arc::new(ISupportParser::full())).clone()
    }
    /// Registers an [`ISupport`], allowing it to be parsed.
    ///
    /// Returns `true` if this operation overwrote an existing [`ISupport`].
    pub fn add<K: ISupport>(&mut self, key: K) -> bool {
        let key_string = key.key();
        self.map
            .insert(
                key_string,
                Box::new(move |si, value| {
                    let value = if !value.is_empty() {
                        K::Value::try_from_word(value).map_err(|e| {
                            ParseError::InvalidField(key.key().to_utf8_lossy_static(), e)
                        })
                    } else {
                        K::default_value()
                    }?;
                    key._severinfo_set(si, Some(value));
                    Ok(())
                }),
            )
            .is_some()
    }
    /// Parses a key and optional value and updates a [`ServerInfo`] with the result.
    ///
    /// If the key-value pair was already present in `si`, overwrites the old value.
    ///
    /// Returns `Ok(true)` if the key was recognized and `si` was successfully updated.
    /// Returns `Ok(false)` if the key was NOT recognized.
    /// Returns `Err` is the key was recognized but parsing failed.
    pub fn parse_and_update(
        &self,
        si: &mut ServerInfo,
        key: &Key<'_>,
        value: Word<'_>,
    ) -> Result<bool, ParseError> {
        if let Some(updater) = self.map.get(key) {
            updater(si, value)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

macro_rules! make_serverinfo {
    ($($name:ident: $assoctype:ty $(= $default:expr)?;)+) => {
        #[doc = "Common [`ISupport`] implementations."]
        pub mod isupport {
            use super::{ServerInfo, ISupport};
            use std::num::{NonZeroU16, NonZeroU32};
            use crate::state::Mode;
            use crate::{error::ParseError, string::{Key, Word}};
            $(
            #[doc = "The"]
            #[doc = stringify!($name)]
            #[doc = "ISUPPORT token."]
            #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
            pub struct $name;

            impl ISupport for $name {
                type Value = $assoctype;
                fn key(&self) -> Key<'static> {
                    Key::from_str(stringify!($name))
                }
                fn default_value() -> Result<Self::Value, ParseError> {
                    $(return $default;)?
                    #[allow(unreachable_code)]
                    Err(ParseError::MissingField(stringify!($name).into()))
                }
                fn _serverinfo_get<'a>(&self, si: &'a ServerInfo) -> Option<&'a Self::Value> {
                    si.$name.as_ref()
                }
                fn _severinfo_set(&self, si: &mut ServerInfo, value: Option<Self::Value>) -> bool {
                    let had = si.$name.is_some();
                    si.$name = value;
                    had
                }
            })+
        }

        #[doc="A collection of information about a server."]
        #[derive(Debug, Default)]
        #[allow(non_snake_case)]
        pub struct ServerInfo {
            version: Option<Arg<'static>>,
            misc: MiscMap,
            $($name: Option<$assoctype>),+
        }

        impl ServerInfo {
            #[doc = "Creates a new empty `ServerInfo`."]
            pub const fn new() -> ServerInfo {
                ServerInfo {
                    version: None,
                    misc: BTreeMap::new(),
                    $($name: None),+
                }
            }
        }

        impl ISupportParser {
            #[doc = "Creates a new [`ISupportParser`] containing all of the"]
            #[doc = "[`ISupport`] implementations in this library."]
            #[doc = ""]
            #[doc = "This is an expensive operation."]
            #[doc = "[`ISupportParser::global()`] may be preferable if your application"]
            #[doc = "does not require ISUPPORT handling outside of what this library offers."]
            pub fn full() -> ISupportParser {
                use isupport::*;
                let mut retval = ISupportParser::empty();
                $(retval.add($name);)+
                retval
            }
        }

    };
}
impl ServerInfo {
    /// Returns the sever version string, or `"*"` if unknown.
    pub fn version(&self) -> Arg<'static> {
        self.version.clone().unwrap_or(crate::consts::STAR.into())
    }
    /// Gets a shared reference to the server version string.
    pub fn version_ref(&self) -> &Option<Arg<'static>> {
        &self.version
    }
    /// Gets a mutable reference to the server version string.
    pub fn version_mut(&mut self) -> &mut Option<Arg<'static>> {
        &mut self.version
    }
    /// Sets an ISUPPORT token.
    pub fn insert<K: ISupport>(&mut self, key: &K, value: K::Value) -> bool {
        key._severinfo_set(self, Some(value))
    }
    /// Clears an ISUPPORT token.
    pub fn remove<K: ISupport>(&mut self, key: &K) -> bool {
        key._severinfo_set(self, None)
    }
    /// Retrieves the value associated with an ISUPPORT token.
    pub fn get<'a, K: ISupport>(&'a self, key: &K) -> Option<&'a K::Value> {
        key._serverinfo_get(self)
    }
    /// Updates from a `RPL_MYINFO` (004) message.
    ///
    /// Currently ignores mode info.
    pub fn parse_myinfo(&mut self, args: &[Arg<'_>]) {
        let mut args = args.iter().skip(2);
        // ^ client, servername
        let Some(version) = args.next() else {
            return;
        };
        self.version = Some(version.clone().owning());
        // TODO: Modes.
    }
    /// Implementation detail. Do not call yourself.
    #[doc(hidden)]
    pub fn _misc(&self) -> &MiscMap {
        &self.misc
    }
    /// Implementation detail. Do not call yourself.
    #[doc(hidden)]
    pub fn _misc_mut(&mut self) -> &mut MiscMap {
        &mut self.misc
    }
}

make_serverinfo! {
    // TODO: Very incomplete. https://defs.ircdocs.horse/defs/isupport
    AWAYLEN: NonZeroU16;
    BOT: Mode;
    ETRACE: () = Ok(());
    CALLERID: Mode = Ok(unsafe {Mode::new_unchecked(b'g')});
    CHANNELLEN: NonZeroU16;
    EXCEPTS: Mode = Ok(unsafe {Mode::new_unchecked(b'e')});
    HOSTLEN: NonZeroU16;
    INVEX: Mode = Ok(unsafe {Mode::new_unchecked(b'I')});
    KICKLEN: NonZeroU16;
    KNOCK: () = Ok(());
    MODES: NonZeroU16;
    MONITOR: Option<NonZeroU32> = Ok(None);
    NETWORK: Word<'static>;
    NICKLEN: NonZeroU16;
    SAFELIST: () = Ok(());
    SILENCE: Option<NonZeroU32> = Ok(None);
    TOPICLEN: NonZeroU16;
    UTF8ONLY: () = Ok(());
    WHOX: () = Ok(());
}

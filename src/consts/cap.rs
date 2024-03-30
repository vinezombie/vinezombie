//! IRCv3 capabilities.
//!
//! To maintain consistency, these names are all uppercased
//! from their official versions.

use super::{Cap, Tag, TagWithValue};
use crate::string::{Bytes, Key, Splitter, Word};
use std::collections::BTreeSet;

macro_rules! defn_cap {
    ($key:ident = $value:literal) => {
        #[doc = concat!("The `", $value, "` capability.")]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
        pub struct $key;
        impl $key {
            /// The capability name `self` stands in for as a [`Key`].
            #[allow(clippy::declare_interior_mutable_const)]
            pub const NAME: Key<'static> = unsafe { Key::from_unchecked(Bytes::from_str($value)) };
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
        impl Tag<Cap> for $key {
            fn as_raw(&self) -> &'static <Cap as super::TagClass>::Raw<'static> {
                self.as_key()
            }
        }
    };
}

defn_cap!(ACCOUNT_NOTIFY = "account-notify");
defn_cap!(ACCOUNT_TAG = "account-tag");
defn_cap!(BATCH = "batch");
defn_cap!(CHGHOST = "chghost");
defn_cap!(ECHO_MESSAGE = "echo-message");
defn_cap!(EXTENDED_JOIN = "extended-join");
defn_cap!(EXTENDED_MONITOR = "extended-monitor");
defn_cap!(INVITE_NOTIFY = "invite-notify");
defn_cap!(LABELED_RESPONSE = "labeled-response");
defn_cap!(MESSAGE_TAGS = "message-tags");
defn_cap!(MSGID = "msgid");
defn_cap!(MULTI_PREFIX = "multi-prefix");
defn_cap!(SASL = "sasl");
defn_cap!(SERVER_TIME = "server-time");
defn_cap!(SETNAME = "setname");
defn_cap!(STANDARD_REPLIES = "standard-replies");
defn_cap!(USERHOST_IN_NAMES = "userhost-in-names");

impl TagWithValue<Cap> for SASL {
    type Value<'a> = BTreeSet<Word<'a>>;

    fn from_union<'a>(
        input: &<Cap as super::TagClass>::Union<'a>,
    ) -> Result<Self::Value<'a>, crate::error::ParseError> {
        use crate::string::tf::AsciiCasemap;
        let (_, mechs_raw) = input;
        let mut splitter = Splitter::new(mechs_raw.clone());
        let mut names = BTreeSet::new();
        while !splitter.is_empty() {
            let mut name = splitter.save_end().until_byte(b',').rest_or_default::<Word>();
            if !name.is_empty() {
                name.transform(AsciiCasemap::<true>);
                names.insert(name);
            }
            splitter.next_byte();
        }
        Ok(names)
    }
}

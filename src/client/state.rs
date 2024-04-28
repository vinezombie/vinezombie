//! Common elements of client state.

use crate::{
    ircmsg::Source,
    names::{Cap, NameMap},
    string::Arg,
};
use std::any::Any;

/// Keys for client state.
pub trait ClientStateKey: Default + Any {
    /// The type of data associated with this key.
    type Value: Any + Send + Sync;
}

macro_rules! csk {
    ($name:ident: $inner:ty = $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
        pub struct $name;

        impl ClientStateKey for $name {
            type Value = $inner;
        }
    };
}

csk!(ClientSource: Source<'static> = "The client's (presumed) source string.");
csk!(ServerSource: Source<'static> = "The server's source.");
csk!(Caps: NameMap<Cap, bool> = "Map of the server's capabilities to whether they're enabled.");
csk!(ISupport: NameMap<crate::names::ISupport> = "The server's ISUPPORT tokens.");
csk!(ServerVersion: Arg<'static> = "The client's source.");
csk!(Account: Option<Arg<'static>> = "The client's source.");

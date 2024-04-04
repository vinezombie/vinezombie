//! Client and server commands.

use super::{ClientMsgKind, Name, NameValued, ServerMsgKind};
use crate::ircmsg::{ClientMsg, ServerMsg, ServerMsgKindRaw, TargetedMsg};
use crate::string::{Bytes, Cmd, Line};

macro_rules! defn_cmd {
    ($cmd:ident) => {
        #[doc = concat!("The `", stringify!($cmd), "` message type.")]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
        pub struct $cmd;
        impl $cmd {
            /// The value `self` stands in for as a [`Cmd`].
            #[allow(clippy::declare_interior_mutable_const)]
            pub const CMD: Cmd<'static> =
                unsafe { Cmd::from_unchecked(Bytes::from_str(stringify!($cmd))) };
            /// Returns a reference to a static [`Cmd`] representing `self`'s value.
            pub fn as_cmd<'a>(&self) -> &'static Cmd<'a> {
                static VALUE: Cmd<'static> = $cmd::CMD;
                &VALUE
            }
        }
        impl std::fmt::Display for $cmd {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                stringify!($cmd).fmt(f)
            }
        }
        impl std::hash::Hash for $cmd {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.as_cmd().hash(state)
            }
        }
        impl<'a> From<$cmd> for Cmd<'a> {
            fn from(v: $cmd) -> Cmd<'a> {
                v.as_cmd().clone()
            }
        }
        impl<'a> PartialEq<Cmd<'a>> for $cmd {
            fn eq(&self, other: &Cmd<'a>) -> bool {
                *self.as_cmd() == *other
            }
        }
        impl<'a> PartialEq<$cmd> for Cmd<'a> {
            fn eq(&self, other: &$cmd) -> bool {
                *other == *self
            }
        }
        impl<'a> std::borrow::Borrow<Cmd<'a>> for $cmd {
            fn borrow(&self) -> &Cmd<'a> {
                self.as_cmd()
            }
        }
    };
}

macro_rules! impl_cmd_client {
    ($cmd:ident) => {
        impl Name<ClientMsgKind> for $cmd {
            fn as_raw(&self) -> &'static Cmd<'static> {
                self.as_cmd()
            }
        }
    };
}

macro_rules! impl_cmd_server {
    ($cmd:ident) => {
        impl $cmd {
            /// The value `self` stands in for as a [`ServerMsgKindRaw`].
            #[allow(clippy::declare_interior_mutable_const)]
            pub const KIND: ServerMsgKindRaw<'static> = ServerMsgKindRaw::Cmd(unsafe {
                Cmd::from_unchecked(Bytes::from_str(stringify!($cmd)))
            });
            /// Returns a reference to a static [`ServerMsgKindRaw`] representing `self`'s value.
            pub fn as_kind<'a>(&self) -> &'static ServerMsgKindRaw<'a> {
                static VALUE: ServerMsgKindRaw<'static> = $cmd::KIND;
                &VALUE
            }
        }
        impl<'a> std::borrow::Borrow<ServerMsgKindRaw<'a>> for $cmd {
            fn borrow(&self) -> &ServerMsgKindRaw<'a> {
                self.as_kind()
            }
        }
        impl Name<ServerMsgKind> for $cmd {
            #[allow(clippy::declare_interior_mutable_const)]
            fn as_raw(&self) -> &'static ServerMsgKindRaw<'static> {
                self.as_kind()
            }
        }
        impl From<$cmd> for ServerMsgKindRaw<'static> {
            fn from(v: $cmd) -> ServerMsgKindRaw<'static> {
                v.as_kind().clone()
            }
        }
        impl<'a> PartialEq<ServerMsgKindRaw<'a>> for $cmd {
            fn eq(&self, other: &ServerMsgKindRaw<'a>) -> bool {
                *self.as_kind() == *other
            }
        }
        impl<'a> PartialEq<$cmd> for ServerMsgKindRaw<'a> {
            fn eq(&self, other: &$cmd) -> bool {
                *other == *self
            }
        }
    };
}

macro_rules! defn_cmd_client {
    ($($cmd:ident)+) => {
        $(
            defn_cmd!($cmd);
            impl_cmd_client!($cmd);
        )+
    }
}

macro_rules! defn_cmd_server {
    ($($cmd:ident)+) => {
        $(
            defn_cmd!($cmd);
            impl_cmd_server!($cmd);
        )+
    }
}

macro_rules! defn_cmd_both {
    ($($cmd:ident)+) => {
        $(
            defn_cmd!($cmd);
            impl_cmd_client!($cmd);
            impl_cmd_server!($cmd);
        )+
    }
}

defn_cmd_client! {
    ACCEPT
    ADMIN
    CHALLENGE
    CHATHISTORY
    HELP
    INFO
    KILL
    KNOCK
    LINKS
    LIST
    LUSERS
    MONITOR
    MAP
    MOTD
    NAMES
    OPER
    PASS
    STATS
    TIME
    USER
    USERHOST
    VERSION
    WEBIRC
    WHO
    WHOIS
    WHOWAS
}

defn_cmd_server! {
    ACCOUNT
    CHGHOST
    ERROR
    FAIL
    NOTE
    WARN
}

defn_cmd_both! {
    AUTHENTICATE
    AWAY
    BATCH
    CAP
    INVITE
    JOIN
    KICK
    MODE
    NICK
    NOTICE
    PART
    PING
    PONG
    PRIVMSG
    QUIT
    REGISTER
    SETNAME
    TAGMSG
    TOPIC
    VERIFY
    WALLOPS
}

macro_rules! basic_unary {
    ($name:ident: $target_pat:pat => $target_val:expr) => {
        impl NameValued<ServerMsgKind> for $name {
            type Value<'a> = TargetedMsg<'a, Line<'a>>;

            fn from_union<'a>(
                input: &<ServerMsgKind as super::NameClass>::Union<'a>,
            ) -> Result<Self::Value<'a>, crate::error::ParseError> {
                let ServerMsg { tags, source, args, .. } = input;
                let ($target_pat, Some(value)) = args.split_last() else {
                    return Err(crate::error::ParseError::InvalidField(
                        concat!(stringify!($name), " args").into(),
                        "invalid arguments".into(),
                    ));
                };
                Ok(TargetedMsg {
                    tags: tags.clone(),
                    source: source.clone(),
                    target: $target_val,
                    value: value.clone(),
                })
            }
        }
        impl NameValued<ClientMsgKind> for $name {
            type Value<'a> = TargetedMsg<'a, Line<'a>>;

            fn from_union<'a>(
                input: &<ClientMsgKind as super::NameClass>::Union<'a>,
            ) -> Result<Self::Value<'a>, crate::error::ParseError> {
                let ClientMsg { tags, args, .. } = input;
                let ($target_pat, Some(value)) = args.split_last() else {
                    return Err(crate::error::ParseError::InvalidField(
                        concat!(stringify!($name), " args").into(),
                        "invalid arguments".into(),
                    ));
                };
                Ok(TargetedMsg {
                    tags: tags.clone(),
                    source: None,
                    target: $target_val,
                    value: value.clone(),
                })
            }
        }
    };
}

basic_unary!(NOTICE: [target] => target.clone());
basic_unary!(PART: [target] => target.clone());
basic_unary!(PRIVMSG: [target] => target.clone());
basic_unary!(QUIT: [] => crate::names::STAR.into());
basic_unary!(TOPIC: [target] => target.clone());
basic_unary!(WALLOPS: [] => crate::names::STAR.into());

impl NameValued<ServerMsgKind> for JOIN {
    type Value<'a> = TargetedMsg<'a, ()>;

    fn from_union<'a>(
        input: &<ServerMsgKind as super::NameClass>::Union<'a>,
    ) -> Result<Self::Value<'a>, crate::error::ParseError> {
        let ServerMsg { tags, source, args, .. } = input;
        let Some([target]) = args.all() else {
            return Err(crate::error::ParseError::InvalidField(
                concat!(stringify!($name), " args").into(),
                "invalid arguments".into(),
            ));
        };
        Ok(TargetedMsg {
            tags: tags.clone(),
            source: source.clone(),
            target: target.clone(),
            value: (),
        })
    }
}

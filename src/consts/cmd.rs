use super::{ClientMsgKind, ServerMsgKind, Tag};
use crate::ircmsg::ServerMsgKindRaw;
use crate::string::{Bytes, Cmd};

macro_rules! defn_cmd {
    ($cmd:ident) => {
        #[doc = concat!("The `", stringify!($cmd), "` message type.")]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
        pub struct $cmd;
        impl $cmd {
            /// Returns a reference to a static [`Cmd`] representing `self`'s value.
            pub fn as_cmd<'a>(&self) -> &'static Cmd<'a> {
                static VALUE: Cmd<'static> =
                    unsafe { Cmd::from_unchecked(Bytes::from_str(stringify!($cmd))) };
                &VALUE
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
        impl Tag<ClientMsgKind> for $cmd {
            #[allow(clippy::declare_interior_mutable_const)]
            const RAW: Cmd<'static> =
                unsafe { Cmd::from_unchecked(Bytes::from_str(stringify!($cmd))) };
        }
    };
}

macro_rules! impl_cmd_server {
    ($cmd:ident) => {
        impl $cmd {
            /// Returns a reference to a static [`ServerMsgKindRaw`] representing `self`'s value.
            pub fn as_kind<'a>(&self) -> &'static ServerMsgKindRaw<'a> {
                static VALUE: ServerMsgKindRaw<'static> = ServerMsgKindRaw::Cmd(unsafe {
                    Cmd::from_unchecked(Bytes::from_str(stringify!($cmd)))
                });
                &VALUE
            }
        }
        impl<'a> std::borrow::Borrow<ServerMsgKindRaw<'a>> for $cmd {
            fn borrow(&self) -> &ServerMsgKindRaw<'a> {
                self.as_kind()
            }
        }
        impl Tag<ServerMsgKind> for $cmd {
            #[allow(clippy::declare_interior_mutable_const)]
            const RAW: ServerMsgKindRaw<'static> = unsafe {
                ServerMsgKindRaw::Cmd(Cmd::from_unchecked(Bytes::from_str(stringify!($cmd))))
            };
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

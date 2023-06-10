use crate::string::{Bytes, Cmd};

macro_rules! defn_cmd {
    ($($cmd:ident)+) => {
        $(
            #[allow(missing_docs)]
            #[allow(clippy::declare_interior_mutable_const)]
            pub const $cmd: Cmd<'static> = unsafe {
                Cmd::from_unchecked(Bytes::from_str(stringify!($cmd)))
            };
        )+
    }
}

defn_cmd! {
    ACCEPT
    ACCOUNT
    ADMIN
    AUTHENTICATE
    AWAY
    CAP
    CHALLENGE
    CHATHISTORY
    CHGHOST
    CONNECT
    ERROR
    HELP
    INFO
    INVITE
    JOIN
    KICK
    KILL
    LINKS
    LIST
    LUSERS
    MODE
    MONITOR
    MOTD
    NAMES
    NICK
    NOTICE
    OPER
    PART
    PASS
    PING
    PONG
    PRIVMSG
    QUIT
    REGISTER
    REHASH
    RESTART
    SETNAME
    SQUIT
    STATS
    TAGMSG
    TIME
    TOPIC
    USER
    USERHOST
    VERIFY
    VERSION
    WALLOPS
    WATCH
    WEBIRC
    WHO
    WHOIS
    WHOWAS
}

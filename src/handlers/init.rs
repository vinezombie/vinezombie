//! Handlers for connection registration and state-tracking initialization.

use crate::{
    init::nick::NickGen,
    known::{Cap, Cmd, MaybeKnown, Numeric},
    msg::{
        data::{CapClientData, CapServerData, Ping},
        ClientMsg, RawClientMsg, RawServerMsg,
    },
    IoResult, IrcStr, IrcWord,
};
use graveseed::handler::{Action, Handler, IntoHandler};
use std::{
    collections::BTreeSet,
    io::ErrorKind,
    time::{Duration, Instant},
};

/// [`IntoHandler`] for [`Register`].
#[derive(Clone, Debug)]
pub struct RegisterBuilder<N> {
    username: IrcWord<'static>,
    realname: IrcStr<'static>,
    nick: IrcWord<'static>,
    fallbacks: N,
    caps: BTreeSet<IrcWord<'static>>,
    timeout: Option<Duration>, // TODO: server password, sasl.
}

impl<N> RegisterBuilder<N> {
    /// Creates a new `RegisterBuilder` from an [`init::Register`][crate::init::Register].
    ///
    /// Returns `None` if the nick generator returns `None`.
    /// This should usually be treated as end user error.
    pub fn from_opts<G: NickGen<Iter = N>, S: crate::sasl::Secret>(
        opts: crate::init::Register<G, S>,
    ) -> Option<Self> {
        let new = Self::new(opts.username, opts.realname, opts.nicks)?;
        // TODO: SASL.
        Some(new)
    }
    /// Creates a new `RegisterBuilder`.
    ///
    /// Specifies a timeout of 10s by default, which should be considerably longer
    /// than is necessary for a server to fail rDNS and ident lookup
    /// under typical conditions.
    ///
    /// Returns `None` if `nicks.nick_gen()` returns `None`.
    /// This should usually be treated as end user error.
    pub fn new(
        username: impl Into<IrcWord<'static>>,
        realname: impl Into<IrcStr<'static>>,
        nicks: impl NickGen<Iter = N>,
    ) -> Option<Self> {
        let (nick, fallbacks) = nicks.nick_gen()?;
        Some(RegisterBuilder {
            username: username.into(),
            realname: realname.into(),
            nick,
            fallbacks,
            caps: BTreeSet::new(),
            timeout: Some(Duration::from_secs(10)),
        })
    }
    /// Adds a capability to request in addition to any capabilities implicitly required.
    pub fn add_cap(&mut self, cap: impl Into<MaybeKnown<'static, Cap>>) -> &mut Self {
        self.caps.insert(cap.into().into_word());
        self
    }
    /// Sets the timeout.
    ///
    /// The timeout is the maximum amount of time that may pass between messages
    /// before the handler simply times out.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.timeout = timeout;
        self
    }
    // TODO: SASL.
}

/// Success! Start normal message processing.
///
/// The included message will always be RPL_WELCOME.
pub struct Success<'a> {
    // TODO: Rich RPL_WELCOME.
    /// The RPL_WELCOME message.
    pub welcome: RawServerMsg<'a>,
    /// The connection state.
    pub state: crate::state::Connection,
}

/// Handler that performs connection registration.
#[derive(Clone)]
pub struct Register<N> {
    needs_cap_end: bool,
    userinfo: Option<(IrcWord<'static>, IrcStr<'static>)>,
    nick_fallbacks: N,
    conn_state: crate::state::Connection,
    caps: BTreeSet<IrcWord<'static>>,
    timeout: Option<(Duration, Instant)>, // TODO: server password, sasl
}

impl<N: Iterator<Item = IrcWord<'static>> + 'static> Register<N> {
    fn handle_msg<'a>(
        mut self: Box<Self>,
        msg: &RawServerMsg<'a>,
        mut send: super::Send<'static>,
    ) -> Action<IoResult<Success<'a>>, RawServerMsg<'a>, RawClientMsg<'static>> {
        let mut can_cap_end = false;
        if msg.kind().as_known_into() == Some(Numeric::RplWelcome) {
            // Handle successful connection.
            // TODO: Probably abort if SASL is required and didn't happen.
            let success = Success { welcome: msg.clone(), state: self.conn_state };
            return Action::ok(success).with_send(send);
        } else if msg.kind().as_known_into() == Some(Numeric::ErrNicknameinuse) {
            // Handle the nick being in use.
            if let Some(new_nick) = self.nick_fallbacks.next() {
                self.conn_state.nick = new_nick.clone();
                // TODO: NICK message data type.
                let mut msg_n = ClientMsg::new_raw(Cmd::Nick);
                msg_n.data_mut().args.add_word(new_nick);
                send = send.with_msg(msg_n);
            } else {
                return Action::io_err(ErrorKind::AlreadyExists, "all nicknames are in use");
            }
        } else if let Some(msg) = msg.to_parsed::<Ping>(()) {
            // Handle pings.
            send = send.with_msg(ClientMsg::new(msg.data().pong()))
        } else if let Some(msg) = msg.to_parsed::<CapServerData>(()) {
            // Handle CAP messages.
            let caps = msg.into_parts().1;
            self.conn_state.update_caps(caps.caps);
            if caps.is_last {
                if !self.caps.is_empty() {
                    let req = CapClientData::req(std::mem::take(&mut self.caps)).1;
                    send = send.with_msg(ClientMsg::new(req));
                } else {
                    // TODO: SASL!!!
                    can_cap_end = true;
                }
            }
        }
        // Ignore messages that the client doesn't know how to handle
        // and just let the timeout kill the connection.
        if can_cap_end && self.needs_cap_end {
            send = send.with_msg(ClientMsg::new(CapClientData::End));
        }
        Action::next(self).with_send(send)
    }
}

// TODO: Definitely need macros for these horrific type signatures.

impl<'a, N: Iterator<Item = IrcWord<'static>> + 'static>
    Handler<IoResult<Success<'a>>, RawServerMsg<'a>, RawClientMsg<'static>> for Register<N>
{
    fn handle(
        mut self: Box<Self>,
        msg: Option<&RawServerMsg<'a>>,
    ) -> Action<IoResult<Success<'a>>, RawServerMsg<'a>, RawClientMsg<'static>> {
        let send = if let Some((uname, rname)) = std::mem::take(&mut self.userinfo) {
            self.needs_cap_end = !self.caps.is_empty();
            register_burst(self.needs_cap_end, uname, rname, self.conn_state.nick.clone())
        } else {
            super::Send::default()
        };
        // Handle the inbound message or lack thereof.
        if let Some(msg) = msg {
            // Update the deadline.
            if let Some((timeout, deadline)) = &mut self.timeout {
                *deadline = Instant::now() + *timeout;
            }
            return self.handle_msg(msg, send);
        } else if let Some((_, deadline)) = self.timeout {
            if deadline < Instant::now() {
                return Action::io_err(ErrorKind::TimedOut, "connection registration timed out");
            }
        }
        Action::next(self).with_send(send)
    }

    fn timeout(&self) -> Option<std::time::Duration> {
        if self.userinfo.is_some() {
            Some(std::time::Duration::ZERO)
        } else {
            self.timeout.map(|(_, i)| i.saturating_duration_since(Instant::now()))
        }
    }
}

impl<'a, N: Iterator<Item = IrcWord<'static>> + 'static>
    IntoHandler<IoResult<Success<'a>>, RawServerMsg<'a>, RawClientMsg<'static>>
    for RegisterBuilder<N>
{
    fn into_handler(
        self,
    ) -> Box<dyn Handler<IoResult<Success<'a>>, RawServerMsg<'a>, RawClientMsg<'static>>> {
        Box::new(Register {
            needs_cap_end: false,
            userinfo: Some((self.username, self.realname)),
            conn_state: crate::state::Connection::new(self.nick, None),
            nick_fallbacks: self.fallbacks,
            caps: self.caps,
            timeout: None,
        })
    }
}

/// Creates a [`Send`][super::Send] with the initial messages needed to register a user connection.
pub fn register_burst(
    cap: bool,
    uname: IrcWord<'static>,
    rname: IrcStr<'static>,
    nick: IrcWord<'static>,
) -> super::Send<'static> {
    let mut send = super::Send::default();
    if cap {
        let msg = ClientMsg::new(CapClientData::Ls);
        send = send.with_msg(msg);
    }
    // TODO: USER message data type.
    let mut msg_u = ClientMsg::new_raw(Cmd::User);
    let args = &mut msg_u.data_mut().args;
    args.add_word(uname);
    // Some IRCds still rely on 8 to set +i by default.
    args.add_word("8");
    args.add_word("*");
    args.add(rname);
    // TODO: NICK message data type.
    let mut msg_n = ClientMsg::new_raw(Cmd::Nick);
    msg_n.data_mut().args.add_word(nick);
    send.with_msg(msg_u).with_msg(msg_n)
}

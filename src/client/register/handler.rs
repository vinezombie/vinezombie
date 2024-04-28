use std::collections::{BTreeMap, BTreeSet};

use super::CapFn;
use crate::{
    client::{
        auth::{self, SaslQueue},
        nick::NickGen,
        ClientMsgSink,
    },
    ircmsg::{ClientMsg, ServerMsg, SharedSource, Source, UserHost},
    names::{
        cmd::{CAP, NICK},
        Cap, ISupport, NameMap,
    },
    string::{Arg, Key, Line, Nick, Splitter, Word},
};

/// A useful subset of information yielded by client registration.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Registration {
    /// The nickname used for this connection.
    pub nick: Nick<'static>,
    /// The user and hostname used for this connection.
    ///
    /// This field will usually not be set unless SASL is completed.
    /// It may contain a spoofed hostname if the server supports those.
    ///
    /// Some server software incorrectly reports the username for this field,
    /// omitting a leading "~" where one is otherwise required.
    /// Relying on the value of the username in this field is not recommended.
    pub userhost: Option<UserHost<'static>>,
    /// The name of logged-into account, if any.
    pub account: Option<Arg<'static>>,
    /// The source associated with the server you're connected to.
    pub source: Option<Source<'static>>,
    /// The capabilities, their values, and whether they are enabled.
    pub caps: NameMap<Cap, bool>,
    /// The server version string, if any.
    pub version: Option<Arg<'static>>,
    /// Information about the server.
    pub isupport: NameMap<ISupport>,
}

impl Registration {
    /// Creates a new [`Registration`] with the provided nick.
    pub fn new(nick: Nick<'static>) -> Self {
        Registration {
            nick,
            userhost: None,
            account: None,
            source: None,
            caps: NameMap::new(),
            version: None,
            isupport: NameMap::new(),
        }
    }
    /// Saves registration to a [`ClientState`][crate::client::ClientState].
    pub fn save(self, state: &mut crate::client::ClientState) {
        use crate::client::state::*;
        let source = Source { nick: self.nick, userhost: self.userhost };
        state.insert::<ClientSource>(source);
        state.insert::<Account>(self.account);
        state.insert::<Caps>(self.caps);
        state.insert::<ISupport>(self.isupport);
        if let Some(server_source) = self.source {
            state.insert::<ServerSource>(server_source);
        }
        if let Some(version) = self.version {
            state.insert::<ServerVersion>(version);
        }
    }
}

impl Registration {
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
}

impl Default for Registration {
    fn default() -> Self {
        Self::new(crate::names::STAR)
    }
}

/// All the possible errors that can occur during registration.
#[derive(Debug)]
pub enum HandlerError {
    /// Wrong server password, or we're banned.
    NoAccess(Line<'static>),
    /// No valid nicknames remaining.
    NoNicks,
    /// Authentication was required, but failed.
    NoLogin,
    /// We've been redirected to another server.
    Redirect(Word<'static>, u16, Line<'static>),
    /// The server sent a reply indicating an error that cannot be handled.
    ServerError(Box<ServerMsg<'static>>),
    /// The server sent an invalid message.
    Broken(Box<dyn std::error::Error + Send + Sync>),
    /// The following required capabilities are not present on the server.
    MissingCaps(BTreeSet<Key<'static>>),
}

impl HandlerError {
    pub(self) fn broken(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> HandlerError {
        HandlerError::Broken(e.into())
    }
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::NoAccess(l) => write!(f, "access denied: {l}"),
            HandlerError::NoNicks => write!(f, "no fallback nicks remaining"),
            HandlerError::NoLogin => write!(f, "failed to log in"),
            HandlerError::ServerError(e) => write!(f, "server error: {e}"),
            HandlerError::Broken(e) => write!(f, "invalid message: {e}"),
            HandlerError::Redirect(s, p, i) => write!(f, "redirected to {s}:{p}: {i}"),
            HandlerError::MissingCaps(c) => {
                let caps = c
                    .iter()
                    .map(|v| v.to_string())
                    .reduce(|mut a, b| {
                        a.push_str(", ");
                        a.push_str(b.as_str());
                        a
                    })
                    .unwrap_or_default();
                write!(f, "missing required capabilities: {caps}")
            }
        }
    }
}

impl std::error::Error for HandlerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let HandlerError::Broken(e) = self {
            Some(e.as_ref())
        } else {
            None
        }
    }
}

impl From<HandlerError> for std::io::Error {
    fn from(value: HandlerError) -> Self {
        use std::io::{Error, ErrorKind};
        match value {
            HandlerError::NoAccess(e) => {
                Error::new(ErrorKind::ConnectionRefused, HandlerError::NoAccess(e))
            }
            HandlerError::Broken(e) => Error::new(ErrorKind::InvalidData, e),
            v => Error::new(ErrorKind::Other, v),
        }
    }
}

#[derive(Default)]
pub(super) enum HandlerState {
    #[default]
    Broken,
    Req(Box<dyn CapFn>, SaslQueue),
    Ack(BTreeSet<Key<'static>>, SaslQueue),
    #[cfg(feature = "base64")]
    Sasl(crate::client::auth::Handler),
    CapEnd,
    AwaitWelcome,
    AwaitEnd,
}

impl HandlerState {
    /// Handle an ACK, NAK, or DEL.
    ///
    /// Also sends the initial AUTHENTICATE message for SASL.
    pub fn ack(
        &mut self,
        ack: bool,
        caps: &BTreeMap<Key<'_>, Word<'_>>,
        mut sink: impl ClientMsgSink<'static>,
    ) -> Result<(), HandlerError> {
        if let HandlerState::Ack(ackd, queue) = self {
            let caps = caps.keys().map(|k| k.clone().owning()).collect();
            if ack {
                *ackd = ackd.difference(&caps).cloned().collect();
            } else {
                let missing: BTreeSet<_> = ackd.intersection(&caps).cloned().collect();
                if !missing.is_empty() {
                    // Ooops. If we're here, the server lied to us about what it supports.
                    return Err(HandlerError::MissingCaps(missing));
                }
            };
            if ackd.is_empty() {
                #[cfg(feature = "base64")]
                if let Some(handler) = auth::Handler::from_queue(std::mem::take(queue)) {
                    // If we're here, SASL was acked,
                    // as the queue was nonempty and we request "sasl" when so.
                    sink.send(handler.auth_msg());
                    *self = HandlerState::Sasl(handler);
                    return Ok(());
                }
                *self = HandlerState::CapEnd;
            }
        }
        Ok(())
    }
}

/// Connection registration logic.
pub struct Handler {
    pub(super) nicks: Option<Box<dyn NickGen>>,
    pub(super) state: HandlerState,
    pub(super) needs_auth: bool,
    pub(super) reg: Registration,
}

impl Handler {
    pub(super) fn new(
        nicks: (Nick<'static>, Option<Box<dyn NickGen>>),
        caps: Box<dyn CapFn>,
        needs_auth: bool,
        auths: SaslQueue,
    ) -> Self {
        let (nick, nicks) = nicks;
        Handler {
            nicks,
            state: HandlerState::Req(caps, auths),
            needs_auth,
            reg: Registration::new(nick),
        }
    }
    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        mut sink: impl ClientMsgSink<'static>,
    ) -> Result<Option<Registration>, HandlerError> {
        if self.reg.source.is_none() {
            self.reg.source = msg.source.clone().map(SharedSource::owning_merged);
        }
        if crate::client::handlers::pong(msg, sink.borrow_mut()) {
            return Ok(None);
        }
        // Ignore errors related to SASL.
        let mut ignore_sasl = false;
        #[cfg(feature = "base64")]
        if let HandlerState::Sasl(sasl) = &mut self.state {
            ignore_sasl = true;
            match sasl.handle(msg, sink.borrow_mut()) {
                Ok(false) => (),
                Ok(true) => {
                    self.state = HandlerState::CapEnd;
                }
                Err(_e) => {
                    // Auth failed irrecoverably.
                    // May still be able to continue depending on needs_auth.
                    #[cfg(feature = "tracing")]
                    tracing::error!("{_e}");
                    self.state = HandlerState::CapEnd;
                }
            }
        }
        let retval = match msg.kind.as_str() {
            "001" | "002" | "003" | "004" if self.needs_auth && self.reg.account.is_none() => {
                // We hit the end of registration without logging in. Bail!
                Err(HandlerError::NoLogin)
            }
            "001" => {
                let nick = msg
                    .args
                    .words()
                    .first()
                    .filter(|n| *n != crate::names::STAR.as_bytes())
                    .and_then(|n| Nick::from_super(n.clone().owning()).ok());
                if let Some(nick) = nick {
                    self.reg.nick = nick;
                }
                if let Some(source) = &msg.source {
                    use std::ops::Deref;
                    if !self.reg.source.as_ref().is_some_and(|src| src == source.deref()) {
                        self.reg.source = Some(source.clone().owning_merged());
                    }
                }
                self.state = HandlerState::AwaitEnd;
                Ok(None)
            }
            "004" if matches!(self.state, HandlerState::AwaitEnd) => {
                self.reg.parse_myinfo(msg.args.words());
                Ok(None)
            }
            "005" if matches!(self.state, HandlerState::AwaitEnd) => {
                let Some((_, isupports)) = msg.args.words().split_first() else {
                    // Bad ISUPPORT message, but let's be forgiving.
                    return Ok(None);
                };
                let mut ism = self.reg.isupport.edit();
                for isupport in isupports {
                    let mut splitter = Splitter::new(isupport.clone().owning());
                    let Ok(key) = splitter.string::<Key>(false) else {
                        continue;
                    };
                    let value: Word<'static> = if splitter.next_byte().is_some_and(|b| b != b'=') {
                        // Weirdness in an ISUPPORT tag. Bail.
                        // TODO: Log.
                        continue;
                    } else {
                        splitter.rest_or_default::<Word>()
                    };
                    ism.insert((key, value), ());
                }
                Ok(None)
            }
            "004" => {
                // We actually care about 001 because it's where we get some basic info.
                // and we'd rather non-compliant severs skip 004 in favor of 005.
                Err(HandlerError::Broken("004 sent before 001".into()))
            }
            "005" => {
                // We probably have an RFC2819 RPL_BOUNCE. Try parsing it.
                // Error either way.
                let Some(last) = msg.args.split_last().1 else {
                    return Err(HandlerError::Broken("empty 005 message".into()));
                };
                let split = || {
                    let mut splitter = last.splitn(2, |c| *c == b',');
                    let server = splitter.next()?.rsplit(|c| !c.is_ascii_graphic()).next()?;
                    let port = splitter.next()?.rsplit(|c| !c.is_ascii_digit()).next()?;
                    // We don't care about this being performant.
                    // This path is so cold it's arctic.
                    let server = Word::from_bytes(server).ok()?;
                    let port = std::str::from_utf8(port).ok()?.parse().ok()?;
                    Some((server, port))
                };
                if let Some((server, port)) = split() {
                    Err(HandlerError::Redirect(
                        server.clone().owning(),
                        port,
                        last.clone().owning(),
                    ))
                } else {
                    Err(HandlerError::ServerError(Box::new(msg.clone().owning())))
                }
            }
            "010" => {
                // We've been redirected.
                // This is also a very cold path.
                if let ([_, client, port], Some(info)) = msg.args.split_last() {
                    match port.to_utf8_lossy().parse() {
                        Ok(port) => Err(HandlerError::Redirect(
                            client.clone().owning().into(),
                            port,
                            info.clone().owning(),
                        )),
                        Err(e) => Err(HandlerError::Broken(
                            format!("not a valid port `{port}`: {e}").into(),
                        )),
                    }
                } else {
                    Err(HandlerError::ServerError(Box::new(msg.clone().owning())))
                }
            }
            "376" | "422" if matches!(self.state, HandlerState::AwaitEnd) => {
                // End of/no MOTD. We're done.
                Ok(Some(std::mem::take(&mut self.reg)))
            }
            "376" | "422" => {
                // If we're here, we did NOT see 004.
                Err(HandlerError::Broken("unexpected MOTD message".into()))
            }
            "432" => {
                // Invalid nick.
                let nicks = self.nicks.take().and_then(|ng| ng.handle_invalid(&self.reg.nick));
                self.nicks = nicks;
                self.next_nick(sink.borrow_mut())?;
                Ok(None)
            }
            "433" | "436" => {
                // Nick in use.
                self.next_nick(sink.borrow_mut())?;
                Ok(None)
            }
            "464" | "465" => {
                let line = msg.args.clone().owning().split_last().1.cloned().unwrap_or_default();
                Err(HandlerError::NoAccess(line))
            }
            "900" => {
                let args = msg.args.split_last().0;
                if let Some((account, args)) = args.split_last() {
                    self.reg.account = Some(account.clone().owning());
                    if let Some(whoami) = args.last() {
                        let whoami =
                            Source::parse(whoami.clone().owning()).map_err(HandlerError::broken)?;
                        self.reg.nick = whoami.nick;
                        self.reg.userhost = whoami.userhost;
                    }
                }
                Ok(None)
            }
            "901" => {
                self.reg.account = None;
                if let Some(whoami) = msg.args.clone().split_last().0.last() {
                    let whoami =
                        Source::parse(whoami.clone().owning()).map_err(HandlerError::broken)?;
                    self.reg.nick = whoami.nick;
                    self.reg.userhost = whoami.userhost;
                }
                Ok(None)
            }
            "902" | "904" | "905" | "906" | "907" if ignore_sasl => Ok(None),
            "CAP" => {
                use crate::client::cap;
                let cap_msg = cap::ServerMsgArgs::parse(&msg.args.clone().owning())
                    .map_err(HandlerError::broken)?;
                match cap_msg.subcmd {
                    cap::SubCmd::Ls if cap_msg.is_last => {
                        let mut caps = self.reg.caps.edit();
                        for (key, value) in cap_msg.caps {
                            caps.try_insert((key, value), false);
                        }
                        std::mem::drop(caps);
                        let state = std::mem::take(&mut self.state);
                        if let HandlerState::Req(reqs, mut auths) = state {
                            use crate::names::cap::{SASL, STS};
                            // Filter SASL mechanisms.
                            match self.reg.caps.get_parsed(SASL) {
                                Some(Ok(mechs)) => {
                                    auths.retain(&|mech| mechs.contains(mech.as_bytes()));
                                }
                                Some(_) => (),
                                None => auths.clear(),
                            }
                            // Check the set of capabilities.
                            let avail = self.reg.caps.keys().cloned().collect();
                            let mut reqs = reqs.require(&avail);
                            if !auths.is_empty() {
                                reqs.insert(SASL::NAME);
                            } else if self.needs_auth {
                                return Err(HandlerError::NoLogin);
                            }
                            let diff: BTreeSet<_> = reqs.difference(&avail).cloned().collect();
                            if !diff.is_empty() {
                                return Err(HandlerError::MissingCaps(diff));
                            }
                            // "sts" is purely informative and must never be requested.
                            reqs.remove(&STS::NAME);
                            self.state = if reqs.is_empty() {
                                HandlerState::CapEnd
                            } else {
                                cap::req(
                                    reqs.iter().cloned(),
                                    Some(self.reg.nick.clone().into_super()),
                                    self.reg.source.as_ref(),
                                    sink.borrow_mut(),
                                );
                                HandlerState::Ack(reqs, auths)
                            };
                        } else {
                            self.state = state;
                        }
                    }
                    cap::SubCmd::Ls | cap::SubCmd::New => {
                        let mut caps = self.reg.caps.edit();
                        for (key, value) in cap_msg.caps {
                            caps.try_insert((key, value), false);
                        }
                    }
                    cap::SubCmd::Ack => {
                        let mut caps = self.reg.caps.edit();
                        self.state.ack(true, &cap_msg.caps, sink.borrow_mut())?;
                        // Assume that every ACK is a positive ACK without actually checking.
                        for (key, value) in cap_msg.caps {
                            caps.insert_or_update((key, value), true);
                        }
                    }
                    cap::SubCmd::Nak => {
                        self.state.ack(false, &cap_msg.caps, sink.borrow_mut())?;
                    }
                    cap::SubCmd::Del => {
                        let mut caps = self.reg.caps.edit();
                        cap_msg.caps.keys().for_each(|cap| {
                            caps.remove_raw(cap);
                        });
                        self.state.ack(false, &cap_msg.caps, sink.borrow_mut())?;
                    }
                    cap::SubCmd::List => return Err(HandlerError::broken("unexpected CAP LIST")),
                }
                Ok(None)
            }
            _ => {
                if msg.kind.is_error() == Some(true) {
                    return Err(HandlerError::ServerError(Box::new(msg.clone().owning())));
                }
                Ok(None)
            }
        }?;
        if matches!(self.state, HandlerState::CapEnd) {
            if self.needs_auth && self.reg.account.is_none() {
                return Err(HandlerError::NoLogin);
            }
            let mut msg = crate::ircmsg::ClientMsg::new(CAP);
            msg.args.edit().add_literal("END");
            sink.send(msg);
            self.state = HandlerState::AwaitWelcome;
        }
        Ok(retval)
    }
    fn next_nick(&mut self, mut sink: impl ClientMsgSink<'static>) -> Result<(), HandlerError> {
        let Some(nicks) = self.nicks.take() else { return Err(HandlerError::NoNicks) };
        let (nick, nicks) = nicks.next_nick();
        let mut msg = ClientMsg::new(NICK);
        msg.args.edit().add_word(nick.clone());
        sink.send(msg);
        self.reg.nick = nick;
        self.nicks = nicks;
        Ok(())
    }
}

impl crate::client::Handler for Handler {
    type Value = Result<(), HandlerError>;

    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        state: &mut crate::client::ClientState,
        mut queue: crate::client::queue::QueueEditGuard<'_>,
        mut channel: crate::client::channel::SenderRef<'_, Self::Value>,
    ) -> bool {
        match self.handle(msg, &mut queue) {
            Ok(Some(v)) => {
                v.save(state);
                channel.send(Ok(()));
                true
            }
            Ok(None) => false,
            Err(e) => {
                channel.send(Err(e));
                true
            }
        }
    }

    fn wants_owning(&self) -> bool {
        matches!(self.state, HandlerState::AwaitWelcome)
    }
}

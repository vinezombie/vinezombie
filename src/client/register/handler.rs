use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    client::{auth::SaslLogic, nick::NickGen, ClientMsgSink},
    ircmsg::{ClientMsg, ServerMsg, SharedSource, Source, UserHost},
    names::{
        cmd::{CAP, NICK},
        Cap, ISupport, NameMap,
    },
    string::{Arg, Key, Line, Nick, Splitter, Word},
};

/// The result of successful registration.
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

pub(super) enum HandlerState {
    Req(BTreeSet<Key<'static>>),
    Ack(VecDeque<Key<'static>>),
    Sasl,
    CapEnd,
    AwaitWelcome,
    AwaitEnd,
}

impl HandlerState {
    pub fn ack(&mut self, caps: &BTreeMap<Key<'_>, Word<'_>>) {
        if let HandlerState::Ack(ackd) = self {
            ackd.retain(|cap| caps.get(cap).is_none());
            if ackd.is_empty() {
                *self = HandlerState::Sasl;
            }
        }
    }
}

/// Connection registration logic.
pub struct Handler {
    pub(super) nicks: Option<Box<dyn NickGen>>,
    pub(super) state: HandlerState,
    pub(super) needs_auth: bool,
    #[cfg(feature = "base64")]
    pub(super) auth: Option<crate::client::auth::Handler>,
    pub(super) auths: VecDeque<(Arg<'static>, Box<dyn SaslLogic>)>,
    pub(super) reg: Registration,
}

impl Handler {
    pub(super) fn new(
        nicks: (Nick<'static>, Option<Box<dyn NickGen>>),
        caps: BTreeSet<Key<'static>>,
        needs_auth: bool,
        auths: VecDeque<(Arg<'static>, Box<dyn SaslLogic>)>,
    ) -> Self {
        let (nick, nicks) = nicks;
        Handler {
            nicks,
            state: HandlerState::Req(caps),
            needs_auth,
            #[cfg(feature = "base64")]
            auth: None,
            auths,
            reg: Registration::new(nick),
        }
    }
    /// Handles a server message sent during connection registration.
    ///
    /// It is a logic error to call `handle` after
    /// it errors or returns `Ok(Done)`.
    pub fn handle(
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
        #[cfg(feature = "base64")]
        if let Some(auth) = &mut self.auth {
            use crate::client::auth;
            match auth.handle(msg, sink.borrow_mut()) {
                Ok(true) => {
                    self.auth = None;
                    self.state = HandlerState::CapEnd;
                }
                Ok(false) => (),
                Err(auth::HandlerError::Fail(_)) => {
                    // TODO: Probably should log the failure.
                    self.state = HandlerState::CapEnd;
                    self.next_sasl(sink.borrow_mut())?;
                    self.cap_end(sink.borrow_mut())?;
                    return Ok(None);
                }
                Err(auth::HandlerError::WrongMechanism(set)) => {
                    self.auths.retain(|(k, _)| set.contains(k));
                    self.state = HandlerState::CapEnd;
                    self.next_sasl(sink.borrow_mut())?;
                    self.cap_end(sink.borrow_mut())?;
                    return Ok(None);
                }
                Err(auth::HandlerError::Broken(_)) => {
                    // TODO: Probably should log the breakage.
                    sink.send(crate::client::auth::msg_abort());
                    self.state = HandlerState::CapEnd;
                    self.next_sasl(sink.borrow_mut())?;
                    self.cap_end(sink.borrow_mut())?;
                    return Ok(None);
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
                        if let HandlerState::Req(reqs) = &mut self.state {
                            let mut reqs = std::mem::take(reqs);
                            reqs.retain(|key| self.reg.caps.get_extra_raw(key).is_some());
                            self.state = if reqs.is_empty() {
                                HandlerState::CapEnd
                            } else {
                                cap::req(
                                    reqs.iter().cloned(),
                                    Some(self.reg.nick.clone().into_super()),
                                    self.reg.source.as_ref(),
                                    sink.borrow_mut(),
                                );
                                HandlerState::Ack(reqs.into_iter().collect())
                            };
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
                        self.state.ack(&cap_msg.caps);
                        // Assume that every ACK is a positive ACK without actually checking.
                        for (key, value) in cap_msg.caps {
                            caps.insert_or_update((key, value), true);
                        }
                    }
                    cap::SubCmd::Nak => {
                        self.state.ack(&cap_msg.caps);
                    }
                    cap::SubCmd::Del => {
                        let mut caps = self.reg.caps.edit();
                        for cap in cap_msg.caps.keys() {
                            caps.remove_raw(cap);
                        }
                        self.state.ack(&cap_msg.caps);
                    }
                    cap::SubCmd::List => return Err(HandlerError::broken("unexpected CAP LIST")),
                }
                if matches!(self.state, HandlerState::Sasl) {
                    if let Some(Ok(names)) = self.reg.caps.get_parsed(crate::names::cap::SASL) {
                        if !names.is_empty() {
                            self.auths.retain(|(name, _)| names.contains(name.as_bytes()));
                        }
                    } else {
                        self.auths.clear();
                    }
                    self.state = HandlerState::CapEnd;
                    #[cfg(feature = "base64")]
                    self.next_sasl(sink.borrow_mut())?;
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
        self.cap_end(sink)?;
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
    #[cfg(feature = "base64")]
    fn next_sasl(&mut self, mut sink: impl ClientMsgSink<'static>) -> Result<(), HandlerError> {
        use crate::names::cmd::AUTHENTICATE;
        let Some((name, logic)) = self.auths.pop_front() else {
            return Ok(());
        };
        let mut msg = ClientMsg::new(AUTHENTICATE);
        msg.args.edit().add_word(name);
        sink.send(msg);
        self.auth = Some(crate::client::auth::Handler::from_logic(logic));
        self.state = HandlerState::Sasl;
        Ok(())
    }
    fn cap_end(&mut self, mut sink: impl ClientMsgSink<'static>) -> Result<bool, HandlerError> {
        if matches!(self.state, HandlerState::CapEnd) {
            if self.needs_auth && self.reg.account.is_none() {
                return Err(HandlerError::NoLogin);
            }
            let mut msg = crate::ircmsg::ClientMsg::new(CAP);
            msg.args.edit().add_literal("END");
            sink.send(msg);
            self.state = HandlerState::AwaitWelcome;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl crate::client::Handler for Handler {
    type Value = Result<Registration, HandlerError>;

    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        mut queue: crate::client::QueueEditGuard<'_>,
        mut channel: crate::client::channel::SenderRef<'_, Self::Value>,
    ) -> bool {
        match self.handle(msg, &mut queue) {
            Ok(Some(v)) => {
                channel.send(Ok(v));
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

use super::FallbackNicks;
use crate::{
    client::{auth::SaslLogic, nick::NickTransformer, ClientMsgSink, HandlerOk, HandlerResult},
    consts::cmd::{CAP, NICK},
    ircmsg::{Args, ClientMsg, ServerMsg, SharedSource, Source},
    string::{Arg, Host, Key, Line, Nick, Word},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The result of successful registration.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Registration {
    /// The nickname used for this connection.
    pub nick: Nick<'static>,
    /// The hostname used for this connection.
    ///
    /// This field will usually not be set unless SASL is completed.
    /// It may contain a spoofed hostname if the server supports those.
    pub host: Option<Host<'static>>,
    /// The name of logged-into account, if any.
    pub account: Option<Arg<'static>>,
    /// The enabled capabilities and their values.
    pub caps: BTreeMap<Key<'static>, Word<'static>>,
    /// The source associated with the server you're connected to.
    pub source: Option<Source<'static>>,
    /// The arguments of the welcome (001) message.
    pub welcome: Args<'static>,
}

impl Default for Registration {
    fn default() -> Self {
        Self {
            nick: crate::consts::STAR,
            host: None,
            account: None,
            caps: BTreeMap::new(),
            source: None,
            welcome: Args::default(),
        }
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
    /// The server sent a reply indicating an error that cannot be handled.
    ServerError(Box<ServerMsg<'static>>),
    /// An I/O error occurred.
    Io(std::io::Error),
}

impl HandlerError {
    pub(crate) fn broken(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        HandlerError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::NoAccess(l) => write!(f, "access denied: {l}"),
            HandlerError::NoNicks => write!(f, "no fallback nicks remaining"),
            HandlerError::NoLogin => write!(f, "failed to log in"),
            HandlerError::ServerError(e) => write!(f, "server error: {e}"),
            HandlerError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for HandlerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let HandlerError::Io(io) = self {
            Some(io)
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
            HandlerError::Io(io) => io,
            v => Error::new(ErrorKind::Other, v),
        }
    }
}
impl From<std::io::Error> for HandlerError {
    fn from(value: std::io::Error) -> Self {
        HandlerError::Io(value)
    }
}

pub(super) enum HandlerState {
    Req(BTreeSet<Key<'static>>),
    Ack(BTreeSet<Key<'static>>),
    Sasl,
    CapEnd,
    Done,
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
pub struct Handler<N1: NickTransformer, N2: NickTransformer + 'static> {
    pub(super) nicks: FallbackNicks<N1, N2>,
    pub(super) state: HandlerState,
    pub(super) needs_auth: bool,
    pub(super) caps_avail: BTreeMap<Key<'static>, Word<'static>>,
    #[cfg(feature = "base64")]
    pub(super) auth: Option<crate::client::auth::Handler>,
    pub(super) auths: VecDeque<(Arg<'static>, Box<dyn SaslLogic>)>,
    pub(super) reg: Registration,
}

impl<N1: NickTransformer, N2: NickTransformer + 'static> Handler<N1, N2> {
    /// Handles a server message sent during connection registration.
    ///
    /// It is a logic error to call `handle` after
    /// it errors or returns `Ok(Done)`.
    pub fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        mut sink: impl ClientMsgSink<'static>,
    ) -> HandlerResult<Registration, Line<'static>, HandlerError> {
        if self.reg.source.is_none() {
            self.reg.source = msg.source.clone().map(SharedSource::owning_merged);
        }
        if let Some(pong) = crate::client::pong(msg) {
            sink.send(pong).map_err(HandlerError::Io)?;
            return Ok(HandlerOk::NeedMore);
        }
        #[cfg(feature = "base64")]
        if let Some(auth) = &mut self.auth {
            use crate::client::auth;
            match auth.handle(msg, sink.borrow_mut()) {
                Ok(HandlerOk::Value(_)) => {
                    self.auth = None;
                    self.state = HandlerState::CapEnd;
                }
                Ok(HandlerOk::Ignored) => (),
                Ok(_) => return Ok(HandlerOk::NeedMore),
                Err(auth::HandlerError::Fail(_)) => {
                    // TODO: Probably should log the failure.
                    self.state = HandlerState::CapEnd;
                    self.next_sasl(sink.borrow_mut())?;
                    self.cap_end(sink.borrow_mut())?;
                    return Ok(HandlerOk::NeedMore);
                }
                Err(auth::HandlerError::WrongMechanism(set)) => {
                    self.auths.retain(|(k, _)| set.contains(k));
                    self.state = HandlerState::CapEnd;
                    self.next_sasl(sink.borrow_mut())?;
                    self.cap_end(sink.borrow_mut())?;
                    return Ok(HandlerOk::NeedMore);
                }
                Err(auth::HandlerError::Broken(_)) => {
                    // TODO: Probably should log the breakage.
                    sink.send(crate::client::auth::msg_abort()).map_err(HandlerError::Io)?;
                    self.state = HandlerState::CapEnd;
                    self.next_sasl(sink.borrow_mut())?;
                    self.cap_end(sink.borrow_mut())?;
                    return Ok(HandlerOk::NeedMore);
                }
                Err(auth::HandlerError::Io(e)) => return Err(HandlerError::Io(e)),
            }
        }
        let retval = match msg.kind.as_str() {
            "001" if self.needs_auth && self.reg.account.is_none() => Err(HandlerError::NoLogin),
            "001" => {
                let nick = msg
                    .args
                    .words()
                    .first()
                    .filter(|n| *n != crate::consts::STAR.as_bytes())
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
                self.reg.welcome = msg.args.clone().owning();
                Ok(HandlerOk::Value(std::mem::take(&mut self.reg)))
            }
            "432" => {
                // Invalid nick. Keep trying user nicks,
                // but don't allow auto-generated fallbacks.
                if self.nicks.has_user_nicks() {
                    self.next_nick(sink.borrow_mut())?;
                    Ok(HandlerOk::NeedMore)
                } else {
                    Err(HandlerError::NoNicks)
                }
            }
            "433" | "436" => {
                self.next_nick(sink.borrow_mut())?;
                Ok(HandlerOk::NeedMore)
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
                        self.reg.host = whoami.userhost.map(|uh| uh.host);
                    }
                }
                Ok(HandlerOk::NeedMore)
            }
            "901" => {
                self.reg.account = None;
                if let Some(whoami) = msg.args.clone().split_last().0.last() {
                    let whoami =
                        Source::parse(whoami.clone().owning()).map_err(HandlerError::broken)?;
                    self.reg.nick = whoami.nick;
                    self.reg.host = whoami.userhost.map(|uh| uh.host);
                }
                Ok(HandlerOk::NeedMore)
            }
            "CAP" => {
                use crate::client::cap;
                let mut cap_msg = cap::ServerMsgArgs::parse(&msg.args.clone().owning())
                    .map_err(HandlerError::broken)?;
                match cap_msg.subcmd {
                    cap::SubCmd::Ls if cap_msg.is_last => {
                        self.caps_avail.append(&mut cap_msg.caps);
                        if let HandlerState::Req(reqs) = &mut self.state {
                            let mut reqs = std::mem::take(reqs);
                            reqs.retain(|key| self.caps_avail.contains_key(key));
                            self.state = if reqs.is_empty() {
                                HandlerState::CapEnd
                            } else {
                                cap::req(
                                    reqs.iter().cloned(),
                                    Some(self.reg.nick.clone().into_super()),
                                    self.reg.source.as_ref(),
                                    sink.borrow_mut(),
                                )
                                .map_err(HandlerError::Io)?;
                                HandlerState::Ack(reqs)
                            };
                        }
                    }
                    cap::SubCmd::Ls | cap::SubCmd::New => {
                        self.caps_avail.append(&mut cap_msg.caps);
                    }
                    cap::SubCmd::Ack => {
                        self.state.ack(&cap_msg.caps);
                        for key in cap_msg.caps.into_keys() {
                            let value = self.caps_avail.get(&key).cloned().unwrap_or_default();
                            self.reg.caps.insert(key, value);
                        }
                    }
                    cap::SubCmd::Nak => {
                        self.state.ack(&cap_msg.caps);
                    }
                    cap::SubCmd::Del => {
                        for cap in cap_msg.caps.keys() {
                            self.caps_avail.remove(cap);
                            self.reg.caps.remove(cap);
                        }
                        self.state.ack(&cap_msg.caps);
                    }
                    cap::SubCmd::List => return Err(HandlerError::broken("unexpected LIST")),
                }
                if matches!(self.state, HandlerState::Sasl) {
                    if let Some(names) = self.caps_avail.get("sasl".as_bytes()) {
                        crate::client::cap::filter_sasl(&mut self.auths, names.clone());
                    } else {
                        self.auths.clear();
                    }
                    self.state = HandlerState::CapEnd;
                    #[cfg(feature = "base64")]
                    self.next_sasl(sink.borrow_mut())?;
                }
                Ok(HandlerOk::NeedMore)
            }
            _ => {
                if msg.kind.is_error() == Some(true) {
                    return Err(HandlerError::ServerError(Box::new(msg.clone().owning())));
                }
                Ok(HandlerOk::Ignored)
            }
        }?;
        self.cap_end(sink)?;
        Ok(retval)
    }
    fn next_nick(&mut self, mut sink: impl ClientMsgSink<'static>) -> Result<(), HandlerError> {
        let nick = self.nicks.next().ok_or(HandlerError::NoNicks)?;
        let mut msg = ClientMsg::new_cmd(NICK);
        msg.args.edit().add_word(nick.clone());
        sink.send(msg).map_err(HandlerError::Io)?;
        self.reg.nick = nick;
        Ok(())
    }
    #[cfg(feature = "base64")]
    fn next_sasl(&mut self, mut sink: impl ClientMsgSink<'static>) -> Result<(), HandlerError> {
        use crate::consts::cmd::AUTHENTICATE;
        let Some((name, logic)) = self.auths.pop_front() else {
            return Ok(());
        };
        let mut msg = ClientMsg::new_cmd(AUTHENTICATE);
        msg.args.edit().add_word(name);
        sink.send(msg).map_err(HandlerError::Io)?;
        self.auth = Some(crate::client::auth::Handler::from_logic(logic));
        self.state = HandlerState::Sasl;
        Ok(())
    }
    fn cap_end(&mut self, mut sink: impl ClientMsgSink<'static>) -> Result<bool, HandlerError> {
        if matches!(self.state, HandlerState::CapEnd) {
            if self.needs_auth && self.reg.account.is_none() {
                return Err(HandlerError::NoLogin);
            }
            let mut msg = crate::ircmsg::ClientMsg::new_cmd(CAP);
            msg.args.edit().add_literal("END");
            sink.send(msg).map_err(HandlerError::Io)?;
            self.state = HandlerState::Done;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl<N1: NickTransformer, N2: NickTransformer + 'static> crate::client::Handler
    for Handler<N1, N2>
{
    type Value = Registration;
    type Warning = Line<'static>;
    type Error = HandlerError;

    fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        queue: &mut crate::client::Queue<'static>,
    ) -> HandlerResult<Self::Value, Self::Warning, Self::Error> {
        self.handle(msg, queue)
    }
}

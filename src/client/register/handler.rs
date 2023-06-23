use super::FallbackNicks;
use crate::{
    client::{nick::NickTransformer, ClientMsgSink},
    ircmsg::{ClientMsg, ServerMsg},
    known::cmd::{CAP, NICK},
    source::Source,
    string::{Arg, Key, Nick, Word},
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

/// The result of successful registration.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Registration {
    /// The nickname.
    nick: Nick<'static>,
    /// The name of logged-into account, if any.
    account: Option<Arg<'static>>,
    /// The enabled capabilities and their values.
    caps: BTreeMap<Key<'static>, Word<'static>>,
}

impl Default for Registration {
    fn default() -> Self {
        Self { nick: crate::known::STAR, account: None, caps: BTreeMap::new() }
    }
}

/// Errors that can occur during authentication.
pub enum HandlerError {
    /// Wrong server password, or we're banned.
    NoAccess,
    /// No valid nicknames remaining.
    NoNicks,
    /// Authentication was required, but failed.
    NoLogin,
    /// The server sent a reply indicating an error that cannot be handled.
    ServerError(ServerMsg<'static>),
    /// Either the client or server are irreperably broken.
    Broken,
    /// An I/O error occurred.
    Io(std::io::Error),
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
pub struct Handler<N1: NickTransformer, N2: NickTransformer + 'static, S> {
    pub(super) source: Option<Source<'static>>,
    pub(super) nicks: FallbackNicks<N1, N2>,
    pub(super) state: HandlerState,
    pub(super) needs_auth: bool,
    pub(super) caps_avail: BTreeMap<Key<'static>, Word<'static>>,
    #[cfg(feature = "base64")]
    pub(super) auth: Option<crate::client::auth::Handler>,
    pub(super) auths: VecDeque<Arc<S>>,
    pub(super) reg: Registration,
}

impl<N1: NickTransformer, N2: NickTransformer + 'static, S: crate::client::auth::Sasl>
    Handler<N1, N2, S>
{
    /// Handles a server message sent during connection registration.
    /// Returns `Ok(Some)` once registered,
    /// or `Ok(None) if more messages are required.`
    ///
    /// It is a logic error to call `handle` after
    /// it has returned anything other than `Ok(None)`.
    pub fn handle(
        &mut self,
        msg: &ServerMsg<'_>,
        mut sink: impl ClientMsgSink<'static>,
    ) -> Result<Option<Registration>, HandlerError> {
        if self.source.is_none() {
            self.source = msg.source.as_ref().cloned().map(Source::owning);
        }
        if let Some(pong) = crate::client::pong(msg) {
            sink.send(pong).map_err(HandlerError::Io)?;
            return Ok(None);
        }
        #[cfg(feature = "base64")]
        if let Some(auth) = &mut self.auth {
            use crate::client::auth::HandlerError as AuthHandlerError;
            match auth.handle(msg, sink.borrow_mut()) {
                Ok(true) => {
                    self.auth = None;
                    self.state = HandlerState::CapEnd;
                }
                Err(AuthHandlerError::WrongMechanism(_)) => {
                    // TODO: Purge mechanisms not in set.
                    self.auth = None;
                }
                Err(AuthHandlerError::Io(e)) => return Err(HandlerError::Io(e)),
                Err(AuthHandlerError::Broken(_)) => {
                    sink.send(crate::client::auth::msg_abort()).map_err(HandlerError::Io)?;
                    self.auth = None;
                }
                Err(AuthHandlerError::Fail(_)) => {
                    self.auth = None;
                    self.state = HandlerState::CapEnd;
                    #[cfg(feature = "base64")]
                    self.next_sasl(sink.borrow_mut())?;
                }
                _ => (),
            }
        }
        let retval = match msg.kind.as_str() {
            "001" if self.needs_auth && self.reg.account.is_none() => Err(HandlerError::NoLogin),
            "001" => {
                let nick = msg
                    .args
                    .args()
                    .first()
                    .filter(|n| *n != crate::known::STAR.as_bytes())
                    .and_then(|n| Nick::from_super(n.clone()).ok());
                if let Some(nick) = nick {
                    self.reg.nick = nick.owning();
                }
                Ok(Some(std::mem::take(&mut self.reg)))
            }
            "432" => {
                // Invalid nick. Keep trying user nicks,
                // but don't allow auto-generated fallbacks.
                if self.nicks.has_user_nicks() {
                    self.next_nick(sink.borrow_mut())?;
                    Ok(None)
                } else {
                    Err(HandlerError::NoNicks)
                }
            }
            "433" | "436" => {
                self.next_nick(sink.borrow_mut())?;
                Ok(None)
            }
            "464" | "465" => Err(HandlerError::NoAccess),
            "900" => {
                self.reg.account = msg.args.args().first().cloned().map(Arg::owning);
                Ok(None)
            }
            "901" => {
                self.reg.account = None;
                Ok(None)
            }
            "CAP" => {
                use crate::client::cap;
                let mut cap_msg = cap::ServerMsgArgs::parse(&msg.args.clone().owning())
                    .ok_or(HandlerError::Broken)?;
                match cap_msg.subcmd {
                    cap::SubCmd::Ls if cap_msg.is_last => {
                        self.caps_avail.append(&mut cap_msg.caps);
                        if let HandlerState::Req(reqs) = &mut self.state {
                            let reqs = std::mem::take(reqs);
                            cap::req(
                                reqs.iter().cloned(),
                                Some(self.reg.nick.clone().into_super()),
                                self.source.as_ref(),
                                sink.borrow_mut(),
                            )
                            .map_err(HandlerError::Io)?;
                            self.state = HandlerState::Ack(reqs);
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
                    cap::SubCmd::List => return Err(HandlerError::Broken),
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
                Ok(None)
            }
            _ => {
                if msg.kind.is_error() == Some(true) {
                    return Err(HandlerError::ServerError(msg.clone().owning()));
                }
                Ok(None)
            }
        }?;
        if matches!(self.state, HandlerState::CapEnd) {
            if self.needs_auth && self.reg.account.is_none() {
                return Err(HandlerError::NoLogin);
            }
            let mut msg = crate::ircmsg::ClientMsg::new_cmd(CAP);
            msg.args.add_literal("END");
            sink.send(msg).map_err(HandlerError::Io)?;
            self.state = HandlerState::Done;
        }
        Ok(retval)
    }
    fn next_nick(&mut self, mut sink: impl ClientMsgSink<'static>) -> Result<(), HandlerError> {
        let nick = self.nicks.next().ok_or(HandlerError::NoNicks)?;
        let mut msg = ClientMsg::new_cmd(NICK);
        msg.args.add(nick.clone());
        sink.send(msg).map_err(HandlerError::Io)?;
        self.reg.nick = nick;
        Ok(())
    }
    #[cfg(feature = "base64")]
    fn next_sasl(&mut self, mut sink: impl ClientMsgSink<'static>) -> Result<(), HandlerError> {
        self.auth = loop {
            let Some(front) = self.auths.pop_front() else {
                return Ok(());
            };
            let (msg, handler) = match crate::client::auth::Handler::new(&*front) {
                Ok(m) => m,
                // TODO: Log somehow?
                Err(_) => continue,
            };
            sink.send(msg).map_err(HandlerError::Io)?;
            break Some(handler);
        };
        self.state = HandlerState::Sasl;
        Ok(())
    }
}

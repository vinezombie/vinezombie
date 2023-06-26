//! Types for defining and performing the initial connection registration handshake.

mod fallbacks;
mod handler;

pub use {fallbacks::*, handler::*};

use super::{auth::Sasl, nick::NickTransformer, ClientMsgSink};
use crate::{
    client::auth::Secret,
    error::InvalidByte,
    ircmsg::ClientMsg,
    string::{Key, Line, Nick, User},
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

/// Connection registration options.
///
/// These are used to create the messages sent during the initial connection registration phase,
/// such as USER and NICK.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Register<P, S, N> {
    /// The server password.
    pub pass: Option<P>,
    /// The list of nicknames to use.
    pub nicks: Vec<Nick<'static>>,
    /// A fallback nick transformer, for when all of the nicks in the list are unavailable.
    pub nickgen: Arc<N>,
    /// The username, historically one's local account name.
    pub username: Option<User<'static>>,
    /// The realname, also sometimes known as the gecos.
    pub realname: Option<Line<'static>>,
    /// The list of SASL authenticators.
    pub sasl: Vec<S>,
    /// Whether to continue registration if SASL authentication fails.
    ///
    /// Does nothing if `sasl` is empty.
    pub allow_sasl_fail: bool,
}

/// Creates a new blank [`Register`] the provided choice of [`Secret`] implementation.
pub fn new<S: Secret>() -> Register<S, crate::client::auth::AnySasl<S>, ()> {
    Register {
        pass: None,
        nicks: Vec::new(),
        nickgen: Arc::new(()),
        username: None,
        realname: None,
        sasl: Vec::new(),
        allow_sasl_fail: false,
    }
}

impl<P, S, N> Register<P, S, N> {
    /// Uses the provided password.
    pub fn with_pass<'a, P2: Secret>(
        self,
        pass: impl TryInto<Line<'a>, Error = impl Into<InvalidByte>>,
    ) -> std::io::Result<Register<P2, S, N>> {
        let pass = pass.try_into().map_err(|e| e.into())?.secret();
        let secret = P2::new(pass.into())?;
        Ok(Register {
            pass: Some(secret),
            nicks: self.nicks,
            nickgen: self.nickgen,
            username: self.username,
            realname: self.realname,
            sasl: self.sasl,
            allow_sasl_fail: self.allow_sasl_fail,
        })
    }
    /// Uses the provided [`NickTransformer`] for fallback nicks.
    pub fn with_nickgen<N2: NickTransformer>(self, ng: N2) -> Register<P, S, N2> {
        Register {
            pass: self.pass,
            nicks: self.nicks,
            nickgen: Arc::new(ng),
            username: self.username,
            realname: self.realname,
            sasl: self.sasl,
            allow_sasl_fail: self.allow_sasl_fail,
        }
    }
    /// Adds a SASL authenticator.
    pub fn add_sasl(&mut self, sasl: impl Into<S>) {
        self.sasl.push(sasl.into());
    }
}
impl<P: Secret, S, N: NickTransformer> Register<P, S, N> {
    /// Sends the initial burst of messages for connection registration.
    /// Also returns an [`Iterator`] that returns fallback nicknames.
    ///
    /// # Errors
    /// Errors only if `send_fn` errors.
    pub fn register_msgs<N2: NickTransformer>(
        &self,
        defaults: &'static impl Defaults<NickGen = N2>,
        mut sink: impl ClientMsgSink<'static>,
    ) -> std::io::Result<FallbackNicks<N, N2>> {
        use crate::known::cmd::{CAP, NICK, PASS, USER};
        if let Some(pass) = &self.pass {
            let mut msg = ClientMsg::new_cmd(PASS);
            let mut secret = Vec::new();
            pass.load(&mut secret)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::PermissionDenied, e))?;
            let pass = Line::from_secret(secret)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            msg.args.add_last(pass);
            sink.send(msg)?;
        }
        // CAP message.
        let mut msg = ClientMsg::new_cmd(CAP);
        msg.args.add_literal("LS");
        // TODO: Don't hardcode this, or at least name this constant.
        msg.args.add_literal("302");
        sink.send(msg)?;
        // USER message.
        msg = ClientMsg::new_cmd(USER);
        let args = &mut msg.args;
        args.add(self.username.clone().unwrap_or_else(|| defaults.username()));
        // Some IRCds still rely on 8 to set +i by default.
        args.add_literal("8");
        args.add_literal("*");
        args.add_last(self.realname.clone().unwrap_or_else(|| defaults.realname()));
        sink.send(msg)?;
        // NICK message.
        msg = ClientMsg::new_cmd(NICK);
        let (nick, fallbacks) = FallbackNicks::new(self, defaults);
        msg.args.add(nick);
        sink.send(msg)?;
        Ok(fallbacks)
    }
}
impl<P: Secret, S: Sasl, N: NickTransformer> Register<P, S, N> {
    /// Sends the initial burst of messages for connection registration.
    /// Also returns a [`Handler`] to perform the rest of the connection registration.
    ///
    /// # Errors
    /// Errors only if `send_fn` errors.
    pub fn handler<N2: NickTransformer>(
        &self,
        mut caps: BTreeSet<Key<'static>>,
        defaults: &'static impl Defaults<NickGen = N2>,
        sink: impl ClientMsgSink<'static>,
    ) -> std::io::Result<Handler<N, N2>> {
        let nicks = self.register_msgs(defaults, sink)?;
        let (auths, needs_auth) = if !self.sasl.is_empty() {
            caps.insert(Key::from_str("sasl"));
            let mut auths = Vec::with_capacity(self.sasl.len());
            for sasl in self.sasl.iter() {
                let name = sasl.name();
                let logic = sasl.logic()?;
                auths.push((name, logic));
            }
            (auths.into(), !self.allow_sasl_fail)
        } else {
            (VecDeque::new(), false)
        };
        Ok(Handler {
            nicks,
            state: HandlerState::Req(caps),
            needs_auth,
            caps_avail: BTreeMap::new(),
            #[cfg(feature = "base64")]
            auth: None,
            auths,
            reg: Registration::default(),
        })
    }
}

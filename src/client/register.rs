//! Types for defining and performing the initial connection registration handshake.

mod fallbacks;
mod handler;

pub use {fallbacks::*, handler::*};

use super::nick::NickTransformer;
use crate::{
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
pub struct Register<S, N> {
    /// The server password.
    pub pass: Option<Line<'static>>,
    /// The list of nicknames to use.
    pub nicks: Vec<Nick<'static>>,
    /// A fallback nick transformer, for when all of the nicks in the list are unavailable.
    pub nickgen: Arc<N>,
    /// The username, historically one's local account name.
    pub username: Option<User<'static>>,
    /// The realname, also sometimes known as the gecos.
    pub realname: Option<Line<'static>>,
    /// The list of SASL authenticators.
    pub sasl: Vec<Arc<S>>,
    /// Whether to continue registration if SASL authentication fails.
    ///
    /// Does nothing if `sasl` is empty.
    pub allow_sasl_fail: bool,
}

impl<S, N> Register<S, N> {
    /// Adds a SASL authenticator.
    pub fn add_sasl(&mut self, sasl: impl Into<S>) {
        self.sasl.push(Arc::new(sasl.into()));
    }
}
impl<S, N: NickTransformer> Register<S, N> {
    /// Sends the initial burst of messages for connection registration.
    /// Also returns an [`Iterator`] that returns fallback nicknames.
    ///
    /// # Errors
    /// Errors only if `send_fn` errors.
    pub fn register_msgs<N2: NickTransformer>(
        &self,
        defaults: &'static impl Defaults<NickGen = N2>,
        mut send_fn: impl FnMut(ClientMsg<'static>) -> std::io::Result<()>,
    ) -> std::io::Result<FallbackNicks<N, N2>> {
        use crate::known::cmd::{CAP, NICK, PASS, USER};
        if let Some(pass) = &self.pass {
            let mut msg = ClientMsg::new_cmd(PASS);
            msg.args.add_last(pass.clone().secret());
            send_fn(msg)?;
        }
        // CAP message.
        let mut msg = ClientMsg::new_cmd(CAP);
        msg.args.add_literal("LS");
        // TODO: Don't hardcode this, or at least name this constant.
        msg.args.add_literal("302");
        send_fn(msg)?;
        // USER message.
        msg = ClientMsg::new_cmd(USER);
        let args = &mut msg.args;
        args.add(self.username.clone().unwrap_or_else(|| defaults.username()));
        // Some IRCds still rely on 8 to set +i by default.
        args.add_literal("8");
        args.add_literal("*");
        args.add_last(self.realname.clone().unwrap_or_else(|| defaults.realname()));
        send_fn(msg)?;
        // NICK message.
        msg = ClientMsg::new_cmd(NICK);
        let (nick, fallbacks) = FallbackNicks::new(self, defaults);
        msg.args.add(nick);
        send_fn(msg)?;
        Ok(fallbacks)
    }
    /// Sends the initial burst of messages for connection registration.
    /// Also returns a [`Handler`] to perform the rest of the connection registration.
    ///
    /// # Errors
    /// Errors only if `send_fn` errors.
    pub fn handler<N2: NickTransformer>(
        &self,
        mut caps: BTreeSet<Key<'static>>,
        defaults: &'static impl Defaults<NickGen = N2>,
        mut send_fn: impl FnMut(ClientMsg<'static>) -> std::io::Result<()>,
    ) -> std::io::Result<Handler<N, N2, S>> {
        let nicks = self.register_msgs(defaults, &mut send_fn)?;
        let (auths, needs_auth) = if !self.sasl.is_empty() {
            caps.insert(Key::from_str("sasl"));
            (self.sasl.iter().cloned().collect(), !self.allow_sasl_fail)
        } else {
            (VecDeque::new(), false)
        };
        Ok(Handler {
            source: None,
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

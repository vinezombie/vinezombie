//! Types for defining and performing the initial connection registration handshake.

mod fallbacks;

pub use fallbacks::*;

use super::nick::NickTransformer;
use crate::{
    ircmsg::ClientMsg,
    string::{Line, Nick, User},
};
use std::sync::Arc;

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
}

impl<S, N> Register<S, N> {
    /// Adds a SASL authenticator.
    pub fn add_sasl(&mut self, sasl: impl Into<S>) {
        self.sasl.push(Arc::new(sasl.into()));
    }
}
impl<S, N: NickTransformer> Register<S, N> {
    /// Generates the initial burst of messages for connection registration,
    /// as well as an [`Iterator`] that returns fallback nicknames.
    pub fn register_msgs<N2: NickTransformer>(
        &self,
        defaults: &'static impl Defaults<NickGen = N2>,
    ) -> (Vec<ClientMsg<'static>>, FallbackNicks<N, N2>) {
        use crate::known::cmd::{CAP, NICK, USER};
        // CAP message.
        let mut msg_c = ClientMsg::new_cmd(CAP);
        msg_c.args.add_literal("LS");
        // TODO: Don't hardcode this, or at least name this constant.
        msg_c.args.add_literal("302");
        // USER message.
        let mut msg_u = ClientMsg::new_cmd(USER);
        let args = &mut msg_u.args;
        args.add(self.username.clone().unwrap_or_else(|| defaults.username()));
        // Some IRCds still rely on 8 to set +i by default.
        args.add_literal("8");
        args.add_literal("*");
        args.add_last(self.realname.clone().unwrap_or_else(|| defaults.realname()));
        // NICK message.
        let mut msg_n = ClientMsg::new_cmd(NICK);
        let (nick, fallbacks) = FallbackNicks::new(self, defaults);
        msg_n.args.add(nick);
        (vec![msg_c, msg_u, msg_n], fallbacks)
    }
}

//! Types for defining and performing the initial connection registration handshake.

mod defaults;
mod handler;
#[cfg(test)]
mod tests;

use super::auth::SaslQueue;

pub use {defaults::*, handler::*};

use crate::{
    client::{nick::NickGen, ClientMsgSink, MakeHandler},
    ircmsg::ClientMsg,
    string::{Arg, Key, Line, Nick, User},
};
use std::collections::BTreeSet;

/// Client logic for the connection registration process.
///
/// Consider using the [`register_as_bot()`], [`register_as_client()`],
/// or [`register_as_custom()`] functions to instantiate one of these.
#[derive(Clone)]
pub struct Register<O> {
    /// Returns the server password, if any.
    pub password: fn(&O) -> Option<std::io::Result<Line<'static>>>,
    /// Returns the username to use for connection.
    pub username: fn(&O) -> User<'static>,
    /// Returns the value used for the first unused USER parameter.
    ///
    /// For older IRC software, this parameter is not actually unusused.
    /// RFC1459 specifies that this should be the connecting system's hostname, while
    /// RFC2812 specifies that this should be a decimal integer whose bits are
    /// used to set user modes on connection.
    pub user_p1: fn(&O) -> Arg<'static>,
    /// Returns the value used for the second unused USER parameter.
    ///
    /// For older IRC software, this parameter is not actually unusused.
    /// RFC1459 specifies that this should be the name of the server being connected to.
    pub user_p2: fn(&O) -> Arg<'static>,
    /// Returns the realname to use for connection.
    pub realname: fn(&O) -> Line<'static>,
    /// Creates a nick generator.
    pub nicks: fn(&O) -> Box<dyn crate::client::nick::NickGen>,
    /// Returns a set of capabilities to request.
    ///
    /// This does not need to include `sasl` if the authenticator list is non-empty.
    pub caps: fn(&O) -> BTreeSet<Key<'static>>,
    /// Returns a [`SaslQueue`] to attempt
    /// and whether to close the connection on non-authentication.
    pub auth: fn(&O) -> (SaslQueue, bool),
}

impl<O> Register<O> {
    /// Sends the initial burst of messages for connection registration.
    /// Also returns the nickname used and a generator for fallback nicknames.
    ///
    /// # Errors
    /// Errors only if retrieving the server password errors.
    pub fn register_msgs(
        &self,
        opts: &O,
        mut sink: impl ClientMsgSink<'static>,
    ) -> std::io::Result<(Nick<'static>, Option<Box<dyn NickGen>>)> {
        use crate::names::cmd::{CAP, NICK, PASS, USER};
        if let Some(pass) = (self.password)(opts) {
            let pass = pass?;
            let mut msg = ClientMsg::new(PASS);
            msg.args.edit().add(pass);
            sink.send(msg);
        }
        // CAP message.
        let mut msg = ClientMsg::new(CAP);
        let mut args = msg.args.edit();
        args.add_literal("LS");
        // TODO: Don't hardcode this, or at least name this constant.
        args.add_literal("302");
        sink.send(msg);
        // USER message.
        msg = ClientMsg::new(USER);
        let mut args = msg.args.edit();
        args.add_word((self.username)(opts));
        args.add_word((self.user_p1)(opts));
        args.add_word((self.user_p2)(opts));
        args.add((self.realname)(opts));
        sink.send(msg);
        // NICK message.
        msg = ClientMsg::new(NICK);
        let nicks = (self.nicks)(opts);
        let (nick, nickgen) = nicks.next_nick();
        msg.args.edit().add_word(nick.clone());
        sink.send(msg);
        Ok((nick, nickgen))
    }
}

impl<O> Register<O> {
    /// Sends the initial burst of messages for connection registration.
    /// Also returns a [`Handler`] to perform the rest of the connection registration.
    ///
    /// # Errors
    /// Errors if registration messages cannot be created,
    /// or if SASL handlers cannot be created.
    pub fn handler(&self, opts: &O, sink: impl ClientMsgSink<'static>) -> std::io::Result<Handler> {
        let nicks = self.register_msgs(opts, sink)?;
        let mut caps = (self.caps)(opts);
        let (auths, mut needs_auth) = (self.auth)(opts);
        if !auths.is_empty() {
            caps.insert(Key::from_str("sasl"));
        } else {
            needs_auth &= auths.had_values();
        };
        Ok(Handler::new(nicks, caps, needs_auth, auths))
    }
}

impl<'a, O> MakeHandler<&'a O> for &'a Register<O> {
    type Value = Result<Registration, HandlerError>;

    type Error = std::io::Error;

    type Receiver<Spec: super::channel::ChannelSpec> = Spec::Oneshot<Self::Value>;

    type Handler = Handler;

    fn make_handler(
        self,
        mut queue: super::QueueEditGuard<'_>,
        opts: &'a O,
    ) -> Result<Handler, Self::Error> {
        self.handler(opts, &mut queue)
    }

    fn make_channel<Spec: super::channel::ChannelSpec>(
        spec: &Spec,
    ) -> (Box<dyn super::channel::Sender<Value = Self::Value> + Send>, Self::Receiver<Spec>) {
        spec.new_oneshot()
    }
}

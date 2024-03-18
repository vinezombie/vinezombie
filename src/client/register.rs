//! Types for defining and performing the initial connection registration handshake.

mod defaults;
mod handler;
#[cfg(test)]
mod tests;

pub use {defaults::*, handler::*};

use crate::{
    client::{auth::Sasl, nick::NickGen, ClientMsgSink, MakeHandler},
    ircmsg::ClientMsg,
    state::serverinfo::ISupportParser,
    string::{Arg, Key, Line, Nick, User},
};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};

/// An iterator of references to [`Sasl`]s and indicator of whether SASL is required.
pub type SaslOptions<'a, A> = (Box<dyn Iterator<Item = &'a A> + 'a>, bool);

/// Client logic for the connection registration process.
///
/// Consider using the [`register_as_bot()`], [`register_as_client()`],
/// or [`register_as_custom()`] functions to instantiate one of these.
#[derive(Clone)]
pub struct Register<O, A> {
    /// Returns the server password, if any.
    pub password: fn(&O) -> std::io::Result<Option<Line<'static>>>,
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
    pub caps: fn(&O) -> &BTreeSet<Key<'static>>,
    /// Returns a boxed iterator of references to [`Sasl`] authenticators to attempt
    /// and whether to close the connection on non-authentication.
    pub auth: fn(&O) -> SaslOptions<'_, A>,
    /// The [`ISupportParser`] used to parse ISUPPORT messages after registration completes.
    ///
    /// `ISupportParser`s are not `Clone`, making this `Arc` necessary.
    pub isupport_parser: Arc<ISupportParser>,
}

impl<O, A> Register<O, A> {
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
        use crate::consts::cmd::{CAP, NICK, PASS, USER};
        if let Some(pass) = (self.password)(opts)? {
            let mut msg = ClientMsg::new_cmd(PASS);
            msg.args.edit().add(pass);
            sink.send(msg);
        }
        // CAP message.
        let mut msg = ClientMsg::new_cmd(CAP);
        let mut args = msg.args.edit();
        args.add_literal("LS");
        // TODO: Don't hardcode this, or at least name this constant.
        args.add_literal("302");
        sink.send(msg);
        // USER message.
        msg = ClientMsg::new_cmd(USER);
        let mut args = msg.args.edit();
        args.add_word((self.username)(opts));
        args.add_word((self.user_p1)(opts));
        args.add_word((self.user_p2)(opts));
        args.add((self.realname)(opts));
        sink.send(msg);
        // NICK message.
        msg = ClientMsg::new_cmd(NICK);
        let nicks = (self.nicks)(opts);
        let (nick, nickgen) = nicks.next_nick();
        msg.args.edit().add_word(nick.clone());
        sink.send(msg);
        Ok((nick, nickgen))
    }
}

impl<O, A: Sasl> Register<O, A> {
    /// Sends the initial burst of messages for connection registration.
    /// Also returns a [`Handler`] to perform the rest of the connection registration.
    ///
    /// # Errors
    /// Errors if registration messages cannot be created,
    /// or if SASL handlers cannot be created.
    pub fn handler(&self, opts: &O, sink: impl ClientMsgSink<'static>) -> std::io::Result<Handler> {
        let nicks = self.register_msgs(opts, sink)?;
        let mut caps = (self.caps)(opts).clone();
        let (auths, needs_auth) = (self.auth)(opts);
        let mut auths_vec = Vec::with_capacity(auths.size_hint().0);
        for sasl in auths {
            let name = sasl.name();
            let Ok(logic) = sasl.logic() else {
                // TODO: Replace with match and log the error.
                continue;
            };
            auths_vec.push((name, logic));
        }
        let (auths, needs_auth) = if !auths_vec.is_empty() {
            caps.insert(Key::from_str("sasl"));
            (auths_vec.into(), needs_auth)
        } else {
            (VecDeque::new(), false)
        };
        Ok(Handler::new(nicks, caps, needs_auth, auths, self.isupport_parser.clone()))
    }
}

impl<'a, O, A: Sasl> MakeHandler<&'a O> for Register<O, A> {
    type Value = Result<Registration, HandlerError>;

    type Error = std::io::Error;

    type Receiver<Spec: super::channel::ChannelSpec> = Spec::Oneshot<Self::Value>;

    fn make_handler(
        &self,
        mut queue: super::QueueEditGuard<'_>,
        opts: &'a O,
    ) -> Result<impl super::Handler<Value = Self::Value>, Self::Error> {
        self.handler(opts, &mut queue)
    }

    fn make_channel<Spec: super::channel::ChannelSpec>(
        spec: &Spec,
    ) -> (Arc<dyn super::channel::Sender<Value = Self::Value>>, Self::Receiver<Spec>) {
        spec.new_oneshot()
    }
}

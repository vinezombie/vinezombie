use super::nick::{NickTransformer, SuffixRandom};
use crate::{
    client::auth::AnySasl,
    ircmsg::ClientMsg,
    string::{Line, Nick, User},
};

/// Connection registration options.
///
/// These are used to create the messages sent during the initial connection registration phase,
/// such as USER and NICK.
/// These options are limited to a subset of the possibilities that
/// are known to be serializeable.
#[derive(Clone, Debug, Default)]
pub struct Register<S> {
    /// The server password.
    pub pass: Option<Line<'static>>,
    /// The list of nicknames to use.
    pub nicks: Vec<Nick<'static>>,
    /// The username, historically one's local account name.
    pub username: Option<User<'static>>,
    /// The realname, also sometimes known as the gecos.
    pub realname: Option<Line<'static>>,
    /// A list of SASL authenticators.
    pub sasl: Vec<AnySasl<S>>,
}

impl<S> Register<S> {
    /// Adds a SASL authenticator.
    pub fn add_sasl(&mut self, sasl: impl Into<AnySasl<S>>) {
        self.sasl.push(sasl.into());
    }
    /// Generates the initial burst of messages for connection registration.
    pub fn register_msgs(&self, defaults: &impl RegisterDefaults) -> Vec<ClientMsg<'static>> {
        use crate::known::cmd::{CAP, NICK, USER};
        // CAP message.
        let mut msg_c = ClientMsg::new_cmd(CAP);
        msg_c.args.add_literal("LS");
        // TODO: Don't hardcode this, or at least name this constant.
        msg_c.args.add_literal("302");
        // USER message.
        let mut msg_u = ClientMsg::new_cmd(USER);
        let args = &mut msg_u.args;
        args.add_word(self.username.clone().unwrap_or_else(|| defaults.username()));
        // Some IRCds still rely on 8 to set +i by default.
        args.add_literal("8");
        args.add_literal("*");
        args.add(self.realname.clone().unwrap_or_else(|| defaults.realname()));
        // NICK message.
        let nick = if let Some(nick) = self.nicks.first() {
            nick.clone()
        } else {
            // Discard for now, use the state to return some sort of nick generator later.
            defaults.nick().0
        };
        let mut msg_n = ClientMsg::new_cmd(NICK);
        msg_n.args.add_word(nick);
        vec![msg_c, msg_u, msg_n]
    }
}

/// Client-wide defaults for new connections.
pub trait RegisterDefaults {
    /// Nick transformer for user-specified nicks.
    type NtUser: NickTransformer;
    /// Nick transformer for the client-wide default nick.
    type NtDefault: NickTransformer;
    /// Returns the nick transformers for fallback nicks.
    fn transformers(&self) -> (&Self::NtUser, &Self::NtDefault);
    /// Returns the default nick and optional transformer state for it.
    fn nick(&self) -> (Nick<'static>, Option<<Self::NtDefault as NickTransformer>::State>);
    /// Returns the default username.
    fn username(&self) -> User<'static>;
    /// Returns the default realname.
    fn realname(&self) -> Line<'static>;
}

/// Sensible default implementation of [`RegisterDefaults`].
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct DefaultRegisterDefaults(pub SuffixRandom);

impl Default for DefaultRegisterDefaults {
    fn default() -> Self {
        use crate::client::nick::NickSuffix;
        DefaultRegisterDefaults(SuffixRandom {
            seed: None,
            suffixes: std::borrow::Cow::Borrowed(&[NickSuffix::Base10, NickSuffix::Base8]),
        })
    }
}

impl RegisterDefaults for DefaultRegisterDefaults {
    type NtUser = SuffixRandom;

    type NtDefault = SuffixRandom;

    fn transformers(&self) -> (&Self::NtUser, &Self::NtDefault) {
        (&self.0, &self.0)
    }

    fn nick(&self) -> (Nick<'static>, Option<<Self::NtDefault as NickTransformer>::State>) {
        let nick = unsafe { Nick::from_unchecked("Guest".into()) };
        if let Some(tf) = self.0.init(&nick) {
            tf
        } else {
            (nick, None)
        }
    }

    fn username(&self) -> User<'static> {
        let username = unsafe { User::from_unchecked("user".into()) };
        User::new_username().unwrap_or(username)
    }

    fn realname(&self) -> Line<'static> {
        let realname = unsafe { Line::from_unchecked("???".into()) };
        Line::new_realname().unwrap_or(realname)
    }
}

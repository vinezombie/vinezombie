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
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Register<S, N> {
    /// The server password.
    pub pass: Option<Line<'static>>,
    /// The list of nicknames to use.
    pub nicks: Vec<Nick<'static>>,
    /// A fallback nick transformer, for when all of the nicks in the list are unavailable.
    pub nickgen: N,
    /// The username, historically one's local account name.
    pub username: Option<User<'static>>,
    /// The realname, also sometimes known as the gecos.
    pub realname: Option<Line<'static>>,
    /// A list of SASL authenticators.
    pub sasl: Vec<AnySasl<S>>,
}

impl<S, N> Register<S, N> {
    /// Adds a SASL authenticator.
    pub fn add_sasl(&mut self, sasl: impl Into<AnySasl<S>>) {
        self.sasl.push(sasl.into());
    }
}
impl<S, N: NickTransformer> Register<S, N> {
    /// Generates the initial burst of messages for connection registration,
    /// as well as an [`Iterator`] that returns fallback nicknames.
    pub fn register_msgs<'a, N2: NickTransformer>(
        &'a self,
        defaults: &'a impl RegisterDefaults<NickGen = N2>,
    ) -> (Vec<ClientMsg<'static>>, FallbackNicks<'a, N, N2>) {
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
        let mut msg_n = ClientMsg::new_cmd(NICK);
        let (nick, fallbacks) = FallbackNicks::new(self, defaults);
        msg_n.args.add_word(nick);
        (vec![msg_c, msg_u, msg_n], fallbacks)
    }
}

/// Client-wide defaults for new connections.
pub trait RegisterDefaults {
    /// Nick transformer for the client-wide default nick.
    type NickGen: NickTransformer;
    /// Returns the nick transformers for fallback nicks.
    fn nick_gen(&self) -> &Self::NickGen;
    /// Returns the default nick and optional transformer state for it.
    fn nick(&self) -> (Nick<'static>, Option<<Self::NickGen as NickTransformer>::State>);
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
    type NickGen = SuffixRandom;

    fn nick_gen(&self) -> &Self::NickGen {
        &self.0
    }

    fn nick(&self) -> (Nick<'static>, Option<<Self::NickGen as NickTransformer>::State>) {
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

/// Source of fallback nicks that uses a [`Register`] and [`RegisterDefaults`].
#[derive(Clone, Copy, Debug)]
pub struct FallbackNicks<'a, N1: NickTransformer, N2: NickTransformer> {
    state: FallbackNicksState<'a, N1::State, N2::State>,
    n1: &'a N1,
    n2: &'a N2,
}

impl<'a, N1: NickTransformer, N2: NickTransformer> FallbackNicks<'a, N1, N2> {
    /// Generate the first nickname and a `FallbackNicks` for more.
    pub fn new<S>(
        reg: &'a Register<S, N1>,
        reg_def: &'a impl RegisterDefaults<NickGen = N2>,
    ) -> (Nick<'static>, Self) {
        let (nick, state) = if let Some((nick, rest)) = reg.nicks.split_first() {
            (nick.clone(), FallbackNicksState::Select(nick, rest))
        } else {
            let (nick, state) = reg_def.nick();
            (nick, state.map(FallbackNicksState::Gen2).unwrap_or(FallbackNicksState::Done))
        };
        (nick, FallbackNicks { state, n1: &reg.nickgen, n2: reg_def.nick_gen() })
    }
}
impl<'a, N1: NickTransformer, N2: NickTransformer> Iterator for FallbackNicks<'a, N1, N2> {
    type Item = Nick<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        match std::mem::take(&mut self.state) {
            FallbackNicksState::Select(seed, rest) => {
                if let Some((nick, rest)) = rest.split_first() {
                    self.state = FallbackNicksState::Select(seed, rest);
                    Some(nick.clone())
                } else if let Some((nick, state)) = self.n1.init(seed) {
                    if let Some(state) = state {
                        self.state = FallbackNicksState::Gen1(state);
                    }
                    Some(nick)
                } else {
                    None
                }
            }
            FallbackNicksState::Gen1(g) => {
                let (nick, state) = self.n1.step(g);
                if let Some(state) = state {
                    self.state = FallbackNicksState::Gen1(state);
                }
                Some(nick)
            }
            FallbackNicksState::Gen2(g) => {
                let (nick, state) = self.n2.step(g);
                if let Some(state) = state {
                    self.state = FallbackNicksState::Gen2(state);
                }
                Some(nick)
            }
            FallbackNicksState::Done => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
enum FallbackNicksState<'a, S1, S2> {
    Select(&'a Nick<'static>, &'a [Nick<'static>]),
    Gen1(S1),
    Gen2(S2),
    #[default]
    Done,
}

use super::Register;
use crate::{
    client::nick::{NickSuffix, NickTransformer, SuffixRandom},
    string::{Line, Nick, User},
};
use std::{collections::VecDeque, sync::Arc};

/// Source of fallback nicks from a [`Register`] and [`Defaults`].
#[derive(Clone, Debug)]
pub struct FallbackNicks<N1: NickTransformer, N2: NickTransformer + 'static> {
    state: FallbackNicksState<N1::State, N2::State>,
    n1: Arc<N1>,
    n2: &'static N2,
}

impl<N1: NickTransformer, N2: NickTransformer> FallbackNicks<N1, N2> {
    /// Generate the first nickname and a `FallbackNicks` for more.
    pub fn new<P, S>(
        reg: &Register<P, S, N1>,
        reg_def: &'static impl Defaults<NickGen = N2>,
    ) -> (Nick<'static>, Self) {
        let (nick, state) = if let Some((nick, rest)) = reg.nicks.split_first() {
            let rest: VecDeque<Nick<'static>> = rest.to_vec().into();
            (nick.clone(), FallbackNicksState::Select(nick.clone(), rest))
        } else {
            let (nick, state) = reg_def.nick();
            (nick, state.map(FallbackNicksState::Gen2).unwrap_or(FallbackNicksState::Done))
        };
        (nick, FallbackNicks { state, n1: reg.nickgen.clone(), n2: reg_def.nick_gen() })
    }
    /// Returns `true` if the next nickname yielded by `self` is a user-specified one.
    pub fn has_user_nicks(&self) -> bool {
        matches!(self.state, FallbackNicksState::Select(_, _))
    }
}
impl<N1: NickTransformer, N2: NickTransformer> Iterator for FallbackNicks<N1, N2> {
    type Item = Nick<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        match std::mem::take(&mut self.state) {
            FallbackNicksState::Select(seed, mut rest) => {
                if let Some(nick) = rest.pop_front() {
                    self.state = FallbackNicksState::Select(seed, rest);
                    Some(nick.clone())
                } else if let Some((nick, state)) = self.n1.init(&seed) {
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

#[derive(Clone, Debug, Default)]
enum FallbackNicksState<S1, S2> {
    Select(Nick<'static>, VecDeque<Nick<'static>>),
    Gen1(S1),
    Gen2(S2),
    #[default]
    Done,
}

/// Client-wide defaults for new connections.
pub trait Defaults {
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

/// Sensible default implementation of [`Defaults`].
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct DefaultDefaults(pub SuffixRandom);

impl Default for DefaultDefaults {
    fn default() -> Self {
        DefaultDefaults(SuffixRandom {
            seed: None,
            suffixes: std::borrow::Cow::Borrowed(&[NickSuffix::Base10, NickSuffix::Base8]),
        })
    }
}

impl Defaults for DefaultDefaults {
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

static BNG: SuffixRandom = SuffixRandom {
    seed: None,
    suffixes: std::borrow::Cow::Borrowed(&[
        NickSuffix::Base10,
        NickSuffix::Base8,
        NickSuffix::Base10,
        NickSuffix::Base8,
        NickSuffix::Base10,
        NickSuffix::Base8,
    ]),
};

/// Overtly bot-like default implementation of [`Defaults`].
pub struct BotDefaults;

impl Defaults for BotDefaults {
    type NickGen = SuffixRandom;

    fn nick_gen(&self) -> &Self::NickGen {
        &BNG
    }

    fn nick(&self) -> (Nick<'static>, Option<<Self::NickGen as NickTransformer>::State>) {
        self.nick_gen().init(&Nick::from_str("VZB")).unwrap()
    }

    fn username(&self) -> User<'static> {
        User::from_str("vnzb_bot")
    }

    fn realname(&self) -> Line<'static> {
        Line::from_str("Vinezombie Bot")
    }
}

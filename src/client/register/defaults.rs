use super::{CapFn, Register};
use crate::{
    client::{
        auth::{AnySasl, LoadSecret, Sasl, SaslQueue, Secret},
        nick::{NickGen, Suffix, SuffixStrategy, SuffixType},
    },
    error::InvalidString,
    string::{Arg, Key, Line, Nick, User},
};
use std::collections::BTreeSet;

/// Connection registration options.
///
/// These cover the options the majority of users will find useful for connection registration.
/// It is (de)serializable if the chosen [`LoadSecret`] and [`Sasl`] implementations are.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        default,
        bound(deserialize = "S: LoadSecret + serde::Deserialize<'de>, A: serde::Deserialize<'de>")
    )
)]
pub struct Options<S, A = AnySasl<S>> {
    /// The server password.
    pub pass: Option<Secret<Line<'static>, S>>,
    /// The list of nicknames to attempt before fallbacks.
    pub nicks: Vec<Nick<'static>>,
    /// The username, historically one's local account name.
    pub username: Option<User<'static>>,
    /// The realname, also sometimes known as the gecos.
    pub realname: Option<Line<'static>>,
    /// The list of SASL authenticators.
    pub sasl: Vec<A>,
    /// Whether to continue registration if SASL authentication fails.
    ///
    /// Does nothing if `sasl` is empty.
    pub allow_sasl_fail: bool,
    /// Additional capabilities to request, on top of what the client supports.
    pub caps: BTreeSet<Key<'static>>,
}

impl<S, A: Sasl> Options<S, A> {
    /// Returns a [`SaslQueue`] and whether SASL is required,
    /// as used by [`Register`][super::Register].
    pub fn auths(&self) -> (SaslQueue, bool) {
        let queue: SaslQueue = self.sasl.iter().collect();
        let require_sasl = !(self.allow_sasl_fail || queue.is_empty());
        (queue, require_sasl)
    }
}

impl<S, A> Default for Options<S, A> {
    fn default() -> Self {
        Options::new()
    }
}

impl<S, A> Options<S, A> {
    /// Creates a new blank [`Register`].
    pub const fn new() -> Self {
        Options {
            pass: None,
            nicks: Vec::new(),
            username: None,
            realname: None,
            sasl: Vec::new(),
            allow_sasl_fail: false,
            caps: BTreeSet::new(),
        }
    }
}

impl<S, A> Options<S, A> {
    /// Uses the provided password.
    pub fn set_pass(
        &mut self,
        pass: impl TryInto<Line<'static>, Error = impl Into<InvalidString>>,
    ) -> std::io::Result<()> {
        let pass = pass.try_into().map_err(|e| e.into())?.secret();
        let secret = Secret::new(pass);
        self.pass = Some(secret);
        Ok(())
    }
}

impl<S, A: Sasl> Options<S, A> {
    /// Adds a SASL authenticator.
    pub fn add_sasl(&mut self, sasl: impl Into<A>) {
        self.sasl.push(sasl.into());
    }
}

/// Returns a [`Register`] with sensible functions.
pub fn register_as_custom<O>(
    password: fn(&O) -> Option<Line<'static>>,
    username: fn(&O) -> User<'static>,
    realname: fn(&O) -> Line<'static>,
    nicks: fn(&O) -> Box<dyn crate::client::nick::NickGen>,
    caps: fn(&O) -> Box<dyn CapFn>,
    auth: fn(&O) -> (SaslQueue, bool),
) -> Register<O> {
    Register {
        password,
        username,
        user_p1: default_user_p1,
        user_p2: default_user_p2,
        realname,
        nicks,
        caps,
        auth,
    }
}

/// Returns a [`Register`] with sensible functions for human-oriented clients.
///
/// The capability set is treated as a set of capabilities to soft-request, on top of an
/// intersect of the available caps and a reasonable set of defaults (see [`default_caps`]).
pub fn register_as_client<S: LoadSecret, A: Sasl>() -> Register<Options<S, A>> {
    register_as_custom(
        |opts| opts.pass.clone().map(Secret::into_inner),
        |opts| default_client_username(opts.username.as_ref()),
        |opts| default_client_realname(opts.realname.as_ref()),
        |opts| default_client_nicks(opts.nicks.clone()),
        |opts| default_caps(opts.caps.clone(), true, false),
        Options::auths,
    )
}

/// Returns a [`Register`] with sensible functions for bots.
///
/// The capability set is treated as a list of capabilities to request,
/// or error if not present.
pub fn register_as_bot<S: LoadSecret, A: Sasl>() -> Register<Options<S, A>> {
    register_as_custom(
        |opts| opts.pass.clone().map(Secret::into_inner),
        |opts| default_bot_username(opts.username.as_ref()),
        |opts| default_bot_realname(opts.realname.as_ref()),
        |opts| default_bot_nicks(opts.nicks.clone()),
        |opts| default_caps(opts.caps.clone(), false, true),
        Options::auths,
    )
}

static DEFAULT_CAPS: std::sync::OnceLock<BTreeSet<Key<'static>>> = std::sync::OnceLock::new();

macro_rules! make_default_caps {
    ($($name:literal,)+) => {
        #[doc="Returns a large set of IRCv3 capabilities."]
        #[doc=""]
        #[doc="Every handler provided by this library can handle these capabilities,"]
        #[doc="and they are a reasonable baseline for modern IRC software."]
        #[doc="This set excludes capabilities that are in draft status or"]
        #[doc="are likely to significantly change how consumers of this library must"]
        #[doc="handle messages from the server (e.g. `batch`, `echo-message`)."]
        #[doc="The specific capabilities included are:"]
        $(#[doc=concat!(" `",$name,"`")])+
        pub fn common_caps() -> &'static BTreeSet<Key<'static>> {
            DEFAULT_CAPS.get_or_init(|| {
                [$(Key::from_str($name)),+].into_iter().collect()
            })
        }
    }
}

// Request sasl as-needed for auth.
// Do not request batch or echo-message because they are prone to dramatically changing
// how downstream software needs to handle messages.
make_default_caps! {
    "account-notify",
    "account-tag",
    "chghost",
    "extended-join",
    "extended-monitor",
    "invite-notify",
    "labeled-response",
    "message-tags",
    "msgid",
    "multi-prefix",
    "server-time",
    "setname",
    "standard-replies",
    "userhost-in-names",
}

/// For use with [`Register`].
///
/// Returns a [`CapFn`] for use during connection registration.
/// If `add_common` is true, opportunistically requests a common set of capabilities
/// (see [`common_caps`]) in addition to `caps`.
/// If `require` is true, the capabilities in `caps` are considered required, and capability
/// negotitation will fail if they are not present.
pub fn default_caps(
    mut caps: BTreeSet<Key<'static>>,
    add_common: bool,
    require: bool,
) -> Box<dyn CapFn> {
    Box::new(move |caps_avail: &BTreeSet<Key<'_>>| match (add_common, require) {
        (false, false) => caps.intersection(caps_avail).map(|k| k.clone().owning()).collect(),
        (false, true) => caps,
        (true, false) => {
            caps = caps.union(common_caps()).cloned().collect();
            caps.intersection(caps_avail).map(|k| k.clone().owning()).collect()
        }
        (true, true) => {
            let common =
                caps_avail.intersection(common_caps()).map(|k| k.clone().owning()).collect();
            caps.union(&common).cloned().collect()
        }
    })
}

/// For use with [`Register`].
///
/// Returns `"8"`, which sets usermode "i"
/// on servers that use the RFC2812 USER message.
pub fn default_user_p1<O>(_: &O) -> Arg<'static> {
    Arg::from_str("8")
}

/// For use with [`Register`].
///
/// The default implementation returns `"*"`,
/// as per recommendations.
pub fn default_user_p2<O>(_: &O) -> Arg<'static> {
    crate::names::STAR.into()
}

/// A sensible nick transformer for fallback user nicks.
pub static NT_FALLBACK: Suffix = Suffix {
    strategy: SuffixStrategy::Seq,
    suffixes: std::borrow::Cow::Borrowed(&[
        SuffixType::Char('`'),
        SuffixType::Base10,
        SuffixType::Base8,
    ]),
};

static NT_GUEST: Suffix = Suffix {
    strategy: SuffixStrategy::Rng(None),
    suffixes: std::borrow::Cow::Borrowed(&[
        SuffixType::Base10,
        SuffixType::Base8,
        SuffixType::Base10,
        SuffixType::Base8,
    ]),
};

static NT_BOT: Suffix = Suffix {
    strategy: SuffixStrategy::Rng(None),
    suffixes: std::borrow::Cow::Borrowed(&[
        SuffixType::Base10,
        SuffixType::Base16(false),
        SuffixType::Base16(false),
        SuffixType::Base16(false),
        SuffixType::Base16(false),
        SuffixType::Base16(false),
    ]),
};

/// For use with [`Register`].
pub fn default_client_nicks<I>(nicks: I) -> Box<dyn NickGen>
where
    I: IntoIterator<Item = Nick<'static>>,
    I::IntoIter: 'static + Send,
{
    use crate::client::nick::{from_iter, NickGenExt};
    if let Some(nicks) = from_iter(nicks) {
        Box::new(nicks).chain_using(&NT_FALLBACK)
    } else {
        Box::new(Nick::from_str("Guest")).chain_using(&NT_GUEST)
    }
}

/// For use with [`Register`].
pub fn default_client_username(username: Option<&User<'static>>) -> User<'static> {
    if let Some(uname) = username {
        return uname.clone();
    }
    #[cfg(feature = "whoami")]
    {
        let mut id = crate::util::mangle(&(whoami::username(), whoami::realname()));
        id = (id >> 16) ^ (id & 0xFFFF);
        return User::from_id_short(id as u16);
    }
    #[allow(unreachable_code)]
    User::from_str("user")
}

/// For use with [`Register`].
pub fn default_client_realname(realname: Option<&Line<'static>>) -> Line<'static> {
    realname.cloned().unwrap_or_else(|| Line::from_str("???"))
}

/// For use with [`Register`].
pub fn default_bot_nicks<I>(nicks: I) -> Box<dyn NickGen>
where
    I: IntoIterator<Item = Nick<'static>>,
    I::IntoIter: 'static + Send,
{
    use crate::client::nick::{from_iter, NickGenExt, NickTransformer};
    let vzbnicks = NT_BOT.transform(Nick::from_str("VNZB"));
    if let Some(usernicks) = from_iter(nicks) {
        Box::new(usernicks).chain(vzbnicks)
    } else {
        vzbnicks
    }
}

/// For use with [`Register`].
pub fn default_bot_username(username: Option<&User<'static>>) -> User<'static> {
    username.cloned().unwrap_or_else(|| User::from_str("vnzb_bot"))
}

/// For use with [`Register`].
pub fn default_bot_realname(realname: Option<&Line<'static>>) -> Line<'static> {
    realname.cloned().unwrap_or_else(|| Line::from_str("Vinezombie Bot"))
}

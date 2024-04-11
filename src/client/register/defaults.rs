use super::Register;
use crate::{
    client::{
        auth::{AnySasl, Sasl, SaslQueue, Secret},
        nick::{NickGen, Suffix, SuffixStrategy, SuffixType},
    },
    error::InvalidString,
    string::{Arg, Key, Line, Nick, User},
};
use std::collections::BTreeSet;

/// Connection registration options.
///
/// These cover the options the majority of users will find useful for connection registration.
/// It is (de)serializable if the chosen [`Secret`] and [`Sasl`] implementations are.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Options<S, A = AnySasl<S>> {
    /// The server password.
    pub pass: Option<S>,
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
        let require_sasl = !self.allow_sasl_fail;
        (self.sasl.iter().collect(), require_sasl)
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

impl<S: Secret, A> Options<S, A> {
    /// Uses the provided password.
    pub fn set_pass<'a>(
        &mut self,
        pass: impl TryInto<Line<'a>, Error = impl Into<InvalidString>>,
    ) -> std::io::Result<()> {
        let pass = pass.try_into().map_err(|e| e.into())?.secret();
        let secret = S::new(pass.into())?;
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
    password: fn(&O) -> Option<std::io::Result<Line<'static>>>,
    username: fn(&O) -> User<'static>,
    realname: fn(&O) -> Line<'static>,
    nicks: fn(&O) -> Box<dyn crate::client::nick::NickGen>,
    caps: fn(&O) -> BTreeSet<Key<'static>>,
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
pub fn register_as_client<S: Secret, A: Sasl>() -> Register<Options<S, A>> {
    register_as_custom(
        |opts| default_password(opts.pass.as_ref()),
        |opts| default_client_username(opts.username.as_ref()),
        |opts| default_client_realname(opts.realname.as_ref()),
        |opts| default_client_nicks(opts.nicks.clone()),
        |opts| default_caps().union(&opts.caps).cloned().collect(),
        Options::auths,
    )
}

/// Returns a [`Register`] with sensible functions for bots.
pub fn register_as_bot<S: Secret, A: Sasl>() -> Register<Options<S, A>> {
    register_as_custom(
        |opts| default_password(opts.pass.as_ref()),
        |opts| default_bot_username(opts.username.as_ref()),
        |opts| default_bot_realname(opts.realname.as_ref()),
        |opts| default_bot_nicks(opts.nicks.clone()),
        |opts| opts.caps.clone(),
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
        pub fn default_caps() -> &'static BTreeSet<Key<'static>> {
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
pub fn default_password(
    pass: Option<&(impl Secret + ?Sized)>,
) -> Option<std::io::Result<Line<'static>>> {
    pass.map(|pass| {
        let mut secret = Vec::new();
        pass.load(&mut secret)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::PermissionDenied, e))?;
        Line::from_secret(secret)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
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
        return User::new_id_short(id as u16);
    }
    #[allow(unreachable_code)]
    User::from_str("user")
}

/// For use with [`Register`].
pub fn default_client_realname(realname: Option<&Line<'static>>) -> Line<'static> {
    realname.cloned().unwrap_or_else(|| Line::from_str("???".into()))
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

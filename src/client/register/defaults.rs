use super::{Register, SaslOptions};
use crate::{
    client::{
        auth::{AnySasl, Sasl, Secret},
        nick::{NickGen, Suffix, SuffixStrategy, SuffixType},
    },
    error::InvalidString,
    state::serverinfo::ISupportParser,
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
pub fn register_as_custom<O, A: Sasl>(
    password: fn(&O) -> std::io::Result<Option<Line<'static>>>,
    username: fn(&O) -> User<'static>,
    realname: fn(&O) -> Line<'static>,
    nicks: fn(&O) -> Box<dyn crate::client::nick::NickGen>,
    caps: fn(&O) -> &BTreeSet<Key<'static>>,
    auth: fn(&O) -> SaslOptions<'_, A>,
) -> Register<O, A> {
    Register {
        password,
        username,
        user_p1: default_user_p1,
        user_p2: default_user_p2,
        realname,
        nicks,
        caps,
        auth,
        isupport_parser: ISupportParser::global(),
    }
}

/// Returns a [`Register`] with sensible functions for human-oriented clients.
pub fn register_as_client<S: Secret, A: Sasl>() -> Register<Options<S, A>, A> {
    register_as_custom(
        default_password,
        default_client_username,
        default_client_realname,
        default_client_nicks,
        default_caps,
        default_auth,
    )
}

/// Returns a [`Register`] with sensible functions for bots.
pub fn register_as_bot<S: Secret, A: Sasl>() -> Register<Options<S, A>, A> {
    register_as_custom(
        default_password,
        default_bot_username,
        default_bot_realname,
        default_bot_nicks,
        default_caps,
        default_auth,
    )
}

static DEFAULT_CAPS: std::sync::OnceLock<BTreeSet<Key<'static>>> = std::sync::OnceLock::new();

macro_rules! make_default_caps {
    ($($name:literal,)+) => {
        #[doc="For use with [`Register`]."]
        #[doc="Returns a large set of IRCv3 capabilities."]
        #[doc=""]
        #[doc="Every handler provided by this library can handle these capabilities."]
        #[doc="The specific capabilities included are:"]
        $(#[doc=concat!(" `",$name,"`")])+
        pub fn default_caps<O>(_: &O) -> &'static BTreeSet<Key<'static>> {
            DEFAULT_CAPS.get_or_init(|| {
                [$(Key::from_str($name)),+].into_iter().collect()
            })
        }
    }
}

// Add batch when we actually have a batch handler type because handling it otherwise is pain.
// Request sasl as-needed for auth.
make_default_caps! {
    "account-notify",
    "account-tag",
    "chghost",
    "echo-message",
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
pub fn default_password<S: Secret, A>(
    opts: &Options<S, A>,
) -> std::io::Result<Option<Line<'static>>> {
    let Some(pass) = opts.pass.as_ref() else {
        return Ok(None);
    };
    let mut secret = Vec::new();
    pass.load(&mut secret)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::PermissionDenied, e))?;
    let pass = Line::from_secret(secret)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    Ok(Some(pass))
}

/// For use with [`Register`].
pub fn default_auth<S, A>(opts: &Options<S, A>) -> SaslOptions<'_, A> {
    let require_sasl = !opts.allow_sasl_fail;
    (Box::new(opts.sasl.iter()), require_sasl)
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
    crate::consts::STAR.into()
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
pub fn default_client_nicks<S, A>(opts: &Options<S, A>) -> Box<dyn NickGen> {
    use crate::client::nick::{from_iter, NickGenExt};
    if let Some(nicks) = from_iter(opts.nicks.clone()) {
        Box::new(nicks).chain_using(&NT_FALLBACK)
    } else {
        Box::new(Nick::from_str("Guest")).chain_using(&NT_GUEST)
    }
}

/// For use with [`Register`].
pub fn default_client_username<O>(_: &O) -> User<'static> {
    #[cfg(feature = "whoami")]
    return User::new_id(crate::util::mangle(&whoami::username()));
    #[allow(unreachable_code)]
    unsafe {
        User::from_unchecked("user".into())
    }
}

/// For use with [`Register`].
pub fn default_client_realname<S, A>(opts: &Options<S, A>) -> Line<'static> {
    opts.realname.clone().unwrap_or_else(|| unsafe { Line::from_unchecked("???".into()) })
}

/// For use with [`Register`].
pub fn default_bot_nicks<S, A>(opts: &Options<S, A>) -> Box<dyn NickGen> {
    use crate::client::nick::{from_iter, NickGenExt, NickTransformer};
    let vzbnicks = NT_BOT.transform(Nick::from_str("VNZB"));
    if let Some(usernicks) = from_iter(opts.nicks.clone()) {
        Box::new(usernicks).chain(vzbnicks)
    } else {
        vzbnicks
    }
}

/// For use with [`Register`].
pub fn default_bot_username<O>(_: &O) -> User<'static> {
    User::from_str("vnzb_bot")
}

/// For use with [`Register`].
pub fn default_bot_realname<O>(_: &O) -> Line<'static> {
    Line::from_str("Vinezombie Bot")
}

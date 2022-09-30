use crate::{
    sasl::{External, Plain},
    IrcStr, IrcWord,
};

/// Enum of included SASL mechanisms and options for them.
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub enum AnySasl<S> {
    External(External),
    Plain(Plain<S>),
}

impl<S> From<External> for AnySasl<S> {
    fn from(value: External) -> Self {
        AnySasl::External(value)
    }
}

impl<S> From<Plain<S>> for AnySasl<S> {
    fn from(value: Plain<S>) -> Self {
        AnySasl::Plain(value)
    }
}

/// Connection registration options.
///
/// These are used to create the messages sent during the initial connection registration phase,
/// such as USER and NICK.
#[derive(Clone, Debug)]
pub struct Register<N, S> {
    /// The nickname generator.
    pub nicks: N,
    /// The username, historically one's local account name.
    pub username: IrcWord<'static>,
    /// The realname, also sometimes known as the gecos.
    pub realname: IrcStr<'static>,
    /// A list of SASL authenticators.
    pub sasl: Vec<AnySasl<S>>,
}

impl<N, S> Register<N, S> {
    /// Creates a new `Register` using the provided nickname generator.
    pub fn new(nicks: N) -> Self {
        let username: IrcWord<'static> = unsafe { IrcWord::new_unchecked("user") };
        #[cfg(feature = "whoami")]
        let username: IrcWord<'static> = IrcWord::new(whoami::username()).unwrap_or(username);
        let realname: IrcStr<'static> = "???".into();
        #[cfg(feature = "whoami")]
        let realname: IrcStr<'static> = whoami::realname().try_into().unwrap_or(realname);
        Register { nicks, username, realname, sasl: Vec::new() }
    }
    /// Adds a SASL authenticator.
    pub fn add_sasl(&mut self, sasl: impl Into<AnySasl<S>>) {
        self.sasl.push(sasl.into())
    }
}

impl<N: Default, S> Default for Register<N, S> {
    fn default() -> Self {
        Register::<N, S>::new(Default::default())
    }
}

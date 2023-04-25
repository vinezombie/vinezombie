use crate::{
    auth::AnySasl,
    string::{Line, User},
};

/// Connection registration options.
///
/// These are used to create the messages sent during the initial connection registration phase,
/// such as USER and NICK.
/// These options are limited to a subset of the possibilities that
/// are known to be serializeable.
#[derive(Clone, Debug)]
pub struct Register<N, S> {
    /// The nickname generator.
    pub nicks: N,
    /// The username, historically one's local account name.
    pub username: User<'static>,
    /// The realname, also sometimes known as the gecos.
    pub realname: Line<'static>,
    /// A list of SASL authenticators.
    pub sasl: Vec<AnySasl<S>>,
}

impl<N, S> Register<N, S> {
    /// Creates a new `Register` using the provided nickname generator.
    pub fn new(nicks: N) -> Self {
        let username: User<'static> = unsafe { User::from_unchecked("user".into()) };
        #[cfg(feature = "whoami")]
        let username: User<'static> = User::from_bytes(whoami::username()).unwrap_or(username);
        let realname: Line<'static> = unsafe { Line::from_unchecked("???".into()) };
        #[cfg(feature = "whoami")]
        let realname: Line<'static> = Line::from_bytes(whoami::realname()).unwrap_or(realname);
        Register { nicks, username, realname, sasl: Vec::new() }
    }
    /// Adds a SASL authenticator.
    pub fn add_sasl(&mut self, sasl: impl Into<AnySasl<S>>) {
        self.sasl.push(sasl.into());
    }
}

impl<N: Default, S> Default for Register<N, S> {
    fn default() -> Self {
        Register::<N, S>::new(Default::default())
    }
}

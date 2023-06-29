//! Options for connecting to IRC servers.

mod sync;

pub use sync::*;

use crate::string::Word;

/// The minimal config necessary to connect to an IRC server.
///
/// This subset of options is typically all that is trivially configurable
/// when using WebSocket gateways and bouncers.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ServerAddr<'a> {
    // Can't use Arg here because of `::1`.
    /// The address to connect to.
    pub address: Word<'a>,
    /// Whether to use TLS.
    pub tls: bool,
    /// An optional port number if a non-default one should be used.
    pub port: Option<u16>,
}

impl<'a> PartialEq for ServerAddr<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.tls == other.tls
            && self.port_num() == other.port_num()
            && self.address == other.address
    }
}

impl<'a> Eq for ServerAddr<'a> {}

impl<'a> std::hash::Hash for ServerAddr<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write(self.address.as_bytes());
        state.write_u8(self.tls as u8);
        state.write_u16(self.port_num());
    }
}

impl<'a> ServerAddr<'a> {
    /// Creates a new `ServerAddr` with `tls = true` and a default port number.
    pub fn from_host<A: TryInto<Word<'a>>>(address: A) -> Result<Self, A::Error> {
        let address = address.try_into()?;
        Ok(Self {
            address, tls: true, port: None
        })
    }
    /// As [`ServerAddr::from_host`] but is `const` and panics on invalid input.
    pub const fn from_host_str(address: &'a str) -> Self {
        let address = Word::from_str(address);
        Self {
            address, tls: true, port: None
        }
    }
    /// Returns a string representation of self.
    pub fn to_word(&self) -> Word<'static> {
        use std::io::Write;
        let mut vec = Vec::with_capacity(self.address.len() + 9);
        vec.extend_from_slice(self.address.as_bytes());
        let _ = write!(vec, ":{}{}", if self.tls { "+" } else { "" }, self.port_num());
        // TODO: We're pretty UTF-8 safe here.
        unsafe { Word::from_unchecked(vec.into()) }
    }
    /// Returns the port number that should be used for connecting to the network.
    pub const fn port_num(&self) -> u16 {
        if let Some(no) = self.port {
            no
        } else if self.tls {
            6697
        } else {
            6667
        }
    }
}

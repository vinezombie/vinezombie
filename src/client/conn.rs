//! Options for connecting to IRC servers.

mod sync;
#[cfg(feature = "tokio")]
mod tokio;

#[cfg(feature = "tokio")]
pub use self::tokio::*;
pub use sync::*;

use crate::string::{Builder, Word};

/// Smallest power of two larger than the largest IRCv3 message.
const BUFSIZE: usize = 16384;

/// The minimal config necessary to connect to an IRC server.
///
/// This subset of options is typically all that is trivially configurable
/// when using WebSocket gateways and bouncers.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
pub struct ServerAddr<'a> {
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
    fn utf8_address(&self) -> std::io::Result<&str> {
        self.address.to_utf8().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "non-utf8 address")
        })
    }
    /// Creates a new `ServerAddr` with `tls = true` and a default port number.
    pub fn from_host<A: TryInto<Word<'a>>>(address: A) -> Result<Self, A::Error> {
        let address = address.try_into()?;
        Ok(Self { address, tls: true, port: None })
    }
    /// As [`ServerAddr::from_host`] but is `const` and panics on invalid input.
    pub const fn from_host_str(address: &'a str) -> Self {
        let address = Word::from_str(address);
        Self { address, tls: true, port: None }
    }
    /// Returns a string representation of self.
    pub fn to_word(&self) -> Word<'static> {
        let mut builder = Builder::<Word<'static>>::default();
        builder.reserve_exact(self.address.len() + 9);
        builder.append(self.address.clone());
        if self.tls {
            builder.append(crate::consts::PLUS);
        }
        unsafe {
            // TODO: Method for appending integers to a builder.
            builder.append_unchecked(self.port_num().to_string(), true);
        }
        builder.build()
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

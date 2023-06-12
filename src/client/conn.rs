//! Options for connecting to IRC servers.

use crate::string::Word;

/// The minimal config necessary to connect to an IRC server.
///
/// This subset of options is typically all that is trivially configurable
/// when using WebSocket gateways and bouncers.
#[derive(Clone, Debug)]
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

impl<'a> ServerAddr<'a> {
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

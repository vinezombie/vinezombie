//! Options for connecting to IRC servers.

use std::time::Duration;

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

/// Types that are usable as synchronous connections.
pub trait Connection {
    /// This type as a [`BufRead`][std::io::BufRead].
    type BufRead: std::io::BufRead;
    /// This type as a [`Write`][std::io::Write].
    type Write: std::io::Write;
    /// Returns self as a `BufRead`.
    fn as_bufread(&mut self) -> &mut Self::BufRead;
    /// Returns self as a `Write`.
    fn as_write(&mut self) -> &mut Self::Write;
    /// Sets the read timeout for this connection.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()>;
}

#[cfg(feature = "native")]
impl Connection for std::io::BufReader<std::net::TcpStream> {
    type BufRead = Self;

    type Write = std::net::TcpStream;

    fn as_bufread(&mut self) -> &mut Self::BufRead {
        self
    }

    fn as_write(&mut self) -> &mut Self::Write {
        self.get_mut()
    }

    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.get_ref().set_read_timeout(timeout)
    }
}

#[cfg(feature = "tls")]
impl<
        'a,
        S: rustls::SideData,
        C: 'a + std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
    > Connection for std::io::BufReader<rustls::Stream<'a, C, std::net::TcpStream>>
{
    type BufRead = Self;

    type Write = rustls::Stream<'a, C, std::net::TcpStream>;

    fn as_bufread(&mut self) -> &mut Self::BufRead {
        self
    }

    fn as_write(&mut self) -> &mut Self::Write {
        self.get_mut()
    }

    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.get_ref().sock.set_read_timeout(timeout)
    }
}

#[cfg(feature = "tls")]
impl<
        S: rustls::SideData,
        C: std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
    > Connection for std::io::BufReader<rustls::StreamOwned<C, std::net::TcpStream>>
{
    type BufRead = Self;

    type Write = rustls::StreamOwned<C, std::net::TcpStream>;

    fn as_bufread(&mut self) -> &mut Self::BufRead {
        self
    }

    fn as_write(&mut self) -> &mut Self::Write {
        self.get_mut()
    }

    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.get_ref().sock.set_read_timeout(timeout)
    }
}

use std::{io::BufReader, net::TcpStream, time::Duration};

impl<'a> super::ServerAddr<'a> {
    /// Creates a synchronous connection, ignoring the `tls` flag.
    pub fn connect_no_tls(&self) -> std::io::Result<BufReader<Stream>> {
        let string = self.address.as_str();
        let sock = std::net::TcpStream::connect((string, self.port_num()))?;
        Ok(BufReader::with_capacity(super::BUFSIZE, Stream(StreamInner::Tcp(sock))))
    }
    /// Creates a synchronous connection.
    #[cfg(feature = "tls")]
    pub fn connect(
        &self,
        config: std::sync::Arc<rustls::ClientConfig>,
    ) -> std::io::Result<BufReader<Stream>> {
        use std::io::{Error, ErrorKind};
        let string = self.address.as_str();
        let stream = if self.tls {
            use std::io::Write;
            let name = rustls::ServerName::try_from(string)
                .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
            let conn = rustls::ClientConnection::new(config, name)
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
            let sock = std::net::TcpStream::connect((string, self.port_num()))?;
            let mut tls = rustls::StreamOwned { conn, sock };
            tls.flush()?;
            StreamInner::Tls(Box::new(tls))
        } else {
            let sock = std::net::TcpStream::connect((string, self.port_num()))?;
            StreamInner::Tcp(sock)
        };
        Ok(BufReader::with_capacity(super::BUFSIZE, Stream(stream)))
    }
}

/// An abstraction of common I/O stream types.
#[derive(Debug)]
pub struct Stream(StreamInner);

#[derive(Debug, Default)]
enum StreamInner {
    #[default]
    Closed,
    Tcp(TcpStream),
    #[cfg(feature = "tls")]
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Stream {
    /// Shuts down the read, write, or both halves of this connection,
    /// as [`TcpStream::shutdown`].
    pub fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
        // TODO: Maybe intercept NotConnected?
        match &self.0 {
            StreamInner::Closed => Ok(()),
            StreamInner::Tcp(s) => s.shutdown(how),
            #[cfg(feature = "tls")]
            StreamInner::Tls(s) => s.sock.shutdown(how),
        }
    }
    /// Sets the read timeout for this stream,
    /// as [`TcpStream::set_read_timeout`].
    ///
    /// Errors if the provided duration is zero.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        match &self.0 {
            StreamInner::Closed => Ok(()),
            StreamInner::Tcp(s) => s.set_read_timeout(timeout),
            #[cfg(feature = "tls")]
            StreamInner::Tls(s) => s.sock.set_read_timeout(timeout),
        }
    }
    /// Sets the write timeout for this stream,
    /// as [`TcpStream::set_write_timeout`].
    ///
    /// Errors if the provided duration is zero.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        match &self.0 {
            StreamInner::Closed => Ok(()),
            StreamInner::Tcp(s) => s.set_write_timeout(timeout),
            #[cfg(feature = "tls")]
            StreamInner::Tls(s) => s.sock.set_write_timeout(timeout),
        }
    }
}

impl std::io::Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.0 {
            StreamInner::Closed => Ok(0),
            StreamInner::Tcp(s) => s.read(buf),
            #[cfg(feature = "tls")]
            StreamInner::Tls(s) => s.read(buf),
        }
    }

    fn read_vectored(&mut self, bufs: &mut [std::io::IoSliceMut<'_>]) -> std::io::Result<usize> {
        match &mut self.0 {
            StreamInner::Closed => Ok(0),
            StreamInner::Tcp(s) => s.read_vectored(bufs),
            #[cfg(feature = "tls")]
            StreamInner::Tls(s) => s.read_vectored(bufs),
        }
    }
}

impl std::io::Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match &mut self.0 {
            StreamInner::Closed => Ok(0),
            StreamInner::Tcp(s) => s.write(buf),
            #[cfg(feature = "tls")]
            StreamInner::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.0 {
            StreamInner::Closed => Ok(()),
            StreamInner::Tcp(s) => s.flush(),
            #[cfg(feature = "tls")]
            StreamInner::Tls(s) => s.flush(),
        }
    }
}

/// [`Read`][std::io::Read]s with configurable timeouts.
pub trait SetReadTimeout: std::io::Read {
    /// Sets the read timeout for this connection.
    ///
    /// May error if a duration of zero is provided to `timeout`.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()>;
}

impl SetReadTimeout for TcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        Self::set_read_timeout(self, timeout)
    }
}

impl SetReadTimeout for Stream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        Self::set_read_timeout(self, timeout)
    }
}

#[cfg(feature = "tls")]
impl<
        'a,
        S: rustls::SideData,
        C: 'a + std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
        T: SetReadTimeout + std::io::Write,
    > SetReadTimeout for rustls::Stream<'a, C, T>
{
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_read_timeout(timeout)
    }
}

#[cfg(feature = "tls")]
impl<
        S: rustls::SideData,
        C: std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
        T: SetReadTimeout + std::io::Write,
    > SetReadTimeout for rustls::StreamOwned<C, T>
{
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_read_timeout(timeout)
    }
}

/// Types that are usable as synchronous connections.
pub trait Connection: SetReadTimeout {
    /// This type as a [`BufRead`][std::io::BufRead].
    type BufRead: std::io::BufRead;
    /// This type as a [`Write`][std::io::Write].
    type Write: std::io::Write;
    /// Returns self as a `BufRead`.
    fn as_bufread(&mut self) -> &mut Self::BufRead;
    /// Returns self as a `Write`.
    fn as_write(&mut self) -> &mut Self::Write;
}

impl<T: SetReadTimeout> SetReadTimeout for BufReader<T> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.get_mut().set_read_timeout(timeout)
    }
}

impl<T: SetReadTimeout + std::io::Write> Connection for BufReader<T> {
    type BufRead = Self;

    type Write = T;

    fn as_bufread(&mut self) -> &mut Self::BufRead {
        self
    }

    fn as_write(&mut self) -> &mut Self::Write {
        self.get_mut()
    }
}

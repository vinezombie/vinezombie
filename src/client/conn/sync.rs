use super::IoTimeout;
use crate::{client::tls::TlsConfig, ircmsg::ServerMsg};
use std::{io::BufReader, net::TcpStream, time::Duration};

impl<'a> super::ServerAddr<'a> {
    /// Creates a synchronous connection, ignoring the `tls` flag.
    pub fn connect_no_tls(&self) -> std::io::Result<BufReader<Stream>> {
        let string = self.utf8_address()?;
        let sock = std::net::TcpStream::connect((string, self.port_num()))?;
        Ok(BufReader::with_capacity(super::BUFSIZE, Stream(StreamInner::Tcp(sock))))
    }
    /// Creates a synchronous connection.
    ///
    /// `tls_fn` is called if a TLS client configuration is needed.
    /// If this function may be called multiple times,
    #[cfg(feature = "tls")]
    pub fn connect(
        &self,
        tls_fn: impl FnOnce() -> std::io::Result<TlsConfig>,
    ) -> std::io::Result<BufReader<Stream>> {
        use std::io::{Error, ErrorKind};
        let string = self.utf8_address()?;
        let stream = if self.tls {
            use std::io::Write;
            let name = rustls::ServerName::try_from(string)
                .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
            let config = tls_fn()?;
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
    /// Returns the read timeout for this stream,
    /// as [`TcpStream::read_timeout`].
    pub fn read_timeout(&self) -> std::io::Result<Option<Duration>> {
        match &self.0 {
            StreamInner::Closed => Ok(None),
            StreamInner::Tcp(s) => s.read_timeout(),
            #[cfg(feature = "tls")]
            StreamInner::Tls(s) => s.sock.read_timeout(),
        }
    }
    /// Returns the write timeout for this stream,
    /// as [`TcpStream::write_timeout`].
    pub fn write_timeout(&self) -> std::io::Result<Option<Duration>> {
        match &self.0 {
            StreamInner::Closed => Ok(None),
            StreamInner::Tcp(s) => s.write_timeout(),
            #[cfg(feature = "tls")]
            StreamInner::Tls(s) => s.sock.write_timeout(),
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

impl IoTimeout for TcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        Self::set_read_timeout(self, timeout)
    }
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        Self::set_write_timeout(self, timeout)
    }
    fn read_timeout(&self) -> std::io::Result<Option<Duration>> {
        Self::read_timeout(self)
    }
    fn write_timeout(&self) -> std::io::Result<Option<Duration>> {
        Self::write_timeout(self)
    }
}

impl IoTimeout for Stream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        Self::set_read_timeout(self, timeout)
    }
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        Self::set_write_timeout(self, timeout)
    }
    fn read_timeout(&self) -> std::io::Result<Option<Duration>> {
        Self::read_timeout(self)
    }
    fn write_timeout(&self) -> std::io::Result<Option<Duration>> {
        Self::write_timeout(self)
    }
}

#[cfg(feature = "tls")]
impl<
        'a,
        S: rustls::SideData,
        C: 'a + std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
        T: IoTimeout + std::io::Read + std::io::Write,
    > IoTimeout for rustls::Stream<'a, C, T>
{
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_read_timeout(timeout)
    }
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_write_timeout(timeout)
    }
    fn read_timeout(&self) -> std::io::Result<Option<Duration>> {
        self.sock.read_timeout()
    }
    fn write_timeout(&self) -> std::io::Result<Option<Duration>> {
        self.sock.write_timeout()
    }
}

#[cfg(feature = "tls")]
impl<
        S: rustls::SideData,
        C: std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
        T: IoTimeout + std::io::Read + std::io::Write,
    > IoTimeout for rustls::StreamOwned<C, T>
{
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_read_timeout(timeout)
    }
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_write_timeout(timeout)
    }
    fn read_timeout(&self) -> std::io::Result<Option<Duration>> {
        self.sock.read_timeout()
    }
    fn write_timeout(&self) -> std::io::Result<Option<Duration>> {
        self.sock.write_timeout()
    }
}

/// Types that are usable as synchronous connections.
pub trait Connection: IoTimeout {
    /// This type as a [`BufRead`][std::io::BufRead].
    type BufRead: std::io::BufRead;
    /// This type as a [`Write`][std::io::Write].
    type Write: std::io::Write;
    /// Returns self as a `BufRead`.
    fn as_bufread(&mut self) -> &mut Self::BufRead;
    /// Returns self as a `Write`.
    fn as_write(&mut self) -> &mut Self::Write;
}

impl<T: IoTimeout> IoTimeout for BufReader<T> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.get_mut().set_read_timeout(timeout)
    }
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.get_mut().set_write_timeout(timeout)
    }
    fn read_timeout(&self) -> std::io::Result<Option<Duration>> {
        self.get_ref().read_timeout()
    }
    fn write_timeout(&self) -> std::io::Result<Option<Duration>> {
        self.get_ref().write_timeout()
    }
}

impl<T: IoTimeout + std::io::Read + std::io::Write> Connection for BufReader<T> {
    type BufRead = Self;

    type Write = T;

    fn as_bufread(&mut self) -> &mut Self::BufRead {
        self
    }

    fn as_write(&mut self) -> &mut Self::Write {
        self.get_mut()
    }
}

impl<C: Connection, A: crate::client::adjuster::Adjuster> crate::client::Client<C, A> {
    /// Reads a message from the server, adjusting the queue if necessary.
    pub fn read_owning(&mut self) -> std::io::Result<ServerMsg<'static>> {
        let msg = ServerMsg::read_owning_from(self.conn.as_bufread(), &mut self.buf)?;
        self.queue.adjust(&msg, &mut self.adjuster);
        Ok(msg)
    }
    /// Flushes the queue until it's empty or blocks.
    ///
    /// I/O failure should be considered non-recoverable,
    /// as any messages that were removed from the queue will be lost.
    pub fn flush(&mut self) -> std::io::Result<Option<Duration>> {
        use std::io::Write;
        let mut timeout = None;
        while let Some(popped) = self.queue.pop(|new_timeout| timeout = new_timeout) {
            let _ = popped.write_to(&mut self.buf);
        }
        let result = self.conn.as_write().write_all(&self.buf);
        self.buf.clear();
        result?;
        self.conn.as_write().flush()?;
        Ok(timeout)
    }
}

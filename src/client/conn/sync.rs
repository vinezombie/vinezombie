use super::{filter_time_error, ReadTimeout, TimeLimitedSync, WriteTimeout};
use crate::ircmsg::ServerMsg;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    time::Duration,
};

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
    /// the client configuration should be stored outside of the closure.
    #[cfg(feature = "tls")]
    pub fn connect(
        &self,
        tls_fn: impl FnOnce() -> std::io::Result<crate::client::tls::TlsConfig>,
    ) -> std::io::Result<BufReader<Stream>> {
        use std::io::{Error, ErrorKind};
        let string = self.utf8_address()?;
        let stream = if self.tls {
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

impl Read for Stream {
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

impl Write for Stream {
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

impl ReadTimeout for TcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        Self::set_read_timeout(self, timeout)
    }
}
impl WriteTimeout for TcpStream {
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        Self::set_write_timeout(self, timeout)
    }
}

impl ReadTimeout for Stream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        Self::set_read_timeout(self, timeout)
    }
}
impl WriteTimeout for Stream {
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        Self::set_write_timeout(self, timeout)
    }
}

#[cfg(feature = "tls")]
impl<
        'a,
        S: rustls::SideData,
        C: 'a + std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
        T: ReadTimeout + Read + Write,
    > ReadTimeout for rustls::Stream<'a, C, T>
{
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_read_timeout(timeout)
    }
}

#[cfg(feature = "tls")]
impl<
        'a,
        S: rustls::SideData,
        C: 'a + std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
        T: WriteTimeout + Read + Write,
    > WriteTimeout for rustls::Stream<'a, C, T>
{
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_write_timeout(timeout)
    }
}

#[cfg(feature = "tls")]
impl<
        S: rustls::SideData,
        C: std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
        T: ReadTimeout + Read + Write,
    > ReadTimeout for rustls::StreamOwned<C, T>
{
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_read_timeout(timeout)
    }
}

#[cfg(feature = "tls")]
impl<
        S: rustls::SideData,
        C: std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
        T: WriteTimeout + Read + Write,
    > WriteTimeout for rustls::StreamOwned<C, T>
{
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_write_timeout(timeout)
    }
}

/// Types that are usable as synchronous connections.
pub trait Connection: ReadTimeout + WriteTimeout {
    /// This type as a [`BufRead`][std::io::BufRead].
    type BufRead: std::io::BufRead;
    /// This type as a [`Write`][std::io::Write].
    type Write: Write;
    /// Returns self as a `BufRead`.
    fn as_bufread(&mut self) -> &mut Self::BufRead;
    /// Returns self as a `Write`.
    fn as_write(&mut self) -> &mut Self::Write;
}

impl<R: BufRead, W: Write> Connection for super::Bidir<R, W>
where
    super::Bidir<R, W>: ReadTimeout + WriteTimeout,
{
    type BufRead = R;

    type Write = W;

    fn as_bufread(&mut self) -> &mut Self::BufRead {
        &mut self.0
    }

    fn as_write(&mut self) -> &mut Self::Write {
        &mut self.1
    }
}

impl<T: ReadTimeout> ReadTimeout for BufReader<T> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.get_mut().set_read_timeout(timeout)
    }
}

impl<T: WriteTimeout> WriteTimeout for BufReader<T> {
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.get_mut().set_write_timeout(timeout)
    }
}

impl<T: ReadTimeout + WriteTimeout + Read + Write> Connection for BufReader<T> {
    type BufRead = Self;

    type Write = T;

    fn as_bufread(&mut self) -> &mut Self::BufRead {
        self
    }

    fn as_write(&mut self) -> &mut Self::Write {
        self.get_mut()
    }
}

impl<C: Connection> crate::client::Client<C> {
    /// Runs handlers off of the connection until any of them yield or finish.
    ///
    /// Returns the IDs of the handlers that yielded or finished, respectively.
    /// Read timeouts are indicated by a return value of `Ok(None)`.
    /// I/O failure should be considered non-recoverable.
    ///
    /// Handlers are not guaranteed to run in the order they were added.
    /// If there are no handlers to run, fully flushes the queue.
    /// If the `tracing` feature is enabled, logs messages at the debug level.
    pub fn run(&mut self) -> std::io::Result<Option<(&[usize], &[usize])>> {
        let finished_at = loop {
            let wait_for = self.flush_partial()?;
            if self.handlers.is_empty() {
                if let Some(wait_for) = wait_for {
                    std::thread::sleep(wait_for);
                    continue;
                }
                return Ok(Some((Default::default(), Default::default())));
            }
            let (mut conn, should_continue) =
                TimeLimitedSync::new(&mut self.conn, &mut self.timeout, wait_for)?;
            // Unfortunately not quite DRY,
            // but this is the easiest way to sidestep lifetime issues.
            let finished_at = if self.handlers.wants_owning() {
                let msg = ServerMsg::read_owning_from(&mut conn, &mut self.buf_i);
                let Some(msg) = filter_time_error(msg)? else {
                    if should_continue {
                        continue;
                    }
                    return Ok(None);
                };
                #[cfg(feature = "tracing")]
                tracing::debug!(target: "vinezombie::recv", "{}", msg);
                self.queue.adjust(&msg);
                self.handlers.handle(&msg, &mut self.queue)
            } else {
                let msg = ServerMsg::read_borrowing_from(&mut conn, &mut self.buf_i);
                let Some(msg) = filter_time_error(msg)? else {
                    if should_continue {
                        continue;
                    }
                    return Ok(None);
                };
                #[cfg(feature = "tracing")]
                tracing::debug!(target: "vinezombie::recv", "{}", msg);
                self.queue.adjust(&msg);
                let fa = self.handlers.handle(&msg, &mut self.queue);
                self.buf_i.clear();
                fa
            };
            if self.handlers.has_results(finished_at) {
                self.flush_partial()?;
                // You give me conniptions, borrowck.
                break finished_at;
            }
        };
        Ok(Some(self.handlers.last_run_results(finished_at)))
    }
    /// Flushes the queue until it's empty or hits rate limits.
    ///
    /// I/O failure should be considered non-recoverable,
    /// as any messages that were removed from the queue will be lost.
    ///
    /// If the `tracing` feature is enabled, logs messages at the debug level.
    pub fn flush_partial(&mut self) -> std::io::Result<Option<Duration>> {
        if self.queue.is_empty() {
            return Ok(None);
        }
        let mut timeout = None;
        while let Some(popped) = self.queue.pop(|new_timeout| timeout = new_timeout) {
            #[cfg(feature = "tracing")]
            tracing::debug!(target: "vinezombie::send", "{}", popped);
            let _ = popped.write_to(&mut self.buf_o);
            self.buf_o.extend_from_slice(b"\r\n");
        }
        let result = self.conn.as_write().write_all(&self.buf_o);
        self.buf_o.clear();
        result?;
        self.conn.as_write().flush()?;
        Ok(timeout)
    }
}

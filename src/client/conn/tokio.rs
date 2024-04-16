use super::{timed_io, Bidir, TimeLimitedTokio};
use crate::{client::tls::TlsConfig, ircmsg::ServerMsg};
use std::{pin::Pin, time::Duration};
use tokio::{
    io::{AsyncBufRead, AsyncWrite, BufReader},
    net::TcpStream,
};

impl<'a> super::ServerAddr<'a> {
    /// Creates an asynchronous connection, ignoring the `tls` flag.
    pub async fn connect_tokio_no_tls(&self) -> std::io::Result<BufReader<StreamTokio>> {
        let string = self.utf8_address()?;
        let sock = tokio::net::TcpStream::connect((string, self.port_num())).await?;
        Ok(BufReader::with_capacity(super::BUFSIZE, StreamTokio { stream: StreamInner::Tcp(sock) }))
    }
    /// Creates an asynchronous connection.
    ///
    /// `tls_fn` is called if a TLS client configuration is needed.
    /// If this function may be called multiple times,
    /// the client configuration should be stored outside of the closure.
    #[cfg(feature = "tls-tokio")]
    pub async fn connect_tokio(
        &self,
        tls_fn: impl FnOnce() -> std::io::Result<TlsConfig>,
    ) -> std::io::Result<BufReader<StreamTokio>> {
        use std::io::{Error, ErrorKind};
        let string = self.utf8_address()?;
        let stream = if self.tls {
            let name = rustls::ServerName::try_from(string)
                .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
            let config = tls_fn()?;
            let conn: tokio_rustls::TlsConnector = config.into();
            let sock = tokio::net::TcpStream::connect((string, self.port_num())).await?;
            let tls = conn.connect(name, sock).await?;
            StreamInner::Tls(tls)
        } else {
            let sock = tokio::net::TcpStream::connect((string, self.port_num())).await?;
            StreamInner::Tcp(sock)
        };
        Ok(BufReader::with_capacity(super::BUFSIZE, StreamTokio { stream }))
    }
}

/// An abstraction of common I/O stream types.
#[derive(Debug, Default)]
pub struct StreamTokio {
    stream: StreamInner,
}

#[derive(Debug, Default)]
enum StreamInner {
    #[default]
    Closed,
    Tcp(TcpStream),
    #[cfg(feature = "tls-tokio")]
    Tls(tokio_rustls::client::TlsStream<TcpStream>),
}

impl tokio::io::AsyncRead for StreamTokio {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut (self.get_mut()).stream {
            StreamInner::Closed => std::task::Poll::Ready(Ok(())),
            StreamInner::Tcp(tcp) => Pin::new(tcp).poll_read(cx, buf),
            #[cfg(feature = "tls-tokio")]
            StreamInner::Tls(tls) => Pin::new(tls).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for StreamTokio {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match &mut (self.get_mut()).stream {
            StreamInner::Closed => std::task::Poll::Ready(Ok(0)),
            StreamInner::Tcp(tcp) => Pin::new(tcp).poll_write(cx, buf),
            #[cfg(feature = "tls-tokio")]
            StreamInner::Tls(tls) => Pin::new(tls).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match &mut (self.get_mut()).stream {
            StreamInner::Closed => std::task::Poll::Ready(Ok(())),
            StreamInner::Tcp(tcp) => Pin::new(tcp).poll_flush(cx),
            #[cfg(feature = "tls-tokio")]
            StreamInner::Tls(tls) => Pin::new(tls).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match &mut (self.get_mut()).stream {
            StreamInner::Closed => std::task::Poll::Ready(Ok(())),
            StreamInner::Tcp(tcp) => Pin::new(tcp).poll_shutdown(cx),
            #[cfg(feature = "tls-tokio")]
            StreamInner::Tls(tls) => Pin::new(tls).poll_shutdown(cx),
        }
    }
}

// Using named &muts instead of Pins here because it means less an Unpin dance is needed
// to use this in run_handler_tokio.

/// Types that are usable as asynchronous connections
pub trait ConnectionTokio: Unpin {
    /// This type as an [`AsyncBufRead`][tokio::io::AsyncBufRead].
    type AsyncBufRead: tokio::io::AsyncBufRead;
    /// This type as an [`AsyncWrite`][tokio::io::AsyncWrite].
    type AsyncWrite: tokio::io::AsyncWrite + Unpin;
    /// Returns self as an `AsyncBufRead`.
    fn as_bufread(&mut self) -> Pin<&mut Self::AsyncBufRead>;
    /// Returns self as an `AsyncWrite`.
    fn as_write(&mut self) -> &mut Self::AsyncWrite;
}

impl<R: AsyncBufRead + Unpin, W: AsyncWrite + Unpin> ConnectionTokio for Bidir<R, W> {
    type AsyncBufRead = R;

    type AsyncWrite = W;

    fn as_bufread(&mut self) -> Pin<&mut Self::AsyncBufRead> {
        Pin::new(&mut self.0)
    }

    fn as_write(&mut self) -> &mut Self::AsyncWrite {
        &mut self.1
    }
}

impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> ConnectionTokio for BufReader<T> {
    type AsyncBufRead = Self;

    type AsyncWrite = T;

    fn as_bufread(&mut self) -> Pin<&mut Self::AsyncBufRead> {
        Pin::new(self)
    }

    fn as_write(&mut self) -> &mut Self::AsyncWrite {
        self.get_mut()
    }
}

impl<C: ConnectionTokio, S> crate::client::Client<C, S> {
    /// Runs handlers off of the connection until any of them yield or finish.
    ///
    /// Returns the IDs of the handlers that yielded or finished, respectively.
    /// Read timeouts are indicated by a return value of `Ok(None)`.
    /// I/O failure should be considered non-recoverable.
    ///
    /// Handlers are not guaranteed to run in the order they were added.
    /// If there are no handlers to run, fully flushes the queue.
    /// If the `tracing` feature is enabled, logs messages at the debug level.
    pub async fn run_tokio(&mut self) -> std::io::Result<Option<(&[usize], &[usize])>> {
        let finished_at = loop {
            let wait_for = self.flush_partial_tokio().await?;
            if self.handlers.is_empty() {
                if let Some(wait_for) = wait_for {
                    tokio::time::sleep(wait_for).await;
                    continue;
                }
                return Ok(Some((Default::default(), Default::default())));
            }
            let mut conn = TimeLimitedTokio::new(&mut self.conn, &self.timeout);
            // Unfortunately not quite DRY,
            // but this is the easiest way to sidestep lifetime issues.
            let finished_at = if self.handlers.wants_owning() {
                let fut = ServerMsg::read_owning_from_tokio(&mut conn, &mut self.buf_i);
                let msg = match timed_io(fut, wait_for, self.timeout.read_timeout()).await? {
                    Ok(m) => m,
                    Err(true) => continue,
                    Err(false) => return Ok(None),
                };
                #[cfg(feature = "tracing")]
                tracing::debug!(target: "vinezombie::recv", "{}", msg);
                self.queue.adjust(&msg);
                self.handlers.handle(&msg, &mut self.queue)
            } else {
                let fut = ServerMsg::read_borrowing_from_tokio(&mut conn, &mut self.buf_i);
                let msg = match timed_io(fut, wait_for, self.timeout.read_timeout()).await? {
                    Ok(m) => m,
                    Err(true) => continue,
                    Err(false) => return Ok(None),
                };
                #[cfg(feature = "tracing")]
                tracing::debug!(target: "vinezombie::recv", "{}", msg);
                self.queue.adjust(&msg);
                let fa = self.handlers.handle(&msg, &mut self.queue);
                self.buf_i.clear();
                fa
            };
            if self.handlers.has_results(finished_at) {
                self.flush_partial_tokio().await?;
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
    pub async fn flush_partial_tokio(&mut self) -> std::io::Result<Option<Duration>> {
        use tokio::io::AsyncWriteExt;
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
        let mut conn = TimeLimitedTokio::new(&mut self.conn, &self.timeout);
        let result = conn.write_all(&self.buf_o).await;
        self.buf_o.clear();
        result?;
        conn.flush().await?;
        Ok(timeout)
    }
}

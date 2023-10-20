use crate::{client::tls::TlsConfig, ircmsg::ServerMsg};
use std::{pin::Pin, time::Duration};
use tokio::{io::BufReader, net::TcpStream};

impl<'a> super::ServerAddr<'a> {
    /// Creates an asynchronous connection, ignoring the `tls` flag.
    pub async fn connect_tokio_no_tls(&self) -> std::io::Result<BufReader<StreamTokio>> {
        let string = self.utf8_address()?;
        let sock = tokio::net::TcpStream::connect((string, self.port_num())).await?;
        Ok(BufReader::with_capacity(super::BUFSIZE, StreamTokio(StreamInner::Tcp(sock))))
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
        Ok(BufReader::with_capacity(super::BUFSIZE, StreamTokio(stream)))
    }
}

/// An abstraction of common I/O stream types.
#[derive(Debug)]
pub struct StreamTokio(StreamInner);

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
        match &mut (self.get_mut()).0 {
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
        match &mut (self.get_mut()).0 {
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
        match &mut (self.get_mut()).0 {
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
        match &mut (self.get_mut()).0 {
            StreamInner::Closed => std::task::Poll::Ready(Ok(())),
            StreamInner::Tcp(tcp) => Pin::new(tcp).poll_shutdown(cx),
            #[cfg(feature = "tls-tokio")]
            StreamInner::Tls(tls) => Pin::new(tls).poll_shutdown(cx),
        }
    }
}

// Using named &muts instead of Pins here because it means less an Unpin dance is needed
// to use this in run_handler_tokio.

/// Types that are usable as asynchronous connections.
pub trait ConnectionTokio {
    /// This type as an [`AsyncBufRead`][tokio::io::AsyncBufRead].
    type AsyncBufRead: tokio::io::AsyncBufRead + Unpin;
    /// This type as an [`AsyncWrite`][tokio::io::AsyncWrite].
    type AsyncWrite: tokio::io::AsyncWrite + Unpin;
    /// Returns self as an `AsyncBufRead`.
    fn as_bufread(&mut self) -> &mut Self::AsyncBufRead;
    /// Returns self as an `AsyncWrite`.
    fn as_write(&mut self) -> &mut Self::AsyncWrite;
}

impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> ConnectionTokio for BufReader<T> {
    type AsyncBufRead = Self;

    type AsyncWrite = T;

    fn as_bufread(&mut self) -> &mut Self::AsyncBufRead {
        self
    }

    fn as_write(&mut self) -> &mut Self::AsyncWrite {
        self.get_mut()
    }
}

impl<C: ConnectionTokio, A: crate::client::adjuster::Adjuster> crate::client::Client<C, A> {
    /// Reads a message from the server, adjusting the queue if necessary.
    pub async fn read_owning_tokio(&mut self) -> std::io::Result<ServerMsg<'static>> {
        let msg = ServerMsg::read_owning_from_tokio(self.conn.as_bufread(), &mut self.buf).await?;
        self.queue.adjust(&msg, &mut self.adjuster);
        Ok(msg)
    }
    /// Flushes the queue until it's empty or blocks.
    pub async fn flush_tokio(&mut self) -> std::io::Result<Option<Duration>> {
        use tokio::io::AsyncWriteExt;
        let mut timeout = None;
        while let Some(popped) = self.queue.pop(|new_timeout| timeout = new_timeout) {
            let _ = popped.write_to(&mut self.buf);
        }
        let result = self.conn.as_write().write_all(&self.buf).await;
        self.buf.clear();
        result?;
        self.conn.as_write().flush().await?;
        Ok(timeout)
    }
}

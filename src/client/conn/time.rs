use std::time::{Duration, Instant};

/// I/O types with a read timeout. Mainly used for sync I/O.
pub trait ReadTimeout {
    /// Sets the read timeout for this connection.
    ///
    /// May error if a duration of zero is provided to `timeout`.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()>;
}

/// I/O types with a write timeout. Mainly used for sync I/O.
pub trait WriteTimeout {
    /// Sets the write timeout for this connection.
    ///
    /// May error if a duration of zero is provided to `timeout`.
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()>;
}

impl ReadTimeout for std::io::Empty {
    fn set_read_timeout(&mut self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}
impl<T: AsRef<[u8]>> ReadTimeout for std::io::Cursor<T> {
    fn set_read_timeout(&mut self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}
impl ReadTimeout for std::collections::VecDeque<u8> {
    fn set_read_timeout(&mut self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}
impl<R: ReadTimeout, W> ReadTimeout for super::Bidir<R, W> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.0.set_read_timeout(timeout)
    }
}

impl WriteTimeout for std::io::Empty {
    fn set_write_timeout(&mut self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}
impl WriteTimeout for std::io::Sink {
    fn set_write_timeout(&mut self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}
impl WriteTimeout for Vec<u8> {
    fn set_write_timeout(&mut self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}
impl WriteTimeout for std::collections::VecDeque<u8> {
    fn set_write_timeout(&mut self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}
impl<R, W: WriteTimeout> WriteTimeout for super::Bidir<R, W> {
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.1.set_write_timeout(timeout)
    }
}

/// Wrapper that allows any type to implement [`ReadTimeout`] and [`WriteTimeout`] by ignoring
/// requests to change the timeout.
///
/// This is correct to use for any tipe that never blocks/yields on read or write,
/// but can also be used on types that do if you don't mind violently incorrect behavior.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NoTimeout<T>(pub T);

impl<T> ReadTimeout for NoTimeout<T> {
    fn set_read_timeout(&mut self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}

impl<T> WriteTimeout for NoTimeout<T> {
    fn set_write_timeout(&mut self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}

fn timeout_fallback(
    new_timeout: Option<Duration>,
    old_timeout: Option<Duration>,
) -> (Option<Duration>, bool) {
    if new_timeout.is_some() {
        (new_timeout, true)
    } else {
        (old_timeout, false)
    }
}

pub(super) fn filter_time_error<T>(result: std::io::Result<T>) -> std::io::Result<Option<T>> {
    use std::io::ErrorKind;
    match result {
        Ok(v) => Ok(Some(v)),
        Err(e) => match e.kind() {
            ErrorKind::TimedOut | ErrorKind::WouldBlock => Ok(None),
            _ => Err(e),
        },
    }
}

#[derive(Default)]
pub(crate) struct TimeLimits {
    read: Option<Duration>,
    write: Option<Duration>,
    /// Whether the write timeout on a stream needs updating. Unused for async.
    update_write: bool,
}

impl TimeLimits {
    pub fn read_timeout(&self) -> Option<Duration> {
        self.read
    }
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.read = timeout.map(|t| std::cmp::max(t, Duration::from_secs(1)));
        self
    }
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.update_write |= true;
        self.write = timeout.map(|t| std::cmp::max(t, Duration::from_secs(1)));
        self
    }
}

pub(super) struct TimeLimitedSync<'a, C> {
    conn: &'a mut C,
    read: Option<Instant>,
}

impl<'a, C: super::Connection> TimeLimitedSync<'a, C> {
    pub fn new(
        conn: &'a mut C,
        timeouts: &mut TimeLimits,
        write_after: Option<Duration>,
    ) -> std::io::Result<(Self, bool)> {
        let TimeLimits { read, write, update_write, .. } = *timeouts;
        if update_write {
            conn.set_write_timeout(write)?;
            timeouts.update_write = false;
        }
        let (read, constrained) = timeout_fallback(write_after, read);
        let read = read.and_then(|dur| Instant::now().checked_add(dur));
        Ok((TimeLimitedSync { conn, read }, constrained))
    }
    fn update_timeout(&mut self) -> std::io::Result<()> {
        if let Some(deadline) = self.read {
            let duration = deadline.saturating_duration_since(Instant::now());
            if duration != Duration::ZERO {
                self.conn.set_read_timeout(Some(duration))?;
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "collective read time limit elapsed",
                ));
            }
        }
        Ok(())
    }
}

impl<'a, C: super::Connection> std::io::Write for TimeLimitedSync<'a, C> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.conn.as_write().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.conn.as_write().flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.conn.as_write().write_all(buf)
    }

    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        self.conn.as_write().write_fmt(fmt)
    }
}

impl<'a, C: super::Connection> std::io::Read for TimeLimitedSync<'a, C> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.update_timeout()?;
        self.conn.as_bufread().read(buf)
    }
}

impl<'a, C: super::Connection> std::io::BufRead for TimeLimitedSync<'a, C> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.update_timeout()?;
        self.conn.as_bufread().fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.conn.as_bufread().consume(amt);
    }
}

#[cfg(feature = "tokio")]
pub(super) struct TimeLimitedTokio<'a, C> {
    conn: &'a mut C,
    write: Option<Duration>,
}

#[cfg(feature = "tokio")]
impl<'a, C: super::ConnectionTokio> TimeLimitedTokio<'a, C> {
    pub fn new(conn: &'a mut C, timeouts: &TimeLimits) -> Self {
        TimeLimitedTokio { conn, write: timeouts.write }
    }
    fn poll_write_timeout(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Error> {
        if let Some(timeout) = self.write {
            use std::future::Future;
            let sleeper = std::pin::pin!(tokio::time::sleep(timeout));
            if sleeper.poll(cx).is_ready() {
                return std::task::Poll::Ready(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "async write time limit elapsed",
                ));
            }
        }
        std::task::Poll::Pending
    }
}

#[cfg(feature = "tokio")]
impl<'a, C: super::ConnectionTokio> tokio::io::AsyncRead for TimeLimitedTokio<'a, C> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.conn.as_bufread().poll_read(cx, buf)
    }
}

#[cfg(feature = "tokio")]
impl<'a, C: super::ConnectionTokio> tokio::io::AsyncBufRead for TimeLimitedTokio<'a, C> {
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        std::pin::Pin::into_inner(self).conn.as_bufread().poll_fill_buf(cx)
    }

    fn consume(mut self: std::pin::Pin<&mut Self>, amt: usize) {
        self.conn.as_bufread().consume(amt);
    }
}

#[cfg(feature = "tokio")]
impl<'a, C: super::ConnectionTokio> tokio::io::AsyncWrite for TimeLimitedTokio<'a, C> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let writer = std::pin::Pin::new(self.conn.as_write());
        if let std::task::Poll::Ready(v) = writer.poll_write(cx, buf) {
            return std::task::Poll::Ready(v);
        }
        self.poll_write_timeout(cx).map(Err)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let writer = std::pin::Pin::new(self.conn.as_write());
        if let std::task::Poll::Ready(v) = writer.poll_flush(cx) {
            return std::task::Poll::Ready(v);
        }
        self.poll_write_timeout(cx).map(Err)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::pin::Pin::new(self.conn.as_write()).poll_shutdown(cx)
    }
}

#[cfg(feature = "tokio")]
pub(super) async fn timed_io<T, F: std::future::Future<Output = std::io::Result<T>>>(
    fut: F,
    new_timeout: Option<Duration>,
    old_timeout: Option<Duration>,
) -> std::io::Result<Result<T, bool>> {
    let (timeout, allow_time_error) = timeout_fallback(new_timeout, old_timeout);
    let msg = if let Some(dur) = timeout {
        match tokio::time::timeout(dur, fut).await {
            Ok(res) => res,
            Err(e) => Err(e.into()),
        }
    } else {
        fut.await
    };
    filter_time_error(msg).map(|v| v.ok_or(allow_time_error))
}

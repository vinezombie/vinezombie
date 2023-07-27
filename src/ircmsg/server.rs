use super::{Args, Numeric, ServerMsgKind, SharedSource, Source, Tags};
use crate::{
    error::{InvalidString, ParseError},
    string::{Cmd, Line, Nick},
};
use std::io::Write;

/// An IRC message sent by a server.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ServerMsg<'a> {
    /// This message's tags, if any.
    pub tags: Tags<'a>,
    /// Where this message originated.
    pub source: Option<SharedSource<'a>>,
    /// What kind of message this is.
    pub kind: ServerMsgKind<'a>,
    /// This message's arguments.
    pub args: Args<'a>,
}

impl ServerMsg<'static> {
    /// Reads a `'static` server message from `read`.
    /// This function may block.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function.
    /// All calls to this function will leave `buf`
    /// in a valid state for future calls.
    ///
    /// If the `tracing` feature is enabled, every successful call of this function
    /// will log an event at the debug level.
    pub fn read_owning_from(
        read: &mut (impl std::io::BufRead + ?Sized),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<Self> {
        use std::io::{BufRead, Read};
        read_msg!(
            ServerMsg::MAX_LEN,
            buf,
            read: Read,
            read.read_until(b'\n', buf),
            ServerMsg::parse(std::mem::take(buf))
        )
    }
    /// Asynchronously reads a `'static` server message from `read`.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function.
    /// All calls to this function will leave `buf`
    /// in a valid state for future calls.
    ///
    /// If the `tracing` feature is enabled, every successful call of this function
    /// will log an event at the debug level.
    #[cfg(feature = "tokio")]
    pub async fn read_owning_from_tokio(
        read: &mut (impl tokio::io::AsyncBufReadExt + ?Sized + Unpin),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<Self> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt};
        read_msg!(
            ServerMsg::MAX_LEN,
            buf,
            read: AsyncReadExt,
            read.read_until(b'\n', buf).await,
            ServerMsg::parse(std::mem::take(buf))
        )
    }
}

impl<'a> ServerMsg<'a> {
    /// Reads a server message from `read`.
    /// This function may block.
    ///
    /// Consider using [`ServerMsg::read_owning_from`] instead
    /// unless minimizing memory allocations is very important.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function.
    /// Both success and parse failure
    /// (indicated by [`InvalidData`][std::io::ErrorKind::InvalidData])
    /// will leave `buf` in an invalid state for future calls.
    ///
    /// If the `tracing` feature is enabled, every successful call of this function
    /// will log an event at the debug level.
    pub fn read_borrowing_from(
        read: &mut (impl std::io::BufRead + ?Sized),
        buf: &'a mut Vec<u8>,
    ) -> std::io::Result<Self> {
        use std::io::{BufRead, Read};
        read_msg!(
            ServerMsg::MAX_LEN,
            buf,
            read: Read,
            read.read_until(b'\n', buf),
            ServerMsg::parse(buf.as_slice())
        )
    }
    /// Asynchronously reads a server message from `read`.
    ///
    /// Consider using [`ServerMsg::read_owning_from_tokio`] instead
    /// unless minimizing memory allocations is very important.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function.
    /// Both success and parse failure
    /// (indicated by [`InvalidData`][std::io::ErrorKind::InvalidData])
    /// will leave `buf` in an invalid state for future calls.
    ///
    /// If the `tracing` feature is enabled, every successful call of this function
    /// will log an event at the debug level.
    #[cfg(feature = "tokio")]
    pub async fn read_borrowing_from_tokio(
        read: &mut (impl tokio::io::AsyncBufReadExt + ?Sized + Unpin),
        buf: &'a mut Vec<u8>,
    ) -> std::io::Result<ServerMsg<'a>> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt};
        read_msg!(
            ServerMsg::MAX_LEN,
            buf,
            read: AsyncReadExt,
            read.read_until(b'\n', buf).await,
            ServerMsg::parse(buf.as_slice())
        )
    }
    /// The length of the longest permissible server message, including tags.
    pub const MAX_LEN: usize = 8703;
    /// Creates a new `ServerMsg` with the provided numeric reply code, source, and target.
    pub fn new_num(num: Numeric, source: SharedSource<'a>, target: Nick<'a>) -> Self {
        ServerMsg {
            tags: Tags::new(),
            source: Some(source),
            kind: ServerMsgKind::Numeric(num),
            args: Args::new(vec![target.into()], None),
        }
    }
    /// Creates a new `ServerMsg` with the provided command.
    pub const fn new_cmd(cmd: Cmd<'a>) -> Self {
        ServerMsg {
            tags: Tags::new(),
            source: None,
            kind: ServerMsgKind::Cmd(cmd),
            args: Args::empty(),
        }
    }
    /// Parses a message from a [`Line`].
    pub fn parse(
        msg: impl TryInto<Line<'a>, Error = impl Into<InvalidString>>,
    ) -> Result<ServerMsg<'a>, ParseError> {
        let msg = msg.try_into().map_err(|e| ParseError::InvalidLine(e.into()))?;
        let (tags, source, kind, args) = super::parse(msg, Source::parse, |kind| {
            Ok(if let Some(num) = Numeric::from_bytes(&kind) {
                num.into()
            } else {
                Cmd::from_word(kind).map_err(ParseError::InvalidKind)?.into()
            })
        })?;
        let source = source.map(SharedSource::new);
        Ok(ServerMsg { tags, source, kind, args })
    }
    /// The number of bytes of space remaining in this message, excluding tags.
    ///
    /// If either of the returned values are negative, this message is too long
    /// to guarantee that it will be delivered whole.
    pub fn bytes_left(&self) -> isize {
        super::bytes_left(&self.kind.as_arg(), self.source.as_deref(), &self.args)
    }
    /// Writes self to the provided [`Write`] WITHOUT a trailing CRLF.
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, write: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        super::write_to(&self.tags, self.source.as_deref(), &self.kind.as_arg(), &self.args, write)
    }
    /// Writes self to `write` WITH a trailing CRLF,
    /// using the provided buffer to minimize the necessary number of writes to `write`.
    ///
    /// The buffer will be cleared after successfully sending this message.
    /// If the buffer is non-empty, message data will be appended to the buffer's contents.
    ///
    /// If the `tracing` feature is enabled, every successful call of this function
    /// will log an event at the debug level.
    pub fn send_to(
        &self,
        write: &mut (impl Write + ?Sized),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<()> {
        self.write_to(buf)?;
        buf.extend_from_slice(b"\r\n");
        write.write_all(buf)?;
        buf.clear();
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "vinezombie::send", "{}", self);
        Ok(())
    }
    /// Asynchronously writes self to `write` WITH a trailing CRLF,
    /// using the provided buffer to minimize the necessary number of writes to `write`.
    ///
    /// The buffer will be cleared after successfully sending this message.
    /// If the buffer is non-empty, message data will be appended to the buffer's contents.
    ///
    /// If the `tracing` feature is enabled, every successful call of this function
    /// will log an event at the debug level.
    #[cfg(feature = "tokio")]
    pub async fn send_to_tokio(
        &self,
        write: &mut (impl tokio::io::AsyncWriteExt + ?Sized + Unpin),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<()> {
        self.write_to(buf)?;
        buf.extend_from_slice(b"\r\n");
        write.write_all(buf).await?;
        buf.clear();
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "vinezombie::send", "{}", self);
        Ok(())
    }

    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> ServerMsg<'static> {
        let source = self.source.map(|src| SharedSource::new(src.owning()));
        ServerMsg {
            tags: self.tags.owning(),
            source,
            kind: self.kind.owning(),
            args: self.args.owning(),
        }
    }
}

impl std::fmt::Display for ServerMsg<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.tags.is_empty() {
            // Tags' Display impl includes the leading @.
            write!(f, "{} ", self.tags)?;
        }
        if let Some(ref src) = self.source {
            write!(f, ":{} ", src)?;
        }
        write!(f, "{}", self.kind.as_arg())?;
        if !self.args.is_empty() {
            write!(f, " {}", self.args)?;
        }
        Ok(())
    }
}

use super::{Args, Tags};
use crate::{
    error::{InvalidByte, ParseError},
    source::Source,
    string::{Cmd, Line},
};
use std::io::Write;

/// An IRC message sent by a client.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ClientMsg<'a> {
    /// This message's tags, if any.
    pub tags: Tags<'a>,
    /// This message's command.
    pub cmd: Cmd<'a>,
    /// This message's arguments.
    pub args: Args<'a>,
}

impl ClientMsg<'static> {
    /// Reads a `'static` client message from `read`.
    /// This function may block.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function.
    /// All calls to this function will leave `buf`
    /// in a valid state for future calls.
    pub fn read_owning_from(
        read: &mut (impl std::io::BufRead + ?Sized),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<Self> {
        use std::io::{BufRead, Read};
        read_msg!(
            ClientMsg::MAX_LEN,
            buf,
            read: Read,
            read.read_until(b'\n', buf),
            ClientMsg::parse(std::mem::take(buf))
        )
    }
    /// Asynchronously reads a `'static` client message from `read`.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function.
    /// All calls to this function will leave `buf`
    /// in a valid state for future calls.
    #[cfg(feature = "tokio")]
    pub async fn read_owning_from_tokio(
        read: &mut (impl tokio::io::AsyncBufReadExt + ?Sized + Unpin),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<Self> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt};
        read_msg!(
            ClientMsg::MAX_LEN,
            buf,
            read: AsyncReadExt,
            read.read_until(b'\n', buf).await,
            ClientMsg::parse(std::mem::take(buf))
        )
    }
}

impl<'a> ClientMsg<'a> {
    /// Reads a client message from `read`.
    /// This function may block.
    ///
    /// Consider using [`ClientMsg::read_owning_from`] instead
    /// unless minimizing memory allocations is very important.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function.
    /// Both success and parse failure
    /// (indicated by [`InvalidData`][std::io::ErrorKind::InvalidData])
    /// will leave `buf` in an invalid state for future calls.
    pub fn read_borrowing_from(
        read: &mut (impl std::io::BufRead + ?Sized),
        buf: &'a mut Vec<u8>,
    ) -> std::io::Result<Self> {
        use std::io::{BufRead, Read};
        read_msg!(
            ClientMsg::MAX_LEN,
            buf,
            read: Read,
            read.read_until(b'\n', buf),
            ClientMsg::parse(buf.as_slice())
        )
    }
    /// Asynchronously reads a client message from `read`.
    ///
    /// Consider using [`ClientMsg::read_owning_from_tokio`] instead
    /// unless minimizing memory allocations is very important.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function.
    /// Both success and parse failure
    /// (indicated by [`InvalidData`][std::io::ErrorKind::InvalidData])
    /// will leave `buf` in an invalid state for future calls.
    #[cfg(feature = "tokio")]
    pub async fn read_borrowing_from_tokio(
        read: &mut (impl tokio::io::AsyncBufReadExt + ?Sized + Unpin),
        buf: &'a mut Vec<u8>,
    ) -> std::io::Result<ClientMsg<'a>> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt};
        read_msg!(
            ClientMsg::MAX_LEN,
            buf,
            read: AsyncReadExt,
            read.read_until(b'\n', buf).await,
            ClientMsg::parse(buf.as_slice())
        )
    }
    /// The length of the longest permissible client message.
    pub const MAX_LEN: usize = 4608;
    /// Creates a new `ServerMsg` with the provided command.
    pub const fn new_cmd(cmd: Cmd<'a>) -> Self {
        ClientMsg { tags: Tags::new(), cmd, args: Args::new() }
    }
    /// Parses a message from a [`Line`].
    pub fn parse(
        msg: impl TryInto<Line<'a>, Error = impl Into<InvalidByte>>,
    ) -> Result<ClientMsg<'a>, ParseError> {
        let msg = msg.try_into().map_err(|e| ParseError::InvalidLine(e.into()))?;
        let (tags, _, cmd, args) = super::parse(
            msg,
            |_| Ok(()),
            |kind| Cmd::from_word(kind).map_err(ParseError::InvalidKind),
        )?;
        Ok(ClientMsg { tags, cmd, args })
    }
    /// The number of bytes of space remaining in this message, excluding tags.
    ///
    /// For messages that will be forwarded to other users (e.g. `PRIVMSG`s),
    /// the caller should provide a `source` constructed from the sender's information
    /// to get a more-accurate count.
    ///
    /// If either of the returned values are negative, this message is too long
    /// to guarantee that it will be processed whole.
    pub fn bytes_left(&self, source: Option<&Source>) -> isize {
        super::bytes_left(&self.cmd, source, &self.args)
    }
    /// Writes self to the provided [`Write`] WITHOUT a trailing CRLF.
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, write: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        super::write_to(&self.tags, None, &self.cmd, &self.args, write)
    }
    /// Writes self to `write` WITH a trailing CRLF,
    /// using the provided buffer to minimize the necessary number of writes to `write`.
    ///
    /// The buffer will be cleared after successfully sending this message.
    /// If the buffer is non-empty, message data will be appended to the buffer's contents.
    pub fn send_to(
        &self,
        write: &mut (impl Write + ?Sized),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<()> {
        self.write_to(buf)?;
        buf.extend_from_slice(b"\r\n");
        write.write_all(buf)?;
        buf.clear();
        Ok(())
    }
    /// Asynchronously writes self to `write` WITH a trailing CRLF,
    /// using the provided buffer to minimize the necessary number of writes to `write`.
    ///
    /// The buffer will be cleared after successfully sending this message.
    /// If the buffer is non-empty, message data will be appended to the buffer's contents.
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
        Ok(())
    }
}

impl std::fmt::Display for ClientMsg<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.tags.is_empty() {
            // Tags' Display impl includes the leading @.
            write!(f, "{} ", self.tags)?;
        }
        write!(f, "{}", self.cmd)?;
        if !self.args.is_empty() {
            write!(f, " {}", self.args)?;
        }
        Ok(())
    }
}

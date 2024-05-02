use super::{Args, Source, Tags};
use crate::{
    error::{InvalidString, ParseError},
    names::{ClientMsgKind, Name, NameValued},
    string::{Cmd, Line},
};
use std::io::Write;

/// An IRC message sent by a client.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
pub struct ClientMsg<'a> {
    /// This message's tags, if any.
    pub tags: Tags<'a>,
    /// This message's command.
    pub cmd: Cmd<'a>,
    /// This message's arguments.
    pub args: Args<'a>,
}

impl ClientMsg<'static> {
    /// Creates a new `ClientMsg` with the provided command.
    pub fn new<T: Name<ClientMsgKind>>(cmd: T) -> Self {
        Self::new_cmd(cmd.as_raw().clone())
    }
    #[deprecated = "Moved to `ServerCodec` in 0.4."]
    /// Reads a `'static` client message from `read`.
    /// This function may block.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function that errored due to
    /// non-blocking I/O or unexpected EOF.
    /// Other errors may leave `buf` in an invalid state for future calls.
    pub fn read_owning_from(
        read: &mut (impl std::io::BufRead + ?Sized),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<Self> {
        super::ServerCodec::read_owning_from(read, buf)
    }
    #[deprecated = "Moved to `ServerCodec` in 0.4."]
    /// Asynchronously reads a `'static` client message from `read`.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function that errored due to
    /// non-blocking I/O or unexpected EOF.
    /// Other errors may leave `buf` in an invalid state for future calls.
    #[cfg(feature = "tokio")]
    pub async fn read_owning_from_tokio(
        read: &mut (impl tokio::io::AsyncBufReadExt + ?Sized + Unpin),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<Self> {
        super::ServerCodec::read_owning_from_tokio(read, buf).await
    }
}

impl<'a> ClientMsg<'a> {
    /// Attempts to parse this message further into a higher-level message type.
    ///
    /// Does not check if the message kind matches, assuming such a check has been done earlier.
    pub fn parse_as<N>(&self, _kind: N) -> Result<N::Value<'a>, ParseError>
    where
        N: NameValued<ClientMsgKind>,
    {
        N::from_union(self)
    }
    #[deprecated = "Moved to `ServerCodec` in 0.4."]
    /// Reads a client message from `read`.
    /// This function may block.
    ///
    /// Consider using [`ClientMsg::read_owning_from`] instead
    /// unless minimizing memory allocations is very important.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function that errored due to
    /// non-blocking I/O or unexpected EOF.
    /// Other errors may leave `buf` in an invalid state for future calls.
    pub fn read_borrowing_from(
        read: &mut (impl std::io::BufRead + ?Sized),
        buf: &'a mut Vec<u8>,
    ) -> std::io::Result<Self> {
        super::ServerCodec::read_borrowing_from(read, buf)
    }
    #[deprecated = "Moved to `ServerCodec` in 0.4."]
    /// Asynchronously reads a client message from `read`.
    ///
    /// Consider using [`ClientMsg::read_owning_from_tokio`] instead
    /// unless minimizing memory allocations is very important.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function that errored due to
    /// non-blocking I/O or unexpected EOF.
    /// Other errors may leave `buf` in an invalid state for future calls.
    #[cfg(feature = "tokio")]
    pub async fn read_borrowing_from_tokio(
        read: &mut (impl tokio::io::AsyncBufReadExt + ?Sized + Unpin),
        buf: &'a mut Vec<u8>,
    ) -> std::io::Result<ClientMsg<'a>> {
        super::ServerCodec::read_borrowing_from_tokio(read, buf).await
    }
    /// The length of the longest permissible client message.
    pub const MAX_LEN: usize = 4608;
    /// Creates a new `ClientMsg` with the provided command.
    pub const fn new_cmd(cmd: Cmd<'a>) -> Self {
        ClientMsg { tags: Tags::new(), cmd, args: Args::empty() }
    }
    /// Parses a message from a [`Line`].
    pub fn parse(
        msg: impl TryInto<Line<'a>, Error = impl Into<InvalidString>>,
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
    #[deprecated = "Moved to `ClientCodec` in 0.4."]
    /// Writes self to the provided [`Write`] WITHOUT a trailing CRLF.
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, write: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        super::ClientCodec::write_to(self, write)
    }
    #[deprecated = "Moved to `ClientCodec` in 0.4."]
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
        super::ClientCodec::send_to(self, write, buf)
    }
    #[deprecated = "Moved to `ClientCodec` in 0.4."]
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
        super::ClientCodec::send_to_tokio(self, write, buf).await
    }

    /// Converts `self` into a version that owns its data.
    pub fn owning(self) -> ClientMsg<'static> {
        ClientMsg { tags: self.tags.owning(), cmd: self.cmd.owning(), args: self.args.owning() }
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

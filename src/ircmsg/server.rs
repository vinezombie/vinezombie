use super::{Args, Numeric, ServerMsgKindRaw, SharedSource, Source, Tags};
use crate::{
    error::{InvalidString, ParseError},
    names::{Name, NameValued, ServerMsgKind},
    string::{Cmd, Line, Nick},
};
use std::io::Write;

/// An IRC message sent by a server.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
pub struct ServerMsg<'a> {
    /// This message's tags, if any.
    pub tags: Tags<'a>,
    /// Where this message originated.
    pub source: Option<SharedSource<'a>>,
    /// What kind of message this is.
    pub kind: ServerMsgKindRaw<'a>,
    /// This message's arguments.
    pub args: Args<'a>,
}

impl ServerMsg<'static> {
    #[deprecated = "Moved to `ClientCodec` in 0.4."]
    /// Reads a `'static` server message from `read`.
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
        super::ClientCodec::read_owning_from(read, buf)
    }
    #[deprecated = "Moved to `ClientCodec` in 0.4."]
    /// Asynchronously reads a `'static` server message from `read`.
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
        super::ClientCodec::read_owning_from_tokio(read, buf).await
    }
}

impl<'a> ServerMsg<'a> {
    /// Creates a new `ServerMsg` with the provided message type and source.
    pub fn new<T: Name<ServerMsgKind>>(kind: T, source: SharedSource<'a>) -> Self {
        ServerMsg {
            tags: Tags::new(),
            source: Some(source),
            kind: kind.as_raw().clone(),
            args: Args::empty(),
        }
    }
    /// Attempts to parse this message further into a higher-level message type.
    ///
    /// Does not check if the message kind matches, assuming such a check has been done earlier.
    pub fn parse_as<N>(&self, _kind: N) -> Result<N::Value<'a>, ParseError>
    where
        N: NameValued<ServerMsgKind>,
    {
        N::from_union(self)
    }
    #[deprecated = "Moved to `ClientCodec` in 0.4."]
    /// Reads a server message from `read`.
    /// This function may block.
    ///
    /// Consider using [`ServerMsg::read_owning_from`] instead
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
        super::ClientCodec::read_borrowing_from(read, buf)
    }
    #[deprecated = "Moved to `ClientCodec` in 0.4."]
    /// Asynchronously reads a server message from `read`.
    ///
    /// Consider using [`ServerMsg::read_owning_from_tokio`] instead
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
    ) -> std::io::Result<ServerMsg<'a>> {
        super::ClientCodec::read_borrowing_from_tokio(read, buf).await
    }
    /// The length of the longest permissible server message, including tags.
    pub const MAX_LEN: usize = 8703;
    /// Creates a new `ServerMsg` with the provided numeric reply code, source, and target.
    pub fn new_num(num: Numeric, source: SharedSource<'a>, target: Nick<'a>) -> Self {
        ServerMsg {
            tags: Tags::new(),
            source: Some(source),
            kind: ServerMsgKindRaw::Numeric(num),
            args: Args::new(vec![target.into()], None),
        }
    }
    /// Creates a new `ServerMsg` with the provided command.
    pub const fn new_cmd(cmd: Cmd<'a>) -> Self {
        ServerMsg {
            tags: Tags::new(),
            source: None,
            kind: ServerMsgKindRaw::Cmd(cmd),
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
    #[deprecated = "Moved to `ServerCodec` in 0.4."]
    /// Writes self to the provided [`Write`] WITHOUT a trailing CRLF.
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, write: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        super::ServerCodec::write_to(self, write)
    }
    #[deprecated = "Moved to `ServerCodec` in 0.4."]
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
        super::ServerCodec::send_to(self, write, buf)
    }
    #[deprecated = "Moved to `ServerCodec` in 0.4."]
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
        super::ServerCodec::send_to_tokio(self, write, buf).await
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

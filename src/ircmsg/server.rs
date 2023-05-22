use std::io::Write;

use crate::string::{Cmd, InvalidByte, Line, Nick};

use super::{Args, Numeric, ParseError, ServerMsgKind, Source, Tags};

/// An IRC message sent by a server.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ServerMsg<'a> {
    /// This message's tags, if any.
    pub tags: Tags<'a>,
    /// Where this message originated.
    pub source: Option<Source<'a>>,
    /// What kind of message this is.
    pub kind: ServerMsgKind<'a>,
    /// This message's arguments.
    pub args: Args<'a>,
}

impl<'a> ServerMsg<'a> {
    /// Creates a new `ServerMsg` with the provided numeric reply code.
    pub const fn new_num(server_name: Nick<'a>, num: Numeric) -> Self {
        ServerMsg {
            tags: Tags::new(),
            source: Some(Source::new_server(server_name)),
            kind: ServerMsgKind::Numeric(num),
            args: Args::new(),
        }
    }
    /// Creates a new `ServerMsg` with the provided command.
    pub const fn new_cmd(source: Source<'a>, cmd: Cmd<'a>) -> Self {
        ServerMsg {
            tags: Tags::new(),
            source: Some(source),
            kind: ServerMsgKind::Cmd(cmd),
            args: Args::new(),
        }
    }
    /// Parses a message from a [`Line`].
    pub fn parse(
        msg: impl TryInto<Line<'a>, Error = impl Into<InvalidByte>>,
    ) -> Result<ServerMsg<'a>, ParseError> {
        let msg = msg.try_into().map_err(|e| ParseError::InvalidLine(e.into()))?;
        let (tags, source, kind, args) = super::parse(msg, Source::parse, |kind| {
            Ok(if let Some(num) = Numeric::from_bytes(&kind) {
                num.into()
            } else {
                Cmd::from_word(kind).map_err(ParseError::InvalidKind)?.into()
            })
        })?;
        Ok(ServerMsg { tags, source, kind, args })
    }
    /// The number of bytes of space remaining in this message, excluding tags.
    ///
    /// If either of the returned values are negative, this message is too long
    /// to guarantee that it will be delivered whole.
    pub fn bytes_left(&self) -> isize {
        super::bytes_left(&self.kind.as_arg(), self.source.as_ref(), &self.args)
    }
    /// Writes self to the provided [`Write`] WITHOUT a trailing CRLF.
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(&self, write: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        super::write_to(&self.tags, self.source.as_ref(), &self.kind.as_arg(), &self.args, write)
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

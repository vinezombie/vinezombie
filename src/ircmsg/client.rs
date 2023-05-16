use std::io::Write;

use super::{Args, ParseError, ServerMsg, ServerMsgKind, Source, Tags};
use crate::string::{Cmd, InvalidByte, Line};

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

impl<'a> ClientMsg<'a> {
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
    /// Converts `self` into a server message with the provided source and kind.
    pub fn into_server_msg<'b, 'c>(
        self,
        source: Source<'b>,
        kind: impl Into<ServerMsgKind<'b>>,
    ) -> ServerMsg<'c>
    where
        'a: 'c,
        'b: 'c,
    {
        ServerMsg { tags: self.tags, source: Some(source), kind: kind.into(), args: self.args }
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

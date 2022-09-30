use super::{
    args,
    client::ClientMsg,
    data::{FlexibleKind, RawData, TerminalKind},
    known, DefaultMsgParser, NewMsgParser,
};
use crate::{
    known::{Kind, MaybeKnown},
    IrcStr, IrcWord,
};

/// Convenience alias for the kind of a [ServerMsg].
pub type ServerMsgKind<'a> = MaybeKnown<'a, Kind>;
/// Representation of the source of a server message.
pub type Source<'a> = Option<IrcWord<'a>>;

/// Error type when parsing a [`ServerMsg`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MsgParseError {
    /// Message exceeds permissible length limits.
    ///
    /// [`ServerMsg::parse`] does not return this, but an I/O step may.
    TooLong,
    /// Expected tags but none were provided.
    NoTags,
    /// Expected a source but none was provided.
    NoSource,
    /// There was no message kind in the provided message.
    NoKind,
}

impl std::fmt::Display for MsgParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MsgParseError::TooLong => write!(f, "invalid message: length limits exceeded"),
            MsgParseError::NoTags => write!(f, "invalid message: no tags after @"),
            MsgParseError::NoSource => write!(f, "invalid message: no source after :"),
            MsgParseError::NoKind => write!(f, "invalid message: kind/command"),
        }
    }
}

impl std::error::Error for MsgParseError {}

/// Message sent by an IRC server.
#[derive(Clone, Debug)]
pub struct ServerMsg<'a, T> {
    /// The sender of this message.
    pub source: Source<'a>,
    /// What kind of message this is, usually a command or numeric reply.
    pub kind: ServerMsgKind<'a>,
    /// The data associated with this message.
    pub data: T,
}

impl<'a> ServerMsg<'a, RawData<'a>> {
    /// Parses a message from a string.
    pub fn parse(msg: impl Into<IrcStr<'a>>) -> Result<ServerMsg<'a, RawData<'a>>, MsgParseError> {
        let mut msg = msg.into();
        let mut source = None;
        msg.slice(str::trim);
        if msg.lex_char(|c| *c == '@').is_some() {
            // TODO: Tags. Specifically, actually parse them.
            let _ = msg.lex_word().ok_or(MsgParseError::NoTags)?;
            msg.slice(str::trim_start);
        }
        if msg.lex_char(|c| *c == ':').is_some() {
            source = Some(msg.lex_word().ok_or(MsgParseError::NoSource)?);
            msg.slice(str::trim_start);
        }
        let kind = msg.lex_word().ok_or(MsgParseError::NoKind)?.into();
        let args = args::Args::parse(msg);
        let data = RawData { args };
        Ok(ServerMsg { source, kind, data })
    }
    /// The number of bytes of space remaining in this message.
    ///
    /// If the returned value is negative, this message is too long
    /// to guarantee its delivery in whole for most IRCd client/server pairs.
    pub fn bytes_left(&self) -> isize {
        // TODO: Encoding.
        // TODO: Tags!
        let mut size = self.kind.len_bytes() + 2; // Newline.
        if let Some(ref word) = self.source {
            size += 2 + word.len_bytes();
        }
        for arg in self.data.args.all() {
            size += arg.len_bytes() + 1; // Space.
        }
        if self.data.args.is_last_long() {
            size += 1; // Colon.
        }
        let size: isize = size.try_into().unwrap_or(isize::MAX);
        512 - size
    }
    /// Tries to convert this message into one with a different data type.
    pub fn to_parsed<T: DefaultMsgParser<'a> + 'a>(
        &self,
        options: <<T as DefaultMsgParser<'a>>::Parser as NewMsgParser<'a>>::Options,
    ) -> Option<ServerMsg<'a, T>> {
        use crate::msg::MsgParser;
        let parser = T::Parser::new_msg_parser(options);
        parser.parse_msg(self).finish()
    }
}

impl<'a, T> ServerMsg<'a, T> {
    /// Breaks this message up into its members.
    pub fn into_parts(self) -> (ServerMsgKind<'a>, T, Source<'a>) {
        (self.kind, self.data, self.source)
    }
    /// Returns a reference to this message's kind.
    pub fn kind(&self) -> &ServerMsgKind<'a> {
        &self.kind
    }
    /// Returns a reference to this message's data.
    pub fn data(&self) -> &T {
        &self.data
    }
    /// Returns a mutable reference to this message's data.
    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data
    }
    /// Returns a reference to this message's source.
    pub fn source(&self) -> &Source<'a> {
        &self.source
    }
    /// Returns a mutable reference to this message's source.
    pub fn source_mut(&mut self) -> &mut Source<'a> {
        &mut self.source
    }
}

impl<'a, T: FlexibleKind + Clone + 'a> ServerMsg<'a, T> {
    /// Returns a mutable reference to this message's kind.
    pub fn kind_mut(&mut self) -> &mut known::MaybeKnown<'a, known::Kind> {
        &mut self.kind
    }
    /// Creates a [ClientMsg] with the same arguments as this message.
    pub fn reply_with_kind<'b, 'c>(
        &self,
        kind: impl Into<known::MaybeKnown<'b, known::Cmd>>,
    ) -> ClientMsg<'c, T>
    where
        'a: 'c,
        'b: 'c,
    {
        ClientMsg::new_with_kind(kind, self.data.clone())
    }
}

impl<'a, T: FlexibleKind + 'a> ServerMsg<'a, T> {
    /// Constructs a new [ServerMsg] with the provided kind, data, and source.
    pub fn new_with_kind(
        kind: impl Into<ServerMsgKind<'a>>,
        data: T,
        source: Source<'a>,
    ) -> ServerMsg<'a, T> {
        ServerMsg { kind: kind.into(), data, source }
    }
}

impl<'a, T: TerminalKind + 'a> ServerMsg<'a, T> {
    /// Constructs a new [ServerMsg] with the provided data and source
    pub fn new(data: T, source: Source<'a>) -> ServerMsg<'a, T> {
        ServerMsg { kind: T::terminal_kind().into(), data, source }
    }
}

impl std::fmt::Display for ServerMsg<'_, RawData<'_>> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Tags.
        if let Some(ref src) = self.source {
            write!(f, ":{} ", src.as_ref())?;
        }
        write!(f, "{}", self.kind)?;
        if !self.data.args.is_empty() {
            write!(f, " {}", self.data.args)?;
        }
        Ok(())
    }
}

use super::{
    data::{FlexibleKind, RawData, TerminalKind},
    known::{self, Cmd},
    server::ServerMsg,
    DefaultMsgWriter, NewMsgWriter, Source,
};
use crate::known::MaybeKnown;

/// Convenience alias for the kind of a [ClientMsg].
pub type ClientMsgKind<'a> = MaybeKnown<'a, Cmd>;

/// Message sent by an IRC client.
#[derive(Clone, Debug)]
pub struct ClientMsg<'a, T> {
    /// The command for this message.
    kind: ClientMsgKind<'a>,
    /// The data associated with this message.
    data: T,
}

impl<'a> ClientMsg<'a, RawData<'a>> {
    /// Creates an empty [crate::msg::RawClientMsg].
    pub fn new_raw(kind: impl Into<known::MaybeKnown<'a, Cmd>>) -> Self {
        ClientMsg { kind: kind.into(), data: Default::default() }
    }
}

// TODO: new functions for T: TerminalKind<Data = constraint>

impl<'a, T: FlexibleKind> ClientMsg<'a, T> {
    /// Constructs a new message with the provided data and kind.
    pub fn new_with_kind(kind: impl Into<known::MaybeKnown<'a, Cmd>>, data: T) -> Self {
        ClientMsg { kind: kind.into(), data }
    }
    /// Returns a mutable reference to this message's kind.
    pub fn kind_mut(&mut self) -> &mut ClientMsgKind<'a> {
        &mut self.kind
    }
}

impl<'a, T: FlexibleKind + Default> ClientMsg<'a, T> {
    /// Constructs a new message with the provided kind.
    pub fn new_default_with_kind(kind: impl Into<ClientMsgKind<'a>>) -> Self {
        ClientMsg { kind: kind.into(), data: Default::default() }
    }
}

impl<'a, K: Into<ClientMsgKind<'static>>, T: TerminalKind<Kind = K>> ClientMsg<'a, T> {
    /// Constructs a new message with the provided data.
    pub fn new(data: T) -> Self {
        ClientMsg { kind: T::terminal_kind().into(), data }
    }
}

impl<'a, K: Into<ClientMsgKind<'static>>, T: TerminalKind<Kind = K> + Default> ClientMsg<'a, T> {
    /// Constructs a new message.
    pub fn new_default() -> Self {
        ClientMsg { kind: T::terminal_kind().into(), data: Default::default() }
    }
}

impl<'a, T: DefaultMsgWriter<'a>> ClientMsg<'a, T> {
    /// Converts this message into the default [MsgWriter] for its data type.
    pub fn to_default_writer(
        self,
        options: <<T as DefaultMsgWriter<'a>>::Writer as NewMsgWriter<'a>>::Options,
    ) -> Box<T::Writer> {
        T::Writer::new_msg_writer(self, options)
    }
}

impl<'a, T> ClientMsg<'a, T> {
    /// Breaks this message up into its members.
    pub fn into_parts(self) -> (MaybeKnown<'a, Cmd>, T) {
        (self.kind, self.data)
    }
    /// Returns a reference to this message's kind.
    pub fn kind(&self) -> &ClientMsgKind<'a> {
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
}

impl<'a, T: Clone> ClientMsg<'a, T> {
    /// Converts this message into a [ServerMsg] as another client would receive it.
    pub fn preview<'b, 'c>(&self, source: Source<'b>) -> ServerMsg<'c, T>
    where
        'a: 'c,
        'b: 'c,
    {
        let kind = self.kind.clone().map_known(known::Kind::from);
        let data = self.data.clone();
        super::ServerMsg { source, kind, data }
    }
}

impl std::fmt::Display for ClientMsg<'_, RawData<'_>> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Tags.
        write!(f, "{}", self.kind)?;
        if !self.data.args.is_empty() {
            write!(f, " {}", self.data.args)?;
        }
        Ok(())
    }
}

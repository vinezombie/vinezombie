/// Markers indicating distinct types of tags.
pub trait TagClass: 'static {
    /// The type of unparsed values for this tag.
    type Raw<'a>: Ord;
    /// The type that typically contains tags in this class.
    type Outer<'a>;
    /// Extract a shared reference to the raw tag from the outer type.
    fn get_tag<'a, 'b>(outer: &'a Self::Outer<'b>) -> &'a Self::Raw<'b>;
    /// Extract a mutable reference to the raw tag type from the outer type.
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Outer<'b>) -> &'a mut Self::Raw<'b>;
}

/// Parsed discriminants within a [`TagClass`].
///
/// Implementors are conventionally zero-sized types.
pub trait Tag<Class: TagClass>: std::any::Any + Copy {
    /// This tag's raw value.
    const RAW: <Class as TagClass>::Raw<'static>;
}

/// Marker for tags that differentiate client-originated messages.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ClientMsgKind {}

impl TagClass for ClientMsgKind {
    type Raw<'a> = crate::string::Cmd<'a>;
    type Outer<'a> = crate::ircmsg::ClientMsg<'a>;
    fn get_tag<'a, 'b>(outer: &'a Self::Outer<'b>) -> &'a Self::Raw<'b> {
        &outer.cmd
    }
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Outer<'b>) -> &'a mut Self::Raw<'b> {
        &mut outer.cmd
    }
}

/// Marker for tags that differentiate server-originated messages.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ServerMsgKind {}

impl TagClass for ServerMsgKind {
    type Raw<'a> = crate::ircmsg::ServerMsgKindRaw<'a>;
    type Outer<'a> = crate::ircmsg::ServerMsg<'a>;
    fn get_tag<'a, 'b>(outer: &'a Self::Outer<'b>) -> &'a Self::Raw<'b> {
        &outer.kind
    }
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Outer<'b>) -> &'a mut Self::Raw<'b> {
        &mut outer.kind
    }
}

/// Marker for the key halves of ISUPPORT tags.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ISupport {}

impl TagClass for ISupport {
    type Raw<'a> = crate::string::Key<'a>;
    type Outer<'a> = (Self::Raw<'a>, crate::string::Word<'a>);
    fn get_tag<'a, 'b>(outer: &'a Self::Outer<'b>) -> &'a Self::Raw<'b> {
        &outer.0
    }
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Outer<'b>) -> &'a mut Self::Raw<'b> {
        &mut outer.0
    }
}

/// Marker for IRCv3 message tags.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MsgTag {}

impl TagClass for MsgTag {
    type Raw<'a> = crate::string::Key<'a>;
    type Outer<'a> = (Self::Raw<'a>, crate::string::NoNul<'a>);
    fn get_tag<'a, 'b>(outer: &'a Self::Outer<'b>) -> &'a Self::Raw<'b> {
        &outer.0
    }
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Outer<'b>) -> &'a mut Self::Raw<'b> {
        &mut outer.0
    }
}

/// Marker for capabilities.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Cap {}

impl TagClass for Cap {
    type Raw<'a> = crate::string::Key<'a>;
    type Outer<'a> = (Self::Raw<'a>, crate::string::Word<'a>);
    fn get_tag<'a, 'b>(outer: &'a Self::Outer<'b>) -> &'a Self::Raw<'b> {
        &outer.0
    }
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Outer<'b>) -> &'a mut Self::Raw<'b> {
        &mut outer.0
    }
}

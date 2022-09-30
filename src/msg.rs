//! IRC messages and fragments.

pub mod args;
mod client;
pub mod data;
mod server;
#[cfg(test)]
mod tests;

pub use self::{client::*, server::*};

/// Convenience alias for a [ClientMsg] with unprocessed arguments and tags.
pub type RawClientMsg<'a> = ClientMsg<'a, data::RawData<'a>>;
/// Convenience alias for a [ServerMsg] with unprocessed arguments and tags.
pub type RawServerMsg<'a> = ServerMsg<'a, data::RawData<'a>>;

use crate::known;

/// State of a [ServerMsg] parsing operation.
pub enum MsgParserStatus<'a, T> {
    /// The message is ready.
    Ready(ServerMsg<'a, T>),
    /// More messages are required for complete message data.
    Pending(Box<dyn MsgParser<'a, Data = T> + 'a>),
    /// A message was received that was of a valid kind but incorrectly structured.
    Invalid(std::borrow::Cow<'static, str>),
}

impl<'a, T: 'a> MsgParserStatus<'a, T> {
    /// Ends parsing and attempts to return a [ServerMsg].
    pub fn finish(self) -> Option<ServerMsg<'a, T>> {
        match self {
            MsgParserStatus::Ready(m) => Some(m),
            MsgParserStatus::Pending(b) => b.finish_early(),
            MsgParserStatus::Invalid(e) => {
                #[cfg(feature = "log")]
                log::warn!("secondary message parsing failed: {}", e);
                None
            }
        }
    }
}

/// Types that can process [RawServerMsg] data into more-useful versions.
pub trait MsgParser<'a> {
    // Would be nice if this was generic, but that kills object safety at this tie.
    /// The data type for messages that this parser can generate.
    type Data: Sized + 'a;
    /// Parses one [RawServerMsg].
    fn parse_msg(self: Box<Self>, msg: &RawServerMsg<'a>) -> MsgParserStatus<'a, Self::Data>;
    /// Ends message parsing and attempts to return a [ServerMsg] from
    /// whatever has already been parsed.
    ///
    /// Not all parsers will implement this method to return anything other than `None`,
    fn finish_early(self: Box<Self>) -> Option<ServerMsg<'a, Self::Data>> {
        None
    }
}

/// [MsgParser]s with a consistent way of being constructed.
pub trait NewMsgParser<'a>: MsgParser<'a> {
    /// The type of options for constructing the parser.
    type Options: 'a;
    /// Constructs a new message parser.
    fn new_msg_parser(options: Self::Options) -> Box<Self>
    where
        Self: Sized;
}

/// Message data types with a sensible default [MsgParser].
pub trait DefaultMsgParser<'a> {
    /// The message parser type.
    type Parser: NewMsgParser<'a, Data = Self>;
}

/// Types that can process client messages into [RawClientMsg]s suitable to be sent.
pub trait MsgWriter<'a> {
    /// Writes one [RawClientMsg].
    fn write_msg(self: Box<Self>) -> (RawClientMsg<'a>, Option<Box<dyn MsgWriter<'a> + 'a>>);
}

/// [MsgWriter]s with a consistent way of being constructed.
pub trait NewMsgWriter<'a>: MsgWriter<'a> {
    /// The type of options for constructing the writer.
    type Options: 'a;
    /// The data type for messages that this writer can be constructed out of.
    type Data: Sized + 'a;
    /// Constructs a new message writer.
    fn new_msg_writer(init: ClientMsg<'a, Self::Data>, options: Self::Options) -> Box<Self>
    where
        Self: Sized;
}

/// Message data types with a sensible default [MsgWriter].
pub trait DefaultMsgWriter<'a> {
    /// The message writer type.
    type Writer: NewMsgWriter<'a, Data = Self>;
}

use super::{Args, ServerMsg};
use crate::string::Kind;

/// Message sent by an IRC client.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ClientMsg<'a> {
    // TODO: Tags.
    /// The command for this message.
    pub kind: Kind<'a>,
    /// This message's arguments.
    pub args: Args<'a>,
}

impl<'a> ClientMsg<'a> {
    /// Creates a new [`ClientMsg`].
    pub const fn new(kind: Kind<'a>) -> Self {
        ClientMsg { kind, args: Args::new() }
    }
}

impl<'a> ClientMsg<'a> {
    /// Converts this message into a [`ServerMsg`] as another client would receive it.
    pub fn preview<'b, 'c>(&self, source: super::Source<'b>) -> ServerMsg<'c>
    where
        'a: 'c,
        'b: 'c,
    {
        let kind = self.kind.clone();
        let args = self.args.clone();
        super::ServerMsg { source, kind, args }
    }
    /// The number of bytes of space remaining in this message
    /// for tags and everything else, respectively.
    ///
    /// If either of the returned values are negative, this message is too long
    /// to guarantee its delivery in whole for most IRCd client/server pairs.
    pub fn bytes_left(&self, source: super::Source<'_>) -> (isize, isize) {
        // TODO: Tags!
        let mut size = self.kind.len() + 2; // Newline.
        if let Some(ref word) = source {
            size += 2 + word.len();
        }
        for arg in self.args.all() {
            size += arg.len() + 1; // Space.
        }
        if self.args.is_last_long() {
            size += 1; // Colon.
        }
        let size: isize = size.try_into().unwrap_or(isize::MAX);
        (0, 512 - size)
    }
}

impl std::fmt::Display for ClientMsg<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Tags.
        write!(f, "{}", self.kind)?;
        if !self.args.is_empty() {
            write!(f, " {}", self.args)?;
        }
        Ok(())
    }
}

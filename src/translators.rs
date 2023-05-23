//! [`Translator`][graveseed::io::Translator] impls and other connection-related utilities.

use std::num::NonZeroUsize;

use graveseed::io::Translator;

use crate::{
    ircmsg::{ClientMsg, ParseError, ServerMsg},
    string::{tf::SplitLine, Bytes},
};

/// [`Translator`] for the client side of an IRC connection.
#[derive(Clone, Copy, Debug, Default)]
pub struct IrcClient;

/// [`Translator`] for the server side of an IRC connection.
#[derive(Clone, Copy, Debug, Default)]
pub struct IrcServer;

fn identify_msg<const MAX_RECV_LEN: usize>(
    mut buf: &[u8],
) -> Result<(usize, Option<NonZeroUsize>), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let Some(skip) = buf.iter().position(|c| !c.is_ascii_whitespace()) else {
        return Ok((buf.len(), None));
    };
    buf = buf.split_at(skip).1;
    if let Some(idx) = buf.iter().position(|c| *c == b'\n') {
        Ok((skip, NonZeroUsize::new(idx + 1)))
    } else if buf.len() >= MAX_RECV_LEN {
        Err(ParseError::TooLong(MAX_RECV_LEN).into())
    } else {
        Ok((skip, None))
    }
}

impl Translator for IrcClient {
    // Power of two, almost double the largest IRCv3 message from a server.
    // Alternatively, 32x IRCv2 messages.
    const RECV_HINT: usize = 16384;

    // 512 bytes of message content and 512 bytes of tag content.
    // Should be enough in most cases.
    const SEND_HINT: usize = 1024;

    type RecvMsg<'a> = ServerMsg<'a>;

    type SendMsg<'a> = ClientMsg<'a>;

    fn write_msg<T: std::io::Write>(
        &mut self,
        buf: &mut T,
        msg: &Self::SendMsg<'_>,
    ) -> std::io::Result<()> {
        msg.write_to(buf)?;
        buf.write_all(b"\r\n")
    }

    fn identify_msg(
        &mut self,
        buf: &[u8],
    ) -> Result<(usize, Option<NonZeroUsize>), Box<dyn std::error::Error + Send + Sync + 'static>>
    {
        // 512 IRCv2 bytes + 8192 tag bytes.
        identify_msg::<8704>(buf)
    }

    fn parse_msg<'a>(
        &mut self,
        buf: &'a [u8],
    ) -> Result<Self::RecvMsg<'a>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut bytes = Bytes::from(buf);
        ServerMsg::parse(bytes.transform(SplitLine)).map_err(|e| e.into())
    }
}

impl Translator for IrcServer {
    /// Allows for 4 IRCv2 messages sent in a burst, such as during connection registration.
    const RECV_HINT: usize = 2048;

    // 512 bytes of message content and 512 bytes of tag content.
    // Should be enough in most cases.
    const SEND_HINT: usize = 1024;

    type RecvMsg<'a> = ClientMsg<'a>;

    type SendMsg<'a> = ServerMsg<'a>;

    fn write_msg<T: std::io::Write>(
        &mut self,
        buf: &mut T,
        msg: &Self::SendMsg<'_>,
    ) -> std::io::Result<()> {
        msg.write_to(buf)?;
        buf.write_all(b"\r\n")
    }

    fn identify_msg(
        &mut self,
        buf: &[u8],
    ) -> Result<(usize, Option<NonZeroUsize>), Box<dyn std::error::Error + Send + Sync + 'static>>
    {
        // 512 IRCv2 bytes + 4096 tag bytes.
        identify_msg::<4608>(buf)
    }

    fn parse_msg<'a>(
        &mut self,
        buf: &'a [u8],
    ) -> Result<Self::RecvMsg<'a>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut bytes = Bytes::from(buf);
        ClientMsg::parse(bytes.transform(SplitLine)).map_err(|e| e.into())
    }
}

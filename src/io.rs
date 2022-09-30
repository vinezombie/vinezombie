//! [`Translator`][graveseed::io::Translator] impls and other connection-related utilities.

use std::num::NonZeroUsize;

use graveseed::io::Translator;

use crate::msg::{MsgParseError, RawClientMsg, RawServerMsg};

/// [`Translator`] for the client side of an IRC connection.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClientTranslator;

// 512 IRCv2 bytes + 8192 tag bytes.
const MAX_RECV_LEN: usize = 8704;

impl Translator for ClientTranslator {
    // Power of two, almost double the largest IRCv3 message.
    const RECV_HINT: usize = 16384;

    // 512 bytes of message content and 512 bytes of tag content.
    // Should be enough in most cases.
    const SEND_HINT: usize = 1024;

    type RecvMsg<'a> = RawServerMsg<'a>;

    type SendMsg<'a> = RawClientMsg<'a>;

    fn write_msg<T: std::io::Write>(
        &mut self,
        buf: &mut T,
        msg: &Self::SendMsg<'_>,
    ) -> std::io::Result<()> {
        write!(buf, "{msg}\r\n")
    }

    fn identify_msg(
        &mut self,
        mut buf: &[u8],
    ) -> Result<(usize, Option<NonZeroUsize>), Box<dyn std::error::Error + Send + Sync + 'static>>
    {
        let Some(skip) = buf.iter().position(|c| !c.is_ascii_whitespace()) else {
            return Ok((buf.len(), None));
        };
        buf = buf.split_at(skip).1;
        if let Some(idx) = buf.iter().position(|c| *c == b'\n') {
            Ok((skip, NonZeroUsize::new(idx + 1)))
        } else if buf.len() >= MAX_RECV_LEN {
            Err(MsgParseError::TooLong.into())
        } else {
            Ok((skip, None))
        }
    }

    fn parse_msg<'a>(
        &mut self,
        buf: &'a [u8],
    ) -> Result<Self::RecvMsg<'a>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(RawServerMsg::parse(String::from_utf8_lossy(buf))?)
    }
}

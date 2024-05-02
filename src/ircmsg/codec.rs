use super::{Args, ClientMsg, ServerMsg, Source, Tags};
use crate::error::{InvalidString, ParseError};
use crate::string::{Line, Splitter, Word};
use std::io::Write;

macro_rules! read_msg {
    (
        $limit:path, $buf:ident, $read:ident: $read_type:ident, $read_expr:expr, $parse_expr:expr
    ) => {{
        use std::io::{Error, ErrorKind};
        let mut $read = $read_type::take($read, 1);
        loop {
            let buflen = $buf.len();
            if buflen < $limit {
                let read_count = $limit - buflen;
                $read.set_limit(read_count as u64);
                if $read_expr? == 0 {
                    return Err(Error::from(ErrorKind::UnexpectedEof));
                }
            }
            read_buf!(|$buf| $parse_expr)
        }
    }};
}

macro_rules! read_buf {
    (|$buf:ident| $parse_expr:expr) => {{
        use crate::error::InvalidString;
        let mut found_newline = false;
        while let Some(c) = $buf.last() {
            match c {
                b'\n' => {
                    found_newline = true;
                    $buf.truncate($buf.len() - 1);
                }
                b'\r' if found_newline => {
                    $buf.truncate($buf.len() - 1);
                }
                b'\r' | b'\0' => {
                    return Err(ParseError::InvalidLine(InvalidString::Byte(*c)).into())
                }
                _ if found_newline => {
                    return match $parse_expr {
                        Ok(msg) => Ok(msg),
                        Err(e) => Err(e.into()),
                    }
                }
                // We stumbled into a non-newline character at the end of a read that
                // was supposed to read up until the newline or the max msg len.
                _ => return Err(ParseError::TooLong.into()),
            }
        }
    }};
}

#[inline(always)]
pub(crate) fn parse<'a, S: 'a, K: 'a>(
    msg: Line<'a>,
    parse_source: impl Fn(Word<'a>) -> Result<S, ParseError>,
    parse_kind: impl FnOnce(Word<'a>) -> Result<K, ParseError>,
) -> Result<(Tags<'a>, Option<S>, K, Args<'a>), ParseError> {
    let mut tags = Tags::new();
    let mut source = None;
    let mut expect_tags = true;
    let mut expect_source = true;
    let mut msg = Splitter::new(msg);
    let kind = loop {
        msg.consume_whitespace();
        let word: Word = msg.string_or_default(false);
        if word.is_empty() {
            return Err(ParseError::InvalidKind(InvalidString::Empty));
        }
        match word.first() {
            Some(b'@') if expect_tags => {
                let mut word = Splitter::new(word);
                expect_tags = false;
                word.next_byte();
                tags = Tags::parse(word.rest_or_default::<Word>());
            }
            Some(b':') if expect_source => {
                let mut word = Splitter::new(word);
                expect_tags = false;
                expect_source = false;
                word.next_byte();
                // Maybe not quiet failure here?
                // Non-parsed sources can sometimes still be useful.
                source = Some(parse_source(word.rest_or_default())?);
            }
            Some(_) => break word,
            None => return Err(ParseError::InvalidKind(InvalidString::Empty)),
        }
    };
    let kind = parse_kind(kind)?;
    let args = Args::parse(msg.rest_or_default::<Line>());
    Ok((tags, source, kind, args))
}

#[inline(always)]
pub(crate) fn bytes_left(kind: &[u8], source: Option<&Source>, args: &Args) -> isize {
    let mut size = kind.len() + 2; // Newline.
    if let Some(src) = source {
        size += 2 + src.len();
    }
    if !args.is_empty() {
        size += args.len_bytes() + 1;
    }
    let size: isize = size.try_into().unwrap_or(isize::MAX);
    512 - size
}

#[inline(always)]
fn write_to(
    tags: &Tags,
    source: Option<&Source>,
    kind: &[u8],
    args: &Args,
    write: &mut (impl std::io::Write + ?Sized),
) -> std::io::Result<()> {
    if !tags.is_empty() {
        tags.write_to(write)?;
        write.write_all(b" ")?;
    }
    if let Some(source) = source {
        write.write_all(b":")?;
        source.write_to(write)?;
        write.write_all(b" ")?;
    }
    write.write_all(kind)?;
    let (words, last) = args.split_last();
    for word in words {
        write.write_all(b" ")?;
        write.write_all(word)?;
    }
    if let Some(last) = last {
        if args.is_last_long() {
            write.write_all(b" :")?;
        } else {
            write.write_all(b" ")?;
        }
        write.write_all(last)?;
    }
    Ok(())
}

/// Stateless encoder/decoder for raw IRC messages on a client.
///
/// Encodes [`ClientMsg`]s and decodes [`ServerMsg`]s.
///
/// If the `tokio-codec` feature is enabled, this type implements
/// [`Decoder`][tokio_util::codec::Decoder] and [`Encoder`][tokio_util::codec::Encoder].
#[derive(Clone, Copy, Debug, Default)]
pub struct ClientCodec;

/// Stateless encoder/decoder for raw IRC messages on a server.
///
/// Encodes [`ServerMsg`]s and decodes [`ClientMsg`]s.
///
/// If the `tokio-codec` feature is enabled, this type implements
/// [`Decoder`][tokio_util::codec::Decoder] and [`Encoder`][tokio_util::codec::Encoder].
#[derive(Clone, Copy, Debug, Default)]
pub struct ServerCodec;

impl ClientCodec {
    /// Reads an owning server message from `read`.
    /// This function may block.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function that errored due to
    /// non-blocking I/O or unexpected EOF.
    /// Other errors may leave `buf` in an invalid state for future calls.
    pub fn read_owning_from<'a>(
        read: &mut (impl std::io::BufRead + ?Sized),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<ServerMsg<'a>> {
        use std::io::{BufRead, Read};
        read_msg!(
            ServerMsg::MAX_LEN,
            buf,
            read: Read,
            read.read_until(b'\n', buf),
            ServerMsg::parse(std::mem::take(buf))
        )
    }
    /// Asynchronously reads an owning server message from `read`.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function that errored due to
    /// non-blocking I/O or unexpected EOF.
    /// Other errors may leave `buf` in an invalid state for future calls.
    #[cfg(feature = "tokio")]
    pub async fn read_owning_from_tokio<'a>(
        read: &mut (impl tokio::io::AsyncBufReadExt + ?Sized + Unpin),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<ServerMsg<'a>> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt};
        read_msg!(
            ServerMsg::MAX_LEN,
            buf,
            read: AsyncReadExt,
            read.read_until(b'\n', buf).await,
            ServerMsg::parse(std::mem::take(buf))
        )
    }
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
    pub fn read_borrowing_from<'a>(
        read: &mut (impl std::io::BufRead + ?Sized),
        buf: &'a mut Vec<u8>,
    ) -> std::io::Result<ServerMsg<'a>> {
        use std::io::{BufRead, Read};
        read_msg!(
            ServerMsg::MAX_LEN,
            buf,
            read: Read,
            read.read_until(b'\n', buf),
            ServerMsg::parse(buf.as_slice())
        )
    }
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
    pub async fn read_borrowing_from_tokio<'a>(
        read: &mut (impl tokio::io::AsyncBufReadExt + ?Sized + Unpin),
        buf: &'a mut Vec<u8>,
    ) -> std::io::Result<ServerMsg<'a>> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt};
        read_msg!(
            ServerMsg::MAX_LEN,
            buf,
            read: AsyncReadExt,
            read.read_until(b'\n', buf).await,
            ServerMsg::parse(buf.as_slice())
        )
    }
    /// Writes a client message to the provided [`Write`] WITHOUT a trailing CRLF.
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(msg: &ClientMsg<'_>, write: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        write_to(&msg.tags, None, &msg.cmd, &msg.args, write)
    }
    /// Writes a client message to `write` WITH a trailing CRLF,
    /// using the provided buffer to minimize the necessary number of writes to `write`.
    ///
    /// The buffer will be cleared after successfully sending this message.
    /// If the buffer is non-empty, message data will be appended to the buffer's contents.
    pub fn send_to(
        msg: &ClientMsg<'_>,
        write: &mut (impl Write + ?Sized),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<()> {
        Self::write_to(msg, buf)?;
        buf.extend_from_slice(b"\r\n");
        write.write_all(buf)?;
        buf.clear();
        Ok(())
    }
    /// Asynchronously writes a client message to `write` WITH a trailing CRLF,
    /// using the provided buffer to minimize the necessary number of writes to `write`.
    ///
    /// The buffer will be cleared after successfully sending this message.
    /// If the buffer is non-empty, message data will be appended to the buffer's contents.
    #[cfg(feature = "tokio")]
    pub async fn send_to_tokio(
        msg: &ClientMsg<'_>,
        write: &mut (impl tokio::io::AsyncWriteExt + ?Sized + Unpin),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<()> {
        Self::write_to(msg, buf)?;
        buf.extend_from_slice(b"\r\n");
        write.write_all(buf).await?;
        buf.clear();
        Ok(())
    }
}

impl ServerCodec {
    /// Reads an owning client message from `read`.
    /// This function may block.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function that errored due to
    /// non-blocking I/O or unexpected EOF.
    /// Other errors may leave `buf` in an invalid state for future calls.
    pub fn read_owning_from<'a>(
        read: &mut (impl std::io::BufRead + ?Sized),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<ClientMsg<'a>> {
        use std::io::{BufRead, Read};
        read_msg!(
            ClientMsg::MAX_LEN,
            buf,
            read: Read,
            read.read_until(b'\n', buf),
            ClientMsg::parse(std::mem::take(buf))
        )
    }
    /// Asynchronously reads an owning client message from `read`.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function that errored due to
    /// non-blocking I/O or unexpected EOF.
    /// Other errors may leave `buf` in an invalid state for future calls.
    #[cfg(feature = "tokio")]
    pub async fn read_owning_from_tokio<'a>(
        read: &mut (impl tokio::io::AsyncBufReadExt + ?Sized + Unpin),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<ClientMsg<'a>> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt};
        read_msg!(
            ClientMsg::MAX_LEN,
            buf,
            read: AsyncReadExt,
            read.read_until(b'\n', buf).await,
            ClientMsg::parse(std::mem::take(buf))
        )
    }
    /// Reads a client message from `read`.
    /// This function may block.
    ///
    /// Consider using [`ClientMsg::read_owning_from`] instead
    /// unless minimizing memory allocations is very important.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function that errored due to
    /// non-blocking I/O or unexpected EOF.
    /// Other errors may leave `buf` in an invalid state for future calls.
    pub fn read_borrowing_from<'a>(
        read: &mut (impl std::io::BufRead + ?Sized),
        buf: &'a mut Vec<u8>,
    ) -> std::io::Result<ClientMsg<'a>> {
        use std::io::{BufRead, Read};
        read_msg!(
            ClientMsg::MAX_LEN,
            buf,
            read: Read,
            read.read_until(b'\n', buf),
            ClientMsg::parse(buf.as_slice())
        )
    }
    /// Asynchronously reads a client message from `read`.
    ///
    /// Consider using [`ClientMsg::read_owning_from_tokio`] instead
    /// unless minimizing memory allocations is very important.
    ///
    /// `buf` must either be empty or contain a partial message from
    /// a previous call to this function that errored due to
    /// non-blocking I/O or unexpected EOF.
    /// Other errors may leave `buf` in an invalid state for future calls.
    #[cfg(feature = "tokio")]
    pub async fn read_borrowing_from_tokio<'a>(
        read: &mut (impl tokio::io::AsyncBufReadExt + ?Sized + Unpin),
        buf: &'a mut Vec<u8>,
    ) -> std::io::Result<ClientMsg<'a>> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt};
        read_msg!(
            ClientMsg::MAX_LEN,
            buf,
            read: AsyncReadExt,
            read.read_until(b'\n', buf).await,
            ClientMsg::parse(buf.as_slice())
        )
    }
    /// Writes a server message to the provided [`Write`] WITHOUT a trailing CRLF.
    ///
    /// This function makes many small writes. Buffering is strongly recommended.
    pub fn write_to(msg: &ServerMsg<'_>, write: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        write_to(&msg.tags, msg.source.as_deref(), &msg.kind.as_arg(), &msg.args, write)
    }
    /// Writes a server message to `write` WITH a trailing CRLF,
    /// using the provided buffer to minimize the necessary number of writes to `write`.
    ///
    /// The buffer will be cleared after successfully sending this message.
    /// If the buffer is non-empty, message data will be appended to the buffer's contents.
    pub fn send_to(
        msg: &ServerMsg<'_>,
        write: &mut (impl Write + ?Sized),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<()> {
        Self::write_to(msg, buf)?;
        buf.extend_from_slice(b"\r\n");
        write.write_all(buf)?;
        buf.clear();
        Ok(())
    }
    /// Asynchronously writes a server message to `write` WITH a trailing CRLF,
    /// using the provided buffer to minimize the necessary number of writes to `write`.
    ///
    /// The buffer will be cleared after successfully sending this message.
    /// If the buffer is non-empty, message data will be appended to the buffer's contents.
    #[cfg(feature = "tokio")]
    pub async fn send_to_tokio(
        msg: &ServerMsg<'_>,
        write: &mut (impl tokio::io::AsyncWriteExt + ?Sized + Unpin),
        buf: &mut Vec<u8>,
    ) -> std::io::Result<()> {
        Self::write_to(msg, buf)?;
        buf.extend_from_slice(b"\r\n");
        write.write_all(buf).await?;
        buf.clear();
        Ok(())
    }
}

#[cfg(feature = "tokio-codec")]
pub(super) mod tokio_codec {
    use super::{ClientCodec, ServerCodec};
    use crate::{
        ircmsg::{ClientMsg, ServerMsg},
        string::Line,
    };
    use std::num::NonZeroUsize;
    use tokio_util::{
        bytes::{Buf, BufMut, BytesMut},
        codec::{Decoder, Encoder},
    };

    impl Encoder<ClientMsg<'_>> for ClientCodec {
        type Error = std::io::Error;

        fn encode(&mut self, item: ClientMsg<'_>, dst: &mut BytesMut) -> Result<(), Self::Error> {
            Self::write_to(&item, &mut dst.writer())?;
            dst.extend_from_slice(b"\r\n");
            Ok(())
        }
    }
    impl Encoder<ServerMsg<'_>> for ServerCodec {
        type Error = std::io::Error;

        fn encode(&mut self, item: ServerMsg<'_>, dst: &mut BytesMut) -> Result<(), Self::Error> {
            Self::write_to(&item, &mut dst.writer())?;
            dst.extend_from_slice(b"\r\n");
            Ok(())
        }
    }

    pub fn scroll_buf(buf: &mut BytesMut, limit: usize) -> Option<NonZeroUsize> {
        let mut leading_ws = true;
        let mut trimmed_ws = 0usize;
        let mut end_idx = 0usize;
        for byte in buf.iter() {
            if leading_ws {
                if byte.is_ascii_whitespace() {
                    trimmed_ws += 1;
                    continue;
                }
                leading_ws = false;
            }
            end_idx += 1;
            if end_idx >= limit || *byte == b'\n' || *byte == b'\0' {
                buf.advance(trimmed_ws);
                return NonZeroUsize::new(end_idx);
            }
        }
        buf.advance(trimmed_ws);
        None
    }

    impl Decoder for ClientCodec {
        type Item = ServerMsg<'static>;
        type Error = std::io::Error;

        fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            let Some(split_at) = scroll_buf(src, ServerMsg::MAX_LEN) else {
                src.reserve(ServerMsg::MAX_LEN.saturating_sub(src.len()));
                return Ok(None);
            };
            let line_raw = src.split_to(split_at.get());
            let line = Line::from_bytes(line_raw.as_ref())?;
            Ok(Some(ServerMsg::parse(line.owning())?))
        }
    }
    impl Decoder for ServerCodec {
        type Item = ClientMsg<'static>;
        type Error = std::io::Error;

        fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            let Some(split_at) = scroll_buf(src, ClientMsg::MAX_LEN) else {
                src.reserve(ClientMsg::MAX_LEN.saturating_sub(src.len()));
                return Ok(None);
            };
            let line_raw = src.split_to(split_at.get());
            let line = Line::from_bytes(line_raw.as_ref())?;
            Ok(Some(ClientMsg::parse(line.owning())?))
        }
    }
}

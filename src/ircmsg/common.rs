use super::{Args, Source, Tags};
use crate::error::{InvalidByte, ParseError};
use crate::string::{Line, Splitter, Word};

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
                // 256 bytes is 1/2 the largest IRCv2 message,
                // ensuring at most 1 realloc for tagless messages
                // assuming Vec's growing strategy doesn't change for non-tiny allocations.
                // IME most messages should fit in 256 bytes anyway.
                $buf.reserve_exact(std::cmp::min(read_count, 256));
                $read_expr?;
            }
            let mut found_newline = false;
            loop {
                match $buf.last() {
                    None => break,
                    Some(b'\n') => {
                        found_newline = true;
                        $buf.pop();
                    }
                    Some(b'\r') => {
                        $buf.pop();
                    }
                    Some(_) if found_newline => {
                        return match $parse_expr {
                            Ok(msg) => {
                                #[cfg(feature = "tracing")]
                                tracing::debug!(target: "vinezombie::recv", "{}", msg);
                                Ok(msg)
                            }
                            Err(e) => Err(e.into()),
                        }
                    }
                    _ => {
                        $buf.clear();
                        return if $buf.len() < $limit {
                            Err(Error::from(ErrorKind::UnexpectedEof))
                        } else {
                            Err(ParseError::TooLong.into())
                        };
                    }
                }
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
            return Err(ParseError::InvalidKind(InvalidByte::new_empty()));
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
            None => return Err(ParseError::InvalidKind(InvalidByte::new_empty())),
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
        size += 1; // Colon.
        for arg in args.all() {
            size += arg.len() + 1; // Space.
        }
    }
    let size: isize = size.try_into().unwrap_or(isize::MAX);
    512 - size
}

#[inline(always)]
pub(crate) fn write_to(
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

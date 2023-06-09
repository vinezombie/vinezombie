use super::{Args, Source, Tags};
use crate::string::{InvalidByte, Line, Word};

/// Error type when parsing an IRC message..
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParseError {
    /// Message exceeds permissible length limits.
    TooLong(usize),
    /// The string provided to a parse function is not a Line.
    InvalidLine(InvalidByte),
    /// The source fragment of the message contains an invalid nickname.
    InvalidNick(InvalidByte),
    /// The source fragment of the message contains an invalid username.
    InvalidUser(InvalidByte),
    /// The message's kind is invalid.
    InvalidKind(InvalidByte),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::TooLong(len) => write!(f, "message is too long (>{len} bytes)"),
            ParseError::InvalidLine(e) => write!(f, "invalid line: {e}"),
            ParseError::InvalidNick(e) => write!(f, "invalid source nickname: {e}"),
            ParseError::InvalidUser(e) => write!(f, "invalid source username: {e}"),
            ParseError::InvalidKind(e) => write!(f, "invalid message kind: {e}"),
        }
    }
}

impl std::error::Error for ParseError {}

#[inline(always)]
pub(crate) fn parse<'a, S: 'a, K: 'a>(
    mut msg: Line<'a>,
    parse_source: impl Fn(Word<'a>) -> Result<S, ParseError>,
    parse_kind: impl FnOnce(Word<'a>) -> Result<K, ParseError>,
) -> Result<(Tags<'a>, Option<S>, K, Args<'a>), ParseError> {
    use crate::string::tf::{SplitFirst, SplitWord};
    let mut tags = Tags::new();
    let mut source = None;
    let mut expect_tags = true;
    let mut expect_source = true;
    let kind = loop {
        let mut word = msg.transform(SplitWord);
        if word.is_empty() {
            return Err(ParseError::InvalidKind(InvalidByte::new_empty()));
        }
        match word.first() {
            Some(b'@') if expect_tags => {
                expect_tags = false;
                word.transform(SplitFirst);
                tags = Tags::parse(word);
            }
            Some(b':') if expect_source => {
                expect_tags = false;
                expect_source = false;
                word.transform(SplitFirst);
                // Maybe not quiet failure here?
                // Non-parsed sources can sometimes still be useful.
                source = Some(parse_source(word)?);
            }
            Some(_) => break word,
            None => return Err(ParseError::InvalidKind(InvalidByte::new_empty())),
        }
    };
    let kind = parse_kind(kind)?;
    let args = Args::parse(msg);
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

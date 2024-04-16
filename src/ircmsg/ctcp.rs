use crate::string::{tf::AsciiCasemap, Builder, Line, Splitter, Word};

/// A pair combining a CTCP query/reply (or empty if not applicable) and its data.
///
/// This type is written under the assumption that CTCP data, if present, will span the length
/// of a PRIVMSG/NOTICE.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct MaybeCtcp<'a, T> {
    /// The CTCP command, or empty if this is not a CTCP command.
    pub cmd: Word<'a>,
    /// The body of the CTCP message, or the whole message if [`cmd`][MaybeCtcp::cmd] is empty.
    pub body: T,
}

impl<'a> MaybeCtcp<'a, Line<'a>> {
    /// Parses `self` out of [`Line`].
    pub fn parse(value: Line<'a>) -> Self {
        let mut splitter = Splitter::new(value.clone());
        // Check for a leading '\x01'.
        if splitter.next_byte() == Some(1) {
            splitter.until_byte_eq(1);
            let mut cmd = splitter.string_or_default::<Word<'_>>(false);
            cmd.transform(AsciiCasemap::<true>);
            splitter.next_byte();
            let body = splitter.rest_or_default();
            MaybeCtcp { cmd, body }
        } else {
            MaybeCtcp { cmd: Word::default(), body: value }
        }
    }
    /// Converts `self` back into a single [`Line`], suitable for inclusion into a message.
    pub fn into_line<'b>(self) -> Line<'b> {
        if self.is_ctcp() {
            let mut builder = Builder::new(Line::default());
            builder.reserve_exact(self.len());
            let _ = builder.try_push_char('\x01');
            builder.append(self.cmd);
            let _ = builder.try_push_char(' ');
            builder.append(self.body);
            let _ = builder.try_push_char('\x01');
            builder.build()
        } else {
            self.body.owning()
        }
    }
    /// Returns `true` if both the command and data are empty.
    pub fn is_empty(&self) -> bool {
        self.cmd.is_empty() && self.body.is_empty()
    }
    /// Returns the length of `self` in bytes, if `self` were converted back into a [`Line`].
    #[inline]
    pub fn len(&self) -> usize {
        if self.cmd.is_empty() {
            self.body.len()
        } else {
            // 3: Two \01s and a space after the command.
            self.cmd.len() + 3 + self.body.len()
        }
    }
}

impl<'a, T> MaybeCtcp<'a, T> {
    /// Returns `true` if [`cmd`][MaybeCtcp::cmd] is non-empty.
    pub fn is_ctcp(&self) -> bool {
        !self.cmd.is_empty()
    }

    /// Applies function `f` to the [`body`][Self::body] of this message.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> MaybeCtcp<'a, U> {
        let MaybeCtcp { cmd, body } = self;
        let body = f(body);
        MaybeCtcp { cmd, body }
    }

    /// Applies fallible function `f` to the [`body`][Self::body] of this message.
    pub fn map_result<U, E>(
        self,
        f: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<MaybeCtcp<'a, U>, E> {
        let MaybeCtcp { cmd, body } = self;
        let body = f(body)?;
        Ok(MaybeCtcp { cmd, body })
    }

    /// Applies iterator-generating function `f` to the [`body`][Self::body] of this message.
    pub fn map_iter<U, I: IntoIterator<Item = U>>(
        self,
        f: impl FnOnce(T) -> I,
    ) -> impl Iterator<Item = MaybeCtcp<'a, U>> {
        let MaybeCtcp { cmd, body } = self;
        f(body).into_iter().map(move |body| MaybeCtcp { cmd: cmd.clone(), body })
    }
}

impl<'a> From<Line<'a>> for MaybeCtcp<'a, Line<'a>> {
    fn from(value: Line<'a>) -> Self {
        Self::parse(value)
    }
}
impl<'a, 'b> From<MaybeCtcp<'a, Line<'a>>> for Line<'b> {
    fn from(value: MaybeCtcp<'a, Line<'a>>) -> Self {
        value.into_line()
    }
}

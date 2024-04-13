use crate::string::{tf::AsciiCasemap, Builder, Line, Splitter, Word};

/// A pair combining a CTCP query/reply (or empty if not applicable) and its data.
///
/// This type is written under the assumption that CTCP data, if present, will span the length
/// of a PRIVMSG/NOTICE.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct MaybeCtcp<'a, T>(pub Word<'a>, pub T);

impl<'a> MaybeCtcp<'a, Line<'a>> {
    /// Returns the length of `self` in bytes, if `self` were converted back into a [`Line`].
    #[inline]
    pub fn len(&self) -> usize {
        if self.0.is_empty() {
            self.1.len()
        } else {
            // 3: Two \01s and a space after the command.
            self.1.len() + 3 + self.0.len()
        }
    }
}

impl<'a> From<Line<'a>> for MaybeCtcp<'a, Line<'a>> {
    fn from(value: Line<'a>) -> Self {
        let mut splitter = Splitter::new(value);
        // Check for a leading '\x01'.
        if splitter.peek_byte() == Some(1) {
            let mut cmd = splitter.string_or_default::<Word<'_>>(false);
            cmd.transform(AsciiCasemap::<true>);
            splitter.next_byte();
            // Check for and consume but don't require a trailing '\x01'.
            if splitter.rpeek_byte() == Some(1) {
                splitter.rnext_byte();
            }
            let body = splitter.rest_or_default();
            MaybeCtcp(cmd, body)
        } else {
            MaybeCtcp(Word::default(), splitter.rest_or_default())
        }
    }
}

impl<'a, 'b> From<MaybeCtcp<'a, Line<'a>>> for Line<'b> {
    fn from(value: MaybeCtcp<'a, Line<'a>>) -> Self {
        if value.0.is_empty() {
            value.1.owning()
        } else {
            let mut builder = Builder::new(Line::default());
            builder.reserve_exact(value.len());
            let _ = builder.try_push_char('\x01');
            builder.append(value.0);
            let _ = builder.try_push_char(' ');
            builder.append(value.1);
            let _ = builder.try_push_char('\x01');
            builder.build()
        }
    }
}

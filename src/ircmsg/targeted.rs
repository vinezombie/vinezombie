use super::{SharedSource, Tags};
use crate::string::Arg;

/// A message type for parsed messages sent to specific targets (e.g. CTCP queries/replies).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TargetedMsg<'a, T> {
    /// This message's tags, if any.
    pub tags: Tags<'a>,
    /// Where this message originated.
    pub source: Option<SharedSource<'a>>,
    /// Who this message is being sent to.
    pub target: Arg<'a>,
    /// This message's contents.
    pub value: T,
}

impl<'a, T: Default> TargetedMsg<'a, T> {
    /// Creates a message for the provided target and a default-initialized value.
    pub fn new_default(target: Arg<'a>) -> Self {
        Self::new(target, T::default())
    }
}

impl<'a, T> TargetedMsg<'a, T> {
    /// Creates a message for the provided target and value.
    pub const fn new(target: Arg<'a>, value: T) -> Self {
        TargetedMsg { tags: Tags::new(), source: None, target, value }
    }

    /// Applies function `f` to the [`value`][Self::value] of this message.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> TargetedMsg<'a, U> {
        let TargetedMsg { tags, source, target, value } = self;
        let value = f(value);
        TargetedMsg { tags, source, target, value }
    }

    /// Applies fallible function `f` to the [`value`][Self::value] of this message.
    pub fn map_result<U, E>(
        self,
        f: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<TargetedMsg<'a, U>, E> {
        let TargetedMsg { tags, source, target, value } = self;
        let value = f(value)?;
        Ok(TargetedMsg { tags, source, target, value })
    }

    /// Applies iterator-generating function `f` to the [`value`][Self::value] of this message.
    pub fn map_iter<U, I: IntoIterator<Item = U>>(
        self,
        f: impl FnOnce(T) -> I,
    ) -> impl Iterator<Item = TargetedMsg<'a, U>> {
        let TargetedMsg { tags, source, target, value } = self;
        f(value).into_iter().map(move |value| TargetedMsg {
            tags: tags.clone(),
            source: source.clone(),
            target: target.clone(),
            value,
        })
    }
}

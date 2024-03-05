use std::{
    fmt::Write,
    iter::FusedIterator,
    num::{NonZeroU64, NonZeroU8},
};

/// A single mode letter.
///
/// This is a newtype around an ASCII alphabetic character.
/// It uses a different [`Ord`][std::cmp::Ord] implementation
/// that orders alphabetically first and by capitalization second.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Mode(std::num::NonZeroU8);

impl Mode {
    /// Creates a new `Mode` from the given ASCII letter.
    pub const fn new(letter: u8) -> Option<Mode> {
        if letter >= b'a' && letter <= b'z' || letter >= b'A' && letter <= b'Z' {
            Some(unsafe { Self::new_unchecked(letter) })
        } else {
            None
        }
    }
    /// Creates a new `Mode` from the given ASCII letter with no validity checks.
    ///
    /// # Safety
    /// The letter must be an ASCII alphabetic character.
    /// Undefined behavior may result otherwise.
    pub const unsafe fn new_unchecked(letter: u8) -> Mode {
        Mode(NonZeroU8::new_unchecked(letter))
    }
    /// Converts `self` into a [`NonZeroU8`].
    pub const fn into_nonzero_u8(self) -> NonZeroU8 {
        self.0
    }
    /// Converts `self` into a `char`.
    pub fn into_char(self) -> char {
        // Aw, it's not const.
        // Safety: We're an ASCII alphabetic character.
        unsafe { char::from_u32_unchecked(self.0.get() as u32) }
    }
    pub(self) const fn index(self) -> u8 {
        let raw = self.0.get();
        let letter_id = 26 - (raw & 31);
        let case = (raw >= b'a') as u8;
        (letter_id << 1) + case
    }
}

impl std::cmp::PartialOrd for Mode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for Mode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.index().cmp(&self.index())
    }
}

impl std::fmt::Debug for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.into_char().fmt(f)
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_char(self.into_char())
    }
}

impl<'a> From<Mode> for crate::string::Arg<'a> {
    fn from(value: Mode) -> Self {
        let mut buf = [0];
        let bytes = &*value.into_char().encode_utf8(&mut buf);
        unsafe { crate::string::Arg::from_unchecked(bytes.into()).owning() }
    }
}

impl From<Mode> for NonZeroU8 {
    fn from(value: Mode) -> Self {
        value.into_nonzero_u8()
    }
}

impl From<Mode> for u8 {
    fn from(value: Mode) -> Self {
        value.into_nonzero_u8().get()
    }
}

impl From<Mode> for char {
    fn from(value: Mode) -> Self {
        value.into_char()
    }
}

// Impls needed for ModeSet.
impl Mode {
    pub(self) unsafe fn new_from_index(index: u32) -> Mode {
        // index = 51 - letter_id - case
        // letter = letter_id + (case << 5)
        let basis = b'A' + ((index & 1) << 5) as u8;
        Mode(NonZeroU8::new_unchecked(basis + (25 - (index >> 1) as u8)))
    }
    pub(self) const fn mask(self) -> u64 {
        // TODO: unchecked_shl
        1u64 << (self.index() as u64)
    }
}

/// A set of (non-list) modes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ModeSet(u64);

impl ModeSet {
    /// Creates a new, empty `ModeSet`.
    pub const fn new() -> ModeSet {
        ModeSet(0)
    }
    /// Returns a version of `self` with the provided mode set.
    pub const fn with(self, mode: Mode) -> ModeSet {
        ModeSet(self.0 | mode.mask())
    }
    /// Sets a mode.
    ///
    /// Returns `true` if there was a change.
    #[inline]
    pub fn set(&mut self, mode: Mode) -> bool {
        let old = self.0;
        self.0 |= mode.mask();
        old != self.0
    }
    /// Unsets a mode.
    ///
    /// Returns `true` if there was a change.
    #[inline]
    pub fn unset(&mut self, mode: Mode) -> bool {
        let old = self.0;
        self.0 &= !mode.mask();
        old != self.0
    }
    /// Tests if a mode is set.
    pub const fn contains(&self, mode: Mode) -> bool {
        (self.0 & mode.mask()) != 0
    }
    /// Returns false if this set is empty.
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }
    /// Returns the number of modes in this set.
    pub const fn len(&self) -> usize {
        self.0.count_ones() as usize
    }
    // TODO: Set operations, converting to strings.
}

impl IntoIterator for ModeSet {
    type Item = Mode;

    type IntoIter = ModeSetIter;

    fn into_iter(self) -> Self::IntoIter {
        ModeSetIter(self.0)
    }
}

impl IntoIterator for &ModeSet {
    type Item = Mode;

    type IntoIter = ModeSetIter;

    fn into_iter(self) -> Self::IntoIter {
        ModeSetIter(self.0)
    }
}

impl FromIterator<Mode> for ModeSet {
    fn from_iter<T: IntoIterator<Item = Mode>>(iter: T) -> Self {
        let mut ms = ModeSet::new();
        for mode in iter {
            ms.set(mode);
        }
        ms
    }
}

impl std::fmt::Debug for ModeSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self).finish()
    }
}

impl std::fmt::Display for ModeSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for mode in self {
            f.write_char(mode.into_char())?;
        }
        Ok(())
    }
}

/// Iterator over [`ModeSet`]s.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct ModeSetIter(u64);

impl Iterator for ModeSetIter {
    type Item = Mode;

    fn next(&mut self) -> Option<Self::Item> {
        NonZeroU64::new(self.0).map(|value| {
            let highest_bit = value.ilog2();
            let mode = unsafe { Mode::new_from_index(highest_bit) };
            self.0 &= !mode.mask();
            mode
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let retval = self.0.count_ones() as usize;
        (retval, Some(retval))
    }
}

impl DoubleEndedIterator for ModeSetIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        NonZeroU64::new(self.0).map(|value| {
            let lowest_bit = value.trailing_zeros();
            let mode = unsafe { Mode::new_from_index(lowest_bit) };
            self.0 &= !mode.mask();
            mode
        })
    }
}

impl FusedIterator for ModeSetIter {}
impl ExactSizeIterator for ModeSetIter {}

// TODO: ModeMap.

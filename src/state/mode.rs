use std::{
    fmt::Write,
    iter::FusedIterator,
    num::{NonZeroU64, NonZeroU8},
};

use crate::{
    error::ParseError,
    names::{ISupport, NameMap},
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
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
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
    /// Returns a set with all of the modes in both sets.
    pub const fn union(self, b: Self) -> Self {
        ModeSet(self.0 | b.0)
    }
    /// Returns a set with only the modes in both sets.
    pub const fn intersection(self, b: Self) -> Self {
        ModeSet(self.0 & b.0)
    }
    /// Returns a set with only the modes that are in `self` but not `b`.
    pub const fn difference(self, b: Self) -> Self {
        ModeSet(self.0 & !b.0)
    }
}

impl PartialOrd for ModeSet {
    fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
        let intersect = self.intersection(*b);
        match (intersect == *self, intersect == *b) {
            (true, false) => Some(std::cmp::Ordering::Less),
            (false, true) => Some(std::cmp::Ordering::Greater),
            (true, true) => Some(std::cmp::Ordering::Equal),
            (false, false) => None,
        }
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

/// The various types of (channel) modes.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ModeType {
    // WARNING: Later transmutation of ints back into these variants!
    /// Type A modes, the list modes.
    TypeA = 0,
    /// Type B modes, the parameterized modes that require a parameter to unset.
    TypeB,
    /// Type C modes, the parameterized modes.
    TypeC,
    /// Type D modes, the unitary modes.
    #[default]
    TypeD,
    /// Channel status modes.
    Status,
}

impl ModeType {
    pub(self) fn index(self) -> usize {
        (self as u8) as usize
    }
    /// Returns `true` if this mode can be meaningfully set multiple times.
    pub fn is_listlike(self) -> bool {
        matches!(self, Self::TypeA | Self::Status)
    }
    /// Returns `true` if a mode of this type needs an argument to be set.
    pub fn needs_arg_to_set(self) -> bool {
        !matches!(self, Self::TypeD)
    }
    /// Returns `true` if a mode of this type needs an argument to be unset.
    pub fn needs_arg_to_unset(self) -> bool {
        matches!(self, Self::TypeA | Self::TypeB | Self::Status)
    }
}

/// A map of the non-status modes to their [`ModeType`]s.
///
/// This type explicitly excludes status modes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ModeTypes([ModeSet; 4]);

impl ModeTypes {
    /// Returns a new, empty `ModeTypes`.
    pub const fn new() -> Self {
        ModeTypes([ModeSet::new(); 4])
    }
    /// Returns a `ModeTypes` constructed from one set of modes of a given [`ModeType`].
    pub fn from_set(set: ModeSet, mode_type: ModeType) -> Self {
        match mode_type {
            ModeType::TypeA => ModeTypes([set, ModeSet::new(), ModeSet::new(), ModeSet::new()]),
            ModeType::TypeB => ModeTypes([ModeSet::new(), set, ModeSet::new(), ModeSet::new()]),
            ModeType::TypeC => ModeTypes([ModeSet::new(), ModeSet::new(), set, ModeSet::new()]),
            ModeType::TypeD => ModeTypes([ModeSet::new(), ModeSet::new(), ModeSet::new(), set]),
            _ => ModeTypes::new(),
        }
    }
    /// Returns a `ModeTypes` constructed from the provided sets.
    ///
    /// To ensure coherence, any overlaps between sets are removed
    /// and returned as a separate set. Use [`insert_set()`][Self::insert_set] to add them back.
    pub fn from_sets(a: ModeSet, b: ModeSet, c: ModeSet, d: ModeSet) -> (Self, ModeSet) {
        let mut overlap = a.intersection(b);
        overlap = overlap.union(b.intersection(c));
        overlap = overlap.union(c.intersection(d));
        overlap = overlap.union(b.intersection(d));
        overlap = overlap.union(c.intersection(d));
        (
            ModeTypes([
                a.difference(overlap),
                b.difference(overlap),
                c.difference(overlap),
                d.difference(overlap),
            ]),
            overlap,
        )
    }
    /// Parses a string as if it's the value of a `CHANMODES` ISUPPORT token.
    ///
    /// This function is deliberately permissive to support mode strings with modes that are not
    /// valid [`Mode`]s.
    ///
    /// In addition to `Self`, returns the rest of the mode string if there are additional
    /// mode types beyond what this type supports.
    /// It also returns any overlapping modes as [`from_sets`][Self::from_sets].
    pub fn parse(mut bytes: &[u8]) -> (Self, ModeSet, &[u8]) {
        let mut sets = [ModeSet::new(); 4];
        let mut set_iter = sets.iter_mut();
        let mut set = set_iter.next().unwrap();
        while let Some((byte, rest)) = bytes.split_first() {
            let byte = *byte;
            bytes = rest;
            if byte == b',' {
                let Some(next_set) = set_iter.next() else {
                    break;
                };
                set = next_set;
            } else if let Some(mode) = Mode::new(byte) {
                set.set(mode);
            }
        }
        let [a, b, c, d] = sets;
        let (retval, overlap) = Self::from_sets(a, b, c, d);
        (retval, overlap, bytes)
    }
    /// Returns the [`ModeType`] the provided mode, if known.
    pub fn get(&self, mode: Mode) -> Option<ModeType> {
        for value in [3u8, 0u8, 1u8, 2u8] {
            if self.0[value as usize].contains(mode) {
                // Safety: The values are a subset of valid values for ModeType.
                return Some(unsafe { std::mem::transmute(value) });
            }
        }
        None
    }
    /// Sets the provided [`Mode`]'s [`ModeType`].
    pub fn insert(&mut self, mode: Mode, mode_type: ModeType) {
        for (idx, set) in self.0.iter_mut().enumerate() {
            if idx != mode_type.index() {
                set.unset(mode);
            } else {
                set.set(mode);
            }
        }
    }
    /// Sets the [`ModeType`] for all the modes in the provided [`ModeSet`].
    pub fn insert_set(&mut self, mode_set: ModeSet, mode_type: ModeType) {
        for (idx, set) in self.0.iter_mut().enumerate() {
            if idx != mode_type.index() {
                *set = set.difference(mode_set);
            } else {
                *set = set.union(mode_set);
            }
        }
    }
}

// TODO: PartialOrd

impl std::fmt::Display for ModeTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c, d] = self.0;
        write!(f, "{a},{b},{c},{d}")
    }
}

/// A bidirectional map of status modes; list-like modes that can be applied to strings.
///
/// This type assumes single-byte status prefixes in ASCII.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)] // TODO: Manual impl PartialOrd.
pub struct StatusModes {
    map: Vec<(Mode, u8)>,
}

impl StatusModes {
    /// Creates a new empty `StatusModes`.
    pub const fn new() -> Self {
        StatusModes { map: Vec::new() }
    }
    /// Parse `self` from the value of the `PREFIX` ISUPPORT token.
    pub fn parse(arg: &[u8]) -> Result<Self, ParseError> {
        let Some((b'(', arg)) = arg.split_first() else {
            return Err(ParseError::InvalidField(
                "PREFIX value".into(),
                "missing leading '('".into(),
            ));
        };
        let mut splitter = arg.splitn(2, |c| *c == b')');
        let Some(modes) = splitter.next() else {
            return Err(ParseError::InvalidField(
                "PREFIX value".into(),
                "empty string after '('".into(),
            ));
        };
        let Some(prefixes) = splitter.next() else {
            return Err(ParseError::InvalidField(
                "PREFIX value".into(),
                "missing closing ')'".into(),
            ));
        };
        let mut map = Vec::with_capacity(modes.len());
        let mut skip_prefixes = false;
        let mut mode_iter = modes.iter().copied();
        let mut prefix_iter = prefixes.iter().copied();
        loop {
            let prefix = if !skip_prefixes {
                prefix_iter.next().filter(u8::is_ascii).unwrap_or_else(|| {
                    skip_prefixes = true;
                    b'\0'
                })
            } else {
                b'\0'
            };
            let Some(mode) = mode_iter.next() else {
                break;
            };
            let Some(mode) = Mode::new(mode) else {
                continue;
            };
            for (mode_b, prefix_b) in map.iter().copied() {
                if mode == mode_b {
                    return Err(ParseError::InvalidField(
                        "PREFIX value".into(),
                        format!("duplicate mode `{mode}`").into(),
                    ));
                }
                if prefix == prefix_b {
                    return Err(ParseError::InvalidField(
                        "PREFIX value".into(),
                        format!("duplicate prefix `{}`", prefix.escape_ascii()).into(),
                    ));
                }
            }
            map.push((mode, prefix))
        }
        Ok(StatusModes { map })
    }
    /// Returns `true` if `self` contains a mapping for the provided mode letter.
    pub fn contains(&self, mode: Mode) -> bool {
        self.map.iter().any(|(m, _)| *m == mode)
    }
    /// Retrieves the prefix for a provided mode.
    ///
    /// Note that this potential returns `Some` for on a subset of values
    /// for which [`contains`][Self::contains] returns `true`.
    pub fn get_prefix(&self, mode: Mode) -> Option<NonZeroU8> {
        self.map.iter().find(|(m, _)| *m == mode).and_then(|(_, p)| NonZeroU8::new(*p))
    }
    /// Retrieves the mode for a provided status prefix.
    pub fn get_mode(&self, prefix: NonZeroU8) -> Option<Mode> {
        self.map.iter().find(|(_, p)| *p == prefix.get()).map(|pair| pair.0)
    }
    // TODO: Iter, ordering lookup methods.
}

/// The available channel modes on a server.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct ServerChanModes {
    nonstatus: ModeTypes,
    status: StatusModes,
    overlap: ModeSet,
    extra: Vec<ModeSet>,
}

impl ServerChanModes {
    /// Parse an instance of `self` from ISUPPORT tokens.
    pub fn from_isupport<T>(isupport: &NameMap<ISupport, T>) -> Self {
        use crate::names::isupport::{CHANMODES, PREFIX};
        let (nonstatus, overlap, extra) = if let Some((_, raw)) = isupport.get_union(CHANMODES) {
            let (nonstatus, overlap, mut extra_raw) = ModeTypes::parse(raw.as_bytes());
            let mut extra = Vec::new();
            let mut set = ModeSet::new();
            while let Some((byte, rest)) = extra_raw.split_first() {
                let byte = *byte;
                extra_raw = rest;
                if byte == b',' {
                    extra.push(std::mem::take(&mut set));
                } else if let Some(mode) = Mode::new(byte) {
                    set.set(mode);
                }
            }
            if !set.is_empty() {
                extra.push(set);
            }
            (nonstatus, overlap, extra)
        } else {
            (ModeTypes::new(), ModeSet::new(), Vec::new())
        };
        let status = if let Some((_, raw)) = isupport.get_union(PREFIX) {
            // TODO: Log error.
            StatusModes::parse(raw.as_bytes()).unwrap_or_default()
        } else {
            StatusModes::new()
        };
        ServerChanModes { nonstatus, status, overlap, extra }
    }
    /// Returns the [`ModeType`] the provided mode, if known.
    pub fn get(&self, mode: Mode) -> Option<ModeType> {
        if self.status.contains(mode) {
            Some(ModeType::Status)
        } else {
            self.nonstatus.get(mode)
        }
    }
}

// TODO: ModeMap.

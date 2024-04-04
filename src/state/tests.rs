use crate::state::StatusModes;

use super::{Mode, ModeSet, ModeType, ModeTypes};

static MODE_O: Mode = unsafe { Mode::new_unchecked(b'o') };
static MODE_RL: Mode = unsafe { Mode::new_unchecked(b'r') };
static MODE_RU: Mode = unsafe { Mode::new_unchecked(b'R') };
static MODE_SL: Mode = unsafe { Mode::new_unchecked(b's') };
static MODE_SU: Mode = unsafe { Mode::new_unchecked(b'S') };
static MODE_V: Mode = unsafe { Mode::new_unchecked(b'v') };

#[test]
fn mode_basic() {
    assert_eq!(MODE_RL.into_char(), 'r');
    assert_eq!(MODE_RU.into_char(), 'R');
    // For modes, r < R < s.
    assert!(MODE_RL < MODE_RU);
    assert!(MODE_RL < MODE_SL);
    assert!(MODE_RU < MODE_SL);
}

#[test]
fn modeset_basic() {
    let mut set = ModeSet::new();
    assert_eq!(set.len(), 0);
    assert!(set.set(MODE_RL));
    assert!(set.contains(MODE_RL));
    assert_eq!(set.len(), 1);
    assert!(set.set(MODE_RU));
    assert_eq!(set.len(), 2);
    assert!(!set.set(MODE_RU));
    assert_eq!(set.len(), 2);
    assert!(set.unset(MODE_RL));
    assert!(!set.contains(MODE_RL));
    assert!(set.contains(MODE_RU));
    assert_eq!(set.len(), 1);
}

#[test]
fn modeset_iter() {
    let set = ModeSet::new().with(MODE_RL).with(MODE_RU);
    let mut iter = set.into_iter();
    assert_eq!(iter.next(), Some(MODE_RL));
    assert_eq!(iter.next(), Some(MODE_RU));
    assert_eq!(iter.next(), None);
}

#[test]
fn modeset_iter_rev() {
    let set = ModeSet::new().with(MODE_RL).with(MODE_RU);
    let mut iter = set.into_iter();
    assert_eq!(iter.next_back(), Some(MODE_RU));
    assert_eq!(iter.next_back(), Some(MODE_RL));
    assert_eq!(iter.next_back(), None);
}

#[test]
fn modetypes_basic() {
    let map = ModeTypes::parse(b"r,R,,Ss,v").0;
    assert_eq!(map.get(MODE_V), None);
    assert_eq!(map.get(MODE_RL), Some(ModeType::TypeA));
    assert_eq!(map.get(MODE_RU), Some(ModeType::TypeB));
    assert_eq!(map.get(MODE_SL), Some(ModeType::TypeD));
    assert_eq!(map.get(MODE_SU), Some(ModeType::TypeD));
}

#[test]
fn statusmodes_basic() {
    use std::num::NonZeroU8;
    let classic = StatusModes::parse(b"(ov)@+").expect("classic PREFIX value should parse");
    assert_eq!(classic.get_prefix(MODE_V).map(NonZeroU8::get), Some(b'+'));
    assert_eq!(classic.get_prefix(MODE_O).map(NonZeroU8::get), Some(b'@'));
    assert_eq!(classic.get_mode(NonZeroU8::new(b'+').unwrap()), Some(MODE_V));
    assert_eq!(classic.get_mode(NonZeroU8::new(b'@').unwrap()), Some(MODE_O));
}

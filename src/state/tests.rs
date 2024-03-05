use super::{Mode, ModeSet};

static MODE_RL: Mode = unsafe { Mode::new_unchecked(b'r') };
static MODE_RU: Mode = unsafe { Mode::new_unchecked(b'R') };
static MODE_SL: Mode = unsafe { Mode::new_unchecked(b's') };

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

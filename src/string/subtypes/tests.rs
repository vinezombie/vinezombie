use super::{Arg, Line, Word};

#[test]
pub fn line() {
    assert!(Line::from_bytes("foo bar").is_ok());
    assert!(Line::from_bytes("").is_ok());
    assert!(Line::from_bytes("foobar\n").is_err());
    assert!(Line::from_bytes("foo\nbar").is_err());
    assert!(Line::from_bytes("foobar\r\n").is_err());
    assert!(Line::from_bytes("foobar\r").is_err());
    assert!(Line::from_bytes("foo\rbar").is_err());
}

#[test]
pub fn word() {
    assert!(Word::from_bytes("foobar").is_ok());
    assert!(Word::from_bytes("").is_ok());
    assert!(Word::from_bytes("foo\nbar").is_err());
    assert!(Word::from_bytes("foo bar").is_err());
    assert!(Word::from_bytes("foobar ").is_err());
    assert!(Word::from_bytes(" foobar").is_err());
}

#[test]
pub fn arg() {
    assert!(Arg::from_bytes("foobar").is_ok());
    assert!(Arg::from_bytes("foo:bar").is_ok());
    assert!(Arg::from_bytes("").is_err());
    assert!(Arg::from_bytes(":foo").is_err());
}

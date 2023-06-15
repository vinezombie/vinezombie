use super::{Bytes, Cmd, Word};

macro_rules! test_kind {
    ($word:expr) => {{
        let word = Word::from_bytes($word).unwrap();
        assert!(Cmd::from_word(word).is_err())
    }};
    ($word:expr, $expected:expr) => {{
        let word = Word::from_bytes($word).unwrap();
        assert_eq!(Cmd::from_word(word).unwrap(), $expected)
    }};
}

#[test]
fn kind_from_word() {
    test_kind!("someWord", "SOMEWORD");
    test_kind!("123");
    test_kind!("two-words");
}

#[test]
fn secrecy() {
    // Initialize Bytes so that it's already owning.
    let bytes_o = Bytes::from("hunter2".to_owned());
    assert!(!bytes_o.is_secret());
    let bytes_c = bytes_o.clone();
    let bytes_s = bytes_o.secret();
    assert!(!bytes_c.is_secret());
    assert!(bytes_s.is_secret());
    let bytes_s2 = bytes_s.clone();
    assert!(bytes_s2.is_secret());
}

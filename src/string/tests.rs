use super::{Cmd, Word};

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

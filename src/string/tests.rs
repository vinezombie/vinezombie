use super::{Kind, Word};

macro_rules! test_kind {
    ($word:expr) => {{
        let word = Word::from_bytes($word).unwrap();
        assert!(Kind::from_word(word).is_err())
    }};
    ($word:expr, $expected:expr) => {{
        let word = Word::from_bytes($word).unwrap();
        assert_eq!(Kind::from_word(word).unwrap(), $expected)
    }};
}

#[test]
fn kind_from_word() {
    test_kind!("someWord", "SOMEWORD");
    test_kind!("123", "123");
    test_kind!("two-words");
}

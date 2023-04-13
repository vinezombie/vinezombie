use crate::string::Bytes;

fn minus_to_plus(byte: &u8) -> u8 {
    let byte = *byte;
    if byte == b'-' {
        b'+'
    } else {
        byte
    }
}
#[test]
fn map_bytes() {
    use super::map_bytes;
    use crate::string::Utf8Policy;
    macro_rules! test_map_bytes {
        ($a:literal, $b:literal) => {
            assert_eq!(
                unsafe { map_bytes($a, Utf8Policy::PreserveStrict, minus_to_plus) }
                    .transformed
                    .as_ref(),
                $b
            )
        };
    }
    test_map_bytes!(b"+++", b"+++");
    test_map_bytes!(b"+-+", b"+++");
    test_map_bytes!(b"++-", b"+++");
    test_map_bytes!(b"--+", b"+++");
    test_map_bytes!(b"---", b"+++");
    test_map_bytes!(b"", b"");
    test_map_bytes!(b"-", b"+");
}

#[test]
fn split_word() {
    use super::SplitWord;
    let mut bytes = Bytes::from_bytes(b"foo bar  baz inga");
    assert_eq!(bytes.transform(&SplitWord), "foo");
    assert_eq!(bytes.transform(&SplitWord), "bar");
    assert_eq!(bytes.transform(&SplitWord), "baz");
    assert_eq!(bytes.transform(&SplitWord), "inga");
}

#[test]
fn split_first() {
    use super::SplitFirst;
    let mut bytes = Bytes::from_bytes(b"foo");
    assert_eq!(bytes.transform(&SplitFirst), Some(b'f'));
    assert_eq!(bytes.transform(&SplitFirst), Some(b'o'));
    assert_eq!(bytes.transform(&SplitFirst), Some(b'o'));
    assert_eq!(bytes.transform(&SplitFirst), None);
}

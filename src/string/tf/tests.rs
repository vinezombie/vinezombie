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

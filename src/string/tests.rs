use super::{Builder, Bytes, Cmd, Line, Splitter, Word};

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

#[test]
fn secrecy_empty() {
    let bytes_o = Bytes::empty();
    assert!(!bytes_o.is_secret());
    let bytes_s = bytes_o.secret();
    assert!(bytes_s.is_secret());
    let bytes_c = bytes_s.clone();
    assert!(bytes_c.is_secret());
}

#[test]
fn builder() {
    let mut builder = Builder::new(Line::from_str("foo"));
    builder.try_push(b' ').unwrap();
    builder.append(Word::from_str("bar"));
    builder.try_push_char(' ').unwrap();
    builder.try_append(b"baz").unwrap();
    let built: Line<'static> = builder.build();
    assert_eq!(built, "foo bar baz");
    assert_eq!(built.is_utf8_lazy(), Some(true));
}

#[test]
fn splitter_basic() {
    let mut splitter = Splitter::new(Line::from_str("foo  bar baz"));
    let word = splitter.string::<Word<'static>>(false).unwrap();
    assert_eq!(word, "foo");
    let empty = splitter.string::<Word<'static>>(false).unwrap();
    assert_eq!(empty, "");
    splitter.consume_whitespace();
    let word = splitter.rest::<Line<'static>>().unwrap();
    assert_eq!(word, "bar baz");
}

#[test]
fn splitter_until() {
    let mut splitter = Splitter::new(Line::from_str("foo.bar"));
    let word: Word = splitter.save_end().until_byte_eq(b'.').string_or_default(true);
    assert_eq!(word, "foo");
    assert_eq!(splitter.next_byte(), Some(b'.'));
}

#[test]
fn map_bytes() {
    fn minus_to_plus(byte: &u8) -> u8 {
        let byte = *byte;
        if byte == b'-' {
            b'+'
        } else {
            byte
        }
    }
    use super::tf::map_bytes;
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

#[cfg(feature = "base64")]
mod base64 {
    use crate::string::base64;
    // Test string taken from
    // https://ircv3.net/specs/extensions/sasl-3.1#example-protocol-exchange
    const STRING_ENC: &str = "amlsbGVzAGppbGxlcwBzZXNhbWU=";
    const STRING_ENC_PARTS: (&str, &str) = ("amlsbGVzAGppbGxlcwBz", "ZXNhbWU=");
    const STRING_DEC: &str = "jilles\0jilles\0sesame";

    #[test]
    fn chunk_decoder() {
        // Test 1: 1 short chunk.
        let mut decoder = base64::ChunkDecoder::new(400);
        let mut decoded = decoder.add(STRING_ENC).unwrap().unwrap();
        assert_eq!(decoded, STRING_DEC);
        // Test 2: 1 full chunk, 1 short chunk.
        decoder = base64::ChunkDecoder::new(20);
        assert!(decoder.add(STRING_ENC_PARTS.0).is_none());
        decoded = decoder.add(STRING_ENC_PARTS.1).unwrap().unwrap();
        assert_eq!(decoded, STRING_DEC);
        // Test 3: 1 full chunk, 1 empty chunk.
        decoder = base64::ChunkDecoder::new(28);
        assert!(decoder.add(STRING_ENC).is_none());
        decoded = decoder.add(b"+").unwrap().unwrap();
        assert_eq!(decoded, STRING_DEC);
    }
    #[test]
    fn chunk_encoder() {
        // Test 1: 1 short chunk.
        let mut encoder = base64::ChunkEncoder::new(STRING_DEC, 400, false);
        assert_eq!(encoder.len(), 1);
        assert_eq!(encoder.next().unwrap(), STRING_ENC);
        assert_eq!(encoder.len(), 0);
        assert!(encoder.next().is_none());
        // Test 2: 1 full chunk, 1 short chunk.
        encoder = base64::ChunkEncoder::new(STRING_DEC, 20, false);
        assert_eq!(encoder.len(), 2);
        assert_eq!(encoder.next().unwrap(), STRING_ENC_PARTS.0);
        assert_eq!(encoder.len(), 1);
        assert_eq!(encoder.next().unwrap(), STRING_ENC_PARTS.1);
        assert_eq!(encoder.len(), 0);
        assert!(encoder.next().is_none());
        // Test 3: 1 full chunk, 1 empty chunk.
        encoder = base64::ChunkEncoder::new(STRING_DEC, 28, false);
        assert_eq!(encoder.len(), 2);
        assert_eq!(encoder.next().unwrap(), STRING_ENC);
        assert_eq!(encoder.len(), 1);
        assert_eq!(encoder.next().unwrap(), b"+");
        assert_eq!(encoder.len(), 0);
        assert!(encoder.next().is_none());
    }
    #[test]
    fn chunk_encoder_empty() {
        let mut encoder = base64::ChunkEncoder::new("", 2, false);
        assert_eq!(encoder.len(), 1);
        assert_eq!(encoder.next().unwrap(), "+");
        assert_eq!(encoder.len(), 0);
        assert_eq!(encoder.next(), None);
    }
}

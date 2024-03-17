use super::{Suffix, SuffixStrategy, SuffixType};
use crate::{client::nick::NickTransformer, string::Nick};

#[test]
pub fn suffix_rng() {
    let prefix = Nick::from_bytes("Foo").unwrap();
    let gen = Suffix {
        suffixes: vec![SuffixType::Base8; 9].into(),
        strategy: SuffixStrategy::Rng(Some(1337)),
    };
    let (mut nick, mut gen) = gen.transform(prefix).next_nick();
    let mut prev: u32 = 9;
    for _ in 0..16 {
        let nick_str = nick.to_utf8().unwrap();
        let num = nick_str.strip_prefix("Foo").unwrap();
        assert_eq!(num.len(), 9);
        let num: u32 = num.parse().unwrap();
        assert_ne!(num, prev);
        prev = num;
        (nick, gen) = gen.unwrap().next_nick();
    }
}

#[test]
pub fn suffix_seq() {
    let prefix = Nick::from_bytes("Foo").unwrap();
    let gen = Suffix {
        suffixes: vec![
            SuffixType::Char('_'),
            SuffixType::Char('_'),
            SuffixType::NonZeroBase8,
            SuffixType::Base8,
        ]
        .into(),
        strategy: SuffixStrategy::Seq,
    };
    let (mut nick, mut gen) = gen.transform(prefix).next_nick();
    assert_eq!(nick, "Foo_");
    (nick, gen) = gen.unwrap().next_nick();
    assert_eq!(nick, "Foo__");
    (nick, gen) = gen.unwrap().next_nick();
    assert_eq!(nick, "Foo__1");
    for _ in 1..7 {
        (nick, gen) = gen.unwrap().next_nick();
    }
    assert_eq!(nick, "Foo__7");
    (nick, gen) = gen.unwrap().next_nick();
    assert_eq!(nick, "Foo__10");
    (nick, _) = gen.unwrap().next_nick();
    assert_eq!(nick, "Foo__20");
}

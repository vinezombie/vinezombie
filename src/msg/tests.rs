use super::{known::*, RawServerMsg};

#[test]
pub fn parse_cmd() {
    let msg = RawServerMsg::parse("privMSG").unwrap();
    assert_eq!(msg.kind.as_known_into(), Some(Cmd::Privmsg));
}

#[test]
pub fn parse_source_word() {
    let msg = RawServerMsg::parse(":server PING 123").unwrap();
    assert_eq!(msg.source.clone().unwrap().as_ref(), "server");
    assert_eq!(msg.kind.into_word(), "PING");
    assert_eq!(msg.data.args.split_last(), ([].as_slice(), Some(&"123".into())));
}

#[test]
pub fn parse_word() {
    let msg = RawServerMsg::parse("PONG 123").unwrap();
    assert_eq!(msg.data.args.split_last(), ([].as_slice(), Some(&"123".into())));
}

#[test]
pub fn parse_words() {
    let msg = RawServerMsg::parse("NOTICE #foo :beep").unwrap();
    assert_eq!(msg.data.args.words(), &["#foo", "beep"]);
}

#[test]
pub fn parse_words_long() {
    let msg = RawServerMsg::parse("PRIVMSG #foo #bar :Hello world").unwrap();
    let (chans, last) = msg.data.args.split_last();
    let last = last.unwrap();
    assert_eq!(chans, &["#foo", "#bar"]);
    assert_eq!(last.as_ref(), "Hello world");
}

#[test]
pub fn parse_tag() {
    let msg = RawServerMsg::parse("@tag TAGMSG").unwrap();
    assert_eq!(msg.source, None);
    assert_eq!(msg.kind.into_word(), "TAGMSG");
}

#[test]
pub fn to_string() {
    let cases = ["CMD", "CMD word :some words", ":src CMD word", ":numeric 001"];
    for case in cases {
        let looped = RawServerMsg::parse(case).unwrap().to_string();
        assert_eq!(looped, case);
    }
}

#[test]
pub fn len_bytes() {
    let cases = [
        "CMD\r\n",
        "CMD word\r\n",
        "CMD word1 word2\r\n",
        "CMD word :some words\r\n",
        ":src CMD word\r\n",
        "CMD uniçödé\r\n",
        "001\r\n",
    ];
    for case in cases {
        let testlen = 512 - RawServerMsg::parse(case).unwrap().bytes_left();
        let caselen = case.as_bytes().len() as isize;
        assert_eq!(testlen, caselen, "wrong length calculation for: {}", case);
    }
}

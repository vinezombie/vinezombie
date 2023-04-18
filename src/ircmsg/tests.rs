use super::IrcMsg;
use crate::string::Line;

macro_rules! irc_msg {
    ($lit:expr) => {
        IrcMsg::parse(Line::from_bytes($lit).unwrap()).unwrap()
    };
}

#[test]
pub fn parse_cmd() {
    assert_eq!(irc_msg!("privMSG").kind, b"PRIVMSG");
    assert_eq!(irc_msg!("  NOTICE").kind, b"NOTICE");
}

#[test]
pub fn parse_source_nickonly() {
    let msg = irc_msg!(":server PING");
    assert_eq!(msg.kind, b"PING");
    let source = msg.source.unwrap();
    assert_eq!(source.to_string(), "server");
    assert_eq!(source.nick, "server");
    assert_eq!(source.address, None);
}

#[test]
pub fn parse_source_full() {
    let msg = irc_msg!(":nick!user@host QUIT");
    assert_eq!(msg.kind, b"QUIT");
    let source = msg.source.unwrap();
    assert_eq!(source.to_string(), "nick!user@host");
    assert_eq!(source.nick, "nick");
    let address = source.address.unwrap();
    assert_eq!(address.user.unwrap(), "user");
    assert_eq!(address.host, "host");
}

#[test]
pub fn parse_arg() {
    let msg = irc_msg!("PONG 123");
    let (leading_args, last_arg) = msg.args.split_last();
    assert!(leading_args.is_empty());
    assert_eq!(last_arg.unwrap(), "123");
}

#[test]
pub fn parse_args() {
    let msg = irc_msg!("NOTICE #foo :beep");
    assert_eq!(msg.args.args(), ["#foo", "beep"]);
}

#[test]
pub fn parse_args_long() {
    let msg = irc_msg!("PRIVMSG #foo #bar :Hello world");
    let (chans, last) = msg.args.split_last();
    let last = last.unwrap();
    assert_eq!(chans, ["#foo", "#bar"]);
    assert_eq!(last, "Hello world");
}

#[test]
pub fn parse_tag_any() {
    let msg = irc_msg!("@tag TAGMSG");
    assert_eq!(msg.source, None);
    assert_eq!(msg.kind.as_ref(), b"TAGMSG");
}

#[test]
pub fn to_string() {
    let cases = [
        "CMD",
        "CMD word :some words",
        ":src CMD word",
        ":numeric 001",
        ":nick!user@host CMD",
        ":nick@host CMD",
    ];
    for case in cases {
        let looped = irc_msg!(case).to_string();
        assert_eq!(looped, case);
    }
}

#[test]
pub fn bytes_left() {
    let cases = [
        "CMD",
        "CMD word",
        "CMD word1 word2",
        "CMD word :some words",
        ":src CMD word",
        "CMD uniçödé",
        "001",
    ];
    for case in cases {
        let testlen = 510 - irc_msg!(case).bytes_left();
        let caselen = case.as_bytes().len() as isize;
        assert_eq!(testlen, caselen, "wrong length calculation for: {}", case);
    }
}

use super::{MaybeCtcp, ServerMsg};
use crate::string::Line;

macro_rules! irc_msg {
    ($lit:expr) => {
        ServerMsg::parse(Line::from_bytes($lit).unwrap()).unwrap()
    };
}

#[test]
pub fn parse_cmd() {
    assert_eq!(irc_msg!("privMSG").kind, "PRIVMSG");
    assert_eq!(irc_msg!("  NOTICE").kind, "NOTICE");
}

#[test]
pub fn parse_source_nickonly() {
    let msg = irc_msg!(":server PING");
    assert_eq!(msg.kind, "PING");
    let source = msg.source.unwrap();
    assert_eq!(source.to_string(), "server");
    assert_eq!(source.nick, "server");
    assert_eq!(source.userhost, None);
}

#[test]
pub fn parse_source_full() {
    let msg = irc_msg!(":nick!user@host QUIT");
    assert_eq!(msg.kind, "QUIT");
    let source = msg.source.unwrap();
    assert_eq!(source.to_string(), "nick!user@host");
    assert_eq!(source.nick, "nick");
    assert_eq!(source.user().unwrap(), "user");
    assert_eq!(source.host().unwrap(), "host");
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
    assert_eq!(msg.args.words(), ["#foo", "beep"]);
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
    assert_eq!(msg.kind, "TAGMSG");
}

#[test]
pub fn parse_tag_keys() {
    let tags = irc_msg!("@foo TAGMSG").tags;
    assert_eq!(tags.get("foo").unwrap(), "");
    let tags = irc_msg!("@foo;bar TAGMSG").tags;
    assert!(tags.get("foo").is_some());
    assert!(tags.get("bar").is_some());
    let tags = irc_msg!("@foo;bar; TAGMSG").tags;
    assert!(tags.get("foo").is_some());
    assert!(tags.get("bar").is_some());
    assert_eq!(tags.len(), 2);
    let tags = irc_msg!("@foo;;bar TAGMSG").tags;
    assert!(tags.get("foo").is_some());
    assert!(tags.get("bar").is_some());
    assert_eq!(tags.len(), 2);
}

#[test]
pub fn parse_tag_keyvalues() {
    let tags = irc_msg!("@foo=foov TAGMSG").tags;
    assert_eq!(tags.get("foo").unwrap(), "foov");
    let tags = irc_msg!("@foo=foov;bar=barv TAGMSG").tags;
    assert_eq!(tags.get("foo").unwrap(), "foov");
    assert_eq!(tags.get("bar").unwrap(), "barv");
    let tags = irc_msg!("@foo= TAGMSG").tags;
    assert_eq!(tags.get("foo").unwrap(), "");
    let tags = irc_msg!("@foo=; TAGMSG").tags;
    assert_eq!(tags.get("foo").unwrap(), "");
    let tags = irc_msg!("@foo=bar;foo=baz TAGMSG").tags;
    assert_eq!(tags.get("foo").unwrap(), "baz");
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
    // Vinezombie assumes the final argument of messages
    // will always be relayed with a colon.
    // The testcases here reflect this.
    let cases = [
        "CMD",
        "CMD :word",
        "CMD word1 :word2",
        "CMD word :some words",
        ":src CMD :word",
        "CMD :uniçödé",
        "001",
    ];
    for case in cases {
        let testlen = 510 - irc_msg!(case).bytes_left();
        let caselen = case.as_bytes().len() as isize;
        assert_eq!(testlen, caselen, "wrong length calculation for: {}", case);
    }
}

#[test]
pub fn ctcp() {
    let cases = [
        ("\x01ACTION tap-dances\x01", "ACTION"),
        ("\x01ACTION beeps.", "ACTION"),
        ("ACTION beeps.\x01", ""),
        ("\x01FOO\x01BAR baz", "FOO"),
    ];
    for (case, expected) in cases {
        let line = Line::from_str(case);
        let ctcp = MaybeCtcp::from(line);
        assert_eq!(ctcp.cmd, expected);
    }
}

#[cfg(feature = "tokio-codec")]
mod tokio_codec {
    #[test]
    fn split() {
        // There's really only one piece of functionality in the tokio codec vs the above,
        // and that's the line splitter. Test only that.
        let cases = [
            ("foo bar\r\n", true),
            ("foo bar", false),
            ("   foo bar\r\n", true),
            ("\n\n\n\nfoo bar\r\n", true),
            ("foo bar\r", false),
            ("     ", false),
        ];
        let mut case_id = 0;
        for (case, expected) in cases {
            let mut buf = tokio_util::bytes::BytesMut::from(case);
            let idx = crate::ircmsg::codec::tokio_codec::scroll_buf(&mut buf, 16);
            case_id += 1;
            assert_eq!(idx.is_some(), expected, "newline disagreement on case {case_id}");
            let Some(idx) = idx else {
                continue;
            };
            let line_raw = buf.split_to(idx.get());
            assert_eq!(line_raw.as_ref(), b"foo bar\r\n", "partial split on case {case_id}");
        }
    }
}

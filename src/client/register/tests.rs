use std::{io::Cursor, time::Duration};

use crate::{
    client::{auth::Clear, channel::SyncChannels, conn::Bidir, new_client},
    string::{Key, Nick},
};

use super::{register_as_bot, HandlerError, Options, Registration};

/// Test registration while ignoring the messages the handler sends.
fn static_register(msg: &[u8]) -> Result<Registration, HandlerError> {
    let mut options: Options<Clear> = Options::new();
    options.nicks = vec![Nick::from_str("Me")];
    let reg = register_as_bot(); // Somewhat more deterministic.
    let io = Bidir::<Cursor<Vec<u8>>, _>(Cursor::new(msg.to_vec()), std::io::sink());
    let mut client = new_client(io);
    client.queue_mut().set_rate_limit(Duration::ZERO, 1);
    let (_, reg) = client.add(&SyncChannels, &reg, &options).unwrap();
    client.run().unwrap();
    reg.0.recv_nonblocking().unwrap()
}

#[test]
fn ircv2_reg() {
    // We should be able to handle any values for messages 001 through 003,
    // so we're just going to put silliness here.
    static_register(
        concat!(
            ":example.com NOTICE Me :senpai!\r\n",
            ":foo!bar@baz 001 Me :Hi, we're glad to have you.\r\n",
            ":foo!bar@baz 002 Me :I'm your host. You can get foo at the bar.\r\n",
            ":foo!bar@baz 003 Me :Someone wrote me in 2024.\r\n",
            // Omit the last param for testing reasons.
            ":foo!bar@baz 004 Me ircv2_reg.fn the-latest-one iw bnt\r\n",
            ":foo!bar@baz 422 Me :Nobody reads MOTDs anyway these days.\r\n",
        )
        .as_bytes(),
    )
    .expect("ircv2 reg failed");
}

#[test]
fn ircv3_reg_simple() {
    use crate::names::{cap::LABELED_RESPONSE, isupport::NETWORK};
    // TODO: Test more thoroughly.
    // We should be able to handle any values for messages 001 through 003,
    // so we're just going to put silliness here.
    let reg = static_register(
        concat!(
            ":example.com CAP * LS :quickbrownfox/lazydogjumping labeled-response\r\n",
            ":example.com CAP * ACK :labeled-response\r\n",
            ":example.com 001 Me :Hi, we're glad to have you.\r\n",
            ":example.com 002 Me :I'm your host. You can get foo at the bar.\r\n",
            ":example.com 003 Me :Someone wrote me in 2024.\r\n",
            // Omit the last param for testing reasons.
            ":example.com 004 Me ircv2_reg.fn the-latest-one iw bnt\r\n",
            ":example.com 005 Me NETWORK=example.com FOXSAID=WHAT :are allegedly supported.\r\n",
            ":example.com NOTICE Me :senpai!\r\n",
            ":example.com 422 Me :Nobody reads MOTDs anyway these days.\r\n",
        )
        .as_bytes(),
    )
    .expect("ircv3 reg failed");
    assert_eq!(reg.caps.get_extra(LABELED_RESPONSE).copied(), Some(true));
    assert_eq!(
        reg.caps.get_extra_raw(&Key::from_str("quickbrownfox/lazydogjumping")).copied(),
        Some(false)
    );
    let netname = reg.isupport.get_parsed(NETWORK).expect("NETWORK should have a value").unwrap();
    assert_eq!(netname, b"example.com");
}

#[test]
fn bounce() {
    let testcases = [
        b":foo!bar@baz 005 :Try server example.com, port 6667\r\n".as_slice(),
        b":foo!bar@baz 005 * :Try server example.com, port 6667\r\n",
        b":foo!bar@baz 005 Me :Try server example.com, port 6667\r\n",
        b":foo!bar@baz 010 Me example.com 6667 :We now live in a yellow submarine\r\n",
    ];
    for testcase in testcases {
        match static_register(testcase) {
            Err(HandlerError::Redirect(serv, port, _)) => {
                assert_eq!(serv.to_utf8_lossy(), "example.com");
                assert_eq!(port, 6667);
            }
            Err(e) => panic!("wrong error: {e}"),
            Ok(_) => panic!("connection registration somehow succeeded"),
        }
    }
}

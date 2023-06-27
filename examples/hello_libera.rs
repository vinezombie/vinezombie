use std::collections::BTreeSet;
use vinezombie::{
    client::{self, auth::Clear},
    ircmsg::ClientMsg,
    string::Line,
};

fn main() -> std::io::Result<()> {
    // Let's get some logging.
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).compact().init();
    // Connection registration options.
    // `Clear` is a Secret implementation that offers minimal added protection
    // for sensitive data.
    // It's a safe choice if the user account running your application can
    // reasonably be assumed to not be compromised.
    let mut options = client::register::new::<Clear>();
    options.realname = Some(Line::from_str("Vinezombie Example: hello_libera"));
    // Rate-limited queue. Used to avoid excess-flooding oneself off the server.
    let mut queue = client::Queue::new();
    // Build the configuration for a TLS connection and actually connect.
    let tls_config = client::tls::TlsConfig::default().build()?;
    let sock = client::tls::connect(tls_config, "irc.libera.chat", 6697)?;
    let mut sock = std::io::BufReader::new(sock);
    // The initial connection registration handshake needs to happen,
    // so let's build a handler for that.
    // The provided set is a set of capabilities to request.
    // We don't need anything, so this set is empty.
    // `BotDefaults` provides default values for anything we didn't specify
    // in `options` above.
    // Passing the `queue` populates it with the initial message burst.
    let mut handler =
        options.handler(BTreeSet::new(), &client::register::BotDefaults, &mut queue)?;
    // Let's do connection registration!
    let reg = vinezombie::client::run_handler(&mut sock, &mut queue, &mut handler)?;
    // Connection registration is done!
    tracing::info!("{} connected to Libera!", reg.nick);
    // From here, we can keep reading messages (including 004 and 005)
    // but we don't care about any of that, so let's just quit.
    let msg = ClientMsg::new_cmd(vinezombie::known::cmd::QUIT);
    msg.send_to(sock.get_mut(), &mut Vec::new())?;
    Ok(())
}

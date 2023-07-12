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
    // TLS can be pretty complicated, but there are sensible defaults.
    // Use those defaults to build a TLS client configuration that we can use later.
    let tls_config = client::tls::TlsConfig::default().build()?;
    // Rate-limited queue. Used to avoid excess-flooding oneself off the server,
    // even though that shouldn't be a risk for this minimal example.
    let mut queue = client::Queue::new();
    // We're connecting to Libera.Chat for this example, so let's do it.
    // To disable TLS, we can set `address.tls`.
    // To change the port number to something non-default, we can set `address.port`.
    let address = client::conn::ServerAddr::from_host_str("irc.libera.chat");
    let mut sock = address.connect(tls_config)?;
    // The initial connection registration handshake needs to happen,
    // so let's build a handler for that.
    // `BotDefaults` provides default values for anything we didn't specify
    // in `options` above.
    // Passing the `queue` populates it with the initial message burst.
    let mut handler = options.handler(&client::register::BotDefaults, &mut queue)?;
    // Let's do connection registration!
    let reg = vinezombie::client::run_handler(&mut sock, &mut queue, &mut handler)?;
    // Connection registration is done!
    tracing::info!("{} connected to Libera!", reg.nick);
    // From here, we can keep reading messages (including 004 and 005)
    // but we don't care about any of that, so let's just quit.
    // send_to takes a Vec for buffering writes.
    let msg = ClientMsg::new_cmd(vinezombie::consts::cmd::QUIT);
    msg.send_to(sock.get_mut(), &mut Vec::new())?;
    Ok(())
}

use vinezombie::{
    client::{
        self,
        auth::Clear,
        channel::SyncChannels,
        register::{register_as_bot, Options},
        Client,
    },
    ircmsg::ClientMsg,
    string::{Line, Word},
};

fn main() -> std::io::Result<()> {
    // Let's get some logging.
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).compact().init();
    // Connection registration options.
    // `Clear` is a Secret implementation that offers minimal added protection for sensitive data.
    // It's a safe choice if the user account running your application can
    // reasonably be assumed to not be compromised.
    let mut options: Options<Clear> = Options::new();
    options.realname = Some(Line::from_str("Vinezombie Example: hello_libera"));
    // We're connecting to Libera.Chat for this example, so let's do it.
    // To disable TLS, we can set `address.tls`.
    // To change the port number to something non-default, we can set `address.port`.
    let address = client::conn::ServerAddr::from_host_str("irc.libera.chat");
    let sock = address.connect(|| {
        // TLS can be pretty complicated, but there are sensible defaults.
        // Use those defaults to build a TLS client configuration.
        client::tls::TlsConfigOptions::default().build()
        // If we may need to reconnect, the client configuration should be stored
        // outside this function, possibly using a `OnceCell` or `OnceLock`.
    })?;
    // `Client` bundles the connection and serves as a host for Handlers
    // that process IRC messages. It also rate-limits outgoing messages to avoid
    // disconnections for flooding, and can adjust the message queue based in incoming messages.
    // For convenience, we use SyncChannels as a way to build an appropriate channel type for
    // the registration handler.
    let mut client = Client::new(sock, SyncChannels);
    // We're not ready to go just yet.
    // The initial connection registration handshake needs to happen.
    // Handlers return values through channels, one channel per handler.
    // Handlers are allowed to fail on construction, but most do not.
    let (_id, reg_result) = client.add(&register_as_bot(), &options).unwrap();
    // Let's actually run the handler now!
    // Normally `run` returns the ids of handlers that have yielded values and finished,
    // but we're only running one handler that always yields one value on completion,
    // so we can ignore it.
    client.run()?;
    // If we're here, the handler finished and gave us a value back.
    // The registration handler only tells us what the error was, if it failed.
    // So, let's check to see if we made it.
    reg_result.0.recv_now().unwrap()?;
    // Connection registration is done!
    // Now, what is our nick? `Client` can store information about the current session
    // for use by other handlers. We can access that information through the `state` method.
    // While we're at it, let's get the network name from ISUPPORT.
    let nick = &client.state().get::<vinezombie::client::state::ClientSource>().unwrap().nick;
    let isupport = client.state().get::<vinezombie::client::state::ISupport>().unwrap();
    let network_name = isupport.get_parsed(vinezombie::names::isupport::NETWORK).transpose()?;
    tracing::info!("{} connected to {}!", nick, network_name.unwrap_or(Word::from_str("IRC")));
    // From here, we can keep reading messages (including 004 and 005)
    // but we don't care about any of that, so let's just quit.
    // We create the message, push it onto the internal message queue,
    // and then fully flush the queue.
    let msg = ClientMsg::new(vinezombie::names::cmd::QUIT);
    client.queue_mut().edit().push(msg);
    client.run()?;
    Ok(())
}

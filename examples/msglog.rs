use vinezombie::{
    client::{
        self,
        auth::Clear,
        channel::TokioChannels,
        conn::ServerAddr,
        handlers::{AutoPong, YieldParsed},
        new_client,
        register::{register_as_bot, Options},
    },
    ircmsg::ClientMsg,
    names::cmd::{JOIN, PRIVMSG},
    string::{Arg, Bytes, Line},
};

// Let's make a simple logging bot,
// something that just dumps PRIVMSGs to standard output for muliple channels.
// Going forward, all examples are going to use tokio,
// because the sync I/O provided by std is suffering.

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Let's be less verbose this time.
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).compact().init();
    // Standard vinezombie boilerplate.
    let mut options: Options<Clear> = Options::new();
    options.realname = Some(Line::from_str("Vinezombie Example: msglog"));
    let address = ServerAddr::from_host_str("irc.libera.chat");
    let sock = address.connect_tokio(|| client::tls::TlsConfigOptions::default().build()).await?;
    let mut client = new_client(sock);
    let (_id, reg_result) = client.add(&TokioChannels, &register_as_bot(), &options)?;
    client.run_tokio().await?;
    // The only piece of reg info we care about for this example is our nick.
    let nick = reg_result.await.unwrap()?.nick;
    tracing::info!("nick: {}", nick);
    // Let's add a handler to auto-reply to PING messages for us. Most IRC networks need this.
    // Do this before anything else.
    let _ = client.add(&TokioChannels, &(), AutoPong);
    // Let's join all the channels provided as command-line arguments.
    // This is a VERY crude way of joining multiple channels, but works for illustration
    // of how to construct messages that are less-trivial than a no-argument QUIT message.
    // Let's start by getting an edit guard on the queue so that we can push messages to it.
    let mut queue = client.queue_mut().edit();
    // vinezombie's Bytes type is not like the Bytes abstraction from the bytes crate.
    // It is byte string type that lazily checks UTF-8 validity and can
    // optionally share ownership of its contents. It sees use throughout the codebase.
    for channel in std::env::args().skip(1).map(Bytes::from) {
        let mut msg = ClientMsg::new(JOIN);
        // Safeguard: vinezombie ensures that you cannot easily create nonsensical messages.
        // The Arg::from_bytes function checks that the string is a valid IRC message argument.
        let Ok(channel) = Arg::from_bytes(channel.clone()) else {
            tracing::warn!("skipping invalid channel {channel}");
            continue;
        };
        // Absolutely nothing fancy here. No multi-target joins, just one channel per join.
        msg.args.edit().add_word(channel);
        queue.push(msg);
    }
    // We now need to receive PRIVMSGs and send them somewhere for further processing.
    // `YieldParsed` exists exactly for this purpose.
    let (_, mut msgs) = client.add(&TokioChannels, &(), YieldParsed::just(PRIVMSG)).unwrap();
    // Since we are async, let's do the actual printing in another task, because we can.
    tokio::spawn(async move {
        while let Some(msg) = msgs.recv().await {
            // If the server didn't give us a source, something's weird. Skip.
            let Some(source) = msg.source else {
                continue;
            };
            // If the message is being sent to our nick, use their nick as the context.
            // An arguably better way of doing this is looking for a channel prefix in the target.
            let context = if msg.target.as_bytes() == nick.as_bytes() {
                source.nick.clone().into()
            } else {
                msg.target
            };
            println!("{} <{}> {}", context, source.nick, msg.value);
        }
    });
    // Drive the client for ever and ever and ever and ever and ever and ever and ever and ever and-
    loop {
        client.run_tokio().await?;
    }
}

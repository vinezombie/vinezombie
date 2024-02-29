use vinezombie::{
    client::{self, auth::Clear, channel::TokioChannels, new_client, register::BotDefaults},
    ircmsg::ClientMsg,
    string::Line,
};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Most of this matches the hello_libera example, so let's fast forward a bit.
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).compact().init();
    let mut options = client::register::new::<Clear>();
    options.realname = Some(Line::from_str("Vinezombie Example: hello_libera_tokio"));
    let address = client::conn::ServerAddr::from_host_str("irc.libera.chat");
    // First difference! We use a different function here to connect asynchronously.
    // Many of the synchronous functions have `_tokio` variants for Tokio-flavored async.
    let sock = address.connect_tokio(|| client::tls::TlsConfigOptions::default().build()).await?;
    let mut client = new_client(sock);
    // We still use the same handler for connection registration,
    // but instead we run it using a run_tokio function.
    // We also use TokioChannels instead of SyncChannels, which changes the flavor
    // of channel used by our handler.
    let (_id, reg_result) = client.add(&TokioChannels, &options, &BotDefaults)?;
    client.run_tokio().await?;
    let reg = reg_result.await.unwrap()?;
    tracing::info!("{} connected to Libera!", reg.nick);
    // As with the earlier example, let's just quit here.
    let msg = ClientMsg::new_cmd(vinezombie::consts::cmd::QUIT);
    client.queue_mut().edit().push(msg);
    client.run_tokio().await?;
    Ok(())
}

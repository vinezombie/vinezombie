use vinezombie::{
    client::{
        self,
        auth::Clear,
        channel::TokioChannels,
        register::{register_as_bot, Options},
        Client,
    },
    ircmsg::ClientMsg,
    string::{Line, Word},
};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Most of this matches the hello_libera example, so let's fast forward a bit.
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).compact().init();
    let mut options: Options<Clear> = Options::new();
    options.realname = Some(Line::from_str("Vinezombie Example: hello_libera_tokio"));
    let address = client::conn::ServerAddr::from_host_str("irc.libera.chat");
    // First difference! We use a different function here to connect asynchronously.
    // Many of the synchronous functions have `_tokio` variants for Tokio-flavored async.
    let sock = address.connect_tokio(|| client::tls::TlsConfigOptions::default().build()).await?;
    // We also use TokioChannels instead of SyncChannels, which changes the flavor
    // of channel used by our handler later on.
    let mut client = Client::new(sock, TokioChannels);
    // We still use the same handler for connection registration,
    // but instead we run it using a run_tokio function.
    let (_id, reg_result) = client.add(&register_as_bot(), &options).unwrap();
    client.run_tokio().await?;
    reg_result.await.unwrap()?;
    // Almost everything past this point is the same as the sync example.
    let nick = &client.state().get::<vinezombie::client::state::ClientSource>().unwrap().nick;
    let isupport = client.state().get::<vinezombie::client::state::ISupport>().unwrap();
    let network_name = isupport.get_parsed(vinezombie::names::isupport::NETWORK).transpose()?;
    tracing::info!("{} connected to {}!", nick, network_name.unwrap_or(Word::from_str("IRC")));
    let msg = ClientMsg::new(vinezombie::names::cmd::QUIT);
    client.queue_mut().edit().push(msg);
    client.run_tokio().await?;
    Ok(())
}

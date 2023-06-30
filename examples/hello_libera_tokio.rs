use std::collections::BTreeSet;
use vinezombie::{
    client::{self, auth::Clear},
    ircmsg::ClientMsg,
    string::Line,
};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Most of this matches the hello_libera example, so let's fast forward a bit.
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).compact().init();
    let mut options = client::register::new::<Clear>();
    options.realname = Some(Line::from_str("Vinezombie Example: hello_libera_tokio"));
    let tls_config = client::tls::TlsConfig::default().build()?;
    let mut queue = client::Queue::new();
    let address = client::conn::ServerAddr::from_host_str("irc.libera.chat");
    // First difference! We use a different function here to connect asynchronously.
    // Many of the synchronous functions have `_tokio` variants for
    // Tokio-flavored async. Whenever the standard library gets better async support,
    // there will also be `_async` variants.
    let mut sock = address.connect_tokio(tls_config).await?;
    // We still use the same handler for connection registration,
    // but instead we run it using a run_handler_tokio function.
    // This function is actually more general than run_handler,
    // but we're not going to make use of its functionality in this example.
    let mut handler =
        options.handler(BTreeSet::new(), &client::register::BotDefaults, &mut queue)?;
    let reg = vinezombie::client::run_handler_tokio(&mut sock, &mut queue, &mut handler).await?;
    tracing::info!("{} connected to Libera!", reg.nick);
    // As with the earlier example, let's just quit here.
    // Keeping with the earlier pattern, there is a `tokio` variant of `send_to` as well.
    let msg = ClientMsg::new_cmd(vinezombie::consts::cmd::QUIT);
    msg.send_to_tokio(sock.get_mut(), &mut Vec::new()).await?;
    Ok(())
}

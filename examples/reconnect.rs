use std::io::BufReader;
use vinezombie::{
    client::{
        self,
        auth::Clear,
        channel::SyncChannels,
        conn::{ServerAddr, Stream},
        handlers::{AutoPong, YieldParsed},
        register::{register_as_bot, Options},
        tls::TlsConfig,
        Client,
    },
    names::cmd::PRIVMSG,
    string::Line,
};

// Any reliable IRC software needs a way to automatically reconnect.
//
// For this example, we're going to use sync I/O, since we need to do stuff in the loop that
// drives the client anyway, so we might as well explore how to use a system we've been ignoring
// this whole time in a context where it's not completely irrelevant.
//
// WARNING: This example does NOT implement progressively less-frequent reconnections.
// This is strongly recommended to do robust usecase.

fn make_sock(
    tls_config: &mut Option<TlsConfig>,
    address: &ServerAddr<'static>,
) -> std::io::Result<BufReader<Stream>> {
    address.connect(|| {
        if let Some(v) = tls_config.as_ref() {
            return Ok(v.clone());
        };
        let config = client::tls::TlsConfigOptions::default().build()?;
        *tls_config = Some(config.clone());
        Ok(config)
    })
}

fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).compact().init();
    // Standard vinezombie boilerplate.
    let mut options: Options<Clear> = Options::new();
    options.realname = Some(Line::from_str("Vinezombie Example: reconnect"));
    let address = ServerAddr::from_host_str("irc.libera.chat");
    let mut tls_config: Option<TlsConfig> = None;
    let mut client = Client::new(make_sock(&mut tls_config, &address)?, SyncChannels);
    loop {
        let (_, reg_result) = client.add(&register_as_bot(), &options)?;
        client.run()?;
        let nick = reg_result.0.recv_now().unwrap()?.nick;
        let _ = client.add((), AutoPong);
        // For the purposes of this example, let's quit and reconnect
        // if literally anyone sends us a message containing the letter "q".
        // In previous examples, we've been ignoring that the client's `run` methods
        // actually disclose which handlers produced values and/or finished.
        // This time we're actually going to use that information,
        // starting by saving the id of the message handler.
        let (id, msgs) = client.add((), YieldParsed::just(PRIVMSG)).unwrap();
        tracing::info!("bot {nick} ready for 'q'~");
        loop {
            let Ok(result) = client.run() else {
                tracing::info!("connection broke, making new connection");
                break;
            };
            // Check if the list of handlers that yielded something contains our id.
            if !result.unwrap().0.contains(&id) {
                continue;
            }
            let msg = msgs.try_recv().unwrap();
            if !msg.value.contains(&b'q') {
                tracing::info!("received \"{}\"", msg.value);
                continue;
            }
            tracing::info!("got 'q', making new connection");
            break;
        }
        client.reset_with_conn(make_sock(&mut tls_config, &address)?);
    }
}

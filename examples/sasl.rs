use std::io::Write;
use vinezombie::{
    client::{
        self,
        auth::{sasl::Password, Clear, Secret},
        channel::TokioChannels,
        register::{register_as_bot, Options},
        state::Account,
        Client,
    },
    string::{tf::TrimAscii, Line, NoNul},
};

// This is a simple example of how to do password authentication using vinezombie.
// Some of what this example does is strongly discouraged in production;
// the code only cares about getting strings from standard input into a `Password` authenticator.

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).compact().init();
    let mut options: Options<Clear> = Options::new();
    let mut username_string = String::new();
    let mut password_string = String::new();
    // Normally using this I/O flavor in async is a mistake.
    // It is used here for demonstration purposes only.
    print!("Username: ");
    std::io::stdout().flush()?;
    std::io::stdin().read_line(&mut username_string)?;
    // WARNING: For production purposes, do not read passwords from a terminal this way.
    // Use something like the rpassword crate instead.
    print!("Password: ");
    std::io::stdout().flush()?;
    std::io::stdin().read_line(&mut password_string)?;
    // vinezombie enforces certain invariants over string types.
    // For usernames and passwords, it requires that the strings have no \0 bytes.
    // While we're at it, let's trim off trailing whitespace left by read_line.
    let mut username: NoNul<'_> = username_string.try_into()?;
    let mut password: NoNul<'_> = password_string.try_into()?;
    username.transform(TrimAscii::default());
    password.transform(TrimAscii::default());
    options.add_sasl(Password::new(username, Secret::new(password)));
    // options.allow_sasl_fail = true;
    // Time for connection registration.
    options.realname = Some(Line::from_str("Vinezombie Example: sasl"));
    let address = client::conn::ServerAddr::from_host_str("irc.libera.chat");
    let sock = address.connect_tokio(|| client::tls::TlsConfigOptions::default().build()).await?;
    let mut client = Client::new(sock, TokioChannels);
    let (_id, reg_result) = client.add(&register_as_bot(), &options).unwrap();
    client.run_tokio().await?;
    reg_result.await.unwrap()?;
    // Who'd we log in as?
    if let Some(account) = client.state().get::<Account>().unwrap() {
        tracing::info!("Logged in as {account}");
    } else {
        // We should never get here unless `options.allow_sasl_fail` is set to `true`.
        tracing::warn!("Did not log in");
    }
    Ok(())
}

# [Vinezombie](https://github.com/vinezombie/vinezombie)

Vinezombie is a library for writing IRC software.
For a basic overview, see its README
on [Github](https://github.com/vinezombie/vinezombie#readme)
or [sr.ht](https://git.sr.ht/~daemoness/vinezombie).

Vinezombie is very modular.
The flexibility it offers comes at the expense of a steeper learning curve.
The examples may be useful in learning how to use this library.

## Optional Features

Vinezombie has no mandatory dependencies besides `std`.
The default feature set is chosen to be enough to write IRC bots,
and includes the following:

* `base64`: Required for SASL.
Adds base64 encoding/decoding.
* `client`:
Adds utilities for building client-side IRC software.
* `tls`:
Adds utilities for working with rustls.
* `tokio`:
Adds functions for Tokio-based I/O.

The following optional features are also available:

* `serde`:
Adds implementations of `Serialize`+`Deserialize` for certain types.
* `tracing`:
Adds logging to a few locations in the library.
If your application uses `log`,
[this](https://docs.rs/tracing/0.1/tracing/#emitting-log-records)
explains how to get `log` events from this library.
* `whoami`:
Enables functions for creating strings from local user info.
* `zeroize`:
Zeroes-out certain byte buffers containing potentially-sensitive data.

## The String Types

The core primitive of vinezombie is [`string::Bytes`][crate::string::Bytes]
(not to be confused with `bytes::Bytes`).
This is a borrowing-or-shared-owning immutable string type with
lazy UTF-8 validity checking and a notion of secret values.
It is thread-safe and cheap to construct out of a `Vec` or `String`.

Atop `Bytes` is a hierarchy of newtypes,
such as [`Line`][crate::string::Line] and [`Arg`][crate::string::Arg].
These enforce invariants on the string that can be checked in
`const` contexts, helping to prevent certain classes of logic errors and
the construction of invalid messages.

## Protocol Features

Vinezombie includes parsers for IRCv3 messages and their components
which can be found in [`ircmsg`][crate::ircmsg].
Vinezombie distinguishes between
[client-originated messages][crate::ircmsg::ClientMsg]
and [server-originated messages][crate::ircmsg::ServerMsg].

To make working within `Bytes` newtypes more-convenient,
vinezombie also includes named constants for common IRC message values in
[`consts`][crate::consts].

## Client Features

If you are writing client-side software,
the [`client`][crate::client] module includes an assortment of utilities
that may be useful while remaining close to the raw IRC protocol.

[`client::register`][crate::client::register] provides an implementation
of the full IRCv3 connection registration handshake, including SASL.

[`client::tls`][crate::client::tls] provides means of creating
rustls client configurations that are adequate for most usecases
and may be significantly more convenient than working with rustls directly.

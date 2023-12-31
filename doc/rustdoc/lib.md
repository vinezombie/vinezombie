# [vinezombie](https://github.com/vinezombie/vinezombie)

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
* `tls-tokio`: Implies `tls` and `tokio`.
Adds support for asynchronous TLS connections.
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

## Features

Vinezombie includes parsers for IRCv3 messages and their components
which can be found in [`ircmsg`][crate::ircmsg].

If you are writing client-side software,
the [`client`][crate::client] module includes an assortment of utilities
that may be useful while remaining close to the raw IRC protocol,
including a rudimentary event-handling system.

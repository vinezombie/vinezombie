*vinezombie is slowly stabilizing. Please hold.*

# vinezombie

**An abstracting IRCv3 library in Rust.**

[![CI](https://github.com/TheDaemoness/vinezombie/actions/workflows/ci.yml/badge.svg)](https://github.com/TheDaemoness/vinezombie/actions/workflows/ci.yml)
[![Chat on libera.chat](https://img.shields.io/badge/libera.chat-%23vinezombie-blueviolet)](https://web.libera.chat/gamja/?channel=#vinezombie)

`vinezombie` is an opinionated Rust library for writing IRCv3 utilities,
namely IRC clients, bots, and plugins for said clients and bots.
It is designed to provide thin abstractions over the underlying protocol
while allowing the mapping between them be highly-configurable at runtime.
The goal is to allow client logic to be written as agnostically as reasonably
possible to the quirks of whatever server is being connected to.

## Optional Features

`vinezombie` aims to have feature gates and minimum mandatory dependencies
to allow you to use only what you need.
The default feature set is designed to be enough to write IRC bots,
and includes the following:

* `abstract`: Uses `ircmsg` and `state`.
Adds abstractions of the raw IRC protocol.
* `base64`: Required for SASL.
Adds base64 encoding/decoding.
* `client`: Uses `ircmsg`.
Adds utilities for building client-side IRC software.
* `ircmsg`:
Adds representations of IRC messages.
* `state`:
Adds types for representing network state.
* `tls`:
Adds utilities for working with rustls.

The following optional features are also available:

* `serde`:
Adds implementations of `Serialize`+`Deserialize` for certain types.
* `tokio`:
Adds functions for Tokio-based I/O.
* `tracing`:
Adds logging to a few locations in the library.
If your application uses `log`,
[this][https://docs.rs/tracing/0.1/tracing/#emitting-log-records]
explains how to get `log` events from this library.
* `whoami`:
Enables functions for creating strings from local user info.
* `zeroize`:
Zeroes-out certain byte buffers containing potentially-sensitive data.

## Documentation

To build and view the documentation locally, run
`RUSTDOCFLAGS="--cfg doc_unstable" cargo +nightly doc --all-features --open`

## License

`vinezombie` is licensed under the GNU GPL v3 (only).
Disclosing the source code of bots written using `vinezombie` to
end users over IRC is also strongly encouraged, but not required.

## Discussion

The author somtimes rambles about code design in
[#vinezombie](ircs://irc.libera.chat/#vinezombie)
on [Libera.Chat](https://libera.chat/).
A link to a webchat is available at the top by clicking the libera.chat badge.

---

```
<jess> why vinezombie lmao
<TheDaemoness> Because. Grapevines. Undead chat protocols.
<jess> oh my god
```

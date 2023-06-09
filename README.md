*vinezombie is slowly stabilizing. Please hold.*

# vinezombie

**An abstracting IRCv3 client library in Rust.**

[![builds.sr.ht status](https://builds.sr.ht/~daemoness/vinezombie/.svg)](https://builds.sr.ht/~daemoness/vinezombie/?)
[![Chat on libera.chat](https://img.shields.io/badge/libera.chat-%23vinezombie-blueviolet)](https://web.libera.chat/gamja/?channel=#vinezombie)

`vinezombie` is an opinionated Rust library for writing IRCv3 utilities,
namely IRC clients, bots, and plugins for said clients and bots.
It is designed to provide thin abstractions over the underlying protocol
while allowing the mapping between them be highly-configurable at runtime.
The goal is to allow client logic to be written as agnostically as reasonably
possible to the quirks of whatever server is being connected to.

## Optional Features

`vinezombie` aims to have minimal mandatory dependencies.
The default feature set is designed to be enough to write IRC bots,
and includes the following:

* `base64`: Adds base64 encoding/decoding. Required for SASL.
* `log`: Enables utilities for logging using `log`.

The following optional features are also available:

* `serde`: Adds implementations of `Serialize`+`Deserialize` for certain types.
* `whoami`: Uses system account info for `crate::init::Register::default()`.

## License

`vinezombie` is licensed under the GNU GPL v3 (only).
Disclosing the source code of bots written using `vinezombie` to
end users over IRC is also strongly encouraged, but not required.

## Discussion

The author somtimes rambles about code design in #vinezombie on
[Libera.Chat](ircs://irc.libera.chat/#vinezombie).
A link to a webchat is available at the top by clicking the libera.chat badge.

---

```
<jess> why vinezombie lmao
<TheDaemoness> Because. Grapevines. Undead chat protocols.
<jess> oh my god
```

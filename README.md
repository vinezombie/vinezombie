*vinezombie is a work in progress. Use with care.
It may have bugs, and there will be further breaking 0.x releases.
Expect many more features in the future.*

# vinezombie

**A modular IRCv3 library in Rust.**

[![CI](https://github.com/vinezombie/vinezombie/actions/workflows/ci.yml/badge.svg)](https://github.com/vinezombie/vinezombie/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/vinezombie.svg)](https://crates.io/crates/vinezombie)
[![API docs](https://docs.rs/vinezombie/badge.svg)](https://docs.rs/vinezombie)
[![Chat on libera.chat](https://img.shields.io/badge/libera.chat-%23vinezombie-rgb?logo=liberadotchat&color=%23ff55dd)](https://web.libera.chat/gamja#vinezombie)

**vinezombie** is a Rust library for writing IRCv3 software
(mostly clients/bots at this time).

## Features

- An emphasis on correctness;
without using `unsafe`, it should be impossible to construct
a correctly-sized message that, once written, does not parse into the same message.
- Zero-copy parsing of IRC messages.
- An implementation of IRCv3 connection registration, including SASL.
- Convenience utilities for creating asynchronous TLS connections.
- Minimal mandatory dependencies.
- Designed to be flexible and modular,
usable as either a library or a highly pluggable framework.

## Building Documentation and Examples

To build and view the documentation locally, run:
```sh
RUSTDOCFLAGS="--cfg doc_unstable" cargo +nightly doc --all-features --open`
```

The strings diagram in `doc` can be re-rendered using:
```sh
d2 -t 200 -l dagre --pad 0 doc/strings.d2 doc/strings.d2.svg
```

vinezombie's examples may use any combination of its features,
and should be built with `--all-features`.

## License

vinezombie is licensed under the GNU GPL v3 (only).
Unless otherwise specified, all contributions submitted by you for inclusion
will be licensed as the rest of the library.

Disclosing the source code of bots written using vinezombie to
end users over IRC is also strongly encouraged, but not required.

## Discussion

If you wish to discuss vinezombie's development in soft-realtime,
our official IRC channel is
[#vinezombie](ircs://irc.libera.chat/#vinezombie)
on [Libera.Chat](https://libera.chat/).
A link to a webchat is available at the top by clicking the libera.chat badge.

---

```
<jess> why vinezombie lmao
<TheDaemoness> Because. Grapevines. Undead chat protocols.
<jess> oh my god
```

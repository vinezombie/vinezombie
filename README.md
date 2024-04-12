*vinezombie is a work in progress. Use with care.
It may have bugs, and there will be further breaking 0.x releases.
Expect many more features in the future.*

# vinezombie

**A modular IRCv3 framework in Rust.**

[![CI](https://github.com/vinezombie/vinezombie/actions/workflows/ci.yml/badge.svg)](https://github.com/vinezombie/vinezombie/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/vinezombie.svg)](https://crates.io/crates/vinezombie)
[![API docs](https://docs.rs/vinezombie/badge.svg)](https://docs.rs/vinezombie)
[![Chat on libera.chat](https://img.shields.io/badge/libera.chat-%23vinezombie-rgb?logo=liberadotchat&color=%23ff55dd)](https://web.libera.chat/gamja#vinezombie)

**vinezombie** is a Rust framework for writing IRCv3 software,
particularly clients/bots.

## Features

- An emphasis on correctness;
without using `unsafe`, it should be impossible to construct a correctly-sized
message that, once written, does not parse into the same message.
- Zero-copy parsing and sharing of message data.
- Highly-extensible parsing of IRC messages; no enums with fallback cases.
- A client-oriented handler system for querying or updating server state.
- First-class support for message tags and `labeled-response`.
- An implementation of IRCv3 connection registration, including SASL.
- Convenience utilities for creating asynchronous rustls connections,
including connections to servers using self-signed certificates.
- Zero mandatory dependencies; minimal optional dependencies.
- Designed to be flexible and modular,
usable as either a library or a highly pluggable framework.

## Building Documentation and Examples

To build and view the documentation locally, run:
```sh
RUSTDOCFLAGS="--cfg doc_unstable" cargo +nightly doc --all-features --open
```

The strings diagram in `doc` can be re-rendered using:
```sh
d2 -t 200 -l dagre --pad 0 doc/strings.d2 doc/strings.d2.svg
```

vinezombie's examples may use any combination of its features,
and should be built with `--all-features`.

## License

Licensed under the EUPL-1.2-only,
[summarized here](https://choosealicense.com/licenses/eupl-1.2/).
You agree for any contributions submitted by you for inclusion into vinezombie
to be redistributed under this license.

The EUPL is a copyleft license that covers network usage.
It requires attribution and that works that incorporate vinezombie
be made available to users (including users over a network connection) under
a compatible copyleft license (GPL v2, LGPL/GPL/AGPL v3, MPL v2, EUPL v1.2,
or refer to the license text for a full list and more information).
It also requires that changes to vinezombie in distributed works
be disclosed and made available under the EUPL.

However, the license is not automatically viral over linking or IPC, and
vinezombie's interfaces may be reproduced for interoperability with software
that uses it.

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

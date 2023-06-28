*vinezombie is slowly stabilizing. Please hold.*

# vinezombie

**An abstracting IRCv3 library in Rust.**

[![CI](https://github.com/TheDaemoness/vinezombie/actions/workflows/ci.yml/badge.svg)](https://github.com/TheDaemoness/vinezombie/actions/workflows/ci.yml)
[![Chat on libera.chat](https://img.shields.io/badge/libera.chat-%23vinezombie-blueviolet)](https://web.libera.chat/gamja/?channel=#vinezombie)

`vinezombie` is an opinionated Rust library for writing IRCv3 software.
It is designed to provide thin abstractions over the underlying protocol
while allowing the mapping between them be highly-configurable at runtime.
The goal is to allow client logic to be written as agnostically as reasonably
possible to the quirks of whatever server is being connected to.

## Building Documentation and Examples

To build and view the documentation locally, run
`RUSTDOCFLAGS="--cfg doc_unstable" cargo +nightly doc --all-features --open`

`vinezombie`'s examples may use any combination of its features,
and should be built with `--all-features`.

## License

`vinezombie` is licensed under the GNU GPL v3 (only).
Disclosing the source code of bots written using `vinezombie` to
end users over IRC is also strongly encouraged, but not required.

## Discussion

The author sometimes rambles about code design in
[#vinezombie](ircs://irc.libera.chat/#vinezombie)
on [Libera.Chat](https://libera.chat/).
A link to a webchat is available at the top by clicking the libera.chat badge.

---

```
<jess> why vinezombie lmao
<TheDaemoness> Because. Grapevines. Undead chat protocols.
<jess> oh my god
```

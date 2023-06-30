*vinezombie is slowly stabilizing. Please hold.*

# vinezombie

**A modular IRCv3 library in Rust.**

[![CI](https://github.com/vinezombie/vinezombie/actions/workflows/ci.yml/badge.svg)](https://github.com/vinezombie/vinezombie/actions/workflows/ci.yml)
[![Chat on libera.chat](https://img.shields.io/badge/libera.chat-%23vinezombie-blueviolet)](https://web.libera.chat/gamja/?channel=#vinezombie)

`vinezombie` is a Rust library for writing IRCv3 software
(mostly clients/bots at this time).
It is a toolbox for creating connections to IRC servers,
correctly parsing inbound IRC messages,
and constructing correct outbound IRC messages.
It is designed to be flexible and modular,
making minimal assumptions about how it will be used and
allowing you to use only the parts of it that you need.

## Building Documentation and Examples

To build and view the documentation locally, run:
```sh
RUSTDOCFLAGS="--cfg doc_unstable" cargo +nightly doc --all-features --open`
````

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

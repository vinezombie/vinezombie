# 0.x

## 0.3.0 (2024-04-28)

### Summary

This release is the first under the EUPL v1.2.
It contains one more round of foundational improvements to the handler system,
as well as a rework of how authentication is done. This should significantly
improve the developer ease of use, in particular regarding writing clean,
correct handlers.

### Breaking

- vinezombie is now licensed under the EUPL v1.2.
  Previous versions remain available under the GPL v3.
- Reworked authentication.
  - Rewrote `Sasl` and `SaslLogic`.
  - Rewrote the `Secret` trait as `LoadSecret`, which is meant to
    load secrets on construction of a surrounding `Secret` object (e.g. during
    deserialization). This changes when secret-related errors happen and
    allows connection registration to be less-fallible.
  - Changed the SASL `Handler` to handle multiple mechanisms.
  - Removed `zeroize` as a feature and dependency,
    opting instead to reimplement its functionality.
  - Removed `Authenticate`. `MakeHandler` is now implemented on
  `names::cmd::AUTHENTICATE` instead.
- Added `Send` bounds to many things. This enables `Client` to be `Send`.
- Added `&ClientState` and `&mut ClientState` arguments
  to handler creation and execution methods, respectively.
  This allows handlers to access state that is stored within a `Client`.
- Replaced `SendCont` with `ControlFlow<Sent>`.
- Changed the return type of `Handler::handle` to `std::ops::ControlFlow`.
- Changed `MakeHandler` to always return boxed `dyn` handlers.
  Additionally removed the `Handler` associated type from it.
- Changed `Client::new` to take a `ChannelSpec`,
  which no longer needs to be passed to every call to `Client::add`.
- Changed the registration handler to no longer return a `Registration`,
  but instead populate the shared state in `Client`.
- Changed `Register` and some of the `default_` functions to be
  more-ergonomic to work with for custom options.
- Changed the names of a few string functions.
- Reorganize `Adjuster`-related code.
  A boxed `Adujster` is now held by `Queue` instead of `Client`.
- Updated `rustls` to `0.23.5`. The `tls` feature of `vinezombie` does not
  pull in a crypto provider; use the `crypto` default feature to use `ring`.

### Non-Breaking

- Added a `crypto` feature to use `ring` for TLS and other cryptography.
It will be used in future versions for SCRAM and PKA.
- Added `ClientState` for storing shared state in a `Client` and
`client::state` to namespace a few common state variables.
- Added a `CtcpVersion` handler for auto-replying to
  CTCP VERSION and SOURCE queries.
- Added `SaslQueue` for collecting and filtering SASL mechanisms.
- Added `SecretBuf` for constructing sensitive byte strings.
- Added a `TrimAscii` string transformation.
- Added methods to `Client` to allow reusing a `Client` with a new connection.
- Fixed incorrect behavior resulting from not running some
edit guards' destructors (e.g. due to `std::mem::forget`).
- Fixed handler completion not causing the `Client` `run` functions to return.

## 0.2.0 (2024-04-07)

### Summary

This release brings sweeping reworks to handlers,
as well as a new second-stage message parsing system.
Much more work is needed to support a reasonably large set of capabilities,
ISUPPORT tokens, and messages, but the foundations have been laid and are
beginning to stabilize.

### Breaking

- Reworked handlers. They now run in `Client` instead of directly off of a
connection and yield values through channels.
- Replaced `consts` with `names` and changed the constants to be expressed
  as zero-sized structs.
  - Reworked message kind names.
  - Added names for some well-known capabilities and ISUPPORT tags.
  - Added value parsing for a small subset these. This will be expanded.
- Reworked client registration. See the `Register` struct and
  changes to `Registration`.
- Reworked nickname generation. See the `NickGen` trait.
- Reworked `ParseError`.
- Changed how one adds messages to `Queue`. See `Queue::edit`.
- Changed the names of a few methods on `Splitter` (`until` family).
- Replaced the trait for I/O timeouts in sync code with
`ReadTimeout` and `WriteTimeout`.
- Removed the `Borrow<str>` impl from `Numeric` due to
  [`clippy::impl_hash_borrow_with_str_and_bytes`](https://rust-lang.github.io/rust-clippy/master/index.html#/impl_hash_borrow_with_str_and_bytes).

### Non-Breaking

- The MSRV has been reduced to 1.70 and is now enforced by CI.
- Added `Client` to contain a connection, queue, and message handlers.
- Added `Mode` (as in channel/user modes) and collections of modes.
- Added the ability for `Queue` users to label outbound messages.
- Added `oneshot` channels and `parker` for use in synchronous contexts.
- Added `Bidir` for using two unidirectional streams as a bidirectional one.
- Added an experimental unsafe trait for types that can become owning
  (e.g. `Bytes`). The full benefits require a better borrow checker, and so
  this trait's use throughout the codebase is inconsistent and very limited.
  It may even be removed in a later version.

## 0.1.0 (2023-08-10)

Initial release!

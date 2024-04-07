# 0.x

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
- Replaced `consts` with `names` and changed the constants to be expressed as
zero-sized structs.
  - Reworked message kind names.
  - Added names for some well-known capabilities and ISUPPORT tags.
  - Added value parsing for a small subset these. This will be expanded.
- Reworked client registration. See the `Register` struct and changes
to `Registration`.
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

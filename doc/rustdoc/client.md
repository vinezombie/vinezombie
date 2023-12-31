Utilities for building IRC clients or bots.

This module includes a wide assortment of structs and traits,
basically anyone one might need to get started writing a
client-side IRC program. They are somewhat opinionated,
though some opinions are due to Rust type system limitations.
While the 

## Handlers

This module includes rudimentary synchronous message handling in the form of
the [`Handler`] trait. Handlers that need to spawn asynchronous tasks should
implement [`HandlerAsync`] instead.

Handlers can be run off of an existing connection using
[`run_handler`] or [`run_handler_tokio`], depending on
what style of I/O your application uses.

## Connections

Much of the early boilerplate required for making an IRC client involves
creating and registering a connection.
As such, this module contains utilities for making that easier.

[`tls`] provides means of creating rustls client configurations that are
adequate for most use cases (including client certificate authentication)
and may be significantly more convenient than working with rustls directly.
See [`TlsConfigOptions`][tls::TlsConfigOptions] for options (of course).

[`conn`] provides types and functions for creating TCP connections,
either with or without TLS, and abstractions over different connection types.
See [`ServerAddr`][conn::ServerAddr] for basic connection options.

[`register`] provides an implementation
of the full IRCv3 connection registration handshake, including SASL.
See [`Register`][register::Register] for available options.

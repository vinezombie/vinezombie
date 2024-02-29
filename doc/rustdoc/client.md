Utilities for building IRC clients or bots.

This module includes a wide assortment of structs and traits,
basically anyone one might need to get started writing a
client-side IRC program. They are somewhat opinionated,
though some opinions are due to Rust type system limitations.
While the 

## `Client` and `Handler`s

[`Client`] combines several utilities for writing correct IRC clients
with an event loop. Most users of this library will likely use this,
though it is possible to use most of its components to build one's own
event loop.

One interacts with a `Client` by [`add`][Client::add]ing [`Handler`]s.
`Handler`s are synchronous message handlers that are used to minimally
process messages and send them elsewhere for the application logic to handle.
Each handler is associated with one channel (message-passing channels,
not IRC channels).

`Handler`s are created using implementations of the [`MakeHandler`] trait.
These implementations can also queue initial messages so the handler
doesn't have to, and are also responsible for creating channels for
the handler if required.

Once one has added their desired handlers, the [`Client::run`]
or [`Client::run_tokio`] (depending on I/O flavor) functions
can be used to exchange messages between the client and server
until one or more handlers reports that it has yielded a value or finished.

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

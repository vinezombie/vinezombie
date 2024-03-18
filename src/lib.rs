#![doc = include_str!("../doc/rustdoc/lib.md")]
#![allow(clippy::borrow_interior_mutable_const)]
#![allow(clippy::mutable_key_type)]
#![deny(clippy::missing_safety_doc)]
#![deny(clippy::redundant_else)]
#![deny(clippy::semicolon_if_nothing_returned)]
#![deny(missing_docs)]
#![deny(rustdoc::bare_urls)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::invalid_codeblock_attributes)]
#![deny(rustdoc::invalid_html_tags)]
#![deny(rustdoc::invalid_rust_codeblocks)]
#![deny(rustdoc::private_intra_doc_links)]
#![cfg_attr(doc_unstable, feature(doc_auto_cfg))]

#[cfg(feature = "client")]
pub mod client;
pub mod consts;
pub mod error;
pub mod ircmsg;
pub mod owning;
pub mod state;
pub mod string;
pub(crate) mod util;

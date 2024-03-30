//! Well-known values for IRC messages.
//!
//! This module contains a large number of zero-sized `structs` which can stand in for
//! specific values within IRC messages. Each of these implements [`Name`],
//! which provides most of the necessary structure. Some implement [`NameValued`],
//! which allows parsing a value out from the context the `Name` implementor can usually be
//! found in (e.g. an IRC message).
#![allow(non_camel_case_types)]

// Throughout this, we'll be doing the `from_unchecked(Bytes::from_str)` dance.
// This helps compilation times, as we trust ourselves.

pub mod cap;
pub mod cmd;
pub mod isupport;
mod types;

pub use types::*;

use crate::string::{Arg, Bytes, Nick};

/// The literal `"*"`.
///
/// This shows up pretty commonly in argument lists,
/// so this constant is provided for convenience.
/// It is occasionally also used as the first argument of numeric replies.
#[allow(clippy::declare_interior_mutable_const)]
pub const STAR: Nick<'static> = unsafe { Nick::from_unchecked(Bytes::from_str("*")) };

/// The literal `"+"`.
///
/// Used as a placeholder when a base64-encoded field is empty.
#[allow(clippy::declare_interior_mutable_const)]
pub const PLUS: Arg<'static> = unsafe { Arg::from_unchecked(Bytes::from_str("+")) };

/// Marker for tags that differentiate client-originated messages.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ClientMsgKind {}

impl NameClass for ClientMsgKind {
    type Raw<'a> = crate::string::Cmd<'a>;
    type Union<'a> = crate::ircmsg::ClientMsg<'a>;
    fn get_tag<'a, 'b>(outer: &'a Self::Union<'b>) -> &'a Self::Raw<'b> {
        &outer.cmd
    }
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Union<'b>) -> &'a mut Self::Raw<'b> {
        &mut outer.cmd
    }
}

/// Marker for tags that differentiate server-originated messages.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ServerMsgKind {}

impl NameClass for ServerMsgKind {
    type Raw<'a> = crate::ircmsg::ServerMsgKindRaw<'a>;
    type Union<'a> = crate::ircmsg::ServerMsg<'a>;
    fn get_tag<'a, 'b>(outer: &'a Self::Union<'b>) -> &'a Self::Raw<'b> {
        &outer.kind
    }
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Union<'b>) -> &'a mut Self::Raw<'b> {
        &mut outer.kind
    }
}

/// Marker for the key halves of ISUPPORT tags.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ISupport {}

impl NameClass for ISupport {
    type Raw<'a> = crate::string::Key<'a>;
    type Union<'a> = (Self::Raw<'a>, crate::string::Word<'a>);
    fn get_tag<'a, 'b>(outer: &'a Self::Union<'b>) -> &'a Self::Raw<'b> {
        &outer.0
    }
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Union<'b>) -> &'a mut Self::Raw<'b> {
        &mut outer.0
    }
}

/// Marker for IRCv3 message tags.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MsgTag {}

impl NameClass for MsgTag {
    type Raw<'a> = crate::string::Key<'a>;
    type Union<'a> = (Self::Raw<'a>, crate::string::NoNul<'a>);
    fn get_tag<'a, 'b>(outer: &'a Self::Union<'b>) -> &'a Self::Raw<'b> {
        &outer.0
    }
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Union<'b>) -> &'a mut Self::Raw<'b> {
        &mut outer.0
    }
}

/// Marker for capabilities.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Cap {}

impl NameClass for Cap {
    type Raw<'a> = crate::string::Key<'a>;
    type Union<'a> = (Self::Raw<'a>, crate::string::Word<'a>);
    fn get_tag<'a, 'b>(outer: &'a Self::Union<'b>) -> &'a Self::Raw<'b> {
        &outer.0
    }
    fn get_tag_mut<'a, 'b>(outer: &'a mut Self::Union<'b>) -> &'a mut Self::Raw<'b> {
        &mut outer.0
    }
}

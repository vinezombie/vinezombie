//! Well-known values for IRC messages.
//!
//! These constants exist to sidestep needing to use `from_unchecked` all over the place
//! for a large subset of possible messages.

// Throughout this, we'll be doing the `from_unchecked(Bytes::from_str)` dance.
// This helps compilation times, as we trust ourselves.

/// Commands.
pub mod cmd;
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

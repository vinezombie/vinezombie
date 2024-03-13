//! Definitions for IRC state tracking.

mod mode;
pub mod serverinfo;
#[cfg(test)]
mod tests;

pub use mode::*;

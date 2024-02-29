mod dynsized;
mod ownedslice;
#[cfg(test)]
mod tests;
mod thinarc;

pub use dynsized::*;
pub use ownedslice::*;
pub use thinarc::*;

#[allow(unused)]
pub fn option_union_with<T, F: FnOnce(T, T) -> T>(a: Option<T>, b: Option<T>, f: F) -> Option<T> {
    match (a, b) {
        (None, None) => None,
        (None, Some(v)) => Some(v),
        (Some(v), None) => Some(v),
        (Some(a), Some(b)) => Some(f(a, b)),
    }
}

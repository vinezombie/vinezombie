#![allow(deprecated)] // No sense letting std's SipHash implementation go to waste.

use std::hash::{Hash, Hasher};

static HASHER: std::sync::OnceLock<std::hash::SipHasher> = std::sync::OnceLock::new();

/// Creates a 32-bit hash using platform information.
///
/// This hash is not suitable for cryptographic purposes.
/// It is essentially a pseudorandom value derived from the provided `Hash` impl
/// that is supposed to be difficult to reverse.
pub fn mangle(h: &impl Hash) -> u32 {
    let mut hasher = HASHER
        .get_or_init(|| {
            let mut seeder = std::hash::SipHasher::new();
            std::env::consts::ARCH.hash(&mut seeder);
            #[cfg(feature = "whoami")]
            {
                // Deprecated, but that doesn't matter for our purposes.
                whoami::hostname().hash(&mut seeder);
                if let Ok(langs) = whoami::langs() {
                    for lang in langs {
                        lang.to_string().hash(&mut seeder);
                    }
                }
            }
            let key_a = seeder.finish();
            std::env::consts::OS.hash(&mut seeder);
            #[cfg(feature = "whoami")]
            {
                whoami::distro().hash(&mut seeder);
                whoami::devicename().hash(&mut seeder);
            }
            let key_b = seeder.finish();
            std::hash::SipHasher::new_with_keys(key_a, key_b)
        })
        .clone();
    h.hash(&mut hasher);
    let hash = hasher.finish();
    // XOR-fold.
    ((hash >> 32) | (hash & 0xFFFFFFFF)) as u32
}

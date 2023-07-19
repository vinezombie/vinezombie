//! Base64 encoding and decoding.

use super::{Arg, Splitter};
use crate::string::Bytes;
use base64::DecodeError;

// Do not impl Debug. The encoders here may handle sensitive data.

/// `AUTHENTICATE`-style Base64 encoder.
/// Encodes data using Base64, then splits them into chunks no longer
/// than some pre-determined number of bytes.
#[derive(Clone)]
pub struct ChunkEncoder {
    splitter: Option<Splitter<Arg<'static>>>,
    max: usize,
}

impl ChunkEncoder {
    /// Constructs a new chunk encoder with a maximum chunk size of `max`.
    /// If `secret` is true, the `Arg`s yielded by this encoder will be secret.
    pub fn new<B: AsRef<[u8]>>(bytes: B, max: usize, secret: bool) -> Self {
        use base64::engine::{general_purpose::STANDARD as ENGINE, Engine};
        if max == 0 {
            return ChunkEncoder::empty();
        }
        let encoded: Bytes<'static> = ENGINE.encode(bytes).into();
        let encoded = if secret { encoded.secret() } else { encoded };
        let splitter = if !encoded.is_empty() {
            let encoded = unsafe { Arg::from_unchecked(encoded) };
            Some(Splitter::new(encoded))
        } else {
            None
        };
        ChunkEncoder { splitter, max }
    }
    /// Constructs an empty chunk encoder.
    pub const fn empty() -> Self {
        ChunkEncoder { splitter: None, max: 0 }
    }
    /// Returns `true` if this chunk encoder is empty.
    pub fn is_empty(&self) -> bool {
        self.splitter.is_some()
    }
}

impl Default for ChunkEncoder {
    fn default() -> Self {
        ChunkEncoder::empty()
    }
}

impl Iterator for ChunkEncoder {
    type Item = Arg<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(splitter) = &mut self.splitter else {
            return None;
        };
        let Ok(chunk) = splitter.save_end().until_count(self.max).rest::<Arg>() else {
            let retval = if splitter.is_secret() {
                crate::consts::PLUS.secret()
            } else {
                crate::consts::PLUS
            };
            self.splitter = None;
            return Some(retval);
        };
        if chunk.len() < self.max {
            self.splitter = None;
        }
        Some(chunk)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // TODO: Can do better.
        if self.is_empty() {
            (0, Some(0))
        } else {
            (1, None)
        }
    }
}
impl std::iter::FusedIterator for ChunkEncoder {}

/// `AUTHENTICATE`-style Base64 decoder.
/// Accepts chunks until it receives one that is not a pre-determined number of bytes long.
#[derive(Clone)]
pub struct ChunkDecoder(Vec<u8>, usize);

impl ChunkDecoder {
    /// Creates a new decoder.
    pub const fn new(chunk_len: usize) -> Self {
        Self(Vec::new(), chunk_len)
    }

    /// Adds a chunk of base64-encoded data.
    ///
    /// If `chunk` is shorter than the chunk length the decoder was provided,
    /// treats `chunk` as the final chunk and attempts decoding.
    ///
    /// If `chunk` is `"+"`, the chunk is treated as an empty chunk.
    pub fn add<B: AsRef<[u8]>>(&mut self, chunk: B) -> Option<Result<Bytes<'static>, DecodeError>> {
        let chunk = chunk.as_ref();
        if chunk.len() < self.1 {
            if chunk != b"+" {
                self.0.extend_from_slice(chunk);
            }
            Some(self.decode())
        } else {
            self.0.extend_from_slice(chunk);
            None
        }
    }

    /// Decodes the data already added to the decoder.
    ///
    /// This operation leaves the decoder empty.
    pub fn decode(&mut self) -> Result<Bytes<'static>, DecodeError> {
        use base64::engine::{general_purpose::STANDARD as ENGINE, Engine};
        ENGINE.decode(std::mem::take(&mut self.0)).map(Bytes::from)
    }
}

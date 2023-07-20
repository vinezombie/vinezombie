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
    splitter: Result<Splitter<Arg<'static>>, bool>,
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
        let splitter = if !encoded.is_empty() {
            let mut encoded = unsafe { Arg::from_unchecked(encoded) };
            if secret {
                encoded = encoded.secret();
            }
            Ok(Splitter::new(encoded))
        } else {
            Err(secret)
        };
        ChunkEncoder { splitter, max }
    }
    /// Constructs an empty chunk encoder.
    pub const fn empty() -> Self {
        ChunkEncoder { splitter: Err(false), max: 0 }
    }
    /// Returns `true` if this chunk encoder is empty.
    pub fn is_empty(&self) -> bool {
        self.max == 0
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
        if self.max == 0 {
            return None;
        }
        match &mut self.splitter {
            Ok(splitter) => {
                let chunk = splitter.save_end().until_count(self.max).rest::<Arg>().unwrap();
                if chunk.len() < self.max {
                    *self = Self::empty();
                } else if splitter.is_empty() {
                    self.splitter = Err(splitter.is_secret());
                }
                Some(chunk)
            }
            Err(secret) => {
                let retval =
                    if *secret { crate::consts::PLUS.secret() } else { crate::consts::PLUS };
                *self = Self::empty();
                Some(retval)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let chunks = std::iter::ExactSizeIterator::len(self);
        (chunks, Some(chunks))
    }
}
impl std::iter::FusedIterator for ChunkEncoder {}
impl std::iter::ExactSizeIterator for ChunkEncoder {
    fn len(&self) -> usize {
        if self.max == 0 {
            0
        } else if let Ok(splitter) = &self.splitter {
            // Integer division plus one is intended here.
            // On exactly max bytes, we need to send an extra +.
            splitter.len() / self.max + 1
        } else {
            1
        }
    }
}

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

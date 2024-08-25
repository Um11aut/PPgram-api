use sha2::{Digest, Sha256};
use base64::prelude::*;

/// Hashes binary and outputs sha256 Hash
pub struct BinaryHasher {
    hasher: Sha256
}

impl BinaryHasher {
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
        }
    }

    pub fn hash_full(mut self, encoded_binary: &str) -> Result<(Vec<u8>, String), base64::DecodeError> {
        let decoded_binary = BASE64_STANDARD.decode(encoded_binary)?;
        self.hasher.update(decoded_binary.as_slice());

        let res = self.hasher.finalize();
        Ok((decoded_binary, BASE64_STANDARD.encode(res)))
    }
}
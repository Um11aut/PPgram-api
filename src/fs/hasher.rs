use log::info;
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

    pub fn hash_part(&mut self, binary: &[u8]) {
        self.hasher.update(binary);
    }

    pub fn finalize(self) -> String {
        let res = self.hasher.finalize();
        hex::encode(res)
    }
}
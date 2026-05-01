use anyhow::Result;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{}", hex::encode(h.finalize()))
}

pub fn sha256_file(path: impl AsRef<Path>) -> Result<String> {
    Ok(sha256_bytes(&std::fs::read(path)?))
}

pub fn sha256_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(sha256_bytes(&serde_json::to_vec(value)?))
}

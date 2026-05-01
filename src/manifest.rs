use crate::hash::sha256_file;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CORPUS_SCHEMA_VERSION: &str = "quick-search-corpus-v0.2.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorpusManifest {
    pub schema_version: String,
    pub docs: usize,
    pub predicate_count: usize,
    pub metadata_digest: String,
    pub predicate_bitset_digest: String,
    pub registry_digest: String,
    pub vector_index_digest: Option<String>,
}

impl CorpusManifest {
    pub fn from_art_dir(art_dir: impl AsRef<Path>, docs: usize, predicate_count: usize) -> Result<Self> {
        let art_dir = art_dir.as_ref();
        let vector_path = art_dir.join("vector_index.json");
        Ok(Self {
            schema_version: CORPUS_SCHEMA_VERSION.to_string(),
            docs,
            predicate_count,
            metadata_digest: sha256_file(art_dir.join("metadata.sqlite"))?,
            predicate_bitset_digest: sha256_file(art_dir.join("predicates.qsbit"))?,
            registry_digest: sha256_file(art_dir.join("registry.json"))?,
            vector_index_digest: if vector_path.exists() { Some(sha256_file(vector_path)?) } else { None },
        })
    }

    pub fn write(&self, art_dir: impl AsRef<Path>) -> Result<()> {
        std::fs::write(art_dir.as_ref().join("corpus_manifest.json"), serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}

use crate::corpus::Paper;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VectorHit {
    pub doc_index: usize,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VectorIndex {
    vocab: BTreeMap<String, usize>,
    idf: Vec<f64>,
    docs: Vec<Vec<(usize, f64)>>,
}

impl VectorIndex {
    pub fn build(papers: &[Paper]) -> Self {
        let tokenized: Vec<Vec<String>> = papers
            .iter()
            .map(|p| tokenize(&format!("{} {}", p.title, p.abstract_text)))
            .collect();

        let mut vocab = BTreeMap::new();
        let mut df = BTreeMap::<String, usize>::new();

        for toks in &tokenized {
            let unique: BTreeSet<_> = toks.iter().cloned().collect();
            for tok in unique {
                *df.entry(tok).or_insert(0) += 1;
            }
        }

        for tok in df.keys() {
            let next = vocab.len();
            vocab.insert(tok.clone(), next);
        }

        let n_docs = papers.len().max(1) as f64;
        let mut idf = vec![0.0; vocab.len()];
        for (tok, idx) in &vocab {
            let d = *df.get(tok).unwrap_or(&1) as f64;
            idf[*idx] = ((1.0 + n_docs) / (1.0 + d)).ln() + 1.0;
        }

        let docs = tokenized.iter().map(|toks| encode_tokens(toks, &vocab, &idf)).collect();

        Self { vocab, idf, docs }
    }

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<VectorHit>> {
        if self.docs.is_empty() {
            return Ok(Vec::new());
        }

        let q = encode_tokens(&tokenize(query), &self.vocab, &self.idf);
        if q.is_empty() {
            return Err(anyhow!("query produced no known vector terms"));
        }

        let mut hits: Vec<_> = self
            .docs
            .iter()
            .enumerate()
            .map(|(doc_index, doc)| VectorHit { doc_index, score: cosine_sparse(&q, doc) })
            .filter(|h| h.score > 0.0)
            .collect();

        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal).then_with(|| a.doc_index.cmp(&b.doc_index)));
        hits.truncate(limit);
        Ok(hits)
    }
}

pub fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            cur.push(ch.to_ascii_lowercase());
        } else if !cur.is_empty() {
            if cur.len() >= 2 {
                out.push(cur.clone());
            }
            cur.clear();
        }
    }

    if cur.len() >= 2 {
        out.push(cur);
    }

    out
}

fn encode_tokens(tokens: &[String], vocab: &BTreeMap<String, usize>, idf: &[f64]) -> Vec<(usize, f64)> {
    let mut tf = BTreeMap::<usize, f64>::new();

    for tok in tokens {
        if let Some(idx) = vocab.get(tok) {
            *tf.entry(*idx).or_insert(0.0) += 1.0;
        }
    }

    let mut v: Vec<_> = tf.into_iter().map(|(idx, count)| (idx, count * idf[idx])).collect();
    normalize_sparse(&mut v);
    v
}

fn normalize_sparse(v: &mut [(usize, f64)]) {
    let norm = v.iter().map(|(_, x)| x * x).sum::<f64>().sqrt();
    if norm > 0.0 {
        for (_, x) in v {
            *x /= norm;
        }
    }
}

fn cosine_sparse(a: &[(usize, f64)], b: &[(usize, f64)]) -> f64 {
    let mut i = 0usize;
    let mut j = 0usize;
    let mut total = 0.0;

    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            Ordering::Equal => {
                total += a[i].1 * b[j].1;
                i += 1;
                j += 1;
            }
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
        }
    }

    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::{PaperMeta, Paper};

    #[test]
    fn vector_search_ranks_semantic_overlap() {
        let papers = vec![
            Paper { id: 1, title: "Theropod endothermy".into(), abstract_text: "Warm blooded dinosaur metabolism".into(), meta: PaperMeta { year: Some(2021), journal: "Nature".into(), authors: vec![], doi: String::new() } },
            Paper { id: 2, title: "Plant fossils".into(), abstract_text: "Leaves and stems".into(), meta: PaperMeta { year: Some(2001), journal: "Other".into(), authors: vec![], doi: String::new() } },
        ];

        let ix = VectorIndex::build(&papers);
        let hits = ix.search("dinosaur metabolism warm blooded", 10).unwrap();

        assert_eq!(hits[0].doc_index, 0);
        assert!(hits[0].score > 0.0);
    }
}

use crate::bitset::{read_named_bitsets, write_named_bitsets, NamedBitset, PackedBitset};
use crate::predicates::{build_registry, load_predicate_pack, Predicate};
use anyhow::{anyhow, Context, Result};
use csv::StringRecord;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaperMeta {
    pub year: Option<i32>,
    pub journal: String,
    pub authors: Vec<String>,
    pub doi: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Paper {
    pub id: i64,
    pub title: String,
    pub abstract_text: String,
    pub meta: PaperMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryResult {
    pub id: i64,
    pub title: String,
    pub preview: String,
    pub year: Option<i32>,
    pub journal: String,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct BitsetCorpus {
    pub registry: BTreeMap<String, Predicate>,
    pub papers: Vec<Paper>,
    pub bits: BTreeMap<String, PackedBitset>,
}

impl BitsetCorpus {
    pub fn new() -> Self {
        let registry = build_registry();
        Self::from_registry(registry)
    }

    pub fn from_predicate_pack(path: impl AsRef<Path>) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let registry = load_predicate_pack(&path_str).map_err(|e| anyhow!("load predicate pack {}: {}", path.as_ref().display(), e))?;
        Ok(Self::from_registry(registry))
    }

    pub fn from_registry(registry: BTreeMap<String, Predicate>) -> Self {
        let bits = registry.keys().map(|k| (k.clone(), PackedBitset::new())).collect();
        Self { registry, papers: Vec::new(), bits }
    }

    pub fn add(&mut self, paper: Paper) {
        let text = format!("{}. {}", paper.title, paper.abstract_text);
        for (name, pred) in &self.registry {
            let matched = pred.matches(&text, &paper.meta);
            self.bits.entry(name.clone()).or_default().push(matched);
        }
        self.papers.push(paper);
    }

    pub fn from_csv(csv_path: impl AsRef<Path>) -> Result<Self> {
        Self::from_csv_with_predicate_pack(csv_path, None::<&Path>)
    }

    pub fn from_csv_with_predicate_pack(csv_path: impl AsRef<Path>, pack_path: Option<impl AsRef<Path>>) -> Result<Self> {
        let mut rdr = csv::Reader::from_path(csv_path.as_ref())
            .with_context(|| format!("read CSV {}", csv_path.as_ref().display()))?;
        let headers = rdr.headers()?.clone();
        let mut corpus = match pack_path {
            Some(path) => Self::from_predicate_pack(path)?,
            None => Self::new(),
        };
        for row in rdr.records() {
            let row = row?;
            corpus.add(parse_paper(&headers, &row)?);
        }
        Ok(corpus)
    }

    pub fn save(&self, art_dir: impl AsRef<Path>) -> Result<()> {
        let art_dir = art_dir.as_ref();
        std::fs::create_dir_all(art_dir)?;
        self.save_metadata(art_dir.join("metadata.sqlite"))?;
        let sets: Vec<NamedBitset> = self.bits.iter().map(|(name, bitset)| NamedBitset { name: name.clone(), bitset: bitset.clone() }).collect();
        write_named_bitsets(art_dir.join("predicates.qsbit"), &sets)?;
        std::fs::write(art_dir.join("registry.json"), serde_json::to_vec_pretty(&self.registry)?)?;
        Ok(())
    }

    pub fn load(art_dir: impl AsRef<Path>) -> Result<Self> {
        let art_dir = art_dir.as_ref();
        let papers = load_metadata(art_dir.join("metadata.sqlite"))?;
        let registry: BTreeMap<String, Predicate> = serde_json::from_slice(&std::fs::read(art_dir.join("registry.json"))?)?;
        let mut bits = BTreeMap::new();
        for named in read_named_bitsets(art_dir.join("predicates.qsbit"))? {
            if named.bitset.len() != papers.len() {
                return Err(anyhow!("bitset {} len {} != papers len {}", named.name, named.bitset.len(), papers.len()));
            }
            bits.insert(named.name, named.bitset);
        }
        Ok(Self { registry, papers, bits })
    }

    pub fn query(&self, include: &[String], exclude: &[String], limit: usize) -> Result<Vec<QueryResult>> {
        if self.papers.is_empty() { return Ok(Vec::new()); }
        let mut mask = PackedBitset::all_ones(self.papers.len());
        for p in include {
            let b = self.bits.get(p).ok_or_else(|| anyhow!("unknown include predicate: {p}"))?;
            mask.and_inplace(b)?;
        }
        for p in exclude {
            let b = self.bits.get(p).ok_or_else(|| anyhow!("unknown exclude predicate: {p}"))?;
            mask.and_not_inplace(b)?;
        }
        Ok(mask.select_indices(limit).into_iter().map(|i| self.result_for_index(i, include.len())).collect())
    }

    pub fn predicate_counts(&self) -> BTreeMap<String, u64> {
        self.bits.iter().map(|(k, b)| (k.clone(), b.count_ones())).collect()
    }

    fn result_for_index(&self, i: usize, include_count: usize) -> QueryResult {
        let p = &self.papers[i];
        QueryResult {
            id: p.id,
            title: p.title.clone(),
            preview: truncate_chars(&p.abstract_text, 240),
            year: p.meta.year,
            journal: p.meta.journal.clone(),
            score: (include_count as f64) + 1.0,
        }
    }

    fn save_metadata(&self, path: PathBuf) -> Result<()> {
        let con = Connection::open(path)?;
        con.execute_batch("DROP TABLE IF EXISTS papers; CREATE TABLE papers(id INTEGER PRIMARY KEY, title TEXT NOT NULL, abstract TEXT NOT NULL, year INTEGER, journal TEXT NOT NULL, authors TEXT NOT NULL, doi TEXT NOT NULL);")?;
        let mut stmt = con.prepare("INSERT INTO papers(id,title,abstract,year,journal,authors,doi) VALUES(?,?,?,?,?,?,?)")?;
        for p in &self.papers {
            stmt.execute(params![p.id, p.title, p.abstract_text, p.meta.year, p.meta.journal, serde_json::to_string(&p.meta.authors)?, p.meta.doi])?;
        }
        Ok(())
    }
}

impl Default for BitsetCorpus { fn default() -> Self { Self::new() } }

fn load_metadata(path: impl AsRef<Path>) -> Result<Vec<Paper>> {
    let con = Connection::open(path.as_ref()).with_context(|| format!("open {}", path.as_ref().display()))?;
    let mut stmt = con.prepare("SELECT id,title,abstract,year,journal,authors,doi FROM papers ORDER BY rowid")?;
    let rows = stmt.query_map([], |r| {
        let authors_json: String = r.get(5)?;
        let authors: Vec<String> = serde_json::from_str(&authors_json).unwrap_or_default();
        Ok(Paper {
            id: r.get(0)?,
            title: r.get(1)?,
            abstract_text: r.get(2)?,
            meta: PaperMeta { year: r.get(3)?, journal: r.get(4)?, authors, doi: r.get(6)? },
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

fn parse_paper(headers: &StringRecord, row: &StringRecord) -> Result<Paper> {
    let get = |name: &str| -> Option<&str> { headers.iter().position(|h| h == name).and_then(|i| row.get(i)) };
    let id: i64 = get("id").ok_or_else(|| anyhow!("missing id column"))?.parse().context("parse id")?;
    let title = get("title").unwrap_or("").to_string();
    let abstract_text = get("abstract").unwrap_or("").to_string();
    let year = match get("year").map(str::trim).filter(|s| !s.is_empty()) { Some(y) => Some(y.parse().context("parse year")?), None => None };
    let journal = get("journal").unwrap_or("").to_string();
    let authors = get("authors").unwrap_or("").split(';').map(str::trim).filter(|s| !s.is_empty()).map(ToString::to_string).collect();
    let doi = get("doi").unwrap_or("").to_string();
    Ok(Paper { id, title, abstract_text, meta: PaperMeta { year, journal, authors, doi } })
}

fn truncate_chars(s: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max { out.push('…'); break; }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_load_and_query_roundtrip() {
        let dir = tempdir().unwrap();
        let mut c = BitsetCorpus::new();
        c.add(Paper { id: 1, title: "Theropod endothermy".into(), abstract_text: "Metabolic rate and bone histology in dinosaurs.".into(), meta: PaperMeta { year: Some(2021), journal: "Nature".into(), authors: vec!["A".into()], doi: "10/x".into() } });
        c.add(Paper { id: 2, title: "Plant fossils".into(), abstract_text: "Leaves.".into(), meta: PaperMeta { year: Some(2001), journal: "Other".into(), authors: vec![], doi: String::new() } });
        c.save(dir.path()).unwrap();
        let loaded = BitsetCorpus::load(dir.path()).unwrap();
        let res = loaded.query(&["topic_dinosaurs".into(), "recent_2020s".into()], &[], 10).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, 1);
    }
}

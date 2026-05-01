use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use strsim::normalized_levenshtein;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PredicateKind {
    Keywords { words: Vec<String>, threshold_percent: u8 },
    YearRange { start: i32, end: i32 },
    JournalContains { needle: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Predicate {
    pub name: String,
    pub kind: PredicateKind,
}

impl Predicate {
    pub fn keywords(name: &str, words: &[&str], threshold_percent: u8) -> Self {
        Self {
            name: name.to_string(),
            kind: PredicateKind::Keywords {
                words: words.iter().map(|s| s.to_lowercase()).collect(),
                threshold_percent,
            },
        }
    }

    pub fn year_range(name: &str, start: i32, end: i32) -> Self {
        Self { name: name.to_string(), kind: PredicateKind::YearRange { start, end } }
    }

    pub fn journal(name: &str, needle: &str) -> Self {
        Self { name: name.to_string(), kind: PredicateKind::JournalContains { needle: needle.to_lowercase() } }
    }

    pub fn matches(&self, text: &str, meta: &crate::corpus::PaperMeta) -> bool {
        match &self.kind {
            PredicateKind::Keywords { words, threshold_percent } => {
                let t = text.to_lowercase();
                let threshold = *threshold_percent as f64 / 100.0;
                words.iter().any(|w| t.contains(w) || fuzzy_substring_match(w, &t, threshold))
            }
            PredicateKind::YearRange { start, end } => meta.year.map(|y| y >= *start && y <= *end).unwrap_or(false),
            PredicateKind::JournalContains { needle } => meta.journal.to_lowercase().contains(needle),
        }
    }
}

fn fuzzy_substring_match(needle: &str, haystack: &str, threshold: f64) -> bool {
    if needle.is_empty() || haystack.is_empty() { return false; }
    let n_chars = needle.chars().count();
    let h: Vec<char> = haystack.chars().collect();
    if h.len() <= n_chars { return normalized_levenshtein(needle, haystack) >= threshold; }
    let span_min = n_chars.saturating_sub(3).max(1);
    let span_max = (n_chars + 8).min(h.len());
    for span in span_min..=span_max {
        for start in 0..=h.len() - span {
            let candidate: String = h[start..start + span].iter().collect();
            if normalized_levenshtein(needle, &candidate) >= threshold { return true; }
        }
    }
    false
}

pub fn build_registry() -> BTreeMap<String, Predicate> {
    let predicates = vec![
        Predicate::keywords("topic_dinosaurs", &["dinosaur", "theropod", "sauropod", "ceratopsian"], 80),
        Predicate::keywords("topic_endothermy", &["endothermy", "warm-blooded", "metabolic rate", "thermoregulation"], 80),
        Predicate::keywords("topic_isotopes", &["oxygen isotope", "isotope analysis", "δ18O", "d18o"], 80),
        Predicate::keywords("topic_histology", &["bone histology", "haversian", "cortical bone"], 80),
        Predicate::year_range("recent_2020s", 2020, 2029),
        Predicate::year_range("recent_2010s", 2010, 2019),
        Predicate::journal("journal_nature", "Nature"),
        Predicate::journal("journal_science", "Science"),
        Predicate::journal("journal_isci", "iScience"),
    ];
    predicates.into_iter().map(|p| (p.name.clone(), p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::PaperMeta;

    #[test]
    fn keyword_and_year_predicates_match() {
        let meta = PaperMeta { year: Some(2023), journal: "Nature Ecology".into(), authors: vec![], doi: String::new() };
        let reg = build_registry();
        assert!(reg["topic_dinosaurs"].matches("Theropod metabolism", &meta));
        assert!(reg["recent_2020s"].matches("", &meta));
        assert!(reg["journal_nature"].matches("", &meta));
    }
}

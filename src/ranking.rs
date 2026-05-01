use crate::corpus::{Paper, QueryResult};
use crate::vector::VectorIndex;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankedCitation {
    pub id: i64,
    pub title: String,
    pub preview: String,
    pub year: Option<i32>,
    pub journal: String,
    pub doi: String,
    pub predicate_score: f64,
    pub vector_score: f64,
    pub recency_score: f64,
    pub authority_score: f64,
    pub final_score: f64,
}

pub fn rank_results(
    papers: &[Paper],
    predicate_results: &[QueryResult],
    vector_index: Option<&VectorIndex>,
    claim_text: &str,
    limit: usize,
) -> Result<Vec<RankedCitation>> {
    let vector_scores: BTreeMap<usize, f64> = match vector_index {
        Some(ix) => ix.search(claim_text, papers.len()).unwrap_or_default().into_iter().map(|h| (h.doc_index, h.score)).collect(),
        None => BTreeMap::new(),
    };

    let mut ranked = Vec::new();

    for r in predicate_results {
        if let Some((idx, paper)) = papers.iter().enumerate().find(|(_, p)| p.id == r.id) {
            let vector_score = *vector_scores.get(&idx).unwrap_or(&0.0);
            let recency_score = recency_score(paper.meta.year);
            let authority_score = authority_score(&paper.meta.journal);
            let predicate_score = r.score;

            let final_score =
                0.45 * predicate_score +
                0.30 * vector_score +
                0.15 * recency_score +
                0.10 * authority_score;

            ranked.push(RankedCitation {
                id: paper.id,
                title: paper.title.clone(),
                preview: r.preview.clone(),
                year: paper.meta.year,
                journal: paper.meta.journal.clone(),
                doi: paper.meta.doi.clone(),
                predicate_score,
                vector_score,
                recency_score,
                authority_score,
                final_score,
            });
        }
    }

    ranked.sort_by(|a, b| {
        b.final_score
            .partial_cmp(&a.final_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.year.cmp(&a.year))
            .then_with(|| a.id.cmp(&b.id))
    });

    ranked.truncate(limit);
    Ok(ranked)
}

fn recency_score(year: Option<i32>) -> f64 {
    match year {
        Some(y) if y >= 2020 => 1.0,
        Some(y) if y >= 2010 => 0.75,
        Some(y) if y >= 2000 => 0.50,
        Some(_) => 0.25,
        None => 0.0,
    }
}

fn authority_score(journal: &str) -> f64 {
    let j = journal.to_ascii_lowercase();
    if j.contains("nature") || j.contains("science") {
        1.0
    } else if j.contains("cell") || j.contains("pnas") || j.contains("isci") {
        0.8
    } else {
        0.4
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::{PaperMeta, QueryResult};

    #[test]
    fn ranking_prefers_recent_authoritative_vector_overlap() {
        let papers = vec![
            Paper { id: 1, title: "Theropod endothermy".into(), abstract_text: "Warm blooded dinosaur metabolism".into(), meta: PaperMeta { year: Some(2021), journal: "Nature".into(), authors: vec![], doi: "10/a".into() } },
            Paper { id: 2, title: "Old dinosaur note".into(), abstract_text: "Dinosaur".into(), meta: PaperMeta { year: Some(1999), journal: "Other".into(), authors: vec![], doi: "10/b".into() } },
        ];

        let predicate_results = vec![
            QueryResult { id: 1, title: papers[0].title.clone(), preview: papers[0].abstract_text.clone(), year: papers[0].meta.year, journal: papers[0].meta.journal.clone(), score: 3.0 },
            QueryResult { id: 2, title: papers[1].title.clone(), preview: papers[1].abstract_text.clone(), year: papers[1].meta.year, journal: papers[1].meta.journal.clone(), score: 3.0 },
        ];

        let ix = VectorIndex::build(&papers);
        let ranked = rank_results(&papers, &predicate_results, Some(&ix), "warm blooded dinosaur metabolism", 10).unwrap();

        assert_eq!(ranked[0].id, 1);
        assert!(ranked[0].final_score > ranked[1].final_score);
    }
}

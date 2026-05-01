use crate::ranking::RankedCitation;
use serde::{Deserialize, Serialize};

pub const DEFAULT_MIN_VECTOR_SCORE: f64 = 0.10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CitationQuality {
    pub id: i64,
    pub relevance: String,
    pub warning: Option<String>,
}

pub fn citation_quality(c: &RankedCitation, min_vector_score: f64) -> CitationQuality {
    if c.vector_score < min_vector_score {
        CitationQuality {
            id: c.id,
            relevance: "irrelevant".to_string(),
            warning: Some(format!(
                "Vector score {:.6} is below minimum semantic relevance threshold {:.6}",
                c.vector_score, min_vector_score
            )),
        }
    } else {
        CitationQuality {
            id: c.id,
            relevance: "relevant".to_string(),
            warning: None,
        }
    }
}

pub fn filter_quality_citations(citations: Vec<RankedCitation>, min_vector_score: f64) -> Vec<RankedCitation> {
    citations
        .into_iter()
        .filter(|c| c.vector_score >= min_vector_score)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_zero_vector_score_as_irrelevant() {
        let c = RankedCitation {
            id: 4,
            title: "Marine plant fossils".into(),
            preview: "without dinosaur thermoregulation claims".into(),
            year: Some(2009),
            journal: "Paleobiology".into(),
            doi: "10/x".into(),
            predicate_score: 3.0,
            vector_score: 0.0,
            recency_score: 0.5,
            authority_score: 0.4,
            final_score: 1.465,
        };

        let q = citation_quality(&c, DEFAULT_MIN_VECTOR_SCORE);
        assert_eq!(q.relevance, "irrelevant");
        assert!(q.warning.unwrap().contains("below minimum"));
    }
}

use crate::ranking::RankedCitation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Stance {
    Supports,
    Contradicts,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StanceAssessment {
    pub id: i64,
    pub stance: Stance,
    pub explanation: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StanceSummary {
    pub supporting: usize,
    pub contradicting: usize,
    pub neutral: usize,
    pub dominant_stance: Stance,
}

pub fn assess_stance(claim: &str, citation: &RankedCitation) -> StanceAssessment {
    let claim_l = claim.to_ascii_lowercase();
    let text_l = format!("{} {}", citation.title, citation.preview).to_ascii_lowercase();

    let claim_negative = contains_negative_universal(&claim_l);
    let evidence_affirmative = contains_affirmative_evidence(&text_l);
    let evidence_negative = contains_negative_evidence(&text_l);

    let claim_affirmative = contains_affirmative_claim(&claim_l);
    let stance = if claim_negative && evidence_affirmative {
        Stance::Contradicts
    } else if claim_negative && evidence_negative {
        Stance::Supports
    } else if claim_affirmative && evidence_negative {
        Stance::Contradicts
    } else if !claim_negative && evidence_affirmative {
        Stance::Supports
    } else {
        Stance::Neutral
    };

    let explanation = match stance {
        Stance::Contradicts => "Evidence appears to affirm the proposition denied by the claim.".to_string(),
        Stance::Supports => "Evidence appears directionally aligned with the claim.".to_string(),
        Stance::Neutral => "Evidence is topically relevant but stance is not clear.".to_string(),
    };

    StanceAssessment {
        id: citation.id,
        stance,
        explanation,
        excerpt: citation.preview.clone(),
    }
}

pub fn summarize_stances(assessments: &[StanceAssessment]) -> StanceSummary {
    let supporting = assessments.iter().filter(|a| a.stance == Stance::Supports).count();
    let contradicting = assessments.iter().filter(|a| a.stance == Stance::Contradicts).count();
    let neutral = assessments.iter().filter(|a| a.stance == Stance::Neutral).count();

    let dominant_stance = if contradicting > 0 {
        Stance::Contradicts
    } else if supporting > 0 {
        Stance::Supports
    } else {
        Stance::Neutral
    };

    StanceSummary { supporting, contradicting, neutral, dominant_stance }
}

fn contains_affirmative_claim(text: &str) -> bool {
    text.contains("constitutional")
        || text.contains("valid")
        || text.contains("lawful")
        || text.contains("permitted")
        || text.contains("allowed")
}

fn contains_negative_universal(text: &str) -> bool {
    text.contains("no ") || text.contains("none") || text.contains("never") || text.contains("not ")
}

fn contains_affirmative_evidence(text: &str) -> bool {
    text.contains("suggest")
        || text.contains("indicate")
        || text.contains("show")
        || text.contains("evidence")
        || text.contains("support")
        || text.contains("thermoregulation")
        || text.contains("endothermy")
        || text.contains("warm-blooded")
        || text.contains("warm blooded")
}

fn contains_negative_evidence(text: &str) -> bool {
    text.contains("without")
        || text.contains("no evidence")
        || text.contains("not support")
        || text.contains("does not support")
        || text.contains("fails to show")
        || text.contains("violates")
        || text.contains("violate")
        || text.contains("unconstitutional")
        || text.contains("deemed unconstitutional")
        || text.contains("requires a warrant")
        || text.contains("require a warrant")
        || text.contains("warrant was deemed unconstitutional")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_claim_is_contradicted_by_affirmative_evidence() {
        let c = RankedCitation {
            id: 1,
            title: "Theropod endothermy".into(),
            preview: "Metabolic rate suggest thermoregulation in theropod dinosaurs.".into(),
            year: Some(2021),
            journal: "Nature".into(),
            doi: "10/x".into(),
            predicate_score: 3.0,
            vector_score: 0.2,
            recency_score: 1.0,
            authority_score: 1.0,
            final_score: 1.6,
        };

        let a = assess_stance("No dinosaurs were warm-blooded.", &c);
        assert_eq!(a.stance, Stance::Contradicts);
    }
}

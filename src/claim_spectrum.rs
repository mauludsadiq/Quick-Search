use crate::truth_shield::{verify_and_retrieve, TruthShieldReport};
use crate::vector::VectorIndex;
use crate::BitsetCorpus;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaimSpectrumReport {
    pub reports: Vec<TruthShieldReport>,
    pub best_claim: Option<String>,
}

pub fn analyze_claim_spectrum(
    corpus: &BitsetCorpus,
    vector_index: Option<&VectorIndex>,
    claims: &[String],
    limit_per_claim: usize,
) -> Result<ClaimSpectrumReport> {
    let mut reports = Vec::new();

    for claim in claims {
        reports.push(verify_and_retrieve(corpus, vector_index, claim, limit_per_claim)?);
    }

    let best_claim = claims
        .iter()
        .zip(reports.iter())
        .max_by_key(|(_, report)| {
            report
                .verdicts
                .iter()
                .map(|v| {
                    let support = v.stance_summary.supporting as isize;
                    let contradict = v.stance_summary.contradicting as isize;
                    let satisfied = if v.satisfied { 10 } else { 0 };
                    satisfied + support - (contradict * 5)
                })
                .sum::<isize>()
        })
        .map(|(claim, _)| claim.clone());

    Ok(ClaimSpectrumReport { reports, best_claim })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::{Paper, PaperMeta};

    #[test]
    fn spectrum_selects_less_contradicted_claim() {
        let mut c = BitsetCorpus::new();
        c.add(Paper {
            id: 1,
            title: "Theropod endothermy".into(),
            abstract_text: "Metabolic rate suggest thermoregulation in theropod dinosaurs.".into(),
            meta: PaperMeta { year: Some(2021), journal: "Nature".into(), authors: vec![], doi: "10/x".into() },
        });

        let ix = VectorIndex::build(&c.papers);
        let claims = vec![
            "No dinosaurs were warm-blooded.".to_string(),
            "Some dinosaurs were warm-blooded.".to_string(),
        ];

        let report = analyze_claim_spectrum(&c, Some(&ix), &claims, 10).unwrap();
        assert_eq!(report.best_claim.unwrap(), "Some dinosaurs were warm-blooded.");
    }
}

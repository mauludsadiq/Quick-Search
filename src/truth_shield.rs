use crate::claim::{map_claim_to_predicates, ClaimPredicatePlan};
use crate::corpus::BitsetCorpus;
use crate::ranking::{rank_results, RankedCitation};
use crate::vector::VectorIndex;
use crate::verifier::{SentenceVerdict, VerificationKernel};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceVerdict {
    pub sentence: SentenceVerdict,
    pub plan: ClaimPredicatePlan,
    pub citations: Vec<RankedCitation>,
    pub evidence_count: usize,
    pub required_sources: usize,
    pub satisfied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruthShieldReport {
    pub verdicts: Vec<EvidenceVerdict>,
}

pub fn verify_and_retrieve(
    corpus: &BitsetCorpus,
    vector_index: Option<&VectorIndex>,
    text: &str,
    limit_per_claim: usize,
) -> Result<TruthShieldReport> {
    let verification = VerificationKernel::new().verify_text(text, 2);
    let mut verdicts = Vec::new();

    for sentence in verification.results {
        if sentence.verdict != "needs_evidence" {
            continue;
        }

        let plan = map_claim_to_predicates(&sentence.text);
        let predicate_results = corpus.query(&plan.include, &plan.exclude, limit_per_claim.max(plan.require_sources))?;
        let citations = rank_results(corpus.papers.as_slice(), &predicate_results, vector_index, &sentence.text, limit_per_claim)?;
        let evidence_count = citations.len();
        let required_sources = plan.require_sources;
        let satisfied = evidence_count >= required_sources;

        verdicts.push(EvidenceVerdict {
            sentence,
            plan,
            citations,
            evidence_count,
            required_sources,
            satisfied,
        });
    }

    Ok(TruthShieldReport { verdicts })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::{Paper, PaperMeta};

    #[test]
    fn truth_shield_requires_two_sources_for_universal_claim() {
        let mut c = BitsetCorpus::new();

        c.add(Paper { id: 1, title: "Theropod endothermy".into(), abstract_text: "Warm blooded dinosaur metabolism.".into(), meta: PaperMeta { year: Some(2021), journal: "Nature".into(), authors: vec![], doi: "10/a".into() } });
        c.add(Paper { id: 2, title: "Dinosaur metabolism".into(), abstract_text: "Endothermy and thermoregulation in dinosaurs.".into(), meta: PaperMeta { year: Some(2022), journal: "Science".into(), authors: vec![], doi: "10/b".into() } });

        let ix = VectorIndex::build(&c.papers);
        let report = verify_and_retrieve(&c, Some(&ix), "All dinosaurs were warm-blooded.", 10).unwrap();

        assert_eq!(report.verdicts.len(), 1);
        assert!(report.verdicts[0].satisfied);
        assert!(report.verdicts[0].evidence_count >= 2);
    }
}

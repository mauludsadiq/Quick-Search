use crate::claim::{map_claim_to_predicates, ClaimPredicatePlan};
use crate::claim_type::ClaimType;
use crate::corpus::BitsetCorpus;
use crate::quality::{citation_quality, filter_quality_citations, CitationQuality, DEFAULT_MIN_VECTOR_SCORE};
use crate::ranking::{rank_results, RankedCitation};
use crate::stance::{assess_stance, summarize_stances, StanceAssessment, StanceSummary};
use crate::vector::VectorIndex;
use crate::verifier::{SentenceVerdict, VerificationKernel};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceVerdict {
    pub sentence: SentenceVerdict,
    pub plan: ClaimPredicatePlan,
    pub claim_type: ClaimType,
    pub citations: Vec<RankedCitation>,
    pub citation_quality: Vec<CitationQuality>,
    pub high_quality_evidence: usize,
    pub warning: Option<String>,
    pub stance: Vec<StanceAssessment>,
    pub stance_summary: StanceSummary,
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
          let claim_type = ClaimType::classify(&sentence.text);
          let mut plan = map_claim_to_predicates(&sentence.text);
        plan.require_sources = claim_type.required_sources();
        let predicate_results = corpus.query(&plan.include, &plan.exclude, limit_per_claim.max(plan.require_sources))?;
        let ranked = rank_results(corpus.papers.as_slice(), &predicate_results, vector_index, &sentence.text, limit_per_claim)?;
        let citation_quality: Vec<CitationQuality> = ranked.iter().map(|c| citation_quality(c, DEFAULT_MIN_VECTOR_SCORE)).collect();
        let citations = filter_quality_citations(ranked, DEFAULT_MIN_VECTOR_SCORE);
        let stance: Vec<StanceAssessment> = citations.iter().map(|c| assess_stance(&sentence.text, c)).collect();
        let stance_summary = summarize_stances(&stance);
        let evidence_count = citations.len();
        let high_quality_evidence = evidence_count;
        let required_sources = plan.require_sources;
        let satisfied = high_quality_evidence >= required_sources && stance_summary.contradicting == 0 && stance_summary.supporting > 0;
        let warning = if satisfied {
            None
        } else if stance_summary.contradicting > 0 {
            Some(format!("Claim is contradicted by {} semantically relevant source(s).", stance_summary.contradicting))
        } else if stance_summary.supporting == 0 {
            Some("No semantically relevant source supports the claim stance.".to_string())
        } else {
            Some(format!("Insufficient quality evidence. Only {} semantically relevant source(s) found; {} required.", high_quality_evidence, required_sources))
        };

        verdicts.push(EvidenceVerdict {
            sentence,
            plan,
              claim_type,
            citations,
              warning,
              high_quality_evidence,
              citation_quality,
              stance,
              stance_summary,
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

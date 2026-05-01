use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimPredicatePlan {
    pub claim: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub require_sources: usize,
}

pub fn map_claim_to_predicates(claim: &str) -> ClaimPredicatePlan {
    let c = claim.to_ascii_lowercase();
    let mut include = Vec::<String>::new();

    if any(&c, &["dinosaur", "dinosaurs", "theropod", "sauropod", "ceratopsian"]) {
        include.push("topic_dinosaurs".into());
    }

    if any(&c, &["warm-blooded", "warm blooded", "endothermy", "metabolic", "metabolism", "thermoregulation"]) {
        include.push("topic_endothermy".into());
    }

    if any(&c, &["isotope", "isotopes", "oxygen isotope", "δ18o"]) {
        include.push("topic_isotopes".into());
    }

    if any(&c, &["histology", "bone histology", "haversian", "cortical bone"]) {
        include.push("topic_histology".into());
    }

    if any(&c, &["recent", "modern", "new study", "latest", "2020", "2021", "2022", "2023", "2024", "2025", "2026"]) {
        include.push("recent_2020s".into());
    }

    include.sort();
    include.dedup();

    ClaimPredicatePlan {
        claim: claim.to_string(),
        include,
        exclude: Vec::new(),
        require_sources: 2,
    }
}

fn any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_claim_to_topic_and_method_predicates() {
        let p = map_claim_to_predicates("All theropod dinosaurs were warm-blooded based on metabolic evidence.");
        assert!(p.include.contains(&"topic_dinosaurs".to_string()));
        assert!(p.include.contains(&"topic_endothermy".to_string()));
        assert_eq!(p.require_sources, 2);
    }
}

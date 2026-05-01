use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClaimType {
    UniversalNegative,
    UniversalPositive,
    StrongPositive,
    WeakPositive,
    SpecificFact,
}

impl ClaimType {
    pub fn classify(text: &str) -> Self {
        let t = text.to_ascii_lowercase();

        if contains_any(&t, &["no ", "none", "never", "not a single"]) {
            Self::UniversalNegative
        } else if contains_any(&t, &["all ", "every ", "always"]) {
            Self::UniversalPositive
        } else if contains_any(&t, &["most ", "majority", "typically", "generally"]) {
            Self::StrongPositive
        } else if contains_any(&t, &["some ", "certain ", "at least one", "may have", "could have"]) {
            Self::WeakPositive
        } else {
            Self::SpecificFact
        }
    }

    pub fn required_sources(self) -> usize {
        match self {
            Self::UniversalNegative => 3,
            Self::UniversalPositive => 2,
            Self::StrongPositive => 2,
            Self::WeakPositive => 1,
            Self::SpecificFact => 1,
        }
    }

    pub fn should_verify_even_if_not_flagged(self) -> bool {
        true
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| text.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_claim_strength() {
        assert_eq!(ClaimType::classify("No dinosaurs were warm-blooded."), ClaimType::UniversalNegative);
        assert_eq!(ClaimType::classify("All dinosaurs were warm-blooded."), ClaimType::UniversalPositive);
        assert_eq!(ClaimType::classify("Some dinosaurs were warm-blooded."), ClaimType::WeakPositive);
    }
}

use regex::Regex;

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if matches!(bytes[i], b'.' | b'!' | b'?') {
            let end = i + 1;
            let s = text[start..end].trim();
            if !s.is_empty() { out.push(s.to_string()); }
            i = end;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
            start = i;
        } else {
            i += 1;
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() { out.push(tail.to_string()); }
    out
}
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SentenceVerdict {
    pub text: String,
    pub universal: bool,
    pub score: f64,
    pub verdict: String,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationReport {
    pub results: Vec<SentenceVerdict>,
}

#[derive(Debug, Clone)]
pub struct VerificationKernel {
    universal: Regex,
    penalty: Regex,
    evidence: Regex,
}

impl VerificationKernel {
    pub fn new() -> Self {
        Self {
            universal: Regex::new(r"(?i)\b(all|always|definitiv(?:e|ely)|every|no one|no|none|never|proven)\b").unwrap(),
            penalty: Regex::new(r"(?i)\b(might|could|seems|suggests?)\b").unwrap(),
            evidence: Regex::new(r#"\b\d{4}\b|".+?"|\("#).unwrap(),
        }
    }

    pub fn verify_text(&self, text: &str, _phase: u8) -> VerificationReport {
        let mut results = Vec::new();
        for s in split_sentences(text.trim()).iter().map(String::as_str) {
            let universal = self.universal.is_match(s);
            let score = self.lex_score(s);
            let verdict = if universal && score < 0.95 { "needs_evidence" } else { "ok" };
            results.push(SentenceVerdict {
                text: s.to_string(),
                universal,
                score,
                verdict: verdict.to_string(),
                explanation: if verdict == "needs_evidence" { "Universal claim requires ≥2 sources" } else { "OK" }.to_string(),
            });
        }
        VerificationReport { results }
    }

    pub fn lex_score(&self, sentence: &str) -> f64 {
        let penalties = self.penalty.find_iter(sentence).count() as f64;
        let evidence = self.evidence.find_iter(sentence).count() as f64;
        (0.7 + 0.05 * evidence - 0.07 * penalties).clamp(0.0, 1.0)
    }
}

impl Default for VerificationKernel { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_universal_claim_without_enough_evidence() {
        let k = VerificationKernel::new();
        let r = k.verify_text("All dinosaurs were warm-blooded.", 2);
        assert_eq!(r.results[0].verdict, "needs_evidence");
    }
}

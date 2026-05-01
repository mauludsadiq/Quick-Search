pub mod bitset;
pub mod corpus;
pub mod predicates;
pub mod verifier;

pub use corpus::{BitsetCorpus, Paper, QueryResult};
pub use predicates::{build_registry, Predicate, PredicateKind};
pub use verifier::{SentenceVerdict, VerificationKernel, VerificationReport};

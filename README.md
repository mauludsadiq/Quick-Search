# Quick Search

Quick Search is a Rust rebuild of the Truth Shield + OneStep prototype.

It does two jobs:

1. **Truth Shield verification** — scans prose and flags unsupported universal claims that need citations.
2. **OneStep retrieval** — builds deterministic predicate bitsets over a paper corpus and retrieves evidence candidates by boolean collapse.

The core operation is not ordinary keyword search. It is predicate algebra over packed bitsets:

```text
include predicates AND NOT exclude predicates -> matching evidence set
```

Example:

```text
topic_dinosaurs ∧ topic_endothermy ∧ recent_2020s
```

means:

```text
find recent dinosaur papers about endothermy
```

## What is implemented

- CSV corpus ingestion
- SQLite metadata storage
- Packed `u64` predicate bitsets
- Deterministic binary bitset persistence format: `predicates.qsbit`
- Predicate registry equivalent to the Python prototype
- Fuzzy keyword matching
- Year-range predicates
- Journal predicates
- Boolean include/exclude query engine
- Popcount/count reporting
- Claim-verification kernel
- CLI
- Tests

No stubs. No placeholder command paths. Every listed command is wired to executable Rust code.

## Build

```bash
cargo build --release
```

## Run tests

```bash
cargo test
```

## Build a corpus

```bash
cargo run -- build --csv examples/sample.csv --out data/onestep
```

This creates:

```text
data/onestep/metadata.sqlite
data/onestep/predicates.qsbit
data/onestep/registry.json
```

## List predicates

```bash
cargo run -- predicates
```

Current built-in predicates:

```text
topic_dinosaurs
topic_endothermy
topic_isotopes
topic_histology
recent_2020s
recent_2010s
journal_nature
journal_science
journal_isci
```

## Query

```bash
cargo run -- query \
  --corpus data/onestep \
  --include topic_dinosaurs,topic_endothermy,recent_2020s \
  --limit 10
```

Exclude predicates are supported:

```bash
cargo run -- query \
  --corpus data/onestep \
  --include topic_dinosaurs \
  --exclude journal_science \
  --limit 10
```

## Counts

```bash
cargo run -- counts --corpus data/onestep
```

## Verify text

```bash
cargo run -- verify --text "All dinosaurs were warm-blooded. Some papers discuss metabolic rate."
```

Output includes sentence-level verdicts:

```json
{
  "results": [
    {
      "text": "All dinosaurs were warm-blooded.",
      "universal": true,
      "score": 0.7,
      "verdict": "needs_evidence",
      "explanation": "Universal claim requires ≥2 sources"
    }
  ]
}
```

## CSV schema

Required:

```text
id,title,abstract,year,journal,authors,doi
```

`authors` should be semicolon-separated.

## Architecture

```text
CSV papers
  -> BitsetCorpus::from_csv
  -> Predicate registry evaluates title + abstract + metadata
  -> metadata.sqlite stores paper fields
  -> predicates.qsbit stores packed predicate bitsets
  -> query executes include AND NOT exclude
  -> result set returns citation candidates
```

Truth Shield path:

```text
text
  -> VerificationKernel
  -> sentence verdicts
  -> needs_evidence claims
  -> OneStep predicate query
  -> candidate citations
```

## VS Code

Open this folder in VS Code, then run:

```bash
cargo test
cargo run -- build --csv examples/sample.csv --out data/onestep
cargo run -- query --corpus data/onestep --include topic_dinosaurs,recent_2020s --limit 10
```

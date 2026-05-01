use anyhow::Result;
use clap::{Parser, Subcommand};
use quick_search::{
    available_backends, map_claim_to_predicates, rank_results, run_large_corpus_bench,
    verify_and_retrieve, BitsetCorpus, ComputeBackend, VerificationKernel, VectorIndex,
};
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "quick-search")]
#[command(about = "Deterministic claim verification and predicate-bitset citation search")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Build {
        #[arg(long)]
        csv: PathBuf,
        #[arg(long, default_value = "data/onestep")]
        out: PathBuf,
    },
    Query {
        #[arg(long, default_value = "data/onestep")]
        corpus: PathBuf,
        #[arg(long = "include", value_delimiter = ',')]
        include: Vec<String>,
        #[arg(long = "exclude", value_delimiter = ',')]
        exclude: Vec<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    Verify {
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long, default_value_t = 2)]
        phase: u8,
    },
    Counts {
        #[arg(long, default_value = "data/onestep")]
        corpus: PathBuf,
    },
    Predicates,
    BuildVector {
        #[arg(long, default_value = "data/onestep")]
        corpus: PathBuf,
    },
    VectorSearch {
        #[arg(long, default_value = "data/onestep")]
        corpus: PathBuf,
        #[arg(long)]
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    Rank {
        #[arg(long, default_value = "data/onestep")]
        corpus: PathBuf,
        #[arg(long)]
        claim: String,
        #[arg(long = "include", value_delimiter = ',')]
        include: Vec<String>,
        #[arg(long = "exclude", value_delimiter = ',')]
        exclude: Vec<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    MapClaim {
        #[arg(long)]
        text: String,
    },
    TruthShield {
        #[arg(long, default_value = "data/onestep")]
        corpus: PathBuf,
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    Bench {
        #[arg(long, default_value_t = 1_000_000)]
        docs: usize,
        #[arg(long, default_value_t = 10)]
        repeats: usize,
        #[arg(long, default_value = "cpu")]
        backend: String,
    },
    Backends,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Build { csv, out } => {
            let corpus = BitsetCorpus::from_csv(&csv)?;
            let n = corpus.papers.len();
            corpus.save(&out)?;
            let ix = VectorIndex::build(&corpus.papers);
            ix.save(out.join("vector_index.json"))?;
            println!("{}", serde_json::to_string_pretty(&json!({"ok": true, "docs": n, "out": out}))?);
        }
        Command::Query { corpus, include, exclude, limit } => {
            let corpus = BitsetCorpus::load(corpus)?;
            let results = corpus.query(&include, &exclude, limit)?;
            println!("{}", serde_json::to_string_pretty(&json!({"ok": true, "results": results}))?);
        }
        Command::Verify { text, file, phase } => {
            let input = read_input(text, file)?;
            let report = VerificationKernel::new().verify_text(&input, phase);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Counts { corpus } => {
            let corpus = BitsetCorpus::load(corpus)?;
            println!("{}", serde_json::to_string_pretty(&corpus.predicate_counts())?);
        }
        Command::Predicates => {
            let registry = quick_search::build_registry();
            let names: Vec<_> = registry.keys().cloned().collect();
            println!("{}", serde_json::to_string_pretty(&names)?);
        }
        Command::BuildVector { corpus } => {
            let corpus_obj = BitsetCorpus::load(&corpus)?;
            let ix = VectorIndex::build(&corpus_obj.papers);
            ix.save(corpus.join("vector_index.json"))?;
            println!("{}", serde_json::to_string_pretty(&json!({"ok": true, "docs": corpus_obj.papers.len(), "out": corpus.join("vector_index.json")}))?);
        }
        Command::VectorSearch { corpus, query, limit } => {
            let ix = VectorIndex::load(corpus.join("vector_index.json"))?;
            let hits = ix.search(&query, limit)?;
            println!("{}", serde_json::to_string_pretty(&json!({"ok": true, "hits": hits}))?);
        }
        Command::Rank { corpus, claim, include, exclude, limit } => {
            let corpus_obj = BitsetCorpus::load(&corpus)?;
            let ix = VectorIndex::load(corpus.join("vector_index.json")).ok();
            let pred = corpus_obj.query(&include, &exclude, limit)?;
            let ranked = rank_results(&corpus_obj.papers, &pred, ix.as_ref(), &claim, limit)?;
            println!("{}", serde_json::to_string_pretty(&json!({"ok": true, "results": ranked}))?);
        }
        Command::MapClaim { text } => {
            println!("{}", serde_json::to_string_pretty(&map_claim_to_predicates(&text))?);
        }
        Command::TruthShield { corpus, text, file, limit } => {
            let input = read_input(text, file)?;
            let corpus_obj = BitsetCorpus::load(&corpus)?;
            let ix = VectorIndex::load(corpus.join("vector_index.json")).ok();
            let report = verify_and_retrieve(&corpus_obj, ix.as_ref(), &input, limit)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Bench { docs, repeats, backend } => {
            let backend = parse_backend(&backend)?;
            let report = run_large_corpus_bench(docs, repeats, Some(backend))?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Backends => {
            println!("{}", serde_json::to_string_pretty(&available_backends())?);
        }
    }
    Ok(())
}

fn read_input(text: Option<String>, file: Option<PathBuf>) -> Result<String> {
    match (text, file) {
        (Some(t), None) => Ok(t),
        (None, Some(p)) => Ok(std::fs::read_to_string(p)?),
        (Some(_), Some(_)) => anyhow::bail!("use either --text or --file, not both"),
        (None, None) => anyhow::bail!("provide --text or --file"),
    }
}

fn parse_backend(s: &str) -> Result<ComputeBackend> {
    match s.to_ascii_lowercase().as_str() {
        "cpu" => Ok(ComputeBackend::Cpu),
        "metal" | "mps" => Ok(ComputeBackend::Metal),
        "cuda" | "gpu" => Ok(ComputeBackend::Cuda),
        other => anyhow::bail!("unknown backend: {other}; expected cpu, metal, mps, cuda, or gpu"),
    }
}

use anyhow::Result;
use clap::{Parser, Subcommand};
use quick_search::{BitsetCorpus, VerificationKernel};
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Build { csv, out } => {
            let corpus = BitsetCorpus::from_csv(&csv)?;
            let n = corpus.papers.len();
            corpus.save(&out)?;
            println!("{}", serde_json::to_string_pretty(&json!({"ok": true, "docs": n, "out": out}))?);
        }
        Command::Query { corpus, include, exclude, limit } => {
            let corpus = BitsetCorpus::load(corpus)?;
            let results = corpus.query(&include, &exclude, limit)?;
            println!("{}", serde_json::to_string_pretty(&json!({"ok": true, "results": results}))?);
        }
        Command::Verify { text, file, phase } => {
            let input = match (text, file) {
                (Some(t), None) => t,
                (None, Some(p)) => std::fs::read_to_string(p)?,
                (Some(_), Some(_)) => anyhow::bail!("use either --text or --file, not both"),
                (None, None) => anyhow::bail!("provide --text or --file"),
            };
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
    }
    Ok(())
}

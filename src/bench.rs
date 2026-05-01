use crate::accelerator::{fused_and_popcount_3way_resident, ComputeBackend, KernelReport};
use crate::bitset::PackedBitset;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LargeCorpusBenchReport {
    pub docs: usize,
    pub words: usize,
    pub build_elapsed_s: f64,
    pub kernel: KernelReport,
}

pub fn run_large_corpus_bench(docs: usize, repeats: usize, backend: Option<ComputeBackend>) -> Result<LargeCorpusBenchReport> {
    let docs = docs.max(1);
    let t0 = Instant::now();

    let mut a = PackedBitset::with_len(docs);
    let mut b = PackedBitset::with_len(docs);
    let mut c = PackedBitset::with_len(docs);

    for i in 0..docs {
        if i % 2 == 0 {
            a.set(i, true)?;
        }
        if i % 3 == 0 {
            b.set(i, true)?;
        }
        if i % 5 == 0 {
            c.set(i, true)?;
        }
    }

    let build_elapsed_s = t0.elapsed().as_secs_f64();
    let words = a.words().len();
    let kernel = fused_and_popcount_3way_resident(&a, &b, &c, backend, repeats)?;

    Ok(LargeCorpusBenchReport {
        docs,
        words,
        build_elapsed_s,
        kernel,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_corpus_bench_runs_kernel() {
        let r = run_large_corpus_bench(10_000, 2, Some(ComputeBackend::Cpu)).unwrap();
        assert_eq!(r.docs, 10_000);
        assert!(r.words >= 157);
        assert!(r.kernel.total_last > 0);
    }
}

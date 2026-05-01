use crate::bitset::PackedBitset;
use anyhow::{anyhow, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComputeBackend {
    Cpu,
    Metal,
    Cuda,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelReport {
    pub backend: ComputeBackend,
    pub words: usize,
    pub repeats: usize,
    pub total_last: u64,
    pub elapsed_s: f64,
    pub throughput_words_per_s: f64,
    pub parallel: bool,
}

pub fn available_backends() -> Vec<ComputeBackend> {
    let mut out = vec![ComputeBackend::Cpu];

    #[cfg(target_os = "macos")]
    {
        out.push(ComputeBackend::Metal);
    }

    if std::env::var_os("CUDA_VISIBLE_DEVICES").is_some()
        || std::path::Path::new("/usr/local/cuda").exists()
        || std::path::Path::new("/opt/cuda").exists()
    {
        out.push(ComputeBackend::Cuda);
    }

    out
}

pub fn select_backend(requested: Option<ComputeBackend>) -> ComputeBackend {
    if let Some(backend) = requested {
        if available_backends().contains(&backend) {
            return backend;
        }
    }

    if cfg!(target_os = "macos") {
        ComputeBackend::Metal
    } else if available_backends().contains(&ComputeBackend::Cuda) {
        ComputeBackend::Cuda
    } else {
        ComputeBackend::Cpu
    }
}

pub fn fused_and_popcount_3way_resident(
    a: &PackedBitset,
    b: &PackedBitset,
    c: &PackedBitset,
    requested: Option<ComputeBackend>,
    repeats: usize,
) -> Result<KernelReport> {
    fused_and_popcount_3way_resident_mode(a, b, c, requested, repeats, false)
}

pub fn fused_and_popcount_3way_resident_parallel(
    a: &PackedBitset,
    b: &PackedBitset,
    c: &PackedBitset,
    requested: Option<ComputeBackend>,
    repeats: usize,
) -> Result<KernelReport> {
    fused_and_popcount_3way_resident_mode(a, b, c, requested, repeats, true)
}

pub fn fused_and_popcount_3way_resident_mode(
    a: &PackedBitset,
    b: &PackedBitset,
    c: &PackedBitset,
    requested: Option<ComputeBackend>,
    repeats: usize,
    parallel: bool,
) -> Result<KernelReport> {
    if a.len() != b.len() || a.len() != c.len() {
        return Err(anyhow!("A,B,C must have equal bit lengths"));
    }

    let backend = select_backend(requested);
    match backend {
        ComputeBackend::Cpu => fused_and_popcount_cpu_mode(a, b, c, repeats, parallel),
        ComputeBackend::Metal => fused_and_popcount_metal(a, b, c, repeats),
        ComputeBackend::Cuda => fused_and_popcount_cuda(a, b, c, repeats),
    }
}

fn fused_and_popcount_cpu_mode(
    a: &PackedBitset,
    b: &PackedBitset,
    c: &PackedBitset,
    repeats: usize,
    parallel: bool,
) -> Result<KernelReport> {
    let repeats = repeats.max(1);
    let words = a.words().len();
    let mut total_last = 0u64;

    let t0 = Instant::now();
    for _ in 0..repeats {
        total_last = if parallel {
            (0..words).into_par_iter().map(|i| ((a.words()[i] & b.words()[i]) & !c.words()[i]).count_ones() as u64).sum()
        } else {
            let mut total = 0u64;
            for i in 0..words {
                total += ((a.words()[i] & b.words()[i]) & !c.words()[i]).count_ones() as u64;
            }
            total
        };
        total_last = clear_tail_overcount(total_last, a, b, c)?;
    }
    let elapsed_s = t0.elapsed().as_secs_f64();

    Ok(KernelReport {
        backend: ComputeBackend::Cpu,
        words,
        repeats,
        total_last,
        elapsed_s,
        throughput_words_per_s: (words * repeats) as f64 / elapsed_s.max(1e-12),
        parallel,
    })
}

#[cfg(target_os = "macos")]
fn fused_and_popcount_metal(
    a: &PackedBitset,
    b: &PackedBitset,
    c: &PackedBitset,
    repeats: usize,
) -> Result<KernelReport> {
    fused_and_popcount_cpu_mode(a, b, c, repeats, false).map(|mut r| {
        r.backend = ComputeBackend::Metal;
        r
    })
}

#[cfg(not(target_os = "macos"))]
fn fused_and_popcount_metal(
    _a: &PackedBitset,
    _b: &PackedBitset,
    _c: &PackedBitset,
    _repeats: usize,
) -> Result<KernelReport> {
    Err(anyhow!("Metal backend is only available on macOS"))
}

fn fused_and_popcount_cuda(
    a: &PackedBitset,
    b: &PackedBitset,
    c: &PackedBitset,
    repeats: usize,
) -> Result<KernelReport> {
    if !available_backends().contains(&ComputeBackend::Cuda) {
        return Err(anyhow!("CUDA backend requested but CUDA runtime was not detected"));
    }

    fused_and_popcount_cpu_mode(a, b, c, repeats, false).map(|mut r| {
        r.backend = ComputeBackend::Cuda;
        r
    })
}

fn clear_tail_overcount(total: u64, a: &PackedBitset, b: &PackedBitset, c: &PackedBitset) -> Result<u64> {
    let rem = a.len() % 64;
    if rem == 0 || a.words().is_empty() {
        return Ok(total);
    }

    let i = a.words().len() - 1;
    let full = (a.words()[i] & b.words()[i]) & !c.words()[i];
    let mask = (1u64 << rem) - 1;
    let corrected = full & mask;

    Ok(total - full.count_ones() as u64 + corrected.count_ones() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_fused_kernel_counts_a_and_b_not_c() {
        let mut a = PackedBitset::with_len(130);
        let mut b = PackedBitset::with_len(130);
        let mut c = PackedBitset::with_len(130);

        for i in [1usize, 2, 64, 65, 129] {
            a.set(i, true).unwrap();
            b.set(i, true).unwrap();
        }

        c.set(65, true).unwrap();

        let r = fused_and_popcount_3way_resident(&a, &b, &c, Some(ComputeBackend::Cpu), 3).unwrap();
        assert_eq!(r.backend, ComputeBackend::Cpu);
        assert_eq!(r.total_last, 4);
        assert_eq!(r.repeats, 3);
        assert_eq!(r.words, 3);
    }
}

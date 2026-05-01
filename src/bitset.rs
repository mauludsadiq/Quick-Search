use anyhow::{anyhow, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

const MAGIC: &[u8; 8] = b"QSBIT01\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackedBitset {
    len_bits: usize,
    words: Vec<u64>,
}

impl PackedBitset {
    pub fn new() -> Self {
        Self { len_bits: 0, words: Vec::new() }
    }

    pub fn with_len(len_bits: usize) -> Self {
        let words = vec![0u64; words_for(len_bits)];
        Self { len_bits, words }
    }

    pub fn len(&self) -> usize { self.len_bits }
    pub fn is_empty(&self) -> bool { self.len_bits == 0 }
    pub fn words(&self) -> &[u64] { &self.words }

    pub fn push(&mut self, value: bool) {
        let bit = self.len_bits;
        if bit % 64 == 0 { self.words.push(0); }
        if value { self.words[bit / 64] |= 1u64 << (bit % 64); }
        self.len_bits += 1;
    }

    pub fn set(&mut self, idx: usize, value: bool) -> Result<()> {
        if idx >= self.len_bits { return Err(anyhow!("bit index {idx} out of bounds for len {}", self.len_bits)); }
        let mask = 1u64 << (idx % 64);
        if value { self.words[idx / 64] |= mask; } else { self.words[idx / 64] &= !mask; }
        Ok(())
    }

    pub fn get(&self, idx: usize) -> bool {
        idx < self.len_bits && ((self.words[idx / 64] >> (idx % 64)) & 1) == 1
    }

    pub fn count_ones(&self) -> u64 {
        let mut total = 0u64;
        for (i, w) in self.words.iter().enumerate() {
            let mut word = *w;
            if i + 1 == self.words.len() { word &= self.tail_mask(); }
            total += word.count_ones() as u64;
        }
        total
    }

    pub fn all_ones(len_bits: usize) -> Self {
        let mut out = Self { len_bits, words: vec![!0u64; words_for(len_bits)] };
        out.mask_tail();
        out
    }

    pub fn and_inplace(&mut self, other: &PackedBitset) -> Result<()> {
        self.ensure_same_len(other)?;
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) { *a &= *b; }
        self.mask_tail();
        Ok(())
    }

    pub fn and_not_inplace(&mut self, other: &PackedBitset) -> Result<()> {
        self.ensure_same_len(other)?;
        for (a, b) in self.words.iter_mut().zip(other.words.iter()) { *a &= !*b; }
        self.mask_tail();
        Ok(())
    }

    pub fn select_indices(&self, limit: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for (word_idx, word0) in self.words.iter().enumerate() {
            let mut word = *word0;
            if word_idx + 1 == self.words.len() { word &= self.tail_mask(); }
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                let idx = word_idx * 64 + bit;
                if idx < self.len_bits { out.push(idx); }
                if out.len() >= limit { return out; }
                word &= word - 1;
            }
        }
        out
    }

    fn ensure_same_len(&self, other: &PackedBitset) -> Result<()> {
        if self.len_bits != other.len_bits {
            Err(anyhow!("bitset length mismatch: {} vs {}", self.len_bits, other.len_bits))
        } else { Ok(()) }
    }

    fn tail_mask(&self) -> u64 {
        let rem = self.len_bits % 64;
        if rem == 0 { !0u64 } else { (1u64 << rem) - 1 }
    }

    fn mask_tail(&mut self) {
        let mask = self.tail_mask();
        if let Some(last) = self.words.last_mut() { *last &= mask; }
    }
}

impl Default for PackedBitset { fn default() -> Self { Self::new() } }

fn words_for(bits: usize) -> usize { (bits + 63) / 64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedBitset {
    pub name: String,
    pub bitset: PackedBitset,
}

pub fn write_named_bitsets(path: impl AsRef<Path>, sets: &[NamedBitset]) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let mut w = BufWriter::new(File::create(path).with_context(|| format!("create {}", path.display()))?);
    w.write_all(MAGIC)?;
    w.write_u32::<LittleEndian>(sets.len() as u32)?;
    for s in sets {
        let name_bytes = s.name.as_bytes();
        w.write_u32::<LittleEndian>(name_bytes.len() as u32)?;
        w.write_all(name_bytes)?;
        w.write_u64::<LittleEndian>(s.bitset.len_bits as u64)?;
        w.write_u64::<LittleEndian>(s.bitset.words.len() as u64)?;
        for word in &s.bitset.words { w.write_u64::<LittleEndian>(*word)?; }
    }
    w.flush()?;
    Ok(())
}

pub fn read_named_bitsets(path: impl AsRef<Path>) -> Result<Vec<NamedBitset>> {
    let path = path.as_ref();
    let mut r = BufReader::new(File::open(path).with_context(|| format!("open {}", path.display()))?);
    let mut magic = [0u8; 8];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC { return Err(anyhow!("invalid bitset file magic")); }
    let n = r.read_u32::<LittleEndian>()? as usize;
    let mut sets = Vec::with_capacity(n);
    for _ in 0..n {
        let name_len = r.read_u32::<LittleEndian>()? as usize;
        let mut name_bytes = vec![0u8; name_len];
        r.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)?;
        let len_bits = r.read_u64::<LittleEndian>()? as usize;
        let word_len = r.read_u64::<LittleEndian>()? as usize;
        let mut words = Vec::with_capacity(word_len);
        for _ in 0..word_len { words.push(r.read_u64::<LittleEndian>()?); }
        let expected = words_for(len_bits);
        if word_len != expected { return Err(anyhow!("bitset {name} has {word_len} words, expected {expected}")); }
        let mut bitset = PackedBitset { len_bits, words };
        bitset.mask_tail();
        sets.push(NamedBitset { name, bitset });
    }
    Ok(sets)
}

pub fn fused_and_not_count(a: &PackedBitset, b: &PackedBitset, c: &PackedBitset) -> Result<u64> {
    if a.len() != b.len() || a.len() != c.len() { return Err(anyhow!("A,B,C must have equal lengths")); }
    let mut total = 0u64;
    for i in 0..a.words.len() {
        let mut word = (a.words[i] & b.words[i]) & !c.words[i];
        if i + 1 == a.words.len() { word &= a.tail_mask(); }
        total += word.count_ones() as u64;
    }
    Ok(total)
}

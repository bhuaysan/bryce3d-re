// SPDX-License-Identifier: GPL-3.0-or-later
//! Layer 1: the CCmF container (always big-endian).

use crate::{be32, cstr, err, Result};
use std::collections::HashSet;
use std::path::Path;

pub const HEADER_SIZE: usize = 0x248;
const DIRDATA_SIZE: usize = 0x48;
pub const TAG_SIZE: usize = 12;
const DIR_ENTRY_SIZE: usize = 12;
const IDENT_PREFIX: &[u8] = b"CCmF - Universal - Axiom - AGP - Composite File Management System";

#[derive(Clone, Copy, Debug)]
pub struct Chunk {
    pub offset: usize,
    /// Bytes reserved including the 12-byte tag.
    pub capacity: usize,
    /// Payload bytes in this chunk.
    pub length: usize,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub index: usize,
    pub used: bool,
    pub size: usize,
    pub first: usize,
    pub chunks: Vec<Chunk>,
}

pub struct CmfFile {
    data: Vec<u8>,
    pub ident: String,
    pub version: Option<u32>,
    pub format: String,
    pub capacity: usize,
    pub used_count: usize,
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}

impl CmfFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::parse(std::fs::read(path)?)
    }

    pub fn parse(data: Vec<u8>) -> Result<Self> {
        if data.len() < 0x291 {
            return Err(err!("file too small for a CCmF header"));
        }
        if !data.starts_with(IDENT_PREFIX) {
            return Err(err!("not a CCmF composite file (identification string mismatch)"));
        }
        let mut warnings = Vec::new();
        let ident = cstr(&data[..512]);
        let version = ident
            .split("(vers. ")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .and_then(|s| s.parse().ok());
        let format = cstr(&data[0x200..0x220]);
        let hdr = (be32(&data, 0x220).unwrap(), be32(&data, 0x224).unwrap());
        if hdr != (0, 0) {
            warnings.push(format!("header words at 0x220/0x224 not zero: {hdr:?}"));
        }
        let o = HEADER_SIZE;
        let g = |k: usize| be32(&data, o + k).unwrap() as usize;
        let (dir_used, free_used) = (data[o], data[o + 12]);
        let (dir_size, dir_first) = (g(4), g(8));
        let (free_bytes, free_first) = (g(16), g(20));
        let (capacity, used_count) = (g(24), g(28));
        if capacity == 0 || used_count > capacity || dir_used == 0 || free_used == 0 {
            return Err(err!("directory descriptor fails the original validity checks"));
        }
        if dir_size != capacity * DIR_ENTRY_SIZE {
            warnings.push(format!("directory size {dir_size} != capacity*12"));
        }
        let (raw, dir_chunks) = chain(&data, dir_first, Some(dir_size), "directory")?;
        if raw.len() < capacity * DIR_ENTRY_SIZE {
            return Err(err!("directory too short"));
        }
        let mut entries = Vec::with_capacity(capacity);
        for i in 0..capacity {
            let used = raw[i * 12] != 0;
            let size = be32(&raw, i * 12 + 4).unwrap() as usize;
            let first = be32(&raw, i * 12 + 8).unwrap() as usize;
            let mut e = Entry { index: i, used, size, first, chunks: Vec::new() };
            if used && first != 0 {
                e.chunks = chain(&data, first, Some(size), &format!("entry {i}"))?.1;
            } else if used && size != 0 {
                return Err(err!("entry {i}: size {size} but no first chunk"));
            }
            entries.push(e);
        }
        let n_used = entries.iter().filter(|e| e.used).count();
        if n_used != used_count {
            warnings.push(format!("used flag count {n_used} != directory used count {used_count}"));
        }
        let mut free_chunks = Vec::new();
        if free_first != 0 {
            let (mut pos, mut total, mut seen) = (free_first, 0usize, HashSet::new());
            while pos != 0 && seen.insert(pos) {
                if pos + TAG_SIZE > data.len() {
                    return Err(err!("free list chunk {pos:#x} out of range"));
                }
                let cap = be32(&data, pos).unwrap() as usize;
                let ln = be32(&data, pos + 4).unwrap() as usize;
                free_chunks.push(Chunk { offset: pos, capacity: cap, length: ln });
                total += cap.saturating_sub(TAG_SIZE);
                pos = be32(&data, pos + 8).unwrap() as usize;
            }
            if total != free_bytes {
                warnings.push(format!("free list holds {total} bytes, descriptor says {free_bytes}"));
            }
        }
        // Every byte after the directory descriptor belongs to exactly one chunk.
        let mut spans: Vec<(usize, usize)> = dir_chunks
            .iter()
            .chain(free_chunks.iter())
            .chain(entries.iter().flat_map(|e| e.chunks.iter()))
            .map(|c| (c.offset, c.capacity))
            .collect();
        spans.sort_unstable();
        let (mut pos, mut overlaps) = (HEADER_SIZE + DIRDATA_SIZE, 0);
        for (off, cap) in spans {
            if off < pos {
                overlaps += 1;
            }
            pos = pos.max(off + cap);
        }
        if overlaps != 0 {
            warnings.push(format!("{overlaps} overlapping chunks"));
        }
        if pos != data.len() {
            warnings.push(format!("chunks end at {pos:#x}, file size {:#x}", data.len()));
        }
        Ok(CmfFile { data, ident, version, format, capacity, used_count, entries, warnings })
    }

    /// Concatenated payload of directory entry `i` (empty if unused or out of range).
    pub fn entry_bytes(&self, i: usize) -> Vec<u8> {
        match self.entries.get(i) {
            Some(e) if e.used && e.first != 0 => {
                chain(&self.data, e.first, Some(e.size), "entry").map(|r| r.0).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }
}

fn chain(d: &[u8], first: usize, expect: Option<usize>, what: &str) -> Result<(Vec<u8>, Vec<Chunk>)> {
    let (mut out, mut chunks, mut pos, mut seen) = (Vec::new(), Vec::new(), first, HashSet::new());
    while pos != 0 {
        if !seen.insert(pos) {
            return Err(err!("{what}: chunk loop at {pos:#x}"));
        }
        if pos < HEADER_SIZE + DIRDATA_SIZE || pos + TAG_SIZE > d.len() {
            return Err(err!("{what}: chunk offset {pos:#x} out of range"));
        }
        let cap = be32(d, pos).unwrap() as usize;
        let ln = be32(d, pos + 4).unwrap() as usize;
        let nxt = be32(d, pos + 8).unwrap() as usize;
        if ln + TAG_SIZE > cap {
            return Err(err!("{what}: chunk at {pos:#x} length {ln} exceeds capacity {cap}"));
        }
        if pos + TAG_SIZE + ln > d.len() {
            return Err(err!("{what}: chunk at {pos:#x} runs past end of file"));
        }
        chunks.push(Chunk { offset: pos, capacity: cap, length: ln });
        out.extend_from_slice(&d[pos + TAG_SIZE..pos + TAG_SIZE + ln]);
        pos = nxt;
    }
    if let Some(n) = expect {
        if out.len() != n {
            return Err(err!("{what}: chain holds {} bytes, directory says {n}", out.len()));
        }
    }
    Ok((out, chunks))
}

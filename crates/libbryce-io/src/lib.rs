// SPDX-License-Identifier: GPL-3.0-or-later
//! Reader for Bryce 3D CCmF composite files.
//!
//! Format description: `docs/formats/axiom-cmf.md`. Layers:
//! 1. [`cmf`]: container (header, directory, chunk chains; big-endian)
//! 2. [`resource`]: Axiom resource tables (master table -> per-type tables; big-endian)
//! 3. [`scene`]: object lists and the scene tree (payload in the writer's byte order)

pub mod cmf;
pub mod records;
pub mod resource;
pub mod scene;

use std::fmt;

/// Error while reading a composite file.
#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! err {
    ($($t:tt)*) => { $crate::Error(format!($($t)*)) };
}
pub(crate) use err;

/// Byte order of a payload record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteOrder {
    Big,
    Little,
}

impl ByteOrder {
    pub fn u32(self, d: &[u8], o: usize) -> Option<u32> {
        let b: [u8; 4] = d.get(o..o + 4)?.try_into().ok()?;
        Some(match self {
            ByteOrder::Big => u32::from_be_bytes(b),
            ByteOrder::Little => u32::from_le_bytes(b),
        })
    }
}

impl ByteOrder {
    pub fn f32(self, d: &[u8], o: usize) -> Option<f32> {
        self.u32(d, o).map(f32::from_bits)
    }

    pub fn f64(self, d: &[u8], o: usize) -> Option<f64> {
        let b: [u8; 8] = d.get(o..o + 8)?.try_into().ok()?;
        Some(match self {
            ByteOrder::Big => f64::from_be_bytes(b),
            ByteOrder::Little => f64::from_le_bytes(b),
        })
    }
}

pub(crate) fn be32(d: &[u8], o: usize) -> Option<u32> {
    ByteOrder::Big.u32(d, o)
}

/// MacRoman code points for bytes 0x80..=0xFF.
const MAC_ROMAN_HIGH: [char; 128] = ['\u{c4}', '\u{c5}', '\u{c7}', '\u{c9}', '\u{d1}', '\u{d6}', '\u{dc}', '\u{e1}', '\u{e0}', '\u{e2}', '\u{e4}', '\u{e3}', '\u{e5}', '\u{e7}', '\u{e9}', '\u{e8}', '\u{ea}', '\u{eb}', '\u{ed}', '\u{ec}', '\u{ee}', '\u{ef}', '\u{f1}', '\u{f3}', '\u{f2}', '\u{f4}', '\u{f6}', '\u{f5}', '\u{fa}', '\u{f9}', '\u{fb}', '\u{fc}', '\u{2020}', '\u{b0}', '\u{a2}', '\u{a3}', '\u{a7}', '\u{2022}', '\u{b6}', '\u{df}', '\u{ae}', '\u{a9}', '\u{2122}', '\u{b4}', '\u{a8}', '\u{2260}', '\u{c6}', '\u{d8}', '\u{221e}', '\u{b1}', '\u{2264}', '\u{2265}', '\u{a5}', '\u{b5}', '\u{2202}', '\u{2211}', '\u{220f}', '\u{3c0}', '\u{222b}', '\u{aa}', '\u{ba}', '\u{3a9}', '\u{e6}', '\u{f8}', '\u{bf}', '\u{a1}', '\u{ac}', '\u{221a}', '\u{192}', '\u{2248}', '\u{2206}', '\u{ab}', '\u{bb}', '\u{2026}', '\u{a0}', '\u{c0}', '\u{c3}', '\u{d5}', '\u{152}', '\u{153}', '\u{2013}', '\u{2014}', '\u{201c}', '\u{201d}', '\u{2018}', '\u{2019}', '\u{f7}', '\u{25ca}', '\u{ff}', '\u{178}', '\u{2044}', '\u{20ac}', '\u{2039}', '\u{203a}', '\u{fb01}', '\u{fb02}', '\u{2021}', '\u{b7}', '\u{201a}', '\u{201e}', '\u{2030}', '\u{c2}', '\u{ca}', '\u{c1}', '\u{cb}', '\u{c8}', '\u{cd}', '\u{ce}', '\u{cf}', '\u{cc}', '\u{d3}', '\u{d4}', '\u{f8ff}', '\u{d2}', '\u{da}', '\u{db}', '\u{d9}', '\u{131}', '\u{2c6}', '\u{2dc}', '\u{af}', '\u{2d8}', '\u{2d9}', '\u{2da}', '\u{b8}', '\u{2dd}', '\u{2db}', '\u{2c7}'];

/// Decode a NUL-terminated MacRoman string.
pub(crate) fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    b[..end]
        .iter()
        .map(|&c| if c < 0x80 { c as char } else { MAC_ROMAN_HIGH[(c - 0x80) as usize] })
        .collect()
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! Layer 2: Axiom resource tables (always big-endian).

use crate::cmf::CmfFile;
use crate::{be32, cstr, err, Result};

const MASTER_REF: (usize, usize) = (16, 17);
const TABLE_HEADER_SIZE: usize = 32;
const MASTER_ELEM: usize = 32;
const RESENTRY_ELEM: usize = 60;

#[derive(Clone, Debug)]
pub struct Resource {
    pub identity: u32,
    pub name: String,
    /// Directory entries holding the payload and the auxiliary data.
    pub payload_ref: usize,
    pub aux_ref: usize,
    pub user: [u32; 4],
}

#[derive(Clone, Debug)]
pub struct ResourceType {
    pub name: String,
    pub resources: Vec<Resource>,
}

/// Read all resource types. Returns `Ok(vec![])` for non-resource composites.
/// Non-fatal problems are appended to `warnings`.
pub fn read_types(f: &CmfFile, warnings: &mut Vec<String>) -> Vec<ResourceType> {
    if f.format != "Resource Composite File" {
        return Vec::new();
    }
    let (info, body) = match table(f, MASTER_REF.0, MASTER_REF.1, MASTER_ELEM, "master table") {
        Ok(x) => x,
        Err(e) => {
            warnings.push(format!("no resource layer: {e}"));
            return Vec::new();
        }
    };
    let mut types = Vec::new();
    for k in 0..info {
        let rec = &body[k * MASTER_ELEM..(k + 1) * MASTER_ELEM];
        let name = cstr(&rec[..16]);
        let (pers, data) = (be32(rec, 16).unwrap() as usize, be32(rec, 20).unwrap() as usize);
        let mut t = ResourceType { name: name.clone(), resources: Vec::new() };
        match table(f, pers, data, RESENTRY_ELEM, &format!("type {name}")) {
            Ok((count, tbody)) => {
                for r in 0..count {
                    let rr = &tbody[r * RESENTRY_ELEM..(r + 1) * RESENTRY_ELEM];
                    let w = |o| be32(rr, o).unwrap();
                    t.resources.push(Resource {
                        identity: w(0),
                        name: cstr(&rr[12..44]),
                        payload_ref: w(4) as usize,
                        aux_ref: w(8) as usize,
                        user: [w(0x2C), w(0x30), w(0x34), w(0x38)],
                    });
                }
            }
            Err(e) => warnings.push(e.to_string()),
        }
        types.push(t);
    }
    types
}

/// Returns (count, table body).
fn table(f: &CmfFile, pers: usize, data: usize, elem: usize, what: &str) -> Result<(usize, Vec<u8>)> {
    let hdr = f.entry_bytes(data);
    if hdr.len() != TABLE_HEADER_SIZE {
        return Err(err!("{what}: table header entry {data} has {} bytes, expected 32", hdr.len()));
    }
    let (esz, cap, count) = (
        be32(&hdr, 0).unwrap() as usize,
        be32(&hdr, 4).unwrap() as usize,
        be32(&hdr, 12).unwrap() as usize,
    );
    if esz != elem || count > cap {
        return Err(err!("{what}: bad table header elemSize={esz} capacity={cap} count={count}"));
    }
    let body = f.entry_bytes(pers);
    if body.len() < count * elem {
        return Err(err!("{what}: table body entry {pers} too short ({} < {})", body.len(), count * elem));
    }
    Ok((count, body))
}

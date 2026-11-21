// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests on synthetic CCmF files built in memory (no original data needed).

use bryce_io::cmf::CmfFile;
use bryce_io::records::{decode_channel_header, decode_object};
use bryce_io::scene::Br3File;
use bryce_io::ByteOrder;

const IDENT: &[u8] = b"CCmF - Universal - Axiom - AGP - Composite File Management System (vers. 5)";

fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn padded(s: &str, n: usize) -> Vec<u8> {
    let mut b = s.as_bytes().to_vec();
    b.resize(n, 0);
    b
}

/// Build a composite file: `entries[i]` is the payload of directory entry i (None = unused).
fn build(format: &str, entries: &[Option<Vec<u8>>]) -> Vec<u8> {
    let cap = entries.len();
    let mut out = padded(std::str::from_utf8(IDENT).unwrap(), 0x200);
    out.extend(padded(format, 0x20));
    out.resize(0x248, 0);
    // chunks start after the 0x48-byte directory descriptor
    let mut pos = 0x248 + 0x48;
    let dir_first = pos;
    pos += 12 + cap * 12;
    let mut dir = Vec::new();
    let mut chunks = Vec::new();
    for e in entries {
        match e {
            Some(p) => {
                dir.push(1);
                dir.extend_from_slice(&[0, 0, 0]);
                put32(&mut dir, p.len() as u32);
                put32(&mut dir, if p.is_empty() { 0 } else { pos as u32 });
                if !p.is_empty() {
                    put32(&mut chunks, (p.len() + 12) as u32);
                    put32(&mut chunks, p.len() as u32);
                    put32(&mut chunks, 0);
                    chunks.extend_from_slice(p);
                    pos += 12 + p.len();
                }
            }
            None => dir.extend_from_slice(&[0; 12]),
        }
    }
    let mut desc = vec![0u8; 0x48];
    desc[0] = 1;
    desc[4..8].copy_from_slice(&((cap * 12) as u32).to_be_bytes());
    desc[8..12].copy_from_slice(&(dir_first as u32).to_be_bytes());
    desc[12] = 1;
    desc[24..28].copy_from_slice(&(cap as u32).to_be_bytes());
    let used = entries.iter().filter(|e| e.is_some()).count();
    desc[28..32].copy_from_slice(&(used as u32).to_be_bytes());
    out.extend(desc);
    put32(&mut out, (12 + dir.len()) as u32);
    put32(&mut out, dir.len() as u32);
    put32(&mut out, 0);
    out.extend(dir);
    out.extend(chunks);
    out
}

fn table_header(elem: u32, count: u32) -> Vec<u8> {
    let mut h = Vec::new();
    for x in [elem, 8, 8, count] {
        put32(&mut h, x);
    }
    h.resize(32, 0);
    h
}

fn master_elem(name: &str, pers: u32, data: u32) -> Vec<u8> {
    let mut e = padded(name, 16);
    put32(&mut e, pers);
    put32(&mut e, data);
    e.resize(32, 0);
    e
}

fn res_elem(ident: u32, name: &str, pers: u32) -> Vec<u8> {
    let mut e = Vec::new();
    put32(&mut e, ident);
    put32(&mut e, pers);
    put32(&mut e, 0);
    e.extend(padded(name, 32));
    e.resize(60, 0);
    e
}

/// 640-byte little-endian sphere record at (1, 2, 3).
fn sphere_record() -> Vec<u8> {
    let mut d = vec![0u8; 0x280];
    d[..4].copy_from_slice(&1u32.to_le_bytes());
    d[8..8 + 6].copy_from_slice(b"Sphere");
    for (i, v) in [1.0f64, 2.0, 3.0].iter().enumerate() {
        let o = 0x108 + 0x74 + 8 * (9 + i);
        d[o..o + 8].copy_from_slice(&v.to_le_bytes());
    }
    d
}

fn sample() -> Vec<u8> {
    let mut objlist = Vec::new();
    objlist.extend_from_slice(&1u32.to_le_bytes());
    objlist.extend_from_slice(&1u32.to_le_bytes());
    objlist.extend_from_slice(&1u32.to_le_bytes());
    objlist.extend_from_slice(b"bod3rhps"); // "3dob"/"sphr" reversed (little-endian writer)
    objlist.extend_from_slice(&7u32.to_le_bytes());
    objlist.extend_from_slice(&0u32.to_le_bytes());
    let mut e: Vec<Option<Vec<u8>>> = vec![None; 24];
    e[16] = Some([master_elem("C3dSphere", 18, 19), master_elem("C3dRootObjList", 21, 22)].concat());
    e[17] = Some(table_header(32, 2));
    e[18] = Some(res_elem(7, "Ball", 20));
    e[19] = Some(table_header(60, 1));
    e[20] = Some(sphere_record());
    e[21] = Some(res_elem(1, "root", 23));
    e[22] = Some(table_header(60, 1));
    e[23] = Some(objlist);
    build("Resource Composite File", &e)
}

#[test]
fn container_and_resources() {
    let cmf = CmfFile::parse(sample()).unwrap();
    assert_eq!(cmf.version, Some(5));
    assert_eq!(cmf.used_count, 8);
    assert!(cmf.warnings.is_empty(), "{:?}", cmf.warnings);
    assert_eq!(cmf.entry_bytes(20).len(), 0x280);
}

#[test]
fn scene_tree_and_object_record() {
    let dir = std::env::temp_dir().join(format!("bryce_io_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("synthetic.br3");
    std::fs::write(&path, sample()).unwrap();
    let mut f = Br3File::open(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(f.resource_count(), 2);
    let tree = f.scene_tree();
    assert_eq!(tree.len(), 1);
    assert_eq!((tree[0].class.as_str(), tree[0].subclass.as_str()), ("3dob", "sphr"));
    assert_eq!(tree[0].name.as_deref(), Some("Ball"));
    let (t, r) = f.find(tree[0].identity, Some("C3dSphere")).unwrap();
    let o = f.object(t, r).unwrap();
    assert_eq!(o.byte_order, ByteOrder::Little);
    assert_eq!(o.name, "Sphere");
    assert_eq!(o.position(), [1.0, 2.0, 3.0]);
    assert!(f.warnings.is_empty(), "{:?}", f.warnings);
}

#[test]
fn rejects_garbage() {
    assert!(CmfFile::parse(vec![0; 1000]).is_err());
    let mut d = sample();
    d.truncate(d.len() - 5);
    assert!(CmfFile::parse(d).is_err());
}

#[test]
fn object_record_wrong_size_or_flag() {
    assert!(decode_object("C3dSphere", &[0; 10]).is_none());
    let mut d = sphere_record();
    d[0] = 7;
    assert!(decode_object("C3dSphere", &d).is_none());
    assert!(decode_object("C3dTexture", &sphere_record()).is_none());
}

#[test]
fn channel_header() {
    let mut d = vec![0u8; 0x94 + 2 * 16];
    for (i, v) in [1u32, 1, 2, 0, 0].iter().enumerate() {
        d[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
    }
    let h = decode_channel_header(&d).unwrap();
    assert_eq!((h.keys, h.key_size, h.extra), (2, 16, 0));
}

#[test]
fn texture_record() {
    use bryce_io::records::decode_texture;
    let mut d = vec![0u8; 1588];
    d[0] = 1; // little-endian payload
    let put = |d: &mut Vec<u8>, o: usize, v: u32| d[o..o + 4].copy_from_slice(&v.to_le_bytes());
    d[8..13].copy_from_slice(b"wood\0");
    d[0x108 + 0x0C..0x108 + 0x14].copy_from_slice(&2.5f64.to_le_bytes());
    put(&mut d, 0x108 + 0x5C, 0x1234); // Btx_eMapping
    put(&mut d, 0x1F4 + 4, 7); // kind[0]
    put(&mut d, 0x1F4 + 0x54 + 0x110 + 0x54, 0xFF8000); // component 1 colour 0
    let t = decode_texture("C3dTexture", &d).unwrap();
    assert_eq!(t.name, "wood");
    assert_eq!(t.scale[0], 2.5);
    assert_eq!(t.channels, vec![("Btx_eMapping", 0x1234)]);
    assert_eq!(t.params.kind[0], 7);
    assert_eq!(t.components[1].colors[0], 0xFF8000);
    assert!(decode_texture("C3dTexture", &d[..100]).is_none());
    assert!(decode_texture("C3dSphere", &d).is_none());
}

#[test]
fn subclass_decides_type() {
    use bryce_io::scene::subclass_type;
    assert_eq!(subclass_type("3dob", "toru"), Some("C3dTorusType"));
    assert_eq!(subclass_type("camr", "base"), Some("C3dBaseCamera"));
    assert_eq!(subclass_type("3dob", "zzzz"), None);
}

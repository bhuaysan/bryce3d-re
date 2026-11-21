// SPDX-License-Identifier: GPL-3.0-or-later
//! Layer 3 (partial): object lists and the scene tree.

use crate::cmf::CmfFile;
use crate::resource::{read_types, Resource, ResourceType};
use crate::records::{decode_material, decode_object, decode_texture, texture_type, MaterialRecord, ObjectRecord, TextureRecord};
use crate::ByteOrder;
use std::collections::HashSet;
use std::collections::HashMap;

/// One node of the scene tree.
#[derive(Clone, Debug)]
pub struct Node {
    pub class: String,
    pub subclass: String,
    pub identity: u32,
    /// Resource type of the object record (e.g. `C3dSphere`), if it resolves.
    pub type_name: Option<String>,
    pub name: Option<String>,
    pub children: Vec<Node>,
}

/// A parsed composite file: container, resource tables and scene tree.
pub struct Br3File {
    pub cmf: CmfFile,
    pub types: Vec<ResourceType>,
    pub warnings: Vec<String>,
}

/// Resource type named by an object-list record's (class, subclass); `None` if not observed.
pub fn subclass_type(class: &str, sub: &str) -> Option<&'static str> {
    Some(match (class, sub) {
        ("camr", "base") => "C3dBaseCamera",
        ("3dob", "cone") => "C3dCone",
        ("3dob", "cube") => "C3dCube",
        ("3dob", "cylr") => "C3dCylinder",
        ("3dob", "grou") => "C3dGroupType",
        ("3dob", "hfld") => "C3dHeightField",
        ("3dob", "path") => "C3dPath",
        ("3dob", "plne") => "C3dPlane",
        ("3dob", "slab") => "C3dSlab",
        ("3dob", "sphr") => "C3dSphere",
        ("3dob", "sqre") => "C3dSquare",
        ("3dob", "tmsh") => "C3dTriMeshType2",
        ("3dob", "toru") => "C3dTorusType",
        ("lght", "cone") => "C3dConeLight",
        _ => return None,
    })
}

struct ObjRec {
    class: String,
    sub: String,
    ident: u32,
    child: u32,
}

impl Br3File {
    pub fn open(path: impl AsRef<std::path::Path>) -> crate::Result<Self> {
        let cmf = CmfFile::open(path)?;
        let mut warnings = cmf.warnings.clone();
        let types = read_types(&cmf, &mut warnings);
        let mut f = Br3File { cmf, types, warnings };
        f.check_material_refs();
        Ok(f)
    }

    /// Find a resource by identity, optionally restricted to a type name.
    pub fn find(&self, identity: u32, type_name: Option<&str>) -> Option<(&str, &Resource)> {
        self.types
            .iter()
            .filter(|t| type_name.map_or(true, |n| n == t.name))
            .flat_map(|t| t.resources.iter().map(move |r| (t.name.as_str(), r)))
            .find(|(_, r)| r.identity == identity)
    }

    /// Decoded object/group/torus/camera record of a resource.
    pub fn object(&self, type_name: &str, r: &Resource) -> Option<ObjectRecord> {
        decode_object(type_name, &self.cmf.entry_bytes(r.payload_ref))
    }

    /// Decoded material record of a resource.
    pub fn material(&self, type_name: &str, r: &Resource) -> Option<MaterialRecord> {
        decode_material(type_name, &self.cmf.entry_bytes(r.payload_ref))
    }

    /// Decoded texture record of a resource.
    pub fn texture(&self, type_name: &str, r: &Resource) -> Option<TextureRecord> {
        decode_texture(type_name, &self.cmf.entry_bytes(r.payload_ref))
    }

    /// Regression check for the material decoder: every texture reference and animation
    /// channel identity must resolve to a resource of the expected type.
    fn check_material_refs(&mut self) {
        let have: HashSet<(&str, u32)> = self
            .types
            .iter()
            .flat_map(|t| t.resources.iter().map(move |r| (t.name.as_str(), r.identity)))
            .collect();
        let mut out = Vec::new();
        for t in self.types.iter().filter(|t| t.name == "C3dBaseMaterial") {
            for r in &t.resources {
                let Some(m) = self.material(&t.name, r) else { continue };
                let label = format!("material {:#x}", r.identity);
                let texs = m.colors.iter().map(|(n, c)| (*n, &c.tex)).chain(m.scalars.iter().map(|(n, c)| (*n, &c.tex)));
                for (n, tex) in texs {
                    for tx in tex {
                        if !texture_type(&tx.class).is_some_and(|w| have.contains(&(w, tx.id))) {
                            out.push(format!("{label}: {n} texture {tx:?} does not resolve"));
                        }
                    }
                }
                for (n, id) in &m.channels {
                    if !have.contains(&(*n, *id)) {
                        out.push(format!("{label}: channel {n} id {id:#x} does not resolve"));
                    }
                }
            }
        }
        self.warnings.extend(out);
    }

    pub fn resource_count(&self) -> usize {
        self.types.iter().map(|t| t.resources.len()).sum()
    }

    fn objlist(&self, r: &Resource) -> Option<(ByteOrder, Vec<ObjRec>)> {
        let d = self.cmf.entry_bytes(r.payload_ref);
        if d.len() < 12 {
            return None;
        }
        for (bo, flag) in [(ByteOrder::Big, 0), (ByteOrder::Little, 1)] {
            let (w0, w1, n) = (bo.u32(&d, 0)?, bo.u32(&d, 4)?, bo.u32(&d, 8)? as usize);
            if w0 == 1 && w1 == flag && d.len() == 12 + 16 * n {
                let recs = (0..n)
                    .map(|k| {
                        let o = 12 + 16 * k;
                        let (mut c, mut s) = (d[o..o + 4].to_vec(), d[o + 4..o + 8].to_vec());
                        if bo == ByteOrder::Little {
                            // FourCCs are native uint32 on x86: stored reversed.
                            c.reverse();
                            s.reverse();
                        }
                        ObjRec {
                            class: c.iter().map(|&b| b as char).collect(),
                            sub: s.iter().map(|&b| b as char).collect(),
                            ident: bo.u32(&d, o + 8).unwrap(),
                            child: bo.u32(&d, o + 12).unwrap(),
                        }
                    })
                    .collect();
                return Some((bo, recs));
            }
        }
        None
    }

    /// Decode `C3dRootObjList` / `C3dObjList` into nested trees. A file with exactly one
    /// root list yields its objects directly; preset libraries yield one synthetic
    /// `root` node per root list.
    pub fn scene_tree(&mut self) -> Vec<Node> {
        let mut by_id: HashMap<u32, Vec<(&str, &Resource)>> = HashMap::new();
        for t in &self.types {
            for r in &t.resources {
                by_id.entry(r.identity).or_default().push((&t.name, r));
            }
        }
        let mut lists: HashMap<u32, Vec<ObjRec>> = HashMap::new();
        let mut roots: Vec<(u32, Vec<ObjRec>)> = Vec::new();
        let mut new_warnings = Vec::new();
        for t in &self.types {
            if t.name != "C3dRootObjList" && t.name != "C3dObjList" {
                continue;
            }
            for r in &t.resources {
                match self.objlist(r) {
                    Some((_, recs)) if t.name == "C3dRootObjList" => roots.push((r.identity, recs)),
                    Some((_, recs)) => {
                        lists.insert(r.identity, recs);
                    }
                    None => new_warnings.push(format!("object list {:#x}: unrecognised header", r.identity)),
                }
            }
        }
        fn build(recs: &[ObjRec], lists: &HashMap<u32, Vec<ObjRec>>, by_id: &HashMap<u32, Vec<(&str, &Resource)>>, depth: u32) -> Vec<Node> {
            recs.iter()
                .map(|o| {
                    // Identities collide between types: the subclass decides when it is known.
                    let want = subclass_type(&o.class, &o.sub);
                    let target = by_id
                        .get(&o.ident)
                        .and_then(|v| v.iter().find(|x| x.0 != "C3dObjList" && want.map_or(true, |w| x.0 == w)));
                    let children = match lists.get(&o.child) {
                        Some(l) if o.child != 0 && depth < 32 => build(l, lists, by_id, depth + 1),
                        _ => Vec::new(),
                    };
                    Node {
                        class: o.class.clone(),
                        subclass: o.sub.clone(),
                        identity: o.ident,
                        type_name: target.map(|x| x.0.to_string()),
                        name: target.map(|x| x.1.name.clone()),
                        children,
                    }
                })
                .collect()
        }
        let tree = if roots.len() == 1 {
            build(&roots[0].1, &lists, &by_id, 0)
        } else {
            roots
                .iter()
                .enumerate()
                .map(|(i, (id, recs))| Node {
                    class: "root".into(),
                    subclass: String::new(),
                    identity: *id,
                    type_name: Some("C3dRootObjList".into()),
                    name: Some(format!("root list {i}")),
                    children: build(recs, &lists, &by_id, 0),
                })
                .collect()
        };
        self.warnings.extend(new_warnings);
        tree
    }
}

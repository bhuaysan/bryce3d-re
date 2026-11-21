// SPDX-License-Identifier: GPL-3.0-or-later
//! Layer 3: decoded Bryce records (`docs/formats/axiom-cmf.md` §6.4-6.9).
//!
//! All records start with a `u32` PC flag at offset 0 (1 = little-endian payload)
//! and a 256-byte name at offset 8.

use crate::{cstr, ByteOrder};

const OBJ_BASE: usize = 0x108;
const MAT_SIZE: usize = 976;
const TEX_SIZE: usize = 1588;
pub const TEX_CHANNEL_SLOTS: [&str; 4] = ["Btx_eMapping", "Btx_eTrnScale", "Btx_eTrnOffset", "Btx_eTrnRot"];

pub const OBJ_CHANNELS: [&str; 7] =
    ["Obj_eUnused", "Obj_ePathT", "Obj_ePosition", "Obj_eOrient", "Obj_eScale", "Obj_eShear", "Obj_eOrigin"];
pub const CAM_CHANNELS: [&str; 7] = [
    "Cam_eCraOrient", "Cam_eCorpOrient/Bank", "Cam_eFOV", "Cam_ePanX", "Cam_ePanY", "Cam_eZoomX", "Cam_eZoomY",
];
pub const MAT_COLOR_CHANNELS: [&str; 5] =
    ["Mat_eDiffuseC", "Mat_eAmbientC", "Mat_eSpecularC", "Mat_eSpecExp", "Mat_eTransC"];
pub const MAT_SCALAR_CHANNELS: [&str; 8] = [
    "Mat_eDiffuse", "Mat_eAmbient", "Mat_eSpecular", "Mat_eTrans", "Mat_eReflect", "Mat_eBump",
    "Mat_eMetallic", "Mat_eHTDensity",
];
/// Order of the 19 animation channel identities at material offset 0x3a4.
pub const MAT_CHANNEL_SLOTS: [&str; 19] = [
    "Mat_eDiffuseC", "Mat_eAmbientC", "Mat_eSpecularC", "Mat_eSpecExp", "Mat_eTransC", "Mat_eDiffuse",
    "Mat_eAmbient", "Mat_eSpecular", "Mat_eTrans", "Mat_eReflect", "Mat_eBump", "Mat_eMetallic",
    "Mat_eHTDensity", "Mat_eMatFlags", "Mat_eVolC", "Mat_eRefract", "Mat_eHTEdge", "Mat_eHTSharp",
    "Mat_eHTQuality",
];

/// Extra bytes after the common 640-byte object part, per resource type.
fn obj_extra(tname: &str) -> Option<usize> {
    Some(match tname {
        "C3dSphere" | "C3dCube" | "C3dCone" | "C3dCylinder" | "C3dPlane" | "C3dSlab" | "C3dSquare"
        | "C3dCircle" | "C3dPyramid" => 0,
        "C3dGroupType" => 4,
        "C3dTorusType" => 28,
        "C3dBaseCamera" => 168,
        _ => return None,
    })
}

fn order(d: &[u8]) -> Option<ByteOrder> {
    match ByteOrder::Little.u32(d, 0)? {
        0 => Some(ByteOrder::Big),
        1 => Some(ByteOrder::Little),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    pub flags: [u32; 2],
    pub state: [f64; 7],
    pub channels: Vec<(&'static str, u32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectRecord {
    pub byte_order: ByteOrder,
    pub name: String,
    /// Identity of the material resource.
    pub material: Option<u32>,
    pub matrix_a: [f64; 12],
    pub matrix_b: [f64; 12],
    /// Non-zero animation channel identities.
    pub channels: Vec<(&'static str, u32)>,
    pub group_ref: Option<u32>,
    pub camera: Option<Camera>,
}

impl ObjectRecord {
    /// Position (translation row of `matrix_b`).
    pub fn position(&self) -> [f64; 3] {
        [self.matrix_b[9], self.matrix_b[10], self.matrix_b[11]]
    }
}

fn channel_ids(bo: ByteOrder, d: &[u8], o: usize, names: &[&'static str]) -> Vec<(&'static str, u32)> {
    names
        .iter()
        .enumerate()
        .filter_map(|(i, n)| bo.u32(d, o + 4 * i).filter(|&v| v != 0).map(|v| (*n, v)))
        .collect()
}

fn f64s<const N: usize>(bo: ByteOrder, d: &[u8], o: usize) -> [f64; N] {
    std::array::from_fn(|i| bo.f64(d, o + 8 * i).unwrap_or(0.0))
}

/// Decode an object, group, torus or camera record. `None` if the type is not an
/// object type or the size does not match.
pub fn decode_object(tname: &str, d: &[u8]) -> Option<ObjectRecord> {
    let extra = obj_extra(tname)?;
    if d.len() != 0x280 + extra {
        return None;
    }
    let bo = order(d)?;
    let o = OBJ_BASE;
    let mat = bo.u32(d, o + 0x10)?;
    let t = 0x280;
    let camera = (tname == "C3dBaseCamera").then(|| Camera {
        flags: [bo.u32(d, t + 0x1C).unwrap(), bo.u32(d, t + 0x20).unwrap()],
        state: f64s(bo, d, t + 0x54),
        channels: channel_ids(bo, d, t + 0x8C, &CAM_CHANNELS),
    });
    Some(ObjectRecord {
        byte_order: bo,
        name: cstr(&d[8..0x108]),
        material: (mat != 0).then_some(mat),
        matrix_a: f64s(bo, d, o + 0x14),
        matrix_b: f64s(bo, d, o + 0x74),
        channels: channel_ids(bo, d, o + 0xDC, &OBJ_CHANNELS),
        group_ref: (tname == "C3dGroupType").then(|| bo.u32(d, t).unwrap()),
        camera,
    })
}

/// Reference to a texture resource: FourCC class (`brtx`, `pctx`) and identity.
#[derive(Clone, Debug, PartialEq)]
pub struct TexRef {
    pub class: String,
    pub id: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatColor {
    pub tex: Vec<TexRef>,
    pub mode: u32,
    pub rgb: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatScalar {
    pub tex: Vec<TexRef>,
    pub mode: u32,
    pub value: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialRecord {
    pub byte_order: ByteOrder,
    pub name: String,
    pub version: u32,
    pub mat_flags: u32,
    pub colors: Vec<(&'static str, MatColor)>,
    pub scalars: Vec<(&'static str, MatScalar)>,
    pub vol_color: [f32; 3],
    pub refract: f32,
    pub ht_edge: f32,
    pub ht_sharp: f32,
    pub ht_quality: f32,
    pub channels: Vec<(&'static str, u32)>,
}

fn texrefs(bo: ByteOrder, d: &[u8], o: usize) -> Vec<TexRef> {
    (0..3)
        .filter_map(|i| {
            let (fc, id) = (bo.u32(d, o + 8 * i)?, bo.u32(d, o + 8 * i + 4)?);
            if fc == 0 && id == 0 {
                return None;
            }
            // File bytes of the code; little-endian writers store it reversed.
            let mut code = match bo {
                ByteOrder::Big => fc.to_be_bytes(),
                ByteOrder::Little => fc.to_le_bytes(),
            };
            if bo == ByteOrder::Little {
                code.reverse();
            }
            Some(TexRef { class: code.iter().map(|&b| b as char).collect(), id })
        })
        .collect()
}

/// Resource type name for a texture reference class.
pub fn texture_type(class: &str) -> Option<&'static str> {
    match class {
        "brtx" => Some("C3dTexture"),
        "pctx" => Some("CPictTexture"),
        _ => None,
    }
}

pub fn decode_material(tname: &str, d: &[u8]) -> Option<MaterialRecord> {
    if tname != "C3dBaseMaterial" || d.len() != MAT_SIZE {
        return None;
    }
    let bo = order(d)?;
    let m = 0x108;
    let f3 = |o| -> [f32; 3] { std::array::from_fn(|i| bo.f32(d, o + 4 * i).unwrap()) };
    let colors = MAT_COLOR_CHANNELS
        .iter()
        .enumerate()
        .map(|(k, n)| {
            let o = m + 8 + 0x38 * k;
            (*n, MatColor { tex: texrefs(bo, d, o), mode: bo.u32(d, o + 0x18).unwrap(), rgb: f3(o + 0x1C) })
        })
        .collect();
    let scalars = MAT_SCALAR_CHANNELS
        .iter()
        .enumerate()
        .map(|(k, n)| {
            let o = m + 0x120 + 0x28 * k;
            (*n, MatScalar { tex: texrefs(bo, d, o), mode: bo.u32(d, o + 0x18).unwrap(), value: bo.f32(d, o + 0x1C).unwrap() })
        })
        .collect();
    Some(MaterialRecord {
        byte_order: bo,
        name: cstr(&d[8..0x108]),
        version: bo.u32(d, m)?,
        mat_flags: bo.u32(d, m + 4)?,
        colors,
        scalars,
        vol_color: f3(m + 0x260),
        refract: bo.f32(d, m + 0x26C)?,
        ht_edge: bo.f32(d, m + 0x270)?,
        ht_sharp: bo.f32(d, m + 0x274)?,
        ht_quality: bo.f32(d, m + 0x278)?,
        channels: channel_ids(bo, d, m + 0x27C, &MAT_CHANNEL_SLOTS),
    })
}

/// Animation channel header: `u32 1, u32 PC flag, u32 key count, u32, u32 extra`; keys of
/// `key_size` bytes start at 0x94 and `len == 0x94 + keys*key_size + extra`.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelHeader {
    pub byte_order: ByteOrder,
    pub keys: u32,
    pub key_size: usize,
    pub extra: u32,
}

pub fn decode_channel_header(d: &[u8]) -> Option<ChannelHeader> {
    if d.len() < 0x94 {
        return None;
    }
    for (bo, flag) in [(ByteOrder::Big, 0), (ByteOrder::Little, 1)] {
        if bo.u32(d, 0)? == 1 && bo.u32(d, 4)? == flag {
            let (n, extra) = (bo.u32(d, 8)?, bo.u32(d, 16)?);
            let body = (d.len() as i64) - 0x94 - extra as i64;
            return (n != 0 && body >= 0 && body % n as i64 == 0)
                .then(|| ChannelHeader { byte_order: bo, keys: n, key_size: (body / n as i64) as usize, extra });
        }
    }
    None
}

/// One of the three procedural component blocks (0x110 bytes) of a texture, head only.
#[derive(Clone, Debug, PartialEq)]
pub struct TexComponent {
    pub head: [u32; 3],
    pub ints_a: [i32; 5],
    pub u20: u32,
    pub ints_b: [i32; 6],
    pub colors: [u32; 3],
    pub flags_b0: [u32; 2],
    pub flag_108: u32,
}

/// Typed parameter header of the procedural part (file offset 0x1f4 on).
#[derive(Clone, Debug, PartialEq)]
pub struct TexParams {
    pub kind: [u32; 5],
    pub ints_a: [i32; 6],
    pub ints_b: [u32; 2],
    pub floats: [f32; 2],
    pub modes: [u32; 2],
    pub colors: [u32; 3],
}

/// `C3dTexture`, partial: texture header, parameter header and component heads. The rest of
/// the components (doubles, matrices) is not interpreted yet.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureRecord {
    pub byte_order: ByteOrder,
    pub name: String,
    pub u: [u32; 3],
    pub scale: [f64; 3],
    pub offset: [f64; 3],
    pub channels: Vec<(&'static str, u32)>,
    pub params: TexParams,
    pub components: Vec<TexComponent>,
}

pub fn decode_texture(tname: &str, d: &[u8]) -> Option<TextureRecord> {
    if tname != "C3dTexture" || d.len() != TEX_SIZE {
        return None;
    }
    let bo = order(d)?;
    let (h, t) = (0x108, 0x1F4);
    let u32s = |o: usize| bo.u32(d, o).unwrap();
    let a = |o: usize| -> [u32; 3] { std::array::from_fn(|i| u32s(o + 4 * i)) };
    let components = (0..3)
        .map(|k| {
            let c = t + 0x54 + 0x110 * k;
            TexComponent {
                head: a(c),
                ints_a: std::array::from_fn(|i| u32s(c + 0x0C + 4 * i) as i32),
                u20: u32s(c + 0x20),
                ints_b: std::array::from_fn(|i| u32s(c + 0x24 + 4 * i) as i32),
                colors: a(c + 0x54),
                flags_b0: [u32s(c + 0xB0), u32s(c + 0xB4)],
                flag_108: u32s(c + 0x108),
            }
        })
        .collect();
    Some(TextureRecord {
        byte_order: bo,
        name: cstr(&d[8..0x108]),
        u: a(h),
        scale: f64s(bo, d, h + 0x0C),
        offset: f64s(bo, d, h + 0x24),
        channels: channel_ids(bo, d, h + 0x5C, &TEX_CHANNEL_SLOTS),
        params: TexParams {
            kind: std::array::from_fn(|i| u32s(t + 4 + 4 * i)),
            ints_a: std::array::from_fn(|i| u32s(t + 0x18 + 4 * i) as i32),
            ints_b: [u32s(t + 0x30), u32s(t + 0x34)],
            floats: [bo.f32(d, t + 0x38).unwrap(), bo.f32(d, t + 0x3C).unwrap()],
            modes: [u32s(t + 0x40), u32s(t + 0x44)],
            colors: a(t + 0x48),
        },
        components,
    })
}

//! FLVER2, the game's model format (as Elden Ring writes it, little-endian): a 0x80 header, then the dummies (0x40 each),
//! materials (0x20), bones (0x80), meshes (0x30), face sets (0x20), vertex buffers (0x20), buffer layouts (0x10) and textures
//! (0x20) in that order; the index arrays a mesh names are at file offsets, face and vertex data at offsets from the header's
//! data offset. Read here: what a body's SHAPE needs (bones with their bind pose, each mesh's positions, normals, skin and
//! its full-detail triangles). Textures and materials' shaders are not read: the finale's look needs none.

use glam::{Mat4, Quat, Vec3, Vec4};

#[derive(Clone, Debug)]
pub struct Bone {
    pub name: String,
    pub parent: i32,
    /// Its place against its parent (the file's translation, euler rotation X/Z/Y, scale).
    pub local: Mat4,
    /// Model space in the bind pose (its parents' locals composed).
    pub bind: Mat4,
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    /// Skinned by weights (1) or rigidly on one bone per vertex (0: `rigid`).
    pub dynamic: bool,
    pub material: i32,
    pub default_bone: i32,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    /// Up to four GLOBAL bone indices per vertex and their weights (a rigid vertex: its bone with weight 1).
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    /// Triangle list (the full-detail face set), indices into this mesh's vertices.
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Flver {
    pub version: u32,
    pub bones: Vec<Bone>,
    pub meshes: Vec<Mesh>,
    pub material_names: Vec<String>,
    pub bbox: (Vec3, Vec3),
    /// What was not understood (members of unknown types, skipped face sets): for the log.
    pub notes: Vec<String>,
}

struct R<'a> {
    b: &'a [u8],
}

impl R<'_> {
    fn u8(&self, at: usize) -> Result<u8, String> {
        self.b.get(at).copied().ok_or(format!("FLVER read past its end at {at:#x}"))
    }
    fn i16(&self, at: usize) -> Result<i16, String> {
        self.b.get(at..at + 2).map(|x| i16::from_le_bytes([x[0], x[1]])).ok_or(format!("FLVER read past its end at {at:#x}"))
    }
    fn u16(&self, at: usize) -> Result<u16, String> {
        Ok(self.i16(at)? as u16)
    }
    fn i32(&self, at: usize) -> Result<i32, String> {
        self.b.get(at..at + 4).map(|x| i32::from_le_bytes([x[0], x[1], x[2], x[3]])).ok_or(format!("FLVER read past its end at {at:#x}"))
    }
    fn u32(&self, at: usize) -> Result<u32, String> {
        Ok(self.i32(at)? as u32)
    }
    fn f32(&self, at: usize) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32(at)?))
    }
    fn v3(&self, at: usize) -> Result<Vec3, String> {
        Ok(Vec3::new(self.f32(at)?, self.f32(at + 4)?, self.f32(at + 8)?))
    }
    fn name(&self, at: usize, unicode: bool) -> String {
        let b = self.b;
        if at >= b.len() {
            return String::new();
        }
        if unicode {
            let mut u = Vec::new();
            let mut k = at;
            while let Some(x) = b.get(k..k + 2) {
                let c = u16::from_le_bytes([x[0], x[1]]);
                if c == 0 {
                    break;
                }
                u.push(c);
                k += 2;
            }
            String::from_utf16_lossy(&u)
        } else {
            let end = b[at..].iter().position(|c| *c == 0).map_or(b.len(), |p| at + p);
            String::from_utf8_lossy(&b[at..end]).to_string()
        }
    }
}

/// The face set flags of the reduced details and the motion blur copies (left out).
const FS_LOD1: u32 = 0x0100_0000;
const FS_LOD2: u32 = 0x0200_0000;
const FS_EDGE: u32 = 0x4000_0000;
const FS_BLUR: u32 = 0x8000_0000;

/// A bone's local matrix from the file (column vectors: T * Ry * Rz * Rx * S, SoulsFormats' row-vector S*Rx*Rz*Ry*T).
pub fn bone_local(t: Vec3, r: Vec3, s: Vec3) -> Mat4 {
    Mat4::from_translation(t) * Mat4::from_rotation_y(r.y) * Mat4::from_rotation_z(r.z) * Mat4::from_rotation_x(r.x) * Mat4::from_scale(s)
}

#[derive(Clone, Copy)]
struct Member {
    buffer: usize,
    offset: usize,
    kind: u32,
    semantic: u32,
    index: i32,
}

pub fn read(b: &[u8]) -> Result<Flver, String> {
    if !b.starts_with(b"FLVER\0") {
        return Err("not a FLVER".to_string());
    }
    if &b[6..8] != b"L\0" {
        return Err("a big-endian FLVER is not read".to_string());
    }
    let r = R { b };
    let version = r.u32(0x08)?;
    if version < 0x20000 {
        return Err(format!("FLVER version {version:#x} (not FLVER2)"));
    }
    let data_offset = r.u32(0x0C)? as usize;
    let dummies = r.i32(0x14)?.max(0) as usize;
    let materials = r.i32(0x18)?.max(0) as usize;
    let bones = r.i32(0x1C)?.max(0) as usize;
    let meshes = r.i32(0x20)?.max(0) as usize;
    let vbuffers = r.i32(0x24)?.max(0) as usize;
    let bbox = (r.v3(0x28)?, r.v3(0x34)?);
    let header_index_size = r.u8(0x48)? as usize;
    let unicode = r.u8(0x49)? != 0;
    let facesets = r.i32(0x50)?.max(0) as usize;
    let layouts = r.i32(0x54)?.max(0) as usize;
    let mut at = 0x80 + dummies * 0x40;
    let mut out = Flver { version, bbox, ..Default::default() };
    for k in 0..materials {
        out.material_names.push(r.name(r.u32(at + k * 0x20)? as usize, unicode));
    }
    at += materials * 0x20;
    for k in 0..bones {
        let e = at + k * 0x80;
        let (t, name, rot, parent, s) = (r.v3(e)?, r.u32(e + 0x0C)?, r.v3(e + 0x10)?, r.i16(e + 0x1C)?, r.v3(e + 0x20)?);
        out.bones.push(Bone { name: r.name(name as usize, unicode), parent: parent as i32, local: bone_local(t, rot, s), bind: Mat4::IDENTITY });
    }
    // the bind pose: parents first (a parent index past the bone, or a loop, leaves the bone at its local)
    for k in 0..out.bones.len() {
        let mut chain = vec![k];
        let mut p = out.bones[k].parent;
        while p >= 0 && (p as usize) < out.bones.len() && chain.len() <= out.bones.len() {
            chain.push(p as usize);
            p = out.bones[p as usize].parent;
        }
        out.bones[k].bind = chain.iter().rev().fold(Mat4::IDENTITY, |m, j| m * out.bones[*j].local);
    }
    at += bones * 0x80;
    let mesh_at = at;
    at += meshes * 0x30;
    let faceset_at = at;
    at += facesets * 0x20;
    let vbuffer_at = at;
    at += vbuffers * 0x20;
    let layout_at = at;
    // the layouts' members
    let mut layout_members: Vec<Vec<Member>> = Vec::new();
    for k in 0..layouts {
        let e = layout_at + k * 0x10;
        let (n, members_at) = (r.i32(e)?.max(0) as usize, r.u32(e + 0x0C)? as usize);
        let mut list = Vec::new();
        for m in 0..n.min(64) {
            let x = members_at + m * 0x14;
            list.push(Member { buffer: 0, offset: r.i32(x + 4)?.max(0) as usize, kind: r.u32(x + 8)?, semantic: r.u32(x + 12)?, index: r.i32(x + 16)? });
        }
        layout_members.push(list);
    }
    for k in 0..meshes {
        let e = mesh_at + k * 0x30;
        let dynamic = r.u8(e)? != 0;
        let material = r.i32(e + 4)?;
        let default_bone = r.i32(e + 0x10)?;
        let (bone_count, bone_at) = (r.i32(e + 0x14)?.max(0) as usize, r.u32(e + 0x1C)? as usize);
        let (fs_count, fs_at) = (r.i32(e + 0x20)?.max(0) as usize, r.u32(e + 0x24)? as usize);
        let (vb_count, vb_at) = (r.i32(e + 0x28)?.max(0) as usize, r.u32(e + 0x2C)? as usize);
        let local_bones: Vec<i32> = (0..bone_count).map(|j| r.i32(bone_at + j * 4)).collect::<Result<_, _>>()?;
        let global = |j: u32| -> u16 {
            if local_bones.is_empty() {
                j as u16
            } else {
                local_bones.get(j as usize).copied().unwrap_or(0).max(0) as u16
            }
        };
        // its vertex buffers (all of the same vertex count) and their members
        let mut members: Vec<(Member, usize, usize)> = Vec::new(); // (member, buffer data offset, stride)
        let mut count = usize::MAX;
        for j in 0..vb_count.min(4) {
            let vb = r.i32(vb_at + j * 4)?.max(0) as usize;
            if vb >= vbuffers {
                continue;
            }
            let v = vbuffer_at + vb * 0x20;
            let (layout, stride, n, offset) = (r.i32(v + 4)?.max(0) as usize, r.i32(v + 8)?.max(0) as usize, r.i32(v + 12)?.max(0) as usize, r.u32(v + 0x1C)? as usize);
            count = count.min(n);
            for m in layout_members.get(layout).into_iter().flatten() {
                members.push((Member { buffer: j, ..*m }, data_offset + offset, stride));
            }
        }
        if count == usize::MAX {
            count = 0;
        }
        let mut mesh = Mesh { dynamic, material, default_bone, ..Default::default() };
        mesh.positions = vec![Vec3::ZERO; count];
        mesh.normals = vec![Vec3::Y; count];
        mesh.joints = vec![[0; 4]; count];
        mesh.weights = vec![[0.0; 4]; count];
        let mut normal_w = vec![0u32; count];
        let (mut have_w, mut have_j) = (false, false);
        for (m, base, stride) in &members {
            let at = |i: usize| base + i * stride + m.offset;
            match (m.semantic, m.index) {
                (0, 0) => {
                    for i in 0..count {
                        mesh.positions[i] = r.v3(at(i))?;
                    }
                }
                (1, 0) => {
                    have_w = true;
                    for i in 0..count {
                        let p = at(i);
                        mesh.weights[i] = match m.kind {
                            0x1A | 0x2E => [r.i16(p)? as f32 / 32767.0, r.i16(p + 2)? as f32 / 32767.0, r.i16(p + 4)? as f32 / 32767.0, r.i16(p + 6)? as f32 / 32767.0],
                            0x03 => [r.f32(p)?, r.f32(p + 4)?, r.f32(p + 8)?, r.f32(p + 12)?],
                            _ => [r.u8(p)? as f32 / 255.0, r.u8(p + 1)? as f32 / 255.0, r.u8(p + 2)? as f32 / 255.0, r.u8(p + 3)? as f32 / 255.0],
                        };
                    }
                }
                (2, 0) => {
                    have_j = true;
                    for i in 0..count {
                        let p = at(i);
                        let raw: [u32; 4] = match m.kind {
                            0x18 | 0x1A | 0x2E => [r.u16(p)? as u32, r.u16(p + 2)? as u32, r.u16(p + 4)? as u32, r.u16(p + 6)? as u32],
                            _ => [r.u8(p)? as u32, r.u8(p + 1)? as u32, r.u8(p + 2)? as u32, r.u8(p + 3)? as u32],
                        };
                        mesh.joints[i] = raw.map(global);
                    }
                }
                (3, 0) => {
                    for i in 0..count {
                        let p = at(i);
                        let (n, w) = match m.kind {
                            0x02 => (r.v3(p)?, 0),
                            0x03 => (r.v3(p)?, r.f32(p + 12)? as u32),
                            0x1A | 0x2E => {
                                let s = |o: usize| -> Result<f32, String> { Ok((r.u16(p + o)? as f32 - 32767.0) / 32767.0) };
                                (Vec3::new(s(0)?, s(2)?, s(4)?), r.u16(p + 6)? as u32)
                            }
                            _ => {
                                let c = |o: usize| -> Result<f32, String> { Ok((r.u8(p + o)? as f32 - 127.0) / 127.0) };
                                (Vec3::new(c(0)?, c(1)?, c(2)?), r.u8(p + 3)? as u32)
                            }
                        };
                        mesh.normals[i] = n.normalize_or(Vec3::Y);
                        normal_w[i] = w;
                    }
                }
                _ => {}
            }
        }
        if !dynamic || !have_w || !have_j {
            // rigid: each vertex on the bone its normal's w names (or the mesh's default bone)
            for i in 0..count {
                let bone = if !dynamic && normal_w[i] < 0xFFFF { global(normal_w[i]) } else if default_bone >= 0 { default_bone as u16 } else { 0 };
                mesh.joints[i] = [bone, 0, 0, 0];
                mesh.weights[i] = [1.0, 0.0, 0.0, 0.0];
            }
        } else {
            for w in mesh.weights.iter_mut() {
                let s: f32 = w.iter().sum();
                if s > 1e-4 {
                    w.iter_mut().for_each(|x| *x /= s);
                } else {
                    *w = [1.0, 0.0, 0.0, 0.0];
                }
            }
        }
        // the full-detail face set
        let sets: Vec<usize> = (0..fs_count).map(|j| r.i32(fs_at + j * 4).map(|v| v.max(0) as usize)).collect::<Result<_, _>>()?;
        let chosen = sets.iter().copied().filter(|s| *s < facesets).find(|s| r.u32(faceset_at + s * 0x20).is_ok_and(|f| f & (FS_LOD1 | FS_LOD2 | FS_BLUR | FS_EDGE) == 0)).or_else(|| sets.first().copied().filter(|s| *s < facesets));
        if let Some(s) = chosen {
            let e = faceset_at + s * 0x20;
            let flags = r.u32(e)?;
            if flags & FS_EDGE != 0 {
                out.notes.push(format!("mesh {k}: edge-compressed faces are not read"));
            } else {
                let strip = r.u8(e + 4)? != 0;
                let (n, idx_at) = (r.i32(e + 8)?.max(0) as usize, data_offset + r.u32(e + 0x0C)? as usize);
                let mut size = if version > 0x20005 { r.i32(e + 0x18)?.max(0) as usize } else { 0 };
                if size == 0 {
                    size = header_index_size;
                }
                if size == 0 {
                    size = 16;
                }
                let index = |i: usize| -> Result<u32, String> { if size == 32 { r.u32(idx_at + i * 4) } else { r.u16(idx_at + i * 2).map(|v| v as u32) } };
                let restart = if size == 32 { u32::MAX } else { 0xFFFF };
                if strip {
                    let mut flip = false;
                    let mut window: Vec<u32> = Vec::new();
                    for i in 0..n {
                        let v = index(i)?;
                        if v == restart {
                            window.clear();
                            flip = false;
                            continue;
                        }
                        window.push(v);
                        if window.len() >= 3 {
                            let (a, b2, c) = (window[window.len() - 3], window[window.len() - 2], window[window.len() - 1]);
                            if a != b2 && b2 != c && a != c {
                                if flip {
                                    mesh.indices.extend([a, c, b2]);
                                } else {
                                    mesh.indices.extend([a, b2, c]);
                                }
                            }
                            flip = !flip;
                        }
                    }
                } else {
                    for i in 0..n / 3 * 3 {
                        mesh.indices.push(index(i)?);
                    }
                }
                let before = mesh.indices.len();
                let verts = count as u32;
                // (a triangle naming a vertex the mesh has not is dropped)
                let tris: Vec<u32> = mesh.indices.chunks_exact(3).filter(|t| t.iter().all(|v| *v < verts)).flatten().copied().collect();
                if tris.len() != before {
                    out.notes.push(format!("mesh {k}: {} triangle(s) out of range dropped", (before - tris.len()) / 3));
                }
                mesh.indices = tris;
            }
        }
        out.meshes.push(mesh);
    }
    Ok(out)
}

impl Flver {
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }

    /// The bone of a name.
    pub fn bone(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }
}

/// A 3x4 row-major matrix as the game keeps its bone matrices (rows; the translation in the fourth column) as a Mat4.
pub fn mat_from_rows34(f: &[f32]) -> Mat4 {
    Mat4::from_cols(Vec4::new(f[0], f[4], f[8], 0.0), Vec4::new(f[1], f[5], f[9], 0.0), Vec4::new(f[2], f[6], f[10], 0.0), Vec4::new(f[3], f[7], f[11], 1.0))
}

/// Rotation + translation of a matrix (scale dropped), for logs and tests.
pub fn place(m: &Mat4) -> (Vec3, Quat) {
    let (_, q, t) = m.to_scale_rotation_translation();
    (t, q)
}

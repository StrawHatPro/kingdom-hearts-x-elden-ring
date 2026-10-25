//! A skinned model of the Sora pack (`sora.glb`, `weapon/kingdom_key.glb`) read into plain
//! arrays: one vertex buffer and one index buffer for all its primitives, the skin's joints as
//! bone names with their inverse bind matrices. Pack space throughout (glTF: metres, Y up).

use std::path::Path;

use glam::Mat4;
use serde_json::Value;

/// One vertex as the vertex shader reads it (88 bytes; `LAYOUT` in pipeline.rs names the offsets).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
    pub uv2: [f32; 2],
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

/// One primitive: a range of the index buffer over a range of the vertex buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct Primitive {
    /// The glTF material's name: the material slot (`p_ex001_mat0`).
    pub material: String,
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: i32,
    pub vertex_count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub primitives: Vec<Primitive>,
    /// The skin's joints, in the order the vertices index them: the bone's name and its inverse
    /// bind matrix.
    pub joints: Vec<(String, Mat4)>,
}

impl Model {
    pub fn load(path: &Path) -> Result<Model, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Model::from_glb(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn from_glb(bytes: &[u8]) -> Result<Model, String> {
        let u32_at = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
        if bytes.get(0..4) != Some(b"glTF".as_slice()) {
            return Err("not a .glb file".to_string());
        }
        let json_length = u32_at(12).ok_or("truncated")?;
        let json: Value = serde_json::from_slice(bytes.get(20..20 + json_length).ok_or("truncated")?).map_err(|e| e.to_string())?;
        let bin_length = u32_at(20 + json_length).ok_or("no binary chunk")?;
        let bin_start = 20 + json_length + 8;
        let bin = bytes.get(bin_start..bin_start + bin_length).ok_or("truncated binary chunk")?;
        let index = |v: &Value| v.as_u64().map(|x| x as usize);

        // An accessor as numbers: `width` components an element, whatever its component type.
        let read = |accessor: usize, width: usize| -> Result<Vec<f64>, String> {
            let a = json["accessors"].get(accessor).ok_or("accessor out of range")?;
            let view = json["bufferViews"].get(index(&a["bufferView"]).ok_or("accessor without a buffer view")?).ok_or("buffer view out of range")?;
            let component = index(&a["componentType"]).ok_or("accessor without a component type")?;
            let size = match component {
                5120 | 5121 => 1,
                5122 | 5123 => 2,
                5125 | 5126 => 4,
                other => return Err(format!("component type {other}")),
            };
            let start = index(&view["byteOffset"]).unwrap_or(0) + index(&a["byteOffset"]).unwrap_or(0);
            let stride = index(&view["byteStride"]).unwrap_or(size * width);
            let count = index(&a["count"]).unwrap_or(0);
            let normalized = a["normalized"].as_bool().unwrap_or(false);
            let mut out = Vec::with_capacity(count * width);
            for i in 0..count {
                for c in 0..width {
                    let at = start + i * stride + c * size;
                    let b = bin.get(at..at + size).ok_or("accessor runs past the binary chunk")?;
                    out.push(match component {
                        5120 => (b[0] as i8) as f64 / if normalized { 127.0 } else { 1.0 },
                        5121 => b[0] as f64 / if normalized { 255.0 } else { 1.0 },
                        5122 => i16::from_le_bytes([b[0], b[1]]) as f64 / if normalized { 32767.0 } else { 1.0 },
                        5123 => u16::from_le_bytes([b[0], b[1]]) as f64 / if normalized { 65535.0 } else { 1.0 },
                        5125 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
                        _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
                    });
                }
            }
            Ok(out)
        };

        let mesh = json["meshes"].get(0).ok_or("no mesh")?;
        // (ER party S4) a STATIC mesh (no skin at all: the team props' meteor o_ex210_0): every vertex on one identity joint
        let unskinned = json.get("skins").and_then(Value::as_array).is_none_or(|s| s.is_empty());
        let mut model = Model::default();
        for primitive in mesh["primitives"].as_array().ok_or("mesh without primitives")? {
            if primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
                return Err("a primitive is not a triangle list".to_string());
            }
            let attributes = &primitive["attributes"];
            let attribute = |name: &str, width: usize| -> Result<Option<Vec<f64>>, String> {
                match index(&attributes[name]) {
                    Some(accessor) => read(accessor, width).map(Some),
                    None => Ok(None),
                }
            };
            let position = attribute("POSITION", 3)?.ok_or("primitive without POSITION")?;
            let count = position.len() / 3;
            let normal = attribute("NORMAL", 3)?.ok_or("primitive without NORMAL")?;
            let tangent = attribute("TANGENT", 4)?;
            let uv0 = attribute("TEXCOORD_0", 2)?.ok_or("primitive without TEXCOORD_0")?;
            // A set the mesh lacks repeats its last one (the engine's vertex factory does the same).
            let uv1 = attribute("TEXCOORD_1", 2)?.unwrap_or_else(|| uv0.clone());
            let uv2 = attribute("TEXCOORD_2", 2)?.unwrap_or_else(|| uv1.clone());
            let joints = match attribute("JOINTS_0", 4)? {
                Some(j) => j,
                None if unskinned => vec![0.0; count * 4],
                None => return Err("primitive without JOINTS_0".to_string()),
            };
            let weights = match attribute("WEIGHTS_0", 4)? {
                Some(w) => w,
                None if unskinned => (0..count * 4).map(|k| if k % 4 == 0 { 1.0 } else { 0.0 }).collect(),
                None => return Err("primitive without WEIGHTS_0".to_string()),
            };
            if [normal.len() / 3, uv0.len() / 2, uv1.len() / 2, uv2.len() / 2, joints.len() / 4, weights.len() / 4].iter().any(|n| *n != count) || tangent.as_ref().is_some_and(|t| t.len() / 4 != count) {
                return Err("a primitive's attributes differ in length".to_string());
            }
            let base_vertex = model.vertices.len();
            for i in 0..count {
                let f3 = |v: &[f64]| [v[i * 3] as f32, v[i * 3 + 1] as f32, v[i * 3 + 2] as f32];
                let f2 = |v: &[f64]| [v[i * 2] as f32, v[i * 2 + 1] as f32];
                let f4 = |v: &[f64]| [v[i * 4] as f32, v[i * 4 + 1] as f32, v[i * 4 + 2] as f32, v[i * 4 + 3] as f32];
                let mut w = f4(&weights);
                let sum: f32 = w.iter().sum();
                if sum > 1.0e-6 {
                    w = w.map(|x| x / sum);
                } else {
                    w = [1.0, 0.0, 0.0, 0.0];
                }
                model.vertices.push(Vertex {
                    position: f3(&position),
                    normal: f3(&normal),
                    tangent: tangent.as_ref().map_or([1.0, 0.0, 0.0, 1.0], |t| f4(t)),
                    uv0: f2(&uv0),
                    uv1: f2(&uv1),
                    uv2: f2(&uv2),
                    joints: [joints[i * 4] as u16, joints[i * 4 + 1] as u16, joints[i * 4 + 2] as u16, joints[i * 4 + 3] as u16],
                    weights: w,
                });
            }
            let indices = read(index(&primitive["indices"]).ok_or("primitive without indices")?, 1)?;
            if indices.iter().any(|i| *i as usize >= count) {
                return Err("an index runs past its primitive's vertices".to_string());
            }
            let material = index(&primitive["material"]).and_then(|m| json["materials"].get(m)).and_then(|m| m["name"].as_str()).unwrap_or("").to_string();
            model.primitives.push(Primitive { material, first_index: model.indices.len() as u32, index_count: indices.len() as u32, base_vertex: base_vertex as i32, vertex_count: count as u32 });
            model.indices.extend(indices.iter().map(|i| *i as u32));
        }
        if unskinned {
            // the mesh node's place in the file (its parents' matrices: the exporter's UE cm -> glTF m scale) baked in, so
            // the vertices are in the pack's space as a skinned mesh's bind pose is
            let nodes = json["nodes"].as_array().cloned().unwrap_or_default();
            let local = |n: &Value| -> Mat4 {
                if let Some(m) = n["matrix"].as_array().filter(|m| m.len() == 16) {
                    return Mat4::from_cols_array(&std::array::from_fn(|k| m[k].as_f64().unwrap_or(0.0) as f32));
                }
                let f = |k: &str, i: usize, d: f32| n[k].get(i).and_then(Value::as_f64).map_or(d, |x| x as f32);
                Mat4::from_scale_rotation_translation(glam::Vec3::new(f("scale", 0, 1.0), f("scale", 1, 1.0), f("scale", 2, 1.0)), glam::Quat::from_xyzw(f("rotation", 0, 0.0), f("rotation", 1, 0.0), f("rotation", 2, 0.0), f("rotation", 3, 1.0)), glam::Vec3::new(f("translation", 0, 0.0), f("translation", 1, 0.0), f("translation", 2, 0.0)))
            };
            let parent_of = |k: usize| nodes.iter().position(|n| n["children"].as_array().is_some_and(|c| c.iter().any(|x| x.as_u64() == Some(k as u64))));
            if let Some(mut k) = nodes.iter().position(|n| n["mesh"].as_u64() == Some(0)) {
                let mut world = local(&nodes[k]);
                let mut guard = 0;
                while let (Some(p), true) = (parent_of(k), guard < 64) {
                    world = local(&nodes[p]) * world;
                    k = p;
                    guard += 1;
                }
                let normal = world.inverse().transpose();
                for v in &mut model.vertices {
                    v.position = world.transform_point3(glam::Vec3::from(v.position)).to_array();
                    v.normal = normal.transform_vector3(glam::Vec3::from(v.normal)).normalize_or_zero().to_array();
                    let t = world.transform_vector3(glam::Vec3::new(v.tangent[0], v.tangent[1], v.tangent[2])).normalize_or_zero();
                    v.tangent = [t.x, t.y, t.z, v.tangent[3]];
                }
            }
            model.joints.push((String::new(), Mat4::IDENTITY));
            return Ok(model);
        }
        let skin = json["skins"].get(0).ok_or("no skin")?;
        let joint_nodes: Vec<usize> = skin["joints"].as_array().ok_or("skin without joints")?.iter().filter_map(index).collect();
        let inverse_bind = read(index(&skin["inverseBindMatrices"]).ok_or("skin without inverse bind matrices")?, 16)?;
        if inverse_bind.len() != joint_nodes.len() * 16 {
            return Err("the inverse bind matrices do not match the joints".to_string());
        }
        for (k, node) in joint_nodes.iter().enumerate() {
            let name = json["nodes"].get(*node).and_then(|n| n["name"].as_str()).ok_or("a joint node without a name")?.to_string();
            let mut m = [0.0f32; 16];
            for (c, slot) in m.iter_mut().enumerate() {
                *slot = inverse_bind[k * 16 + c] as f32;
            }
            model.joints.push((name, Mat4::from_cols_array(&m)));
        }
        if model.vertices.iter().any(|v| v.joints.iter().any(|j| *j as usize >= model.joints.len())) {
            return Err("a vertex names a joint the skin does not have".to_string());
        }
        Ok(model)
    }

    /// The vertices as the bytes of the vertex buffer.
    pub fn vertex_bytes(&self) -> &[u8] {
        // SAFETY: `Vertex` is `repr(C)` of plain numbers without padding (88 bytes, asserted in the tests).
        unsafe { std::slice::from_raw_parts(self.vertices.as_ptr() as *const u8, std::mem::size_of_val(self.vertices.as_slice())) }
    }

    pub fn index_bytes(&self) -> &[u8] {
        // SAFETY: plain u32s.
        unsafe { std::slice::from_raw_parts(self.indices.as_ptr() as *const u8, std::mem::size_of_val(self.indices.as_slice())) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-triangle skinned .glb with two UV sets and one joint.
    fn tiny_glb() -> Vec<u8> {
        let mut bin: Vec<u8> = Vec::new();
        let mut views: Vec<(usize, usize)> = Vec::new();
        let mut push = |bytes: Vec<u8>, bin: &mut Vec<u8>| {
            views.push((bin.len(), bytes.len()));
            bin.extend(bytes);
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
        };
        let floats = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
        push(floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]), &mut bin); // 0 POSITION
        push(floats(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]), &mut bin); // 1 NORMAL
        push(floats(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]), &mut bin); // 2 TEXCOORD_0
        push(floats(&[0.5, 0.5, 0.25, 0.5, 0.5, 0.25]), &mut bin); // 3 TEXCOORD_1
        push([0u16; 12].iter().flat_map(|x| x.to_le_bytes()).collect(), &mut bin); // 4 JOINTS_0
        push(floats(&[2.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]), &mut bin); // 5 WEIGHTS_0
        push([0u16, 1, 2].iter().flat_map(|x| x.to_le_bytes()).collect(), &mut bin); // 6 indices
        push(floats(&Mat4::from_translation(glam::Vec3::new(0.0, -1.0, 0.0)).to_cols_array()), &mut bin); // 7 inverse bind
        let buffer_views: Vec<String> = views.iter().map(|(o, l)| format!(r#"{{"buffer":0,"byteOffset":{o},"byteLength":{l}}}"#)).collect();
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"buffers":[{{"byteLength":{}}}],"bufferViews":[{}],
            "accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"}},{{"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"}},
            {{"bufferView":2,"componentType":5126,"count":3,"type":"VEC2"}},{{"bufferView":3,"componentType":5126,"count":3,"type":"VEC2"}},
            {{"bufferView":4,"componentType":5123,"count":3,"type":"VEC4"}},{{"bufferView":5,"componentType":5126,"count":3,"type":"VEC4"}},
            {{"bufferView":6,"componentType":5123,"count":3,"type":"SCALAR"}},{{"bufferView":7,"componentType":5126,"count":1,"type":"MAT4"}}],
            "materials":[{{"name":"slot_a"}}],
            "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2,"TEXCOORD_1":3,"JOINTS_0":4,"WEIGHTS_0":5}},"indices":6,"material":0}}]}}],
            "nodes":[{{"name":"root_bone"}},{{"name":"mesh","mesh":0,"skin":0}}],"skins":[{{"joints":[0],"inverseBindMatrices":7}}]}}"#,
            bin.len(),
            buffer_views.join(",")
        );
        let mut json = json.into_bytes();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut glb = Vec::new();
        glb.extend(b"glTF");
        glb.extend(2u32.to_le_bytes());
        glb.extend(((12 + 8 + json.len() + 8 + bin.len()) as u32).to_le_bytes());
        glb.extend((json.len() as u32).to_le_bytes());
        glb.extend(b"JSON");
        glb.extend(&json);
        glb.extend((bin.len() as u32).to_le_bytes());
        glb.extend(b"BIN\0");
        glb.extend(&bin);
        glb
    }

    #[test]
    fn a_glb_is_read_into_one_buffer() {
        assert_eq!(std::mem::size_of::<Vertex>(), 88);
        let model = Model::from_glb(&tiny_glb()).unwrap();
        assert_eq!((model.vertices.len(), model.indices.clone(), model.primitives.len()), (3, vec![0, 1, 2], 1));
        assert_eq!(model.primitives[0], Primitive { material: "slot_a".to_string(), first_index: 0, index_count: 3, base_vertex: 0, vertex_count: 3 });
        // Weights are normalised; an all-zero vertex is bound to its first joint; a missing third
        // UV set repeats the second, a missing tangent is +X.
        assert_eq!((model.vertices[0].weights, model.vertices[1].weights, model.vertices[2].weights), ([1.0, 0.0, 0.0, 0.0], [0.5, 0.5, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]));
        assert_eq!((model.vertices[1].uv1, model.vertices[1].uv2, model.vertices[1].tangent), ([0.25, 0.5], [0.25, 0.5], [1.0, 0.0, 0.0, 1.0]));
        assert_eq!(model.joints.len(), 1);
        assert_eq!((model.joints[0].0.as_str(), model.joints[0].1.w_axis.y), ("root_bone", -1.0));
        assert_eq!((model.vertex_bytes().len(), model.index_bytes().len()), (3 * 88, 12));
    }
}

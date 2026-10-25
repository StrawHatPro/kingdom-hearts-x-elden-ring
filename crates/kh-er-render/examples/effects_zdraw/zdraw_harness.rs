//! (ZDRAW) THE OFFLINE PROOF OF ZETTAFLARE'S TARGET DRAWN FROM ITS OWN MODEL (examples/effects.rs `--zdraw CHR`): a real
//! Elden Ring character model read from the game's files (kh-erfile, read-only) stands where the first `--erchars` character
//! stands, in its bind pose or a hurt pose (`--zdraw-pose bind|hurt`, default hurt: a heavy hurt frozen halfway made of named
//! bone turns, as ZETTA freezes the game's heaviest reaction; the harness has no ER animation reader); `--zdraw-parts
//! bd_m_1100,..`: worn equipment parts on a `c0000` (a human), posed by its bones, drawn by the "game" (lit flat, depth and the characters' stencil, as Elden Ring draws
//! its enemy) until the finale hides it; the finale's mask is fed exactly as kh-er zdraw.rs feeds it (kh-er-render zdraw.rs
//! `skin` + `Driver`). `--zdraw-groups u,0,21`: the display groups shown (u = meshes without one; default every mesh: the
//! harness has no NpcParam), `--zdraw-skip WORDS`: meshes whose material names hold one of the words left out (a stand-in for
//! parts the game collapses by their bones' scale, e.g. Malenia's ground petals).

use std::ffi::c_void;

use glam::{Mat4, Quat, Vec3};
use kh_er_render::finale::FinaleTarget;
use kh_er_render::zdraw::{Ask, Driver, MeshPart, Model, Tri};
use windows::core::PCSTR;
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

fn model_of(name: &str, f: &kh_erfile::Flver) -> Model {
    let mut m = Model { name: name.to_string(), ..Default::default() };
    m.bone_names = f.bones.iter().map(|b| b.name.clone()).collect();
    m.inv_bind = f.bones.iter().map(|b| b.bind.inverse()).collect();
    for mesh in &f.meshes {
        let base = m.positions.len() as u32;
        m.positions.extend(&mesh.positions);
        m.joints.extend(&mesh.joints);
        m.weights.extend(&mesh.weights);
        let first = m.indices.len();
        m.indices.extend(mesh.indices.iter().map(|i| i + base));
        let material = f.material_names.get(mesh.material.max(0) as usize).cloned().unwrap_or_default();
        m.meshes.push(MeshPart { first, count: mesh.indices.len(), group: kh_er_render::zdraw::disp_group(&material), material });
    }
    // (as kh-er zdraw.rs: a model over the frame's budget reduced once, in its bind pose)
    m.reduced(kh_er_render::zdraw::DRAW_TRIS)
}

/// A HEAVY HURT, frozen about halfway (the user's ask: the target held in its heaviest reaction, visibly hurt; ZETTA freezes
/// the game's own animation there, the harness has no ER animation reader): thrown back by the blow, head back, arms flung
/// up and back, one knee up. (bone, model-space axis, degrees) turned about the joint, in order, parents first. Model space:
/// y up, the model faces -z, +x is its left.
const HURT: [(&str, Vec3, f32); 20] = [
    ("Spine", Vec3::X, 14.0),
    ("Spine1", Vec3::X, 12.0),
    ("Spine2", Vec3::X, 8.0),
    ("Neck", Vec3::X, 14.0),
    ("Head", Vec3::X, 22.0),
    ("L_UpperArm", Vec3::Z, 40.0),
    ("L_UpperArm", Vec3::Y, -35.0),
    ("R_UpperArm", Vec3::Z, -40.0),
    ("R_UpperArm", Vec3::Y, 35.0),
    ("L_Forearm", Vec3::Z, 25.0),
    ("R_Forearm", Vec3::Z, -25.0),
    ("L_Hand", Vec3::Z, 15.0),
    ("R_Hand", Vec3::Z, -15.0),
    ("L_Thigh", Vec3::X, 30.0),
    ("L_Calf", Vec3::X, -45.0),
    ("L_Foot", Vec3::X, 15.0),
    ("R_Thigh", Vec3::X, -12.0),
    ("R_Calf", Vec3::X, -18.0),
    ("L_Clavicle", Vec3::Z, 10.0),
    ("R_Clavicle", Vec3::Z, -10.0),
];

/// The bones in model space: the bind, or the bind with the heavy hurt's turns.
fn pose(f: &kh_erfile::Flver, hurt: bool) -> Vec<Mat4> {
    let mut out: Vec<Mat4> = Vec::with_capacity(f.bones.len());
    for (k, b) in f.bones.iter().enumerate() {
        let parent = if b.parent >= 0 && (b.parent as usize) < k { out[b.parent as usize] } else { Mat4::IDENTITY };
        let mut p = if b.parent >= 0 && (b.parent as usize) < k { parent * b.local } else { b.bind };
        if hurt {
            for (_, axis, deg) in HURT.iter().filter(|h| h.0 == b.name) {
                let at = p.w_axis.truncate();
                p = Mat4::from_translation(at) * Mat4::from_quat(Quat::from_axis_angle(*axis, deg.to_radians())) * Mat4::from_translation(-at) * p;
            }
        }
        out.push(p);
    }
    out
}

/// (c0000) A worn part posed by its wearer: each of its bones the body's bone of that name, else its parent's with its own
/// local turn, else its own bind (kh-er zdraw.rs poses a part by its own model item, checked to lie on the body's bones).
fn pose_part(part: &kh_erfile::Flver, body: &kh_erfile::Flver, body_posed: &[Mat4]) -> Vec<Mat4> {
    let mut out: Vec<Mat4> = Vec::with_capacity(part.bones.len());
    for (k, b) in part.bones.iter().enumerate() {
        let p = match body.bone(&b.name) {
            Some(j) => body_posed[j],
            None if b.parent >= 0 && (b.parent as usize) < k => out[b.parent as usize] * b.local,
            None => b.bind,
        };
        out.push(p);
    }
    out
}

struct BodyGpu {
    root: ID3D12RootSignature,
    pipeline: ID3D12PipelineState,
    upload: ID3D12Resource,
    mapped: *mut u8,
    bytes: usize,
    turn: usize,
}

pub struct ZHarness {
    pub chr: String,
    model: Model,
    posed: Vec<Mat4>,
    /// (c0000) its worn parts, posed
    parts: Vec<(Model, Vec<Mat4>)>,
    groups: Option<Vec<Option<u8>>>,
    skip: Vec<String>,
    feet: Vec3,
    yaw: f32,
    driver: Driver,
    gpu: Option<BodyGpu>,
    body: Vec<Tri>,
    hidden: bool,
}

/// Elden Ring's world (m) to game metres (kh-er coords.rs with the origin at 0).
fn to_game(p: Vec3) -> Vec3 {
    Vec3::new(p.z, p.x, p.y)
}

impl ZHarness {
    /// `feet` game cm, `yaw` KH degrees (the `--erchars` row of the target).
    pub fn new(chr: &str, hurt: bool, groups: Option<&str>, skip: Option<&str>, parts: Option<&str>, feet: Vec3, yaw: f32) -> ZHarness {
        let game = kh_erfile::archive::default_game_dir();
        let started = std::time::Instant::now();
        let mut lib = kh_erfile::Library::open(&game).unwrap_or_else(|e| panic!("zdraw: the game's archives: {e}"));
        let f = lib.character(chr).unwrap_or_else(|e| panic!("zdraw: {chr}: {e}"));
        let model = model_of(chr, &f);
        let posed = pose(&f, hurt);
        let parts: Vec<(Model, Vec<Mat4>)> = parts.into_iter().flat_map(|p| p.split(',')).filter(|n| !n.trim().is_empty()).map(|n| {
            let pf = lib.part(n.trim()).unwrap_or_else(|e| panic!("zdraw: part {n}: {e}"));
            println!("zdraw: part {n}: {} bones, {} triangles", pf.bones.len(), pf.triangle_count());
            (model_of(n, &pf), pose_part(&pf, &f, &posed))
        }).collect();
        let groups = groups.map(|g| g.split(',').filter_map(|x| if x.trim() == "u" { Some(None) } else { x.trim().parse().ok().map(Some) }).collect());
        let skip = skip.map(|s| s.split(',').map(|w| w.trim().to_ascii_lowercase()).filter(|w| !w.is_empty()).collect()).unwrap_or_default();
        println!("zdraw: {chr} read from the game's files in {:.2} s: {} bones, {} meshes, {} triangles; pose {}", started.elapsed().as_secs_f32(), model.bone_names.len(), model.meshes.len(), model.indices.len() / 3, if hurt { "heavy hurt, frozen halfway (named bone turns)" } else { "bind" });
        ZHarness { chr: chr.to_string(), model, posed, parts, groups, skip, feet, yaw, driver: { let mut d = Driver::default(); d.threaded = true; d }, gpu: None, body: Vec::new(), hidden: false }
    }

    /// The body as drawn this step (game metres).
    fn skin(&self) -> Vec<Tri> {
        let feet_er = Vec3::new(self.feet.y, self.feet.z, self.feet.x) / 100.0;
        let place = Mat4::from_translation(feet_er) * Mat4::from_quat(Quat::from_rotation_y(self.yaw.to_radians() + std::f32::consts::PI));
        let world: Vec<Mat4> = self.posed.iter().map(|p| place * *p).collect();
        // the display groups and skipped meshes, as a model with only the shown meshes
        let mut model = self.model.clone();
        model.meshes.retain(|m| {
            let group_ok = self.groups.as_ref().is_none_or(|g| g.contains(&m.group));
            let skip = self.skip.iter().any(|w| m.material.to_ascii_lowercase().contains(w.as_str()));
            group_ok && !skip
        });
        let mut tris = kh_er_render::zdraw::skin(&model, &world, &|_| true, &|p| to_game(p));
        for (m, posed) in &self.parts {
            let w: Vec<Mat4> = posed.iter().map(|p| place * *p).collect();
            tris.extend(kh_er_render::zdraw::skin(m, &w, &|_| true, &|p| to_game(p)));
        }
        tris
    }

    /// One step: the finale's target (id 7, examples/effects.rs `erchr_frame`) drawn from its own model, handed to the renderer.
    pub fn step(&mut self, finale: Option<&FinaleTarget>) {
        if self.body.is_empty() {
            self.body = self.skin();
            let (lo, hi) = self.body.iter().flat_map(|t| t.p).fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(p), b.max(p)));
            println!("zdraw: {}: {} triangles shown, from {lo:.2} to {hi:.2} m (game axes)", self.chr, self.body.len());
        }
        let Some(t) = finale else {
            kh_er_render::zdraw::set(Vec::new());
            return;
        };
        let ask = Ask { id: t.id, silhouette: t.silhouette, crumble: t.crumble, feet: t.body.feet, radius: t.body.radius, half_height: t.body.half_height, push: t.push };
        if t.silhouette && t.crumble > 0.0 {
            self.hidden = true;
        }
        let soup = self.driver.step(&ask, if self.driver.has(t.id) && t.crumble > 0.0 { None } else { Some(&self.body) });
        if let Some((id, n, took)) = self.driver.last_cut.take() {
            println!("zdraw: target {id}: broken apart from its own model ({n} triangles cut in {:.1} ms)", took.as_secs_f64() * 1000.0);
        }
        // its body in the light (kh-er zdraw.rs: while the game still draws it)
        let mut bodies = Vec::new();
        if t.crumble <= 0.0 && t.hidden_for.is_none() && !(t.silhouette && t.dark >= 0.35) {
            let mut b = Vec::new();
            kh_er_render::zdraw::body_soup(&self.body, &mut b);
            bodies.push((t.id, t.immerse, b));
        }
        kh_er_render::zdraw::set_frame(soup.map(|s| vec![(t.id, s)]).unwrap_or_default(), bodies);
    }

    /// The "game's" own draw of the body (into the bound scene target and depth, stencil 0x50), until the finale hides it.
    /// Leaves the root signature, pipeline and root CBVs changed.
    pub unsafe fn draw_body(&mut self, device: &ID3D12Device, list: &ID3D12GraphicsCommandList, clip: &Mat4, scene_format: DXGI_FORMAT, shader_dir: &std::path::Path) {
        if self.hidden || self.body.is_empty() {
            return;
        }
        if self.gpu.is_none() {
            let read = |n: &str| std::fs::read(shader_dir.join(n)).unwrap_or_else(|e| panic!("zdraw: {}: {e}", shader_dir.join(n).display()));
            let (vs, ps) = (read("zdraw_body.vs.dxil"), read("zdraw_body.ps.dxil"));
            let cbv = |r: u32| D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: r, RegisterSpace: 0 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL };
            let params = [cbv(0), cbv(1)];
            let desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: 2, pParameters: params.as_ptr(), Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT, ..Default::default() };
            let mut blob: Option<windows::Win32::Graphics::Direct3D::ID3DBlob> = None;
            D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None).expect("zdraw: root signature");
            let blob = blob.unwrap();
            let root: ID3D12RootSignature = device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())).expect("zdraw: root signature");
            let element = [D3D12_INPUT_ELEMENT_DESC { SemanticName: PCSTR(b"LOC\0".as_ptr()), SemanticIndex: 0, Format: DXGI_FORMAT_R32G32B32A32_FLOAT, InputSlot: 0, AlignedByteOffset: 0, InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, InstanceDataStepRate: 0 }];
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = scene_format;
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let replace = D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_REPLACE, StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS };
            let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vs.as_ptr() as *const c_void, BytecodeLength: vs.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: ps.as_ptr() as *const c_void, BytecodeLength: ps.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: true.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: true.into(), StencilReadMask: 0xFF, StencilWriteMask: 0xFF, FrontFace: replace, BackFace: replace },
                InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: element.as_ptr(), NumElements: 1 },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                RTVFormats: formats,
                DSVFormat: DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            let pipeline: ID3D12PipelineState = device.CreateGraphicsPipelineState(&pso).expect("zdraw: the body's pipeline");
            let slot = (256 + self.body.len() * 3 * 16).next_power_of_two();
            let bytes = slot * 3;
            let bdesc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: bytes as u64, Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() };
            let mut upload: Option<ID3D12Resource> = None;
            device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_UPLOAD, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &bdesc, D3D12_RESOURCE_STATE_GENERIC_READ, None, &mut upload).expect("zdraw: upload");
            let upload = upload.unwrap();
            let mut at: *mut c_void = std::ptr::null_mut();
            upload.Map(0, None, Some(&mut at)).expect("zdraw: map");
            self.gpu = Some(BodyGpu { root, pipeline, upload, mapped: at as *mut u8, bytes: slot, turn: 0 });
        }
        let g = self.gpu.as_mut().unwrap();
        g.turn = (g.turn + 1) % 3;
        let base = g.bytes * g.turn;
        let mut block: Vec<f32> = clip.to_cols_array().to_vec();
        block.extend_from_slice(&[0.45, -0.35, 0.82, 1.0, 0.36, 0.33, 0.30, 1.0]);
        std::ptr::copy_nonoverlapping(block.as_ptr() as *const u8, g.mapped.add(base), block.len() * 4);
        let verts: Vec<[f32; 4]> = self.body.iter().flat_map(|t| t.p).map(|p| [p.x * 100.0, p.y * 100.0, p.z * 100.0, 1.0]).collect();
        std::ptr::copy_nonoverlapping(verts.as_ptr() as *const u8, g.mapped.add(base + 256), verts.len() * 16);
        let address = g.upload.GetGPUVirtualAddress() + base as u64;
        list.SetGraphicsRootSignature(&g.root);
        list.SetPipelineState(&g.pipeline);
        list.SetGraphicsRootConstantBufferView(0, address);
        list.SetGraphicsRootConstantBufferView(1, address);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.IASetVertexBuffers(0, Some(&[D3D12_VERTEX_BUFFER_VIEW { BufferLocation: address + 256, SizeInBytes: (verts.len() * 16) as u32, StrideInBytes: 16 }]));
        list.OMSetStencilRef(0x50);
        list.DrawInstanced(verts.len() as u32, 1, 0, 0);
        list.OMSetStencilRef(0);
    }
}

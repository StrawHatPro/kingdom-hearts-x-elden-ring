//! What the evaluator hands to the host each drawn frame. Everything is in KH3 space (cm, X
//! forward, Y right, Z up, left-handed); colours are linear and unclamped (above 1 = HDR
//! intensity); UVs have their origin at the top left of the texture.

use glam::{Affine3A, Vec3};

/// The host's camera, KH3 space, unit axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub position: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

impl Camera {
    /// Looks along +X with Z up (tests).
    pub const DEFAULT: Camera = Camera { position: Vec3::new(-500.0, 0.0, 0.0), forward: Vec3::X, right: Vec3::Y, up: Vec3::Z };
}

/// How an emitter asks to be blended (`ParticleModuleRequired.EmitterBlendType`); the host
/// compares it with the material's own blend mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Blend {
    /// Whatever the material says (opaque, masked, modulate ...).
    #[default]
    Material,
    Translucent,
    Additive,
}

/// One textured quad, already oriented (camera-facing, velocity-aligned ...).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    /// Top left, top right, bottom right, bottom left.
    pub corners: [Vec3; 4],
    /// (u0, v0, u1, v1) of the sub-image.
    pub uv: [f32; 4],
    /// The next sub-image and how much of it to blend in (0 = none).
    pub uv2: [f32; 4],
    pub blend: f32,
    pub color: [f32; 4],
    pub material: u32,
    /// The four values of `ParticleModuleParameterDynamic` (the material's DynamicParameter).
    pub dynamic: [f32; 4],
    /// Seconds the emitter instance has lived (the materials' UV-scroll clock).
    pub time: f32,
    /// The particle's relative time, 0..1.
    pub rel: f32,
    /// The particle's centre and its size (cm).
    pub center: Vec3,
    pub size: f32,
    pub mode: Blend,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RibbonPoint {
    /// The two edges of the strip at this point.
    pub a: Vec3,
    pub b: Vec3,
    /// 0 at the head (newest), 1 at the tail.
    pub u: f32,
    pub color: [f32; 4],
}

/// A triangle strip: the keyblade trail, a tail.
#[derive(Clone, Debug, PartialEq)]
pub struct Ribbon {
    pub points: Vec<RibbonPoint>,
    pub material: u32,
    pub dynamic: [f32; 4],
    pub time: f32,
    pub blend: Blend,
    /// How many times the texture repeats along the ribbon (1 = stretched once over it).
    pub tiles: f32,
    /// The texture's V range across the ribbon is 0..v_scale; `uv_turned` swaps U and V.
    pub v_scale: f32,
    pub uv_turned: bool,
}

/// One instance of a static mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshInstance {
    pub mesh: u32,
    pub material: u32,
    /// Mesh space (UE cm, as the mesh was authored) to world.
    pub transform: Affine3A,
    pub color: [f32; 4],
    pub dynamic: [f32; 4],
    pub time: f32,
    pub rel: f32,
    pub blend: Blend,
    /// (fifth pass) `EmitterDef::mesh_uv_mode`: the UV turn / reverse the mesh's vertex shader applies.
    pub uv_mode: f32,
    /// (ZA) The particle's sub-UV image for a SubUV material on a mesh (`sub_image_code`; 0 = the emitter has one image).
    /// UE's FMeshParticleVertexFactory hands such a material `SubUV0AndTexCoord0` / `SubUV1AndLerp` (TEXCOORD1 / 2 of
    /// the KH3 mesh pixel shaders): the mesh's UV squeezed into the current (and next) image of the emitter's grid.
    pub sub: f32,
}

/// (ZA) The sub-UV image of a mesh particle as one float for the hosts' shaders (`kh_mesh_subuv` in both WGSL files):
/// -(1 + lerp7 | image8 << 7 | (h - 1) << 15 | (v - 1) << 19), exact in an f32; 0 = no sub images.
pub fn sub_image_code(image: f32, h: u32, v: u32, blend: bool) -> f32 {
    if h * v <= 1 {
        return 0.0;
    }
    let (h, v) = (h.clamp(1, 16), v.clamp(1, 16));
    let f = image.clamp(0.0, (h * v - 1) as f32);
    let i0 = (f.floor() as u32).min(255);
    let lerp = if blend { ((f - f.floor()) * 127.0).round() as u32 } else { 0 };
    -(1.0 + (lerp.min(127) | i0 << 7 | (h - 1) << 15 | (v - 1) << 19) as f32)
}

/// A point light a particle carries (`ParticleModuleLight`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub position: Vec3,
    /// Linear colour x brightness.
    pub color: Vec3,
    /// cm.
    pub radius: f32,
    /// (IMPL_ER56) the light's `LightExponent`.
    pub exponent: f32,
    /// (IMPL_ER56, a host's choice per system: `World::set_lights_on_characters`) false = an AMBIENT effect's light: a host
    /// lights characters with a small share of it only.
    pub characters: bool,
}

/// A deferred decal (`TypeDataSQEX_Decal`): its material is projected along the box's X axis
/// onto whatever lies inside the box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decal {
    pub material: u32,
    /// The decal's box: local [-1, 1] on every axis to world cm (X = the projection axis).
    pub transform: Affine3A,
    /// `DecalColor` (linear, unclamped) and `DecalOpacity`.
    pub color: [f32; 4],
    /// `CustomParam0..2`.
    pub custom: [f32; 3],
    /// `ZFadePow`, `ZFadeSegment`.
    pub z_fade: [f32; 2],
    pub time: f32,
}

/// (DON-MAGIC tint) A run of tinted draws: `kind` 0 quads, 1 ribbons, 2 meshes, 3 decals; items `from..to`; `tint` = (r, g, b,
/// intensity). The renderer recolours their final colour: rgb = max(r, g, b) x tint x intensity, alpha kept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TintRange {
    pub kind: u8,
    pub from: u32,
    pub to: u32,
    pub tint: [f32; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawList {
    pub quads: Vec<Quad>,
    pub ribbons: Vec<Ribbon>,
    pub meshes: Vec<MeshInstance>,
    pub lights: Vec<Light>,
    pub decals: Vec<Decal>,
    /// (DON-MAGIC tint) the tinted draws (kh_vfx::World::set_tint).
    pub tints: Vec<TintRange>,
    /// (VFXR) the screen-wide passes the live effects ask for this frame (`crate::screen`; `screen::pick` chooses).
    pub screen: Vec<crate::screen::ScreenFx>,
    /// (VFXR) the character-material drives of the live effects this frame (`SQEX_Material_*`).
    pub char_params: Vec<crate::screen::CharParam>,
    /// (VFXR sort) the draws of each live emitter, in the list's order (`DrawGroup`): what a host sorts its translucent
    /// draws by. Every quad / ribbon / mesh / decal belongs to exactly one group.
    pub groups: Vec<DrawGroup>,
}

impl DrawList {
    /// (DON-MAGIC tint) The tint of item `index` of kind `kind` (0 quads, 1 ribbons, 2 meshes, 3 decals), None: untinted.
    pub fn tint_of(&self, kind: u8, index: usize) -> Option<[f32; 4]> {
        let i = index as u32;
        self.tints.iter().find(|t| t.kind == kind && t.from <= i && i < t.to).map(|t| t.tint)
    }
}

impl DrawList {
    pub fn clear(&mut self) {
        self.tints.clear(); // (DON-MAGIC tint)
        self.screen.clear(); // (VFXR)
        self.char_params.clear(); // (VFXR)
        self.groups.clear(); // (VFXR sort)
        self.quads.clear();
        self.ribbons.clear();
        self.meshes.clear();
        self.lights.clear();
        self.decals.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.quads.is_empty() && self.ribbons.is_empty() && self.meshes.is_empty() && self.decals.is_empty()
    }
}

/// (VFXR sort) One emitter's draws this frame, and where they sit in KH3's translucency order: by `priority` (higher
/// over lower), then by the component's distance to the camera (far first; `origin` its place, cm, `offset` its
/// SQEXTranslucencySortOffset), then by emitter order inside the component (`emitter`, lower first).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawGroup {
    /// 0 quads, 1 ribbons, 2 meshes, 3 decals (as `TintRange`); items `from..to`.
    pub kind: u8,
    pub from: u32,
    pub to: u32,
    /// The live system's id (`EffectId`), and the emitter's index in its effect.
    pub system: u64,
    pub emitter: u32,
    pub priority: i32,
    pub origin: Vec3,
    pub offset: f32,
}

impl DrawGroup {
    /// The sort key against `camera` (cm): (priority, -(distance - offset), emitter). Ascending = KH3's drawing order.
    pub fn key(&self, camera: Vec3) -> (i32, f32, u64, u32) {
        (self.priority, -(self.origin.distance(camera) - self.offset), self.system, self.emitter)
    }
}

/// (VFXR sort) KH3's drawing order of two groups (`DrawGroup::key` compared field by field; the distance by total order).
pub fn group_order(a: &DrawGroup, b: &DrawGroup, camera: Vec3) -> std::cmp::Ordering {
    let (ka, kb) = (a.key(camera), b.key(camera));
    ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1)).then(ka.2.cmp(&kb.2)).then(ka.3.cmp(&kb.3))
}

impl DrawList {
    /// (VFXR sort) The index in `groups` of item `index` of kind `kind`; None when no group holds it.
    pub fn group_of(&self, kind: u8, index: usize) -> Option<usize> {
        let i = index as u32;
        self.groups.iter().position(|g| g.kind == kind && g.from <= i && i < g.to)
    }

    /// (VFXR sort) For every item of `kind`, its group index (`usize::MAX`: none), in one pass.
    pub fn group_table(&self, kind: u8, len: usize) -> Vec<usize> {
        let mut out = vec![usize::MAX; len];
        for (gi, g) in self.groups.iter().enumerate().filter(|(_, g)| g.kind == kind) {
            for slot in out.iter_mut().take((g.to as usize).min(len)).skip(g.from as usize) {
                *slot = gi;
            }
        }
        out
    }
}

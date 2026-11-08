// THE HARNESS OF THE EFFECT MATERIALS in the Elden Ring renderer (crates/kh-er-render/src/vfx.rs).
//
// The same contract as the test app's harness (crates/kh-app/src/vfx_material.wgsl), with this
// host's inputs instead of Bevy's: the body of a material is `kh_material(KhIn) -> KhOut`, a 1:1
// translation of the game's own compiled base-pass pixel shader (PACK/vfx/shaders/materials.wgsl,
// tools/sora_vfx_shaders.js), spliced in at the marker below and selected by the definition
// KH_S<owner>_<SPRITE|MESH|DECAL>; `kh_vertex` is the owner's mesh-particle vertex shader (KH_VS),
// `kh_distort` its distortion pass (KH_DISTORT), `kh_mask` its opacity mask (KH_MASK).
// crates/kh-shaderc builds one DXIL file per stage and variant through naga and dxc.
//
// Everything is in GAME space (KH3's: cm, X forward, Y right, Z up): the draw lists of kh-vfx are,
// the translated shaders work in it, and `er.clip_from_game` projects it into the host's picture.
//
// Vertex stages (one per kind of geometry):
//   ER_QUAD   sprites: one INSTANCE per quad (nine rows), six vertices from the vertex index;
//   ER_PLAIN  ribbons and decal boxes: plain vertices built on the CPU;
//   ER_MESH   mesh particles: the mesh's own vertices + one instance (six rows) each; with KH_VS
//             the owner's translated vertex shader does the transform (world position offset,
//             UV turn, its own interpolants), else the generic transform below.
// Bindings (kh-shaderc's map): b0 `er` (frame), b1 `kh` (material); t0..t9 the material's
// textures, t10 / t11 the engine's shared wrap / clamp textures, t12 the scene depth, t13 the copy
// of the scene colour (distortion); samplers by the index buffer t0 space1 (per material: the
// address modes of each texture).

struct ErFrame {
    clip_from_game: mat4x4<f32>,
    // xyz: the camera in GAME space
    camera: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    forward: vec4<f32>,
    // width, height, 1 / width, 1 / height of the target
    size: vec4<f32>,
    // x, y: view depth (m) = 1 / (device depth * x + y); z: 1 = a scene depth is bound; w: projection [0][0]
    depth: vec4<f32>,
    // x: what KH3's scene-linear 1.0 becomes in the target (the HDR scale); y: 1 = diagnosis (solid magenta)
    out_scale: vec4<f32>,
    // rgb: what a white lit surface shows in the target's units, divided by out_scale.x (lit materials)
    lit: vec4<f32>,
#ifdef HY_SCENE
    // (HY, the offline harness only: hylight2 / hyfog) THE GAME'S LIGHT PASSES' STAND-IN, a scene of Elden Ring's kind. All
    // positions in GAME space (cm), light in SCENE units (what the light target holds for a white surface).
    // sun: direction TO it (w: 1 = there), its light per unit N.L (rgb), w: 1 = his own capsules cast into the "cascades"
    hs_sun: vec4<f32>,
    hs_sun_col: vec4<f32>,
    // the undirected light: everywhere (rgb) + per unit of the normal's upward part (rgb); w: the occlusion's strength
    hs_amb: vec4<f32>,
    hs_amb_up: vec4<f32>,
    // his feet (xyz), w: the number of local lights; the fog: colour (rgb), w: density per metre
    hs_feet: vec4<f32>,
    hs_fog: vec4<f32>,
    // world casters (capsules, relative to his feet): a.xyz + radius, b.xyz + 1 = there
    hs_occ: array<vec4<f32>, 8>,
    // local point lights (relative to his feet): position + radius, colour (rgb)
    hs_lpos: array<vec4<f32>, 4>,
    hs_lcol: array<vec4<f32>, 4>,
    // his capsules this frame (the renderer's, game space; also the friends'): a + radius, b + 1 = there
    hs_caps: array<vec4<f32>, 64>,
#endif
#ifdef ER_CHARS
    // (ZFIN, the offline harness only: erchr.ps.dxil) ELDEN RING'S CHARACTERS' STAND-IN: capsules (game space, cm):
    // [2k] one end + radius, [2k + 1] the other end + the part's tint (0 = none: the list ends)
    ec: array<vec4<f32>, 48>,
#endif
    // (VFX-2 lit) ELDEN RING'S LIGHT FOR THE FORWARD-LIT EFFECTS (kh-er-render fwdlight.rs; KH3 units): UE's
    // ForwardGlobalLightData rows 0..6 (the sun as its directional light, one light-grid cell) + [7] (the sun's visibility = the
    // primitive's precomputed shadowing, the number of lights); the reflection environment (zenith, horizon, ground) and the
    // scene colours (bright, dark; w = 0: the harness's own); the cell's lights, UE's ForwardLocalLightBuffer (4 rows each)
    fwd_g: array<vec4<f32>, 8>,
    fwd_env: array<vec4<f32>, 5>,
    fwd_l: array<vec4<f32>, 64>,
#ifdef KH_FXNAT
    // (PARTY-NATIVE) THE NATIVE ROUTE OF THE LIT (DEFERRED) EFFECTS (vfx.rs `FXN_FLOATS`, appended after the forward rows):
    // x 1 = the capture of the game's light (gfx.rs hycap2, the native layout with its specular) is bound at t14 / t15 this
    // frame; y the light-set mark of the KH G-buffer white (G-buffer 2 .a, hybrid.rs row 3.w); z the normal match (cos,
    // r.hybrid_match); w -
    fxn: vec4<f32>,
#endif
#ifdef KH_FXFOG
    // (PARTY-NATIVE) THE GAME'S FOG (vfx.rs `FXF_FLOATS`, after the native row): x 1 = this frame's copy of the fog composite's
    // buffer and volume is bound at t16 / t17 (vfx.rs `fog_copy`); yzw -
    fxf: vec4<f32>,
#endif
#ifdef KH_FXLIT
    // (PARTY-NATIVE-LIGHT) ELDEN RING'S OWN LIGHT for the lit translucent effects (vfxlight.rs `fxl_row`, after the fog row): [0] (1 =
    // this frame's copy is bound at t18..t27, the volumes' source (1 the particle pass, 2 the regions), the volume mask, 1 = the light
    // buffers copied), [1] the sun's colour as read (FC_DirLight Col_0, w 1 = a sun), [2] (1 = the cascades bound at t28 / t29, the
    // regions, the lists' width, (round 3) 1 = a two-volume region), [3..9) the cascades' rows (hybrid.rs sun_rows), [9..13) the game
    // -> host matrix (columns)
    fxl: array<vec4<f32>, 13>,
#endif
}

// (VFX-2 lit) THE FORWARD LIGHTING'S INPUTS of a translated forward-lit shader (tools/sora_vfx_shaders.js, its `fwd` owners),
// integer values held as floats (the translator's convention). The light grid is ONE cell (fwdlight.rs: every pixel in cell 0):
// NumCulledLightsGrid = (lights, start 0) for cell 0 and (1 capture, start = lights) for the captures' cell 1;
// CulledLightDataGrid = the light indices, then capture 0.
fn kh_fwd_grid(i: f32) -> vec4<f32> {
    let k = i32(round(i));
    let n = er.fwd_g[7].y;
    var v = 0.0;
    if (k == 0) {
        v = n;
    } else if (k == 2) {
        v = 1.0;
    } else if (k == 3) {
        v = n;
    }
    return vec4<f32>(v, 0.0, 0.0, 0.0);
}

fn kh_fwd_data(i: f32) -> vec4<f32> {
    let n = er.fwd_g[7].y;
    return vec4<f32>(select(0.0, round(i), i < n - 0.5), 0.0, 0.0, 0.0);
}

fn kh_fwd_light(i: f32) -> vec4<f32> {
    let k = i32(round(i));
    if (k < 0 || k >= 64) {
        return vec4<f32>(0.0);
    }
    return er.fwd_l[k];
}

// ReflectionCapture's arrays (rows of 341 captures): capture 0 is an infinite sphere around the camera (brightness 1, cube 0,
// shape 0 = sphere); no other capture, no offset, no box.
fn kh_fwd_capture(row: f32) -> vec4<f32> {
    let r = i32(round(row));
    if (r == 0) {
        return vec4<f32>(er.camera.xyz, 1.0e7);
    }
    if (r == 341) {
        return vec4<f32>(1.0, 0.0, 0.0, 0.0);
    }
    return vec4<f32>(0.0);
}

// The capture cube at a direction and mip: kh-look's sky cube (Env::Sky::sample: ground below a soft horizon, horizon to
// zenith above, blurred by the roughness the mip stands for: UE's mip = MaxMip - 1 - (1 - 1.2 log2 roughness), MaxMip =
// fwdlight.rs CUBE_MAX_MIP) in the game's reflection environment (fwd_env 0..2); alpha 1 (it covers).
#ifdef KH_FXLIT
// (PARTY-NATIVE-LIGHT) a lit variant: the capture = Elden Ring's light arriving from d at this pixel (the SH, KH units: kh_lit_e)
fn kh_fwd_env(d: vec3<f32>, lod: f32) -> vec4<f32> {
    let n = select(vec3<f32>(0.0, 0.0, 1.0), normalize(d), dot(d, d) > 1.0e-20);
    return vec4<f32>(kh_lit_e(kh_fx_host_dir(n)), 1.0);
}
#else
fn kh_fwd_env(d: vec3<f32>, lod: f32) -> vec4<f32> {
    let n = select(vec3<f32>(0.0, 0.0, 1.0), normalize(d), dot(d, d) > 1.0e-20);
    let rough = clamp(exp2((lod - 7.0 + 2.0) / 1.2), 0.0, 1.0);
    let blur = clamp(rough * rough, 0.0, 1.0);
    let width = 0.04 + 0.9 * blur;
    var up = clamp((n.z / width) * 0.5 + 0.5, 0.0, 1.0);
    up = up * up * (3.0 - 2.0 * up);
    let sky = mix(er.fwd_env[1].xyz, er.fwd_env[0].xyz, pow(max(n.z, 0.0), 0.6 + blur));
    return vec4<f32>(mix(er.fwd_env[2].xyz, sky, up), 1.0);
}
#endif

// (PARTY-NATIVE) THE INDIRECT LIGHTING CACHE of a lit translucent translated from its ILC permutation (tools/sora_vfx_shaders.js
// --fwl: PrecomputedLightingBuffer[8..14] -> kh_fwd_ilc(k)): rows 8..10 = IndirectLightingSHCoefficients0[R, G, B] = (V0, V1, V2,
// V3), 11..13 = Coefficients1 (V4..V7), 14 = Coefficients2 (V8). The material evaluates them as UE does: 0.886228 V0 +
// 1.023328 (-V1 n.y + V2 n.z - V3 n.x) (+ band 2), n its own world normal; a non-directional mode reads only 0.886228 V0.
#ifdef KH_FXLIT
// A lit variant: Elden Ring's SH at this pixel (kh_lit_sh, KH units, host axes) in that layout, so the material's sum is
// kh_lit_e(kh_fx_host_dir(n)) = W + n.y S.x + n.z S.y + n.x S.z (and W, the SH's constant term, for a non-directional mode);
// band 2 = 0 (Elden Ring's SH is L1)
fn kh_fwd_ilc(k: i32) -> vec4<f32> {
    if (k < 8 || k > 10) {
        return vec4<f32>(0.0);
    }
    let s = kh_lit_sh[k - 8];
    return vec4<f32>(s.w / 0.886228, -s.x / 1.023328, s.y / 1.023328, -s.z / 1.023328);
}
#else
// The other variants: no cache sample (the light the NoLightMap translation had; the Kingdom Hearts 3 style is unchanged)
fn kh_fwd_ilc(k: i32) -> vec4<f32> {
    return vec4<f32>(0.0);
}
#endif

// The integer operations on values held as floats (D3D's and / ushr / ubfe / bfi / f16tof32).
fn kh_iand(a: f32, b: f32) -> f32 {
    return f32(u32(max(a, 0.0)) & u32(max(b, 0.0)));
}

fn kh_ushr(a: f32, b: f32) -> f32 {
    return f32(u32(max(a, 0.0)) >> (u32(max(b, 0.0)) & 31u));
}

fn kh_ubfe(w: f32, o: f32, s: f32) -> f32 {
    let width = u32(max(w, 0.0)) & 31u;
    return f32((u32(max(s, 0.0)) >> (u32(max(o, 0.0)) & 31u)) & ((1u << width) - 1u));
}

fn kh_bfi(w: f32, o: f32, s: f32, b: f32) -> f32 {
    let width = u32(max(w, 0.0)) & 31u;
    let offset = u32(max(o, 0.0)) & 31u;
    let mask = ((1u << width) - 1u) << offset;
    return f32((u32(max(b, 0.0)) & ~mask) | ((u32(max(s, 0.0)) << offset) & mask));
}

fn kh_f16lo(a: f32) -> f32 {
    return unpack2x16float(bitcast<u32>(a)).x;
}

struct KhParams {
    m: array<vec4<f32>, 64>,
    // x: opacity-mask clip value, y: seconds (effect clock), z: 1 = emitter draws additive although the shader is translucent, w: 1 = the reverse
    misc: vec4<f32>,
    // x: 1 = drawn with depth write (no scene depth is read)
    er: vec4<f32>,
#ifdef KH_FXFOG
    // (PARTY-NATIVE) x: the game's fog mode for this material's pass (vfx.rs `fog_mode`): 0 blended (attenuated + in-scatter),
    // 1 additive (attenuated only), 2 modulate (toward no effect), 3 none (a distortion / an opaque pass)
    fx: vec4<f32>,
#endif
}

@group(0) @binding(0) var<uniform> er: ErFrame;
@group(0) @binding(1) var<uniform> kh: KhParams;
@group(1) @binding(0) var kh_t0: texture_2d<f32>;
@group(1) @binding(1) var kh_t1: texture_2d<f32>;
@group(1) @binding(2) var kh_t2: texture_2d<f32>;
@group(1) @binding(3) var kh_t3: texture_2d<f32>;
@group(1) @binding(4) var kh_t4: texture_2d<f32>;
@group(1) @binding(5) var kh_t5: texture_2d<f32>;
@group(1) @binding(6) var kh_t6: texture_2d<f32>;
@group(1) @binding(7) var kh_t7: texture_2d<f32>;
@group(1) @binding(8) var kh_t8: texture_2d<f32>;
@group(1) @binding(9) var kh_t9: texture_2d<f32>;
@group(1) @binding(10) var kh_twrap: texture_2d<f32>;
@group(1) @binding(11) var kh_tclamp: texture_2d<f32>;
@group(1) @binding(12) var er_depth: texture_2d<f32>;
@group(1) @binding(13) var er_scene: texture_2d<f32>;
#ifdef KH_FXNAT
// (PARTY-NATIVE) the capture of the game's light (gfx.rs `HyCapture`, the characters' own: hycap2's native layout): t14 the light
// a white surface got at the texel (rgb, scene units) + the game's specular .b; t15 the octahedral normal + 2 (xy) and the
// specular .rg
@group(1) @binding(14) var er_hy_light: texture_2d<f32>;
@group(1) @binding(15) var er_hy_geo: texture_2d<f32>;
#endif
#ifdef KH_FXFOG
// (PARTY-NATIVE) the game's fog, this frame's copy (vfx.rs `fog_copy`): t16 the fog composite's g_InScatterTex (rgb in-scatter,
// a transmittance, texel for texel), t17 its CB_SHADER_COMMON_PARAM as raw words (texels 0..75; texel 76 the volume's size)
@group(1) @binding(16) var er_fog_vol: texture_3d<f32>;
@group(1) @binding(17) var er_fog_rows: texture_2d<u32>;
#endif
#ifdef KH_FXLIT
// (PARTY-NATIVE-LIGHT) this frame's copy of the game's light inputs (vfxlight.rs `write_views`): t18 the rows (raw words), t19..t26 the
// irradiance volumes (A: X Y Z W, B: X Y Z W; or two regions), t27 the light buffers, t28 the game's cascade atlas (16-bit, reversed
// depth), t29 the cascades' constants (gfx.rs SunKit)
@group(1) @binding(18) var er_lit_rows: texture_2d<u32>;
@group(1) @binding(19) var er_iv0: texture_3d<f32>;
@group(1) @binding(20) var er_iv1: texture_3d<f32>;
@group(1) @binding(21) var er_iv2: texture_3d<f32>;
@group(1) @binding(22) var er_iv3: texture_3d<f32>;
@group(1) @binding(23) var er_iv4: texture_3d<f32>;
@group(1) @binding(24) var er_iv5: texture_3d<f32>;
@group(1) @binding(25) var er_iv6: texture_3d<f32>;
@group(1) @binding(26) var er_iv7: texture_3d<f32>;
@group(1) @binding(27) var er_lit_lists: texture_2d<u32>;
@group(1) @binding(28) var er_sun_atlas: texture_2d<f32>;
@group(1) @binding(29) var er_sun_casc: texture_2d<f32>;
// the forward lighting's global rows as the material reads them (kh-shaderc writes `kh_fwdg[` for `er.fwd_g[` in a lit variant) and
// the SH at this pixel (KH units), set in fs_main before the material runs
var<private> kh_fwdg: array<vec4<f32>, 8>;
var<private> kh_lit_sh: array<vec4<f32>, 3>;
#endif
@group(2) @binding(0) var kh_s0: sampler;
@group(2) @binding(1) var kh_s1: sampler;
@group(2) @binding(2) var kh_s2: sampler;
@group(2) @binding(3) var kh_s3: sampler;
@group(2) @binding(4) var kh_s4: sampler;
@group(2) @binding(5) var kh_s5: sampler;
@group(2) @binding(6) var kh_s6: sampler;
@group(2) @binding(7) var kh_s7: sampler;
@group(2) @binding(8) var kh_s8: sampler;
@group(2) @binding(9) var kh_s9: sampler;
@group(2) @binding(10) var kh_swrap: sampler;
@group(2) @binding(11) var kh_sclamp: sampler;
@group(2) @binding(12) var er_sscene: sampler;

struct KhVaryings {
    @builtin(position) position: vec4<f32>,
    // GAME space, cm
    @location(0) world_position: vec3<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) vcol: vec4<f32>,
    @location(4) dynamic: vec4<f32>,
    // x: emitter seconds, y: the particle's relative time, z: sub-image blend (mesh: TEXCOORD10.w), w: tangent sign
    @location(5) misc: vec4<f32>,
    @location(6) normal: vec3<f32>,
    @location(7) tangent: vec3<f32>,
    @location(8) particle: vec4<f32>,
    @location(9) tc1: vec4<f32>,
}

// What a translated vertex shader (`kh_vertex`) reads, by the game's names (see the test app's harness).
struct KhVsIn {
    a0: vec4<f32>, a1: vec4<f32>, a2: vec4<f32>, a3: vec4<f32>, a4: vec4<f32>, a5: vec4<f32>, a6: vec4<f32>, a7: vec4<f32>,
    a8: vec4<f32>, a9: vec4<f32>, a10: vec4<f32>, a12: vec4<f32>, a13: vec4<f32>, a14: vec4<f32>, a15: vec4<f32>,
    mp0: vec4<f32>, mp1: vec4<f32>, mp2: vec4<f32>, mp3: vec4<f32>, mp4: vec4<f32>, mp5: vec4<f32>,
    p0: vec4<f32>, p1: vec4<f32>, p2: vec4<f32>, p3: vec4<f32>, p8: vec4<f32>, p9: vec4<f32>,
    v8: vec4<f32>, v9: vec4<f32>, v10: vec4<f32>, v11: vec4<f32>, v12: vec4<f32>, v13: vec4<f32>, v14: vec4<f32>, v15: vec4<f32>,
    v32: vec4<f32>, v33: vec4<f32>, v34: vec4<f32>, v35: vec4<f32>, v48: vec4<f32>, v55: vec4<f32>, v56: vec4<f32>, v58: vec4<f32>,
    v128: vec4<f32>, v161: vec4<f32>,
}

struct KhVsOut {
    tc10: vec4<f32>, tc11: vec4<f32>, tc0: vec4<f32>, tc1: vec4<f32>,
    col0: vec4<f32>, col1: vec4<f32>, col2: vec4<f32>, col3: vec4<f32>,
    wpos: vec3<f32>,
}

struct KhIn {
    tc0: vec4<f32>, tc1: vec4<f32>, tc2: vec4<f32>, tc3: vec4<f32>, tc7: vec4<f32>, tc10: vec4<f32>, tc11: vec4<f32>,
    col0: vec4<f32>, col1: vec4<f32>, col2: vec4<f32>, col3: vec4<f32>,
    ptime: vec4<f32>, ppos: vec4<f32>, psubuv: vec4<f32>, psize: vec4<f32>,
    svpos: vec4<f32>, front: vec4<f32>,
    ptr0: vec4<f32>, ptr1: vec4<f32>, ptr2: vec4<f32>,
    // Translated world position (relative to the camera), game space.
    twp: vec3<f32>,
    // 1 / (scene depth behind this pixel, cm); 0 when nothing is there.
    inv_scene_depth: f32,
    v8: vec4<f32>, v9: vec4<f32>, v10: vec4<f32>, v53: vec4<f32>, v54: vec4<f32>, v55: vec4<f32>, v58: vec4<f32>,
    // (VFX-2 cube) View[16..19] TranslatedWorldToCameraView (the matcap's view-space normal)
    v16: vec4<f32>, v17: vec4<f32>, v18: vec4<f32>, v19: vec4<f32>,
    v117: vec4<f32>, v118: vec4<f32>, v119: vec4<f32>, v122: vec4<f32>, v123: vec4<f32>, v124: vec4<f32>, v125: vec4<f32>,
    v126: vec4<f32>, v127: vec4<f32>, v128: vec4<f32>, v130: vec4<f32>, v137: vec4<f32>, v161: vec4<f32>,
    p8: vec4<f32>, p9: vec4<f32>, p11: vec4<f32>, p17: vec4<f32>, p18: vec4<f32>,
    s10: vec4<f32>, l5: vec4<f32>,
    decal_pos: vec3<f32>, dcol: vec4<f32>, dprm: vec4<f32>, dz: vec4<f32>, d12: vec4<f32>, d23: vec4<f32>,
    // (XW) the decal's axes X, Y, Z in the world, unit length (rows 0..2; row 3 = (0, 0, 0, 1)): what a decal material's
    // world -> tangent transforms read (the shader's own table: tools/sora_vfx_shaders.js decalRowsOf, --xsi)
    drot0: vec4<f32>, drot1: vec4<f32>, drot2: vec4<f32>, drot3: vec4<f32>,
    dist12: vec4<f32>,
    // (VFX-XLATE) the scene's particle colours of the lit materials (SceneBaseColor / SceneBrightColor / SceneDarkColor /
    // SceneColorSynthesis of the particle vertex factories; tools/sora_vfx_shaders.js --root)
    pbase: vec4<f32>, pbright: vec4<f32>, pdark: vec4<f32>, psynth: vec4<f32>,
    // (VFX-2 lit) the same two of the forward-lit owners: Elden Ring's light on a white surface facing up, lit / in the shade
    // (the rig: fwdlight.rs), else the two above
    fbright: vec4<f32>, fdark: vec4<f32>,
}

// (VFX-2 cube) A material cube map (TextureCube), packed by tools/vfx_cube_pack.js as six faces side by side (+X -X +Y -Y +Z
// -Z, each N x N) in RGBM (linear = rgb x a x 8): the face and uv of the direction as D3D picks them (major axis; +X: (-z, -y),
// -X: (z, -y), +Y: (x, z), -Y: (x, -z), +Z: (x, -y), -Z: (-x, -y) over |major|), half a texel inside the face, top mip.
fn kh_cube(t: texture_2d<f32>, s: sampler, d: vec3<f32>) -> vec4<f32> {
    let a = abs(d);
    var face = 0.0;
    var st = vec2<f32>(0.0);
    var ma = 1.0;
    if (a.x >= a.y && a.x >= a.z) {
        ma = a.x;
        if (d.x >= 0.0) { face = 0.0; st = vec2<f32>(-d.z, -d.y); } else { face = 1.0; st = vec2<f32>(d.z, -d.y); }
    } else if (a.y >= a.z) {
        ma = a.y;
        if (d.y >= 0.0) { face = 2.0; st = vec2<f32>(d.x, d.z); } else { face = 3.0; st = vec2<f32>(d.x, -d.z); }
    } else {
        ma = a.z;
        if (d.z >= 0.0) { face = 4.0; st = vec2<f32>(d.x, -d.y); } else { face = 5.0; st = vec2<f32>(-d.x, -d.y); }
    }
    let n = f32(textureDimensions(t).y);
    let uv = clamp(st / max(ma, 1.0e-6) * 0.5 + vec2<f32>(0.5), vec2<f32>(0.5 / n), vec2<f32>(1.0 - 0.5 / n));
    let c = textureSampleLevel(t, s, vec2<f32>((face + uv.x) / 6.0, uv.y), 0.0);
    return vec4<f32>(c.xyz * c.w * 8.0, 1.0);
}

// (VFX-XLATE) A scene-colour read of a translated material (the blur / warp materials: tools/sora_vfx_shaders.js --root): the
// frame behind the effects at that screen uv, from the copy kh-er-render vfx.rs makes before the translucent passes (t13), in
// KH3's units (the copy holds the host's: the material's output is scaled back by out_scale.x).
fn kh_scene_color(uv: vec2<f32>) -> vec4<f32> {
    let c = textureSampleLevel(er_scene, er_sscene, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0).xyz;
    return vec4<f32>(c / max(er.out_scale.x, 1.0e-9), 1.0);
}

// (ZA) The mesh sub-UV coordinates of UE's FMeshParticleVertexFactory from kh_vfx::sub_image_code (as the test app's
// vfx_material.wgsl): -(1 + lerp7 | image8 << 7 | (h - 1) << 15 | (v - 1) << 19).
struct KhSubUv {
    sub0: vec4<f32>,
    sub1: vec4<f32>,
}

fn kh_mesh_subuv(uv: vec2<f32>, code: f32) -> KhSubUv {
    let c = u32(round(-code)) - 1u;
    let lerp = f32(c & 127u) / 127.0;
    let f0 = (c >> 7u) & 255u;
    let h = ((c >> 15u) & 15u) + 1u;
    let v = ((c >> 19u) & 15u) + 1u;
    let f1 = min(f0 + 1u, h * v - 1u);
    let size = vec2<f32>(1.0 / f32(h), 1.0 / f32(v));
    let s0 = (uv + vec2<f32>(f32(f0 % h), f32(f0 / h))) * size;
    let s1 = (uv + vec2<f32>(f32(f1 % h), f32(f1 / h))) * size;
    return KhSubUv(vec4<f32>(s0, uv), vec4<f32>(s1, lerp, 0.0));
}

struct KhOut {
    o0: vec4<f32>,
    o1: vec4<f32>,
    o2: vec4<f32>,
    o3: vec4<f32>,
}

#ifdef KH_GENERIC
// No translated shader: emissive texture (or the two-colour ramp of a mask) x particle colour,
// opacity texture x particle alpha (the test app's generic variant, unchanged).
fn kh_material(i: KhIn) -> KhOut {
    let uv = i.tc3.xy * kh.m[3].xy;
    var rgb = vec3<f32>(1.0);
    var a = 1.0;
    if (kh.m[0].x != 0.0) {
        let e = textureSampleLevel(kh_t0, kh_s0, uv, 0.0);
        if (kh.m[0].y != 0.0) {
            rgb = mix(kh.m[2].xyz, kh.m[1].xyz, saturate(e.x * e.x * kh.m[3].z + kh.m[3].w));
        } else {
            rgb = e.xyz * kh.m[3].z + vec3<f32>(kh.m[3].w);
            a = e.w;
        }
    }
    if (kh.m[0].z != 0.0) {
        let o = textureSampleLevel(kh_t1, kh_s1, uv, 0.0).x;
        a = o * o;
    }
    rgb = rgb * i.tc2.xyz * i.col0.xyz * i.col0.xyz;
    a = saturate(a * i.tc2.w * i.col0.w * i.col0.w);
    if (kh.m[0].w != 0.0) {
        return KhOut(vec4<f32>(rgb * a, 0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    }
    return KhOut(vec4<f32>(rgb, a), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
}
#endif

//KH_GENERATED

#ifdef ER_QUAD
// ---- sprites: one instance per quad
struct QuadInstance {
    @builtin(vertex_index) vertex: u32,
    // corner k (top left, top right, bottom right, bottom left) in xyz; w: emitter seconds, relative time, sub-image blend, size
    @location(0) c0: vec4<f32>,
    @location(1) c1: vec4<f32>,
    @location(2) c2: vec4<f32>,
    @location(3) c3: vec4<f32>,
    // (u0, v0, u1, v1) of the sub-image, and of the next one
    @location(4) uv: vec4<f32>,
    @location(5) uv2: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) dynamic: vec4<f32>,
    // xyz: the particle's centre
    @location(8) center: vec4<f32>,
}

@vertex
fn vs_quad(v: QuadInstance) -> KhVaryings {
    var out: KhVaryings;
    // Two triangles: 0 1 2, 0 2 3.
    var order = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let k = order[v.vertex % 6u];
    var corner = v.c0.xyz;
    var uv = vec4<f32>(v.uv.x, v.uv.y, v.uv2.x, v.uv2.y);
    if (k == 1u) {
        corner = v.c1.xyz;
        uv = vec4<f32>(v.uv.z, v.uv.y, v.uv2.z, v.uv2.y);
    } else if (k == 2u) {
        corner = v.c2.xyz;
        uv = vec4<f32>(v.uv.z, v.uv.w, v.uv2.z, v.uv2.w);
    } else if (k == 3u) {
        corner = v.c3.xyz;
        uv = vec4<f32>(v.uv.x, v.uv.w, v.uv2.x, v.uv2.w);
    }
    out.world_position = corner;
    out.position = er.clip_from_game * vec4<f32>(corner, 1.0);
    out.uv = uv;
    out.color = v.color;
    out.vcol = vec4<f32>(1.0);
    out.dynamic = v.dynamic;
    out.misc = vec4<f32>(v.c0.w, v.c1.w, v.c2.w, 1.0);
    let to_camera = er.camera.xyz - v.center.xyz;
    out.normal = select(-er.forward.xyz, normalize(to_camera), dot(to_camera, to_camera) > 1.0e-6);
    let along = v.c1.xyz - v.c0.xyz;
    out.tangent = select(er.right.xyz, normalize(along), dot(along, along) > 1.0e-12);
    out.particle = vec4<f32>(v.center.xyz, v.c3.w);
    out.tc1 = v.dynamic;
    return out;
}
#endif

#ifdef ER_PLAIN
// ---- ribbons and decal boxes: vertices built on the CPU
struct PlainVertex {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) vcol: vec4<f32>,
    @location(4) dynamic: vec4<f32>,
    @location(5) misc: vec4<f32>,
    @location(6) normal: vec3<f32>,
    @location(7) tangent: vec3<f32>,
    @location(8) particle: vec4<f32>,
}

@vertex
fn vs_plain(v: PlainVertex) -> KhVaryings {
    var out: KhVaryings;
    out.world_position = v.position;
    out.position = er.clip_from_game * vec4<f32>(v.position, 1.0);
    out.uv = v.uv;
    out.color = v.color;
    out.vcol = v.vcol;
    out.dynamic = v.dynamic;
    out.misc = v.misc;
    out.normal = v.normal;
    out.tangent = v.tangent;
    out.particle = v.particle;
    out.tc1 = v.dynamic;
    return out;
}
#endif

#ifdef ER_MESH
// ---- mesh particles: the mesh's own vertices (UE cm, its own axes) + one instance each
struct MeshVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // w: the sign of the binormal
    @location(2) tangent: vec4<f32>,
    // UV set 0 in xy, set 1 in zw
    @location(3) uv: vec4<f32>,
    @location(4) vcol: vec4<f32>,
    // The rows of the instance's local -> world transform (GAME space, cm).
    @location(5) row0: vec4<f32>,
    @location(6) row1: vec4<f32>,
    @location(7) row2: vec4<f32>,
    @location(8) color: vec4<f32>,
    @location(9) dynamic: vec4<f32>,
    // x: emitter seconds, y: relative time, z: the UV selectors (turn 0 / 1 / 2 + 4 x U reverse + 8 x V reverse)
    @location(10) imisc: vec4<f32>,
}

@vertex
fn vs_mesh(v: MeshVertex) -> KhVaryings {
    var out: KhVaryings;
    let origin = vec3<f32>(v.row0.w, v.row1.w, v.row2.w);
    let x_axis = vec3<f32>(v.row0.x, v.row1.x, v.row2.x);
    out.particle = vec4<f32>(origin, length(x_axis));
#ifdef KH_VS
    // The game's vertex shader does the work: instance transform, UV turn, world position offset
    // and the interpolants; this stage only feeds it and projects its result.
    var i: KhVsIn;
    let seconds = v.imisc.x;
    i.a0 = vec4<f32>(v.position, 1.0);
    i.a1 = vec4<f32>(v.tangent.xyz * 0.5 + vec3<f32>(0.5), 0.0);
    i.a2 = vec4<f32>(v.normal * 0.5 + vec3<f32>(0.5), v.tangent.w * 0.5 + 0.5);
    i.a3 = v.vcol;
    i.a4 = vec4<f32>(v.uv.xy, 0.0, 0.0);
    i.a5 = vec4<f32>(v.uv.zw, 0.0, 0.0);
    i.a6 = i.a5;
    i.a7 = i.a5;
    i.a8 = v.row0;
    i.a9 = v.row1;
    i.a10 = v.row2;
    i.a12 = vec4<f32>(0.0, v.imisc.y, 0.0, 0.0);
    i.a13 = v.dynamic;
    i.a14 = v.color;
    i.a15 = vec4<f32>(0.0);
    i.mp2 = vec4<f32>(1.0);
    let code = u32(v.imisc.z + 0.5);
    i.mp3 = vec4<f32>(f32(code & 3u), f32((code >> 2u) & 1u), f32((code >> 3u) & 1u), 0.0);
    i.mp5 = vec4<f32>(0.0, seconds, 0.0, 0.0);
    i.p9 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.v58 = vec4<f32>(-er.camera.xyz, 0.0);
    i.v128 = vec4<f32>(seconds);
    i.v161 = vec4<f32>(1.0, 1.0, seconds, 0.0);
    let o = kh_vertex(i);
    let world = o.wpos + er.camera.xyz;
    out.world_position = world;
    out.position = er.clip_from_game * vec4<f32>(world, 1.0);
    out.uv = o.tc0;
    out.tc1 = o.tc1;
    out.color = o.col1;
    out.vcol = o.col0;
    out.dynamic = o.col2;
    out.misc = vec4<f32>(o.col3.z, v.imisc.y, o.tc10.w, o.tc11.w);
    out.normal = o.tc11.xyz;
    out.tangent = o.tc10.xyz;
#else
    // The generic transform (what the test app does on the CPU for an owner without a translated
    // vertex shader): the instance's rows, normals through the inverse transpose, the UV turn.
    let r0 = v.row0.xyz;
    let r1 = v.row1.xyz;
    let r2 = v.row2.xyz;
    let world = vec3<f32>(dot(r0, v.position), dot(r1, v.position), dot(r2, v.position)) + origin;
    out.world_position = world;
    out.position = er.clip_from_game * vec4<f32>(world, 1.0);
    // The inverse transpose of the 3 x 3 part, up to its determinant: the cofactors.
    let cx = cross(r1, r2);
    let cy = cross(r2, r0);
    let cz = cross(r0, r1);
    let det = dot(r0, cx);
    var n = vec3<f32>(dot(cx, v.normal), dot(cy, v.normal), dot(cz, v.normal)) * sign(det);
    if (abs(det) < 1.0e-12) {
        n = vec3<f32>(dot(r0, v.normal), dot(r1, v.normal), dot(r2, v.normal));
    }
    out.normal = select(vec3<f32>(0.0, 0.0, 1.0), normalize(n), dot(n, n) > 1.0e-20);
    let t = vec3<f32>(dot(r0, v.tangent.xyz), dot(r1, v.tangent.xyz), dot(r2, v.tangent.xyz));
    out.tangent = select(vec3<f32>(1.0, 0.0, 0.0), normalize(t), dot(t, t) > 1.0e-20);
    var uv = v.uv;
    let mode = u32(v.imisc.z + 0.5);
    if (mode != 0u) {
        let a = select(uv.x, -uv.x, (mode & 4u) != 0u);
        let b = select(uv.y, -uv.y, (mode & 8u) != 0u);
        if ((mode & 3u) == 1u) {
            uv = vec4<f32>(b, -a, uv.z, uv.w);
        } else if ((mode & 3u) == 2u) {
            uv = vec4<f32>(-b, a, uv.z, uv.w);
        } else {
            uv = vec4<f32>(a, b, uv.z, uv.w);
        }
    }
    out.uv = uv;
    out.tc1 = vec4<f32>(v.uv.zw, v.uv.zw);
    out.color = v.color;
    out.vcol = v.vcol;
    out.dynamic = v.dynamic;
    out.misc = vec4<f32>(v.imisc.x, v.imisc.y, v.imisc.w, v.tangent.w); // (ZA) misc.z: the mesh sub-UV image (kh_mesh_subuv)
#endif
    return out;
}
#endif

#ifdef ER_FRAGMENT
#ifdef KH_ORB
// (ORBS) THE PRIZE ORBS' LIT BALLS (omt_prz_hpb_lit_00o, omt_prz_mpb_lit_00o; kh-shaderc gives them KH_ORB): what KH3's
// deferred pass makes of their G-buffer, in place of the generic "emissive + base colour x a fixed light" of the other lit
// effect materials, which clipped the base colour per channel (saturate of a colour the orbs' gain had pushed over 1: a
// whitish ball) and had neither the metallic nor the specular the shaders write (MP ball: metallic 1, roughness 0.3: a
// chrome ball; HP ball: specular 1, roughness 0.4: a glossy one). In KH3 units, `white` = what a white lit surface shows.
// - base colour: the shader's emissive (= its unclipped base colour x the particle colour) scaled down to a peak of 1, so
//   the hue holds where the old saturate went white;
// - diffuse (1 - metallic) under the key light, as before; specular: Schlick fresnel F0 = mix(0.08 x specular, base,
//   metallic) over a sky-over-ground environment and a GGX highlight of the key light (white highlights, as KH3's);
// - the emissive and the coloured light SOFT-CLIPPED ON THEIR BRIGHTEST CHANNEL (hue and saturation kept) to
//   KH_ORB_PEAK x white, so a bright orb stays green / blue in its core instead of burning to white in the host's tone
//   mapper; the glow around it is its own additive layers (and the host's bloom).
const KH_ORB_PEAK: f32 = 1.15;
const KH_ORB_KNEE: f32 = 0.55;
const KH_ORB_KEY: f32 = 1.4;

fn kh_orb_soft(c: vec3<f32>, white: f32) -> vec3<f32> {
    let m = max(c.x, max(c.y, c.z));
    let knee = KH_ORB_KNEE * white;
    let top = KH_ORB_PEAK * white;
    if (m <= knee) {
        return c;
    }
    let m2 = knee + (top - knee) * (1.0 - exp(-(m - knee) / (top - knee)));
    return c * (m2 / m);
}

fn kh_orb_shade(o: KhOut, n: vec3<f32>, l: vec3<f32>, v: vec3<f32>, white: vec3<f32>) -> vec3<f32> {
    let w = max(dot(white, vec3<f32>(0.2126, 0.7152, 0.0722)), 1.0e-4);
    let e = max(o.o0.xyz, vec3<f32>(0.0));
    let peak = max(e.x, max(e.y, e.z));
    let base = select(saturate(o.o3.xyz), e / max(peak, 1.0e-6), peak > 1.0);
    let metallic = saturate(o.o2.x);
    let specular = saturate(o.o2.y);
    let rough = clamp(o.o2.z, 0.08, 1.0);
    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 1.0e-3);
    let diffuse = base * (1.0 - metallic) * (0.45 + 0.75 * nl) * white;
    let f0 = mix(vec3<f32>(0.08 * specular), base, metallic);
    let fr = f0 + (max(vec3<f32>(1.0 - rough), f0) - f0) * pow(1.0 - nv, 5.0);
    let r = reflect(-v, n);
    let env = mix(vec3<f32>(0.2), vec3<f32>(1.0), smoothstep(-0.5, 0.7, r.z)) * white;
    let h = normalize(l + v);
    let nh = max(dot(n, h), 0.0);
    let a2 = rough * rough * rough * rough;
    let dd = nh * nh * (a2 - 1.0) + 1.0;
    let d = a2 / (3.14159265 * dd * dd);
    let k = rough * rough * 0.5;
    let vis = 0.25 / ((nl * (1.0 - k) + k) * (nv * (1.0 - k) + k));
    let highlight = fr * min(d * vis, 40.0) * nl * KH_ORB_KEY * white;
    return kh_orb_soft(e + diffuse + fr * env * metallic, w) + fr * env * (1.0 - metallic) + highlight;
}
#endif
#endif

#ifdef ER_FRAGMENT
#ifdef KH_FXNAT
// (PARTY-NATIVE, the user 2026-11-05: "Donald and Goofy need to be rendering how Sora is rendering as well, as well as all of their
// effects"; 2026-11-03: "no estimating, it MUST be Elden Ring's lighting"; "and not just Sora, all of KH objects") THE LIT
// (DEFERRED) EFFECT MATERIALS ON THE CHARACTERS' NATIVE ROUTE. In KH3 these materials (the lit ice of Blizzard, the rocks, stones,
// sand, pots, keys, prize orbs ... : outputs o1..o3 = GBufferA..C) are drawn into the G-buffer and lit by the engine's own light
// passes. Here the same: their `*_gb` twin (KH_FX_GB, below) writes the KH G-buffer surface into the game's G-buffer in the early
// list beside the characters' (vfx.rs `gbuffer_replay`), the game's light passes light it, the characters' capture takes the light
// a white surface got at each texel, and the colour pass composes KH3's deferred result from it: emissive + base x (1 -
// metallic) x the game's diffuse light + the game's specular for the material's F0 (as er_char.wgsl kh_native_light / hy_f0_ratio).
// No fixed key light, no ambient constant. A pixel whose texel is not its own (no capture this frame, another surface there:
// the normal does not match) keeps the earlier shading for that pixel.
struct FxTap {
    // the game's diffuse light on white, and its specular for the G-buffer's F0, KH units; ok = 1 when the texel is this surface's
    diffuse: vec3<f32>,
    spec: vec3<f32>,
    ok: f32,
}

fn er_oct_decode_fx(e: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    let t = max(-n.z, 0.0);
    n.x += select(t, -t, n.x >= 0.0);
    n.y += select(t, -t, n.y >= 0.0);
    return normalize(n);
}

// game axes -> the host's (math.rs host_from_game: host = (game.y, game.z, game.x))
fn kh_fx_host_dir(n: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(n.y, n.z, n.x);
}

// a texel of the capture in the native layout (the octahedral normal + 2, any light colour): its normal match, -2 = not usable
fn kh_fx_tap_ok(a: vec4<f32>, b: vec4<f32>, n_host: vec3<f32>, cos_min: f32) -> f32 {
    if (b.x < 0.5 || b.y < 0.5 || !(a.x >= 0.0) || !(a.y >= 0.0) || !(a.z >= 0.0)) {
        return -2.0;
    }
    let c = dot(er_oct_decode_fx(b.xy - vec2<f32>(2.0)), n_host);
    return select(-2.0, c, c >= cos_min);
}

fn kh_fx_tap(n_game: vec3<f32>, pixel: vec2<i32>) -> FxTap {
    var out: FxTap;
    out.diffuse = vec3<f32>(0.0);
    out.spec = vec3<f32>(0.0);
    out.ok = 0.0;
    if (er.fxn.x < 0.5) {
        return out;
    }
    let size = vec2<i32>(textureDimensions(er_hy_light));
    let n_host = kh_fx_host_dir(n_game);
    // THE CHARACTERS' TEXEL RULE (er_char.wgsl hy_tap): its own texel, else the best of its 1 and 2 px neighbours with the normal
    // match relaxed by 0.15 (a masked edge whose G-buffer pixel fell beside it); none: not this route for the pixel
    var a = textureLoad(er_hy_light, clamp(pixel, vec2<i32>(0), size - vec2<i32>(1)), 0);
    var b = textureLoad(er_hy_geo, clamp(pixel, vec2<i32>(0), size - vec2<i32>(1)), 0);
    if (kh_fx_tap_ok(a, b, n_host, er.fxn.z) < -1.5) {
        var best = -1.5;
        for (var r = 1; r <= 2; r++) {
            for (var q = 0; q < 8; q++) {
                let o = vec2<i32>(select(select(0, -r, q == 3 || q == 5 || q == 6), r, q == 0 || q == 4 || q == 7), select(select(0, -r, q == 2 || q == 6 || q == 7), r, q == 1 || q == 4 || q == 5));
                let p = clamp(pixel + o, vec2<i32>(0), size - vec2<i32>(1));
                let a1 = textureLoad(er_hy_light, p, 0);
                let b1 = textureLoad(er_hy_geo, p, 0);
                let c = kh_fx_tap_ok(a1, b1, n_host, er.fxn.z - 0.15);
                if (c > best) {
                    best = c;
                    a = a1;
                    b = b1;
                }
            }
            if (best > -1.5) {
                break;
            }
        }
        if (best < -1.5) {
            return out;
        }
    }
    // scene units -> KH units: this harness hands its colour over x out_scale.x
    let k = 1.0 / max(er.out_scale.x, 1.0e-9);
    out.diffuse = min(a.xyz, vec3<f32>(6.0e4)) * k;
    out.spec = min(max(vec3<f32>(b.z, b.w, a.w), vec3<f32>(0.0)), vec3<f32>(6.0e4)) * k;
    out.ok = 1.0;
    return out;
}

// The game's specular was worked out for the G-buffer's F0 (lerp(0.04, white, metalness at most 0.85)); the material's own F0
// (UE 4.17: lerp(0.08 x specular, base colour, metallic)) over it (er_char.wgsl hy_f0_ratio)
fn kh_fx_f0_ratio(o: KhOut, base: vec3<f32>) -> vec3<f32> {
    let metal = clamp(o.o2.x, 0.0, 1.0);
    let f0_kh = mix(vec3<f32>(0.08 * saturate(o.o2.y)), base, metal);
    let f0_gb = mix(vec3<f32>(0.04), vec3<f32>(1.0), min(metal, 0.85));
    return max(f0_kh, vec3<f32>(0.0)) / f0_gb;
}
#endif

#ifdef KH_FX_GB
// (PARTY-NATIVE) the G-buffer twin's three targets (er_char.wgsl ErGbuf, gb_main's native rows): the world normal (host axes),
// (metalness at most 0.85, gloss, translucency 0, 1), the KH white with the light-set mark
struct FxGbuf {
    @location(0) normal: vec4<f32>,
    @location(1) params: vec4<f32>,
    @location(2) albedo: vec4<f32>,
}

fn kh_fx_gbuffer(o: KhOut) -> FxGbuf {
    var g: FxGbuf;
    let raw = o.o1.xyz * 2.0 - vec3<f32>(1.0);
    let len = length(raw);
    let n = select(vec3<f32>(0.0, 0.0, 1.0), raw / max(len, 1.0e-6), len > 1.0e-6 && len < 1.0e6);
    g.normal = vec4<f32>(kh_fx_host_dir(n) * 0.5 + vec3<f32>(0.5), 0.0);
    let rough = clamp(o.o2.z, 0.0, 1.0);
    let gloss = clamp((1.0 - rough) / 0.95, 0.0, 1.0);
    g.params = vec4<f32>(clamp(o.o2.x, 0.0, 0.85), gloss * gloss, 0.0, 1.0);
    g.albedo = vec4<f32>(1.0, 1.0, 1.0, er.fxn.y);
    return g;
}
#endif

#ifdef KH_FXFOG
// (PARTY-NATIVE, the user 2026-11-05: "literally the lighting code needs to work for ALL KH objects") THE GAME'S OWN FOG ON A KH
// TRANSLUCENT EFFECT, at the effect pixel's own view depth: GXVolumetricFog_SceneComposite's function, line for line
// (er78/dis/rend/GXVolumetricFog_SceneComposite.ppo.txt: the slice of the log-depth volume, the analytic height fog beyond it,
// the per-channel extinction), with the effect's depth in place of the scene's. Its rows are CB_SHADER_COMMON_PARAM's (row =
// offset / 16): 2 DepthRangeInfo, 3 LogDepthRangeInfo, 4 PreComputed, 5..8 ViewMatrix (columns), 9 (ViewTangent,
// DepthComputeParam), 10 g_HeightFactor, 11 Global, 13 g_HeightFogParam, 35 (ExtinctionScale, PreExposure), 36 (ScaledSceneSize,
// EnableAnalyticalFog, AnalyticalFogEndDepth). Returns (in-scatter rgb in the scene target's units, transmittance per channel).
fn fx_fog_row(k: i32) -> vec4<f32> {
    return bitcast<vec4<f32>>(textureLoad(er_fog_rows, vec2<i32>(k, 0), 0));
}

struct FxFog {
    inscatter: vec3<f32>,
    transmit: vec3<f32>,
}

fn kh_fx_fog(uv: vec2<f32>, view_z: f32) -> FxFog {
    var f: FxFog;
    f.inscatter = vec3<f32>(0.0);
    f.transmit = vec3<f32>(1.0);
    if (er.fxf.x < 0.5) {
        return f;
    }
    let r2 = fx_fog_row(2);
    let r3 = fx_fog_row(3);
    let r4 = fx_fog_row(4);
    let r9 = fx_fog_row(9);
    let r35 = fx_fog_row(35);
    let r36 = fx_fog_row(36);
    let analytic = textureLoad(er_fog_rows, vec2<i32>(36, 0), 0).z != 0u;
    let z = view_z;
    // the volume's slice (log2 of the depth, as the composite)
    let slice = ((log2(r3.y + min(z, r2.y)) - r3.x) * r3.w + r4.y) * r4.z;
    let s = textureSampleLevel(er_fog_vol, kh_sclamp, vec3<f32>(uv, slice), 0.0);
    var inscatter = s.xyz;
    var t_vol = s.w;
    var t_od = 1.0;
    if (z > r2.y && analytic) {
        let zz = min(r36.w, z);
        let px = zz * (uv.x * 2.0 - 1.0) * r9.x;
        let py = zz * (1.0 - uv.y * 2.0) * r9.y;
        let r10 = fx_fog_row(10);
        let start = vec4<f32>(px * r2.y / zz, py * r2.y / zz, r2.y, 1.0);
        let h0 = dot(start, r10);
        let h1 = dot(vec4<f32>(px, py, zz, 1.0), r10);
        let global = fx_fog_row(11).x;
        let hf = fx_fog_row(13).x;
        let hd = fx_fog_row(13).y;
        let len = sqrt(px * px + py * py + zz * zz);
        let seg = (1.0 - r2.y / zz) * len;
        var od = 0.0;
        if (abs(hf * (h1 - h0)) < 1.0e-4) {
            let e = select(2.0 - exp(h0 * hf), exp(-h0 * hf), h0 > 0.0);
            od = (e * hd + global) * seg;
        } else {
            let w = fma(zz, fx_fog_row(7).y, fma(py, fx_fog_row(6).y, fx_fog_row(5).y * px));
            let k = abs(len / w);
            let mx = max(h0, h1);
            let mn = min(h0, h1);
            let a = (exp(-hf * max(mx, 0.0)) - exp(-hf * max(mn, 0.0))) / hf;
            let b = (min(mx, 0.0) - min(mn, 0.0)) * 2.0;
            let c = (exp(hf * min(mx, 0.0)) - exp(hf * min(mn, 0.0))) / hf;
            od = k * hd * (b - a - c) + seg * global;
        }
        let far = textureSampleLevel(er_fog_vol, kh_sclamp, vec3<f32>(uv, 1.0), 0.0).xyz;
        t_od = exp(-od);
        let ext = max(r35.xyz, vec3<f32>(1.0e-6));
        inscatter += pow(vec3<f32>(max(t_vol, 1.0e-12)), ext) * ((vec3<f32>(1.0) - pow(vec3<f32>(max(t_od, 1.0e-12)), ext)) * far / ext);
    }
    f.inscatter = max(inscatter, vec3<f32>(0.0));
    f.transmit = pow(vec3<f32>(clamp(t_vol * t_od, 1.0e-12, 1.0)), max(r35.xyz, vec3<f32>(1.0e-6)));
    return f;
}
#endif

#ifdef KH_FXLIT
// (PARTY-NATIVE-LIGHT, the user 2026-11-05: "literally the lighting code needs to work for ALL KH objects"; "no estimating, it
// MUST be Elden Ring's lighting") ELDEN RING'S OWN LIGHT ON A KH LIT TRANSLUCENT EFFECT, as Elden Ring lights its own lit
// particles (build/sora_tmp/weapons/PARTY-NATIVE/ffx/FFX_FINDINGS.md 4; ffx/dis/GS_VFX_StandardParticleBumpLight.gpo.txt = G,
// er78/dis/rend/GXEnvMapRegion[Box][Diff].ppo.txt = R), at the effect pixel: the L1 SH of the irradiance volumes + the
// tile-clustered point / spot lights, and the sun x its cascade shadow. The inputs are this frame's copies (vfxlight.rs LitKit):
// t18 the rows (raw words: the tile lighting's cbSceneParam from LIT_ROW_SCENE and cbLightAccParam from LIT_ROW_LACC, the light
// buffers' counts, the particle pass's gcbSceneParam from LIT_ROW_PASS, the regions' CB_ENVMAP_REGION from LIT_ROW_REGION),
// t19..t26 the volumes, t27 the light buffers, t28 / t29 the game's cascade atlas and the cascades' constants.
const LIT_ROW_SCENE: i32 = 0;
const LIT_ROW_LACC: i32 = 126;
const LIT_ROW_COUNTS: i32 = 150;
const LIT_ROW_PASS: i32 = 160;
const LIT_ROW_REGION: i32 = 230;
const LIT_REGION_STRIDE: i32 = 42;
const LIT_LISTS_W: u32 = 1024u;

struct LitSh {
    r: vec4<f32>,
    g: vec4<f32>,
    b: vec4<f32>,
}

fn lit_row(k: i32) -> vec4<f32> {
    return bitcast<vec4<f32>>(textureLoad(er_lit_rows, vec2<i32>(k, 0), 0));
}

fn lit_rowu(k: i32) -> vec4<u32> {
    return textureLoad(er_lit_rows, vec2<i32>(k, 0), 0);
}

// three rows of one of the game's matrices as its shaders read them (out_k = dot(register base + k, v))
fn lit_m3(base: i32, v: vec4<f32>) -> vec3<f32> {
    return vec3<f32>(dot(lit_row(base), v), dot(lit_row(base + 1), v), dot(lit_row(base + 2), v));
}

fn lit_zero() -> LitSh {
    return LitSh(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
}

fn lit_scaled(s: LitSh, k: f32) -> LitSh {
    return LitSh(s.r * k, s.g * k, s.b * k);
}

fn lit_iv_tap(k: i32, uv: vec3<f32>) -> vec4<f32> {
    var t = vec4<f32>(0.5);
    switch k {
        case 0: { t = textureSampleLevel(er_iv0, kh_sclamp, uv, 0.0); }
        case 1: { t = textureSampleLevel(er_iv1, kh_sclamp, uv, 0.0); }
        case 2: { t = textureSampleLevel(er_iv2, kh_sclamp, uv, 0.0); }
        case 3: { t = textureSampleLevel(er_iv3, kh_sclamp, uv, 0.0); }
        case 4: { t = textureSampleLevel(er_iv4, kh_sclamp, uv, 0.0); }
        case 5: { t = textureSampleLevel(er_iv5, kh_sclamp, uv, 0.0); }
        case 6: { t = textureSampleLevel(er_iv6, kh_sclamp, uv, 0.0); }
        case 7: { t = textureSampleLevel(er_iv7, kh_sclamp, uv, 0.0); }
        default: {}
    }
    return t;
}

// one basis of a volume: (rgb - 0.5) x 2 x exp((a - 0.5) x 20) (G:860-898, R: the same)
fn lit_decode(t: vec4<f32>) -> vec3<f32> {
    return (t.xyz - vec3<f32>(0.5)) * 2.0 * exp((t.w - 0.5) * 20.0);
}

// ONE IRRADIANCE VOLUME's SH at p (G:773-933; R): q = M_q (p, 1); uv = ((q + 1) / 2 - 0.5) x safe voxel + 0.5; + normalize(M_o (p
// + n, 1) - q) / sampling; the four bases X Y Z W decoded, each through the colour convert (channel c = row conv + c . rgb):
// SH_c = (X'_c, Y'_c, Z'_c, W'_c)
fn lit_iv(vol: i32, qb: i32, ob: i32, p: vec3<f32>, n: vec3<f32>, safe: vec3<f32>, samp: vec3<f32>, conv: i32) -> LitSh {
    let q = lit_m3(qb, vec4<f32>(p, 1.0));
    var uv = ((q + vec3<f32>(1.0)) * 0.5 - vec3<f32>(0.5)) * safe + vec3<f32>(0.5);
    let o = lit_m3(ob, vec4<f32>(p + n, 1.0)) - q;
    let ol = dot(o, o);
    if (ol > 1.0e-20) {
        uv += (o * inverseSqrt(ol)) / select(samp, vec3<f32>(1.0), abs(samp) < vec3<f32>(1.0e-12));
    }
    let x = lit_decode(lit_iv_tap(vol, uv));
    let y = lit_decode(lit_iv_tap(vol + 1, uv));
    let z = lit_decode(lit_iv_tap(vol + 2, uv));
    let w = lit_decode(lit_iv_tap(vol + 3, uv));
    let c0 = lit_row(conv).xyz;
    let c1 = lit_row(conv + 1).xyz;
    let c2 = lit_row(conv + 2).xyz;
    return LitSh(vec4<f32>(dot(x, c0), dot(y, c0), dot(z, c0), dot(w, c0)), vec4<f32>(dot(x, c1), dot(y, c1), dot(z, c1), dot(w, c1)), vec4<f32>(dot(x, c2), dot(y, c2), dot(z, c2), dot(w, c2)));
}

// THE PARTICLE PASS's two volumes (gcbSceneParam rows 29..31 / 33..35 the inverse matrices A / B, 37 (interp, -, scale A, scale
// B), 38 (enable A, enable B), 39 / 40 sampling, 41 / 42 safe voxel, 52..54 the colour convert): SH = lerp(A x scale A, B x scale
// B, interp) (G:1115-1170); nb = the billboard normal (toward the camera, G:529-531)
fn lit_pass_sh(p: vec3<f32>, nb: vec3<f32>, mask: u32) -> LitSh {
    let g = LIT_ROW_PASS;
    let blend = lit_row(g + 37);
    let ab_on = lit_row(g + 38);
    var a = lit_zero();
    var b = lit_zero();
    if (ab_on.x > 0.0 && (mask & 0x0fu) == 0x0fu) {
        a = lit_iv(0, g + 29, g + 29, p, nb, lit_row(g + 41).xyz, lit_row(g + 39).xyz, g + 52);
    }
    if (ab_on.y > 0.0 && (mask & 0xf0u) == 0xf0u) {
        b = lit_iv(4, g + 33, g + 33, p, nb, lit_row(g + 42).xyz, lit_row(g + 40).xyz, g + 52);
    }
    a = lit_scaled(a, blend.z);
    b = lit_scaled(b, blend.w);
    return LitSh(mix(a.r, b.r, blend.x), mix(a.g, b.g, blend.x), mix(a.b, b.b, blend.x));
}

// THE DEFERRED REGIONS (a frame without a particle draw: R): each region's box (CB_ENVMAP_REGION rows 4..6 the inverse world,
// 8 the extents, 9.z the inverse fade distance, 20 (fade rate, diffuse scale), 21..23 the volume's inverse matrix, 25 sampling,
// 32 safe voxel; the colour convert = cbSceneParam SC_EnvColorConvert 89..91), blended over the one before in draw order with
// its fade x fade rate (R: outside the box nothing)
fn lit_region_sh(p: vec3<f32>, n: vec3<f32>, mask: u32, regions: i32) -> LitSh {
    var s = lit_zero();
    for (var r = 0; r < 2; r++) {
        if (r >= regions || ((mask >> u32(4 * r)) & 0x0fu) != 0x0fu) {
            continue;
        }
        let b = LIT_ROW_REGION + LIT_REGION_STRIDE * r;
        let q = lit_m3(b + 4, vec4<f32>(p, 1.0));
        let e = lit_row(b + 8).xyz * (vec3<f32>(1.0) - abs(q));
        let m = min(e.x, min(e.y, e.z));
        if (m < 0.0) {
            continue;
        }
        let t = saturate(lit_row(b + 9).z * m);
        let a = t * t * (3.0 - 2.0 * t) * lit_row(b + 20).x;
        var v = lit_iv(4 * r, b + 4, b + 21, p, n, lit_row(b + 32).xyz, lit_row(b + 25).xyz, LIT_ROW_SCENE + 89);
        // (PARTY-NATIVE-LIGHT round 3) a two-volume region (GXEnvMapRegion[Box][Diff][Lerp].ppo.txt:454-620): its second volume (the
        // slots 4..7) at the same place with the safe voxel row 33, the two blended by row 26.x, then the diffuse scale
        if (r == 0 && er.fxl[2].w > 0.5) {
            if ((mask & 0xf0u) != 0xf0u) {
                continue;
            }
            let v2 = lit_iv(4, b + 4, b + 21, p, n, lit_row(b + 33).xyz, lit_row(b + 25).xyz, LIT_ROW_SCENE + 89);
            let w = lit_row(b + 26).x;
            v = LitSh(mix(v.r, v2.r, w), mix(v.g, v2.g, w), mix(v.b, v2.b, w));
        }
        v = lit_scaled(v, lit_row(b + 20).y);
        s = LitSh(mix(s.r, v.r, a), mix(s.g, v.g, a), mix(s.b, v.b, a));
    }
    return s;
}

fn lit_list(t: u32) -> vec4<u32> {
    return textureLoad(er_lit_lists, vec2<i32>(i32(t % LIT_LISTS_W), i32(t / LIT_LISTS_W)), 0);
}

fn lit_listf(t: u32) -> vec4<f32> {
    return bitcast<vec4<f32>>(lit_list(t));
}

// ONE LOCAL LIGHT's SH without its colour (G:1316-1380 / 1423-1606): d = the point - the light (view space); att = window / (1 +
// 0.7 (|d| x DiffColor.w)^2); near = (1.4 / (|d| x DiffColor.w + 1.4))^2: (0.25 Ldir (1 - near), 0.25 + 0.75 near) x att
fn lit_light(d: vec3<f32>, window: f32, col: vec4<f32>) -> vec4<f32> {
    let dd = dot(d, d);
    let x = sqrt(dd) * col.w;
    let att = window / (1.0 + 0.7 * x * x);
    let k = 1.4 / (x + 1.4);
    let near = k * k;
    let inv = select(inverseSqrt(dd), 3.4e38, dd == 0.0);
    return vec4<f32>(-d * inv * 0.25 * (1.0 - near), 0.75 * near + 0.25) * att;
}

fn lit_add(s: LitSh, v: vec4<f32>, col: vec3<f32>) -> LitSh {
    return LitSh(s.r + v * col.x, s.g + v * col.y, s.b + v * col.z);
}

// THE TILE-CLUSTERED LOCAL LIGHTS at view-space pv and pixel px (G:1180-1660; the cell of the deferred tile lighting: cbSceneParam
// 66 (FC_LightTile_Width, NumCellX), 65 (FC_FrustumNear, SliceScale, SliceBias)): the cell's (start, point count | spot count
// << 16), each index's slice mask against the point's depth slice; linear parts in view space
fn lit_lights(pv: vec3<f32>, px: vec2<f32>) -> LitSh {
    var s = lit_zero();
    let c = lit_rowu(LIT_ROW_COUNTS);
    let tile = lit_rowu(LIT_ROW_SCENE + 66);
    if (er.fxl[0].w < 0.5 || tile.x == 0u || c.y == 0u) {
        return s;
    }
    let o1 = (c.x + 3u) / 4u;
    let o2 = o1 + c.y;
    let o3 = o2 + 13u * c.z;
    let p = vec2<u32>(max(px, vec2<f32>(0.0)));
    let cell = p.x / tile.x + (p.y / tile.x) * tile.y;
    if (cell >= c.y) {
        return s;
    }
    let look = lit_list(o1 + cell);
    let f = lit_row(LIT_ROW_SCENE + 65);
    let slice = clamp((log2(max(pv.z - f.x, 1.0e-6)) - f.z) / select(f.y, 1.0, f.y == 0.0), 0.0, 14.0);
    let bit = 1u << (u32(slice) & 31u);
    let points = min(look.y & 0xffffu, 256u);
    let spots = min(look.y >> 16u, 256u);
    let v4 = vec4<f32>(pv, 1.0);
    for (var i = 0u; i < points + spots; i++) {
        let at = look.x + i;
        if (at >= c.x) {
            break;
        }
        let e = lit_list(at / 4u)[at % 4u];
        let li = e & 0xffffu;
        if (((e >> 16u) & bit) == 0u) {
            continue;
        }
        if (i < points) {
            if (li >= c.z) {
                continue;
            }
            let pos = lit_listf(o2 + 13u * li);
            let col = lit_listf(o2 + 13u * li + 1u);
            let d = pv - pos.xyz;
            s = lit_add(s, lit_light(d, saturate(1.0 - dot(d, d) * pos.w), col), col.xyz);
        } else {
            if (li >= c.w) {
                continue;
            }
            let base = o3 + 12u * li;
            let pos = lit_listf(base);
            let col = lit_listf(base + 1u);
            let z = dot(lit_listf(base + 6u), v4);
            if (!(z > 0.0)) {
                continue;
            }
            let w = dot(lit_listf(base + 7u), v4);
            let u = dot(lit_listf(base + 4u), v4) / w;
            let v = dot(lit_listf(base + 5u), v4) / w;
            let cone = saturate(1.0 - (u * u + v * v));
            let d = pv - pos.xyz;
            let window = saturate(1.0 - dot(d, d) / max(pos.w * pos.w, 1.0e-12));
            s = lit_add(s, lit_light(d, window, col) * cone, col.xyz);
        }
    }
    return s;
}

// a linear part from view space into the world (G:1608-1643: the view rotation transposed = cbSceneParam FC_MatrixInvView 20..22)
fn lit_to_world(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(lit_m3(LIT_ROW_SCENE + 20, vec4<f32>(v.xyz, 0.0)), v.w);
}

// THE SH AT THIS EFFECT PIXEL (KH units, into kh_lit_sh): its world position from the device depth through the tile lighting's
// FC_MatrixInvViewProj (cbSceneParam 8..11, as R), its view position through cbLightAccParam g_InvProjMatrix (0..3, as
// GXRend_TileBasedLighting); the volumes (the particle pass's, else the regions') + the local lights; x SC_PreExposure (72.z) /
// this harness's out scale
fn kh_lit_at(pos: vec4<f32>) {
    let uv = pos.xy * er.size.zw;
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, pos.z, 1.0);
    let wp = vec4<f32>(lit_m3(LIT_ROW_SCENE + 8, ndc), dot(lit_row(LIT_ROW_SCENE + 11), ndc));
    let p = wp.xyz / select(wp.w, 1.0, wp.w == 0.0);
    let lv = vec4<f32>(lit_m3(LIT_ROW_LACC, ndc), dot(lit_row(LIT_ROW_LACC + 3), ndc));
    let pv = lv.xyz / select(lv.w, 1.0, lv.w == 0.0);
    let cw = lit_row(LIT_ROW_SCENE + 23).w;
    let cam = vec3<f32>(lit_row(LIT_ROW_SCENE + 20).w, lit_row(LIT_ROW_SCENE + 21).w, lit_row(LIT_ROW_SCENE + 22).w) / select(cw, 1.0, cw == 0.0);
    let to_cam = cam - p;
    let nb = select(vec3<f32>(0.0, 0.0, 1.0), normalize(to_cam), dot(to_cam, to_cam) > 1.0e-12);
    let mask = u32(er.fxl[0].z + 0.5);
    var s = lit_zero();
    if (er.fxl[0].y > 0.5 && er.fxl[0].y < 1.5) {
        s = lit_pass_sh(p, nb, mask);
    } else if (er.fxl[0].y > 1.5) {
        s = lit_region_sh(p, nb, mask, i32(er.fxl[2].y + 0.5));
    }
    let l = lit_lights(pv, pos.xy);
    let k = lit_row(LIT_ROW_SCENE + 72).z / max(er.out_scale.x, 1.0e-9);
    kh_lit_sh[0] = (s.r + lit_to_world(l.r)) * k;
    kh_lit_sh[1] = (s.g + lit_to_world(l.g)) * k;
    kh_lit_sh[2] = (s.b + lit_to_world(l.b)) * k;
}

// the light on a white surface facing n (world axes) from the SH: max(dot((n, 1), SH_c), 0) (the particle's pixel shader,
// FFX_FINDINGS.md 4.2)
fn kh_lit_e(n: vec3<f32>) -> vec3<f32> {
    let v = vec4<f32>(n, 1.0);
    return max(vec3<f32>(dot(v, kh_lit_sh[0]), dot(v, kh_lit_sh[1]), dot(v, kh_lit_sh[2])), vec3<f32>(0.0));
}

// THE SUN'S SHADOW at the effect pixel from the game's own cascades (hybrid.rs csm_visibility, er_char.wgsl hc_sun_vis without
// the KH maps' owner rule: every caster in the atlas shades an effect): of the cascades holding the point, the finest; a
// bilinear PCF over 3 x 3 texels; 1 = no cascade bound or none holds it
fn kh_lit_sun_vis(game_pos: vec3<f32>) -> f32 {
    let rows = i32(er.fxl[3].x + 0.5);
    if (rows < 1 || er.fxl[2].x < 0.5) {
        return 1.0;
    }
    let m = mat4x4<f32>(er.fxl[9], er.fxl[10], er.fxl[11], er.fxl[12]);
    let host = (m * vec4<f32>(game_pos, 1.0)).xyz;
    var best_r = -1;
    var best_texel = 1.0e30;
    var best_s = vec3<f32>(0.0);
    for (var r = 0; r < 5; r++) {
        if (r >= rows) {
            break;
        }
        let rel = vec4<f32>(host - textureLoad(er_sun_casc, vec2<i32>(3, r), 0).xyz, 1.0);
        let row0 = textureLoad(er_sun_casc, vec2<i32>(24, r), 0);
        let c = vec4<f32>(dot(row0, rel), dot(textureLoad(er_sun_casc, vec2<i32>(25, r), 0), rel), dot(textureLoad(er_sun_casc, vec2<i32>(26, r), 0), rel), dot(textureLoad(er_sun_casc, vec2<i32>(27, r), 0), rel));
        if (!(c.w > 1.0e-6)) {
            continue;
        }
        let s = c.xyz / c.w;
        if (abs(s.x) > 0.97 || abs(s.y) > 0.97 || s.z <= 0.0 || s.z >= 1.0) {
            continue;
        }
        let texel = 2.0 / max(length(row0.xyz) * er.fxl[4 + r].z, 1.0e-9);
        if (texel < best_texel) {
            best_texel = texel;
            best_r = r;
            best_s = s;
        }
    }
    if (best_r < 0) {
        return 1.0;
    }
    let vp = er.fxl[4 + best_r];
    let px = vp.xy + vec2<f32>(best_s.x * 0.5 + 0.5, 0.5 - best_s.y * 0.5) * vp.zw;
    let step = max(er.fxl[3].y, 0.0);
    let bias = er.fxl[3].z / 65535.0;
    if (step <= 0.0) {
        return select(0.0, 1.0, textureLoad(er_sun_atlas, vec2<i32>(floor(px)), 0).x <= best_s.z + bias);
    }
    let g = (px - vec2<f32>(0.5 * step)) / step;
    let b = floor(g);
    let f = g - b;
    let wx = vec4<f32>(1.0 - f.x, 1.0, 1.0, f.x);
    let wy = vec4<f32>(1.0 - f.y, 1.0, 1.0, f.y);
    var lit = 0.0;
    for (var j = 0; j < 4; j++) {
        for (var i = 0; i < 4; i++) {
            let at = (b + vec2<f32>(f32(i) - 1.0, f32(j) - 1.0)) * step + vec2<f32>(0.5 * step);
            if (textureLoad(er_sun_atlas, vec2<i32>(floor(at)), 0).x <= best_s.z + bias) {
                lit += wx[i] * wy[j];
            }
        }
    }
    return lit / 9.0;
}
#endif

@fragment
#ifdef KH_FX_GB
fn fs_main(in: KhVaryings) -> FxGbuf {
#else
fn fs_main(in: KhVaryings) -> @location(0) vec4<f32> {
    if (er.out_scale.y != 0.0) {
        // Diagnosis: what the geometry covers, whatever its shader would draw.
        return vec4<f32>(2.0 * er.out_scale.x, 0.0, 2.0 * er.out_scale.x, 1.0);
    }
#endif
    var i: KhIn;
    let time = in.misc.x;
    let camera = er.camera.xyz;
    i.twp = in.world_position - camera;
    // SV_Position: pixel centre, device depth, and w = view depth in cm. DIRECT3D HANDS THE PIXEL STAGE
    // THE CLIP-SPACE w ITSELF (view depth, here metres), not its reciprocal as WGSL defines the builtin
    // (and as the test app gets it on Vulkan): naga passes SV_Position through unchanged, and this file
    // is only ever built for Direct3D. (Seen: with the reciprocal every soft particle vanished: its
    // "pixel depth" was a few centimetres, inside the materials' near-camera fade.)
    i.svpos = vec4<f32>(in.position.xy, in.position.z, 100.0 * in.position.w);
    // SV_IsFrontFace from the shading normal (the two-sided materials only use the flag to make the normal face the viewer).
    i.front = vec4<f32>(select(0.0, 1.0, dot(in.normal, -i.twp) >= 0.0));
    i.inv_scene_depth = 0.0;
    let pixel = vec2<i32>(in.position.xy);
    if (er.depth.z != 0.0 && kh.er.x == 0.0) {
        // Reversed-Z device depth of the scene behind this pixel -> 1 / (view depth in cm).
        let device = textureLoad(er_depth, pixel, 0).x;
        i.inv_scene_depth = (device * er.depth.x + er.depth.y) * 0.01;
    }
#ifdef KH_DECAL
    // The decal's box is drawn; what it shades is the scene point behind each of its pixels, found
    // from the scene depth and taken into the box's own space (rows of the world -> decal matrix
    // ride in the vertex colour, dynamic and particle attributes).
    i.decal_pos = vec3<f32>(9.0);
    if (er.depth.z == 0.0) {
        discard;
    }
    {
        let device_d = textureLoad(er_depth, pixel, 0).x;
        if (device_d <= 0.0) {
            discard;
        }
        let scene_depth = 1.0 / (device_d * er.depth.x + er.depth.y);
        let frag_depth = in.position.w;
        let g = camera + (in.world_position - camera) * (scene_depth / frag_depth);
        i.decal_pos = vec3<f32>(dot(in.vcol.xyz, g) + in.vcol.w, dot(in.dynamic.xyz, g) + in.dynamic.w, dot(in.particle.xyz, g) + in.particle.w);
    }
    i.dcol = in.color;
    i.dprm = vec4<f32>(in.misc.yzw, 0.0);
    i.dz = vec4<f32>(in.uv.zw, 0.0, 0.0);
    i.d12 = vec4<f32>(1.0);
    i.d23 = vec4<f32>(in.misc.x);
    // (XW) the world -> decal rows are axis / scale: their directions are the decal's unit axes
    i.drot0 = vec4<f32>(normalize(in.vcol.xyz), 0.0);
    i.drot1 = vec4<f32>(normalize(in.dynamic.xyz), 0.0);
    i.drot2 = vec4<f32>(normalize(in.particle.xyz), 0.0);
    i.drot3 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
#endif
    let n = normalize(in.normal);
    let t = normalize(in.tangent);
    i.tc10 = vec4<f32>(t, in.misc.z);
    i.tc11 = vec4<f32>(n, in.misc.w);
    i.tc7 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.ptime = vec4<f32>(in.misc.y);
    i.ppos = in.particle;
    i.psubuv = in.uv;
    i.psize = vec4<f32>(time);
    i.col0 = in.vcol;
    i.col3 = vec4<f32>(0.0, 0.0, time, 0.0);
    i.tc0 = in.uv;
    // (a mesh: UV set 1 or the vertex shader's TEXCOORD1; a sprite: the dynamic parameter)
    i.tc1 = in.tc1;
    i.tc2 = in.color;
#ifdef KH_MESH
#ifdef KH_DECAL
#else
    // (ZA) a SubUV material on a mesh reads SubUV0AndTexCoord0 (TEXCOORD1) and SubUV1AndLerp (TEXCOORD2), which
    // FMeshParticleVertexFactory makes from the particle's sub image; misc.z < 0 carries that image (kh_vfx::sub_image_code)
    if (in.misc.z < -0.5) {
        let s = kh_mesh_subuv(in.uv.xy, in.misc.z);
        i.tc1 = s.sub0;
        i.tc2 = s.sub1;
        i.tc10.w = 0.0;
    }
#endif
#endif
    i.tc3 = in.uv;
    i.col1 = in.color;
    i.col2 = in.dynamic;
    // ---- View (slots identified by how the compiled shaders use them)
    let right = er.right.xyz;
    let up = er.up.xyz;
    let forward = er.forward.xyz;
    i.v8 = vec4<f32>(right.x, up.x, forward.x, 0.0);
    i.v9 = vec4<f32>(right.y, up.y, forward.y, 0.0);
    i.v10 = vec4<f32>(right.z, up.z, forward.z, 0.0);
    i.v16 = i.v8; // (VFX-2 cube)
    i.v17 = i.v9;
    i.v18 = i.v10;
    i.v19 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.v53 = vec4<f32>(0.0, 0.0, 1.0, 0.0);
    i.v54 = vec4<f32>(0.5, -0.5, 0.5, 0.5);
    i.v55 = vec4<f32>(camera, 0.0);
    i.v58 = vec4<f32>(-camera, 0.0);
    i.v117 = vec4<f32>(0.0);
    i.v118 = er.size;
    i.v119 = er.size;
    i.v122 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.v123 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.v124 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.v125 = vec4<f32>(0.0, 1.0, 0.0, 0.0);
    i.v126 = vec4<f32>(0.0);
    i.v127 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.v128 = vec4<f32>(time);
    i.v130 = vec4<f32>(0.0);
    i.v137 = vec4<f32>(0.0);
    i.v161 = vec4<f32>(1.0, 1.0, time, 0.0);
    i.p8 = vec4<f32>(0.0);
    i.p9 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.p11 = vec4<f32>(0.0);
    i.p17 = vec4<f32>(0.0);
    i.p18 = vec4<f32>(0.0);
    i.s10 = vec4<f32>(0.0, time, 0.0, 0.0);
    i.l5 = vec4<f32>(1.0);
    i.ptr0 = vec4<f32>(1.0, 0.0, 0.0, 0.0);
    i.ptr1 = vec4<f32>(0.0, 1.0, 0.0, 0.0);
    i.ptr2 = vec4<f32>(0.0, 0.0, 1.0, 0.0);
    i.dist12 = vec4<f32>(er.depth.w, er.size.x / er.size.y, 1920.0, 1920.0 * er.size.y / er.size.x);
    // (VFX-XLATE) no level tint (base 1, synthesis 0); the lit side white, the shadowed side the ambient share of the lit rule
    // above (0.45) [A: Elden Ring has no KH3 level colour table]
    i.pbase = vec4<f32>(1.0);
    i.pbright = vec4<f32>(1.0);
    i.pdark = vec4<f32>(0.45, 0.45, 0.45, 1.0);
    i.psynth = vec4<f32>(0.0);
    // (VFX-2 lit) Elden Ring's light for the forward-lit owners' scene colours
    i.fbright = select(i.pbright, vec4<f32>(er.fwd_env[3].xyz, 1.0), er.fwd_env[3].w > 0.5);
    i.fdark = select(i.pdark, vec4<f32>(er.fwd_env[4].xyz, 1.0), er.fwd_env[4].w > 0.5);
#ifdef KH_FXLIT
    // (PARTY-NATIVE-LIGHT) ELDEN RING'S OWN LIGHT in place of the forward lighting's inputs, at this pixel (kh_lit_at): the directional
    // light = the sun as read (Col_0 x SC_PreExposure, KH units) x its cascade shadow here (the material does its own N.L and
    // BRDF); no light-grid light (the game's local lights are in the SH, with its own falloff); the capture = the SH (kh_fwd_env);
    // the scene colours = the light a white surface facing the effect's normal gets as Elden Ring lights its particles: shaded =
    // the SH, lit = the SH + the sun x shadow x N.L / pi
    kh_fwdg = er.fwd_g;
    kh_lit_sh = array<vec4<f32>, 3>(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    if (er.fxl[0].x > 0.5) {
        kh_lit_at(in.position);
        let k = lit_row(LIT_ROW_SCENE + 72).z / max(er.out_scale.x, 1.0e-9);
        let has_sun = er.fxl[1].w > 0.5 && kh_fwdg[0].z > 0.5;
        let sun = select(vec3<f32>(0.0), er.fxl[1].xyz * k, has_sun);
        let vis = kh_lit_sun_vis(in.world_position);
        let to_sun = kh_fwdg[4].xyz;
        kh_fwdg[0] = vec4<f32>(0.0, kh_fwdg[0].y, select(0.0, 1.0, has_sun), kh_fwdg[0].w);
        kh_fwdg[5] = vec4<f32>(sun, kh_fwdg[5].w);
        kh_fwdg[7] = vec4<f32>(vis, 0.0, kh_fwdg[7].z, kh_fwdg[7].w);
        let ng = select(vec3<f32>(0.0, 0.0, 1.0), normalize(in.normal), dot(in.normal, in.normal) > 1.0e-12);
        let dark = kh_lit_e(kh_fx_host_dir(ng));
        let bright = dark + sun * vis * saturate(dot(ng, to_sun)) / 3.14159265;
        i.fbright = vec4<f32>(bright, 1.0);
        i.fdark = vec4<f32>(dark, 1.0);
        i.pbright = i.fbright;
        i.pdark = i.fdark;
    }
#endif

#ifdef KH_MASK
    // The opacity mask of a masked material (the game clips in its depth-only pass).
    kh_mask(i);
#endif
#ifdef KH_DISTORT
    // The material's distortion pass (refraction): the scene behind the surface, read at the
    // offset its shader asks for, from the copy of the scene colour made before these draws.
    let acc = kh_distort(i).o0;
    let offset = (acc.xy - acc.zw) * 0.25;
    let scene_uv = clamp(in.position.xy * er.size.zw + offset, vec2<f32>(0.0), vec2<f32>(1.0));
    return vec4<f32>(textureSampleLevel(er_scene, er_sscene, scene_uv, 0.0).xyz, 1.0);
#else
    let o = kh_material(i);
#ifdef KH_FX_GB
    // (PARTY-NATIVE) the G-buffer twin: the material's surface into the game's G-buffer (its opacity mask clipped above)
    return kh_fx_gbuffer(o);
#else
    var color = o.o0;
#ifdef KH_DEFERRED
    // A lit (deferred) material: its emissive plus its base colour under a key light and an
    // ambient term (the test app's formula; `er.lit` is what white shows in this host's scene).
    let normal = normalize(o.o1.xyz * 2.0 - vec3<f32>(1.0));
    let light = normalize(vec3<f32>(0.35, 0.25, 0.9));
    let diffuse = 0.45 + 0.75 * max(dot(normal, light), 0.0);
#ifdef KH_ORB
    // (ORBS) the prize orbs' balls: their own lit shading (kh_orb_shade)
    color = vec4<f32>(kh_orb_shade(o, normal, light, normalize(-i.twp), er.lit.xyz), 1.0);
#else
    color = vec4<f32>(o.o0.xyz + o.o3.xyz * diffuse * er.lit.xyz, 1.0);
#endif
#ifdef KH_FXNAT
    // (PARTY-NATIVE) the game's light at this texel, when it is this surface's (kh_fx_tap): KH3's deferred result in place of
    // the stand-in above
    let fx = kh_fx_tap(normal, pixel);
    if (fx.ok > 0.5) {
#ifdef KH_ORB
        // (ORBS) the orbs' own base colour and soft clip of the emissive kept (kh_orb_shade), the game's light in place of the
        // key, sky and highlight of ours
        let e = max(o.o0.xyz, vec3<f32>(0.0));
        let peak = max(e.x, max(e.y, e.z));
        let base = select(saturate(o.o3.xyz), e / max(peak, 1.0e-6), peak > 1.0);
        let w = max(dot(er.lit.xyz, vec3<f32>(0.2126, 0.7152, 0.0722)), 1.0e-4);
        color = vec4<f32>(kh_orb_soft(e + base * (1.0 - saturate(o.o2.x)) * fx.diffuse, w) + fx.spec * kh_fx_f0_ratio(o, base), 1.0);
#else
        let base = o.o3.xyz;
        color = vec4<f32>(o.o0.xyz + base * (1.0 - saturate(o.o2.x)) * fx.diffuse + fx.spec * kh_fx_f0_ratio(o, base), 1.0);
#endif
    }
#endif
#endif
    // The emitter's blend type against the shader's own (ParticleModuleRequired.EmitterBlendType).
    if (kh.misc.z != 0.0) {
        color = vec4<f32>(color.xyz * color.w, 0.0);
    }
    if (kh.misc.w != 0.0) {
        color = vec4<f32>(color.xyz, saturate(max(color.x, max(color.y, color.z))));
    }
    // (DON-MAGIC tint) a per-spawn recolour (kh_core::Effects::tint; kh-er-render puts it in er.yzw, zero = none): the
    // brightest channel times the tint, alpha kept
    if (kh.er.y + kh.er.z + kh.er.w > 0.0) {
        color = vec4<f32>(max(color.x, max(color.y, color.z)) * kh.er.yzw, color.w);
    }
    // Scene-linear colour in the host's units: the host's own tone mapping follows.
#ifdef KH_FXFOG
    // (PARTY-NATIVE) the game's fog at this effect pixel's own depth (kh_fx_fog), as its own particles take it: blended = colour x
    // transmittance + in-scatter (FogMultMode 0); additive = colour x transmittance (FogMultMode 1: many layers do not add the
    // fog colour many times); modulate = toward no change
    var scene = min(max(color.xyz, vec3<f32>(0.0)), vec3<f32>(1000.0)) * er.out_scale.x;
    if (kh.fx.x < 2.5) {
        let fog = kh_fx_fog(in.position.xy * er.size.zw, in.position.w);
        if (kh.fx.x < 0.5) {
            scene = scene * fog.transmit + fog.inscatter;
        } else if (kh.fx.x < 1.5) {
            scene = scene * fog.transmit;
        } else {
            scene = vec3<f32>(1.0) - (vec3<f32>(1.0) - scene) * fog.transmit;
        }
    }
    return vec4<f32>(scene, max(color.w, 0.0));
#else
    return vec4<f32>(min(max(color.xyz, vec3<f32>(0.0)), vec3<f32>(1000.0)) * er.out_scale.x, max(color.w, 0.0));
#endif
#endif
#endif
}
#endif

#ifdef ER_SCREEN
// ---- the offline harness's own passes (the standalone example): a full-screen triangle
struct ScreenOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_screen(@builtin(vertex_index) vertex: u32) -> ScreenOut {
    var out: ScreenOut;
    let p = vec2<f32>(f32((vertex << 1u) & 2u), f32(vertex & 2u));
    out.position = vec4<f32>(p * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = p;
    return out;
}

fn er_aces(c: vec3<f32>) -> vec3<f32> {
    // The fitted ACES curve (Stephen Hill's fit: what the test app's camera runs over its frame).
    let m_in = mat3x3<f32>(vec3<f32>(0.59719, 0.07600, 0.02840), vec3<f32>(0.35458, 0.90834, 0.13383), vec3<f32>(0.04823, 0.01566, 0.83777));
    let m_out = mat3x3<f32>(vec3<f32>(1.60475, -0.10208, -0.00327), vec3<f32>(-0.53108, 1.10813, -0.07276), vec3<f32>(-0.07367, -0.00605, 1.07602));
    let v = m_in * c;
    let a = v * (v + vec3<f32>(0.0245786)) - vec3<f32>(0.000090537);
    let b = v * (0.983729 * v + vec3<f32>(0.4329510)) + vec3<f32>(0.238081);
    return saturate(m_out * (a / b));
}

// The harness's "tone mapping": the scene target through the fitted ACES curve, sRGB encoded.
// er.out_scale.z: exposure (what the scene target's values are multiplied with first).
@fragment
fn fs_tone(in: ScreenOut) -> @location(0) vec4<f32> {
    let scene = textureLoad(er_scene, vec2<i32>(in.position.xy), 0).xyz * er.out_scale.z;
    let d = er_aces(scene);
    let lo = d * 12.92;
    let hi = 1.055 * pow(d, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return vec4<f32>(select(hi, lo, d <= vec3<f32>(0.0031308)), 1.0);
}

// The harness's "world": a flat floor at game z = 0 seen through the frame's camera, written into
// the scene target with its depth (a checker of 1 m tiles; er.lit = its colour scale), and a sky.
struct FloorOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fs_floor(in: ScreenOut) -> FloorOut {
    var out: FloorOut;
    // The view ray of the pixel in GAME space.
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let tan_x = 1.0 / er.depth.w;
    let tan_y = tan_x * er.size.y / er.size.x;
    let ray = normalize(er.forward.xyz + er.right.xyz * ndc.x * tan_x + er.up.xyz * ndc.y * tan_y);
    let sky = mix(vec3<f32>(0.52, 0.62, 0.74), vec3<f32>(0.24, 0.40, 0.68), saturate(ray.z * 1.5)) * er.lit.w;
    out.color = vec4<f32>(sky * er.out_scale.x, 1.0);
    out.depth = 0.0;
    if (ray.z < -1.0e-5 && er.camera.z > 0.0) {
        let distance = er.camera.z / -ray.z;
        let hit = er.camera.xyz + ray * distance;
        let clip = er.clip_from_game * vec4<f32>(hit, 1.0);
        let tile = floor(hit.x * 0.01) + floor(hit.y * 0.01);
        let shade = select(0.30, 0.36, (tile - 2.0 * floor(tile * 0.5)) < 0.5);
        let fade = saturate(distance / 12000.0);
        out.color = vec4<f32>(mix(vec3<f32>(shade) * er.lit.xyz, sky, fade) * er.out_scale.x, 1.0);
        out.depth = clamp(clip.z / clip.w, 0.0, 1.0);
    }
    // (IMPL_ER39, the light pass's proof) er.out_scale.w > 0: a wall across game X at that distance (cm), 4 m high,
    // with a 1 m gap in its middle at y = 0 (0: no wall, the effects harness as before)
    let wall_x = er.out_scale.w;
    if (wall_x > 0.0 && abs(ray.x) > 1.0e-5) {
        let t = (wall_x - er.camera.x) / ray.x;
        let hit = er.camera.xyz + ray * t;
        let floor_t = select(1.0e9, er.camera.z / max(-ray.z, 1.0e-6), ray.z < -1.0e-5);
        if (t > 0.0 && t < floor_t && hit.z >= 0.0 && hit.z <= 400.0 && abs(hit.y) > 50.0) {
            let clip = er.clip_from_game * vec4<f32>(hit, 1.0);
            let brick = floor(hit.z * 0.02) + floor(hit.y * 0.01 + 0.5 * floor(hit.z * 0.02));
            let shade = select(0.26, 0.32, (brick - 2.0 * floor(brick * 0.5)) < 0.5);
            out.color = vec4<f32>(vec3<f32>(shade) * er.lit.xyz * er.out_scale.x, 1.0);
            out.depth = clamp(clip.z / clip.w, 0.0, 1.0);
        }
    }
    return out;
}

#ifdef ER_CHARS
// (ZFIN, the offline harness only: erchr.ps.dxil) ELDEN RING'S CHARACTERS IN THE "G-BUFFER": capsule figures traced per
// pixel, written with their depth (the pipeline tests GREATER_EQUAL and writes the characters' stencil 0x50, as the game's
// characters leave it). Tints: 1 the finale's target (rusty armour), 2 a neighbour (green), 3 a blade (steel).
fn erchr_t(ro: vec3<f32>, rd: vec3<f32>, pa: vec3<f32>, pb: vec3<f32>, r: f32) -> f32 {
    let ba = pb - pa;
    let oa = ro - pa;
    let baba = dot(ba, ba);
    if (baba < 1.0e-4) {
        let b = dot(oa, rd);
        let h = b * b - (dot(oa, oa) - r * r);
        return select(-1.0, -b - sqrt(max(h, 0.0)), h > 0.0);
    }
    let bard = dot(ba, rd);
    let baoa = dot(ba, oa);
    let rdoa = dot(rd, oa);
    let oaoa = dot(oa, oa);
    let a = baba - bard * bard;
    let b = baba * rdoa - baoa * bard;
    let c = baba * oaoa - baoa * baoa - r * r * baba;
    let h = b * b - a * c;
    if (h >= 0.0) {
        let t = (-b - sqrt(h)) / max(a, 1.0e-6);
        let y = baoa + t * bard;
        if (y > 0.0 && y < baba) {
            return t;
        }
        let oc = select(oa, ro - pb, y > 0.0);
        let b2 = dot(rd, oc);
        let h2 = b2 * b2 - (dot(oc, oc) - r * r);
        if (h2 > 0.0) {
            return -b2 - sqrt(h2);
        }
    }
    return -1.0;
}

@fragment
fn fs_erchr(in: ScreenOut) -> FloorOut {
    var out: FloorOut;
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let tan_x = 1.0 / er.depth.w;
    let tan_y = tan_x * er.size.y / er.size.x;
    let rd = normalize(er.forward.xyz + er.right.xyz * ndc.x * tan_x + er.up.xyz * ndc.y * tan_y);
    let ro = er.camera.xyz;
    var best = 1.0e9;
    var hit = -1;
    for (var k = 0; k < 24; k++) {
        let a = er.ec[2 * k];
        let b = er.ec[2 * k + 1];
        if (b.w < 0.5) {
            break;
        }
        let t = erchr_t(ro, rd, a.xyz, b.xyz, a.w);
        if (t > 0.0 && t < best) {
            best = t;
            hit = k;
        }
    }
    if (hit < 0) {
        discard;
    }
    let a = er.ec[2 * hit];
    let b = er.ec[2 * hit + 1];
    let p = ro + rd * best;
    let ba = b.xyz - a.xyz;
    let s = clamp(dot(p - a.xyz, ba) / max(dot(ba, ba), 1.0e-4), 0.0, 1.0);
    let n = normalize(p - (a.xyz + ba * s));
    var tint = vec3<f32>(0.55, 0.34, 0.24);
    if (b.w > 1.5 && b.w < 2.5) {
        tint = vec3<f32>(0.26, 0.46, 0.33);
    }
    if (b.w > 2.5) {
        tint = vec3<f32>(0.72, 0.72, 0.78);
    }
    let sun = normalize(vec3<f32>(0.5, 0.3, 0.8));
    let shade = 0.3 + 0.7 * max(dot(n, sun), 0.0);
    out.color = vec4<f32>(tint * shade * er.lit.xyz * er.out_scale.x, 1.0);
    let clip = er.clip_from_game * vec4<f32>(p, 1.0);
    out.depth = clamp(clip.z / clip.w, 0.0, 1.0);
    return out;
}
#endif

// (IMPL_ER53, the offline harness only: fxtest.ps.dxil) THE GAME'S FORWARD STAGE STAND-IN: additive layers drawn after
// the deferred stage with the scene depth bound (tested GREATER_EQUAL, not written), as the game's translucent effects.
// er.lit.x = 1: a GLOW DECAL like the floor seal of the user's screenshot: the top face of its box (game z = er.lit.z,
// a square of half size er.lit.w around (er.lit.y, 0)) is what is rasterised and depth-tested; its pattern is projected
// from a DEPTH COPY (t13) the "game" made after its G-buffer, onto what that copy shows on the floor (|z| < 3 cm).
// er.lit.x = 2: a glowing PANEL across game X at er.lit.y, |y| < er.lit.z, 0 < z < er.lit.w (an effect sprite).
// er.lit.x = 3 (IMPL_ER56): a SKY GLOW like the Erdtree: golden branches over the upper sky, drawn where the stencil
// says "nothing drawn here" (the pipeline's stencil test; no depth test), as a sky pass masked by the G-buffer's stencil.
// er.out_scale.x: brightness; er.out_scale.y: 0 teal, 1 orange.
@fragment
fn fs_fxtest(in: ScreenOut) -> FloorOut {
    var out: FloorOut;
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let tan_x = 1.0 / er.depth.w;
    let tan_y = tan_x * er.size.y / er.size.x;
    let ray = normalize(er.forward.xyz + er.right.xyz * ndc.x * tan_x + er.up.xyz * ndc.y * tan_y);
    let tint = mix(vec3<f32>(0.25, 1.0, 0.9), vec3<f32>(1.0, 0.55, 0.15), er.out_scale.y);
    var hit = vec3<f32>(0.0);
    var glow = 0.0;
    // (IMPL_ER65, fxtest3) er.lit.x = 4: the same sky glow, masked by an EARLY depth copy (t13: made before his early depth)
    // instead of the stencil: where that copy shows the sky
    if (er.lit.x > 3.5) {
        if (textureLoad(er_scene, vec2<i32>(in.position.xy), 0).x > 0.0) {
            discard;
        }
    }
    if (er.lit.x > 2.5) {
        let up = saturate(ray.z * 2.5);
        let a = atan2(ray.y, ray.x) * 9.0;
        let branch = 1.0 - smoothstep(0.0, 0.12, abs(fract(a + sin(ray.z * 30.0) * 0.6) - 0.5));
        out.depth = 0.0;
        out.color = vec4<f32>(vec3<f32>(1.0, 0.8, 0.3) * (0.35 + branch) * up * er.out_scale.x, 1.0);
        return out;
    }
    if (er.lit.x < 1.5) {
        if (ray.z > -1.0e-5) {
            discard;
        }
        let t = (er.camera.z - er.lit.z) / -ray.z;
        hit = er.camera.xyz + ray * t;
        if (t <= 0.0 || abs(hit.x - er.lit.y) > er.lit.w || abs(hit.y) > er.lit.w) {
            discard;
        }
        let d = textureLoad(er_scene, vec2<i32>(in.position.xy), 0).x;
        if (d <= 0.0) {
            discard;
        }
        let view_z = 1.0 / (d * er.depth.x + er.depth.y);
        let p = er.camera.xyz + ray * (view_z * 100.0 / max(dot(ray, er.forward.xyz), 1.0e-4));
        if (abs(p.z) > 3.0 || abs(p.x - er.lit.y) > er.lit.w || abs(p.y) > er.lit.w) {
            discard;
        }
        let r = length(p.xy - vec2<f32>(er.lit.y, 0.0));
        let ring = 1.0 - smoothstep(0.03, 0.09, abs(fract(r / 45.0) - 0.5));
        let spoke = 1.0 - smoothstep(0.02, 0.06, abs(fract(atan2(p.y, p.x - er.lit.y) * 1.2732) - 0.5));
        glow = max(ring, spoke * 0.6) + 0.15;
    } else {
        if (abs(ray.x) < 1.0e-5) {
            discard;
        }
        let t = (er.lit.y - er.camera.x) / ray.x;
        hit = er.camera.xyz + ray * t;
        if (t <= 0.0 || abs(hit.y) > er.lit.z || hit.z < 0.0 || hit.z > er.lit.w) {
            discard;
        }
        let edge = saturate((er.lit.z - abs(hit.y)) / 15.0) * saturate(min(hit.z, er.lit.w - hit.z) / 15.0);
        glow = edge * (0.6 + 0.4 * sin(hit.z * 0.08));
    }
    let clip = er.clip_from_game * vec4<f32>(hit, 1.0);
    out.depth = clamp(clip.z / clip.w, 0.0, 1.0);
    out.color = vec4<f32>(tint * glow * er.out_scale.x, 1.0);
    return out;
}

// (IMPL_ER78, the offline harness only: hylight.ps.dxil) THE GAME'S LIGHT PASSES STAND-IN for the hybrid: the diffuse light a
// white surface gets from the G-buffer's normal (t13 = G-buffer 0, world normal * 0.5 + 0.5 in the host's axes): a warm sun
// toward er.lit.xyz at er.out_scale.x and a blue sky light er.out_scale.y everywhere; the specular target stays 0.
struct HyLightOut {
    @location(0) diffuse: vec4<f32>,
    @location(1) specular: vec4<f32>,
}

@fragment
fn fs_hylight(in: ScreenOut) -> HyLightOut {
    let g = textureLoad(er_scene, vec2<i32>(in.position.xy), 0);
    let n = normalize(g.xyz * 2.0 - vec3<f32>(1.0));
    let sun = max(dot(n, normalize(er.lit.xyz)), 0.0) * er.out_scale.x * vec3<f32>(1.0, 0.86, 0.66);
    var o: HyLightOut;
    o.diffuse = vec4<f32>(sun + er.out_scale.y * vec3<f32>(0.55, 0.65, 1.0), 1.0);
    o.specular = vec4<f32>(0.0);
    return o;
}

#ifdef HY_SCENE
// (HY, the offline harness only: hylight2.ps.dxil) THE GAME'S LIGHT PASSES, A SCENE OF ELDEN RING'S KIND, for the hybrid's
// proof. Per pixel of the G-buffer (t13 = G-buffer 0: world normal * 0.5 + 0.5, HOST axes) at the scene depth (t12, with his
// early depth in it, as the game's): the sun (its light per N.L, shadowed by world casters: capsules near him, and by HIS
// capsules only with `hs_sun_col.w` = his real shadow in the cascades), the sky / environment (everywhere + up), local point
// lights (unshadowed: the game's local shadow maps do not hold him), an occlusion from his capsules (the game's SSAO sees his
// depth) applied as the game's light shaders do (x min(mask.r, mask.g)). Out: 0 the diffuse light of a white surface, 1 the
// specular (0), 2 the screen-space mask (r, g = occlusion, a = the sun's visibility), 3 the composed scene: the floor's checker
// x its light, the sky, his pixels as the white surface the game composes there (his colour covers them).
struct HyLight2Out {
    @location(0) diffuse: vec4<f32>,
    @location(1) specular: vec4<f32>,
    @location(2) mask: vec4<f32>,
    @location(3) scene: vec4<f32>,
}

fn hs_capsule_shadow(p: vec3<f32>, to_light: vec3<f32>, a: vec3<f32>, b: vec3<f32>, radius: f32, reach: f32) -> f32 {
    let ab = b - a;
    let c = max(dot(ab, ab), 0.0001);
    let u0 = saturate(dot(p - a, ab) / c);
    if (length(p - (a + ab * u0)) < radius * 1.2) {
        return 0.0;
    }
    let w0 = p - a;
    let bb = dot(to_light, ab);
    let denom = c - bb * bb;
    var u = 0.5;
    if (denom > 0.0001) {
        u = saturate((dot(ab, w0) - bb * dot(to_light, w0)) / denom);
    }
    let s = max(dot(to_light, a + ab * u - p), 0.0);
    u = saturate(dot(ab, p + to_light * s - a) / c);
    let d = length(p + to_light * s - (a + ab * u));
    // (a cascade's filtered edge: a soft rim growing with the distance)
    let soft = 3.0 + s * 0.02;
    return (1.0 - smoothstep(radius - soft, radius + soft, d)) * step(1.0, s) * saturate(1.0 - s / reach);
}

fn hs_capsule_ao(p: vec3<f32>, n: vec3<f32>, a: vec3<f32>, b: vec3<f32>, radius: f32) -> f32 {
    let ab = b - a;
    let u = saturate(dot(p - a, ab) / max(dot(ab, ab), 0.0001));
    let q = a + ab * u;
    let d = length(p - q);
    if (d < radius * 1.05) {
        return 0.0;
    }
    let toward = saturate(dot(n, (q - p) / d));
    let x = radius / d;
    return saturate(x * x * (0.35 + 0.65 * toward));
}

@fragment
fn fs_hylight2(in: ScreenOut) -> HyLight2Out {
    var o: HyLight2Out;
    let t = vec2<i32>(in.position.xy);
    let d = textureLoad(er_depth, t, 0).x;
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let tan_x = 1.0 / er.depth.w;
    let tan_y = tan_x * er.size.y / er.size.x;
    let ray = normalize(er.forward.xyz + er.right.xyz * ndc.x * tan_x + er.up.xyz * ndc.y * tan_y);
    o.specular = vec4<f32>(0.0);
    if (d <= 0.0) {
        // the sky: the fog's colour toward the horizon, the sky light above
        let w = er.hs_amb_up.w;
        let sky = mix(vec3<f32>(0.62, 0.68, 0.75) * w, vec3<f32>(0.33, 0.48, 0.80) * w, saturate(ray.z * 2.0));
        o.diffuse = vec4<f32>(0.0);
        o.mask = vec4<f32>(1.0);
        o.scene = vec4<f32>(sky, 1.0);
        return o;
    }
    let view_z = 1.0 / (d * er.depth.x + er.depth.y);
    let p = er.camera.xyz + ray * (view_z * 100.0 / max(dot(ray, er.forward.xyz), 1.0e-4));
    let gh = textureLoad(er_scene, t, 0).xyz * 2.0 - vec3<f32>(1.0);
    // (host -> game axes: (x, y, z) -> (z, x, y))
    let n = normalize(vec3<f32>(gh.z, gh.x, gh.y));
    let feet = er.hs_feet.xyz;
    // ---- the sun, shadowed
    var vis = 1.0;
    let to_sun = normalize(er.hs_sun.xyz);
    if (er.hs_sun.w > 0.0) {
        for (var k = 0; k < 4; k++) {
            let a = er.hs_occ[2 * k];
            let b = er.hs_occ[2 * k + 1];
            if (b.w > 0.5) {
                vis = min(vis, 1.0 - hs_capsule_shadow(p, to_sun, feet + a.xyz, feet + b.xyz, a.w, 6000.0));
            }
        }
        if (fract(er.hs_sun_col.w * 0.5) > 0.25) {
            for (var k = 0; k < 32; k++) {
                let a = er.hs_caps[2 * k];
                let b = er.hs_caps[2 * k + 1];
                if (b.w > 0.5) {
                    vis = min(vis, 1.0 - hs_capsule_shadow(p, to_sun, a.xyz, b.xyz, a.w, 1500.0));
                }
            }
        }
    }
    // ---- the occlusion: his capsules (the game's SSAO sees his depth) and the ground right under him
    var occ = 0.0;
    for (var k = 0; k < 32; k++) {
        let a = er.hs_caps[2 * k];
        let b = er.hs_caps[2 * k + 1];
        if (b.w > 0.5) {
            occ = max(occ, hs_capsule_ao(p, n, a.xyz, b.xyz, a.w));
        }
    }
    var ao = clamp(1.0 - occ * er.hs_amb.w, 0.05, 1.0);
    // (RSF2 round 5, `--hy3-print`: hs_sun_col.w >= 2) THE MECHANISM OF fix6/ref: a screen-space mask made without him. On
    // anything that is not the floor (his pixels, in this scene) the mask is the one of a WALL BEHIND HIM with a barred window
    // in it: in the shade (sun hidden, occlusion 0.45) except in the window's opening (in the sun, open) and its bars
    let printed = er.hs_sun_col.w >= 1.5 && er.hs_sun_col.w < 3.5 && !(abs(p.z) < 2.0 && n.z > 0.9);
    if (printed) {
        let s = in.position.xy * er.size.zw;
        let in_window = s.x > 0.42 && s.x < 0.58 && s.y > 0.18 && s.y < 0.42;
        let bar = (i32(in.position.x) % 24) < 7;
        if (in_window && !bar) {
            vis = 1.0;
            ao = 1.0;
        } else {
            vis = 0.0;
            ao = 0.45;
        }
    }
    var e = er.hs_sun_col.xyz * max(dot(n, to_sun), 0.0) * vis * er.hs_sun.w;
    e += er.hs_amb.xyz + er.hs_amb_up.xyz * max(n.z, 0.0);
    for (var k = 0; k < 4; k++) {
        if (f32(k) >= er.hs_feet.w) {
            break;
        }
        let l = feet + er.hs_lpos[k].xyz - p;
        let r = er.hs_lpos[k].w;
        let d2 = dot(l, l);
        if (d2 < r * r) {
            let f = saturate(1.0 - d2 / (r * r));
            e += er.hs_lcol[k].xyz * max(dot(n, l * inverseSqrt(max(d2, 1.0))), 0.0) * f * f;
        }
    }
    e *= ao;
    // (RSF2 round 10, `--hy3-print 2`: hs_sun_col.w >= 3.5) THE IN-GAME EVIDENCE (fix12/ref A_hybrid_debug2): the LIGHT itself
    // cut to black in the shapes of what lies behind him (here: the wall with its barred window), while the mask stays clean
    // (RSF round 19, `--hy3-print 3`, the user's ghost_donald_1..4: a close camera, Donald filling the screen, the candles, the
    // pillars and the arches behind him printed across his face) LARGE printed shapes, wider than the capture's blur: bright
    // vertical columns (x 2.5, a candle's light) and dark ones (x 0.08, a pillar's shade), 70 px wide every 180 px, on every KH pixel
    if (er.hs_sun_col.w >= 5.5 && !(abs(p.z) < 2.0 && n.z > 0.9)) {
        let col = i32(in.position.x) % 180;
        if (col < 70) {
            e *= 2.5;
        } else if (col >= 90 && col < 160) {
            e *= 0.08;
        }
    } else if (er.hs_sun_col.w >= 3.5 && !(abs(p.z) < 2.0 && n.z > 0.9)) {
        let s2 = in.position.xy * er.size.zw;
        // (black shapes over a minority of his pixels, as in the user's frame: crenellations, the window's bars, an arch)
        let bar = s2.x > 0.40 && s2.x < 0.60 && s2.y > 0.18 && s2.y < 0.42 && (i32(in.position.x) % 24) < 7;
        let crenel = (i32(in.position.x) % 40) < 20 && s2.y > 0.55 && s2.y < 0.60;
        let arch = length((s2 - vec2<f32>(0.47, 0.80)) * vec2<f32>(1.0, 0.6)) < 0.035;
        if (bar || crenel || arch) {
            e *= 0.03;
        }
    }
    o.diffuse = vec4<f32>(e, 1.0);
    o.mask = vec4<f32>(ao, ao, 1.0, vis);
    // the floor's checker (as fs_floor), anything else white (his pixels: the game composes his white G-buffer surface)
    var albedo = vec3<f32>(1.0);
    if (abs(p.z) < 2.0 && n.z > 0.9) {
        let tile = floor(p.x * 0.01) + floor(p.y * 0.01);
        // (Elden Ring's ground: surface colour 0.12 .. 0.17, measured at his feet in the user's logs)
        albedo = vec3<f32>(select(0.12, 0.16, (tile - 2.0 * floor(tile * 0.5)) < 0.5));
    }
    let fog = 1.0 - exp(-view_z * er.hs_fog.w);
    o.scene = vec4<f32>(mix(albedo * e, er.hs_fog.xyz, fog * 0.0), 1.0);
    return o;
}

// (HY, the offline harness only: hyfog.ps.dxil) THE GAME'S FOG over the composed picture with him in it (its volumetric fog's
// scene composite reads the depth: he fades into the haze with his distance as the world does): alpha-blended over the scene.
@fragment
fn fs_hyfog(in: ScreenOut) -> @location(0) vec4<f32> {
    let d = textureLoad(er_depth, vec2<i32>(in.position.xy), 0).x;
    if (d <= 0.0 || er.hs_fog.w <= 0.0) {
        discard;
    }
    let view_z = 1.0 / (d * er.depth.x + er.depth.y);
    return vec4<f32>(er.hs_fog.xyz, 1.0 - exp(-view_z * er.hs_fog.w));
}
#endif

// (IMPL_ER80, the offline harness only: glowsrc.ps.dxil) THE GAME'S DEFERRED COMPOSITE'S SECOND TARGET stand-in, the GLOW
// SOURCE (Elden Ring's GXLightComposite writes SV_Target1 = emissive x its alpha; its forward passes draw [scene | glow]).
// Target 0 (the scene) is left alone by the pipeline (write mask 0); target 1 gets the Erdtree's emissive: golden branches
// over the sky where the G-buffer's depth (t13: a copy made BEFORE his early depth, as the game's G-buffer behind him)
// shows nothing drawn. Sora is not in it, as he is not in the game's G-buffer. er.out_scale.x: brightness.
struct GlowSrcOut {
    @location(0) scene: vec4<f32>,
    @location(1) glow: vec4<f32>,
}

@fragment
fn fs_glowsrc(in: ScreenOut) -> GlowSrcOut {
    var o: GlowSrcOut;
    o.scene = vec4<f32>(0.0);
    o.glow = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if (textureLoad(er_scene, vec2<i32>(in.position.xy), 0).x > 0.0) {
        return o;
    }
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let tan_x = 1.0 / er.depth.w;
    let tan_y = tan_x * er.size.y / er.size.x;
    let ray = normalize(er.forward.xyz + er.right.xyz * ndc.x * tan_x + er.up.xyz * ndc.y * tan_y);
    let up = saturate(ray.z * 2.5 + 0.35);
    let a = atan2(ray.y, ray.x) * 9.0;
    let branch = 1.0 - smoothstep(0.0, 0.12, abs(fract(a + sin(ray.z * 30.0) * 0.6) - 0.5));
    let twig = 1.0 - smoothstep(0.0, 0.08, abs(fract(a * 3.7 + ray.z * 40.0) - 0.5));
    o.glow = vec4<f32>(vec3<f32>(1.0, 0.78, 0.3) * (0.3 + branch + 0.5 * twig) * up * er.out_scale.x, 1.0);
    return o;
}

// (IMPL_ER80, the offline harness only: glowcomp.ps.dxil) THE GAME'S GLOW CHAIN stand-in (EmissiveToGlowPS: rgb x a; the
// blur pyramid; GlowCompositeSinglePS): the glow source (t13) blurred (its centre and two rings of 12 taps at 4 and 10
// pixels) and ADDED over the scene (the pipeline blends ONE + ONE), times er.out_scale.x.
@fragment
fn fs_glowcomp(in: ScreenOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let last = vec2<i32>(textureDimensions(er_scene)) - vec2<i32>(1);
    let c = textureLoad(er_scene, p, 0);
    var sum = c.xyz * c.w * 0.2;
    for (var k = 0; k < 12; k++) {
        let a = f32(k) * 0.5235988;
        let d = vec2<f32>(cos(a), sin(a));
        let t1 = textureLoad(er_scene, clamp(p + vec2<i32>(d * 4.0), vec2<i32>(0), last), 0);
        let t2 = textureLoad(er_scene, clamp(p + vec2<i32>(d * 10.0), vec2<i32>(0), last), 0);
        sum += (t1.xyz * t1.w + t2.xyz * t2.w) * (0.4 / 12.0);
    }
    return vec4<f32>(sum * er.out_scale.x, 1.0);
}

// (IMPL_ER80b, the offline harness only: shaftsrc.ps.dxil) THE GAME'S LIGHT-SHAFT SOURCE stand-in (LightShaft_DetectSky:
// sky = depth <= threshold; LightShaft_SkyMaskBlur: a radial blur toward the light): t13 = the scene depth WITH him; the sky
// along the line from the pixel toward the light (er.lit.xy, uv) is summed, fading with the distance travelled, with an
// angular streak pattern; er.lit.z: strength. The game's own LightShaft_CompositePS then adds it over the picture.
@fragment
fn fs_shaftsrc(in: ScreenOut) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(er_scene));
    let light = er.lit.xy;
    var sum = 0.0;
    for (var k = 0; k < 48; k++) {
        let t = f32(k) / 48.0;
        let uv = mix(in.uv, light, t * 0.92);
        let p = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
        let sky = select(0.0, 1.0, textureLoad(er_scene, p, 0).x <= 0.0);
        sum += sky * (1.0 - t);
    }
    let a = atan2(in.uv.y - light.y, (in.uv.x - light.x) * er.size.x / er.size.y);
    let streak = 0.5 + 0.5 * sin(a * 23.0) * sin(a * 7.0 + 1.3);
    let s = sum / 24.0 * streak * er.lit.z;
    return vec4<f32>(vec3<f32>(1.0, 0.85, 0.55) * s, 1.0);
}

// (IMPL_ER80b, the offline harness only: upadd.ps.dxil) a reduced-size target (t13) added over the scene at full size (nearest
// texel of er.lit.x times the pixel position): a composite the game draws at a reduced resolution, then up.
@fragment
fn fs_upadd(in: ScreenOut) -> @location(0) vec4<f32> {
    let last = vec2<i32>(textureDimensions(er_scene)) - vec2<i32>(1);
    let p = clamp(vec2<i32>(in.position.xy * er.lit.x), vec2<i32>(0), last);
    return vec4<f32>(textureLoad(er_scene, p, 0).xyz, 1.0);
}

// (IMPL_ER80, the offline harness only: taa.ps.dxil) THE GAME'S TEMPORAL ANTI-ALIASING stand-in (TempAA): the history (t12)
// clamped to the 3x3 neighbourhood of the current picture (t13), blended with it at er.out_scale.x (1 = no history).
@fragment
fn fs_taa(in: ScreenOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let last = vec2<i32>(textureDimensions(er_scene)) - vec2<i32>(1);
    let cur = textureLoad(er_scene, p, 0).xyz;
    var lo = cur;
    var hi = cur;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let n = textureLoad(er_scene, clamp(p + vec2<i32>(x, y), vec2<i32>(0), last), 0).xyz;
            lo = min(lo, n);
            hi = max(hi, n);
        }
    }
    let history = clamp(textureLoad(er_depth, p, 0).xyz, lo, hi);
    return vec4<f32>(mix(history, cur, er.out_scale.x), 1.0);
}
// ================================================================================================================
// (VFXR) THE EFFECT LAYER'S POST PASSES (kh-er-render src/vfxpost.rs): KH3's glow on our effects and the screen-wide
// tricks of its effects (kh_vfx::screen). Full-screen triangles (vs_screen) into our own half / quarter ... size targets
// or into the scene target. Inputs are the material table's textures kh_t0.. (linear clamp samplers); kh.m rows:
// m[0] = (input width, height, 1 / width, 1 / height), m[1] = (output width, height, 1 / width, 1 / height),
// m[2..] the pass's own values (vfxpost.rs writes them, the names are there).

fn fxp_luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.3, 0.59, 0.11));
}

// The 2x2 average of t0 around uv (t0 twice the output's size).
fn fxp_box(uv: vec2<f32>) -> vec3<f32> {
    let h = kh.m[0].zw * 0.5;
    let a = textureSampleLevel(kh_t0, kh_s0, uv + vec2<f32>(-h.x, -h.y), 0.0).xyz;
    let b = textureSampleLevel(kh_t0, kh_s0, uv + vec2<f32>(h.x, -h.y), 0.0).xyz;
    let c = textureSampleLevel(kh_t0, kh_s0, uv + vec2<f32>(-h.x, h.y), 0.0).xyz;
    let d = textureSampleLevel(kh_t0, kh_s0, uv + vec2<f32>(h.x, h.y), 0.0).xyz;
    return (a + b + c + d) * 0.25;
}

// THE BLOOM SOURCE: what the effects added to the scene (t0 = the scene after them, t1 = before them; full size, the
// output half), times UE4's bloom set-up amount saturate((L - threshold) / 2) with L in KH3's exposed units.
// m[2] = (the target's value of KH3's exposed white, UE4 BloomThreshold, -, -)
@fragment
fn fs_fxp_bsrc(in: ScreenOut) -> @location(0) vec4<f32> {
    let h = kh.m[0].zw * 0.5;
    var sum = vec3<f32>(0.0);
    for (var k = 0; k < 4; k++) {
        let o = vec2<f32>(select(-h.x, h.x, (k & 1) == 1), select(-h.y, h.y, k >= 2));
        let after = textureSampleLevel(kh_t0, kh_s0, in.uv + o, 0.0).xyz;
        let before = textureSampleLevel(kh_t1, kh_s1, in.uv + o, 0.0).xyz;
        sum += max(after - before, vec3<f32>(0.0));
    }
    let added = min(sum * 0.25, vec3<f32>(65000.0));
    let l = fxp_luma(added) / max(kh.m[2].x, 1.0e-9);
    let amount = saturate((l - kh.m[2].y) * 0.5);
    return vec4<f32>(added * amount, 1.0);
}

// A 2x down-sample (the bloom chain, the radial blur's and the gauss's set-up). m[2].x: the bloom radial blur's
// threshold in the target's units (subtracted; 0 otherwise), as FPostProcessSqexBloomRadialBlurSetupPS.
@fragment
fn fs_fxp_down(in: ScreenOut) -> @location(0) vec4<f32> {
    return vec4<f32>(max(fxp_box(in.uv) - vec3<f32>(kh.m[2].x), vec3<f32>(0.0)), 1.0);
}

// One direction of a gaussian: m[2] = (step x, step y (uv per tap), sigma (taps), taps to each side).
@fragment
fn fs_fxp_blur(in: ScreenOut) -> @location(0) vec4<f32> {
    let n = i32(kh.m[2].w);
    let s2 = 2.0 * kh.m[2].z * kh.m[2].z;
    var sum = vec3<f32>(0.0);
    var total = 0.0;
    for (var k = -64; k <= 64; k++) {
        if (k < -n || k > n) {
            continue;
        }
        let w = exp(-f32(k * k) / max(s2, 1.0e-6));
        sum += textureSampleLevel(kh_t0, kh_s0, in.uv + kh.m[2].xy * f32(k), 0.0).xyz * w;
        total += w;
    }
    return vec4<f32>(sum / max(total, 1.0e-6), 1.0);
}

// THE BLOOM COMPOSITE: the six blurred levels (t0..t5) times UE4's tints x BloomIntensity, ADDED to the scene.
// m[2] = tints 0..3, m[3] = tints 4, 5, -, -.
@fragment
fn fs_fxp_bloom(in: ScreenOut) -> @location(0) vec4<f32> {
    var c = textureSampleLevel(kh_t0, kh_s0, in.uv, 0.0).xyz * kh.m[2].x;
    c += textureSampleLevel(kh_t1, kh_s1, in.uv, 0.0).xyz * kh.m[2].y;
    c += textureSampleLevel(kh_t2, kh_s2, in.uv, 0.0).xyz * kh.m[2].z;
    c += textureSampleLevel(kh_t3, kh_s3, in.uv, 0.0).xyz * kh.m[2].w;
    c += textureSampleLevel(kh_t4, kh_s4, in.uv, 0.0).xyz * kh.m[3].x;
    c += textureSampleLevel(kh_t5, kh_s5, in.uv, 0.0).xyz * kh.m[3].y;
    return vec4<f32>(min(c, vec3<f32>(65000.0)), 0.0);
}

// TPostProcessSqexRadialBlurPS<0|1>: ten taps from uv toward the centre (RADIAL) or along a direction (LINEAR).
// m[2] = (centre u, v, power, 1 = LINEAR), m[3] = (direction x, y, -, -).
@fragment
fn fs_fxp_radial(in: ScreenOut) -> @location(0) vec4<f32> {
    var step = (kh.m[2].xy - in.uv) * kh.m[2].z;
    if (kh.m[2].w > 0.5) {
        step = kh.m[3].xy * kh.m[2].z;
    }
    var sum = textureSampleLevel(kh_t0, kh_s0, in.uv, 0.0).xyz * 0.19;
    sum += textureSampleLevel(kh_t0, kh_s0, in.uv + step, 0.0).xyz * 0.17;
    sum += textureSampleLevel(kh_t0, kh_s0, in.uv + step * 2.0, 0.0).xyz * 0.15;
    sum += textureSampleLevel(kh_t0, kh_s0, in.uv + step * 3.0, 0.0).xyz * 0.13;
    sum += textureSampleLevel(kh_t0, kh_s0, in.uv + step * 4.0, 0.0).xyz * 0.11;
    sum += textureSampleLevel(kh_t0, kh_s0, in.uv + step * 5.0, 0.0).xyz * 0.09;
    sum += textureSampleLevel(kh_t0, kh_s0, in.uv + step * 6.0, 0.0).xyz * 0.07;
    sum += textureSampleLevel(kh_t0, kh_s0, in.uv + step * 7.0, 0.0).xyz * 0.05;
    sum += textureSampleLevel(kh_t0, kh_s0, in.uv + step * 8.0, 0.0).xyz * 0.03;
    sum += textureSampleLevel(kh_t0, kh_s0, in.uv + step * 9.0, 0.0).xyz * 0.01;
    return vec4<f32>(min(sum, vec3<f32>(65535.0)), 1.0);
}

// The ring weight of FPostProcessSqexRadialBlurCompositePS: m[4] = (hole, inside fade, range, outside fade), m[2].xy the
// centre; the vertical distance is scaled by height / width (m[1]).
fn fxp_ring(uv: vec2<f32>) -> f32 {
    let r = kh.m[4];
    let d = length(vec2<f32>(kh.m[2].x - uv.x, (kh.m[2].y - uv.y) * kh.m[1].y * kh.m[1].z));
    let a = saturate((d - r.x) / (r.y + 1.0e-5));
    let b = saturate((d - r.z) / (r.w + 1.0e-5));
    return max(a - b, 0.0);
}

// THE RADIAL / GAUSS COMPOSITE (t0 = the blurred half-size picture): m[3] = (alpha, 1 = ADDED (bloom radial blur), -, -).
// Mixed: alpha-blended (rgb, weight); added: rgb x weight with ONE + ONE.
@fragment
fn fs_fxp_rcomp(in: ScreenOut) -> @location(0) vec4<f32> {
    let w = fxp_ring(in.uv) * kh.m[3].x;
    let c = textureSampleLevel(kh_t0, kh_s0, in.uv, 0.0).xyz;
    if (kh.m[3].y > 0.5) {
        return vec4<f32>(c * max(w, 0.0), 0.0);
    }
    return vec4<f32>(c, saturate(w));
}

// TPostProcessSqexScreenBlurPS<0|2> (t0 = the scene, a copy): the copy displaced along the turned radial direction by the
// ring weight x (offset x, y), mixed in by weight x intensity; mode 2 mixes toward white (m[5].x: white in target units).
// m[3] = (intensity, angle (radians), offset x, offset y), m[5] = (white, mode, -, -).
@fragment
fn fs_fxp_sblur(in: ScreenOut) -> @location(0) vec4<f32> {
    let w = fxp_ring(in.uv);
    let aspect = kh.m[1].y * kh.m[1].z;
    let d = vec2<f32>(kh.m[2].x - in.uv.x, (kh.m[2].y - in.uv.y) * aspect);
    let s = sin(kh.m[3].y);
    let c = cos(kh.m[3].y);
    let turned = vec2<f32>(d.x * c - d.y * s, d.y * c + d.x * s);
    let at = in.uv + turned * w * kh.m[3].zw;
    var col = textureSampleLevel(kh_t0, kh_s0, at, 0.0).xyz;
    if (kh.m[5].y > 1.5) {
        col = vec3<f32>(kh.m[5].x);
    }
    return vec4<f32>(col, saturate(w * kh.m[3].x));
}

// SQEX_Filter: m[2] = (colour rgb, alpha), m[3] = (mode 0 ADD / 1 ALPHA / 2 GRAY, white in target units, -, -).
// ADD: colour x alpha x white ADDED; ALPHA: the colour x white alpha-blended; GRAY (t0 = the scene, a copy): the effect
// filter shader's negative (colour.x) and grey (colour.y) amounts on the picture in units of white.
@fragment
fn fs_fxp_filter(in: ScreenOut) -> @location(0) vec4<f32> {
    let mode = kh.m[3].x;
    let white = kh.m[3].y;
    if (mode < 0.5) {
        return vec4<f32>(kh.m[2].xyz * kh.m[2].w * white, 0.0);
    }
    if (mode < 1.5) {
        return vec4<f32>(kh.m[2].xyz * white, saturate(kh.m[2].w));
    }
    // (AUDIT-B W-VFX) mode 4: KH3's GRAY, mode 5: its NPR (TPostProcessSqexEffectFilterPS; kh-vfx screen.rs): m[2] = the summed
    // (FilterColor, FilterAlpha) = cb0[22] (x spares object class 5 [(AUDIT-B W-VFX2) by vfxpost.rs's stencil test on the
    // player mask, not in this shader], y the edge colour k, z the EDGE
    // amount, w the GREY amount) / cb0[21] (w the NEGATIVE amount); m[3].w = 1: the depth (t12) is readable. Modes 2 / 3 stay
    // the earlier reading (the deployed build sends 2).
    // (AUDIT-B W-VFX2) mode 6: GRAY's EDGE alone (vfxpost.rs draws it after mode 4 with m[3].w = 0, with the pipeline's stencil
    // test keeping KH3's object class 0 = no character; the DSV is read-only, the depth t12 readable): (k, 1.01 k, k) x white
    // with alpha = the edge amount where the mean |depth step| to the 8 neighbours 2 px away is >= 1e-4, so the blend gives
    // KH3's g + (edge colour - g) x z; a pixel with nothing drawn (depth 0: the sky, KH3's SQEX_MD_Sky class 2) gets none.
    if (mode > 5.5) {
        let last6 = vec2<i32>(textureDimensions(er_depth)) - vec2<i32>(1);
        let p6 = clamp(vec2<i32>(in.uv * vec2<f32>(textureDimensions(er_depth))), vec2<i32>(0), last6);
        let d6 = textureLoad(er_depth, p6, 0).x;
        if (kh.m[2].z <= 0.0 || d6 <= 0.0) {
            return vec4<f32>(0.0);
        }
        var sum6 = 0.0;
        var taps6 = array<vec2<i32>, 8>(vec2<i32>(-2, 0), vec2<i32>(2, 0), vec2<i32>(0, -2), vec2<i32>(0, 2), vec2<i32>(-2, 2), vec2<i32>(2, -2), vec2<i32>(2, 2), vec2<i32>(-2, -2));
        for (var k6 = 0; k6 < 8; k6 = k6 + 1) {
            sum6 = sum6 + abs(textureLoad(er_depth, clamp(p6 + taps6[k6], vec2<i32>(0), last6), 0).x - d6);
        }
        if (min(sum6 * 0.125, 1.0) < 0.0001) {
            return vec4<f32>(0.0);
        }
        return vec4<f32>(vec3<f32>(kh.m[2].y, kh.m[2].y * 1.01, kh.m[2].y) * white, saturate(kh.m[2].z));
    }
    if (mode > 3.5) {
        let c4 = textureSampleLevel(kh_t0, kh_s0, in.uv, 0.0).xyz / max(white, 1.0e-9);
        let w4 = kh.m[2].w;
        if (mode > 4.5) {
            var n4 = c4;
            if (w4 > 0.0) {
                n4 = c4 + (vec3<f32>(1.0) - 2.0 * c4) * w4;
            }
            return vec4<f32>(max(n4, vec3<f32>(0.0)) * white, 1.0);
        }
        if (w4 <= 0.0) {
            return vec4<f32>(max(c4, vec3<f32>(0.0)) * white, 1.0);
        }
        var g4 = c4 + (vec3<f32>((c4.x + c4.y + c4.z) / 3.0) - c4) * w4;
        if (kh.m[2].z > 0.0 && kh.m[3].w > 0.5) {
            let last4 = vec2<i32>(textureDimensions(er_depth)) - vec2<i32>(1);
            let p4 = clamp(vec2<i32>(in.uv * vec2<f32>(textureDimensions(er_depth))), vec2<i32>(0), last4);
            let d4 = textureLoad(er_depth, p4, 0).x;
            var sum4 = 0.0;
            var taps4 = array<vec2<i32>, 8>(vec2<i32>(-2, 0), vec2<i32>(2, 0), vec2<i32>(0, -2), vec2<i32>(0, 2), vec2<i32>(-2, 2), vec2<i32>(2, -2), vec2<i32>(2, 2), vec2<i32>(-2, -2));
            for (var k4 = 0; k4 < 8; k4 = k4 + 1) {
                sum4 = sum4 + abs(textureLoad(er_depth, clamp(p4 + taps4[k4], vec2<i32>(0), last4), 0).x - d4);
            }
            if (min(sum4 * 0.125, 1.0) >= 0.0001) {
                g4 = g4 + (vec3<f32>(kh.m[2].y, kh.m[2].y * 1.01, kh.m[2].y) - g4) * kh.m[2].z;
            }
        }
        return vec4<f32>(max(g4, vec3<f32>(0.0)) * white, 1.0);
    }
    let c = textureSampleLevel(kh_t0, kh_s0, in.uv, 0.0).xyz / max(white, 1.0e-9);
    let neg = saturate(kh.m[2].x * kh.m[2].w);
    let n = c + (vec3<f32>(1.0) - 2.0 * c) * neg;
    let grey = saturate(kh.m[2].y * kh.m[2].w);
    var g = mix(n, vec3<f32>((n.x + n.y + n.z) / 3.0), grey);
    // (VFX-XLATE) the EDGE amount (FilterColor.z x alpha; KH3's shader: the mean |depth step| to the 8 neighbours 2 px away
    // >= 0.0001 marks an edge, which goes toward (k, 1.01 k, k), k = m[3].z [A: 0, black]); m[3].w = 1: the depth (t12) is
    // readable in this pass
    let edge = saturate(kh.m[2].z * kh.m[2].w);
    if (edge > 0.0 && kh.m[3].w > 0.5) {
        let last = vec2<i32>(textureDimensions(er_depth)) - vec2<i32>(1);
        let p = clamp(vec2<i32>(in.uv * vec2<f32>(textureDimensions(er_depth))), vec2<i32>(0), last);
        let c0 = textureLoad(er_depth, p, 0).x;
        var sum = 0.0;
        var taps = array<vec2<i32>, 8>(vec2<i32>(-2, 0), vec2<i32>(2, 0), vec2<i32>(0, -2), vec2<i32>(0, 2), vec2<i32>(-2, 2), vec2<i32>(2, -2), vec2<i32>(2, 2), vec2<i32>(-2, -2));
        for (var k = 0; k < 8; k = k + 1) {
            sum = sum + abs(textureLoad(er_depth, clamp(p + taps[k], vec2<i32>(0), last), 0).x - c0);
        }
        if (min(sum * 0.125, 1.0) >= 0.0001) {
            g = mix(g, vec3<f32>(kh.m[3].z, kh.m[3].z * 1.01, kh.m[3].z), edge);
        }
    }
    return vec4<f32>(max(g, vec3<f32>(0.0)) * white, 1.0);
}

// SQEX_PostprocessFringe (t0 = the scene, a copy): red and blue taken from points pushed out / in from the screen's
// centre by intensity x 0.004 of the distance [A]; m[2].x = intensity.
@fragment
fn fs_fxp_fringe(in: ScreenOut) -> @location(0) vec4<f32> {
    let o = (in.uv - vec2<f32>(0.5)) * kh.m[2].x * 0.004;
    let r = textureSampleLevel(kh_t0, kh_s0, in.uv + o, 0.0).x;
    let g = textureSampleLevel(kh_t0, kh_s0, in.uv, 0.0).y;
    let b = textureSampleLevel(kh_t0, kh_s0, in.uv - o, 0.0).z;
    return vec4<f32>(r, g, b, 1.0);
}

#endif

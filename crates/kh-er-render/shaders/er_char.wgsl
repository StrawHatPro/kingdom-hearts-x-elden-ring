// The Elden Ring host's side of the character materials (crates/kh-er-render): the skinning vertex
// stage and the fragment entry that fills the shared harness's inputs from OUR OWN frame constants
// (the game's camera, a stand-in light rig) instead of Bevy's view and lights.
//
// Built offline by crates/kh-shaderc: this file + crates/kh-look/shaders/char_common.wgsl (at
// KH_COMMON) + PACK/materials/materials.wgsl (at KH_GENERATED), `#ifdef`s resolved per variant,
// naga -> HLSL -> dxc -> PACK/materials/er/<variant>.ps.dxil and skin.vs.dxil.
//
// Spaces: the mesh is in the pack's space (glTF: metres, Y up, right-handed); `sk.game_from_pack`
// takes a skinned pack position to the GAME's world space (KH3: cm, X forward, Y right, Z up,
// left-handed), in which the translated material code works; `er.clip_from_game` takes that to
// Elden Ring's clip space (through the host's KH3 frame and the game's camera).

struct ErFrame {
    clip_from_game: mat4x4<f32>,
    // Camera position (game space, cm); w: the game clock (s).
    camera: vec4<f32>,
    // The camera's right / up / forward axes in game axes (unit).
    // (NF) cam_right.w: KH3's camera fade of the character this block is his (1 - his CameraFade value, 0 = whole: a block
    // that never sets it draws him whole); cam_up.w: the screen door's phase this frame (er_camfade_keep).
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_forward: vec4<f32>,
    // Target width, height, 1 / width, 1 / height (pixels).
    viewport: vec4<f32>,
    // xyz: unit vector to the key light (game space). STAND-IN rig (kh_look::stand_in).
    key_dir: vec4<f32>,
    // rgb: a tint / scale of the key light (its colour x intensity is the material block's light_color).
    key_color: vec4<f32>,
    // x: tan(half horizontal fov), y: tan(half vertical fov), z: near (m), w: far (m).
    proj: vec4<f32>,
    // The scene depth of the game: x, y: unused; z, w: view depth (m) = 1 / (device_z * z + w)
    // (reversed Z; the contact shadow rebuilds positions with it).
    depth: vec4<f32>,
    // xy: scale from our pixel position to the depth texture's texel (dynamic resolution, or a
    // depth target of another size); z: opacity of the masked slots (1; less = a see-through
    // Sora for checking him against the debug skeleton); w: unused.
    depth_uv: vec4<f32>,
    // x: output transform: 0 = scene linear, 1 = filmic curve (ACES fit) + sRGB encoding
    // (display-referred, for a back buffer after the game's tone mapping), 2 = the curve
    // without the encoding (an sRGB or float back buffer);
    // y: 1 = premultiply alpha into the colour; z: 1 = a flat lit grey instead of the material, 2 = pure magenta (debug);
    // w: 1 = a masked material's dither coverage becomes alpha (a target without multisampling).
    out_mode: vec4<f32>,
    // The contact shadow under him: xyz the actor origin = his feet (game space, cm), w its radius (cm).
    blob: vec4<f32>,
    // x: its strength (0 = none), y: the height (cm) over which it fades when he is above a surface;
    // z: his height (cm) and w: the strength of the shadow he throws from the key light.
    blob_fade: vec4<f32>,
    // THE LIGHT PROBES (probe_ps): the game's targets are read around his feet. xy: the pixel of
    // his feet, z: the spread of the taps (pixels), w: 1 = on.
    probe: vec4<f32>,
    // xyz: what the shaded colour is multiplied with on its way into the target (light.rs: the
    // rig is handed to the materials at KH3's light level and the result scaled back to the
    // game's); w: 1 = the vertex stage uses the GAME's own view-projection of this frame (the
    // game-data texture, row 6: its temporal anti-aliasing jitter included).
    tune: vec4<f32>,
    // (for out_mode.z = 1 / 4) What a white diffuse surface shows facing the key light / in its
    // shade, in the target's units.
    lit_key: vec4<f32>,
    lit_ambient: vec4<f32>,
    lit_up: vec4<f32>,
    // Game space (cm) -> the host's world space (m): of this frame and of the one drawn before.
    host_from_game: mat4x4<f32>,
    prev_host_from_game: mat4x4<f32>,
    // (IMPL_ER9) HIS BODY FOR THE THROWN SHADOW: 16 capsules along his main bones and the Keyblade, game space (cm):
    // [2k] = one end (xyz) and the radius (w), [2k + 1] = the other end (xyz) and w = 1 when the capsule is there.
    caps: array<vec4<f32>, 32>,
#ifdef KH_EXTRA_LIGHTS
    // (IMPL_ER44) SORA'S OWN POINT LIGHTS (pointlight.rs `sora_block`; only the *_pl variants declare them, so the
    // older pixel shaders of the pack read the block as before): the hero light (placed toward the camera, above him)
    // and the effects' fire lights near him. x: lights (0 = none: his look as without them); then per light
    // game space (cm) + radius (cm), colour (KH3 light units: the shaded colour is scaled by tune.xyz afterwards) +
    // the falloff exponent: pow(1 - d^2 / r^2, exponent), as the light pass.
    xl_head: vec4<f32>,
    xl_pos: array<vec4<f32>, 6>,
    xl_col: array<vec4<f32>, 6>,
#endif
#ifdef KH_BLOOD
    // (IMPL_ER60) THE BLOOD ON HIM (src/blood.rs `block`; only the *_bl variants declare it, and they are always built
    // WITH KH_EXTRA_LIGHTS, so it follows the lights' block). bl_head: x on the body, y on the Keyblade (0..1),
    // z fresh (1 wet red .. 0 dried brown), w the splatter's seed; bl_feet: his feet (game space, cm) + his height;
    // bl_hand_l / bl_hand_r: the hands + their reach (cm, 0 = none); bl_blade_a / bl_blade_b: grip and tip of the
    // Keyblade + its reach (cm, 0 = not out).
    bl_head: vec4<f32>,
    bl_feet: vec4<f32>,
    bl_hand_l: vec4<f32>,
    bl_hand_r: vec4<f32>,
    bl_blade_a: vec4<f32>,
    bl_blade_b: vec4<f32>,
    // the middle of his head + its reach (cm, 0 = none)
    bl_face: vec4<f32>,
#endif
#ifdef KH_HYBRID
    // (IMPL_ER78) THE HYBRID (src/hybrid.rs `block`; only the *_hy / *_gb variants declare it, always built with the two
    // blocks above, so it follows them). All zero = today's light (hy[0].x = 0). [0] on, share of the game's light, share
    // of the rig's light, KH3's sun specular; [1] band step, highlight step, half width, staircase mix; [2] shade level,
    // highlight add, AO strength, -; [3] the white level (KH units), scene -> KH units, G-buffer translucency, light set /
    // 255; [4] fog colour (KH units), fog per metre; [5] masked coverage floor, -, G-buffer gloss, -.
#ifdef KH_HY3
    // (HY, the *_hy3 / *_gb3 sets: src/hybrid.rs `block`, 12 rows) [0]..[4] as above (the scale [3].y unused: each character's
    // own 1 / tune.x); [5] masked coverage floor, share of the rig's ambient taken out of the game's light, G-buffer gloss,
    // debug view; [6] the sun's direction (game space) + the strength of his own capsule shadow; [7] the sun's light per unit
    // N.L on a white surface (SCENE units) + the lift; [8] normal match (cos), neighbour search (px), the floor share, -;
    // [9]..[11] reserved (0).
#ifdef ER_HYCAP
    // (RSF2 round 12) the capture shader alone reads 16 rows: [9] the capture's box, [10] the cascades (count, filter width,
    // bias, normal offset), [11]..[15] their viewports in the atlas (src/hybrid.rs `sun_rows`). The block is the frame block's
    // last part: the colour shaders' 12 rows are its first 12.
    hy: array<vec4<f32>, 23>,
#else
    hy: array<vec4<f32>, 12>,
#endif
#else
    hy: array<vec4<f32>, 6>,
#endif
#endif
}

struct ErSkin {
    game_from_pack: mat4x4<f32>,
    // The joint matrices of the skin (pack space): joint world x inverse bind.
#ifdef KH_JOINTS_EXTRA
    joints: array<mat4x4<f32>, 320>,
#else
    joints: array<mat4x4<f32>, 200>,
#endif
}

@group(0) @binding(0) var<uniform> er: ErFrame;
@group(0) @binding(2) var<uniform> sk: ErSkin;

// (NF) KH3'S CAMERA FADE (TresCameraManager's shut-out fade; kh-camera camfade.rs, sheets/port/sora-camera-fade.json): the
// masked materials' own DitherTemporalAA test (materials.wgsl kh_mask, first test) against his CameraFade value, decided per
// PIXEL from the frame block alone, so that the same pixel of him is gone from EVERY pass that draws him (the early depth,
// the colour, the hybrid's G-buffer, the glow mask, the light-shaft mask's depth, his motion vectors, the hair passes): KH3's
// SHARE of his pixels (its six-phase test frac(d) x 6/7 + value - 5/6 >= 0: 3 of 6 at 0.5) laid out as a still 4 x 4 ordered
// matrix (camfade.screen_door: a 1-pixel checkerboard at 0.5) shifted by the phase (char_common.wgsl kh_camfade_keep's twin;
// the velocity shader has no KH_COMMON).
fn er_camfade_keep(position: vec2<f32>) -> bool {
    let value = 1.0 - er.cam_right.w;
    if (value >= 1.0) {
        return true;
    }
    var kept = 0u;
    for (var k = 0u; k < 6u; k++) {
        if (f32(k) / 6.0 * 0.857143 + value - 0.8333 >= 0.0) {
            kept += 1u;
        }
    }
    let phase = u32(max(er.cam_up.w, 0.0));
    let p = vec2<u32>(max(floor(position), vec2<f32>(0.0))) + vec2<u32>(phase, 3u * phase);
    let a = vec2<u32>(p.x & 1u, p.y & 1u);
    let b = vec2<u32>((p.x >> 1u) & 1u, (p.y >> 1u) & 1u);
    let cell = 4u * (((a.x ^ a.y) & 1u) * 2u + a.y) + ((b.x ^ b.y) & 1u) * 2u + b.y;
    return (f32(cell) + 0.5) / 16.0 < f32(kept) / 6.0;
}

#ifdef ER_FRAGMENT
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
@group(1) @binding(10) var kh_t10: texture_2d<f32>;
@group(1) @binding(11) var kh_t11: texture_2d<f32>;
@group(1) @binding(12) var kh_t12: texture_2d<f32>;
@group(1) @binding(13) var kh_t13: texture_2d<f32>;
// (IMPL_ER59) an outfit's material with more texture slots (bindings 17..20: after the depth copy t16)
#ifdef KH_TEX_EXTRA
@group(1) @binding(17) var kh_t14: texture_2d<f32>;
@group(1) @binding(18) var kh_t15: texture_2d<f32>;
@group(1) @binding(19) var kh_t16: texture_2d<f32>;
@group(1) @binding(20) var kh_t17: texture_2d<f32>;
#endif
// The post-process material's colour table (RGBTable_PP).
@group(1) @binding(14) var kh_tclamp: texture_2d<f32>;
// The reflection environment: a prefiltered cube (mip = roughness, the engine's capture layout).
@group(1) @binding(15) var kh_tenv: texture_cube<f32>;
// Wrap and clamp (trilinear, anisotropic), and the cube's.
@group(2) @binding(0) var kh_swrap: sampler;
@group(2) @binding(1) var kh_sclamp: sampler;
@group(2) @binding(2) var kh_senv: sampler;
#endif

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv0: vec2<f32>,
    @location(4) uv1: vec2<f32>,
    @location(5) uv2: vec2<f32>,
    @location(6) joints: vec4<u32>,
    @location(7) weights: vec4<f32>,
}

#ifdef KH_HAIR91_SAMPLE
// (ER91) the 4x hair pass shaded per sample: every interpolant at its sample (the same layout as below)
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(perspective, sample) world: vec3<f32>,
    @location(1) @interpolate(perspective, sample) normal: vec3<f32>,
    @location(2) @interpolate(perspective, sample) tangent: vec4<f32>,
    @location(3) @interpolate(perspective, sample) uv0: vec2<f32>,
    @location(4) @interpolate(perspective, sample) uv1: vec2<f32>,
    @location(5) @interpolate(perspective, sample) uv2: vec2<f32>,
    @location(6) @interpolate(perspective, sample) view_z: f32,
    @location(7) @interpolate(perspective, sample) cur: vec4<f32>,
    @location(8) @interpolate(perspective, sample) prev: vec4<f32>,
#ifdef KH_PSP_OUT
    @location(9) @interpolate(perspective, sample) psp: vec3<f32>,
#endif
}
#else
struct VsOut {
    @builtin(position) position: vec4<f32>,
    // World position, game space (cm).
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv0: vec2<f32>,
    @location(4) uv1: vec2<f32>,
    @location(5) uv2: vec2<f32>,
    // View depth (m).
    @location(6) view_z: f32,
    // His clip position now and a frame ago (the game's own matrices: its jitter cancels out), for
    // the motion vectors.
    @location(7) cur: vec4<f32>,
    @location(8) prev: vec4<f32>,
#ifdef KH_PSP_OUT
    // (ER party S4) a party pack's shaders (kh-shaderc --party): the pre-skinned local position (game axes, cm) for the
    // owners that read it (KH_PSP: TEXCOORD3.xy / TEXCOORD4.x = v0 * MeshExtension + MeshOrigin, as kh-app's char_material)
    @location(9) psp: vec3<f32>,
#endif
}
#endif

#ifdef ER_BLOB
// THE CONTACT SHADOW (a stand-in for a real shadow: Elden Ring's shadow maps do not know him): a
// screen-space decal. For every pixel near his feet the scene position is rebuilt from the game's
// depth buffer; where it lies under his feet within the radius the picture is darkened
// (multiplied). Drawn before him, into the finished picture.
@group(1) @binding(16) var er_tdepth: texture_2d<f32>;
// The game's screen-space shadow mask (alpha: 1 = lit by its directional light), or white.
@group(1) @binding(15) var er_tshadow: texture_2d<f32>;

// HIS THROWN SHADOW (a stand-in for his silhouette in the game's shadow maps): how much of the
// key light a point `p` loses to his body, taken as capsules along his main bones and the
// Keyblade (`er.caps`): for each, the distance between the ray from `p` to the light and the
// capsule's axis, soft at the rim and softer with the distance the shadow is thrown; the darkest
// capsule counts (overlapping limbs do not add up).
fn er_thrown_one(p: vec3<f32>, to_light: vec3<f32>, a: vec3<f32>, b_end: vec3<f32>, radius: f32) -> f32 {
    let ab = b_end - a;
    let w0 = p - a;
    let b = dot(to_light, ab);
    let c = max(dot(ab, ab), 0.0001);
    let denom = c - b * b;
    var u = 0.5;
    if (denom > 0.0001) {
        u = saturate((dot(ab, w0) - b * dot(to_light, w0)) / denom);
    }
    let t = max(dot(to_light, a + ab * u - p), 0.0);
    u = saturate(dot(ab, p + to_light * t - a) / c);
    let d = length(p + to_light * t - (a + ab * u));
    let soft = radius * 0.35 + t * 0.035;
    let rim = 1.0 - smoothstep(radius - soft * 0.5, radius + soft, d);
    return rim * saturate(1.0 - t / 1500.0) * step(2.0, t);
}

fn er_thrown(p: vec3<f32>, to_light: vec3<f32>) -> f32 {
    var darkest = 0.0;
    for (var k = 0; k < 16; k = k + 1) {
        let a = er.caps[2 * k];
        let b = er.caps[2 * k + 1];
        if (b.w > 0.5) {
            darkest = max(darkest, er_thrown_one(p, to_light, a.xyz, b.xyz, a.w));
        }
    }
    return darkest;
}

@vertex
fn blob_vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One triangle over the whole target (the scissor rectangle keeps it to his surroundings).
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(p * 2.0 - vec2<f32>(1.0), 0.0, 1.0);
}

@fragment
fn blob_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let device_z = textureLoad(er_tdepth, vec2<i32>(position.xy), 0).x;
    if (device_z <= 0.0) {
        discard;
    }
    let view_z = 1.0 / (device_z * er.depth.z + er.depth.w);
    let ndc = vec2<f32>(position.x * er.viewport.z * 2.0 - 1.0, 1.0 - position.y * er.viewport.w * 2.0);
    let v = vec3<f32>(ndc.x * er.proj.x, ndc.y * er.proj.y, 1.0) * (view_z * 100.0);
    let p = er.camera.xyz + er.cam_right.xyz * v.x + er.cam_up.xyz * v.y + er.cam_forward.xyz * v.z;
    let d = p - er.blob.xyz;
    let near = saturate(1.0 - length(d.xy) / er.blob.w);
    // How far the surface lies under his feet: fading with his height above it; a surface more
    // than a hand above his feet (a wall, a ledge) gets none.
    let under = -d.z;
    let height = saturate(1.0 - max(under, 0.0) / er.blob_fade.y) * saturate(1.0 + min(under, 0.0) / 25.0);
    let s = near * near * (3.0 - 2.0 * near) * height * er.blob_fade.x;
    var thrown = 0.0;
    if (er.blob_fade.w > 0.0) {
        let lit = textureLoad(er_tshadow, vec2<i32>(position.xy), 0).w;
        thrown = er_thrown(p, normalize(er.key_dir.xyz)) * er.blob_fade.w * lit;
    }
    return vec4<f32>(vec3<f32>((1.0 - s) * (1.0 - thrown)), 1.0);
}
#endif

#ifdef ER_PLIGHT
// (IMPL_ER39) LIGHT FROM SORA'S EFFECTS AND THE HERO LIGHT (src/pointlight.rs): one additive full-screen
// triangle (scissored) into the scene target right after the game's deferred stage, before Sora. Per pixel:
// the position from the game's depth, the normal from the depth's neighbours (the nearer side on each axis, so
// that edges do not smear), the surface colour from the game's G-buffer colour target (or a grey without one);
// each light: colour x wrapped N.L x pow(1 - d^2 / r^2, exponent), occluded by a short march through the depth
// buffer toward the light (what lies in front of the ray on screen by less than a thickness hides it).
struct ErLights {
    // x: lights, y: 1 = the surface colour is the game's, z: the grey used without it, w: occlusion steps
    head: vec4<f32>,
    // x: occlusion thickness (m), y: occlusion bias (m)
    head2: vec4<f32>,
    // game space (cm) + radius (cm); colour (the target's units on a white surface) + exponent; x: wrap
    pos: array<vec4<f32>, 16>,
    col: array<vec4<f32>, 16>,
    opt: array<vec4<f32>, 16>,
}
@group(0) @binding(1) var<uniform> pl: ErLights;
@group(1) @binding(16) var er_tdepth: texture_2d<f32>;
@group(1) @binding(3) var er_talbedo: texture_2d<f32>;

fn pl_view_z(texel: vec2<i32>) -> f32 {
    let t = clamp(texel, vec2<i32>(0), vec2<i32>(er.viewport.xy) - vec2<i32>(1));
    let device_z = max(textureLoad(er_tdepth, t, 0).x, 1.0e-9);
    return 1.0 / (device_z * er.depth.z + er.depth.w);
}

fn pl_world(pixel: vec2<f32>, view_z: f32) -> vec3<f32> {
    let ndc = vec2<f32>(pixel.x * er.viewport.z * 2.0 - 1.0, 1.0 - pixel.y * er.viewport.w * 2.0);
    let v = vec3<f32>(ndc.x * er.proj.x, ndc.y * er.proj.y, 1.0) * (view_z * 100.0);
    return er.camera.xyz + er.cam_right.xyz * v.x + er.cam_up.xyz * v.y + er.cam_forward.xyz * v.z;
}

#ifdef ER_PLIGHT_CAPS
// (IMPL_ER44) HIS BODY'S SHADOW FROM THE KEYBLADE LIGHT (plight2.ps.dxil): Sora is not in the game's depth buffer (he is
// drawn after this pass), so his silhouette is taken from the capsules along his main bones (`er.caps`, the thrown
// shadow's): for each, the closest approach of the segment from the lit point to the light and the capsule's axis; soft
// at the rim, the penumbra growing with the distance from him to the lit point over the distance from him to the light
// (a small source: the blade). The Keyblade's own capsules (w = 2) and a capsule that holds the light are left out; the
// darkest capsule counts. Returns how much of the light his body takes away (0..1).
fn pl_body(p: vec3<f32>, light: vec3<f32>) -> f32 {
    let d1 = light - p;
    let len = length(d1);
    var darkest = 0.0;
    for (var k = 0; k < 16; k++) {
        let a = er.caps[2 * k];
        let b = er.caps[2 * k + 1];
        if (b.w < 0.5 || b.w > 1.5) {
            continue;
        }
        let radius = a.w;
        let d2 = b.xyz - a.xyz;
        // (the light inside or touching this capsule: it cannot shade from it)
        let lu = saturate(dot(light - a.xyz, d2) / max(dot(d2, d2), 1.0e-4));
        if (length(light - (a.xyz + d2 * lu)) < radius + 2.0) {
            continue;
        }
        // closest points of the segments p + d1 s and a + d2 u (s, u in 0..1)
        let r = p - a.xyz;
        let aa = dot(d1, d1);
        let ee = max(dot(d2, d2), 1.0e-4);
        let ff = dot(d2, r);
        let cc = dot(d1, r);
        let bb = dot(d1, d2);
        let denom = aa * ee - bb * bb;
        var sv = 0.0;
        if (denom > 1.0e-4) {
            sv = saturate((bb * ff - cc * ee) / denom);
        }
        var uv = (bb * sv + ff) / ee;
        if (uv < 0.0) {
            uv = 0.0;
            sv = saturate(-cc / max(aa, 1.0e-4));
        } else if (uv > 1.0) {
            uv = 1.0;
            sv = saturate((bb - cc) / max(aa, 1.0e-4));
        }
        let gap = length(p + d1 * sv - (a.xyz + d2 * uv));
        let to_receiver = sv * len;
        let to_light = max((1.0 - sv) * len, 5.0);
        let soft = radius * 0.25 + 6.0 * to_receiver / to_light;
        let rim = 1.0 - smoothstep(radius - soft * 0.5, radius + soft, gap);
        darkest = max(darkest, rim * step(1.0, to_receiver));
    }
    return darkest;
}
#endif

// 1 = nothing in the way from p to the light on screen, 0 = hidden.
fn pl_visible(p: vec3<f32>, light: vec3<f32>, steps: i32) -> f32 {
    var hidden = 0.0;
    for (var k = 1; k <= steps; k++) {
        // (from a little off the surface to just short of the light)
        let t = 0.08 + 0.84 * f32(k) / f32(steps);
        let q = mix(p, light, t);
        let c = er.clip_from_game * vec4<f32>(q, 1.0);
        if (c.w <= 0.01) {
            continue;
        }
        let uv = vec2<f32>(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5);
        if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
            continue;
        }
        let surface = pl_view_z(vec2<i32>(uv * er.viewport.xy));
        let ray = dot(q - er.camera.xyz, er.cam_forward.xyz) * 0.01;
        let gap = ray - surface;
        if (gap > pl.head2.y + 0.01 * ray && gap < pl.head2.x) {
            hidden = hidden + 1.0;
        }
    }
    return 1.0 - saturate(hidden * 0.5);
}

@fragment
fn plight_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = vec2<i32>(position.xy);
    let device_z = textureLoad(er_tdepth, texel, 0).x;
    // (reversed Z: 0 = nothing drawn there, the sky)
    if (device_z <= 0.0) {
        discard;
    }
    let z = 1.0 / (device_z * er.depth.z + er.depth.w);
    let p = pl_world(position.xy, z);
    // The normal from the depth: on each axis the neighbour whose depth is nearer this pixel's.
    let zr = pl_view_z(texel + vec2<i32>(1, 0));
    let zl = pl_view_z(texel - vec2<i32>(1, 0));
    let zd = pl_view_z(texel + vec2<i32>(0, 1));
    let zu = pl_view_z(texel - vec2<i32>(0, 1));
    var dx = pl_world(position.xy + vec2<f32>(1.0, 0.0), zr) - p;
    if (abs(zl - z) < abs(zr - z)) {
        dx = p - pl_world(position.xy - vec2<f32>(1.0, 0.0), zl);
    }
    var dy = pl_world(position.xy + vec2<f32>(0.0, 1.0), zd) - p;
    if (abs(zu - z) < abs(zd - z)) {
        dy = p - pl_world(position.xy - vec2<f32>(0.0, 1.0), zu);
    }
    var n = normalize(cross(dx, dy));
    if (dot(n, er.camera.xyz - p) < 0.0) {
        n = -n;
    }
    var albedo = vec3<f32>(pl.head.z);
    if (pl.head.y > 0.5) {
        albedo = textureLoad(er_talbedo, texel, 0).xyz;
    }
    let steps = i32(pl.head.w);
    var sum = vec3<f32>(0.0);
    for (var k = 0; k < 16; k++) {
        if (f32(k) >= pl.head.x) {
            break;
        }
        let r = pl.pos[k].w;
        let to_light = pl.pos[k].xyz - p;
        let d2 = dot(to_light, to_light);
        if (d2 >= r * r) {
            continue;
        }
        let l = to_light * inverseSqrt(max(d2, 1.0e-4));
        let wrap = pl.opt[k].x;
        let n_l = saturate((dot(n, l) + wrap) / (1.0 + wrap));
        let radial = pow(saturate(1.0 - d2 / (r * r)), pl.col[k].w);
        var lit = pl.col[k].xyz * (n_l * radial);
        if (steps > 0 && max(lit.x, max(lit.y, lit.z)) > 0.0) {
            lit = lit * pl_visible(p, pl.pos[k].xyz, steps);
        }
#ifdef ER_PLIGHT_CAPS
        if (pl.opt[k].y > 0.0 && max(lit.x, max(lit.y, lit.z)) > 0.0) {
            lit = lit * (1.0 - pl.opt[k].y * pl_body(p, pl.pos[k].xyz));
        }
#endif
        sum = sum + lit;
    }
    return vec4<f32>(albedo * sum, 0.0);
}
#endif

#ifdef ER_DUMP
// DISCOVERY TOOL (gfx.rs `copy_tables`): a constant buffer of the GAME (a descriptor of its own
// heap, in a table of the shape its root signature has: registers b4..b13) is written into a row
// of a float texture, one vec4 per pixel. One variant per register.
struct ErDump {
    v: array<vec4<f32>, 256>,
}
#ifdef ER_DUMP_B4
@group(3) @binding(4) var<uniform> gd: ErDump;
#endif
#ifdef ER_DUMP_B5
@group(3) @binding(5) var<uniform> gd: ErDump;
#endif
#ifdef ER_DUMP_B6
@group(3) @binding(6) var<uniform> gd: ErDump;
#endif
#ifdef ER_DUMP_B7
@group(3) @binding(7) var<uniform> gd: ErDump;
#endif
#ifdef ER_DUMP_B8
@group(3) @binding(8) var<uniform> gd: ErDump;
#endif
#ifdef ER_DUMP_B9
@group(3) @binding(9) var<uniform> gd: ErDump;
#endif
#ifdef ER_DUMP_B10
@group(3) @binding(10) var<uniform> gd: ErDump;
#endif
#ifdef ER_DUMP_B11
@group(3) @binding(11) var<uniform> gd: ErDump;
#endif
#ifdef ER_DUMP_B12
@group(3) @binding(12) var<uniform> gd: ErDump;
#endif
#ifdef ER_DUMP_B13
@group(3) @binding(13) var<uniform> gd: ErDump;
#endif

@fragment
fn dump_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return gd.v[u32(position.x)];
}
#endif

#ifdef ER_PROBE
// THE LIGHT PROBES (gfx.rs, the game-light pass; light.rs reads the result): nine taps around the
// pixel of his feet in the game's targets (which hold the scene without him), written to texels
// 0.. of the game-data texture's probe row:
//   0: the light on the ground as the game composed it = scene colour / surface colour (taps
//      whose surface colour is too dark to divide by are left out; w = taps counted),
//   1: the direct light accumulation, 2: the screen-space shadow mask, 3: the scene colour,
//   4: the surface colour; 6: the scene colour along the top of the picture (the environment taps).
@group(1) @binding(0) var er_tscene: texture_2d<f32>;
@group(1) @binding(1) var er_tdirect: texture_2d<f32>;
@group(1) @binding(2) var er_tmask: texture_2d<f32>;
@group(1) @binding(3) var er_talbedo: texture_2d<f32>;
@group(1) @binding(4) var er_tother: texture_2d<f32>;

// (RSF2, probe4.ps.dxil) A probe tap that lands on a KH character drawn into the game's G-buffer (the hybrid writes his albedo
// as exact WHITE, every KH character alike) measures the light on his body, not on the ground around his feet: it is left out.
// Without ER_PROBE_SKIP_KH (probe / probe2 / probe3) every tap counts as before.
fn er_kh_texel(albedo: vec3<f32>) -> bool {
#ifdef ER_PROBE_SKIP_KH
    return min(albedo.x, min(albedo.y, albedo.z)) > 0.996;
#else
    return false;
#endif
}

#ifdef ER_PROBE_RING
// (IMPL_ER64, probe2.ps.dxil: the user: "Sora needs to be lit directionally by items in Elden Ring's world too such as
// torches") THE RING: 24 taps on the ground around his feet (12 directions x 120 / 250 cm, at his feet's height),
// projected into the picture: texel 16 + k = the game's DIRECT light accumulation there over its surface colour (what a
// white surface there gets of the game's lights: torches, lamps, spells, the sun), w = 1 when the tap is valid (in the
// picture, its surface colour readable, and the depth there (er_tother: the scene depth without him) is the ground near
// the tap, not something in front); texel 40 + k = where the depth puts the tap, relative to his feet (cm).
fn ring_tap(k: u32) -> vec3<f32> {
    let a = f32(k % 12u) * 0.5235988;
    let r = select(120.0, 250.0, k >= 12u);
    return er.blob.xyz + vec3<f32>(cos(a), sin(a), 0.0) * r;
}


fn ring_probe(which: u32) -> vec4<f32> {
    let k = select(which - 16u, which - 40u, which >= 40u);
    let p = ring_tap(k);
    let c = er.clip_from_game * vec4<f32>(p, 1.0);
    if (c.w <= 0.3) {
        return vec4<f32>(0.0);
    }
    let uv = vec2<f32>(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5);
    if (any(uv < vec2<f32>(0.01)) || any(uv > vec2<f32>(0.99))) {
        return vec4<f32>(0.0);
    }
    let texel = vec2<i32>(uv * er.viewport.xy);
    let device_z = textureLoad(er_tother, texel, 0).x;
    if (device_z <= 0.0) {
        return vec4<f32>(0.0);
    }
    let view_z = 1.0 / (device_z * er.depth.z + er.depth.w);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let v = vec3<f32>(ndc.x * er.proj.x, ndc.y * er.proj.y, 1.0) * (view_z * 100.0);
    let q = er.camera.xyz + er.cam_right.xyz * v.x + er.cam_up.xyz * v.y + er.cam_forward.xyz * v.z;
    if (length(q - p) > 60.0) {
        return vec4<f32>(0.0);
    }
    if (which >= 40u) {
        return vec4<f32>(q - er.blob.xyz, 1.0);
    }
    let albedo = textureLoad(er_talbedo, texel, 0).xyz;
    if (min(albedo.x, min(albedo.y, albedo.z)) <= 0.02 || er_kh_texel(albedo)) {
        return vec4<f32>(0.0);
    }
    let direct = max(textureLoad(er_tdirect, texel, 0).xyz, vec3<f32>(0.0));
    return vec4<f32>(direct / albedo, 1.0);
}
#endif

#ifdef ER_PROBE_WORLD
// (IMPL_ER86, probe3.ps.dxil, kh-er-render stable.rs: the user: "his lighting changes when the camera moves") THE WORLD PROBE,
// when er.probe.w = 2: texels 0..5 as the screen grid gives them, but from 16 taps ON THE GROUND around his feet in FIXED
// WORLD directions (every 22.5 degrees from 11.25, er.probe.z cm away, at his feet's height), so the ground the rig measures
// no longer turns with the camera. A tap counts where it is in the picture, the scene depth without him (er_tother) shows the
// ground within 30 cm of it, his body capsules (er.caps) do not hide it from the camera, and its surface colour is readable.
// Texel 0..5: the means over the taps that count (w = their number), texel 2 the shadow mask's mean. Mirrored in stable.rs
// (world_taps, seg_dist, hidden_by) and checked there.
fn er_seg_dist(p1: vec3<f32>, q1: vec3<f32>, p2: vec3<f32>, q2: vec3<f32>) -> f32 {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);
    let c = dot(d1, r);
    let b = dot(d1, d2);
    let denom = a * e - b * b;
    var s = 0.0;
    if (denom > 1.0e-6) {
        s = clamp((b * f - c * e) / denom, 0.0, 1.0);
    }
    var t = (b * s + f) / max(e, 1.0e-6);
    if (t < 0.0) {
        t = 0.0;
        s = clamp(-c / max(a, 1.0e-6), 0.0, 1.0);
    } else if (t > 1.0) {
        t = 1.0;
        s = clamp((b - c) / max(a, 1.0e-6), 0.0, 1.0);
    }
    return length((p1 + d1 * s) - (p2 + d2 * t));
}

fn er_hidden_by_him(p: vec3<f32>) -> bool {
    for (var k = 0u; k < 16u; k++) {
        let a = er.caps[2u * k];
        let b = er.caps[2u * k + 1u];
        if (b.w > 0.5 && er_seg_dist(er.camera.xyz, p, a.xyz, b.xyz) < a.w + 4.0) {
            return true;
        }
    }
    return false;
}

// The texel of world tap k when it counts (z = 1), else z = 0.
fn world_tap_at(k: u32) -> vec3<i32> {
    let angle = (f32(k) * 22.5 + 11.25) * 0.017453292;
    let p = er.blob.xyz + vec3<f32>(cos(angle), sin(angle), 0.0) * er.probe.z;
    let c = er.clip_from_game * vec4<f32>(p, 1.0);
    if (c.w <= 0.3) {
        return vec3<i32>(0);
    }
    let uv = vec2<f32>(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5);
    if (any(uv < vec2<f32>(0.01)) || any(uv > vec2<f32>(0.99))) {
        return vec3<i32>(0);
    }
    let texel = vec2<i32>(uv * er.viewport.xy);
    let device_z = textureLoad(er_tother, texel, 0).x;
    if (device_z <= 0.0) {
        return vec3<i32>(0);
    }
    let view_z = 1.0 / (device_z * er.depth.z + er.depth.w);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let v = vec3<f32>(ndc.x * er.proj.x, ndc.y * er.proj.y, 1.0) * (view_z * 100.0);
    let q = er.camera.xyz + er.cam_right.xyz * v.x + er.cam_up.xyz * v.y + er.cam_forward.xyz * v.z;
    if (length(q - p) > 30.0 || er_hidden_by_him(p)) {
        return vec3<i32>(0);
    }
    let albedo = textureLoad(er_talbedo, texel, 0).xyz;
    if (min(albedo.x, min(albedo.y, albedo.z)) <= 0.02 || er_kh_texel(albedo)) {
        return vec3<i32>(0);
    }
    return vec3<i32>(texel, 1);
}

fn world_probe(which: u32) -> vec4<f32> {
    var sum = vec4<f32>(0.0);
    var n = 0.0;
    for (var k = 0u; k < 16u; k++) {
        let at = world_tap_at(k);
        if (at.z == 0) {
            continue;
        }
        let texel = at.xy;
        let albedo = textureLoad(er_talbedo, texel, 0).xyz;
        let scene = max(textureLoad(er_tscene, texel, 0).xyz, vec3<f32>(0.0));
        n += 1.0;
        if (which == 0u) {
            sum += vec4<f32>(scene / albedo, 1.0);
        } else if (which == 1u) {
            sum += vec4<f32>(max(textureLoad(er_tdirect, texel, 0).xyz, vec3<f32>(0.0)), 1.0);
        } else if (which == 2u) {
            sum += textureLoad(er_tmask, texel, 0);
        } else if (which == 3u) {
            sum += vec4<f32>(scene, 1.0);
        } else if (which == 5u) {
            sum += vec4<f32>(max(textureLoad(er_tother, texel, 0).xyz, vec3<f32>(0.0)), 1.0);
        } else {
            sum += vec4<f32>(albedo, 1.0);
        }
    }
    if (n < 0.5) {
        return vec4<f32>(0.0);
    }
    if (which == 2u) {
        return sum / n;
    }
    return vec4<f32>(sum.xyz / n, n);
}

// Texels 64..127: each tap on its own (64 + k direct light x surface colour with w = 1 when it counts, 80 + k surface
// colour, 96 + k scene colour, 112 + k shadow mask): the host keeps the last value of a direction out of sight (stable.rs
// TapMemory), so the ground it averages does not change with what the camera sees.
fn world_tap_texel(which: u32) -> vec4<f32> {
    let k = (which - 64u) % 16u;
    let kind = (which - 64u) / 16u;
    let at = world_tap_at(k);
    if (at.z == 0) {
        return vec4<f32>(0.0);
    }
    if (kind == 0u) {
        return vec4<f32>(max(textureLoad(er_tdirect, at.xy, 0).xyz, vec3<f32>(0.0)), 1.0);
    } else if (kind == 1u) {
        return vec4<f32>(textureLoad(er_talbedo, at.xy, 0).xyz, 1.0);
    } else if (kind == 2u) {
        return vec4<f32>(max(textureLoad(er_tscene, at.xy, 0).xyz, vec3<f32>(0.0)), 1.0);
    }
    return textureLoad(er_tmask, at.xy, 0);
}
#endif

@fragment
fn probe_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let which = u32(position.x);
#ifdef ER_PROBE_RING
    if (er.probe.w != 0.0 && which >= 16u && which < 64u) {
        return ring_probe(which);
    }
#endif
#ifdef ER_PROBE_WORLD
    if (er.probe.w == 2.0 && which <= 5u) {
        return world_probe(which);
    }
    if (er.probe.w == 2.0 && which >= 64u && which < 128u) {
        return world_tap_texel(which);
    }
#endif
    if (er.probe.w == 0.0 || which > 6u) {
        return vec4<f32>(0.0);
    }
    var sum = vec4<f32>(0.0);
    var best = vec3<f32>(0.0);
    for (var k = 0; k < 9; k++) {
        let offset = vec2<f32>(f32(k % 3) - 1.0, f32(k / 3) - 1.0) * er.probe.z;
        let texel = clamp(vec2<i32>(er.probe.xy + offset), vec2<i32>(0), vec2<i32>(er.viewport.xy) - vec2<i32>(1));
        let scene = max(textureLoad(er_tscene, texel, 0).xyz, vec3<f32>(0.0));
        let albedo = textureLoad(er_talbedo, texel, 0).xyz;
        // (RSF2) a tap on a KH character's own G-buffer is not the ground's light
        if (which != 2u && which != 6u && er_kh_texel(albedo)) {
            continue;
        }
        if (which == 0u) {
            if (min(albedo.x, min(albedo.y, albedo.z)) > 0.02) {
                sum += vec4<f32>(scene / albedo, 1.0);
            }
        } else if (which == 1u) {
            sum += vec4<f32>(max(textureLoad(er_tdirect, texel, 0).xyz, vec3<f32>(0.0)), 1.0);
        } else if (which == 2u) {
            // (all four channels: the count is not kept)
            sum += textureLoad(er_tmask, texel, 0) / 9.0;
        } else if (which == 3u) {
            sum += vec4<f32>(scene, 1.0);
        } else if (which == 5u) {
            sum += vec4<f32>(max(textureLoad(er_tother, texel, 0).xyz, vec3<f32>(0.0)), 1.0);
        } else if (which == 6u) {
            // THE ENVIRONMENT TAPS (IMPL_ER6, the Keyblade's reflections): the scene colour along the top of
            // the picture (what is above and ahead of the camera: the sky outdoors), nine taps.
            // THE BRIGHTEST of them is kept: the open sky where any of it shows (walls and trees beside it
            // are darker); under a roof all are dark and so is the environment.
            for (var row = 0; row < 2; row++) {
                let env_at = vec2<i32>(vec2<f32>((0.1 + 0.1 * f32(k)) * er.viewport.x, (0.04 + 0.09 * f32(row)) * er.viewport.y));
                let c = max(textureLoad(er_tscene, env_at, 0).xyz, vec3<f32>(0.0));
                if (dot(c, vec3<f32>(0.2126, 0.7152, 0.0722)) > dot(best, vec3<f32>(0.2126, 0.7152, 0.0722))) {
                    best = c;
                }
            }
        } else {
            sum += vec4<f32>(albedo, 1.0);
        }
    }
    if (which == 2u) {
        return sum;
    }
    if (which == 6u) {
        return vec4<f32>(best, 1.0);
    }
    return vec4<f32>(sum.xyz / max(sum.w, 1.0), sum.w);
}
#endif

#ifdef ER_VELO
// HIS MOTION VECTORS, in the encoding of the game's own velocity shaders (read from their
// disassembly: (now.xy / now.w - before.xy / before.w) * (aspect / 2, -1 / 2) *
// FC_VelocityEncodeParam.x + FC_VelocityEncodeParam.y), into the game's velocity target.
@group(1) @binding(16) var er_tgame: texture_2d<f32>;

@fragment
fn velo_ps(in: VsOut) -> @location(0) vec4<f32> {
    // (NF) a pixel of him the camera fade leaves out has no motion vector of his either
    if (!er_camfade_keep(in.position.xy)) {
        discard;
    }
    var encode = textureLoad(er_tgame, vec2<i32>(70, 6), 0).xy;
    if (encode.x == 0.0) {
        encode = vec2<f32>(1.0, 0.0);
    }
    let moved = (in.cur.xy / in.cur.w - in.prev.xy / in.prev.w) * vec2<f32>(0.5 * er.viewport.x / er.viewport.y, -0.5) * er.depth_uv.w;
    return vec4<f32>(moved * encode.x + vec2<f32>(encode.y), 0.0, 1.0);
}
#endif

#ifdef ER_SHADOW
// (IMPL_ER17) HIS REAL SHADOW (gfx.rs `draw_cast`): his skinned mesh, depth only, into one cascade of the game's shadow
// atlas. The data texture's one row is the cbSceneParam the game's own casters of that pass were drawn with: texel 3 =
// SC_CameraPos, texels 24..27 = VC_MatrixViewProj (rows; relative to SC_CameraPos): the light's view-projection.
@group(1) @binding(16) var er_tcast: texture_2d<f32>;

@vertex
fn shadow_vs(v: VsIn) -> @builtin(position) vec4<f32> {
    let skin = sk.joints[v.joints.x] * v.weights.x + sk.joints[v.joints.y] * v.weights.y + sk.joints[v.joints.z] * v.weights.z + sk.joints[v.joints.w] * v.weights.w;
    let world = sk.game_from_pack * skin * vec4<f32>(v.position, 1.0);
    let host = er.host_from_game * vec4<f32>(world.xyz, 1.0);
    let rel = vec4<f32>(host.xyz - textureLoad(er_tcast, vec2<i32>(3, 0), 0).xyz, 1.0);
    return vec4<f32>(
        dot(textureLoad(er_tcast, vec2<i32>(24, 0), 0), rel),
        dot(textureLoad(er_tcast, vec2<i32>(25, 0), 0), rel),
        dot(textureLoad(er_tcast, vec2<i32>(26, 0), 0), rel),
        dot(textureLoad(er_tcast, vec2<i32>(27, 0), 0), rel));
}
#endif

#ifdef ER_VERTEX
// The game-data texture (light.rs): row 6 = the game's cbSceneParam of this frame.
@group(1) @binding(16) var er_tgame: texture_2d<f32>;
// The skin of the frame drawn before.
@group(0) @binding(3) var<uniform> skp: ErSkin;

#ifdef KH_CLOTH_GUARD
// (ER79, the user: "I just want his pants to never clip into his leg") THE OUTFIT'S LEG GUARD, per vertex: the palette's
// last six entries (kh-er-render outfit.rs `write_leg_guard`, only for a skin of at most 314 joints): [319] col 0 = (-,
// capsule count, -, 1 = on: a joint matrix has 0 there); [318] = one bit per joint (16 words): the joint is cloth; [314] .. [317] = two capsules each (pack
// space: end a + radius, end b + radius; flat ends). A vertex with any cloth weight that ends inside a capsule is put on its
// surface; every other vertex (the leg) is never moved.
fn kh_guard_cloth(header: vec4<f32>, flags: mat4x4<f32>, j: vec4<u32>, w: vec4<f32>) -> f32 {
    if (header.w != 1.0) {
        return 0.0;
    }
    var c = 0.0;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let joint = j[k];
        let word = (joint >> 5u) & 15u;
        let bits = bitcast<u32>(flags[word >> 2u][word & 3u]);
        if (joint < 512u && ((bits >> (joint & 31u)) & 1u) != 0u) {
            c = c + w[k];
        }
    }
    return c;
}

fn kh_guard_push(p0: vec3<f32>, a: vec4<f32>, b: vec4<f32>) -> vec3<f32> {
    let d = b.xyz - a.xyz;
    let l2 = dot(d, d);
    if (l2 < 1.0e-10) {
        return p0;
    }
    let t = dot(p0 - a.xyz, d) / l2;
    if (t < 0.0 || t > 1.0) {
        return p0;
    }
    let q = a.xyz + d * t;
    let r = mix(a.w, b.w, t);
    let off = p0 - q;
    let l = length(off);
    if (l >= r || l < 1.0e-6) {
        return p0;
    }
    return q + off * (r / l);
}

fn kh_guard(p: vec3<f32>, header: vec4<f32>, c01: mat4x4<f32>, c23: mat4x4<f32>, c45: mat4x4<f32>, c67: mat4x4<f32>) -> vec3<f32> {
    var q = p;
    let n = u32(header.y);
    // eight rounds: a push out of one capsule can land in another (thigh / shin at the knee, the other leg); three left a
    // few vertices between the legs in the offline proof, eight none (crates/kh-er-render/examples/effects.rs --stretch)
    for (var round = 0; round < 8; round = round + 1) {
        if (n > 0u) { q = kh_guard_push(q, c01[0], c01[1]); }
        if (n > 1u) { q = kh_guard_push(q, c01[2], c01[3]); }
        if (n > 2u) { q = kh_guard_push(q, c23[0], c23[1]); }
        if (n > 3u) { q = kh_guard_push(q, c23[2], c23[3]); }
        if (n > 4u) { q = kh_guard_push(q, c45[0], c45[1]); }
        if (n > 5u) { q = kh_guard_push(q, c45[2], c45[3]); }
        if (n > 6u) { q = kh_guard_push(q, c67[0], c67[1]); }
        if (n > 7u) { q = kh_guard_push(q, c67[2], c67[3]); }
    }
    return q;
}
#endif

@vertex
fn vs_main(v: VsIn) -> VsOut {
    let skin = sk.joints[v.joints.x] * v.weights.x + sk.joints[v.joints.y] * v.weights.y + sk.joints[v.joints.z] * v.weights.z + sk.joints[v.joints.w] * v.weights.w;
    let m = sk.game_from_pack * skin;
#ifdef KH_CLOTH_GUARD
    var pack_pos = (skin * vec4<f32>(v.position, 1.0)).xyz;
    if (kh_guard_cloth(sk.joints[319u][0], sk.joints[318u], v.joints, v.weights) > 0.0) {
        pack_pos = kh_guard(pack_pos, sk.joints[319u][0], sk.joints[314u], sk.joints[315u], sk.joints[316u], sk.joints[317u]);
    }
    let world = sk.game_from_pack * vec4<f32>(pack_pos, 1.0);
#else
    let world = m * vec4<f32>(v.position, 1.0);
#endif
    var o: VsOut;
    o.world = world.xyz;
    o.normal = (m * vec4<f32>(v.normal, 0.0)).xyz;
    o.tangent = vec4<f32>((m * vec4<f32>(v.tangent.xyz, 0.0)).xyz, v.tangent.w);
    o.uv0 = v.uv0;
    o.uv1 = v.uv1;
    o.uv2 = v.uv2;
#ifdef KH_PSP_OUT
    o.psp = vec3<f32>(v.position.x, v.position.z, v.position.y) * 100.0;
#endif
    o.position = er.clip_from_game * vec4<f32>(world.xyz, 1.0);
    o.cur = o.position;
    o.prev = o.position;
    let skin_before = skp.joints[v.joints.x] * v.weights.x + skp.joints[v.joints.y] * v.weights.y + skp.joints[v.joints.z] * v.weights.z + skp.joints[v.joints.w] * v.weights.w;
#ifdef KH_CLOTH_GUARD
    var pack_before = (skin_before * vec4<f32>(v.position, 1.0)).xyz;
    if (kh_guard_cloth(skp.joints[319u][0], skp.joints[318u], v.joints, v.weights) > 0.0) {
        pack_before = kh_guard(pack_before, skp.joints[319u][0], skp.joints[314u], skp.joints[315u], skp.joints[316u], skp.joints[317u]);
    }
    let world_before = skp.game_from_pack * vec4<f32>(pack_before, 1.0);
#else
    let world_before = skp.game_from_pack * skin_before * vec4<f32>(v.position, 1.0);
#endif
    if (er.tune.w != 0.0) {
        // The game's own view-projection of this frame (VC_MatrixViewProj: rows, relative to
        // SC_CameraPos), when the row is this frame's scene (its screen size is the target's).
        let size = textureLoad(er_tgame, vec2<i32>(1, 6), 0);
        if (abs(size.x - er.viewport.x) < 0.5 && abs(size.y - er.viewport.y) < 0.5) {
            let host = er.host_from_game * vec4<f32>(world.xyz, 1.0);
            let rel = vec4<f32>(host.xyz - textureLoad(er_tgame, vec2<i32>(3, 6), 0).xyz, 1.0);
            o.position = vec4<f32>(
                dot(textureLoad(er_tgame, vec2<i32>(24, 6), 0), rel),
                dot(textureLoad(er_tgame, vec2<i32>(25, 6), 0), rel),
                dot(textureLoad(er_tgame, vec2<i32>(26, 6), 0), rel),
                dot(textureLoad(er_tgame, vec2<i32>(27, 6), 0), rel));
            o.cur = o.position;
#ifdef ER_VELO_OWN
            // (ER88, skin_v88 / skin_cg_v88: the velocity draw only) THE CAMERA'S PART FROM OUR OWN RECORD (kh-er-render taa.rs).
            // er.prev_host_from_game = R x (game -> host), R = the camera motion from the camera he was drawn with a frame ago
            // to this frame's; its w row is 2 when the record is there. His position a frame ago is then seen through THIS frame's
            // view-projection (VC_MatrixViewProj: the jitter cancels), i.e. exactly where he was drawn. Used where our own view of
            // this frame lands within 1.5 px of the game's (its jitter taken off), else the game's previous matrix as before.
            let host_own = er.prev_host_from_game * vec4<f32>(world_before.xyz, 1.0);
            let own_now = er.clip_from_game * vec4<f32>(world.xyz, 1.0);
            let jit = vec2<f32>(textureLoad(er_tgame, vec2<i32>(16, 6), 0).z, textureLoad(er_tgame, vec2<i32>(17, 6), 0).z);
            var off = vec2<f32>(1.0e9, 1.0e9);
            if (own_now.w > 1.0e-4 && o.position.w > 1.0e-4) {
                off = abs(own_now.xy / own_now.w - (o.position.xy / o.position.w - jit)) * 0.5 * er.viewport.xy;
            }
            if (host_own.w > 1.5 && off.x < 1.5 && off.y < 1.5) {
                let rel_own = vec4<f32>(host_own.xyz - textureLoad(er_tgame, vec2<i32>(3, 6), 0).xyz, 1.0);
                o.prev = vec4<f32>(
                    dot(textureLoad(er_tgame, vec2<i32>(24, 6), 0), rel_own),
                    dot(textureLoad(er_tgame, vec2<i32>(25, 6), 0), rel_own),
                    dot(textureLoad(er_tgame, vec2<i32>(26, 6), 0), rel_own),
                    dot(textureLoad(er_tgame, vec2<i32>(27, 6), 0), rel_own));
            } else {
                // (the record flagged = same origin as this frame: the plain game -> host is this frame's)
                var host_before = er.prev_host_from_game * vec4<f32>(world_before.xyz, 1.0);
                if (host_own.w > 1.5) {
                    host_before = er.host_from_game * vec4<f32>(world_before.xyz, 1.0);
                }
                let rel_before = vec4<f32>(host_before.xyz - textureLoad(er_tgame, vec2<i32>(28, 6), 0).xyz, 1.0);
                o.prev = vec4<f32>(
                    dot(textureLoad(er_tgame, vec2<i32>(29, 6), 0), rel_before),
                    dot(textureLoad(er_tgame, vec2<i32>(30, 6), 0), rel_before),
                    dot(textureLoad(er_tgame, vec2<i32>(31, 6), 0), rel_before),
                    dot(textureLoad(er_tgame, vec2<i32>(32, 6), 0), rel_before));
            }
#else
            // A frame ago: VC_PrevMatrixViewProj (rows 29..32) relative to SC_PrevCameraPos (28).
            let host_before = er.prev_host_from_game * vec4<f32>(world_before.xyz, 1.0);
            let rel_before = vec4<f32>(host_before.xyz - textureLoad(er_tgame, vec2<i32>(28, 6), 0).xyz, 1.0);
            o.prev = vec4<f32>(
                dot(textureLoad(er_tgame, vec2<i32>(29, 6), 0), rel_before),
                dot(textureLoad(er_tgame, vec2<i32>(30, 6), 0), rel_before),
                dot(textureLoad(er_tgame, vec2<i32>(31, 6), 0), rel_before),
                dot(textureLoad(er_tgame, vec2<i32>(32, 6), 0), rel_before));
#endif
        }
    }
    o.view_z = o.position.w;
    return o;
}
#endif

#ifdef ER_FRAGMENT
//KH_COMMON

//KH_GENERATED

struct ErFrag {
    @location(0) color: vec4<f32>,
#ifdef KH_HAIR91_MSAA
    // (ER91) the hair's own 4x target pair (hair91.rs): his motion vector (the game's encoding) x coverage, his device depth
    // x coverage, the coverage
    @location(1) aux: vec4<f32>,
#endif
#ifdef KH_MASKED
    @builtin(sample_mask) mask: u32,
#endif
}

#ifdef KH_HAIR91_MSAA
// (ER91) the game-data texture (every slot's table entry 16; light.rs): row 6 texel 70 = FC_VelocityEncodeParam
@group(1) @binding(16) var er_tgame: texture_2d<f32>;
#endif

#ifdef KH_EXTRA_LIGHTS
// (IMPL_ER44) The extra lights through the material's own light shader (kh_light: Lambert + GGX, the subsurface
// transmission of shading model 2), summed into the light buffer that the post-process material composes.
fn kh_extra_lights(g: KhOut, n: vec3<f32>, v: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    for (var k = 0; k < 6; k++) {
        if (f32(k) >= er.xl_head.x) {
            break;
        }
        let r = er.xl_pos[k].w;
        let to_light = er.xl_pos[k].xyz - world;
        let d2 = dot(to_light, to_light);
        if (r <= 0.0 || d2 >= r * r) {
            continue;
        }
        let l = to_light * inverseSqrt(max(d2, 1.0e-4));
        let radial = pow(saturate(1.0 - d2 / (r * r)), er.xl_col[k].w);
        sum += kh_light(g, n, v, l, 1.0, 0.08) * (radial * er.xl_col[k].xyz);
    }
    return sum;
}
#endif

#ifdef KH_HYBRID
// (IMPL_ER78) THE GAME'S LIGHT ON HIM. t21: the game's diffuse light accumulation as its deferred stage left it (at his
// pixels: the light on the white, non-metallic surface his G-buffer pass wrote = what he received: every local light,
// shadow, AO, GI); t22: its screen-space shadow mask (r, g occlusion; a: lit by the sun). White when not bound.
@group(1) @binding(21) var hy_tlight: texture_2d<f32>;
@group(1) @binding(22) var hy_tmask: texture_2d<f32>;

fn hy_luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// The staircase (src/hybrid.rs `band`, the same formula, unit-tested there): l = received light / the white level.
fn hy_band(l: f32) -> f32 {
    let lo = max(er.hy[1].x, 0.001);
    let soft = max(er.hy[1].z, 0.0001);
    let dark = er.hy[2].x * sqrt(clamp(l / lo, 0.0, 1.0));
    let s1 = smoothstep(lo - soft, lo + soft, l);
    let s2 = smoothstep(er.hy[1].y - soft, er.hy[1].y + soft, l);
    return dark + (1.0 - dark) * s1 + er.hy[2].y * s2 + max(l - 1.0, 0.0);
}

#ifdef KH_HY3
// (HY) THE HYBRID, third version (`*_hy3`): the light of the game's own render pass, read from OUR CAPTURE of it (gfx.rs
// `HyCapture`, hycap.ps.dxil: drawn right after the game's deferred stage over the box of his G-buffer pass; never the game's
// targets themselves, so a draw at a later point of the frame (the hair fringe at the velocity split) reads the same values).
// t21 = (the game's diffuse light on the surface his G-buffer pass wrote, its albedo taken out again: what a white surface
// received, SCENE units; the game's occlusion min(mask.r, mask.g)), t22 = (the G-buffer's world normal there (host axes), the
// game's sun visibility (its screen-space cascaded shadow mask)). Cleared texels (0) = nothing captured there.
// (`r.hybrid_debug`: what kh_hybrid_light found at this pixel, shown by fs_main; < 0 = nothing to show)
var<private> hy_dbg: vec3<f32> = vec3<f32>(-1.0);
// (RSF2 round 13) the world's shadow factor at this pixel (the capture's geo.w with the wide fill, er.hy[8].w = 2): fs_main
// multiplies the whole colour by it; 1 = none
var<private> hy_world: f32 = 1.0;

fn hy_k() -> f32 {
    // scene -> KH units: each character's own (gfx/party.rs writes a friend's own out scale into tune.x)
    return 1.0 / max(er.tune.x, 1.0e-9);
}

// The capture at a texel, VALIDATED against this pixel's own normal: the G-buffer there must hold HIS surface (the normal he
// wrote, within `er.hy[8].x` (cos)); else the nearest of up to 16 neighbours (1 and 2 px) that does (the hair's soft fringe has
// no G-buffer of its own: its core lies beside it). Returns (light rgb, occlusion) and (normal, sun visibility); .w of the
// first < 0 = no valid texel: the caller falls back to today's light for this pixel.
struct HyTap {
    light: vec4<f32>,
    geo: vec4<f32>,
    // 0 its own texel, 1 a neighbour, 2 none
    source: f32,
}

fn hy_tap_ok(a: vec4<f32>, b: vec4<f32>, n_host: vec3<f32>, cos_min: f32) -> f32 {
    let len = length(b.xyz);
    if (a.w <= 0.0 || len < 0.9 || len > 1.1 || !(a.x >= 0.0) || !(a.y >= 0.0) || !(a.z >= 0.0)) {
        return -2.0;
    }
    let c = dot(b.xyz / len, n_host);
    return select(-2.0, c, c >= cos_min);
}

fn hy_tap(svpos: vec4<f32>, n_host: vec3<f32>) -> HyTap {
    var out: HyTap;
    let t = vec2<i32>(svpos.xy);
    let size = vec2<i32>(textureDimensions(hy_tlight));
    let cos_min = clamp(er.hy[8].x, -1.0, 1.0);
    let a0 = textureLoad(hy_tlight, clamp(t, vec2<i32>(0), size - vec2<i32>(1)), 0);
    let b0 = textureLoad(hy_tmask, clamp(t, vec2<i32>(0), size - vec2<i32>(1)), 0);
    if (hy_tap_ok(a0, b0, n_host, cos_min) > -1.5) {
        out.light = a0;
        out.geo = b0;
        out.source = 0.0;
        return out;
    }
    var best = -1.5;
    out.light = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    out.geo = vec4<f32>(0.0, 0.0, 1.0, 1.0);
    out.source = 2.0;
    let reach = i32(clamp(er.hy[8].y, 0.0, 2.0));
    for (var r = 1; r <= reach; r++) {
        for (var k = 0; k < 8; k++) {
            let o = vec2<i32>(select(select(0, -r, k == 3 || k == 5 || k == 6), r, k == 0 || k == 4 || k == 7), select(select(0, -r, k == 2 || k == 6 || k == 7), r, k == 1 || k == 4 || k == 5));
            let p = clamp(t + o, vec2<i32>(0), size - vec2<i32>(1));
            let a = textureLoad(hy_tlight, p, 0);
            let b = textureLoad(hy_tmask, p, 0);
            let c = hy_tap_ok(a, b, n_host, cos_min - 0.15);
            if (c > best) {
                best = c;
                out.light = a;
                out.geo = b;
                out.source = 1.0;
            }
        }
        if (best > -1.5) {
            break;
        }
    }
    // (RSF2 round 13) still none (the hair fringe's tips, up to tens of pixels from the core: it has no G-buffer of its own):
    // rings at 4, 8, 16 and 32 px, ANY captured KH texel (no normal check: the fringe takes its core's light), so a fringe never
    // falls back to the rig, which is black wherever the feet probe has no reading (a close camera, the feet out of the picture)
    if (best < -1.5 && er.hy[8].w > 1.5) {
        for (var q = 2; q <= 5; q++) {
            let r = 1 << u32(q);
            for (var k = 0; k < 8; k++) {
                let o = vec2<i32>(select(select(0, -r, k == 3 || k == 5 || k == 6), r, k == 0 || k == 4 || k == 7), select(select(0, -r, k == 2 || k == 6 || k == 7), r, k == 1 || k == 4 || k == 5));
                let p = clamp(t + o, vec2<i32>(0), size - vec2<i32>(1));
                let a = textureLoad(hy_tlight, p, 0);
                let b = textureLoad(hy_tmask, p, 0);
                if (hy_tap_ok(a, b, n_host, -2.0) > -1.5) {
                    out.light = a;
                    out.geo = b;
                    out.source = 1.0;
                    return out;
                }
            }
        }
    }
    return out;
}

// HIS OWN SHADOW from the sun on himself: his capsules (er.caps, as the thrown shadow) between the point and the sun, soft;
// a capsule that holds the point itself (the limb the point lies on) is skipped. 1 = lit.
fn hy_self_shadow(p: vec3<f32>, to_sun: vec3<f32>) -> f32 {
    var lit = 1.0;
    for (var k = 0; k < 16; k++) {
        let a = er.caps[2 * k];
        let b = er.caps[2 * k + 1];
        // (the Keyblade's capsules (w = 2) cast too; absent ones (0) not)
        if (b.w < 0.5) {
            continue;
        }
        let ab = b.xyz - a.xyz;
        let c = max(dot(ab, ab), 0.0001);
        // the point's own capsule: skipped
        let u0 = saturate(dot(p - a.xyz, ab) / c);
        if (length(p - (a.xyz + ab * u0)) < a.w * 1.35) {
            continue;
        }
        // the closest approach of the ray p + s to_sun (s > 0) to the axis
        let w0 = p - a.xyz;
        let bb = dot(to_sun, ab);
        let denom = c - bb * bb;
        var u = 0.5;
        if (denom > 0.0001) {
            u = saturate((dot(ab, w0) - bb * dot(to_sun, w0)) / denom);
        }
        let s = max(dot(to_sun, a.xyz + ab * u - p), 0.0);
        u = saturate(dot(ab, p + to_sun * s - a.xyz) / c);
        let d = length(p + to_sun * s - (a.xyz + ab * u));
        let soft = a.w * 0.45 + s * 0.06;
        let cover = 1.0 - smoothstep(a.w - soft * 0.5, a.w + soft, d);
        lit = min(lit, 1.0 - cover * step(1.0, s) * saturate(1.0 - s / 400.0));
    }
    return lit;
}

// (char_common.wgsl calls it after the rig's lights, before the extra lights) THE LIGHT BUFFER KH3's post-process material
// composes (its 16-step colour table makes the toon bands, its boards the rims): KH3 keeps the DIRECT light here and the
// undirected light in the indirect term (the rig's SH, kept by fs_main at its share x the game's occlusion). So: the game's
// light at this pixel (every light of its pass: the sun with its cascaded shadow, the sky, every local light, GI, AO) less
// the share of the rig's undirected light the indirect term already gives (`r.hybrid_amb_sub`), his own sun shadow on himself
// (capsules), lifted (`lift`), the soft staircase on its brightness (hue kept), times the diffuse colour; + a share of the
// rig's light; + KH3's specular from the sun, shadowed per pixel. A pixel whose capture is not valid: today's light.
fn kh_hybrid_light(g: KhOut, n: vec3<f32>, v: vec3<f32>, key_dir: vec3<f32>, key_color: vec3<f32>, svpos: vec4<f32>, rig_lit: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    if (er.hy[0].x == 0.0) {
        return rig_lit;
    }
    let n_host = normalize((er.host_from_game * vec4<f32>(n, 0.0)).xyz);
    let tap = hy_tap(svpos, n_host);
    let debug = er.hy[5].w;
    if (tap.light.w < 0.0) {
        if (debug == 1.0) {
            hy_dbg = vec3<f32>(1.0, 0.0, 0.0);
        }
        return rig_lit;
    }
    let k = hy_k();
    let ao = clamp(tap.light.w, 0.05, 1.0);
    var vis = clamp(tap.geo.w, 0.0, 1.0);
    // (RSF2 round 13) the wide fill: the captured light is the light in the sun, geo.w the world's shadow factor (fs_main)
    if (er.hy[8].w > 1.5) {
        hy_world = vis;
    }
    // (the game's light, finite, in KH units, its occlusion to the asked strength)
    var e = min(max(tap.light.xyz, vec3<f32>(0.0)), vec3<f32>(6.0e4)) * k * pow(ao, er.hy[2].z - 1.0);
    // the undirected light the indirect term gives: out of the light buffer (KH3 keeps it apart)
    let up = max(n.z, 0.0);
    let ambient = (er.lit_ambient.xyz + er.lit_up.xyz * up) * k * ao;
    e = max(e - ambient * er.hy[5].y, vec3<f32>(0.0));
    // his own shadow on himself from the sun: what the sun puts here (its light per N.L, the game's visibility) taken away
    let to_sun = normalize(er.hy[6].xyz);
    var self_lit = 1.0;
    if (er.hy[6].w > 0.0 && dot(er.hy[6].xyz, er.hy[6].xyz) > 0.5) {
        self_lit = mix(1.0, hy_self_shadow(world, to_sun), clamp(er.hy[6].w, 0.0, 1.0));
        let sun = er.hy[7].xyz * k * max(dot(n, to_sun), 0.0) * vis;
        e = max(e - min(sun, e) * (1.0 - self_lit), vec3<f32>(0.0));
    }
    e *= max(er.hy[7].w, 0.0);
    let l = hy_luma(e) / max(er.hy[3].x, 1.0e-6);
    let toned = l + (hy_band(l) - l) * er.hy[1].w;
    let scale = select(0.0, toned / max(l, 1.0e-5), l > 1.0e-5);
    let diffuse = g.o3.xyz - g.o3.xyz * g.o2.x;
    var out = e * scale * diffuse * er.hy[0].y + rig_lit * er.hy[0].z;
    if (er.hy[0].w > 0.0) {
        // KH3's own sun specular (its light shader less its diffuse part), shadowed by the game's mask and his own shadow
        let shadow = vis * self_lit;
        let sun_kh = er.hy[7].xyz * k * 3.141593;
        let both = kh_light(g, n, v, to_sun, shadow, 0.08);
        let plain = diffuse * 0.318310 * clamp(dot(n, to_sun), 0.0, 1.0) * shadow;
        out += max(both - plain, vec3<f32>(0.0)) * sun_kh * er.hy[0].w;
    }
    // (`r.hybrid_debug`, fs_main shows it in place of his colour: 1 the source (green: his own texel, yellow: a neighbour's,
    // red: none = today's light), 2 the game's light in his light buffer (before the colour), 3 its occlusion, 4 the sun's
    // visibility x his own shadow)
    if (debug == 1.0) {
        hy_dbg = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), tap.source > 0.5);
    } else if (debug == 2.0) {
        hy_dbg = e * scale / max(er.hy[3].x, 1.0e-6);
    } else if (debug == 3.0) {
        hy_dbg = vec3<f32>(ao);
    } else if (debug == 4.0) {
        hy_dbg = vec3<f32>(vis * self_lit);
    } else if (debug == 6.0) {
        // (round 13) the colour shaders' world shadow factor alone (1 = a white surface's brightness)
        hy_dbg = vec3<f32>(hy_world);
    } else if (debug == 5.0) {
        // (the offline orbit test: where on him this pixel lies, around his feet, 4 m to the unit)
        hy_dbg = (world - er.blob.xyz) / 400.0 + vec3<f32>(0.5);
    }
    return out;
}
#else
// (char_common.wgsl calls it after the rig's lights, before the extra lights) The light buffer KH3's post-process material
// composes: the game's light at this pixel (KH units), AO to its strength, the staircase on its brightness (hue kept),
// times the diffuse colour; + a share of the rig's light; + KH3's specular from the sun, shadowed by the game's mask here.
fn kh_hybrid_light(g: KhOut, n: vec3<f32>, v: vec3<f32>, key_dir: vec3<f32>, key_color: vec3<f32>, svpos: vec4<f32>, rig_lit: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    if (er.hy[0].x == 0.0) {
        return rig_lit;
    }
    let t = vec2<i32>(svpos.xy);
    let m = textureLoad(hy_tmask, t, 0);
    let ao = clamp(min(m.x, m.y), 0.05, 1.0);
    // (IMPL_ER78b) the light clamped to a finite range (an R11G11B10 texel can hold infinity; min / max drop a NaN)
    let e = min(max(textureLoad(hy_tlight, t, 0).xyz, vec3<f32>(0.0)), vec3<f32>(6.0e4)) * er.hy[3].y * pow(ao, er.hy[2].z - 1.0);
    let l = hy_luma(e) / max(er.hy[3].x, 1.0e-6);
    let toned = l + (hy_band(l) - l) * er.hy[1].w;
    let scale = select(0.0, toned / max(l, 1.0e-5), l > 1.0e-5);
    let diffuse = g.o3.xyz - g.o3.xyz * g.o2.x;
    var out = e * scale * diffuse * er.hy[0].y + rig_lit * er.hy[0].z;
    if (er.hy[0].w > 0.0) {
        let shadow = clamp(m.w, 0.0, 1.0);
        let both = kh_light(g, n, v, key_dir, shadow, 0.08);
        let plain = diffuse * 0.318310 * clamp(dot(n, key_dir), 0.0, 1.0) * shadow;
        out += max(both - plain, vec3<f32>(0.0)) * key_color * er.hy[0].w;
    }
    return out;
}
#endif
#endif

#ifdef KH_BLOOD
// (IMPL_ER60) THE BLOOD LAYER (char_common.wgsl calls it on the material's G-buffer values, BEFORE any light): a
// splatter mask in the mesh's own UV space (so it stays on the cloth and the blade as he moves), more of it where the
// region asks first (boots and legs, the hands, the Keyblade toward its tip; the rest of him only with more blood);
// inside the mask the base colour is multiplied toward a dark crimson (brown when dried), the diffuse part of the
// emissive + indirect term with it, and the surface turns smooth, non-metallic and a little more specular (wet).
// No blood (both amounts 0): the values come back untouched.
fn bl_hash(p: vec2<f32>) -> f32 {
    var q = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    q += dot(q, q.yzx + vec3<f32>(33.33));
    return fract((q.x + q.y) * q.z);
}

fn bl_noise(p: vec2<f32>) -> f32 {
    let c = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    let a = bl_hash(c);
    let b = bl_hash(c + vec2<f32>(1.0, 0.0));
    let d = bl_hash(c + vec2<f32>(0.0, 1.0));
    let e = bl_hash(c + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(d, e, u.x), u.y);
}

// Splats: a few octaves of value noise (0..1, stretched round 0.5).
fn bl_splat(p: vec2<f32>) -> f32 {
    let n = 0.5 * bl_noise(p) + 0.25 * bl_noise(p * 2.03 + vec2<f32>(17.1, 3.7)) + 0.125 * bl_noise(p * 4.07 + vec2<f32>(5.3, 41.9)) + 0.0625 * bl_noise(p * 8.11 + vec2<f32>(29.4, 13.2));
    return saturate((n / 0.9375 - 0.5) * 2.4 + 0.5);
}

// Droplets: one round drop of random size and place per cell, kept by `keep` (0..1 of the cells).
fn bl_drops(p: vec2<f32>, keep: f32) -> f32 {
    let c = floor(p);
    let f = fract(p);
    var best = 0.0;
    for (var k = 0; k < 9; k++) {
        let o = vec2<f32>(f32(k % 3) - 1.0, f32(k / 3) - 1.0);
        let cell = c + o;
        let h = bl_hash(cell + vec2<f32>(91.7, 11.3));
        if (h < keep) {
            let centre = o + vec2<f32>(bl_hash(cell + vec2<f32>(3.1, 7.9)), bl_hash(cell + vec2<f32>(13.7, 5.3)));
            let radius = 0.08 + 0.22 * bl_hash(cell + vec2<f32>(23.9, 61.1));
            best = max(best, 1.0 - smoothstep(radius * 0.7, radius, length(f - centre)));
        }
    }
    return best;
}

fn bl_segment(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>) -> vec2<f32> {
    let ab = b - a;
    let t = saturate(dot(p - a, ab) / max(dot(ab, ab), 1.0e-4));
    return vec2<f32>(length(p - (a + ab * t)), t);
}

fn kh_blood(g_in: KhOut, world: vec3<f32>, uv: vec2<f32>) -> KhOut {
    var g = g_in;
    // (the mesh's UV islands are of very different sizes: the splats' scale in UV follows how many cm one UV unit spans
    // here, in whole octaves so that it holds still; taken before any branch)
    let span = (length(dpdx(world)) + length(dpdy(world))) / max(length(dpdx(uv)) + length(dpdy(uv)), 1.0e-7);
    let body = er.bl_head.x;
    let blade_amount = er.bl_head.y;
    if (body <= 0.0 && blade_amount <= 0.0) {
        return g;
    }
#ifdef KH_TRANSLUCENT
    // (the translucent parts: eyes' highlights, lashes; no blood on them)
    return g;
#endif
    // (a part whose UV hardly changes over its surface (one UV unit over more than 20 m) has no usable pattern: none)
    if (span > 2000.0) {
        return g;
    }
    let fresh = saturate(er.bl_head.z);
    // ---- the region: how early this place takes blood (0 = only with a lot, 1 = first)
    var region = 0.0;
    // (the head (face, hair) takes the least, and droplets only: a splat there covers half the face)
    var high = 1.0;
    if (er.bl_feet.w > 0.0) {
        let h = saturate((world.z - er.bl_feet.z) / er.bl_feet.w);
        region = 1.0 - smoothstep(0.08, 0.62, h);
    }
    if (er.bl_face.w > 0.0) {
        high = 1.0 - 0.75 * (1.0 - smoothstep(er.bl_face.w * 0.8, er.bl_face.w * 1.25, length(world - er.bl_face.xyz)));
    }
    if (er.bl_hand_l.w > 0.0) {
        region = max(region, 1.0 - smoothstep(er.bl_hand_l.w * 0.45, er.bl_hand_l.w, length(world - er.bl_hand_l.xyz)));
    }
    if (er.bl_hand_r.w > 0.0) {
        region = max(region, 1.0 - smoothstep(er.bl_hand_r.w * 0.45, er.bl_hand_r.w, length(world - er.bl_hand_r.xyz)));
    }
    // (even at the most blood, splats with clean cloth between them: at most 0.72 of the boots, a fifth of the chest)
    var c = body * (0.22 + 0.5 * region) * high;
    var splat_ok = smoothstep(0.4, 0.9, high);
    // ---- the Keyblade: its own amount, more toward the striking end
    if (er.bl_blade_a.w > 0.0) {
        let s = bl_segment(world, er.bl_blade_a.xyz, er.bl_blade_b.xyz);
        let on_blade = 1.0 - smoothstep(er.bl_blade_a.w * 0.7, er.bl_blade_a.w, s.x);
        c = mix(c, blade_amount * (0.3 + 0.5 * s.y), on_blade);
        splat_ok = mix(splat_ok, 1.0, on_blade);
    }
    if (c <= 0.0) {
        return g;
    }
    // ---- the mask: splats grow from their cores as the cover rises; droplets around them
    // (a splat cell of about 9 cm)
    let freq = exp2(round(log2(clamp(span / 9.0, 4.0, 128.0))));
    let q = uv * freq + vec2<f32>(er.bl_head.w * 7.31, er.bl_head.w * 3.17);
    let n = bl_splat(q);
    let threshold = 1.0 - c;
    // (the head (face, hair): droplets only, no splats: a splat there covers half the face)
    var m = smoothstep(threshold - 0.04, threshold + 0.04, n) * splat_ok;
    let core = smoothstep(threshold, threshold + 0.3, n);
    m = max(m, bl_drops(uv * (freq * 4.3) +vec2<f32>(er.bl_head.w * 1.9, 0.0), saturate(c * 1.4 - 0.05)) * 0.85);
    if (m <= 0.0) {
        return g;
    }
    // ---- the colour: a multiply toward dark crimson (thin) .. near black-red (thick); dried blood browner and darker
    let thin = mix(vec3<f32>(0.34, 0.12, 0.08), vec3<f32>(0.40, 0.06, 0.04), fresh);
    let thick = mix(vec3<f32>(0.15, 0.05, 0.03), vec3<f32>(0.20, 0.018, 0.012), fresh);
    let tint = mix(thin, thick, core);
    let own = mix(vec3<f32>(0.045, 0.014, 0.009), vec3<f32>(0.07, 0.005, 0.003), fresh);
    let base = g.o3.xyz;
    let bloody = mix(base * tint, own, 0.35 + 0.35 * core);
    g.o3 = vec4<f32>(mix(base, bloody, m), g.o3.w);
    g.o0 = vec4<f32>(g.o0.xyz * mix(vec3<f32>(1.0), tint, m), g.o0.w);
    g.o4 = vec4<f32>(g.o4.xyz * (1.0 - 0.75 * m), g.o4.w);
    // ---- wet: smooth, a little more specular (fresh blood shines, dried hardly); metal stays metal under a thin film (its
    // reflection takes the red of the multiplied base colour), not under a thick one
    let wet = m * (0.25 + 0.5 * fresh);
    g.o2 = vec4<f32>(mix(g.o2.x, 0.0, m * core), mix(g.o2.y, 0.6, wet), mix(g.o2.z, 0.3, wet), g.o2.w);
    return g;
}
#endif

fn er_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// The harness inputs of one pixel from the vertex stage's interpolants and the frame (fs_main and, IMPL_ER78, gb_main).
fn er_kh_in(in: VsOut, is_front: bool) -> KhIn {
    var i: KhIn;
    let time = er.camera.w;
    let camera = er.camera.xyz;
    i.twp = in.world - camera;
    i.svpos = vec4<f32>(in.position.xy, in.position.z, in.view_z * 100.0);
    i.front = vec4<f32>(select(0.0, 1.0, is_front));
    // The vertex factory's interpolants: TEXCOORD10 world tangent, TEXCOORD11 world normal with
    // the sign of the basis in w, TEXCOORD0.. the mesh's UV sets (a set the mesh lacks repeats
    // its last one), TEXCOORD9 the translated world position, TEXCOORD7 the vertex fog.
#ifdef KH_INTERIOR
    // (ER79, the user's 54.jpg: KH1's shorts looked hollow from below) a cloth slot drawn two-sided for its inside (kh.khs.w =
    // its interior factor, gfx.rs / outfit.rs `apply_cloth_twosided`): a back face is the cloth's inside, its normal turned in
    let n = select(normalize(in.normal), -normalize(in.normal), !is_front && kh.khs.w > 0.0);
#else
    let n = normalize(in.normal);
#endif
    let t = normalize(in.tangent.xyz);
    // pack -> game space is a reflection: the sign of the basis turns round.
    i.tc10 = vec4<f32>(t, 0.0);
    i.tc11 = vec4<f32>(n, -in.tangent.w);
    i.tc0 = vec4<f32>(in.uv0, 0.0, 0.0);
    i.tc1 = vec4<f32>(in.uv1, 0.0, 0.0);
    i.tc2 = vec4<f32>(in.uv2, 0.0, 0.0);
    i.tc3 = i.tc2;
#ifdef KH_PSP
    // (ER party S4) the owner's TEXCOORD3 = the pre-skinned local position; no third UV set then (kh-app's rule)
    i.tc2 = i.tc1;
    i.tc3 = vec4<f32>(in.psp, 0.0);
#endif
    i.tc7 = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    i.tc9 = vec4<f32>(i.twp, 1.0);
    i.col0 = vec4<f32>(1.0);
    // ---- View
    let right = er.cam_right.xyz;
    let up = er.cam_up.xyz;
    let forward = er.cam_forward.xyz;
    i.v8 = vec4<f32>(right.x, up.x, forward.x, 0.0);       // TranslatedWorldToView, per world axis
    i.v9 = vec4<f32>(right.y, up.y, forward.y, 0.0);
    i.v10 = vec4<f32>(right.z, up.z, forward.z, 0.0);
    i.v12 = vec4<f32>(right, 0.0);                         // (XW) ViewToTranslatedWorld, per view axis
    i.v13 = vec4<f32>(up, 0.0);
    i.v14 = vec4<f32>(forward, 0.0);
    i.v28 = vec4<f32>(er.proj.x, 0.0, 0.0, 0.0);           // ClipToView[0]: tan(half fov x)
    i.v53 = vec4<f32>(0.0, 0.0, 1.0, 0.0);
    i.v54 = vec4<f32>(0.5, -0.5, 0.5, 0.5);
    i.v55 = vec4<f32>(camera, 0.0);                        // WorldCameraOrigin
    i.v58 = vec4<f32>(-camera, 0.0);                       // PreViewTranslation
    i.v117 = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    i.v118 = er.viewport;
    i.v119 = i.v118;                                       // BufferSizeAndInvSize
    i.v122 = vec4<f32>(0.0, 0.0, 0.0, 1.0);                // DiffuseOverrideParameter
    i.v123 = vec4<f32>(0.0, 0.0, 0.0, 1.0);                // SpecularOverrideParameter
    i.v124 = vec4<f32>(0.0, 0.0, 0.0, 1.0);                // NormalOverrideParameter
    i.v125 = vec4<f32>(0.0, 1.0, 0.0, 0.0);                // RoughnessOverrideParameter
    i.v127 = vec4<f32>(0.0, 0.0, 0.0, 1.0);                // .w CullingSign
    i.v128 = vec4<f32>(time);
    i.v137 = vec4<f32>(0.0);                               // temporal dither offset
    i.v142 = vec4<f32>(1.0);                               // IndirectLightingColorScale
    i.v160 = vec4<f32>(1.0);                               // .w the fork's exposure scale
    i.v161 = vec4<f32>(1.0, 1.0, time, 0.0);               // eye adaptation fixed at 1
    // ---- Primitive
    i.p4 = kh.w2l[0];
    i.p5 = kh.w2l[1];
    i.p6 = kh.w2l[2];
    i.p7 = kh.w2l[3];
    i.p8 = kh.obj;
    i.p9 = vec4<f32>(kh.bounds.xyz, 1.0);                  // .w LocalToWorld determinant sign
    i.p10 = kh.actor;
    i.p15 = kh.bmin;
    i.p16 = kh.bmax;
    // ---- the indirect lighting cache: the rig's bands as given
    i.l5 = vec4<f32>(1.0);
    i.l8 = kh.sh[0];
    i.l9 = kh.sh[1];
    i.l10 = kh.sh[2];
    i.l11 = kh.sh[3];
    i.l12 = kh.sh[4];
    i.l13 = kh.sh[5];
    i.l14 = kh.sh[6];
    i.l15 = kh.sh[7];
    return i;
}

@fragment
#ifdef KH_HAIR91_SAMPLE
fn fs_main(in: VsOut, @builtin(front_facing) is_front: bool, @builtin(sample_index) sample: u32) -> ErFrag {
#else
fn fs_main(in: VsOut, @builtin(front_facing) is_front: bool) -> ErFrag {
#endif
    var frag: ErFrag;
#ifndef KH_TRANSLUCENT
    // (NF) KH3's camera fade: this pixel of him is gone (the early depth draws this shader too, so no depth, stencil, G-buffer,
    // glow or motion vector of his is left here either). The translucent slots are KH3's as well: their shaders do not read
    // CameraFade (the eyes' highlights stay).
    if (!er_camfade_keep(in.position.xy)) {
        discard;
    }
#endif
    var i = er_kh_in(in, is_front);
#ifdef KH_HAIR91_MSAA
    // (RSF2 round 14, the user: "does Sora's hair also respond to light and shadow?") the 4x hair pass draws into a tile whose
    // viewport is offset by the tile's place (hair91.rs): its pixel position is NOT the screen's, so the hybrid read the capture
    // at the wrong texels, found none of his, and lit the hair with the rig alone (no game light, no world shadow). The screen
    // position from his clip position (the game's own matrices) for every screen-space read of his.
    i.svpos = vec4<f32>((in.cur.xy / in.cur.w * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5)) * er.viewport.xy, i.svpos.zw);
#endif
    let camera = er.camera.xyz;
    let n = normalize(in.normal);
#ifdef KH_HYBRID
#ifndef KH_TRANSLUCENT
    // (IMPL_ER78) lit by the game: the rig's indirect light only at its share (the game's light carries its ambient / GI)
    if (er.hy[0].x != 0.0) {
#ifdef KH_HY3
        // (HY) the share x the game's occlusion at this pixel (its SSAO / material occlusion, our capture; none captured: 1)
        let occ = textureLoad(hy_tlight, clamp(vec2<i32>(i.svpos.xy), vec2<i32>(0), vec2<i32>(textureDimensions(hy_tlight)) - vec2<i32>(1)), 0).w;
        let w = er.hy[2].w * select(1.0, pow(clamp(occ, 0.05, 1.0), max(er.hy[2].z, 0.0)), occ > 0.0);
#else
        let w = er.hy[2].w;
#endif
        i.l8 = i.l8 * w;
        i.l9 = i.l9 * w;
        i.l10 = i.l10 * w;
        i.l11 = i.l11 * w;
        i.l12 = i.l12 * w;
        i.l13 = i.l13 * w;
        i.l14 = i.l14 * w;
        i.l15 = i.l15 * w;
    }
#endif
#endif

    // The material, the lights, the post-process material and the reflection: the shared harness.
    // (The key light: the stand-in rig of the frame constants; no shadow from Elden Ring's world.)
    let shaded = kh_shade(i, er.key_dir.xyz, er.key_color.xyz * kh.light_color.xyz, 1.0, camera);
    var alpha = shaded.color.w;
#ifdef KH_MASKED
#ifdef KH_HAIR
    // (ER87, the user: "fix Sora's hair on ALL of his costumes ... it looks buggy") THE HAIR IN TWO PASSES (kh-er-render
    // hair.rs): `shaded.cover` = how many of the mask's dither phases pass (char_common.wgsl KH_HAIR: all of them, so the
    // coverage is the one KH3's TAA resolves, with no screen-door pattern left for Elden Ring's TAA). KH_HAIR_CORE: the
    // well covered pixels, opaque (they write the depth, the early depth, the stencil, the glow mask); KH_HAIR_FRINGE: the
    // thinly covered ones (the strand ends), alpha from the coverage, drawn after his draws (at the game's velocity split)
    // with the depth test GREATER and no depth write (gfx.rs / hair.rs): a soft strand end over the hair behind it.
    var phases = 16.0;
#ifdef KH_HAIR6
    phases = 6.0;
#endif
#ifdef KH_HAIR91
    // (ER91) the continuous, supersampled coverage (char_common.wgsl kh91_cover) in 1/1024 steps
    phases = 1024.0;
#endif
    let coverage = f32(shaded.cover) / phases;
    // (the core: at least half of the phases (shots/cmp_cuts.png: the strands full, as the reference; with the core at all
    // of them the half-covered strand bodies read as smoke); below it the fringe's alpha rises to 1 at the cut, so the two
    // meet without a step. Diagnostic builds: KH_HAIR_CUT2 (0.3), KH_HAIR_CUTALL (all).)
    var cut = 0.5;
#ifdef KH_HAIR_CUT2
    cut = 0.3;
#endif
#ifdef KH_HAIR_CUTALL
    cut = 1.0 - 0.5 / phases;
#endif
#ifdef KH_HAIR91_CUT9
    cut = 0.9;
#endif
#ifdef KH_HAIR_CORE
    if (coverage < cut) {
        discard;
    }
    // (ER91, the 4x pass too: per sample, the core opaque; er91/shots/z_kh1tip2.png)
    alpha *= er.depth_uv.z;
#else
    if (coverage >= cut) {
        discard;
    }
    alpha *= saturate(coverage / cut) * er.depth_uv.z;
#endif
    frag.mask = 15u;
#else
    if (er.out_mode.w != 0.0) {
        // No multisampling in the target: the coverage of the four dither patterns becomes alpha.
        alpha *= f32(countOneBits(shaded.cover)) * 0.25 * er.depth_uv.z;
#ifdef KH_HYBRID
        // (IMPL_ER78) behind a partly covered pixel lies the game-lit white surface of his G-buffer pass: at least this opaque
        if (er.hy[0].x != 0.0) {
            alpha = max(alpha, er.hy[5].x * er.depth_uv.z);
        }
#endif
        frag.mask = 15u;
    } else {
        frag.mask = shaded.cover;
    }
#endif
#endif
    var rgb = shaded.color.xyz;
#ifdef KH_HY3
    // (RSF2 round 13) the world's shadow on him, as on the ground beside him (kh_hybrid_light: hy_world)
    if (er.hy[0].x != 0.0) {
        rgb *= hy_world;
    }
#endif
#ifdef KH_HYBRID
    // (IMPL_ER78, r.hybrid_fog) extra haze toward the game's fog colour (the game's own fog reaches him anyway, later)
    if (er.hy[0].x != 0.0 && er.hy[4].w > 0.0) {
        let f = 1.0 - exp(-length(i.twp) * 0.01 * er.hy[4].w);
        rgb = mix(rgb, er.hy[4].xyz, f);
    }
#endif
    // The rig was handed over at KH3's light level: back to the target's.
    rgb *= er.tune.xyz;
    if (er.out_mode.z == 1.0 || er.out_mode.z == 4.0) {
        // (measuring aid) A plain diffuse surface, 18 % grey or white, under the same light.
        let albedo = select(1.0, 0.18, er.out_mode.z == 1.0);
        rgb = albedo * max(er.lit_ambient.xyz + er.lit_up.xyz * n.z + er.lit_key.xyz * saturate(dot(n, normalize(er.key_dir.xyz))), vec3<f32>(0.0));
    }
    if (er.out_mode.x == 1.0) {
        rgb = er_srgb(kh_aces(rgb));
    }
    if (er.out_mode.x == 2.0) {
        rgb = kh_aces(rgb);
    }
    if (er.out_mode.z == 2.0) {
        rgb = vec3<f32>(1.0, 0.0, 1.0);
    }
    if (er.out_mode.y != 0.0) {
        rgb *= alpha;
    }
#ifdef KH_INTERIOR
    // (ER79) the cloth's inside: darkened by the slot's interior factor
    if (!is_front && kh.khs.w > 0.0) {
        rgb *= kh.khs.w;
    }
#endif
#ifdef KH_HY3
    // (HY, `r.hybrid_debug`) what the hybrid found at this pixel, at a white surface's brightness in the scene
    if (er.hy[0].x != 0.0 && hy_dbg.x >= 0.0) {
        // (view 5, the place on him: as it is, for the offline test to read back)
        frag.color = vec4<f32>(select(hy_dbg * er.tune.xyz * er.hy[3].x, hy_dbg, er.hy[5].w == 5.0), 1.0);
        return frag;
    }
#endif
    frag.color = vec4<f32>(rgb, alpha);
#ifdef KH_HAIR91_MSAA
    // (ER91) premultiplied into the hair's own 4x targets (blend ONE, INV_SRC_ALPHA); his motion vector in the game's
    // encoding (as velo_ps) and his device depth, both x coverage, so the resolve gives their coverage-weighted mean
    var encode = textureLoad(er_tgame, vec2<i32>(70, 6), 0).xy;
    if (encode.x == 0.0) {
        encode = vec2<f32>(1.0, 0.0);
    }
    let moved = (in.cur.xy / in.cur.w - in.prev.xy / in.prev.w) * vec2<f32>(0.5 * er.viewport.x / er.viewport.y, -0.5) * er.depth_uv.w;
    let velocity = moved * encode.x + vec2<f32>(encode.y);
    frag.color = vec4<f32>(select(rgb * alpha, rgb, er.out_mode.y != 0.0), alpha);
    frag.aux = vec4<f32>(velocity * alpha, in.position.z * alpha, alpha);
#ifdef KH_HAIR91_SAMPLE
    // (this invocation is its sample's only)
    frag.mask = 1u << sample;
#endif
#endif
    return frag;
}

#ifdef KH_GBUFFER
// (IMPL_ER78, the *_gb variants) HIS SURFACE IN THE GAME'S G-BUFFER, before its lights (gfx.rs, the early list): the
// game's five targets in its own packing (IMPL_ER78_STATE.md, read from its G-buffer and light shaders): 0 world normal *
// 0.5 + 0.5 (Elden Ring's world axes = the host's), 1 (metalness 0, gloss, translucency, occlusion 1), 2 WHITE + the light
// set / 255 (the game's light at his pixels is then the light he receives, untouched by any colour). (IMPL_ER78b) 3
// (subsurface) and 4 (emissive) are not written: three targets only. Masked slots: the pixels the early depth took (any of the four dither patterns).
struct ErGbuf {
    @location(0) normal: vec4<f32>,
    @location(1) params: vec4<f32>,
    @location(2) albedo: vec4<f32>,
}

@fragment
fn gb_main(in: VsOut, @builtin(front_facing) is_front: bool) -> ErGbuf {
    // (NF) the camera fade's pixels are not his in the G-buffer either (the game's own surface stays under them)
    if (!er_camfade_keep(in.position.xy)) {
        discard;
    }
    var i = er_kh_in(in, is_front);
#ifdef KH_MASKED
#ifdef KH_HAIR91
    // (ER91, *_gb2_h91 / *_gb3_h91) the hair's G-buffer pixels = its colour core's (the continuous coverage at least 1/2), so
    // the game-lit white surface never shows beside the strands
    if (kh91_cover(i) < 0.5) {
        discard;
    }
#else
#ifdef KH_HAIR
    // (HY, the hair path's G-buffer writer `*_gb3_hair`) exactly the pixels of the hair's CORE (er_char.wgsl fs_main
    // KH_HAIR_CORE: at least half of the phases, char_common.wgsl kh_shade's count): the soft fringe gets no G-buffer (it is
    // drawn alpha-blended over what lies behind it; its light comes from the core beside it, hy_tap)
    var count = 0u;
    var phases = 16.0;
#ifdef KH_HAIR6
    phases = 6.0;
    let x0 = i.svpos.x;
    i.v137 = vec4<f32>(0.0);
    for (var k = 0u; k < 6u; k++) {
        i.svpos.x = x0 + f32(k);
        if (kh_mask(i) != 0.0) {
            count += 1u;
        }
    }
    i.svpos.x = x0;
#else
    for (var k = 0u; k < 16u; k++) {
        i.v137 = vec4<f32>(0.0, 1.0, (f32(k) + 0.5) / 16.0 * (5.0 / 6.0) - 1.0 / 12.0, 0.0);
        if (kh_mask(i) != 0.0) {
            count += 1u;
        }
    }
#endif
    if (f32(count) / phases < 0.5) {
        discard;
    }
#else
    var cover = 0u;
    for (var k = 0u; k < 4u; k++) {
        i.v137 = vec4<f32>(f32(k));
        if (kh_mask(i) != 0.0) {
            cover |= 1u << k;
        }
    }
    if (cover == 0u) {
        discard;
    }
#endif
#endif
    i.v137 = vec4<f32>(0.0);
#endif
    let g = kh_material(i);
    // (IMPL_ER78b) a degenerate normal (zero length, NaN) is written as straight up, never as NaN
    let n_raw = (er.host_from_game * vec4<f32>(g.o1.xyz * 2.0 - vec3<f32>(1.0), 0.0)).xyz;
    let n_len = length(n_raw);
    let n_world = select(vec3<f32>(0.0, 1.0, 0.0), n_raw / max(n_len, 1.0e-6), n_len > 1.0e-6 && n_len < 1.0e6);
    var o: ErGbuf;
    o.normal = vec4<f32>(n_world * 0.5 + vec3<f32>(0.5), 0.0);
    o.params = vec4<f32>(0.0, er.hy[5].z, er.hy[3].z, 1.0);
    o.albedo = vec4<f32>(1.0, 1.0, 1.0, er.hy[3].w);
    return o;
}
#endif
#endif

#ifdef ER_HYCAP
// (HY, hycap.ps.dxil) THE CAPTURE OF THE GAME'S LIGHT for the hybrid: drawn in his colour list right after the game's deferred
// stage, over the box his G-buffer pass covered (scissor), into two targets of OUR OWN (gfx.rs `HyCapture`): every shader of his
// reads these, never the game's targets, so a draw later in the frame (the hair fringe at the velocity split) finds the same
// values, and nothing of ours touches the game's resources after this point. t0 the game's diffuse light accumulation (by
// now x the G-buffer's albedo x (1 - metalness): GXLightCmbAlbedo), t1 its screen-space shadow mask (r, g occlusion; a the sun's
// visibility; white when the frame had none), t2 G-buffer 0 (world normal * 0.5 + 0.5), t3 G-buffer 1 (metalness, gloss,
// translucency, occlusion), t4 G-buffer 2 through an sRGB view (albedo).
// Out 0: the light a WHITE surface got there (the albedo taken out again: his pass wrote white, but a decal or the rain may
// have changed it since), SCENE units, finite; a = the occlusion (0.05..1). Out 1: the unit normal (host axes); a = the sun's
// visibility. A texel whose surface colour is too dark to divide by, or whose normal is not a normal: zero (= not captured).
@group(1) @binding(0) var hc_light: texture_2d<f32>;
@group(1) @binding(1) var hc_mask: texture_2d<f32>;
@group(1) @binding(2) var hc_gb0: texture_2d<f32>;
@group(1) @binding(3) var hc_gb1: texture_2d<f32>;
@group(1) @binding(4) var hc_gb2: texture_2d<f32>;
// (RSF2 round 12) the scene depth WITH the KH characters in it (our copy at the end of the early list), the game's sun cascade
// atlas (16-bit, reversed: the nearer to the light the larger), the cascades' constants (row r = the cbSceneParam of the game's
// caster pass of cascade r: texel 3 SC_CameraPos, texels 24..27 VC_MatrixViewProj; gfx.rs `read_cascades`)
@group(1) @binding(5) var hc_depth: texture_2d<f32>;
@group(1) @binding(6) var hc_atlas: texture_2d<f32>;
@group(1) @binding(7) var hc_casc: texture_2d<f32>;
// (RSF2 round 15) THE KH CASTERS, one depth map each (slice 0 Sora, 1..3 the party in plan order), drawn with the light matrices of
// the finest cascade (gfx.rs draw_cast, r.shadow_real): which of the atlas's occluders is a KH character's own body
@group(1) @binding(8) var hc_kh_maps: texture_2d_array<f32>;
// (RSF round 19) THE LIGHT PROBES OF THE KH SURFACES (gfx.rs `HyProbe`): an array of 6 slices (one per direction, +x -x +y -y +z -z
// of the host's axes), level 0 a texel per 8 x 8 px tile, 4 levels more each a 2 x 2 average; a texel = (sum of the light / its
// occlusion x the direction's weight over the tile's KH texels, sum of the weights) / the tile's texel count. A null view when
// the files are not in the pack (reads 0: hycap keeps hc_blur).
@group(1) @binding(9) var hc_probe: texture_2d_array<f32>;

struct HyCapOut {
    @location(0) light: vec4<f32>,
    @location(1) geo: vec4<f32>,
}

// (RSF2 round 11, hygb.ps.dxil) THE REST OF HIS G-BUFFER. The G-buffer pass writes targets 0..2 only (IMPL_ER78b); targets 3
// (subsurface: r strength, g the profile id) and 4 (emissive) kept WHAT LIES BEHIND HIM at his pixels. The game's subsurface
// blur (GXSssBlurX / Y: g_SSSTexture t5, after its light passes, before the composite, into the very light accumulation the
// capture reads) then treated his pixels as the background's material: foliage (the Erdtree, a tree, grass) and whatever else
// behind him carries a subsurface profile got its blur, with the background's width and profile, over his light, which the
// capture took. Drawn right after the G-buffer pass over its box: where target 2 holds the KH characters' white (hc_kh), 3 and
// 4 become (0, 0) / (0, 0, 0, 0): no subsurface, no emissive, as any plain surface of the game. Every KH character alike.
struct HyGbOut {
    @location(0) sss: vec4<f32>,
    @location(1) emissive: vec4<f32>,
}

// (RSF2 round 5, the user: "Things are STILL showing through characters!") THE GAME'S SCREEN-SPACE MASK IS NOT HIS. Every
// light shader of the game multiplies its diffuse light by min(mask.r, mask.g) (its screen-space occlusion) and the sun's by
// mask.a (its screen-space cascaded shadow): GXLightAcc_*, GXRend_TileBasedLighting, GXLightDeferredAO (er78/dis). That mask
// is a SCREEN-SPACE term: where it is made from the world without him (a depth copy, a half-size buffer, a history), his pixels
// carry the occlusion and the sun patches of what lies BEHIND him: the chapel window with its bars printed on Goofy's snout
// (fix6/ref). Everything else the capture holds is worked out per pixel from HIS G-buffer (his normal, his depth, white).
// So in the capture the mask's two terms are taken out again and put back as ONE value for all the KH characters on screen:
// the mean of the mask over the KH pixels (the white the G-buffer pass writes) of the capture's box, or over the whole box
// when fewer than 4 of its grid points are KH (`hc_mean`, a fixed 12 x 12 grid, the same for every texel), so that a pattern
// of the background can no longer land on any of them (Sora in any form, the friends, their weapons and shields alike). The light: L / ao - the
// sun's part with the mask's visibility (its light per N.L, er.hy[7], the rig's calibrated sun) + the sun's part with the mean
// visibility, x the mean occlusion. `r.hybrid_mask` (er.hy[8].z, default 1; 0 = the capture as before). The box: er.hy[9]
// (gfx.rs writes it with the capture). Without a KH texel on the grid: as before.
fn hc_kh(albedo: vec3<f32>) -> bool {
    return min(albedo.x, min(albedo.y, albedo.z)) > 0.996;
}

// (RSF2 round 10, the user's r.hybrid_debug=2 frame at the Erdtree: the light the game gave Sora is white with HARD BLACK
// SHAPES, the castle wall's crenellations and arches behind him and branch lines on his chest; r.hybrid_gloss 0, r.taa_char 0
// and r.hybrid_mask 0 change nothing.) The game's light passes work part of their light out at our pixels from something that
// does not hold the KH characters (a depth or a screen-space buffer made without them): its shadows and cut-offs fall on them
// where the BACKGROUND has them. Where exactly cannot be read offline, so the route holds whatever the cause:
// THE PRINTED DARK SHAPES ARE CLOSED. A KH texel's light = the mean of the BRIGHT captured lights (at least 0.6 of the
// brightest) among the KH texels of an 8 x 8 grid around it (half a side 8 % of the picture's height) whose normal faces the same way (cos >= 0.9), itself included: a surface
// facing the same way a few centimetres away gets the same light from the sun, the sky and a torch, so a black cut-out smaller
// than the grid (a crenellation, a window bar, a branch line) takes its neighbours' light; the shading over his form (normals)
// stays. `r.hybrid_fit` = 1 (er.hy[8].w; the default until round 11, now 2: hc_wide); 0 = the per-pixel capture as round 7.
const HC_FIT_N: i32 = 8;

fn hc_close(t: vec2<i32>, n_t: vec3<f32>, own: vec3<f32>) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(hc_gb2));
    let reach = 0.08 * f32(size.y);
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
    var ls: array<vec3<f32>, 65>;
    var count = 1;
    ls[0] = own;
    var best_l = dot(own, luma);
    for (var j = 0; j < HC_FIT_N; j++) {
        for (var i = 0; i < HC_FIT_N; i++) {
            let f = (vec2<f32>(f32(i), f32(j)) + vec2<f32>(0.5)) / f32(HC_FIT_N) * 2.0 - vec2<f32>(1.0);
            let p = clamp(t + vec2<i32>(f * reach), vec2<i32>(0), size - vec2<i32>(1));
            if (!hc_kh(textureLoad(hc_gb2, p, 0).xyz)) {
                continue;
            }
            let nn = textureLoad(hc_gb0, p, 0).xyz * 2.0 - vec3<f32>(1.0);
            let ln = length(nn);
            if (!(ln > 0.5 && ln < 1.5) || dot(nn / ln, n_t) < 0.9) {
                continue;
            }
            let l = min(max(textureLoad(hc_light, p, 0).xyz, vec3<f32>(0.0)), vec3<f32>(6.0e4));
            ls[count] = l;
            count += 1;
            best_l = max(best_l, dot(l, luma));
        }
    }
    // (the mean of the bright ones, not the single brightest: the game's light has a fine noise of its own)
    var sum = vec3<f32>(0.0);
    var n = 0.0;
    for (var k = 0; k < 65; k++) {
        if (k >= count) {
            break;
        }
        if (dot(ls[k], luma) >= 0.6 * best_l) {
            sum += ls[k];
            n += 1.0;
        }
    }
    return sum / max(n, 1.0);
}

// (RSF2 round 11, in game at the user's tree spot: r.hybrid_debug=2 with round 10's fill showed blotches all over the KH
// characters, and without it the light still carried the tree's branch lines and the grass; r.hybrid_debug=4 with the mask
// rule off showed the game's per-pixel sun visibility full of the wall's blocks: every SCREEN-SPACE term of the game's light at
// our pixels is made partly from what lies behind us.) THE WIDE FILL: of the game's light at a KH texel only what does not
// depend on the screen is kept per pixel: the sun's part (its light per N.L, er.hy[7]) with the mean visibility, x the mean
// occlusion (round 5). The REST (sky, bounce, every local light) is the mean of the rest over the KH texels of a FIXED 12 x 12
// grid over the capture's box (er.hy[9], the same samples for every texel: a grid that moved with the texel made blotches from
// the samples' own spread, in game), each weighted by how far its normal faces the same way (cos^4): the light on them follows
// their normals and the light of the place, and no shape of the background lands on them; each sample's own sun part (its own
// visibility) is taken out with its own occlusion first, so the sun is not counted twice. Without a KH sample: today's
// per-pixel rest (round 5).
// `r.hybrid_fit` 2 (er.hy[8].w, the default); 1 = round 10's close; 0 = the per-pixel capture with the round-5 means.

// (RSF2 round 13, the user: "Sora's lighting is VERY flickery") THE SAMPLES ARE A LATTICE FIXED ON THE SCREEN AROUND THE TEXEL.
// Round 11 spread its 12 x 12 grid over the capture's box, which covers the whole party and moves every frame: with the party in
// view the grid was ~170 px apart, a handful of points fell on Sora and WHICH ones changed with his idle sway, so the means jumped
// (in game, rsf13: the light view pumping 7 - 12 grey levels a frame against 0.3 per pixel). Now: absolute screen points every
// HC step (1/45 of the picture's height) within HC_R steps of the texel, each weighted by a tent that falls to 0 at the
// window's edge: a texel that moves keeps the same points with weights that change smoothly, and a still character gets the
// same sums every frame.
const HC_R: i32 = 4;
fn hc_step() -> f32 {
    return max(8.0, round(f32(textureDimensions(hc_gb2).y) / 32.0));
}

// (RSF2 round 13) a sample's sun visibility, ONE tap of the finest cascade (row 0: the smallest viewport, hybrid.rs
// cascade_passes); the matrix rows are the caller's (loaded once per texel). < 0 = not held by it / no depth.
fn hc_vis_tap(p: vec2<i32>, n_host: vec3<f32>, cam: vec3<f32>, m0: vec4<f32>, m1: vec4<f32>, m2: vec4<f32>, m3: vec4<f32>) -> f32 {
    let device_z = textureLoad(hc_depth, p, 0).x;
    if (!(device_z > 0.0)) {
        return -1.0;
    }
    let uv = (vec2<f32>(p) + vec2<f32>(0.5)) * er.viewport.zw;
    let view_z = 1.0 / (device_z * er.depth.z + er.depth.w);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let v = vec3<f32>(ndc.x * er.proj.x, ndc.y * er.proj.y, 1.0) * (view_z * 100.0);
    let q = er.camera.xyz + er.cam_right.xyz * v.x + er.cam_up.xyz * v.y + er.cam_forward.xyz * v.z;
    let rel = vec4<f32>((er.host_from_game * vec4<f32>(q, 1.0)).xyz + n_host * er.hy[10].w - cam, 1.0);
    let c = vec4<f32>(dot(m0, rel), dot(m1, rel), dot(m2, rel), dot(m3, rel));
    if (!(c.w > 1.0e-6)) {
        return -1.0;
    }
    let sc = c.xyz / c.w;
    if (abs(sc.x) > 0.97 || abs(sc.y) > 0.97 || sc.z <= 0.0 || sc.z >= 1.0) {
        return -1.0;
    }
    let vp = er.hy[11 + i32(er.hy[19].w + 0.5)];
    let maps_on = er.hy[19].x > 0.5;
    let at = vp.xy + vec2<f32>(sc.x * 0.5 + 0.5, 0.5 - sc.y * 0.5) * vp.zw;
    return hc_tap_kh(at, sc.z, er.hy[10].z / 65535.0, select(-1, hc_owner_maps(at, sc.z), maps_on && er.hy[19].x < 1.5), maps_on);
}

fn hc_sun(n: vec3<f32>) -> vec3<f32> {
    let to_sun = (er.host_from_game * vec4<f32>(er.hy[6].xyz, 0.0)).xyz;
    if (dot(to_sun, to_sun) <= 0.25) {
        return vec3<f32>(0.0);
    }
    return max(er.hy[7].xyz, vec3<f32>(0.0)) * max(dot(n, normalize(to_sun)), 0.0);
}

// (RSF2 round 12/13) THE REST OF THE PLACE, THE SUN'S REAL STRENGTH, THE OCCLUSION, over the local lattice (hc_step): the game
// shaded each KH texel with its own (printed) mask, y = l / ao = R0 + R1 up + k (model sun x N.L x mask.a); a weighted least-squares
// fit of the samples gives k, how many times our model of the sun (er.hy[7]) the game's sun on them really is. k is trusted only
// as far as the fit is determined (the spread of the sun term among the samples); with little spread (overcast, fog, all lit or all
// shaded) it falls back smoothly to 1 (r.hybrid_sun_fit 0: always 1). The rest = the normal-weighted mean of y less k x the
// samples' own sun parts. The occlusion and the game's visibility: their tent-weighted means (the visibility is used only where
// no cascade holds the point).
struct HcWide {
    rest: vec3<f32>,
    k: f32,
    ao: f32,
    vis: f32,
    ok: bool,
}

fn hc_wide(t: vec2<i32>, n_t: vec3<f32>) -> HcWide {
    var o: HcWide;
    o.ok = false;
    o.k = 1.0;
    o.rest = vec3<f32>(0.0);
    o.ao = 1.0;
    o.vis = 1.0;
    let size = vec2<i32>(textureDimensions(hc_gb2));
    let msize = vec2<i32>(textureDimensions(hc_mask));
    let step = hc_step();
    let reach = f32(HC_R) * step;
    let base = floor((vec2<f32>(t) + vec2<f32>(0.5)) / step);
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
    let sun_l = dot(max(er.hy[7].xyz, vec3<f32>(0.0)), luma);
    var sum_y = vec3<f32>(0.0);
    var sum_x = 0.0;
    var wsum = 0.0;
    var ao_sum = 0.0;
    var vis_sum = 0.0;
    var tsum = 0.0;
    // (the weighted normal equations of y = b0 + b1 up + b2 x)
    var a00 = 0.0; var a01 = 0.0; var a02 = 0.0; var a11 = 0.0; var a12 = 0.0; var a22 = 0.0;
    var r0 = 0.0; var r1 = 0.0; var r2 = 0.0;
    let casc = er.hy[10].x > 0.5;
    // (round 15) the cascade of the KH caster maps when they are on (row 0 otherwise)
    let rr = i32(er.hy[19].w + 0.5);
    let cam0 = textureLoad(hc_casc, vec2<i32>(3, rr), 0).xyz;
    let m0 = textureLoad(hc_casc, vec2<i32>(24, rr), 0);
    let m1 = textureLoad(hc_casc, vec2<i32>(25, rr), 0);
    let m2 = textureLoad(hc_casc, vec2<i32>(26, rr), 0);
    let m3 = textureLoad(hc_casc, vec2<i32>(27, rr), 0);
    for (var j = -HC_R; j <= HC_R; j++) {
        for (var i = -HC_R; i <= HC_R; i++) {
            let pc = (base + vec2<f32>(f32(i), f32(j)) + vec2<f32>(0.5)) * step;
            let d = abs(pc - (vec2<f32>(t) + vec2<f32>(0.5))) / reach;
            let tent = max(1.0 - d.x, 0.0) * max(1.0 - d.y, 0.0);
            if (tent <= 0.0) {
                continue;
            }
            let p = vec2<i32>(pc);
            if (p.x < 0 || p.y < 0 || p.x >= size.x || p.y >= size.y) {
                continue;
            }
            if (!hc_kh(textureLoad(hc_gb2, p, 0).xyz)) {
                continue;
            }
            let nn = textureLoad(hc_gb0, p, 0).xyz * 2.0 - vec3<f32>(1.0);
            let ln = length(nn);
            if (!(ln > 0.5 && ln < 1.5)) {
                continue;
            }
            let ns = nn / ln;
            let m = textureLoad(hc_mask, min(p, msize - vec2<i32>(1)), 0);
            let ao_s = clamp(min(m.x, m.y), 0.05, 1.0);
            let l = min(max(textureLoad(hc_light, p, 0).xyz, vec3<f32>(0.0)), vec3<f32>(6.0e4));
            let y = l / ao_s;
            // (the sample's sun part per unit k: the model's N.L x its visibility: the finest cascade's at its own place, else the
            // game's mask.a)
            var vs = clamp(m.w, 0.0, 1.0);
            if (casc) {
                let vt = hc_vis_tap(p, ns, cam0, m0, m1, m2, m3);
                if (vt >= 0.0) {
                    vs = vt;
                }
            }
            let xs = dot(hc_sun(ns), luma) / max(sun_l, 1.0e-9) * vs;
            let c = max(dot(ns, n_t), 0.0);
            let w = (c * c * c * c + 0.02) * tent;
            sum_y += y * w;
            sum_x += xs * w;
            wsum += w;
            ao_sum += ao_s * tent;
            vis_sum += vs * tent;
            tsum += tent;
            let yl = dot(y, luma);
            // (host axes: x is up, math.rs host_from_game)
            let up = ns.x;
            let x = xs * sun_l;
            a00 += tent; a01 += up * tent; a02 += x * tent; a11 += up * up * tent; a12 += up * x * tent; a22 += x * x * tent;
            r0 += yl * tent; r1 += up * yl * tent; r2 += x * yl * tent;
        }
    }
    if (wsum <= 0.0 || tsum < 0.5) {
        return o;
    }
    o.ok = true;
    o.ao = ao_sum / tsum;
    o.vis = vis_sum / tsum;
    if (er.hy[16].x > 0.5 && sun_l > 0.0 && a00 > 4.0) {
        let det = a00 * (a11 * a22 - a12 * a12) - a01 * (a01 * a22 - a12 * a02) + a02 * (a01 * a12 - a11 * a02);
        let scale = a00 * a11 * a22;
        if (abs(det) > 1.0e-6 * max(scale, 1.0e-20)) {
            let d2 = a00 * (a11 * r2 - r1 * a12) - a01 * (a01 * r2 - r1 * a02) + r0 * (a01 * a12 - a11 * a02);
            let k_fit = clamp(d2 / det, er.hy[16].y, er.hy[16].z);
            // (how well the fit is determined: the sun term's spread among the samples against the sun's own size)
            let mx = a02 / a00;
            let spread = sqrt(max(a22 / a00 - mx * mx, 0.0)) / max(sun_l, 1.0e-9);
            let trust = smoothstep(0.08, 0.25, spread) * smoothstep(4.0, 24.0, a00);
            o.k = mix(1.0, k_fit, trust);
        }
    }
    o.rest = max(sum_y / wsum - max(er.hy[7].xyz, vec3<f32>(0.0)) * (o.k * sum_x / wsum), vec3<f32>(0.0));
    return o;
}

// (RSF2 round 13, fit 3: "the shadow on him as dark as on the ground beside him") THE SHADE RATIO AROUND THE TEXEL: over the
// local lattice, the game's light in the cascades' shadow over the game's light in the sun, for the KH texels facing the same
// way (cos >= 0.8) and, as a second source, for the ground (world texels facing up, their albedo taken out). Every sample's
// visibility is the finest cascade's at its own place (hc_vis_tap). Returns (ratio, trust 0..1): trust grows with the number of
// samples in each class; no class pair = trust 0. Returns (KH ratio, its trust, ground ratio, its trust): in game (rsf13) the
// game's own light on the KH G-buffer keeps far less of the sun's contrast (lit / shade 1.3 - 1.8) than the ground (about 6).
fn hc_shade_ratio(t: vec2<i32>, n_t: vec3<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(hc_gb2));
    let msize = vec2<i32>(textureDimensions(hc_mask));
    if (er.hy[10].x < 0.5) {
        return vec4<f32>(1.0, 0.0, 1.0, 0.0);
    }
    let step = hc_step();
    let reach = f32(HC_R) * step;
    let base = floor((vec2<f32>(t) + vec2<f32>(0.5)) / step);
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
    // (round 15) the cascade of the KH caster maps when they are on (row 0 otherwise)
    let rr = i32(er.hy[19].w + 0.5);
    let cam0 = textureLoad(hc_casc, vec2<i32>(3, rr), 0).xyz;
    let m0 = textureLoad(hc_casc, vec2<i32>(24, rr), 0);
    let m1 = textureLoad(hc_casc, vec2<i32>(25, rr), 0);
    let m2 = textureLoad(hc_casc, vec2<i32>(26, rr), 0);
    let m3 = textureLoad(hc_casc, vec2<i32>(27, rr), 0);
    // (KH lit sum, weight, KH shade sum, weight), the same for the ground
    var kh = vec4<f32>(0.0);
    var gr = vec4<f32>(0.0);
    for (var j = -HC_R; j <= HC_R; j++) {
        for (var i = -HC_R; i <= HC_R; i++) {
            let pc = (base + vec2<f32>(f32(i), f32(j)) + vec2<f32>(0.5)) * step;
            let d = abs(pc - (vec2<f32>(t) + vec2<f32>(0.5))) / reach;
            let tent = max(1.0 - d.x, 0.0) * max(1.0 - d.y, 0.0);
            if (tent <= 0.0) {
                continue;
            }
            let p = vec2<i32>(pc);
            if (p.x < 0 || p.y < 0 || p.x >= size.x || p.y >= size.y) {
                continue;
            }
            let nn = textureLoad(hc_gb0, p, 0).xyz * 2.0 - vec3<f32>(1.0);
            let ln = length(nn);
            if (!(ln > 0.5 && ln < 1.5)) {
                continue;
            }
            let ns = nn / ln;
            let alb = textureLoad(hc_gb2, p, 0).xyz;
            let is_kh = hc_kh(alb);
            // (KH texels: facing as this one; the ground: facing up (the G-buffer's normals: y up))
            if (is_kh && dot(ns, n_t) < 0.8) {
                continue;
            }
            if (!is_kh && (ns.y < 0.7 || dot(alb, luma) < 0.05)) {
                continue;
            }
            let vt = hc_vis_tap(p, ns, cam0, m0, m1, m2, m3);
            if (vt < 0.0) {
                continue;
            }
            let m = textureLoad(hc_mask, min(p, msize - vec2<i32>(1)), 0);
            let ao_s = clamp(min(m.x, m.y), 0.05, 1.0);
            let pr = textureLoad(hc_gb1, p, 0);
            let a = select(max(alb * (1.0 - clamp(pr.x, 0.0, 1.0)), vec3<f32>(0.03)), vec3<f32>(1.0), is_kh);
            let y = dot(min(max(textureLoad(hc_light, p, 0).xyz, vec3<f32>(0.0)), vec3<f32>(6.0e4)) / a, luma) / ao_s;
            let add = select(vec4<f32>(0.0, 0.0, y * tent, tent), vec4<f32>(y * tent, tent, 0.0, 0.0), vt > 0.5);
            if (is_kh) {
                kh += add;
            } else {
                gr += add;
            }
        }
    }
    let tk = smoothstep(0.5, 2.0, min(kh.y, kh.w));
    let tg = smoothstep(0.5, 2.0, min(gr.y, gr.w));
    let rk = select(1.0, (kh.z / max(kh.w, 1.0e-6)) / max(kh.x / max(kh.y, 1.0e-6), 1.0e-9), tk > 0.0);
    let rg = select(1.0, (gr.z / max(gr.w, 1.0e-6)) / max(gr.x / max(gr.y, 1.0e-6), 1.0e-9), tg > 0.0);
    return vec4<f32>(clamp(rk, 0.03, 1.0), tk, clamp(rg, 0.03, 1.0), tg);
}

// (RSF2 round 14, the user: "you could see in the picture there was fire visible thru Sora") THE GAME'S HALF-SIZE DEPTH FOR ITS
// EFFECTS. Elden Ring draws its fire, embers and glows at half size, depth-tested against a half-size depth it rebuilds in its
// forward stage (rsf11 capture: the list before the half-size effect passes). Drawn right after that list (hook.rs
// effects_split): the scene depth (which holds the KH characters since the G-buffer stage) taken 2 x 2 at a time, the NEAREST of
// the four (reversed: the largest), written where it is nearer than what the half-size depth holds (GREATER). The world's own
// depth is already there (no change); the KH characters' is added: an effect behind any of them is hidden, one in front draws
// over them. hdfill_ps (r.effects_depth = 2, a test): the nearest depth everywhere (every half-size effect hidden).
struct HdOut {
    @builtin(frag_depth) depth: f32,
}

@fragment
fn hdmerge_ps(@builtin(position) position: vec4<f32>) -> HdOut {
    let size = vec2<i32>(textureDimensions(hc_depth));
    let t = vec2<i32>(position.xy) * 2;
    var d = 0.0;
    for (var j = 0; j < 2; j++) {
        for (var i = 0; i < 2; i++) {
            d = max(d, textureLoad(hc_depth, clamp(t + vec2<i32>(i, j), vec2<i32>(0), size - vec2<i32>(1)), 0).x);
        }
    }
    if (!(d > 0.0)) {
        discard;
    }
    var o: HdOut;
    o.depth = min(d, 1.0);
    return o;
}

@fragment
fn hdfill_ps(@builtin(position) position: vec4<f32>) -> HdOut {
    var o: HdOut;
    o.depth = 1.0;
    return o;
}

// (RSF2 round 16, the user: "you can see here that they are still seethrough") THE GAME'S LIGHT AT A KH TEXEL, BLURRED OVER HIS
// OWN SURFACE: a 9 x 9 grid of offsets RELATIVE to the texel (they move with him: a body that moves or sways keeps the same
// samples, so the result is as steady as the per-pixel light), spacing 1/90 of the picture's height (radius 4 steps), each KH
// texel facing the same way (cos^8) weighted by a gaussian of its distance; y = light / occlusion of each sample. What the game's
// screen-space and ray-traced terms print from the background (rock, leaves: tens of pixels) averages out; the shading over his
// form (the normals) and our own sun shadow (the world factor, from the cascades) stay. Returns (y, weight).
// (RSF round 19, the user: "lighting and even some detail behind Donald is appearing thru him like he's a ghost", a close camera,
// Donald filling the screen in front of candles and pillars) THE BLUR'S REACH IS ON HIS BODY, NOT ON THE SCREEN. The game's
// screen-space and ray-traced terms print the background's shapes at THEIR size on the screen (a candle's column, a pillar, an
// arch: 100 - 300 px), whatever the size of the KH character in front; a grid of 1/90 of the picture's height (16 px at 1440,
// reach 64 px) averaged them away on a character at play distance but not on one filling the screen. The step is now also at
// least 5 cm of the character's own surface at this texel's depth (reach 4 steps: 20 cm), at most 1/10 of the picture: close up
// the grid grows with him and every printed shape is smaller than it; at play distance the screen step stays (round 16, tested in
// game). The same rule for every KH character (Sora, the friends, every copy drawn through this pipeline: no names).
fn hc_view_m(p: vec2<i32>) -> f32 {
    let device_z = textureLoad(hc_depth, p, 0).x;
    if (!(device_z > 0.0)) {
        return -1.0;
    }
    return 1.0 / (device_z * er.depth.z + er.depth.w);
}

const HC_BLUR_STEP_M: f32 = 0.15;
const HC_BLUR_SPREAD: f32 = 48.0;

fn hc_world_step(view_m: f32, size: vec2<i32>) -> f32 {
    let screen = max(4.0, f32(size.y) / 90.0);
    if (!(view_m > 0.01) || !(er.proj.y > 1.0e-4)) {
        // (no depth here: the KH characters' capture box instead, a 30th of its height: a character filling the screen gets the
        // large grid all the same)
        let box_h = max(er.hy[9].w - er.hy[9].y, 0.0);
        return clamp(box_h / 30.0, screen, f32(size.y) / 8.0);
    }
    // pixels per metre at this depth: half the picture's height over tan(fov / 2) x the distance
    let per_m = 0.5 * f32(size.y) / (er.proj.y * view_m);
    return clamp(HC_BLUR_STEP_M * per_m, screen, f32(size.y) / 6.0);
}

// (RSF round 19) THE GAME'S LIGHT ON THE KH SURFACES, PREFILTERED (the user: "lighting and even some detail behind Donald is appearing
// thru him like he's a ghost"; then "fix it for EVERY character"). A grid of single samples (hc_blur) leaves the background's printed
// shapes on a character that fills the screen: the shapes are larger than its reach, and a sparse grid reach would alias with them.
// The probes average EVERY KH texel of a tile (no gaps), per direction, and are averaged again level by level, so a read at a level
// whose tile is a third of the reach covers the whole reach without a gap: a printed candle, pillar or arch of any size up to the
// reach is averaged away, the form's shading (the six directions) stays. The reach: 30 cm of the character's own surface at this
// texel's depth (from 64 px at the least to half the picture's height), so it is the same on Sora, Donald and Goofy at any distance.
const HC_TILE: f32 = 8.0;
const HC_PROBE_LEVELS: i32 = 5;
const HC_PROBE_REACH_M: f32 = 0.30;

fn hc_dir_w(n: vec3<f32>, b: i32) -> f32 {
    let a = select(max(-n, vec3<f32>(0.0)), max(n, vec3<f32>(0.0)), (b & 1) == 0);
    let c = select(select(a.z, a.y, b < 4), a.x, b < 2);
    return c * c;
}

struct HcProbeOut {
    @location(0) p0: vec4<f32>,
    @location(1) p1: vec4<f32>,
    @location(2) p2: vec4<f32>,
    @location(3) p3: vec4<f32>,
    @location(4) p4: vec4<f32>,
    @location(5) p5: vec4<f32>,
}

fn hc_probe_out(s: array<vec4<f32>, 6>) -> HcProbeOut {
    var o: HcProbeOut;
    o.p0 = s[0];
    o.p1 = s[1];
    o.p2 = s[2];
    o.p3 = s[3];
    o.p4 = s[4];
    o.p5 = s[5];
    return o;
}

// level 0: a tile of 8 x 8 texels of the capture's inputs
@fragment
fn hcprobe_ps(@builtin(position) position: vec4<f32>) -> HcProbeOut {
    let size = vec2<i32>(textureDimensions(hc_gb2));
    let msize = vec2<i32>(textureDimensions(hc_mask));
    let base = vec2<i32>(position.xy) * i32(HC_TILE);
    var s: array<vec4<f32>, 6>;
    for (var k = 0; k < 6; k++) {
        s[k] = vec4<f32>(0.0);
    }
    for (var j = 0; j < i32(HC_TILE); j++) {
        for (var i = 0; i < i32(HC_TILE); i++) {
            let p = base + vec2<i32>(i, j);
            if (p.x >= size.x || p.y >= size.y || !hc_kh(textureLoad(hc_gb2, p, 0).xyz)) {
                continue;
            }
            let nn = textureLoad(hc_gb0, p, 0).xyz * 2.0 - vec3<f32>(1.0);
            let ln = length(nn);
            if (!(ln > 0.5 && ln < 1.5)) {
                continue;
            }
            let n = nn / ln;
            let m = textureLoad(hc_mask, min(p, msize - vec2<i32>(1)), 0);
            let ao_s = clamp(min(m.x, m.y), 0.05, 1.0);
            let l = min(max(textureLoad(hc_light, p, 0).xyz, vec3<f32>(0.0)), vec3<f32>(6.0e4)) / ao_s;
            for (var k = 0; k < 6; k++) {
                let w = hc_dir_w(n, k);
                s[k] += vec4<f32>(l * w, w);
            }
        }
    }
    let inv = 1.0 / (HC_TILE * HC_TILE);
    for (var k = 0; k < 6; k++) {
        s[k] *= inv;
    }
    return hc_probe_out(s);
}

// a level from the one below (bound alone at t9): the 2 x 2 average
@fragment
fn hcprobedown_ps(@builtin(position) position: vec4<f32>) -> HcProbeOut {
    let below = vec2<i32>(textureDimensions(hc_probe, 0));
    let t = vec2<i32>(position.xy) * 2;
    var s: array<vec4<f32>, 6>;
    for (var k = 0; k < 6; k++) {
        var a = vec4<f32>(0.0);
        for (var q = 0; q < 4; q++) {
            let p = min(t + vec2<i32>(q & 1, q >> 1), below - vec2<i32>(1));
            a += textureLoad(hc_probe, p, k, 0);
        }
        s[k] = a * 0.25;
    }
    return hc_probe_out(s);
}

// (RSF round 19) the light at texel t facing n_t from the probes: a 7 x 7 tent of tiles at the level whose tile is a third of the
// reach; (light, weight) (weight 0: no probes bound or no KH texel near)
fn hc_probe_light(t: vec2<i32>, n_t: vec3<f32>, z_t: f32) -> vec4<f32> {
    let d0 = vec2<i32>(textureDimensions(hc_probe, 0));
    if (d0.x < 2 || d0.y < 2) {
        return vec4<f32>(0.0);
    }
    let size = vec2<i32>(textureDimensions(hc_gb2));
    var reach = max(er.hy[9].w - er.hy[9].y, 0.0) / 4.0;
    if (z_t > 0.01 && er.proj.y > 1.0e-4) {
        reach = HC_PROBE_REACH_M * 0.5 * f32(size.y) / (er.proj.y * z_t);
    }
    reach = clamp(reach, 64.0, 0.5 * f32(size.y));
    let level = clamp(i32(round(log2(reach / (3.0 * HC_TILE)))), 0, HC_PROBE_LEVELS - 1);
    let tile = HC_TILE * exp2(f32(level));
    let dl = vec2<i32>(textureDimensions(hc_probe, level));
    let c = (vec2<f32>(t) + vec2<f32>(0.5)) / tile;
    let ci = vec2<i32>(floor(c));
    var wd: array<f32, 6>;
    for (var k = 0; k < 6; k++) {
        wd[k] = hc_dir_w(n_t, k);
    }
    var num = vec3<f32>(0.0);
    var den = 0.0;
    for (var j = -3; j <= 3; j++) {
        for (var i = -3; i <= 3; i++) {
            let p = ci + vec2<i32>(i, j);
            if (p.x < 0 || p.y < 0 || p.x >= dl.x || p.y >= dl.y) {
                continue;
            }
            let dc = (vec2<f32>(p) + vec2<f32>(0.5) - c) * tile / reach;
            let g = max(1.0 - length(dc), 0.0);
            if (g <= 0.0) {
                continue;
            }
            for (var k = 0; k < 6; k++) {
                if (wd[k] <= 0.0) {
                    continue;
                }
                let v = textureLoad(hc_probe, p, k, level);
                num += v.xyz * (g * wd[k]);
                den += v.w * (g * wd[k]);
            }
        }
    }
    return vec4<f32>(num / max(den, 1.0e-9), den);
}

fn hc_blur(t: vec2<i32>, n_t: vec3<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(hc_gb2));
    let msize = vec2<i32>(textureDimensions(hc_mask));
    let z_t = hc_view_m(t);
    let step = hc_world_step(z_t, size); // (RSF round 19)
    var sum = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var j = -4; j <= 4; j++) {
        for (var i = -4; i <= 4; i++) {
            let p = t + vec2<i32>(vec2<f32>(f32(i), f32(j)) * step);
            if (p.x < 0 || p.y < 0 || p.x >= size.x || p.y >= size.y) {
                continue;
            }
            if (!hc_kh(textureLoad(hc_gb2, p, 0).xyz)) {
                continue;
            }
            // (RSF round 19) his own body only: a KH texel much nearer or farther (another KH character) is not his light
            let z_s = hc_view_m(p);
            if (z_t > 0.0 && z_s > 0.0 && abs(z_s - z_t) > 0.35 + 0.1 * z_t) {
                continue;
            }
            let nn = textureLoad(hc_gb0, p, 0).xyz * 2.0 - vec3<f32>(1.0);
            let ln = length(nn);
            if (!(ln > 0.5 && ln < 1.5)) {
                continue;
            }
            let c = max(dot(nn / ln, n_t), 0.0);
            let c2 = c * c;
            let c4 = c2 * c2;
            let g = exp(-f32(i * i + j * j) / HC_BLUR_SPREAD); // (RSF round 19) a broad tent: the whole reach counts
            let w = c4 * c4 * g;
            let m = textureLoad(hc_mask, min(p, msize - vec2<i32>(1)), 0);
            let ao_s = clamp(min(m.x, m.y), 0.05, 1.0);
            let l = min(max(textureLoad(hc_light, p, 0).xyz, vec3<f32>(0.0)), vec3<f32>(6.0e4));
            sum += l / ao_s * w;
            wsum += w;
        }
    }
    return vec4<f32>(sum / max(wsum, 1.0e-6), wsum);
}

@fragment
fn hygb_ps(@builtin(position) position: vec4<f32>) -> HyGbOut {
    let t = vec2<i32>(position.xy);
    if (!hc_kh(textureLoad(hc_gb2, t, 0).xyz)) {
        discard;
    }
    var o: HyGbOut;
    o.sss = vec4<f32>(0.0);
    o.emissive = vec4<f32>(0.0);
    return o;
}

// (RSF2 round 12, the user: "make the shadow edges work") THE SUN'S VISIBILITY AT THIS TEXEL'S OWN PLACE, from Elden Ring's own
// cascaded shadow maps: the point from the depth with the KH characters in it (on his body it is HIS point), moved along its
// normal; of the cascades whose picture holds it, the one with the finest texel; there a bilinear PCF over 3 x 3 texels (er.hy[10].y
// scales its step). A building's, a wall's or a branch's shadow edge lands on him sharp where it really is; nothing of the
// background's screen pattern can (the maps are world space). The KH characters are not in the maps (no acne from their own
// bodies: their own shadow on themselves is the capsules', kh_hybrid_light). < 0 = no cascade this frame, no depth here, or no
// cascade holds the point (the caller keeps the round-5 mean). Mirrored in hybrid.rs `csm_visibility` (tested there).
// (RSF2 round 15, the user: "Sora, Donald and Goofy don't cast shadows on each other") With r.shadow_real the KH characters are in
// the game's sun atlas (their shadows on the world and on EACH OTHER); looked up at a KH texel, the atlas also holds that
// character's own body, so every KH character darkened itself (rsf14/rs_sheet). The owner of a point: the KH character whose feet
// are nearest (er.blob for Sora, rows 20..22 the party, game space cm, z up), within 1.3 m. An atlas occluder that IS the owner's
// own body (the atlas depth equals the owner's map within er.hy[19].z steps) does not shade him, unless another KH map shades the
// point; any other occluder (the world, a friend) does. His own shadow on himself stays the capsules' (kh_hybrid_light).
// Rows: [18] the maps' cascade viewport in the atlas, [19] (on, map size, self tolerance (atlas steps), the cascade's row index).
fn hc_owner(q: vec3<f32>) -> i32 {
    var best = -1;
    var best_d = 130.0;
    for (var k = 0; k < 4; k++) {
        var f = er.blob.xyz;
        var ok = true;
        if (k > 0) {
            f = er.hy[19 + k].xyz;
            ok = er.hy[19 + k].w > 0.5;
        }
        if (!ok || q.z < f.z - 60.0 || q.z > f.z + 260.0) {
            continue;
        }
        let d = length(q.xy - f.xy);
        if (d < best_d) {
            best_d = d;
            best = k;
        }
    }
    return best;
}

// a tap of the atlas at texel `at` (atlas pixels) for a point at light depth `z`: 1 lit, 0 shaded, the owner's own body ignored
fn hc_tap_kh(at: vec2<f32>, z: f32, bias: f32, owner: i32, maps_on: bool) -> f32 {
    let a = textureLoad(hc_atlas, vec2<i32>(floor(at)), 0).x;
    let lit = a <= z + bias;
    if (!maps_on) {
        return select(0.0, 1.0, lit);
    }
    let mv = er.hy[18];
    let size = er.hy[19].y;
    let mf = clamp((at - mv.xy) / mv.zw * size, vec2<f32>(0.0), vec2<f32>(size - 1.0));
    let m = vec2<i32>(mf);
    let tol = er.hy[19].z / 65535.0;
    // (RSF2 round 17, the user: "strange shadow artifacts on his face and pants") an atlas occluder is a KH BODY when any KH map
    // reaches it (3 x 3 maximum of every slice: the maps are half the atlas's resolution, a point match leaked at hair and
    // Keyblade edges onto his own face and trousers); then only the OTHER characters' maps decide. A world occluder (nearer to
    // the sun than every KH map there) shades.
    if (!lit) {
        var kh_max = 0.0;
        for (var k = 0; k < 4; k++) {
            for (var dj = -1; dj <= 1; dj++) {
                for (var di = -1; di <= 1; di++) {
                    let q = clamp(m + vec2<i32>(di, dj), vec2<i32>(0), vec2<i32>(i32(size) - 1));
                    kh_max = max(kh_max, textureLoad(hc_kh_maps, q, k, 0).x);
                }
            }
        }
        if (a > kh_max + tol) {
            return 0.0;
        }
    }
    // (another KH character between the point and the sun: his own map; never the owner's)
    for (var k = 0; k < 4; k++) {
        if (k != owner && textureLoad(hc_kh_maps, m, k, 0).x > z + bias + tol) {
            return 0.0;
        }
    }
    return 1.0;
}

// (RSF2 round 16) THE OWNER OF A POINT FROM THE MAPS THEMSELVES: the KH map whose depth at the point's own texel is nearest the
// point's light depth (the surface it lies on; another KH character's body that shades it is farther from it along the light).
// In game (rsf16, a friend pinned 0.7 m from Sora) the nearest-feet rule took Sora as the owner of the friend's points facing him,
// and the friend shaded himself all over. -1 = no KH map there.
fn hc_owner_maps(at: vec2<f32>, z: f32) -> i32 {
    let mv = er.hy[18];
    let size = er.hy[19].y;
    let m = vec2<i32>(clamp((at - mv.xy) / mv.zw * size, vec2<f32>(0.0), vec2<f32>(size - 1.0)));
    var best = -1;
    var best_d = 1.0e9;
    for (var k = 0; k < 4; k++) {
        let d = textureLoad(hc_kh_maps, m, k, 0).x;
        if (d <= 0.0) {
            continue;
        }
        let e = abs(d - z);
        if (e < best_d) {
            best_d = e;
            best = k;
        }
    }
    return best;
}

fn hc_sun_vis(t: vec2<i32>, n_host: vec3<f32>) -> f32 {
    let rows = i32(er.hy[10].x + 0.5);
    if (rows < 1) {
        return -1.0;
    }
    let device_z = textureLoad(hc_depth, t, 0).x;
    if (!(device_z > 0.0)) {
        return -1.0;
    }
    let uv = (vec2<f32>(t) + vec2<f32>(0.5)) * er.viewport.zw;
    let view_z = 1.0 / (device_z * er.depth.z + er.depth.w);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let v = vec3<f32>(ndc.x * er.proj.x, ndc.y * er.proj.y, 1.0) * (view_z * 100.0);
    let q = er.camera.xyz + er.cam_right.xyz * v.x + er.cam_up.xyz * v.y + er.cam_forward.xyz * v.z;
    let host = (er.host_from_game * vec4<f32>(q, 1.0)).xyz + n_host * er.hy[10].w;
    var best_r = -1;
    var best_texel = 1.0e30;
    var best_s = vec3<f32>(0.0);
    for (var r = 0; r < 5; r++) {
        if (r >= rows) {
            break;
        }
        let rel = vec4<f32>(host - textureLoad(hc_casc, vec2<i32>(3, r), 0).xyz, 1.0);
        let row0 = textureLoad(hc_casc, vec2<i32>(24, r), 0);
        let c = vec4<f32>(dot(row0, rel), dot(textureLoad(hc_casc, vec2<i32>(25, r), 0), rel), dot(textureLoad(hc_casc, vec2<i32>(26, r), 0), rel), dot(textureLoad(hc_casc, vec2<i32>(27, r), 0), rel));
        if (!(c.w > 1.0e-6)) {
            continue;
        }
        let s = c.xyz / c.w;
        if (abs(s.x) > 0.97 || abs(s.y) > 0.97 || s.z <= 0.0 || s.z >= 1.0) {
            continue;
        }
        let texel = 2.0 / max(length(row0.xyz) * er.hy[11 + r].z, 1.0e-9);
        if (texel < best_texel) {
            best_texel = texel;
            best_r = r;
            best_s = s;
        }
    }
    if (best_r < 0) {
        return -1.0;
    }
    let vp = er.hy[11 + best_r];
    let px = vp.xy + vec2<f32>(best_s.x * 0.5 + 0.5, 0.5 - best_s.y * 0.5) * vp.zw;
    let step = max(er.hy[10].y, 0.0);
    // (round 15) the KH caster maps apply where the chosen cascade is theirs
    let maps_on = er.hy[19].x > 0.5 && all(abs(vp - er.hy[18]) < vec4<f32>(0.5));
    // (test, r.hybrid_kh_test = 1: no owner: every KH map shades every KH texel, his own body included: shows the maps' shadows land
    // where his body really shades himself)
    let owner = select(-1, hc_owner_maps(px, best_s.z), maps_on && er.hy[19].x < 1.5);
    // (the atlas holds reversed depth: the nearer to the light, the larger; lit where the stored depth is not nearer than the point
    // by more than the bias. MEASURED in game, rsf12/dumpC: the open field matches itself within -0.3 m, the floor of a ruin and
    // the KH characters on it lie ~85 m (along the light) behind the tower that shades them.)
    let bias = er.hy[10].z / 65535.0;
    if (step <= 0.0) {
        return hc_tap_kh(px, best_s.z, bias, owner, maps_on);
    }
    // (the bilinear PCF: the tent over 3 x 3 texels sampled on a 4 x 4 grid)
    let g = (px - vec2<f32>(0.5 * step)) / step;
    let b = floor(g);
    let f = g - b;
    let wx = vec4<f32>(1.0 - f.x, 1.0, 1.0, f.x);
    let wy = vec4<f32>(1.0 - f.y, 1.0, 1.0, f.y);
    var lit = 0.0;
    for (var j = 0; j < 4; j++) {
        for (var i = 0; i < 4; i++) {
            let at = (b + vec2<f32>(f32(i) - 1.0, f32(j) - 1.0)) * step + vec2<f32>(0.5 * step);
            lit += hc_tap_kh(at, best_s.z, bias, owner, maps_on) * wx[i] * wy[j];
        }
    }
    return lit / 9.0;
}

// (mean sun visibility, mean occlusion, KH taps counted)
fn hc_mean() -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(hc_gb2));
    let msize = vec2<i32>(textureDimensions(hc_mask));
    let lo = er.hy[9].xy;
    let hi = er.hy[9].zw;
    if (!(hi.x > lo.x + 1.0) || !(hi.y > lo.y + 1.0)) {
        return vec3<f32>(0.0);
    }
    // (x: visibility, y: occlusion, z: count) over the KH texels, and over every texel of the box
    var kh = vec3<f32>(0.0);
    var all = vec3<f32>(0.0);
    for (var j = 0; j < 12; j++) {
        for (var i = 0; i < 12; i++) {
            let f = (vec2<f32>(f32(i), f32(j)) + vec2<f32>(0.5)) / 12.0;
            let t = clamp(vec2<i32>(lo + (hi - lo) * f), vec2<i32>(0), size - vec2<i32>(1));
            let m = textureLoad(hc_mask, min(t, msize - vec2<i32>(1)), 0);
            let one = vec3<f32>(clamp(m.w, 0.0, 1.0), clamp(min(m.x, m.y), 0.05, 1.0), 1.0);
            all += one;
            if (hc_kh(textureLoad(hc_gb2, t, 0).xyz)) {
                kh += one;
            }
        }
    }
    // (the KH texels when there are a few; else the box around them: the place they stand in)
    let use_ = select(all, kh, kh.z >= 4.0);
    if (use_.z < 0.5) {
        return vec3<f32>(0.0);
    }
    return vec3<f32>(use_.x / use_.z, use_.y / use_.z, use_.z);
}

@fragment
fn hycap_ps(@builtin(position) position: vec4<f32>) -> HyCapOut {
    var o: HyCapOut;
    o.light = vec4<f32>(0.0);
    o.geo = vec4<f32>(0.0);
    let t = vec2<i32>(position.xy);
    let n = textureLoad(hc_gb0, t, 0).xyz * 2.0 - vec3<f32>(1.0);
    let len = length(n);
    if (!(len > 0.5 && len < 1.5)) {
        return o;
    }
    let params = textureLoad(hc_gb1, t, 0);
    let albedo = textureLoad(hc_gb2, t, 0).xyz * (1.0 - clamp(params.x, 0.0, 1.0));
    if (dot(albedo, vec3<f32>(0.2126, 0.7152, 0.0722)) < 0.1) {
        return o;
    }
    // (RSF2 round 7, the user: "Goofy and Donald didn't have anything appearing behind them again, but Sora did") ONLY KH
    // TEXELS ARE CAPTURED. A KH pixel whose own texel holds no G-buffer of a KH character (Sora's hair fringe above all: it has
    // no G-buffer of its own; also a rim pixel the colour draw covers and the G-buffer draw does not) took the WORLD's texel
    // there or beside it whenever its normal happened to agree (hy_tap): the light of what lies behind him, on him. The friends
    // have no fringe, so it showed on Sora only. Now a texel that is not the white a KH G-buffer draw writes stays empty
    // (= not captured): such a pixel takes the nearest KH texel (his core beside it) or today's light. With `r.hybrid_mask` 0:
    // every texel as before.
    if (er.hy[0].x != 0.0 && er.hy[8].z > 0.5 && !hc_kh(textureLoad(hc_gb2, t, 0).xyz)) {
        return o;
    }
    let raw = textureLoad(hc_light, t, 0).xyz;
    // (min / max drop a NaN; an R11G11B10 texel may hold infinity)
    var light = min(max(raw, vec3<f32>(0.0)), vec3<f32>(6.0e4)) / max(albedo, vec3<f32>(0.05));
    // (the mask may be the 1 x 1 white stand-in: a load outside a texture reads 0)
    let m = textureLoad(hc_mask, min(t, vec2<i32>(textureDimensions(hc_mask)) - vec2<i32>(1)), 0);
    var ao = clamp(min(m.x, m.y), 0.05, 1.0);
    var vis = clamp(m.w, 0.0, 1.0);
    // (RSF2 round 10) the printed dark shapes closed (see hc_close); the mask's
    // occlusion and the sun's visibility as their means (round 5)
    var fitted = false;
    if (er.hy[0].x != 0.0 && er.hy[8].w > 2.5) {
        // (round 15: fit 3 = round 13 as it was (stable); fit 4 = the rest of the place + the sun, no per-pixel light (flickered))
        // (RSF2 round 13, fit 3, the default) THE GAME'S OWN LIGHT HERE, its occlusion taken out and the local mean put back (no
        // background pattern through the screen-space occlusion), which carries the game's own sun shadow at his own place (in
        // game, rsf13/dumpF: our cascade lookup agrees with it), held AS IF IN THE SUN for the KH staircase (divided by the world
        // factor), and the world factor in geo.w: mix(1, the shade ratio around him, 1 - his visibility), what the ground beside him
        // shows between shade and sun. The colour shaders multiply by it (fs_main). No fitted sun model: nothing to jump.
        let wide = hc_wide(t, n / len);
        var ao_m = ao;
        if (wide.ok) {
            ao_m = wide.ao;
        }
        let y_px = light / ao;
        // (the game's own shade on him (its ratio on the KH texels) taken out: the light he would have in the sun; then the
        // ground's ratio is the world factor: in the shade he is as dark as the ground beside him)
        var game_shade = 1.0;
        var world = 1.0;
        let own_vis = hc_sun_vis(t, n / len);
        if (own_vis >= 0.0) {
            let sr = hc_shade_ratio(t, n / len);
            // (no measured pair: er.hy[17].y for the KH texels, er.hy[17].x for the ground, through the trust)
            game_shade = mix(1.0, mix(clamp(er.hy[17].y, 0.03, 1.0), sr.x, sr.y), 1.0 - own_vis);
            world = mix(1.0, mix(clamp(er.hy[17].x, 0.03, 1.0), sr.z, sr.w), 1.0 - own_vis);
        }
        world = mix(1.0, world, clamp(er.hy[16].w, 0.0, 1.0));
        light = y_px / max(game_shade, 0.15) * ao_m;
        // (RSF2 round 15, the user: "you can see here that they are still seethrough") the game's light AT THIS TEXEL (y_px) carries
        // what lies behind the KH characters (round 11: the screen-space terms of its light, made partly from the background): round
        // 13 brought it back. Fit 3 now takes no per-pixel light of the game at all: the light in the sun = the local rest of the
        // place (hc_wide, the KH texels' light less their sun part, a smooth tent-weighted mean) + the sun (its model x N.L x the
        // fitted strength); the shadow comes from the cascades (the world factor above). OPT-IN, r.hybrid_fit 4: in game (rsf15) it
        // flickered (frame-to-frame 0.50 grey levels standing still, against 0.05 for round 13's per-pixel light, fit 3, the default).
        if (wide.ok && er.hy[8].w > 3.5 && er.hy[8].w < 4.5) {
            light = (wide.rest + hc_sun(n / len) * wide.k) * ao_m;
        }
        // (RSF2 round 16, fit 5, THE DEFAULT) the game's light blurred over his own surface (hc_blur) in place of the per-pixel light.
        // In game (rsf16, the Gatefront tower wall, low camera, RT MAX and OFF): fit 3 printed leaf-shaped mottling on the trousers
        // (r.hybrid_debug=2), fit 5 none; frame-to-frame flicker standing 0.07-0.13 (fit 3 0.03-0.19), orbiting 0.05-0.08 (fit 3 0.07-0.25)
        // (RSF round 19, fit 6, THE DEFAULT) the probes (hc_probe_light): no printed shape of the background at any size or distance
        var probed = false;
        if (er.hy[8].w > 5.5) {
            let pr = hc_probe_light(t, n / len, hc_view_m(t));
            if (pr.w > 1.0e-4) {
                light = pr.xyz / max(game_shade, 0.15) * ao_m;
                probed = true;
            }
            // (testing aid, r.hybrid_fit 7: the probes' weight as the light)
            if (er.hy[8].w > 6.5) {
                light = vec3<f32>(pr.w * 50.0);
                probed = true;
            }
        }
        if (er.hy[8].w > 4.5 && !probed) {
            let bl = hc_blur(t, n / len);
            if (bl.w > 1.0e-3) {
                light = bl.xyz / max(game_shade, 0.15) * ao_m;
            }
        }
        o.light = vec4<f32>(min(light, vec3<f32>(6.0e4)), ao_m);
        o.geo = vec4<f32>(n / len, world);
        return o;
    } else if (er.hy[0].x != 0.0 && er.hy[8].w > 1.5) {
        // (round 11) the wide fill: the sun's part with the mean visibility + the rest of the place, x the mean occlusion
        let wide = hc_wide(t, n / len);
        var rest = max(light / ao - hc_sun(n / len) * vis, vec3<f32>(0.0));
        var k = 1.0;
        if (wide.ok) {
            rest = wide.rest;
            k = wide.k;
            vis = wide.vis;
            ao = wide.ao;
        }
        // (RSF2 round 12) the sun's visibility at this texel's own place from the game's cascades; else the local mean
        let own_vis = hc_sun_vis(t, n / len);
        if (own_vis >= 0.0) {
            vis = own_vis;
        }
        // (RSF2 round 13, the user: "I don't see any shadow lines") the capture holds the light AS IF IN THE SUN (the form's shading
        // follows the KH staircase on it) and, in geo.w, THE WORLD'S SHADOW FACTOR: the light with the visibility here over the
        // light in the sun, the ratio the ground beside him shows between its shade and its sun. The colour shaders multiply the
        // whole colour by it (kh_hybrid_light, fs_main: hy_world), so a building's or a branch's shadow darkens a KH character as
        // much as the ground beside him; the staircase and the rig's fill no longer flatten it. er.hy[16].w: its strength.
        let sun_here = hc_sun(n / len) * k;
        let lit = rest + sun_here;
        let shade = (rest + sun_here * vis) / max(dot(lit, vec3<f32>(0.2126, 0.7152, 0.0722)), 1.0e-9);
        let world = mix(1.0, clamp(dot(shade, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0, 1.0), clamp(er.hy[16].w, 0.0, 1.0));
        light = lit * ao;
        o.light = vec4<f32>(min(light, vec3<f32>(6.0e4)), ao);
        o.geo = vec4<f32>(n / len, world);
        return o;
    } else if (er.hy[0].x != 0.0 && er.hy[8].w > 0.5) {
        // (the closed light; the occlusion and the sun's visibility as their means, round 5)
        light = hc_close(t, n / len, light);
        let mean = hc_mean();
        if (mean.z > 0.5) {
            vis = mean.x;
            ao = mean.y;
        }
        fitted = true;
    }
    // (RSF2 round 5) every texel of the capture (only KH pixels read it: a world texel is taken only where its normal is a KH
    // pixel's own): the game's screen-space mask out, the mean in (see hc_mean)
    if (!fitted && er.hy[0].x != 0.0 && er.hy[8].z > 0.5) {
        let mean = hc_mean();
        if (mean.z > 0.5) {
            let unit = n / len;
            let to_sun = (er.host_from_game * vec4<f32>(er.hy[6].xyz, 0.0)).xyz;
            var sun = vec3<f32>(0.0);
            if (dot(to_sun, to_sun) > 0.25) {
                sun = max(er.hy[7].xyz, vec3<f32>(0.0)) * max(dot(unit, normalize(to_sun)), 0.0);
            }
            let rest = max(light / ao - sun * vis, vec3<f32>(0.0));
            vis = mean.x;
            // (RSF2 round 12) the sun's visibility from the game's cascades at this texel's own place, when there
            let own_vis = hc_sun_vis(t, unit);
            if (own_vis >= 0.0) {
                vis = own_vis;
            }
            ao = mean.y;
            light = (rest + sun * vis) * ao;
        }
    }
    o.light = vec4<f32>(min(light, vec3<f32>(6.0e4)), ao);
    o.geo = vec4<f32>(n / len, vis);
    return o;
}
#endif

#ifdef ER_H91
// (ER91, the user: "the tips of Sora's hair ... needs to look like this image") THE HAIR'S OWN 4x PASS AT THE GAME'S VELOCITY
// SPLIT (kh-er-render hair91.rs): after the game's forward stage (sky, far field, translucents behind him are in), before its
// TAA. In a box around his head:
// (1) h91_init_ps: the 4x depth from t0: the scene depth WITHOUT him (the early depth's snapshot, h9.p1.w = 1), so that every
//     hair layer is drawn, the ones behind the early core too; without the snapshot (p1.w = 0) the scene depth, pushed back
//     by h9.p1.x (relative) so the core's own fragments pass. h91_init1_ps: the scene depth as it is now (t6: the world, his
//     body, the early core) into the 1x init texture;
// (2) every hair draw twice into the 4x pair (the *_h91m solid half with the depth written, the *_h91n thin half over it),
//     premultiplied, shaded per sample;
// (3) h91_comp_ps: the four samples resolved and laid over the scene, where no part of him lies in front of the hair (the
//     scene depth nearer than the hair's front, beyond a tolerance: his face over the hair behind his head); where the early
//     core lies in the scene (its pixel lost what was behind it) and the hair does not cover all of it, the background is the
//     nearest hair-free scene around it;
// (4) h91_velo_ps: his motion vector, depth and TAA class where the hair covers at least h9.p1.y of a pixel that the early
//     core did not take (only with the snapshot: the pass writes the scene depth that t0 would otherwise be).
struct H91Params {
    // the box: x, y (scene pixels), width, height
    box_: vec4<f32>,
    // x: the depth tolerance (relative), y: the coverage from which a pixel is his (velocity, depth), z: the background
    // search radius (pixels), w: 1 = t0 is the scene depth without him (the snapshot)
    p1: vec4<f32>,
}
@group(0) @binding(1) var<uniform> h9: H91Params;
@group(1) @binding(0) var h9_depth: texture_2d<f32>;
@group(1) @binding(1) var h9_col: texture_multisampled_2d<f32>;
@group(1) @binding(2) var h9_aux: texture_multisampled_2d<f32>;
@group(1) @binding(3) var h9_hd: texture_depth_multisampled_2d;
@group(1) @binding(4) var h9_scene: texture_2d<f32>;
// the scene depth as it is now, of the box (1x, h91_init1_ps): the composite and the velocity pass read it
@group(1) @binding(5) var h9_init: texture_2d<f32>;
// the scene depth as it is now (h91_init1_ps only)
@group(1) @binding(6) var h9_now: texture_2d<f32>;
// THE MARK (full size, scene pixels): 1 where the early hair core is the scene's surface (hair91.rs: the core shaders
// once more, depth EQUAL against the scene depth)
@group(1) @binding(7) var h9_mark: texture_2d<f32>;

@vertex
fn h91_vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(p * 2.0 - vec2<f32>(1.0), 0.0, 1.0);
}

// what the 4x depth was set to at a scene pixel
fn h91_init_depth(scene_px: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(h9_depth));
    let d = textureLoad(h9_depth, clamp(scene_px, vec2<i32>(0), size - vec2<i32>(1)), 0).x;
    return select(d * (1.0 - h9.p1.x), d, h9.p1.w > 0.5);
}

// (1) the box's pixel p (its own target) = scene pixel p + box.xy
@fragment
fn h91_init_ps(@builtin(position) p: vec4<f32>) -> @builtin(frag_depth) f32 {
    return h91_init_depth(vec2<i32>(p.xy + h9.box_.xy));
}

// (1b) the scene depth now into the box's 1x init texture
@fragment
fn h91_init1_ps(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(h9_now));
    let q = clamp(vec2<i32>(p.xy + h9.box_.xy), vec2<i32>(0), size - vec2<i32>(1));
    return vec4<f32>(textureLoad(h9_now, q, 0).x, 0.0, 0.0, 1.0);
}

struct H91Px {
    // premultiplied colour + coverage, (velocity, depth) x coverage, the coverage
    col: vec4<f32>,
    aux: vec4<f32>,
    // the early core is the scene's surface here (its pixel lost what lay behind it), and its colour there (the mark)
    core: bool,
    mark: vec3<f32>,
    // hair to lay over the scene here (some, and nothing of him in front of it)
    any: bool,
    // any hair here at all (in front of him or not)
    some: bool,
}

fn h91_px(local: vec2<i32>, scene_px: vec2<i32>) -> H91Px {
    var o: H91Px;
    o.col = vec4<f32>(0.0);
    o.aux = vec4<f32>(0.0);
    o.core = false;
    o.any = false;
    o.some = false;
    // the early core is the surface here: the mark (exact: its own shader at this pixel's centre, depth EQUAL)
    let mark_size = vec2<i32>(textureDimensions(h9_mark));
    let mark = textureLoad(h9_mark, clamp(scene_px, vec2<i32>(0), mark_size - vec2<i32>(1)), 0);
    o.core = mark.w > 0.5;
    o.mark = mark.xyz;
    let size = vec2<i32>(textureDimensions(h9_col));
    if (any(local < vec2<i32>(0)) || any(local >= size)) {
        return o;
    }
    let now = textureLoad(h9_init, local, 0).x;
    let base = h91_init_depth(scene_px);
    var solid = false;
    var zmax = 0.0;
    var zmin = 1.0e9;
    for (var s = 0; s < 4; s++) {
        o.col += textureLoad(h9_col, local, s);
        o.aux += textureLoad(h9_aux, local, s);
        let d = textureLoad(h9_hd, local, s);
        if (d > base) {
            solid = true;
            zmax = max(zmax, d);
            zmin = min(zmin, d);
        }
    }
    o.col *= 0.25;
    o.aux *= 0.25;
    o.some = solid || o.col.w > 0.002;
    if (!o.some) {
        return o;
    }
    // the hair's front here: its nearest solid sample, or the thin layers' mean depth
    let front = max(select(0.0, zmax, solid), select(0.0, o.aux.z / max(o.aux.w, 1.0e-6), o.aux.w > 0.0));
    // something of his other than the hair core (his face, his body: the scene depth changed by him, not the mark) nearer
    // than the hair's front: the hair lies behind it here
    let his = now > base * (1.0 + 1.0e-6) + 1.0e-9;
    o.any = o.core || !his || front >= now * (1.0 - 1.0e-5);
    let _z = zmin;
    return o;
}

// (3) laid over the scene (the scene pass's viewport; scissored to the box)
@fragment
fn h91_comp_ps(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let scene_px = vec2<i32>(p.xy);
    let local = scene_px - vec2<i32>(h9.box_.xy);
    let h = h91_px(local, scene_px);
    if (!h.any) {
        discard;
    }
    let here = textureLoad(h9_scene, local, 0).xyz;
    var behind = here;
    // (the early core's pixel: what the game's forward stage added over it since (its effects, fog in front of him) is
    // kept: the scene less the core's own colour)
    var over = vec3<f32>(0.0);
    if (h.core && h.col.w < 0.999) {
        // the early core took this pixel: what lies behind its uncovered part is the nearest hair-free scene around it
        let r = i32(h9.p1.z);
        var sum = vec3<f32>(0.0);
        var weight = 0.0;
        let copy_size = vec2<i32>(textureDimensions(h9_scene));
        for (var y = -r; y <= r; y++) {
            for (var x = -r; x <= r; x++) {
                if (x == 0 && y == 0) {
                    continue;
                }
                let q = local + vec2<i32>(x, y);
                if (any(q < vec2<i32>(0)) || any(q >= copy_size)) {
                    continue;
                }
                let n = h91_px(q, scene_px + vec2<i32>(x, y));
                // (a pixel whose scene is not the early core: its scene is what lies there, the 4x hair is not in it yet)
                if (!n.core) {
                    let w = 1.0 / f32(x * x + y * y);
                    sum += textureLoad(h9_scene, q, 0).xyz * w;
                    weight += w;
                }
            }
        }
        if (weight > 0.0) {
            behind = sum / weight;
        }
    }
    if (h.core) {
        over = here - h.mark;
        behind = select(behind, vec3<f32>(0.0), h.col.w >= 0.999);
    }
    return vec4<f32>(max(h.col.xyz + behind * (1.0 - h.col.w) + over, vec3<f32>(0.0)), 1.0);
}

struct H91Velo {
    @location(0) velocity: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

// (4) his motion vector, depth (and, by the pipeline, his stencil) where the hair covers at least h9.p1.y of a pixel the
// early core did not take (the core's pixels have them from his own draws)
@fragment
fn h91_velo_ps(@builtin(position) p: vec4<f32>) -> H91Velo {
    let scene_px = vec2<i32>(p.xy);
    let h = h91_px(scene_px - vec2<i32>(h9.box_.xy), scene_px);
    if (!h.any || h.core || h.aux.w < h9.p1.y) {
        discard;
    }
    var o: H91Velo;
    o.velocity = vec4<f32>(h.aux.xy / h.aux.w, 0.0, 1.0);
    o.depth = h.aux.z / h.aux.w;
    return o;
}
#endif

#ifdef ER_RECOMP
// (IMPL_ER65, recomp.ps.dxil; the user: "Seeing the erdtree thru sora like that is NOT normal, FIX IT") THE LATE
// RE-COMPOSITE: after the game's forward stage, before its bloom / exposure / tone mapping, his own colour as it was right
// after we drew him (t0) goes back over the scene where the depth is still HIS (t1: the scene depth right after him,
// t3: the scene depth now; something opaque the game drew in front of him since then wins) and he changed the depth (t2:
// the depth without him). Only where what lies behind him is the sky or farther than er.depth.x metres (0 = everywhere):
// there everything the game drew over him is behind him (sky, far field, Erdtree), while in front of a near background a
// translucent effect over him may be in front of him and is kept.
@group(1) @binding(0) var rc_saved: texture_2d<f32>;
@group(1) @binding(1) var rc_him: texture_2d<f32>;
@group(1) @binding(2) var rc_bg: texture_2d<f32>;
@group(1) @binding(3) var rc_now: texture_2d<f32>;

@fragment
fn recomp_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let t = vec2<i32>(position.xy);
    let him = textureLoad(rc_him, t, 0).x;
    let bg = textureLoad(rc_bg, t, 0).x;
    let now = textureLoad(rc_now, t, 0).x;
    if (him <= 0.0 || him == bg || now != him) {
        discard;
    }
    if (er.depth.x > 0.0 && bg > 0.0) {
        let behind = 1.0 / (bg * er.depth.z + er.depth.w);
        if (behind < er.depth.x) {
            discard;
        }
    }
    return vec4<f32>(textureLoad(rc_saved, t, 0).xyz, 1.0);
}
#endif

#ifdef ER_MASKDOWN
// (IMPL_ER80b, maskdown.ps.dxil) HIS MASK AT A REDUCED SIZE for the game's light-shaft composite when it draws below the full
// resolution: t0 = the stencil plane of his full-size mask (1 = his visible surface); the target's size is er.viewport.xy.
// A pixel of the reduced mask is his when ANY full-size texel under it is (the shafts never add over any of his pixels):
// the pixel shader passes (the pipeline writes stencil 1) or discards.
@group(1) @binding(0) var md_mask: texture_2d<u32>;

@fragment
fn maskdown_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let full = vec2<i32>(textureDimensions(md_mask));
    let scale = vec2<f32>(full) / max(er.viewport.xy, vec2<f32>(1.0));
    let lo = vec2<i32>(floor((position.xy - vec2<f32>(0.5)) * scale));
    let n = clamp(vec2<i32>(ceil(scale)), vec2<i32>(1), vec2<i32>(8));
    var his = false;
    for (var y = 0; y < n.y; y++) {
        for (var x = 0; x < n.x; x++) {
            let t = clamp(lo + vec2<i32>(x, y), vec2<i32>(0), full - vec2<i32>(1));
            if ((textureLoad(md_mask, t, 0).y & 0xFFu) != 0u) {
                his = true;
            }
        }
    }
    if (!his) {
        discard;
    }
    return vec4<f32>(0.0);
}
#endif

#ifdef ER_RECOMP2
// (IMPL_ER80, recomp2.ps.dxil) THE LATE RE-COMPOSITE, TEMPORALLY STABLE. Its hair flicker in the game (IMPL_ER74, 48.jpg) came
// from where and how it pasted: it ran after the game's forward SUBMISSION, and that submission holds the game's temporal
// anti-aliasing (TempAA[StencilMask] right after the velocity list, measured in er/logs/cbcap_0.txt), so it pasted his
// JITTERED, un-resolved colour of this frame over the resolved picture (his edges and thin hair changed every frame with
// the sub-pixel jitter, nothing averaged them), and its far condition switched each pixel on or off as the background
// behind his hair crossed the distance. This version is drawn BEFORE the game's TAA (gfx.rs: at the velocity split), so the
// TAA resolves his restored colour like the rest of the picture, and the far condition is a smooth weight (the alpha the
// pipeline blends with) from half the distance to the distance. The same inputs as recomp.ps.dxil.
@group(1) @binding(0) var rc_saved: texture_2d<f32>;
@group(1) @binding(1) var rc_him: texture_2d<f32>;
@group(1) @binding(2) var rc_bg: texture_2d<f32>;
@group(1) @binding(3) var rc_now: texture_2d<f32>;

@fragment
fn recomp2_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let t = vec2<i32>(position.xy);
    let him = textureLoad(rc_him, t, 0).x;
    let bg = textureLoad(rc_bg, t, 0).x;
    let now = textureLoad(rc_now, t, 0).x;
    if (him <= 0.0 || him == bg || now != him) {
        discard;
    }
    var w = 1.0;
    if (er.depth.x > 0.0 && bg > 0.0) {
        let behind = 1.0 / (bg * er.depth.z + er.depth.w);
        w = smoothstep(0.5 * er.depth.x, er.depth.x, behind);
    }
    if (w <= 0.0) {
        discard;
    }
    return vec4<f32>(textureLoad(rc_saved, t, 0).xyz, w);
}
#endif

#ifdef ER_FINALE
// (ZFIN, finale.ps.dxil; kh-er-render src/finale.rs) ZETTAFLARE'S FINALE ON AN ELDEN RING CHARACTER (the user: "the enemy to
// get immersed in the white light, with a black silhouette that crumbles leaving only the white light ... universally on any
// enemy"). Drawn over the scene target after our effects (alpha-blended), one full-screen triangle per target, scissored to
// its screen box. WHICH PIXELS ARE IT, from the game's own buffers and the host's capsules, no per-enemy data:
// - t1 / t2: the snapshot of the scene depth taken at the early split (the G-buffer's depth and STENCIL, before any KH
//   character): ITS OWN STENCIL VALUE, learnt at run time (finale_probe_ps: the value the snapshot holds where its depth lies
//   inside the target's capsule, at points along it; kept in t3 from frame to frame while it falls or is hidden from view);
//   a pixel is its when the stencil's upper four bits match (the classes measured: characters 0x50, terrain 0x12 / 0xd2 /
//   0x90, map objects 0x10, sky 0); when its value is the ground's around its feet, only what stands 8 cm above them;
// - t0: the scene depth now: a pixel something nearer covered since (a KH character's early depth, an opaque effect) is not it;
// - the position the snapshot's depth gives lies within the target's reach (around its feet, up to its top) and nearer the
//   target's capsule than any neighbour's (the host's other characters: fz.f[11..27]).
// When its value cannot be learnt (no sample of it yet), or none of its pixels is found (taps over its box), or the host says
// the buffers are missing (mode 2), the look falls back to a WHITE-OUT of its screen box (an ellipse, behind whatever is
// nearer than its front). Mode 3: the mask. After the crumble its body is hidden by the host and the last white shape (t4,
// written by finale_g_ps) fades out (finale_fade_ps).
struct FinaleBlock {
    // [0] camera (game space, cm), mode (1 auto, 2 the white-out only, 3 the mask as a debug view)
    // [1] right + tan(half horizontal fov); [2] up + tan(half vertical fov); [3] forward + the white (target units)
    // [4] width, height, 1 / width, 1 / height (pixels of the target)
    // [5] view depth (m) = 1 / (device * x + y); z: 1 = t0 bound; w: 1 = t1 / t2 bound (the snapshot of this frame)
    // [6] its feet (game space, cm) + radius; [7] half height, immerse (0..1), silhouette (1), crumble (0..1)
    // [8] seed, neighbours (0..8), reach (cm, around its feet), top (cm above its feet)
    // [9] its body's screen box (pixels): x0, y0, x1, y1; [10] its front's view depth (m), the rim's white, -, -
    // [11..27) the neighbours: feet + radius, (half height, -, -, -)
    // [27..31) clip from game space (columns); [31] its slot in t3, the ghost's alpha (fade), -, its id; [32..38) the
    // breaking apart (finale_snap_ps, finale_frag_vs below; [37] the view at the snapshot); [38..40) reserved
    // (round 5, the beam's colour) [38] 1 = write the MASK (r: its body / pieces, g: its light: cracks, embers, what is
    // eroded), the band (px), (round 6) how far it has darkened (ScriptFinale::dark); [39] the deep core + the scale of the light to the target; [40] the edge; [41] the light's
    // colour (display-linear: finale_comp_ps turns its result into the target's units)
    f: array<vec4<f32>, 44>,
}
@group(0) @binding(1) var<uniform> fz: FinaleBlock;
@group(1) @binding(0) var fz_live: texture_2d<f32>;
@group(1) @binding(1) var fz_snap: texture_2d<f32>;
@group(1) @binding(2) var fz_sten: texture_2d<u32>;
// its learnt stencil value: x value (-1 none), y id, z the ground's value around its feet (-1 none), w samples
@group(1) @binding(3) var fz_mem: texture_2d<f32>;
// the last white shape (finale_g_ps), for its fade
@group(1) @binding(4) var fz_ghost: texture_2d<f32>;

fn fz_hash(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + vec3<f32>(33.33));
    return fract((q.x + q.y) * q.z);
}

fn fz_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec3<f32>(3.0) - 2.0 * f);
    let x00 = mix(fz_hash(i), fz_hash(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(fz_hash(i + vec3<f32>(0.0, 1.0, 0.0)), fz_hash(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(fz_hash(i + vec3<f32>(0.0, 0.0, 1.0)), fz_hash(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(fz_hash(i + vec3<f32>(0.0, 1.0, 1.0)), fz_hash(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// How far a point lies outside a standing capsule (feet, radius, half height), in its radii.
fn fz_score(p: vec3<f32>, feet: vec3<f32>, radius: f32, half_height: f32) -> f32 {
    let r = max(radius, 1.0);
    let lo = feet.z + min(r, half_height);
    let hi = feet.z + max(2.0 * half_height - r, min(r, half_height));
    let c = vec3<f32>(feet.x, feet.y, clamp(p.z, lo, hi));
    return max(length(p - c) - r, 0.0) / max(r, 25.0);
}

// The texel of the depth textures under a pixel of the target (a depth of another size is scaled).
fn fz_texel(px: vec2<f32>) -> vec2<i32> {
    let dims = vec2<f32>(textureDimensions(fz_snap));
    let t = vec2<i32>(px * dims * fz.f[4].zw);
    return clamp(t, vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1));
}

// The game-space position of a target pixel at a snapshot depth.
fn fz_pos(px: vec2<f32>, ds: f32) -> vec3<f32> {
    let view_m = 1.0 / (ds * fz.f[5].x + fz.f[5].y);
    let ndc = vec2<f32>(px.x * fz.f[4].z * 2.0 - 1.0, 1.0 - px.y * fz.f[4].w * 2.0);
    let dir = fz.f[3].xyz + fz.f[1].xyz * (ndc.x * fz.f[1].w) + fz.f[2].xyz * (ndc.y * fz.f[2].w);
    return fz.f[0].xyz + dir * (view_m * 100.0);
}

// A game-space point's pixel (x < 0: behind the camera).
fn fz_project(p: vec3<f32>) -> vec2<f32> {
    let c = fz.f[27] * p.x + fz.f[28] * p.y + fz.f[29] * p.z + fz.f[30];
    if (c.w <= 1.0e-3) {
        return vec2<f32>(-1.0);
    }
    return vec2<f32>((c.x / c.w * 0.5 + 0.5) * fz.f[4].x, (0.5 - c.y / c.w * 0.5) * fz.f[4].y);
}

struct FzSample {
    ok: bool,
    value: u32,
    p: vec3<f32>,
}

// What the snapshot holds at a pixel (its stencil value and position), when the scene depth now is not nearer there.
fn fz_sample(px: vec2<f32>) -> FzSample {
    var o: FzSample;
    o.ok = false;
    o.value = 0u;
    o.p = vec3<f32>(0.0);
    if (px.x < 0.0 || px.y < 0.0 || px.x >= fz.f[4].x || px.y >= fz.f[4].y) {
        return o;
    }
    let t = fz_texel(px);
    let ds = textureLoad(fz_snap, t, 0).x;
    if (ds <= 0.0) {
        return o;
    }
    if (fz.f[5].z > 0.5 && textureLoad(fz_live, t, 0).x > ds) {
        return o;
    }
    o.ok = true;
    o.value = textureLoad(fz_sten, t, 0).y & 0xFFu;
    o.p = fz_pos(px, ds);
    return o;
}

// THE PROBE (one pixel per target of the 8 x 1 memory, read back by the main pass of the same frame and by the next frame's
// probe): its own stencil value, the most frequent among the snapshot's values at points along its capsule whose depth lies
// inside the capsule (8 heights x 3 across the view), and the ground's around its feet (8 points); the value learnt before is
// kept while no new one is seen (it fell over, something covers it).
@fragment
fn finale_probe_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let slot = i32(fz.f[31].x);
    let id = fz.f[31].w;
    let prev = textureLoad(fz_mem, vec2<i32>(slot, 0), 0);
    let feet = fz.f[6].xyz;
    let r = max(fz.f[6].w, 5.0);
    let height = 2.0 * max(fz.f[7].x, r);
    var vals = array<u32, 4>(0u, 0u, 0u, 0u);
    var counts = array<i32, 4>(0, 0, 0, 0);
    for (var i = 0; i < 8; i++) {
        for (var s = -1; s <= 1; s++) {
            let at = feet + vec3<f32>(0.0, 0.0, height * (0.12 + 0.11 * f32(i))) + fz.f[1].xyz * (0.45 * r * f32(s));
            let smp = fz_sample(fz_project(at));
            if (!smp.ok || smp.p.z < feet.z + 8.0) {
                continue;
            }
            let axis = vec3<f32>(feet.x, feet.y, clamp(smp.p.z, feet.z, feet.z + height));
            if (length(smp.p - axis) > r * 1.15 + 5.0) {
                continue;
            }
            var placed = false;
            for (var k = 0; k < 4; k++) {
                if (!placed && counts[k] > 0 && vals[k] == smp.value) {
                    counts[k] += 1;
                    placed = true;
                }
            }
            for (var k = 0; k < 4; k++) {
                if (!placed && counts[k] == 0) {
                    vals[k] = smp.value;
                    counts[k] = 1;
                    placed = true;
                }
            }
        }
    }
    var best = -1;
    var best_n = 1;
    for (var k = 0; k < 4; k++) {
        if (counts[k] > best_n) {
            best_n = counts[k];
            best = i32(vals[k]);
        }
    }
    // the ground around its feet
    var g_vals = array<u32, 4>(0u, 0u, 0u, 0u);
    var g_counts = array<i32, 4>(0, 0, 0, 0);
    for (var a = 0; a < 8; a++) {
        let ang = f32(a) * 0.785398;
        let at = feet + vec3<f32>(cos(ang), sin(ang), 0.0) * (r * 1.8 + 20.0) + vec3<f32>(0.0, 0.0, 2.0);
        let smp = fz_sample(fz_project(at));
        if (!smp.ok || abs(smp.p.z - feet.z) > 25.0) {
            continue;
        }
        var placed = false;
        for (var k = 0; k < 4; k++) {
            if (!placed && g_counts[k] > 0 && g_vals[k] == smp.value) {
                g_counts[k] += 1;
                placed = true;
            }
        }
        for (var k = 0; k < 4; k++) {
            if (!placed && g_counts[k] == 0) {
                g_vals[k] = smp.value;
                g_counts[k] = 1;
                placed = true;
            }
        }
    }
    var ground = -1;
    var ground_n = 0;
    for (var k = 0; k < 4; k++) {
        if (g_counts[k] > ground_n) {
            ground_n = g_counts[k];
            ground = i32(g_vals[k]);
        }
    }
    let known = prev.y == id && prev.x >= 0.0;
    if (best < 0 && known) {
        return vec4<f32>(prev.x, id, select(prev.z, f32(ground), ground >= 0), prev.w);
    }
    return vec4<f32>(f32(best), id, f32(ground), f32(best_n));
}

struct FzTap {
    body: bool,
    p: vec3<f32>,
}

// Is the pixel (target pixels) the target's surface? With its position (game space, cm). `value`: its learnt stencil
// value; `guard`: that value is the ground's (only what stands above its feet).
fn fz_tap(px: vec2<f32>, value: u32, guard: bool) -> FzTap {
    var o: FzTap;
    o.body = false;
    o.p = vec3<f32>(0.0);
    let smp = fz_sample(px);
    if (!smp.ok || (smp.value & 0xF0u) != (value & 0xF0u)) {
        return o;
    }
    // (ZDRAW, 2026-10-23, the user's screenshot: a small creature's "silhouette" painted on the grass round it) a target whose
    // class is the ground's cannot be told from the ground by the screen: none of its pixels are taken (it shows as the game
    // draws it, in the beam's light)
    if (guard) {
        return o;
    }
    let p = smp.p;
    let rel = p - fz.f[6].xyz;
    if (length(rel.xy) > fz.f[8].z || rel.z < -40.0 || rel.z > fz.f[8].w || (guard && rel.z < 8.0)) {
        return o;
    }
    let mine = fz_score(p, fz.f[6].xyz, fz.f[6].w, fz.f[7].x);
    // (ZDRAW) only within its own capsule (half its radius around it, at least 12 cm): never the grass or foliage about it
    if (mine > 0.5) {
        return o;
    }
    let n = i32(fz.f[8].y);
    for (var k = 0; k < 8; k++) {
        if (k >= n) {
            break;
        }
        let a = fz.f[11 + 2 * k];
        let b = fz.f[12 + 2 * k];
        if (fz_score(p, a.xyz, a.w, b.x) < mine) {
            return o;
        }
    }
    o.body = true;
    o.p = p;
    return o;
}

// The finale's colour at a pixel (rgb, alpha) and, in w of the second, how much of it is the white light (for the ghost).
struct FzOut {
    color: vec4<f32>,
    white: f32,
    draw: bool,
}

fn fz_shade(position: vec4<f32>) -> FzOut {
    var o: FzOut;
    o.color = vec4<f32>(0.0);
    o.white = 0.0;
    o.draw = false;
    let mode = fz.f[0].w;
    let white = fz.f[3].w;
    let immerse = clamp(fz.f[7].y, 0.0, 1.0);
    let silhouette = fz.f[7].z > 0.5;
    let crumble = clamp(fz.f[7].w, 0.0, 1.0);
    let seed = fz.f[8].x;
    let mem = textureLoad(fz_mem, vec2<i32>(i32(fz.f[31].x), 0), 0);
    let learnt = mem.y == fz.f[31].w && mem.x >= 0.0;
    let value = u32(max(mem.x, 0.0));
    let guard = mem.z >= 0.0 && (u32(mem.z) & 0xF0u) == (value & 0xF0u);
    let isolate = (mode < 1.5 || mode > 2.5) && fz.f[5].w > 0.5 && learnt;
    // its pixels around this one (a soft one-pixel edge: the late pass draws after the game's TAA)
    var count = 0.0;
    var centre = false;
    var p = vec3<f32>(0.0);
    if (isolate) {
        for (var j = -1; j <= 1; j++) {
            for (var i = -1; i <= 1; i++) {
                let tap = fz_tap(position.xy + vec2<f32>(f32(i), f32(j)), value, guard);
                if (tap.body) {
                    count += 1.0;
                    if (i == 0 && j == 0) {
                        centre = true;
                        p = tap.p;
                    } else if (!centre) {
                        p = tap.p;
                    }
                }
            }
        }
    }
    // is it found at all? (taps over its box from the middle out; every pixel gets the same answer)
    var found = count > 0.0;
    let b = fz.f[9];
    if (!found && isolate) {
        for (var j = 0; j < 10; j++) {
            let row = select(4 - j / 2, 5 + j / 2, (j & 1) == 1);
            for (var i = 0; i < 6; i++) {
                let at = mix(b.xy, b.zw, (vec2<f32>(f32(i), f32(row)) + vec2<f32>(0.5)) / vec2<f32>(6.0, 10.0));
                if (fz_tap(at, value, guard).body) {
                    found = true;
                    break;
                }
            }
            if (found) {
                break;
            }
        }
    }
    if (found) {
        if (count <= 0.0) {
            return o;
        }
        o.draw = true;
        let cover = count / 9.0;
        let edge = select(smoothstep(0.0, 0.67, cover), 1.0, centre && cover > 0.6);
        if (mode > 2.5) {
            o.color = vec4<f32>(white, 0.0, white, 0.85 * edge);
            return o;
        }
        if (!silhouette) {
            // engulfed in white (a target the beam did not kill)
            // (round 7) its own look shows through (as the test level whitens its material): never a flat plate
            o.color = vec4<f32>(vec3<f32>(white), 0.8 * immerse * edge);
            o.white = immerse * edge;
            return o;
        }
        // the black silhouette, crumbling from the top down along a noise fixed to its body
        let rel = p - fz.f[6].xyz;
        let h01 = clamp(rel.z / max(2.0 * fz.f[7].x, 1.0), 0.0, 1.0);
        let grain = 0.65 * fz_noise(rel * 0.075 + vec3<f32>(seed)) + 0.35 * fz_noise(rel * 0.23 + vec3<f32>(seed * 1.7 + 11.0));
        let key = 0.55 * grain + 0.45 * (1.0 - h01);
        let gone = crumble * 1.12;
        if (key < gone) {
            // broken away: only the white light is left
            o.color = vec4<f32>(vec3<f32>(white), edge);
            o.white = edge;
            return o;
        }
        if (key < gone + 0.045 && crumble > 0.0) {
            // the crumbling rim glows
            o.color = vec4<f32>(vec3<f32>(white * fz.f[10].y), edge);
            o.white = edge;
            return o;
        }
        o.color = vec4<f32>(0.0, 0.0, 0.0, edge);
        return o;
    }
    // THE FALLBACK: a white-out of its screen box (an ellipse), behind whatever lies nearer than its front
    // (round 7, the user's Malenia: a survivor under a huge white ellipse) only a killed target is ever whited out: a survivor
    // not found shows as the game draws it, in the beam's light
    if (!silhouette) {
        return o;
    }
    // (ZDRAW, 2026-10-23, the user: "a weird white light that is extremely obvious") the white-out ellipse is never drawn: a body
    // not found shows as the game draws it, in the beam's light (mode 9 alone, never set, would still reach it)
    if (fz.f[0].w < 8.5) {
        return o;
    }
    let c = 0.5 * (b.xy + b.zw);
    let r = max(0.5 * (b.zw - b.xy), vec2<f32>(1.0));
    let q = (position.xy - c) / r;
    let e = dot(q, q);
    if (e >= 1.0) {
        return o;
    }
    if (fz.f[5].z > 0.5) {
        let dl = textureLoad(fz_live, fz_texel(position.xy), 0).x;
        if (dl > 0.0 && 1.0 / (dl * fz.f[5].x + fz.f[5].y) < fz.f[10].x) {
            return o;
        }
    }
    var a = select(immerse, 1.0, silhouette) * smoothstep(1.0, 0.75, e);
    if (crumble > 0.0) {
        let grain = fz_noise(vec3<f32>(q * 5.0, seed));
        a *= smoothstep(crumble * 1.15 - 0.08, crumble * 1.15, 0.7 * grain + 0.3 * (1.0 - e));
    }
    if (a <= 0.0) {
        return o;
    }
    o.draw = true;
    o.white = a;
    if (mode > 2.5) {
        o.color = vec4<f32>(0.0, white, white, 0.6 * a);
        return o;
    }
    o.color = vec4<f32>(vec3<f32>(white), a);
    return o;
}

@fragment
fn finale_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let o = fz_shade(position);
    if (!o.draw) {
        discard;
    }
    // (round 5) the mask: black = its body, anything lit = its light
    if (fz.f[38].x > 0.5) {
        let lit = max(o.color.r, max(o.color.g, o.color.b)) > 0.0;
        return select(vec4<f32>(o.color.a, 0.0, 0.0, 1.0), vec4<f32>(0.0, o.color.a, 0.0, 1.0), lit);
    }
    return o.color;
}

// The same, and the white light's share into the ghost (R8, cleared to 0 first): the shape that fades once it is hidden.
struct FzTwo {
    @location(0) color: vec4<f32>,
    @location(1) ghost: vec4<f32>,
}

@fragment
fn finale_g_ps(@builtin(position) position: vec4<f32>) -> FzTwo {
    let o = fz_shade(position);
    if (!o.draw) {
        discard;
    }
    var two: FzTwo;
    two.color = o.color;
    two.ghost = vec4<f32>(o.white, 0.0, 0.0, 1.0);
    return two;
}

// ITS BODY HIDDEN (the host, after the crumble): the last white shape fades out over the game's own picture behind it.
@fragment
fn finale_fade_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(fz_ghost));
    let t = clamp(vec2<i32>(position.xy * dims * fz.f[4].zw), vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1));
    let a = textureLoad(fz_ghost, t, 0).x * clamp(fz.f[31].y, 0.0, 1.0);
    if (a <= 0.0) {
        discard;
    }
    return vec4<f32>(vec3<f32>(fz.f[3].w), a);
}

// ---- (ZFIN round 3) THE SILHOUETTE BREAKS APART (the user: "it needs to look like the silhouette is actually breaking
// apart"; the test level's kh-app zetta_crumble.rs, mirrored: its `timeline`, `crack_reveal` and `fly` below). While it is a
// silhouette and has not begun to crumble, finale_snap_ps keeps a SNAPSHOT of it: its own pixels (fz_tap) over its screen box,
// each texel its place relative to its feet (game cm) and its footprint (cm), in t5. From the first crumble on the host hides
// its body and finale_frag_vs / finale_frag_ps draw the snapshot instead, one camera-facing splat per texel at its place NOW
// (the camera may move): cut into Voronoi cells of a jittered lattice fixed to its body (seeded by its id), glowing cracks
// along the cells' borders first (spreading from the side the beam strikes), the cells released top-first on the beam's push
// (`ScriptFinale::push`), tumbling under gravity, landing and sliding, each splitting into three fragments, which shrink to
// nothing shedding ash and embers. Nothing is left at crumble 1. Rows: [32] its box (x0, y0), the step (px), the atlas width;
// [33] the atlas height, the cell size (cm), its scale (height / 180 cm), the crumble's seconds; [34] the push (unit) + the
// floor (game z, cm); [35] its feet at the snapshot + its height (cm); [36] the embers' colour (target units) + the cracks'.
@group(1) @binding(5) var fz_atlas: texture_2d<f32>;

// the snapshot: its place relative to its feet (cm) and its footprint (cm, 0 = not it)
@fragment
fn finale_snap_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let mem = textureLoad(fz_mem, vec2<i32>(i32(fz.f[31].x), 0), 0);
    if (mem.y != fz.f[31].w || mem.x < 0.0) {
        return vec4<f32>(0.0);
    }
    let value = u32(max(mem.x, 0.0));
    let guard = mem.z >= 0.0 && (u32(mem.z) & 0xF0u) == (value & 0xF0u);
    let step = fz.f[32].z;
    let px = fz.f[32].xy + position.xy * step;
    let tap = fz_tap(px, value, guard);
    if (!tap.body) {
        return vec4<f32>(0.0);
    }
    let view_m = dot(tap.p - fz.f[0].xyz, fz.f[3].xyz) / 100.0;
    let footprint = view_m * 100.0 * 2.0 * fz.f[2].w * fz.f[4].w * step;
    return vec4<f32>(tap.p - fz.f[6].xyz, max(footprint, 0.05));
}

fn fz_h(c: vec3<f32>, k: f32) -> f32 {
    return fz_hash(c * 1.37 + vec3<f32>(k * 17.31 + fz.f[8].x, k * 3.71, k * 11.13));
}

fn fz_rdir(c: vec3<f32>, k: f32) -> vec3<f32> {
    let z = fz_h(c, k) * 2.0 - 1.0;
    let a = fz_h(c, k + 0.5) * 6.2831853;
    let r = sqrt(max(1.0 - z * z, 0.0));
    return vec3<f32>(r * cos(a), r * sin(a), z);
}

fn fz_rot(axis: vec3<f32>, angle: f32, v: vec3<f32>) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return v * c + cross(axis, v) * s + axis * dot(axis, v) * (1.0 - c);
}

// kh-app zetta_crumble.rs `timeline`: (released, split, its fragments' dissolve start and end), crumble units
fn fz_timeline(height: f32, r: f32, r2: f32) -> vec4<f32> {
    let release = 0.16 + 0.5 * (0.7 * (1.0 - clamp(height, 0.0, 1.0)) + 0.3 * clamp(r, 0.0, 1.0));
    let split = release + 0.1;
    let start = split + 0.06 + 0.08 * clamp(r2, 0.0, 1.0);
    let end = min(start + 0.18, 1.0);
    return vec4<f32>(release, split, min(start, end - 0.02), end);
}

// kh-app zetta_crumble.rs `crack_reveal`
fn fz_crack_reveal(dist: f32, r: f32) -> f32 {
    return 0.01 + 0.15 * clamp(dist, 0.0, 1.0) + 0.03 * clamp(r, 0.0, 1.0);
}

struct FzFly {
    p: vec3<f32>,
    v: vec3<f32>,
    land: f32,
}

// kh-app zetta_crumble.rs `fly` (metres, game z up, gravity 5.5): closed-form flight, landing, sliding to rest
fn fz_fly(p0: vec3<f32>, v0: vec3<f32>, floor_z: f32, t: f32) -> FzFly {
    // (round 6, the user: "the Zettaflare enemy's breaking effect pieces needs to travel along the path of the beam, not just
    // falling down") a little gravity only: the beam's force carries them (fz_swept)
    let g = 0.9;
    let h = max(p0.z - floor_z, 0.0);
    var land = 0.0;
    if (!(h <= 1.0e-5 && v0.z <= 0.0)) {
        land = (v0.z + sqrt(v0.z * v0.z + 2.0 * g * h)) / g;
    }
    var o: FzFly;
    o.land = land;
    if (t <= land) {
        o.p = p0 + v0 * t - vec3<f32>(0.0, 0.0, 0.5 * g * t * t);
        o.v = v0 - vec3<f32>(0.0, 0.0, g * t);
        return o;
    }
    let pc = p0 + v0 * land - vec3<f32>(0.0, 0.0, 0.5 * g * land * land);
    let vh = vec3<f32>(v0.x, v0.y, 0.0) * 0.45;
    let e = exp(-4.0 * (t - land));
    o.p = vec3<f32>(pc.x, pc.y, floor_z) + vh * ((1.0 - e) / 4.0);
    o.v = vh * e;
    return o;
}

// (round 4) A seed of the body's cell lattice (relative to its feet, cm).
fn fz_seed(c: vec3<f32>) -> vec3<f32> {
    return (c + vec3<f32>(0.15) + 0.7 * vec3<f32>(fz_h(c, 1.0), fz_h(c, 2.0), fz_h(c, 3.0))) * max(fz.f[33].y, 2.0);
}

struct FzCells {
    cell: vec3<f32>,
    cell2: vec3<f32>,
    // the distance (cm) to the border of its cell with the next one (the bisector of their seeds: exact for Voronoi)
    border: f32,
}

fn fz_cells(rel: vec3<f32>) -> FzCells {
    let base = floor(rel / max(fz.f[33].y, 2.0));
    var f1 = 1.0e9;
    var f2 = 1.0e9;
    var o: FzCells;
    o.cell = base;
    o.cell2 = base;
    for (var dz = -1; dz <= 1; dz++) {
        for (var dy = -1; dy <= 1; dy++) {
            for (var dx = -1; dx <= 1; dx++) {
                let c = base + vec3<f32>(f32(dx), f32(dy), f32(dz));
                let d = length(rel - fz_seed(c));
                if (d < f1) {
                    f2 = f1;
                    o.cell2 = o.cell;
                    f1 = d;
                    o.cell = c;
                } else if (d < f2) {
                    f2 = d;
                    o.cell2 = c;
                }
            }
        }
    }
    o.border = (f2 * f2 - f1 * f1) / max(2.0 * length(fz_seed(o.cell2) - fz_seed(o.cell)), 1.0e-3);
    return o;
}

// The fragment of a cell a point is in (0..2), its seed (rel, cm) and its distance to the fragment's border (cm).
fn fz_frag_of(cell: vec3<f32>, rel: vec3<f32>) -> vec4<f32> {
    let sd = fz_seed(cell);
    var best = 0.0;
    var d1 = 1.0e9;
    var d2 = 1.0e9;
    var s1 = sd;
    var s2 = sd;
    for (var j = 0; j < 3; j++) {
        let jf = f32(j);
        let sub = sd + (vec3<f32>(fz_h(cell, 10.0 + jf * 3.0), fz_h(cell, 11.0 + jf * 3.0), fz_h(cell, 12.0 + jf * 3.0)) - vec3<f32>(0.5)) * max(fz.f[33].y, 2.0) * 0.7;
        let d = length(rel - sub);
        if (d < d1) {
            d2 = d1;
            s2 = s1;
            d1 = d;
            s1 = sub;
            best = jf;
        } else if (d < d2) {
            d2 = d;
            s2 = sub;
        }
    }
    return vec4<f32>(best, (d2 * d2 - d1 * d1) / max(2.0 * length(s2 - s1), 1.0e-3), 0.0, 0.0);
}

fn fz_release(cell: vec3<f32>) -> f32 {
    return fz_timeline(clamp(fz_seed(cell).z / max(fz.f[35].w, 1.0), 0.0, 1.0), fz_h(cell, 4.0), 0.0).x;
}

/// The layers of a released piece: copies of its surface along the snapshot's view, so that edge-on it is a slab, not a sheet.
const FZ_LAYERS: u32 = 4u;

// (round 6) kh-app zetta_crumble.rs `swept`: a flight swept on along the beam by its force (1.6 m/s^2 along `sweep` = the push
// x the body's scale's square root), never under its floor
fn fz_swept(p0: vec3<f32>, v0: vec3<f32>, sweep: vec3<f32>, floor_z: f32, t: f32) -> FzFly {
    var o = fz_fly(p0, v0, floor_z, t);
    o.p = o.p + sweep * (0.5 * 1.6 * t * t);
    o.p.z = max(o.p.z, floor_z);
    o.v = o.v + sweep * (1.6 * t);
    return o;
}

struct FzFrag {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // x: 0 in place, 1 a chunk, 2 a fragment, 3 ash, 4 an ember; y: its view depth (m); z: its fragment (0..2); w: one
    // pixel's size there (cm)
    @location(1) @interpolate(flat) look: vec4<f32>,
    // its cell (lattice coordinates)
    @location(2) @interpolate(flat) cell: vec3<f32>,
    // the place on its body (relative to its feet at the snapshot, cm) under this point of the splat
    @location(3) rel: vec3<f32>,
}

@vertex
fn finale_frag_vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> FzFrag {
    var o: FzFrag;
    o.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    o.uv = vec2<f32>(0.0);
    o.look = vec4<f32>(0.0);
    o.cell = vec3<f32>(0.0);
    o.rel = vec3<f32>(0.0);
    let aw = u32(fz.f[32].w);
    let ah = u32(fz.f[33].x);
    if (aw == 0u || ii >= aw * ah * FZ_LAYERS) {
        return o;
    }
    let layer = ii / (aw * ah);
    let ti = ii % (aw * ah);
    let texel = vec2<i32>(i32(ti % aw), i32(ti / aw));
    let at = textureLoad(fz_atlas, texel, 0);
    if (at.w <= 0.0) {
        return o;
    }
    let k = clamp(fz.f[7].w, 0.0, 1.0);
    let s = max(fz.f[33].y, 2.0);
    let scale = max(fz.f[33].z, 0.1);
    let secs = fz.f[33].w;
    let push = fz.f[34].xyz;
    let feet = fz.f[35].xyz;
    let height = max(fz.f[35].w, 1.0);
    let rel = at.xyz;
    let cs = fz_cells(rel);
    let cell = cs.cell;
    let seed_rel = fz_seed(cell);
    let fr = fz_frag_of(cell, rel);
    let fj = fr.x;
    let fc_rel = seed_rel + (vec3<f32>(fz_h(cell, 10.0 + fj * 3.0), fz_h(cell, 11.0 + fj * 3.0), fz_h(cell, 12.0 + fj * 3.0)) - vec3<f32>(0.5)) * s * 0.7;
    let up = clamp(seed_rel.z / height, 0.0, 1.0);
    let tl = fz_timeline(up, fz_h(cell, 4.0), fz_h(cell, 20.0 + fj));
    if (k >= tl.w) {
        return o;
    }
    // the layer's place along the view at the snapshot: every piece (and the part still in place) is a slab a third of a cell
    // thick (layers closer than a splat: solid), so that a camera turning round it sees a chunk, not a sheet
    let thick = s * 0.35 * select(1.0, 0.8, k >= tl.y);
    let lay = f32(layer) / f32(FZ_LAYERS - 1u);
    let rel_l = rel + fz.f[37].xyz * (thick * lay);
    let cm = 0.01;
    let c_w = (feet + seed_rel) * cm;
    let p_w = (feet + rel_l) * cm;
    let fc_w = (feet + fc_rel) * cm;
    let floor_m = fz.f[34].w * cm;
    var world = p_w;
    // (pieces in flight are filled more densely: one solid chunk)
    var size = at.w * select(1.8, 2.3, k >= tl.x) * cm;
    var look = 0.0;
    // how a point of the splat (a world offset now, cm) maps back onto the body (rel), as the piece is turned now
    var axis1 = vec3<f32>(0.0, 0.0, 1.0);
    var ang1 = 0.0;
    var axis2 = vec3<f32>(0.0, 0.0, 1.0);
    var ang2 = 0.0;
    var shrink = 1.0;
    if (k >= tl.x) {
        let sq = sqrt(scale);
        let out_d = vec3<f32>(c_w.x - feet.x * cm, c_w.y - feet.y * cm, 0.0);
        let out = select(vec3<f32>(0.0), normalize(out_d), length(out_d) > 1.0e-5);
        // (round 6) carried down the beam: launched along it at 2.2..4 m/s, a slight spread off its axis, a little lift; without a
        // beam (push zero) out and up as before
        let beam = dot(push, push) > 0.5;
        var v0 = (out * (0.5 + 1.1 * fz_h(cell, 5.0)) + vec3<f32>(0.0, 0.0, 0.9 + 1.4 * fz_h(cell, 6.0)) + fz_rdir(cell, 8.0) * 0.35) * sq;
        if (beam) {
            v0 = (push * (2.2 + 1.8 * fz_h(cell, 7.0)) + out * (0.25 + 0.35 * fz_h(cell, 5.0)) + vec3<f32>(0.0, 0.0, 0.1 + 0.3 * fz_h(cell, 6.0)) + fz_rdir(cell, 8.0) * 0.3) * sq;
        }
        let sweep = push * sq;
        let axis = fz_rdir(cell, 9.0);
        let rate = 2.0 + 5.0 * fz_h(cell, 10.5);
        let csize = s * 0.5 * cm;
        look = 1.0;
        if (k < tl.y) {
            // the chunk flies
            let age = (k - tl.x) * secs;
            let fl = fz_swept(c_w, v0, sweep, floor_m + csize * 0.3, age);
            ang1 = rate * min(age, fl.land);
            axis1 = axis;
            world = fl.p + fz_rot(axis, ang1, p_w - c_w);
        } else {
            // its fragments
            look = 2.0;
            let age_s = (tl.y - tl.x) * secs;
            let fs = fz_swept(c_w, v0, sweep, floor_m + csize * 0.3, age_s);
            let ang0 = rate * min(age_s, fs.land);
            let to_frag = fc_w - c_w;
            let dv_d = select(vec3<f32>(0.0), normalize(to_frag), length(to_frag) > 1.0e-5);
            let dv = select((dv_d * 0.6 + fz_rdir(cell, 30.0 + fj) * 0.4 + vec3<f32>(0.0, 0.0, 0.3)) * sq, (dv_d * 0.35 + fz_rdir(cell, 30.0 + fj) * 0.3 + push * 0.8 + vec3<f32>(0.0, 0.0, 0.1)) * sq, beam);
            let faxis = fz_rdir(cell, 40.0 + fj);
            let frate = 3.0 + 6.0 * fz_h(cell, 50.0 + fj);
            let fsize = s * 0.25 * cm;
            let start = fs.p + fz_rot(axis, ang0, to_frag);
            let local0 = fz_rot(axis, ang0, p_w - fc_w);
            let age = (k - tl.y) * secs;
            let ff = fz_swept(start, fs.v + dv, sweep, floor_m + fsize * 0.25, age);
            shrink = 1.0 - smoothstep(tl.z, tl.w, k);
            axis1 = axis;
            ang1 = ang0;
            axis2 = faxis;
            ang2 = frate * min(age, ff.land);
            world = ff.p + fz_rot(faxis, ang2, local0) * shrink;
            size = size * shrink;
            // a few of its texels are its ash and embers: they leave it as it dissolves, drifting up in the light
            let ha = fz_hash(rel * 0.731 + vec3<f32>(fz.f[8].x * 3.0));
            if (ha < 0.07 && k >= tl.z) {
                if (layer > 0u) {
                    return o;
                }
                let end = min(tl.z + 0.12 + 0.1 * fz_h(cell, 70.0 + ha * 100.0), 1.0);
                if (k >= end) {
                    return o;
                }
                let age_d = (tl.z - tl.y) * secs;
                let fd0 = fz_swept(start, fs.v + dv, sweep, floor_m + fsize * 0.25, age_d);
                let pa = fd0.p + fz_rot(faxis, frate * min(age_d, fd0.land), local0);
                let t = (k - tl.z) * secs;
                // (round 6) its ash carried on down the beam, drifting up a little, into the light
                let va = (vec3<f32>(0.0, 0.0, 0.1 + 0.2 * fz_h(rel, 80.0)) + push * (1.2 + 0.8 * fz_h(rel, 83.0)) + fz_rdir(rel, 81.0) * 0.25) * sq;
                world = pa + va * t + (sweep * 1.6 + vec3<f32>(0.0, 0.0, 0.15)) * (0.5 * t * t);
                size = (0.012 + 0.02 * fz_h(rel, 82.0)) * scale * (1.0 - smoothstep(tl.z, end, k));
                look = select(3.0, 4.0, ha < 0.014);
            }
        }
    }
    if (size <= 0.0) {
        return o;
    }
    // a splat facing the camera, at its place now
    let corners = array<vec2<f32>, 6>(vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
    let q = corners[vi % 6u];
    let offset = (fz.f[1].xyz * q.x + fz.f[2].xyz * q.y) * (size * 50.0);
    let wcm = world * 100.0 + offset;
    o.position = fz.f[27] * wcm.x + fz.f[28] * wcm.y + fz.f[29] * wcm.z + fz.f[30];
    o.uv = q;
    let view_m = dot(wcm - fz.f[0].xyz, fz.f[3].xyz) / 100.0;
    o.look = vec4<f32>(look, view_m, fj, max(view_m * 100.0 * 2.0 * fz.f[2].w * fz.f[4].w, 1.0e-3));
    o.cell = cell;
    // the body point under this corner: the offset turned back (and grown back) into the body's frame
    o.rel = rel + fz_rot(axis1, -ang1, fz_rot(axis2, -ang2, offset)) / max(shrink, 1.0e-3);
    return o;
}

@fragment
fn finale_frag_ps(in: FzFrag) -> @location(0) vec4<f32> {
    let kind = in.look.x;
    // (round 4) its cell here and the screen-space rate of the distance to its border, taken first (uniform control flow):
    // the cracks and rims a constant number of pixels wide whatever the slant of the surface or of the border
    let cs = fz_cells(in.rel);
    let per_px = max(fwidth(cs.border), in.look.w * 0.05);
    if (kind > 2.5) {
        // ash, an ember
        if (dot(in.uv, in.uv) > 1.0) {
            discard;
        }
        if (fz.f[5].z > 0.5) {
            let dl = textureLoad(fz_live, fz_texel(in.position.xy), 0).x;
            if (dl > 0.0 && 1.0 / (dl * fz.f[5].x + fz.f[5].y) < in.look.y - 0.05) {
                discard;
            }
        }
        if (fz.f[38].x > 0.5) {
            return select(vec4<f32>(1.0, 0.0, 0.0, 1.0), vec4<f32>(0.0, 1.0, 0.0, 1.0), kind > 3.5);
        }
        return select(vec4<f32>(0.0, 0.0, 0.0, 1.0), vec4<f32>(fz.f[36].xyz, 1.0), kind > 3.5);
    }
    if (dot(in.uv, in.uv) > 1.0) {
        discard;
    }
    // behind what is nearer now (Sora, the party, the world)
    if (fz.f[5].z > 0.5) {
        let dl = textureLoad(fz_live, fz_texel(in.position.xy), 0).x;
        if (dl > 0.0 && 1.0 / (dl * fz.f[5].x + fz.f[5].y) < in.look.y - 0.05) {
            discard;
        }
    }
    let k = clamp(fz.f[7].w, 0.0, 1.0);
    let px = in.look.w;
    if (kind < 0.5) {
        // IN PLACE: a point of a cell already gone is not drawn (the clean broken edge); the cracks: continuous lines of a
        // constant width on the screen along the cells' borders, anti-aliased, glowing, widening just before they let go
        if (k >= fz_release(cs.cell)) {
            discard;
        }
        let gone = min(fz_release(cs.cell), fz_release(cs.cell2));
        let feet = fz.f[35].xyz;
        let height = max(fz.f[35].w, 1.0);
        let origin = feet + vec3<f32>(0.0, 0.0, 0.62 * height) - fz.f[34].xyz * fz.f[6].w;
        let reveal = fz_crack_reveal(length(feet + in.rel - origin) / max(height * 0.9, 1.0), fz_h(cs.cell + cs.cell2, 60.0));
        let grow = smoothstep(reveal, reveal + 0.05, k);
        let open = 1.0 + 1.6 * smoothstep(gone - 0.08, gone, k);
        let d = cs.border / per_px;
        let half_width = 0.6 * open * grow;
        let line = 1.0 - smoothstep(half_width - 0.6, half_width + 0.6, d);
        let halo = exp(-d / (0.7 * open)) * 0.12 * grow;
        let glow = clamp(max(line, halo), 0.0, 1.0);
        if (fz.f[38].x > 0.5) {
            return vec4<f32>(1.0, glow, 0.0, 1.0);
        }
        return vec4<f32>(fz.f[36].w * vec3<f32>(1.0, 0.92, 0.8) * glow, 1.0);
    }
    // A PIECE: only its own cell (and fragment) - its broken edges clean - one solid black chunk with a darker rim (its
    // face a touch lifted from black: its edge reads as an edge)
    if (any(cs.cell != in.cell)) {
        discard;
    }
    var border = cs.border;
    if (kind > 1.5) {
        let f = fz_frag_of(in.cell, in.rel);
        if (f.x != in.look.z) {
            discard;
        }
        border = min(border, f.y);
    }
    let rim = smoothstep(0.8, 2.2, border / per_px);
    if (fz.f[38].x > 0.5) {
        return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }
    return vec4<f32>(vec3<f32>(fz.f[3].w * 0.012 * rim), 1.0);
}

// ---- (round 5) THE BEAM'S COLOUR (the user's style reference, build/sora_tmp/party/ZETTA/ref/user_silhouette_style_ref.png;
// kh-app zetta_crumble.rs OVERLAY_WGSL mirrored): the silhouette and its pieces are first written into a MASK (t6: r its body,
// g its light) and this pass colours it over the finished picture (after our effects' bloom and screen passes): a deep core,
// a lighter edge (blended by the coverage of a 32-tap disk of the band's radius), mottled by low-frequency noise, a pale rim
// where the light bites in, a ragged eroding outline (the lookup warped by noise; what erodes shows the light, never the
// body), its cracks and embers the light's colour. Its colours are display-linear (as the test level's overlay): turned into
// the target's units through the inverse of the ACES fit KH3's picture is made with, times the light's scale.
@group(1) @binding(6) var fz_maskt: texture_2d<f32>;

fn fz_hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn fz_vnoise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    return mix(mix(fz_hash2(i), fz_hash2(i + vec2<f32>(1.0, 0.0)), u.x), mix(fz_hash2(i + vec2<f32>(0.0, 1.0)), fz_hash2(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

fn fz_fbm2(p: vec2<f32>) -> f32 {
    return (0.5 * fz_vnoise2(p) + 0.25 * fz_vnoise2(p * 2.03) + 0.125 * fz_vnoise2(p * 4.11)) / 0.875;
}

fn fz_mask_at(px: vec2<f32>) -> vec4<f32> {
    let dims = vec2<i32>(textureDimensions(fz_maskt));
    return textureLoad(fz_maskt, clamp(vec2<i32>(floor(px)), vec2<i32>(0), dims - vec2<i32>(1)), 0);
}

// the ACES fit's inverse (Narkowicz: y = x (2.51 x + 0.03) / (x (2.43 x + 0.59) + 0.14)), per channel
fn fz_inv_aces(y: vec3<f32>) -> vec3<f32> {
    let c = clamp(y, vec3<f32>(0.0), vec3<f32>(1.0));
    let a = 2.43 * c - vec3<f32>(2.51);
    let b = 0.59 * c - vec3<f32>(0.03);
    let k = 0.14 * c;
    return (-b - sqrt(max(b * b - 4.0 * a * k, vec3<f32>(0.0)))) / (2.0 * a);
}

// (ZFIN round 9, the user: Zettaflare "needs to look PERFECT like the reference images taken from the test level") THE LOOK
// OF THE BEAM'S COLOUR at a pixel of the TARGET (the mask's size): (the display colour, alpha); alpha < 0: nothing of it near.
// kh-app zetta_crumble.rs OVERLAY_WGSL, line for line (the test level draws it as a UI layer: display colours after its tone
// mapper and bloom).
fn fz_look(px: vec2<f32>) -> vec4<f32> {
    let band = max(fz.f[38].y, 1.0);
    // nothing of it near: out at once
    var near = fz_mask_at(px).r + fz_mask_at(px).g;
    for (var i = 0; i < 6; i++) {
        let a = f32(i) * 1.0471976;
        let t = fz_mask_at(px + vec2<f32>(cos(a), sin(a)) * band * 2.6);
        near += t.r + t.g;
    }
    if (near <= 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    // the noise at the band's own scale: a ragged, eroding outline and a mottled fill
    let q = px / max(band * 2.2, 1.0);
    // (round 6, the user: "make the transition from the first to the second much smoother, no full black suddenly popping in")
    // its look grows in with ScriptFinale::dark: the outline goes ragged as it darkens, from the light's colour
    let kd = clamp(fz.f[38].z, 0.0, 1.0);
    let warp = (vec2<f32>(fz_fbm2(q), fz_fbm2(q + vec2<f32>(17.3, 5.1))) - vec2<f32>(0.5)) * band * 1.8 * kd;
    let at = px + warp;
    let m = fz_mask_at(at);
    // the share of the silhouette round this pixel within the band (soft, continuous)
    var cov = 0.0;
    for (var i = 0; i < 16; i++) {
        let a = f32(i) * 0.3926991;
        let d = vec2<f32>(cos(a), sin(a)) * band;
        cov += fz_mask_at(at + d).r + fz_mask_at(at + d * 0.5).r;
    }
    cov = cov / 32.0;
    let ragged = m.r * 0.6 + cov * 0.4 + (fz_fbm2(q * 1.7 + vec2<f32>(3.7, 9.2)) - 0.5) * 0.38 * kd;
    let inside = smoothstep(0.42, 0.56, ragged);
    let deep = smoothstep(0.5, 1.0, cov);
    var col = mix(fz.f[40].xyz, fz.f[39].xyz, deep) * (0.88 + 0.24 * fz_fbm2(q * 0.55 + vec2<f32>(11.0, 2.0)));
    let rim = (1.0 - smoothstep(0.5, 0.8, cov)) * (0.75 + 0.25 * fz_fbm2(q * 2.3));
    col = mix(col, fz.f[41].xyz, clamp(rim, 0.0, 1.0) * 0.9 * kd);
    let body = max(smoothstep(0.3, 0.6, m.r), smoothstep(0.03, 0.2, cov));
    col = mix(fz.f[41].xyz, col, inside);
    col = mix(col, fz.f[41].xyz, m.g);
    // from what the body showed (lit, washed white): pale first, its colour coming in with kd, the layer itself fading in
    col = mix(fz.f[41].xyz, col, kd);
    let alpha = max(max(inside, body), m.g) * smoothstep(0.0, 0.3, kd);
    return vec4<f32>(col, alpha);
}

// never over what was drawn in front of it since the G-buffer (Sora, the party, an opaque effect: the scene depth now nearer
// than the snapshot's)
fn fz_hidden(px: vec2<f32>) -> bool {
    if (fz.f[5].z > 0.5 && fz.f[5].w > 0.5) {
        let t = fz_texel(px);
        return textureLoad(fz_live, t, 0).x > textureLoad(fz_snap, t, 0).x;
    }
    return false;
}

// The comp in the HDR scene before the game's tone mapper (the fallback): the display colour through the ACES fit's inverse.
@fragment
fn finale_comp_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let look = fz_look(position.xy);
    if (look.w <= 0.0 || fz_hidden(position.xy)) {
        discard;
    }
    return vec4<f32>(fz_inv_aces(look.xyz) * fz.f[39].w, look.w);
}

// (ZFIN round 9) where the comp must not show (in front of the target since the G-buffer), written into the mask's blue
// (max blend) in the late pass, for the display pass at the game's Present (where the scene depth can no longer be read)
@fragment
fn finale_occl_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    if (!fz_hidden(position.xy)) {
        discard;
    }
    return vec4<f32>(0.0, 0.0, 1.0, 0.0);
}

// (ZFIN round 9) THE COMP AT THE GAME'S PRESENT, in DISPLAY colours over the finished picture (after the game's tone mapper,
// colour grading and bloom, as the test level's UI layer): exactly the test level's colours whatever the game's tone curve.
// f[42]: x 1 = the target takes linear values (an sRGB view), 0 = display-encoded values; yz the mask's pixels per back-buffer
// pixel.
fn fz_srgb(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055), x * 12.92, x <= vec3<f32>(0.0031308));
}

@fragment
fn finale_disp_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let px = position.xy * max(fz.f[42].yz, vec2<f32>(1.0e-4));
    // (the testing aid KH_FZ_MASK) the mask itself
    if (fz.f[42].w > 0.5) {
        return vec4<f32>(fz_mask_at(px).rgb, 1.0);
    }
    if (fz_mask_at(px).b > 0.5) {
        discard;
    }
    let look = fz_look(px);
    if (look.w <= 0.0) {
        discard;
    }
    return vec4<f32>(select(fz_srgb(look.xyz), clamp(look.xyz, vec3<f32>(0.0), vec3<f32>(1.0)), fz.f[42].x > 0.5), look.w);
}
#endif

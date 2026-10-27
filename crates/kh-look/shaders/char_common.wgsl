// THE SHARED HARNESS OF THE CHARACTER MATERIALS (crates/kh-look; spliced by every host in front
// of its own fragment entry: kh-app/src/char_material.wgsl for the Bevy test app,
// kh-er-render/shaders/er_char.wgsl for the Elden Ring renderer).
//
// `kh_material(KhIn) -> KhOut` is a 1:1 translation of the game's own compiled base-pass pixel
// shader of a character material (PACK/materials/materials.wgsl, tools/sora_material_pack.js),
// selected by the shader definition KH_C<owner>; `kh_mask(KhIn)` is its depth-only shader (the
// opacity mask of a masked material). The game shades deferred: the base pass writes the
// G-buffer and the emissive + indirect term, the engine's light shader adds the lights. Here
// both run in one forward fragment (`kh_shade`):
//   1. kh_material   -> o0 emissive + indirect diffuse (the SH of the indirect lighting cache),
//                       o1 world normal * 0.5 + 0.5, o2 (metallic, specular, roughness, model),
//                       o3 base colour, o4 (sqrt subsurface colour, opacity), o6 the fork's extra
//   2. kh_light      -> TDeferredLightPS of the game's global shader cache for a directional
//                       light, written out by hand from its disassembly (the BRDF is stock UE 4.17:
//                       Lambert + GGX / Smith joint approx / Schlick, and for shading model 2 the
//                       stock Subsurface transmission term). The fork's shader has TWO outputs: the
//                       scene colour and, with the same value, the separate light buffer
//                       "SQEXLightMap" that the post-process material below reads.
//   3. kh_khs        -> the post-process material Shader/KHS_set/postprocess/KHS (blendable
//                       location BL_SQEXAfterLight: after the lights, before the reflection
//                       environment), translated like the base pass. It composes the pixel from
//                       the G-buffer, the light buffer, the base pass's emissive + indirect term
//                       and the two parameter collections; every texture it samples is, for a
//                       pixel of the character, a value of this same surface, so it is evaluated
//                       here per pixel instead of as a fullscreen pass (its inputs: the s* fields
//                       of KhIn, filled in `kh_shade`).
//   4. kh_reflection -> the reflection environment (after KHS, as in the renderer's order).
// Inputs carry the names of the D3D semantics and constant-buffer slots the code reads (View /
// Primitive / PrecomputedLightingBuffer rows by slot, the fork's layout). Positions and
// directions handed to the material are in the game's space (cm, Z up).
//
// A host declares: `kh` (uniform KhParams), the textures kh_t0 .. kh_t13, kh_tclamp (the
// post-process material's colour table), the samplers kh_swrap / kh_sclamp, the cube kh_tenv with
// kh_senv; it fills a KhIn (interpolants, View, Primitive, the SH rows) and calls `kh_shade`.
// Shader definitions: KH_C<owner>, KH_MASKED, KH_TRANSLUCENT, KH_SUBSURFACE.

struct KhParams {
    // The material constant buffer (uniform expressions of the shader map, evaluated on the CPU).
    m: array<vec4<f32>, 160>,
    // Material parameter collection 0 (KHS_Control).
    c0: array<vec4<f32>, 48>,
    // The post-process material's own collection (KHS_RefKit_MatParamCol): its 18 rows.
    k0: array<vec4<f32>, 18>,
    // Primitive: WorldToLocal rows (game space), object position + radius, box extent, actor
    // position, local bounds min / max.
    w2l: array<vec4<f32>, 4>,
    obj: vec4<f32>,
    bounds: vec4<f32>,
    actor: vec4<f32>,
    bmin: vec4<f32>,
    bmax: vec4<f32>,
    // PrecomputedLightingBuffer rows 8..15: the indirect lighting cache's SH (three bands).
    sh: array<vec4<f32>, 8>,
    // xyz: direction to the key light (game space); w: 1 = this light and `sh` are used as given
    // (the menu's rig), 0 = direction and shadow of the scene's first directional light.
    light_dir: vec4<f32>,
    // rgb: the light's colour x intensity in the engine's units (a white Lambert surface facing
    // it shows rgb / pi), or with look.y = 1 a scale of the scene's first directional light as
    // the camera exposes it; w: 1 = receive the scene's shadow.
    light_color: vec4<f32>,
    // x: unused, y: the game clock (s), z: 1 = filmic tone curve in the shader, w: opacity scale.
    misc: vec4<f32>,
    // x: the exposure in front of the camera's tone curve (1 = the camera's own, as the level);
    // y: 1 = light_color.rgb scales the scene's directional light (0 = it is the light);
    // z: 1 = `sh` scales the scene's ambient light (0 = it is the indirect lighting as given).
    look: vec4<f32>,
    // x: 1 = the KHS post-process material composes the pixel, y: the primitive's discernment
    // mask / 65536 (the fork's seventh target, alpha), z: ScreenSpaceAO, w: unused.
    khs: vec4<f32>,
    // The environment cube: x its last mip, y its scale.
    env: vec4<f32>,
    // Spot lights of a rig of its own (light_dir.w = 1): xyz position (game space, cm), w the
    // intensity x 16 (inverse-squared falloff, engine units); xyz the axis it points along, w the
    // attenuation radius; x cos(outer cone), y 1 / (cos(inner) - cos(outer)), z MinRoughness.
    spot_pos: array<vec4<f32>, 2>,
    spot_dir: array<vec4<f32>, 2>,
    spot_cone: array<vec4<f32>, 2>,
    // (game over) Point lights of the rig (a particle light of an effect, exponent falloff): xyz position
    // (game space, cm), w the radius (0 = none); rgb colour x brightness, w the falloff exponent.
    point_pos: array<vec4<f32>, 4>,
    point_col: array<vec4<f32>, 4>,
    // (NF) KH3's camera fade of the character this material belongs to (kh-camera camfade.rs): x = 1 - his CameraFade value
    // (0 = whole), y = the screen door's phase. APPENDED: a shader built before it reads the block as before. (The Elden Ring
    // renderer carries the same two numbers in its frame block, er_char.wgsl `er_camfade_keep`, so its velocity pass has them.)
    // (HAIRPERF) z = 1: the ER91 hair's coverage at one point a pixel (kh91_cover), the character being big on screen.
    fade: vec4<f32>,
}

struct KhIn {
    tc0: vec4<f32>, tc1: vec4<f32>, tc2: vec4<f32>, tc3: vec4<f32>, tc7: vec4<f32>, tc9: vec4<f32>, tc10: vec4<f32>, tc11: vec4<f32>,
    col0: vec4<f32>, svpos: vec4<f32>, front: vec4<f32>,
    // Translated world position (relative to the camera), game space.
    twp: vec3<f32>,
    v0: vec4<f32>, v1: vec4<f32>, v2: vec4<f32>, v3: vec4<f32>, v8: vec4<f32>, v9: vec4<f32>, v10: vec4<f32>, v26: vec4<f32>, v28: vec4<f32>,
    // (XW) ViewToTranslatedWorld rows 12..14 (the view axes in the world): read only by the XS form weapons' translations
    v12: vec4<f32>, v13: vec4<f32>, v14: vec4<f32>,
    v36: vec4<f32>, v37: vec4<f32>, v38: vec4<f32>, v39: vec4<f32>, v53: vec4<f32>, v54: vec4<f32>, v55: vec4<f32>, v58: vec4<f32>,
    v117: vec4<f32>, v118: vec4<f32>, v119: vec4<f32>, v122: vec4<f32>, v123: vec4<f32>, v124: vec4<f32>, v125: vec4<f32>, v126: vec4<f32>,
    v127: vec4<f32>, v128: vec4<f32>, v129: vec4<f32>, v130: vec4<f32>, v137: vec4<f32>, v141: vec4<f32>, v142: vec4<f32>, v151: vec4<f32>, v160: vec4<f32>,
    v161: vec4<f32>, v172: vec4<f32>,
    p0: vec4<f32>, p1: vec4<f32>, p2: vec4<f32>, p3: vec4<f32>, p4: vec4<f32>, p5: vec4<f32>, p6: vec4<f32>, p7: vec4<f32>, p8: vec4<f32>,
    p9: vec4<f32>, p10: vec4<f32>, p11: vec4<f32>, p15: vec4<f32>, p16: vec4<f32>, p17: vec4<f32>, p18: vec4<f32>,
    l4: vec4<f32>, l5: vec4<f32>, l8: vec4<f32>, l9: vec4<f32>, l10: vec4<f32>, l11: vec4<f32>, l12: vec4<f32>, l13: vec4<f32>, l14: vec4<f32>, l15: vec4<f32>,
    // The scene textures of the post-process material at this pixel: GBufferA..D, the fork's
    // seventh target, SceneDepth (device z; with v53 = (0, 0, 1, 0) the value 1 / depth in cm),
    // ScreenSpaceAO, SQEXLightMap, SQEXBackupSceneColor, SQEXAmbientColor,
    // SQEXIndirectLightWithEmissive.
    sa: vec4<f32>, sb: vec4<f32>, sc: vec4<f32>, sd: vec4<f32>, sf: vec4<f32>, sdepth: vec4<f32>, sao: vec4<f32>, slight: vec4<f32>,
    sbackup: vec4<f32>, samb: vec4<f32>, sind: vec4<f32>,
}

struct KhOut {
    o0: vec4<f32>,
    o1: vec4<f32>,
    o2: vec4<f32>,
    o3: vec4<f32>,
    o4: vec4<f32>,
    o5: vec4<f32>,
    o6: vec4<f32>,
}

// A quotient that is 0 where the divisor is 0 (the post-process material converts colours to hue
// and saturation and divides 0 by 0 for black and grey; Direct3D then drops the NaN in the next
// min / max / saturate, which other back ends need not do).
fn kh_div1(a: f32, b: f32) -> f32 { return select(a / b, 0.0, b == 0.0); }
fn kh_div2(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> { return select(a / b, vec2<f32>(0.0), b == vec2<f32>(0.0)); }
fn kh_div3(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> { return select(a / b, vec3<f32>(0.0), b == vec3<f32>(0.0)); }
fn kh_div4(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> { return select(a / b, vec4<f32>(0.0), b == vec4<f32>(0.0)); }

// UE4's filmic tone curve as the ACES fit by Stephen Hill (the game's own curve parameters
// were not read from the executable), then the inverse of a Reinhard curve so that the
// camera's tone mapper (which this app keeps for the level) shows about that colour.
fn kh_aces(c: vec3<f32>) -> vec3<f32> {
    let m_in = mat3x3<f32>(vec3<f32>(0.59719, 0.07600, 0.02840), vec3<f32>(0.35458, 0.90834, 0.13383), vec3<f32>(0.04823, 0.01566, 0.83777));
    let m_out = mat3x3<f32>(vec3<f32>(1.60475, -0.10208, -0.00327), vec3<f32>(-0.53108, 1.10813, -0.07276), vec3<f32>(-0.07367, -0.00605, 1.07602));
    let v = m_in * c;
    let a = v * (v + vec3<f32>(0.0245786)) - vec3<f32>(0.000090537);
    let b = v * (0.983729 * v + vec3<f32>(0.4329510)) + vec3<f32>(0.238081);
    return saturate(m_out * (a / b));
}

// TDeferredLightPS (directional, no IES, no lighting channels) of the global shader cache for
// one pixel of the G-buffer `g`: `n` world normal, `v` unit vector to the camera, `l` unit
// vector to the light, `shadow` the light's shadow factor. Returns the light's contribution
// before the light colour (multiply by the colour x intensity).
fn kh_light(g: KhOut, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, shadow: f32, min_roughness: f32) -> vec3<f32> {
    let pi = 3.141593;
    let metallic = g.o2.x;
    let specular = g.o2.y;
    // (the light's MinRoughness; DeferredLightUniforms row 4 .x)
    let roughness = max(g.o2.z, min_roughness);
    let base = g.o3.xyz;
    let diffuse_color = base - base * metallic;
    let specular_color = mix(vec3<f32>(0.08 * specular), base, metallic);
    let n_l0 = dot(n, l);
    let n_v0 = dot(n, v);
    let v_l = dot(l, v);
    let inv_len_h = inverseSqrt(2.0 + 2.0 * v_l);
    let n_h = saturate((n_l0 + n_v0) * inv_len_h);
    let v_h = saturate(inv_len_h + inv_len_h * v_l);
    let n_l = saturate(n_l0);
    let n_v = min(abs(n_v0) + 0.00001, 1.0);
    let a = roughness * roughness;
    // Vis_SmithJointApprox
    let vis_v = n_l * (n_v * (1.0 - a) + a);
    let vis_l = n_v * (n_l * (1.0 - a) + a);
    let vis = 0.5 / (vis_v + vis_l);
    // D_GGX
    let a2 = a * a;
    let d0 = (n_h * a2 - n_h) * n_h + 1.0;
    let d = a2 / (pi * d0 * d0);
    // F_Schlick
    let fc0 = 1.0 - v_h;
    let fc = fc0 * fc0 * fc0 * fc0 * fc0;
    let f = saturate(50.0 * specular_color.y) * fc + (1.0 - fc) * specular_color;
    let surface = diffuse_color * 0.318310 + d * vis * f;
    var result = surface * (n_l * shadow);
#ifdef KH_SUBSURFACE
    // Shading model 2 (Subsurface): the transmission term.
    let subsurface_color = g.o4.xyz * g.o4.xyz;
    let opacity = g.o4.w;
    let h = normalize(v + l);
    let in_scatter = pow(saturate(dot(l, -v)), 12.0) * (opacity * -2.9 + 3.0);
    let opacity_factor = saturate(dot(n, h) * opacity + 1.0 - opacity);
    let back_scatter = opacity_factor * 0.159155;
    result += subsurface_color * (in_scatter * (1.0 - back_scatter) + back_scatter) * shadow;
#endif
    return result;
}

// The specular part of the image-based lighting: the engine's reflection capture look-up (mip
// from roughness: ComputeReflectionCaptureMipFromRoughness, roughest mip 1, scale 1.2) against
// the instance's environment cube, times the environment BRDF. NOT the game's: the cube itself
// (generated, see kh-look), EnvBRDFApprox in place of the preintegrated table, no
// screen-space reflections, no fork attenuation (SqexReflectionAttenuation*).
fn kh_reflection(g: KhOut, n: vec3<f32>, v: vec3<f32>) -> vec3<f32> {
    let base = g.o3.xyz;
    let specular_color = mix(vec3<f32>(0.08 * g.o2.y), base, g.o2.x);
    let roughness = g.o2.z;
    let n_v = saturate(dot(n, v));
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * n_v)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    let brdf = specular_color * ab.x + vec3<f32>(saturate(50.0 * specular_color.y) * ab.y);
    let refl = reflect(-v, n);
    let level_from_1x1 = 1.0 - 1.2 * log2(max(roughness, 0.001));
    let mip = kh.env.x - 1.0 - level_from_1x1;
    // (the cube is laid out in the axes of a Y-up host)
    let env = textureSampleLevel(kh_tenv, kh_senv, vec3<f32>(refl.x, refl.z, refl.y), max(mip, 0.0)).xyz * kh.env.y * vec3<f32>(1.0 + kh.env.z, 1.0, 1.0 + kh.env.w);
    return env * brdf;
}

// What `kh_shade` gives a host: the pixel's colour (scene linear x the exposure, or with
// misc.z the earlier in-shader curve) and alpha, and the coverage of the four samples of a
// masked material (15 for the others).
struct KhShaded {
    color: vec4<f32>,
    cover: u32,
}

#ifdef KH_HAIR91
// (ER91, the user: "the tips of Sora's hair ... needs to look like this image") THE HAIR'S COVERAGE, CONTINUOUS AND
// SUPERSAMPLED. KH3 shows its hair masks' DitherTemporalAA screen door (phase d over 5 steps) averaged by its TAA; the
// resolved coverage of a pixel is the share of d in [-1/12, 3/4] for which the mask passes. The ER91 translation
// (er91/tools/hair91.js) takes d from the host (i.v137.y = 1, d = i.v137.z) and, with i.v137.w = 1, returns the final
// test's margin (4 + value - clip; 0 = cut by a test that does not depend on d). The margin rises with d, so the
// crossing is found by bisection (fixed steps: every pixel of a quad runs the same evaluations) and interpolated: a
// continuous coverage, no staircase. It is taken at four sub-pixel positions (a rotated grid, the interpolants moved
// by their screen derivatives), so a strand end thinner than a pixel covers its true share of it: no dotted rows, no
// sparkle, the same every frame.
fn kh91_margin(i_in: KhIn, d: f32) -> f32 {
    var i = i_in;
    i.v137 = vec4<f32>(0.0, 1.0, d, 1.0);
    let m = kh_mask(i);
    return select(m - 4.0, -1.0, m == 0.0);
}

fn kh91_cover_at(i: KhIn) -> f32 {
    let d_lo = -1.0 / 12.0;
    let d_hi = 0.75;
    let m_lo = kh91_margin(i, d_lo);
    let m_hi = kh91_margin(i, d_hi);
    var lo = d_lo;
    var hi = d_hi;
    var mlo = m_lo;
    var mhi = m_hi;
    for (var k = 0; k < 3; k++) {
        let mid = 0.5 * (lo + hi);
        let mm = kh91_margin(i, mid);
        let ok = mm >= 0.0;
        hi = select(hi, mid, ok);
        mhi = select(mhi, mm, ok);
        lo = select(mid, lo, ok);
        mlo = select(mm, mlo, ok);
    }
    let t = clamp(-mlo / max(mhi - mlo, 1.0e-6), 0.0, 1.0);
    let d_cross = lo + (hi - lo) * t;
    let c = clamp((d_hi - d_cross) / (d_hi - d_lo), 0.0, 1.0);
    return select(select(c, 1.0, m_lo >= 0.0), 0.0, m_hi < 0.0);
}

fn kh91_cover(i_in: KhIn) -> f32 {
    // (HAIRPERF, the user: "the game also lags badly when you zoom into Sora's hair") kh.fade.z = 1: he is big on screen (the
    // host's `r.hair_point_px`), a strand end spans several pixels and the coverage at the pixel's centre is the same: one
    // point instead of four (the mask taken 5 times a pixel, not 20). A uniform: the whole draw takes one branch.
    if (kh.fade.z > 0.5) {
        return kh91_cover_at(i_in);
    }
    let g0x = dpdx(i_in.tc0); let g0y = dpdy(i_in.tc0);
    let g1x = dpdx(i_in.tc1); let g1y = dpdy(i_in.tc1);
    let g2x = dpdx(i_in.tc2); let g2y = dpdy(i_in.tc2);
    let gwx = dpdx(i_in.twp); let gwy = dpdy(i_in.twp);
    var sum = 0.0;
    for (var s = 0; s < 4; s++) {
        var o = vec2<f32>(-0.125, -0.375);
        if (s == 1) { o = vec2<f32>(0.375, -0.125); }
        if (s == 2) { o = vec2<f32>(0.125, 0.375); }
        if (s == 3) { o = vec2<f32>(-0.375, 0.125); }
        var i = i_in;
        i.tc0 = i_in.tc0 + g0x * o.x + g0y * o.y;
        i.tc1 = i_in.tc1 + g1x * o.x + g1y * o.y;
        i.tc2 = i_in.tc2 + g2x * o.x + g2y * o.y;
        i.tc3 = i.tc2;
        i.twp = i_in.twp + gwx * o.x + gwy * o.y;
        i.tc9 = vec4<f32>(i.twp, 1.0);
        i.svpos = vec4<f32>(i_in.svpos.xy + o, i_in.svpos.zw);
        sum += kh91_cover_at(i);
    }
    return sum * 0.25;
}
#endif

// (NF) KH3'S CAMERA FADE (TresCameraManager's shut-out fade, kh-camera camfade.rs; sheets/port/sora-camera-fade.json): the
// masked character materials' own mask test (materials.wgsl kh_mask, first test: DitherTemporalAA) against the character's
// CameraFade value: the pixel stays where frac((x + 2 y + 1 + phase) / 6) x 6/7 + value - 5/6 >= 0 (camfade.dither_phases,
// camfade.dither_scale, camfade.dither_bias; the + 1 is the pixel centre's trunc). Value 0.5 keeps three phases of six.
// (NF, camfade.screen_door) the pixels kept are that SHARE laid out as a still 4 x 4 ordered matrix (a 1-pixel checkerboard at
// 0.5), shifted by the phase (kh-camera camfade.rs `keeps`; er_char.wgsl er_camfade_keep is the same).
fn kh_camfade_keep(position: vec2<f32>, value: f32, phase: f32) -> bool {
    if (value >= 1.0) {
        return true;
    }
    var kept = 0u;
    for (var k = 0u; k < 6u; k++) {
        if (f32(k) / 6.0 * 0.857143 + value - 0.8333 >= 0.0) {
            kept += 1u;
        }
    }
    let ph = u32(max(phase, 0.0));
    let p = vec2<u32>(max(floor(position), vec2<f32>(0.0))) + vec2<u32>(ph, 3u * ph);
    let a = vec2<u32>(p.x & 1u, p.y & 1u);
    let b = vec2<u32>((p.x >> 1u) & 1u, (p.y >> 1u) & 1u);
    let cell = 4u * (((a.x ^ a.y) & 1u) * 2u + a.y) + ((b.x ^ b.y) & 1u) * 2u + b.y;
    return (f32(cell) + 0.5) / 16.0 < f32(kept) / 6.0;
}

// (NF) the same as multisample coverage (the test level: 4x MSAA, no TAA): of the four samples the share of the six phases
// that keep the pixel (camfade.rs `coverage`), turned by the pixel so that neighbours take other samples.
fn kh_camfade_cover(position: vec2<f32>, value: f32) -> u32 {
    if (value >= 1.0) {
        return 15u;
    }
    var kept = 0u;
    for (var k = 0u; k < 6u; k++) {
        if (f32(k) / 6.0 * 0.857143 + value - 0.8333 >= 0.0) {
            kept += 1u;
        }
    }
    let samples = u32(round(f32(kept) / 6.0 * 4.0));
    if (samples == 0u) {
        return 0u;
    }
    if (samples >= 4u) {
        return 15u;
    }
    let p = vec2<u32>(max(floor(position), vec2<f32>(0.0)));
    let turn = (p.x + 2u * p.y) % 4u;
    let bits = (1u << samples) - 1u;
    return ((bits << turn) | (bits >> (4u - turn))) & 15u;
}

// One pixel of a character material. `i_in`: the interpolants, View, Primitive and SH rows as the
// host filled them; `key_dir`: unit vector to the key light (game space) and `key_color` its
// colour x intensity, `shadow` its shadow factor (used when the material has no rig of its own:
// light_dir.w = 0); `camera`: the camera's world position (game space, cm).
fn kh_shade(i_in: KhIn, key_dir: vec3<f32>, key_color: vec3<f32>, shadow: f32, camera: vec3<f32>) -> KhShaded {
    var i = i_in;
    var shaded: KhShaded;
    shaded.cover = 15u;
#ifdef KH_MASKED
#ifdef KH_HAIR
    // (ER87, a host's hair path: kh-er-render er_char.wgsl KH_HAIR_CORE / KH_HAIR_FRINGE) KH3's hair masks are a
    // DitherTemporalAA screen door: the phase is (x + 2 y) mod 5 (mod 6 for the fur's), and the game's TAA shows their
    // average. Here the phases are all taken, so `cover` = how many pass is the resolved coverage of this pixel, with no
    // frame-to-frame pattern: the hair's masks take their phase d from the harness (i.v137.y = 1, d = i.v137.z; the
    // host's patched translation) as 16 even steps over the five phases' range (-1/12 .. 3/4: the 5-step staircase made
    // fine); KH_HAIR6 (the fur's mod-6 dither, a mask without that input): the pixel position stepped by one pixel per
    // phase, six phases.
#ifdef KH_HAIR91
    // (ER91) the continuous, supersampled coverage (kh91_cover) in 1/1024 steps (er_char.wgsl KH_HAIR91)
#ifdef KH_HAIR91_POINT
    // (the 4x pass shaded per sample: the coverage at this sample's point)
    var cover = u32(round(kh91_cover_at(i) * 1024.0));
#else
    var cover = u32(round(kh91_cover(i) * 1024.0));
#endif
#else
    var cover = 0u;
#endif
#ifdef KH_HAIR91
#else
#ifdef KH_HAIR6
    let x0 = i.svpos.x;
    i.v137 = vec4<f32>(0.0);
    for (var k = 0u; k < 6u; k++) {
        i.svpos.x = x0 + f32(k);
        if (kh_mask(i) != 0.0) {
            cover += 1u;
        }
    }
    i.svpos.x = x0;
#else
    for (var k = 0u; k < 16u; k++) {
        i.v137 = vec4<f32>(0.0, 1.0, (f32(k) + 0.5) / 16.0 * (5.0 / 6.0) - 1.0 / 12.0, 0.0);
        if (kh_mask(i) != 0.0) {
            cover += 1u;
        }
    }
#endif
#endif
    i.v137 = vec4<f32>(0.0);
#else
    // The opacity mask is a dither against the pixel position plus the temporal anti-aliasing
    // offset (View row 137): the game shows the average of eight frames' patterns. Here the
    // mask is evaluated at four offsets and written as the coverage of the four samples
    // (without multisampling the first one alone decides, as a clip would).
    var cover = 0u;
    for (var k = 0u; k < 4u; k++) {
        i.v137 = vec4<f32>(f32(k));
        if (kh_mask(i) != 0.0) {
            cover |= 1u << k;
        }
    }
#endif
    if (cover == 0u) {
        discard;
    }
    shaded.cover = cover;
    i.v137 = vec4<f32>(0.0);
#endif
#ifdef KH_BLOOD
    // (IMPL_ER60, a host's blood layer on the G-buffer values, before any light: the Elden Ring renderer defines
    // kh_blood in er_char.wgsl; no other host defines KH_BLOOD)
    let g = kh_blood(kh_material(i), i.twp + camera, i.tc0.xy);
#else
    let g = kh_material(i);
#endif
    var color: vec3<f32>;
    var alpha = 1.0;
    var lit = vec3<f32>(0.0);
#ifdef KH_TRANSLUCENT
    // Forward shaded by its own base pass: emissive + indirect, and its opacity.
    color = g.o0.xyz;
    alpha = saturate(g.o0.w * kh.misc.w);
#else
    let normal = normalize(g.o1.xyz * 2.0 - vec3<f32>(1.0));
    let to_camera = normalize(-i.twp);
    // ---- the lights: what the light shader adds to the scene colour and to SQEXLightMap
    if (kh.light_dir.w == 0.0) {
        lit = kh_light(g, normal, to_camera, normalize(key_dir), shadow, 0.08) * key_color;
    } else {
        // The rig's spot lights: stock UE 4.17 radial (inverse squared, windowed by the
        // attenuation radius) and cone attenuation; the BRDF is the same light shader.
        let world = i.twp + camera;
        for (var k = 0; k < 2; k++) {
            if (kh.spot_pos[k].w <= 0.0) {
                continue;
            }
            let to_light = kh.spot_pos[k].xyz - world;
            let d2 = dot(to_light, to_light);
            let l = to_light * inverseSqrt(d2);
            let r = kh.spot_dir[k].w;
            let window = saturate(1.0 - (d2 / (r * r)) * (d2 / (r * r)));
            let radial = window * window / (d2 + 1.0);
            let cone = saturate((dot(-l, kh.spot_dir[k].xyz) - kh.spot_cone[k].x) * kh.spot_cone[k].y);
            lit += kh_light(g, normal, to_camera, l, 1.0, kh.spot_cone[k].z) * (kh.spot_pos[k].w * radial * cone * cone) * kh.light_color.xyz;
        }
        // (game over) The rig's point lights: the engine's exponent falloff (a light without
        // bUseInverseSquaredFalloff: pow(1 - saturate(d^2 / r^2), exponent)); the same light shader.
        for (var k = 0; k < 4; k++) {
            let r = kh.point_pos[k].w;
            if (r <= 0.0) {
                continue;
            }
            let to_point = kh.point_pos[k].xyz - world;
            let d2 = dot(to_point, to_point);
            let l = to_point * inverseSqrt(max(d2, 1.0e-4));
            let radial = pow(saturate(1.0 - d2 / (r * r)), kh.point_col[k].w);
            lit += kh_light(g, normal, to_camera, l, 1.0, 0.08) * radial * kh.point_col[k].xyz;
        }
    }
#ifdef KH_HYBRID
    // (IMPL_ER78, a host's hybrid light: the Elden Ring renderer defines kh_hybrid_light in er_char.wgsl, which replaces the
    // rig's light above by the light the host's game gave this pixel, stepped into bands; the extra lights below still add.
    // No other host defines KH_HYBRID.)
    // (HY: + the world position, game space, for the host's own shadow terms)
    lit = kh_hybrid_light(g, normal, to_camera, normalize(key_dir), key_color, i.svpos, lit, i.twp + camera);
#endif
#ifdef KH_EXTRA_LIGHTS
    // (IMPL_ER44, a host's own extra point lights, IN ADDITION to the light above: the Elden Ring renderer's hero
    // light and effect lights; the host defines kh_extra_lights, which goes through the same light shader, so the
    // post-process material below composes them as the game composes its point lights.)
    lit += kh_extra_lights(g, normal, to_camera, i.twp + camera);
#endif
    if (kh.khs.x != 0.0) {
        // ---- the post-process material, on this pixel's own G-buffer values
        i.sa = vec4<f32>(g.o1.xyz, 0.0);
        i.sb = g.o2;
        i.sc = g.o3;
        i.sd = g.o4;
        i.sf = vec4<f32>(g.o6.xyz, kh.khs.y);
        i.sdepth = vec4<f32>(1.0 / max(i.svpos.w, 1.0));
        i.sao = vec4<f32>(kh.khs.z);
        i.slight = vec4<f32>(lit, 0.0);
        i.sbackup = vec4<f32>(g.o0.xyz + lit, 1.0);
        i.samb = vec4<f32>(0.0);
        i.sind = vec4<f32>(g.o0.xyz, 0.0);
        color = kh_khs(i);
    } else {
        color = g.o0.xyz + lit;
    }
    color += kh_reflection(g, normal, to_camera);
#endif
    color = max(color, vec3<f32>(0.0));
    // KH_CHAR_DEBUG: 1 base colour, 2 world normal, 3 (metallic, specular, roughness), 4 subsurface colour,
    // 5 emissive + indirect, 6 the key light alone, 7 opacity / the fork's two extra values.
    if (kh.misc.x != 0.0) {
        var dbg = g.o3.xyz;
        if (kh.misc.x == 2.0) { dbg = g.o1.xyz; }
        if (kh.misc.x == 3.0) { dbg = g.o2.xyz; }
        if (kh.misc.x == 4.0) { dbg = g.o4.xyz * g.o4.xyz; }
        if (kh.misc.x == 5.0) { dbg = g.o0.xyz; }
        if (kh.misc.x == 6.0) { dbg = lit; }
        if (kh.misc.x == 7.0) { dbg = vec3<f32>(g.o4.w, g.o6.y, g.o6.z); }
        shaded.color = vec4<f32>(max(dbg, vec3<f32>(0.0)), alpha);
        return shaded;
    }
    // The exposure in front of the camera's tone curve: 1 in the level (one exposure for him and
    // the level); the menu's capture, whose film settings are not reproduced, keeps a factor.
    color *= kh.look.x;
    if (kh.misc.z != 0.0) {
        // The earlier arrangement (KH_VFX_TONE=tony): the camera's tone mapper is not the
        // filmic one, so the curve is applied here and expanded again for the camera.
        let d = min(kh_aces(color), vec3<f32>(0.985));
        color = d / (vec3<f32>(1.0) - d);
    }
    shaded.color = vec4<f32>(color, alpha);
    return shaded;
}

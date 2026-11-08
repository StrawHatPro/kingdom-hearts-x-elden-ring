// (MIMIC SORA, THE USER 2026-11-02: "Make sure mimic Sora uses the shaders that mimic should use from Elden Ring.") THE MIMIC
// TEAR'S LOOK on a KH character, the GPU side of src/mimic.rs (the numbers, their source in the regulation and the reading are
// documented there; a unit test checks that this file carries the same rows). NOT YET IN THE PIPELINE: kh-shaderc does not
// build it and gfx.rs draws no mimic (kh-partyhost src/mimic.rs, step 4 of what is missing). A naga test parses and validates it.
//
// Two passes for a mimic's draws:
// 1. mm_gb_ps, in place of the hybrid's G-buffer writer (`gb_main`): his surface with WetAspectParam 18 (the liquid silver)
//    in the game's G-buffer packing; the game's own light passes then shade it (its reflections and specular on metal);
// 2. mm_tint_ps, after the game's composite, blended ONE + ONE: PhantomParam 200 (every spirit's sheen), its tint into the
//    scene colour and glowScale of it into the glow source. His KH colour pass is not drawn over a mimic.

// ---- WetAspectParam 18 (regulation 2.7.2.0): base colour (200, 200, 200) at 0.8, metallic 255 at 1.0, shininess 255 at 0.85
const MM_BASE_SRGB: f32 = 200.0 / 255.0;
const MM_BASE_RATE: f32 = 0.8;
const MM_METAL: f32 = 255.0 / 255.0;
const MM_METAL_RATE: f32 = 1.0;
const MM_SHINE: f32 = 255.0 / 255.0;
const MM_SHINE_RATE: f32 = 0.85;

// ---- PhantomParam 200: edge (250, 250, 255) x 0.1, front (35, 35, 45) x 0.25, diffuse x 0.9, specular x 1, edgePower 0.5,
// glowScale 0.3, added (isEdgeSubtract / isFrontSubtract 0), alpha 1
const MM_EDGE_SRGB: vec3<f32> = vec3<f32>(250.0 / 255.0, 250.0 / 255.0, 255.0 / 255.0);
const MM_EDGE_A: f32 = 0.1;
const MM_FRONT_SRGB: vec3<f32> = vec3<f32>(35.0 / 255.0, 35.0 / 255.0, 45.0 / 255.0);
const MM_FRONT_A: f32 = 0.25;
const MM_DIFF_MUL: f32 = 0.9;
const MM_SPEC_MUL: f32 = 1.0;
const MM_EDGE_POWER: f32 = 0.5;
const MM_GLOW_SCALE: f32 = 0.3;

fn mm_linear1(c: f32) -> f32 {
    if (c <= 0.04045) {
        return c / 12.92;
    }
    return pow((c + 0.055) / 1.055, 2.4);
}

fn mm_linear(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(mm_linear1(c.x), mm_linear1(c.y), mm_linear1(c.z));
}

// The surface with the wet aspect: (albedo linear, metalness, gloss) each `rate` of the way to the row's.
struct MmSurface {
    albedo: vec3<f32>,
    metalness: f32,
    gloss: f32,
}

fn mm_surface(kh_albedo: vec3<f32>, gloss: f32) -> MmSurface {
    var s: MmSurface;
    s.albedo = mix(kh_albedo, vec3<f32>(mm_linear1(MM_BASE_SRGB)), MM_BASE_RATE);
    s.metalness = mix(0.0, MM_METAL, MM_METAL_RATE);
    s.gloss = mix(gloss, MM_SHINE, MM_SHINE_RATE);
    return s;
}

// The silhouette weight (1 - N.V) ^ edgePower and the row's tint.
fn mm_edge(n_dot_v: f32) -> f32 {
    return pow(1.0 - clamp(n_dot_v, 0.0, 1.0), MM_EDGE_POWER);
}

fn mm_tint(n_dot_v: f32) -> vec3<f32> {
    let f = mm_edge(n_dot_v);
    return mm_linear(MM_EDGE_SRGB) * (MM_EDGE_A * f) + mm_linear(MM_FRONT_SRGB) * (MM_FRONT_A * (1.0 - f));
}

// The lit colour with the row (for a host that shades the mimic itself instead of the game's light passes).
fn mm_shade(diffuse: vec3<f32>, specular: vec3<f32>, n_dot_v: f32) -> vec3<f32> {
    return max(diffuse * MM_DIFF_MUL + specular * MM_SPEC_MUL + mm_tint(n_dot_v), vec3<f32>(0.0));
}

// ---- the passes' interfaces (the real ones take their inputs from the skinned vertex shader and the KH material, as gb_main)
struct MmIn {
    @builtin(position) position: vec4<f32>,
    // the world normal (host axes) and the KH material's base colour (linear)
    @location(0) normal: vec3<f32>,
    @location(1) albedo: vec3<f32>,
    // the direction from the pixel to the camera (host axes)
    @location(2) to_eye: vec3<f32>,
}

struct MmGbuf {
    @location(0) normal: vec4<f32>,
    @location(1) params: vec4<f32>,
    @location(2) albedo: vec4<f32>,
}

// The hybrid's G-buffer gloss (r.hybrid_gloss), the light set (G-buffer 2 .a): the defaults of hybrid.rs.
const MM_HYBRID_GLOSS: f32 = 0.3;
const MM_LIGHTSET: f32 = 0.0;

@fragment
fn mm_gb_ps(in: MmIn) -> MmGbuf {
    let n = normalize(in.normal);
    let s = mm_surface(in.albedo, MM_HYBRID_GLOSS);
    var o: MmGbuf;
    o.normal = vec4<f32>(n * 0.5 + vec3<f32>(0.5), 0.0);
    o.params = vec4<f32>(s.metalness, s.gloss, 0.0, 1.0);
    o.albedo = vec4<f32>(s.albedo, MM_LIGHTSET / 255.0);
    return o;
}

struct MmTint {
    // the scene colour (blend ONE + ONE) and the glow source (blend ONE + ONE)
    @location(0) color: vec4<f32>,
    @location(1) glow: vec4<f32>,
}

@fragment
fn mm_tint_ps(in: MmIn) -> MmTint {
    let n_dot_v = dot(normalize(in.normal), normalize(in.to_eye));
    let t = mm_tint(n_dot_v);
    var o: MmTint;
    o.color = vec4<f32>(t, 0.0);
    o.glow = vec4<f32>(max(t, vec3<f32>(0.0)) * MM_GLOW_SCALE, 0.0);
    return o;
}

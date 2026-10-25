// (ZDRAW, kh-er-render src/zdraw.rs; built by kh-shaderc `--only zdraw` into materials/er/zdraw.vs.dxil + zdraw.ps.dxil)
// ZETTAFLARE'S TARGET FROM ITS OWN MODEL INTO THE FINALE'S MASK: its triangles (game centimetres, the channel in w: 1 the
// silhouette and its pieces, 2 its light: cracks, embers) projected by the frame's clip (b0, columns) and written with max
// blending into the mask finale.rs colours over the picture (er_char.wgsl fz_look: the test level's overlay).
// (the soups are drawn with a depth of their own in the small mask: the body and its pieces first, then its light tested
// against them, so a crack or an ember shows only on the surface seen, never on a layer under another (an Elden Ring model is
// armour over cloth over skin). bias.x: the light's nudge toward the camera, as a factor of its clip z (its sign: the
// projection's depth direction).)
struct ZdBlock {
    clip: mat4x4<f32>,
    bias: vec4<f32>,
}
@group(0) @binding(0) var<uniform> zd: ZdBlock;

struct ZdOut {
    @builtin(position) position: vec4<f32>,
    @location(0) channel: f32,
}

@vertex
fn zdraw_vs(@location(0) p: vec4<f32>) -> ZdOut {
    var o: ZdOut;
    o.position = zd.clip * vec4<f32>(p.xyz, 1.0);
    if (p.w > 1.5) {
        o.position.z = o.position.z * (1.0 + zd.bias.x);
    }
    o.channel = p.w;
    return o;
}

@fragment
fn zdraw_ps(i: ZdOut) -> @location(0) vec4<f32> {
    if (i.channel > 1.5) {
        return vec4<f32>(0.0, 1.0, 0.0, 1.0);
    }
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}

// ---- THE TEST LEVEL'S MASK SIZE (zdraw_up.vs/ps): the soups are drawn into a mask 960 pixels wide (kh-app zetta_crumble.rs
// MASK_W x MASK_H: the target's aspect), which is then laid over the finale's full-size mask with the test level's linear
// filtering (its overlay samples its 960-wide mask with a linear sampler): the same line widths of the cracks, the same soft
// edges, whatever the screen's size. zu.size: the target's width, height; the small mask's width, height.
struct ZdUp {
    size: vec4<f32>,
}
@group(0) @binding(3) var<uniform> zu: ZdUp;
@group(1) @binding(0) var zu_small: texture_2d<f32>;

@vertex
fn zdraw_up_vs(@builtin(vertex_index) k: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((k << 1u) & 2u), f32(k & 2u));
    return vec4<f32>(uv * 2.0 - vec2<f32>(1.0), 0.0, 1.0);
}

fn zu_at(t: vec2<i32>) -> vec4<f32> {
    let dims = vec2<i32>(zu.size.zw);
    return textureLoad(zu_small, clamp(t, vec2<i32>(0), dims - vec2<i32>(1)), 0);
}

@fragment
fn zdraw_up_ps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    // the small mask's texel centres under this pixel's centre (bilinear, as a linear sampler at the test level's size)
    let s = position.xy * zu.size.zw / zu.size.xy - vec2<f32>(0.5);
    let i = vec2<i32>(floor(s));
    let f = s - floor(s);
    let a = mix(zu_at(i), zu_at(i + vec2<i32>(1, 0)), f.x);
    let b = mix(zu_at(i + vec2<i32>(0, 1)), zu_at(i + vec2<i32>(1, 1)), f.x);
    let m = mix(a, b, f.y);
    if (m.r + m.g <= 0.0) {
        discard;
    }
    return vec4<f32>(m.r, m.g, 0.0, 1.0);
}

// ---- THE BODY IN THE LIGHT (zdraw_imm.vs/ps): the test level's look of a body Zettaflare holds (kh-app zetta_target.rs: its
// own colours lerped to a light grey, a little glow, by ScriptFinale::immerse): its own model drawn over the game's draw of it,
// a light grey lit by a soft key, faded in by `immerse`; behind whatever the scene depth holds nearer than it (Sora, the
// party, a wall). Its colour is display-linear, turned into the target's units as finale.rs's comp does (the ACES fit's
// inverse x the light's scale).
struct ZdImm {
    clip: mat4x4<f32>,
    // xyz the camera (game cm), w the depth's a; xyz forward, w the depth's b (view metres = 1 / (device * a + b))
    camera: vec4<f32>,
    forward: vec4<f32>,
    // width, height, 1 / width, 1 / height of the target
    size: vec4<f32>,
    // x its alpha (immerse), y the light's scale to the target, z the depth's slack (cm), w 1 = the depth test on
    look: vec4<f32>,
    // the grey (display-linear); w its glow
    colour: vec4<f32>,
    // the key's direction (game axes); w how much it shades (0: flat)
    light: vec4<f32>,
}
@group(0) @binding(2) var<uniform> zi: ZdImm;
@group(1) @binding(0) var zi_depth: texture_2d<f32>;

struct ZiOut {
    @builtin(position) position: vec4<f32>,
    @location(0) place: vec3<f32>,
}

@vertex
fn zdraw_imm_vs(@location(0) p: vec4<f32>) -> ZiOut {
    var o: ZiOut;
    o.position = zi.clip * vec4<f32>(p.xyz, 1.0);
    o.place = p.xyz;
    return o;
}

fn zi_inv_aces(y: vec3<f32>) -> vec3<f32> {
    let c = clamp(y, vec3<f32>(0.0), vec3<f32>(0.98));
    let a = 2.43 * c - vec3<f32>(2.51);
    let b = 0.59 * c - vec3<f32>(0.03);
    let k = 0.14 * c;
    return (-b - sqrt(max(b * b - 4.0 * a * k, vec3<f32>(0.0)))) / (2.0 * a);
}

@fragment
fn zdraw_imm_ps(i: ZiOut) -> @location(0) vec4<f32> {
    if (zi.look.w > 0.5) {
        let dims = vec2<f32>(textureDimensions(zi_depth));
        let t = clamp(vec2<i32>(i.position.xy * dims * zi.size.zw), vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1));
        let ds = textureLoad(zi_depth, t, 0).x;
        let scene_m = 1.0 / max(ds * zi.camera.w + zi.forward.w, 1.0e-6);
        let mine_m = dot(i.place - zi.camera.xyz, zi.forward.xyz) / 100.0;
        if (scene_m < mine_m - zi.look.z / 100.0) {
            discard;
        }
    }
    let n = normalize(cross(dpdx(i.place), dpdy(i.place)));
    // (the test level's body in the light reads as one flat light grey: a trace of shading only, light.w)
    let lit = 1.0 - zi.light.w + zi.light.w * abs(dot(n, normalize(zi.light.xyz)));
    let display = zi.colour.rgb * lit + vec3<f32>(zi.colour.w);
    return vec4<f32>(zi_inv_aces(display) * zi.look.y, clamp(zi.look.x, 0.0, 1.0));
}

// ---- (the offline harness, kh-er-render examples/zdraw_harness.rs) THE "GAME'S" OWN DRAW OF THE BODY: its triangles lit
// flat (a normal from the screen derivatives of its place), as a stand-in for Elden Ring drawing its enemy. zd.clip as above;
// zd.light: xyz the light's direction (game axes), w the albedo's scale.
struct ZdBody {
    clip: mat4x4<f32>,
    light: vec4<f32>,
    albedo: vec4<f32>,
}
@group(0) @binding(1) var<uniform> zb: ZdBody;

struct ZbOut {
    @builtin(position) position: vec4<f32>,
    @location(0) place: vec3<f32>,
}

@vertex
fn zdraw_body_vs(@location(0) p: vec4<f32>) -> ZbOut {
    var o: ZbOut;
    o.position = zb.clip * vec4<f32>(p.xyz, 1.0);
    o.place = p.xyz;
    return o;
}

@fragment
fn zdraw_body_ps(i: ZbOut) -> @location(0) vec4<f32> {
    let n = normalize(cross(dpdx(i.place), dpdy(i.place)));
    let l = normalize(zb.light.xyz);
    let lit = 0.28 + 0.72 * abs(dot(n, l));
    return vec4<f32>(zb.albedo.rgb * lit * zb.light.w, 1.0);
}

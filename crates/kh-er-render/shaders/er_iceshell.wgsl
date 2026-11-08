// (FREEZE-FX, kh-er-render src/iceshell.rs; built by kh-shaderc `--only iceshell` into materials/er/iceshell.vs.dxil +
// iceshell.ps.dxil) THE ICE SHELL OF A FROZEN ELDEN RING BODY: its own posed model (the triangles zdraw skins from its FLVER
// and the game's bones, game centimetres) drawn over it with KH3's ice response, as KH3's character materials draw "Ice" on
// its own characters (the compiled base pass of DisneyOrg_Basemat / pc_skinbasemat, read in outfits/kh1 materials.wgsl):
// - the ice tile of KH3's LibraryTiling16a atlas (tile 12, 2: its mask's alpha and its normal's xy, the real texels, handed in
//   `ice_tile`), laid over the body in three planes (an Elden Ring mesh has no KH UVs);
// - KH3's ice colour: mask x (0.192609, 0.346788, 0.395) + (0.192791, 0.310386, 0.425);
// - KH3's ice share A = f^3 + (1 - f^3) x facing, f = 1 - N.V with the ice normal (the rim fully iced);
// - the amount `ice` (KH3's "Ice" drive: 0 -> 1 over 0.3 s at the freeze, back over 0.1 s at the break).
// The game's depth is tested by the pipeline (reversed z); the shell is lifted toward the eye so it never fights the body's
// own draw. Colour as ZDRAW's body in the light (zdraw_imm): a display colour taken back through the fitted ACES curve, times
// the target's scale.
struct IsBlock {
    clip: mat4x4<f32>,
    // xyz the eye (game cm), w the ice amount 0..1
    camera: vec4<f32>,
    // xyz toward the light, w the light wrap
    light: vec4<f32>,
    // x tile texels per side, y cm per tile, z facing share, w emissive share
    tile: vec4<f32>,
    // xyz KH3's ice colour slope, w the target's scale
    slope: vec4<f32>,
    // xyz KH3's ice colour base, w the lift toward the eye (cm)
    base: vec4<f32>,
}
@group(0) @binding(0) var<uniform> is: IsBlock;
// (normal x, normal y, mask, 0) per texel of the ice tile, rows of `tile.x`
@group(1) @binding(0) var<storage, read> ice_tile: array<vec4<f32>>;

struct IsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) place: vec3<f32>,
}

@vertex
fn iceshell_vs(@location(0) p: vec4<f32>) -> IsOut {
    var o: IsOut;
    let toward = normalize(is.camera.xyz - p.xyz);
    let lifted = p.xyz + toward * is.base.w;
    o.position = is.clip * vec4<f32>(lifted, 1.0);
    o.place = p.xyz;
    return o;
}

fn is_texel(x: i32, y: i32) -> vec4<f32> {
    let n = i32(is.tile.x);
    let xx = ((x % n) + n) % n;
    let yy = ((y % n) + n) % n;
    return ice_tile[u32(yy * n + xx)];
}

// The ice tile at `uv` (tiles), bilinear, wrapped.
fn is_sample(uv: vec2<f32>) -> vec4<f32> {
    let n = is.tile.x;
    let t = uv * n - vec2<f32>(0.5);
    let f = fract(t);
    let i = vec2<i32>(floor(t));
    let a = mix(is_texel(i.x, i.y), is_texel(i.x + 1, i.y), f.x);
    let b = mix(is_texel(i.x, i.y + 1), is_texel(i.x + 1, i.y + 1), f.x);
    return mix(a, b, f.y);
}

fn is_inv_aces(y: vec3<f32>) -> vec3<f32> {
    let c = clamp(y, vec3<f32>(0.0), vec3<f32>(0.98));
    let a = 2.43 * c - vec3<f32>(2.51);
    let b = 0.59 * c - vec3<f32>(0.03);
    let k = 0.14 * c;
    return (-b - sqrt(max(b * b - 4.0 * a * k, vec3<f32>(0.0)))) / (2.0 * a);
}

@fragment
fn iceshell_ps(i: IsOut) -> @location(0) vec4<f32> {
    let v = normalize(is.camera.xyz - i.place);
    var n = normalize(cross(dpdx(i.place), dpdy(i.place)));
    if (dot(n, v) < 0.0) {
        n = -n;
    }
    // the tile in three planes, blended by the facing
    let w0 = pow(abs(n), vec3<f32>(4.0));
    let w = w0 / max(w0.x + w0.y + w0.z, 1e-4);
    let s = 1.0 / max(is.tile.y, 1.0);
    let tx = is_sample(i.place.yz * s);
    let ty = is_sample(i.place.xz * s);
    let tz = is_sample(i.place.xy * s);
    let t = tx * w.x + ty * w.y + tz * w.z;
    // the ice normal: the tile's xy (KH3's 2x - 1) bent over the surface
    let bend = t.xy * 2.0 - vec2<f32>(1.0);
    let side = normalize(cross(n, select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.z) > 0.9)));
    let up = cross(side, n);
    let ni = normalize(n + (side * bend.x + up * bend.y) * 0.5);
    // KH3's ice share and colour
    let f = 1.0 - max(dot(ni, v), 0.0);
    let f3 = f * f * f;
    let share = f3 + (1.0 - f3) * is.tile.z;
    let ice = is.slope.xyz * t.z + is.base.xyz;
    let lit = (1.0 - is.light.w) + is.light.w * abs(dot(ni, normalize(is.light.xyz)));
    let display = ice * lit + ice * share * is.tile.w;
    let alpha = clamp(is.camera.w * share, 0.0, 1.0);
    return vec4<f32>(is_inv_aces(display) * is.slope.w, alpha);
}

// (PARTY-NATIVE) THE GAME'S FOG FOR THE KH TRANSLUCENT EFFECTS: the copy of what its fog composite reads (kh-er-render vfx.rs
// `fog_copy`; build/sora_tmp/weapons/PARTY-NATIVE/ffx/FFX_FINDINGS.md section 5).
//
// GXVolumetricFog_SceneComposite (gxrenderershader, 2.7.2.0; disassembly er78/dis/rend/GXVolumetricFog_SceneComposite.ppo.txt)
// reads CB_SHADER_COMMON_PARAM at b0 (the GX root signature's pixel-stage table 28) and g_InScatterTex, a 3D float4 volume
// (rgb in-scatter, a transmittance), at t0 (table 30). These two passes are drawn with OUR root signature of the same shape
// at those registers (vfx.rs `fog_root`: table b0..b3, table t0..t3, root constants b5), the game's views heap bound and the
// game's own table handles of this frame's composite draw (ffx.rs `fog_bind`):
//   fs_rows: the buffer's 76 rows, as raw words, into texels 0..75 of a 77 x 1 RGBA32_UINT target of ours; texel 76 = the
//            volume's size (textureDimensions);
//   fs_vol:  one slice (root constant z) of the volume, texel for texel, into a slice of our own 3D RGBA16F texture.
// The effects' shaders (er_vfx.wgsl KH_FXFOG) then apply the composite's function to each effect pixel at its own depth.

struct GameCb {
    r: array<vec4<u32>, 76>,
}

struct Slice {
    z: vec4<u32>,
}

@group(3) @binding(0) var<uniform> game: GameCb;
@group(1) @binding(0) var inscatter: texture_3d<f32>;
@group(3) @binding(5) var<uniform> slice: Slice;

@vertex
fn vs_fog(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
}

@fragment
fn fs_rows(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let row = u32(p.x);
    if (row < 76u) {
        return game.r[row];
    }
    let d = textureDimensions(inscatter);
    return vec4<u32>(d.x, d.y, d.z, 1u);
}

@fragment
fn fs_vol(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(inscatter, vec3<i32>(i32(p.x), i32(p.y), i32(slice.z.x)), 0);
}

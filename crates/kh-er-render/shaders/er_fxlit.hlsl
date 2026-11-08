// (PARTY-NATIVE-LIGHT) ELDEN RING'S OWN LIGHT FOR THE KH LIT TRANSLUCENT EFFECTS: the copy of what its lit particles read
// (kh-er-render src/vfxlight.rs `LitKit`; build/sora_tmp/weapons/PARTY-NATIVE/LIGHT_STATE.md). Written in HLSL (dxc, no naga):
// the game's light buffers are STRUCTURED buffers, which naga cannot declare.
//
// These passes are drawn with OUR root signature shaped like the game's GX one at the pixel stage (vfxlight.rs `LitKit::new`:
// table b0..b3, table b4..b13, table t4..t19, table t20..t65, root constants b14), the game's views heap bound and the game's
// own table handles of this frame's draws (ffx.rs `lit_binds`):
//   ps_scene  : the deferred tile lighting's cbSceneParam (b8, 126 rows) and cbLightAccParam (b0, 24 rows) as raw words, and the
//               element counts of its light buffers t59..t62, into texels 0..150 of our rows target (RGBA32_UINT);
//   ps_pass   : the particle pass's gcbSceneParam (b10, 58 rows) and the sizes of its irradiance volumes t8..t15, texels 160..225;
//   ps_region : a region's CB_ENVMAP_REGION (b3, 36 rows) and the sizes of its volume t27..t30, texels 230 + 42 r ..;
//   ps_dspass : (round 3) a tessellated lit particle's DSCommonBuffer (b0 of its domain stage, 37 rows) remapped into the pass's
//               gcbSceneParam rows (A 29..31 <- 19..21, B 33..35 <- 23..25, 37 <- 27, 38 <- 28, 39 / 40 <- 29 / 30, 41 / 42 <- 31 / 32,
//               52..54 <- 8..10), and the sizes of its volumes t8..t15 (the same registers);
//   ps_vol    : one slice (k.x) of volume k.y (k.z 0: the pass's t8 + k.y, 1: the region's t27 + (k.y & 3), (round 3) 2: a two-volume
//               region's second volume t31 + (k.y & 3)), texel for texel;
//   ps_lists  : the four light buffers, flattened into one RGBA32_UINT target LISTS_W wide (the layout: vfxlight.rs
//               `lists_layout`, er_vfx.wgsl `kh_lit_layout`; the same clamps).
// The numbers here and in vfxlight.rs / er_vfx.wgsl must agree (vfxlight.rs tests read this file).

#define ROW_SCENE 0
#define ROW_LACC 126
#define ROW_COUNTS 150
#define ROW_PASS 160
#define ROW_PASS_DIMS 218
#define ROW_REGION 230
#define REGION_STRIDE 42
#define LISTS_W 1024
#define MAX_INDICES 4194304
#define MAX_LOOKUP 262144
#define MAX_POINTS 8192
#define MAX_SPOTS 8192

cbuffer RootK : register(b14) { uint4 k; };
// (b0: cbLightAccParam of the tile lighting (24 rows read) or, round 3, a tessellated particle's DSCommonBuffer (37 rows))
cbuffer B0 : register(b0) { uint4 b0rows[37]; };
cbuffer EnvRegion : register(b3) { uint4 region[36]; };
cbuffer Scene : register(b8) { uint4 scene[126]; };
cbuffer GsScene : register(b10) { uint4 gscene[58]; };

Texture3D<float4> pass_iv0 : register(t8);
Texture3D<float4> pass_iv1 : register(t9);
Texture3D<float4> pass_iv2 : register(t10);
Texture3D<float4> pass_iv3 : register(t11);
Texture3D<float4> pass_iv4 : register(t12);
Texture3D<float4> pass_iv5 : register(t13);
Texture3D<float4> pass_iv6 : register(t14);
Texture3D<float4> pass_iv7 : register(t15);
Texture3D<float4> region_iv0 : register(t27);
Texture3D<float4> region_iv1 : register(t28);
Texture3D<float4> region_iv2 : register(t29);
Texture3D<float4> region_iv3 : register(t30);
Texture3D<float4> region2_iv0 : register(t31);
Texture3D<float4> region2_iv1 : register(t32);
Texture3D<float4> region2_iv2 : register(t33);
Texture3D<float4> region2_iv3 : register(t34);

struct PointLight { float4 r[13]; };
struct SpotLight { float4 r[12]; };
StructuredBuffer<uint> g_LightIndices : register(t59);
StructuredBuffer<uint3> g_LightLookupBuffer : register(t60);
StructuredBuffer<PointLight> g_LightBuffer : register(t61);
StructuredBuffer<SpotLight> g_SpotLightBuffer : register(t62);

float4 vs_lit(uint i : SV_VertexID) : SV_Position {
    float2 uv = float2((i << 1) & 2, i & 2);
    return float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
}

// the element counts of the light buffers, clamped (vfxlight.rs clamp_counts)
uint4 light_counts() {
    uint ni, nl, np, ns, stride;
    g_LightIndices.GetDimensions(ni, stride);
    g_LightLookupBuffer.GetDimensions(nl, stride);
    g_LightBuffer.GetDimensions(np, stride);
    g_SpotLightBuffer.GetDimensions(ns, stride);
    return uint4(min(ni, MAX_INDICES), min(nl, MAX_LOOKUP), min(np, MAX_POINTS), min(ns, MAX_SPOTS));
}

uint4 ps_scene(float4 p : SV_Position) : SV_Target0 {
    uint t = (uint)p.x;
    if (t < ROW_LACC) {
        return scene[t - ROW_SCENE];
    }
    if (t < ROW_COUNTS) {
        return b0rows[t - ROW_LACC];
    }
    if (t == ROW_COUNTS) {
        return light_counts();
    }
    return uint4(0, 0, 0, 0);
}

uint4 dims3(Texture3D<float4> v) {
    uint w, h, d;
    v.GetDimensions(w, h, d);
    return uint4(w, h, d, 1);
}

uint4 pass_dims(uint i) {
    uint4 o = uint4(0, 0, 0, 0);
    switch (i) {
        case 0: o = dims3(pass_iv0); break;
        case 1: o = dims3(pass_iv1); break;
        case 2: o = dims3(pass_iv2); break;
        case 3: o = dims3(pass_iv3); break;
        case 4: o = dims3(pass_iv4); break;
        case 5: o = dims3(pass_iv5); break;
        case 6: o = dims3(pass_iv6); break;
        default: o = dims3(pass_iv7); break;
    }
    return o;
}

uint4 region_dims(uint i) {
    uint4 o = uint4(0, 0, 0, 0);
    switch (i) {
        case 0: o = dims3(region_iv0); break;
        case 1: o = dims3(region_iv1); break;
        case 2: o = dims3(region_iv2); break;
        default: o = dims3(region_iv3); break;
    }
    return o;
}

uint4 ps_pass(float4 p : SV_Position) : SV_Target0 {
    uint t = (uint)p.x;
    if (t >= ROW_PASS && t < ROW_PASS_DIMS) {
        return gscene[t - ROW_PASS];
    }
    if (t >= ROW_PASS_DIMS && t < ROW_PASS_DIMS + 8) {
        return pass_dims(t - ROW_PASS_DIMS);
    }
    return uint4(0, 0, 0, 0);
}

uint4 region2_dims(uint i) {
    uint4 o = uint4(0, 0, 0, 0);
    switch (i) {
        case 0: o = dims3(region2_iv0); break;
        case 1: o = dims3(region2_iv1); break;
        case 2: o = dims3(region2_iv2); break;
        default: o = dims3(region2_iv3); break;
    }
    return o;
}

// k.w: the region's place (0 / 1); (round 3) k.z 1: a two-volume region (always place 0): its second volume's sizes go where a
// second region's would (texels ROW_REGION + REGION_STRIDE + 36 ..: the volume slots 4..7)
uint4 ps_region(float4 p : SV_Position) : SV_Target0 {
    uint t = (uint)p.x;
    uint base = ROW_REGION + REGION_STRIDE * k.w;
    if (t >= base && t < base + 36) {
        return region[t - base];
    }
    if (t >= base + 36 && t < base + 40) {
        return region_dims(t - base - 36);
    }
    uint second = ROW_REGION + REGION_STRIDE + 36;
    if (k.z == 1 && t >= second && t < second + 4) {
        return region2_dims(t - second);
    }
    return uint4(0, 0, 0, 0);
}

// (round 3) the gcbSceneParam row a DSCommonBuffer row stands for (DS = GXFfxtessellateBumpTexture_quad.dpo: g_ds_irradianceVolume*
// at offsets 304..527, g_ds_envMapColorConvert at 128), 0xffffffff = none
uint ds_row_of(uint g) {
    if (g >= 29 && g <= 31) return g - 10;
    if (g >= 33 && g <= 35) return g - 10;
    if (g >= 37 && g <= 42) return g - 10;
    if (g >= 52 && g <= 54) return g - 44;
    return 0xffffffff;
}

uint4 ps_dspass(float4 p : SV_Position) : SV_Target0 {
    uint t = (uint)p.x;
    if (t >= ROW_PASS && t < ROW_PASS_DIMS) {
        uint r = ds_row_of(t - ROW_PASS);
        return r == 0xffffffff ? uint4(0, 0, 0, 0) : b0rows[r];
    }
    if (t >= ROW_PASS_DIMS && t < ROW_PASS_DIMS + 8) {
        return pass_dims(t - ROW_PASS_DIMS);
    }
    return uint4(0, 0, 0, 0);
}

float4 ps_vol(float4 p : SV_Position) : SV_Target0 {
    int4 at = int4((int)p.x, (int)p.y, (int)k.x, 0);
    float4 o = float4(0.5, 0.5, 0.5, 0.5);
    if (k.z == 0) {
        switch (k.y) {
            case 0: o = pass_iv0.Load(at); break;
            case 1: o = pass_iv1.Load(at); break;
            case 2: o = pass_iv2.Load(at); break;
            case 3: o = pass_iv3.Load(at); break;
            case 4: o = pass_iv4.Load(at); break;
            case 5: o = pass_iv5.Load(at); break;
            case 6: o = pass_iv6.Load(at); break;
            default: o = pass_iv7.Load(at); break;
        }
    } else if (k.z == 2) {
        switch (k.y & 3) {
            case 0: o = region2_iv0.Load(at); break;
            case 1: o = region2_iv1.Load(at); break;
            case 2: o = region2_iv2.Load(at); break;
            default: o = region2_iv3.Load(at); break;
        }
    } else {
        switch (k.y & 3) {
            case 0: o = region_iv0.Load(at); break;
            case 1: o = region_iv1.Load(at); break;
            case 2: o = region_iv2.Load(at); break;
            default: o = region_iv3.Load(at); break;
        }
    }
    return o;
}

uint4 ps_lists(float4 p : SV_Position) : SV_Target0 {
    uint t = (uint)p.y * LISTS_W + (uint)p.x;
    uint4 c = light_counts();
    uint o1 = (c.x + 3) / 4;
    uint o2 = o1 + c.y;
    uint o3 = o2 + 13 * c.z;
    uint total = o3 + 12 * c.w;
    if (t < o1) {
        uint4 v = uint4(0, 0, 0, 0);
        uint i = t * 4;
        v.x = i < c.x ? g_LightIndices[i] : 0;
        v.y = i + 1 < c.x ? g_LightIndices[i + 1] : 0;
        v.z = i + 2 < c.x ? g_LightIndices[i + 2] : 0;
        v.w = i + 3 < c.x ? g_LightIndices[i + 3] : 0;
        return v;
    }
    if (t < o2) {
        return uint4(g_LightLookupBuffer[t - o1], 0);
    }
    if (t < o3) {
        uint e = t - o2;
        return asuint(g_LightBuffer[e / 13].r[e % 13]);
    }
    if (t < total) {
        uint e = t - o3;
        return asuint(g_SpotLightBuffer[e / 12].r[e % 12]);
    }
    return uint4(0, 0, 0, 0);
}

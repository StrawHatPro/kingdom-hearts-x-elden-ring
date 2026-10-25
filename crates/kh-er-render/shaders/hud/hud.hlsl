// The 2D overlay of kh-er-render/src/hud.rs: one layer picture of the HUD as a rectangle of the back buffer,
// texel for pixel (the pictures are rasterised at the size they have on screen: no filtering).
// Built by build.sh (dxc) into hud.vs.dxil / hud.ps.dxil, which hud.rs embeds.
cbuffer Quad : register(b0) {
    float4 rect;   // x0, y0, x1, y1 in pixels of the target
    float4 target; // width, height of the target; mode (0 = display-encoded output, 1 = linear output); linear scale
    float4 tint;   // x = opacity of the whole overlay (the host fades it while a full-screen menu of the game is up); y > 0.5 = a plain plate of the colour `fill` (no picture)
    float4 fill;   // (IMPL_ER15, the game-over plate) straight-alpha sRGB colour of a plate
};
Texture2D<float4> picture : register(t0);

float4 vs(uint vertex : SV_VertexID) : SV_Position {
    float2 corner = float2(vertex & 1, vertex >> 1);
    float2 px = lerp(rect.xy, rect.zw, corner);
    return float4(px.x / target.x * 2.0 - 1.0, 1.0 - px.y / target.y * 2.0, 0.0, 1.0);
}

float3 to_linear(float3 c) {
    return lerp(c / 12.92, pow(abs((c + 0.055) / 1.055), 2.4), step(0.04045, c));
}

float4 ps(float4 position : SV_Position) : SV_Target {
    // Straight alpha, display-encoded (sRGB) bytes, read through a UNORM view.
    float4 texel = fill;
    if (tint.y <= 0.5) {
        texel = picture.Load(int3(int2(position.xy) - int2(rect.xy), 0));
    }
    float3 colour = target.z > 0.5 ? to_linear(texel.rgb) * target.w : texel.rgb;
    // Premultiplied: the blend state is ONE, INV_SRC_ALPHA.
    float alpha = texel.a * saturate(tint.x);
    return float4(colour * alpha, alpha);
}

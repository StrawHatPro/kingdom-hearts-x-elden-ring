// UMapBuildDataRegistry: precomputed lighting per mesh, level light volumes, light build data.
// Docs: docs/formats/mapbuilddata.md.
const { T, register } = require('../package');

const guids = T.arr(T.guid, 16);
// FLightMap2D (type 2)
const lightMap2D = T.struct([
  ['lightGuids', guids],
  ['textures', T.fixed(T.obj, 2)], ['skyOcclusionTexture', T.obj], ['aoMaterialMaskTexture', T.obj],
  // ScaleVectors[i], AddVectors[i] for the four stored coefficients
  ['coefficients', T.fixed(T.struct([['scale', T.vec4], ['add', T.vec4]]), 4)],
  ['coordinateScale', T.vec2], ['coordinateBias', T.vec2],
]);
const lightMap = T.struct([['type', T.u32], ['lightMap2D', (o) => { if (o.type === 0) return null; if (o.type === 2) return lightMap2D; throw new Error('light map type ' + o.type); }]]);
const shadowMap2D = T.struct([
  ['lightGuids', guids], ['texture', T.obj], ['coordinateScale', T.vec2], ['coordinateBias', T.vec2],
  ['channelValid', T.fixed(T.b32, 4)], ['invUniformPenumbraSize', T.vec4],
]);
const shadowMap = T.struct([['type', T.u32], ['shadowMap2D', (o) => { if (o.type === 0) return null; if (o.type === 2) return shadowMap2D; throw new Error('shadow map type ' + o.type); }]]);
const meshBuildData = T.struct([
  ['lightMap', lightMap], ['shadowMap', shadowMap], ['irrelevantLights', guids],
  // FPerInstanceLightmapData: LightmapUVBias, ShadowmapUVBias (FVector2D each)
  ['perInstanceLightmapData', T.bulkArr(T.struct([['lightmapUVBias', T.vec2], ['shadowmapUVBias', T.vec2]]), 16)],
]);

// FPrecomputedLightVolumeData. FVolumeLightingSample is 132 bytes: FVector Position, float Radius,
// TSHVectorRGB<3> Lighting (27 floats: 9 R, 9 G, 9 B), FColor PackedSkyBentNormal,
// float DirectionalLightShadowing. Sample arrays are kept packed.
const lightVolumeBody = T.struct([
  ['bounds', T.box], ['sampleSpacing', T.f32], ['numSHSamples', T.i32],
  ['highQualitySamples', T.packed(132)], ['lowQualitySamples', T.packed(132)],
]);
const lightVolume = T.struct([['valid', T.b32], ['initialized', T.when((o) => o.valid, T.b32)], ['volume', T.when((o) => o.initialized === true, lightVolumeBody)]]);
// FLightComponentMapBuildData: ShadowMapChannel + FStaticShadowDepthMapData (DepthSamples are FFloat16, packed)
const lightBuildData = T.struct([['shadowMapChannel', T.i32], ['worldToLight', T.fixed(T.f32, 16)], ['shadowMapSizeX', T.i32], ['shadowMapSizeY', T.i32], ['depthSamples', T.packed(2)]]);

register('MapBuildDataRegistry', T.struct([
  ['strip', T.strip],
  ['meshBuildData', T.map(T.guid, meshBuildData, 16)],
  ['levelPrecomputedLightVolumeBuildData', T.map(T.guid, lightVolume, 20)],
  ['lightBuildData', T.map(T.guid, lightBuildData, 20)],
  // A fourth map that stock 4.17 does not have. From the bytes: an int32 N and a byte array of
  // 6 * (N^2 + (N/2)^2 + ... + 1) * 8 bytes, i.e. a full mip chain of six cube faces of 16-bit float
  // RGBA texels (cubemapSize, capturedData); the float and the second array are not identified.
  // (UE 4.19's FReflectionCaptureMapBuildData has this shape; that it is the same thing is not verified.)
  ['cubemapCaptures', T.map(T.guid, T.struct([['cubemapSize', T.i32], ['unk_f32', T.f32], ['capturedData', T.bytes], ['unk_bytes', T.bytes]]), 28)],
]), 'lighting');

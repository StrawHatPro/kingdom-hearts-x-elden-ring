// Smaller classes with native data: skeleton / rig, animation assets (skeleton guid), morph targets,
// particles, vector fields, fonts, sounds. Docs: docs/formats/misc_classes.md.
const { T, register, registerStruct } = require('../package');

// FReferenceSkeleton: bone info (FName, parent index), reference pose (FTransform), name -> index map
const referenceSkeleton = T.struct([
  ['refBoneInfo', T.arr(T.struct([['name', T.name], ['parentIndex', T.i32]]), 12)],
  ['refBonePose', T.arr(T.transform, 40)],
  ['nameToIndexMap', T.map(T.name, T.i32, 12)],
]);
register('Rig', T.struct([['sourceSkeleton', referenceSkeleton]]), 'skeleton');
register('Skeleton', T.struct([
  ['referenceSkeleton', referenceSkeleton],
  ['animRetargetSources', T.map(T.name, T.struct([['poseName', T.name], ['referencePose', T.arr(T.transform, 40)]]), 20)],
  ['guid', T.guid],
  // FSmartNameContainer: container name -> FSmartNameMapping
  ['smartNames', T.map(T.name, T.struct([
    ['guidMap', T.map(T.name, T.guid, 24)],
    ['curveMetaDataMap', T.map(T.name, T.struct([['material', T.b32], ['morphtarget', T.b32], ['linkedBones', T.arr(T.name, 8)]]), 20)],
  ]), 16)],
]), 'skeleton');

// UAnimationAsset::Serialize: SkeletonGuid
register(['BlendSpace', 'BlendSpace1D', 'AimOffsetBlendSpace', 'AimOffsetBlendSpace1D', 'AnimMontage', 'AnimComposite', 'PoseAsset'], T.struct([['skeletonGuid', T.guid]]), 'animation asset');

// UMorphTarget: FMorphTargetLODModel { Vertices, NumBaseMeshVerts }.
// FMorphTargetDelta is 28 bytes: FVector PositionDelta, FVector TangentZDelta, uint32 SourceIdx (kept packed).
register('MorphTarget', T.struct([
  ['strip', T.strip],
  ['morphLODModels', T.when((o) => !(o.strip.global & 2), T.arr(T.struct([['vertices', T.packed(28)], ['numBaseMeshVerts', T.i32]]), 8))],
]), 'morph target');

// UParticleModuleRequired: bCooked + BoundingGeometry (cutout vertices)
register('ParticleModuleRequired', T.struct([['cooked', T.b32], ['boundingGeometry', T.when((o) => o.cooked, T.arr(T.vec2, 8))]]), 'particles');

// UVectorFieldStatic: SourceData bulk (FFloat16Color per voxel, SizeX*SizeY*SizeZ*8 bytes)
register('VectorFieldStatic', T.struct([['sourceData', T.bulk]]), 'particles');

// UFont: CharRemap TMap<uint16, uint16>
register('Font', T.struct([['charRemap', T.map(T.u16, T.u16, 4)]]), 'font');
// UFontFace: bLoadInlineData (0 everywhere: the face is the .ufont file next to the package)
register('FontFace', T.struct([['loadInlineData', T.constant(T.u32, 0)]]), 'font');

// FFontCharacter (native struct, 21 bytes)
registerStruct('FontCharacter', T.struct([['startU', T.i32], ['startV', T.i32], ['uSize', T.i32], ['vSize', T.i32], ['textureIndex', T.u8], ['verticalOffset', T.i32]]));
// FFontData: bIsCooked, FontFaceAsset; without a face asset the file name, hinting and loading policy; SubFaceIndex
registerStruct('FontData', T.struct([
  ['cooked', T.constant(T.u32, 1)], ['fontFaceAsset', T.obj],
  ['fontFilename', T.when((o) => o.fontFaceAsset === 0, T.str)], ['hinting', T.when((o) => o.fontFaceAsset === 0, T.u8)], ['loadingPolicy', T.when((o) => o.fontFaceAsset === 0, T.u8)],
]));
// FClothLODData: tagged properties, then two array counts (TransitionUpSkinData and
// TransitionDownSkinData in the stock engine). Both are 0 in the two assets that exist, so the
// element layout is not established and a non-empty array is rejected.
const emptyArray = { r: (r) => { const n = r.i32(); if (n !== 0) throw new Error(`cloth transition array with ${n} elements (layout not established) at ${r.p - 4}`); return []; }, w: (w) => w.i32(0) };
registerStruct('ClothLODData', T.struct([['props', T.props], ['transitionUpSkinData', emptyArray], ['transitionDownSkinData', emptyArray]]));
// FSmartName as cooked: the display name only
registerStruct('SmartName', T.struct([['displayName', T.name]]));
// UTextureRenderTarget2D: UTexture::Serialize strip flags only
register(['TextureRenderTarget2D', 'TextureRenderTargetCube'], T.struct([['strip', T.strip]]), 'texture');
// UHoudiniAsset (Houdini Engine plugin): version, the .hda file, flags, source file name
register('HoudiniAsset', T.struct([['houdiniAssetVersion', T.i32], ['assetBytes', T.foreign('Houdini digital asset (.hda, INDX container)', T.i32)], ['flagsPacked', T.u32], ['assetFileName', T.str]]), 'houdini');

// USoundWave: bCooked, FFormatContainer of compressed audio, CompressedDataGuid
register('SoundWave', T.struct([
  ['cooked', T.b32],
  ['compressedFormatData', T.formatContainer({ OGG: T.foreign('Ogg Vorbis audio') })],
  ['compressedDataGuid', T.guid],
]), 'sound');
register('SoundCue', T.struct([['strip', T.strip]]), 'sound');
register('SoundNodeWavePlayer', T.struct([['strip', T.strip], ['soundWave', T.obj]]), 'sound');

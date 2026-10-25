# Smaller classes with native data

Code: `tools/lib/pkg_classes/misc.js`, `tools/lib/pkg_classes/delegated.js`. Evidence: the
classes below in `audit/package_manifest.json`. All follow the common export framing.

| Class | Native data after the properties |
|---|---|
| Skeleton | FReferenceSkeleton; int32 count + { FName, FName PoseName, int32 count + FTransform } (AnimRetargetSources); guid; smart names: int32 count + { FName container, int32 count + { FName, guid }, int32 count + { FName curve, uint32 bMaterial, uint32 bMorphtarget, int32 count + FName linked bones } } |
| Rig | FReferenceSkeleton |
| BlendSpace, BlendSpace1D, AimOffsetBlendSpace, AimOffsetBlendSpace1D, AnimMontage | guid SkeletonGuid |
| MorphTarget | strip (2 × u8); int32 count + { int32 count + FMorphTargetDelta, int32 NumBaseMeshVerts } |
| ParticleModuleRequired | uint32 bCooked; int32 count + FVector2D (cutout geometry) |
| VectorFieldStatic | FByteBulkData (inline): 16-bit float RGBA per voxel |
| Font | int32 count + { uint16, uint16 } (CharRemap) |
| FontFace | uint32 0 (the face data is the `.ufont` file beside the package, a TrueType font) |
| SoundWave | uint32 bCooked; FFormatContainer (`OGG`: Ogg Vorbis stream, third-party); guid |
| SoundCue | strip (2 × u8) |
| SoundNodeWavePlayer | strip (2 × u8); package index SoundWave |
| TextureRenderTarget2D | strip (2 × u8) |
| HoudiniAsset | int32 version (2); int32 size + the `.hda` file (third-party, `INDX` container); uint32 flags; FString source file name |
| DataTable | int32 row count + { FName row name, tagged properties } |

FReferenceSkeleton: int32 count + { FName bone, int32 parent index }; int32 count +
FTransform (FQuat rotation, FVector translation, FVector scale: 40 bytes); int32 count +
{ FName, int32 } (name to index map).

FMorphTargetDelta is 28 bytes - FVector PositionDelta, FVector TangentZDelta, uint32
SourceIdx - and the delta arrays are kept packed in the JSON.

All 1,311 skeletons parse to their last byte. The smart-name containers are `AnimationCurves`
and `AnimationTrackCurves`; the guid map is non-empty in 130 containers and the curve metadata
map in 76, AnimRetargetSources in one skeleton (`audit/package_facts.json`).

`ClothingAsset` has no native data of its own, but its `LodData` property is an array of
`ClothLODData` structs, each a tagged property list followed by two int32 array counts (the
transition skinning arrays of the stock engine). Both counts are 0 in the two assets that
exist (`Maps/bt/bt_gimmick/g_bt_curtain/mdl/g_bt_curtain.uasset`), so the element layout is
not established; the decoder rejects a non-empty array instead of assuming one.

## Classes left to other converters

| Class | Converter |
|---|---|
| Texture2D, LightMapTexture2D, ShadowMapTexture2D, TextureLightProfile, TextureCube | `tools/lib/texture.js` (`texture.md`) |
| StaticMesh, SkeletalMesh | `tools/lib/mesh.js` (`staticmesh.md`, `skeletalmesh.md`) |
| SQEXSEADSound, SQEXSEADMusic, SQEXSEADSoundBank | `tools/lib/sead.js` (`sead_audio.md`) |
| AnimSequence | `tools/lib/anim.js` (`animsequence.md`, `audit/anim_manifest.json`) |

The package JSON records such an export as `{ delegated, size, sha256 }`.

The `Skeleton` layout above covers the whole export, including the part after the reference
skeleton (retarget sources, guid, smart names): e.g.
`Character/c_npc/c_bx200/mdl/c_bx200_0_Skeleton` ends with 40 bytes = int32 0 (no retarget
sources), 16-byte guid, int32 1 + name `AnimationCurves` + two empty maps, and
`Maps/bt/bt_gimmick/g_bt_FlagC/mdl/g_bt_FlagC_Skeleton` (the one skeleton with retarget sources)
also parses to its last byte.

## DestructibleMesh

Code: `tools/lib/pkg_classes/destructible.js`. Seven exports.

```
USkeletalMesh export data (properties, guid flag, render data)   read and re-written by tools/lib/mesh.js
uint32 name length + name bytes                                   (length 0 in the example below)
uint32 size + APEX destructible asset                             NvParameterized binary, magic 5A 5B 5C 5D
uint32 size + blob, repeated to the end of the export            in the example one blob, a collision cache starting with "DestructibleAsset.10"
```

The skeletal part has no length field. The decoder looks for the NvParameterized magic
preceded by a size, requires that the length-prefixed blobs from there run exactly to the end
of the export, and accepts the split only if `tools/lib/mesh.js` reads the part before it as a
SkeletalMesh to its last byte and writes the same bytes back. The JSON records that part as
`{ delegated, size, sha256 }` like other mesh exports. Example:
`Maps/he/he_gimmick/g_he_Ter_RollRockL/DM_he_Ter_RollRockL_wet_.uasset` - skeletal part
1,511,596 bytes, APEX asset 1,731,456 bytes, collision cache 161,914 bytes.

The two APEX blobs are **third-party data kept verbatim** (NVIDIA APEX: the NvParameterized
binary serialization of `DestructibleAssetParameters`, with class names such as
`RenderMeshAssetParameters` readable inside, and APEX's cached chunk collision, which embeds
PhysX `NXS` `CVXM` convex meshes). They are identified, delimited and counted under
third-party bytes; their internal object tables are not decoded.

# MapBuildDataRegistry (precomputed lighting)

Code: `tools/lib/pkg_classes/lighting.js`. Evidence: `MapBuildDataRegistry` in
`audit/package_manifest.json`.

```
strip                                   2 × u8 (FStripDataFlags)
MeshBuildData                           int32 count + { guid, FMeshMapBuildData }
LevelPrecomputedLightVolumeBuildData    int32 count + { guid, light volume }
LightBuildData                          int32 count + { guid, FLightComponentMapBuildData }
cubemapCaptures                         int32 count + { guid, capture }        (not in stock 4.17)
```

## FMeshMapBuildData

| Field | Type |
|---|---|
| LightMap | uint32 type: 0 none, 2 = FLightMap2D |
| ShadowMap | uint32 type: 0 none, 2 = FShadowMap2D |
| IrrelevantLights | int32 count + guid |
| PerInstanceLightmapData | bulk array (element size 16): FVector2D LightmapUVBias, FVector2D ShadowmapUVBias |

FLightMap2D: int32 count + guid (LightGuids); package index Textures[0], Textures[1],
SkyOcclusionTexture, AOMaterialMaskTexture; 4 × { FVector4 ScaleVector, FVector4 AddVector };
FVector2D CoordinateScale, CoordinateBias. (Example: the first entry of
`Maps/bx/umap/bx_02/bx_02_ENV_Night_BuiltData.uasset` - 543 light guids, texture export 1.)

FShadowMap2D: int32 count + guid; package index Texture; FVector2D CoordinateScale,
CoordinateBias; 4 × uint32 bChannelValid; FVector4 InvUniformPenumbraSize.

## Light volume (FPrecomputedLightVolumeData)

uint32 bValid; when set uint32 bVolumeInitialized; when set:

| Field | Type |
|---|---|
| Bounds | FBox (min, max, u8 valid) |
| SampleSpacing | f32 |
| NumSHSamples | int32 (9) |
| HighQualitySamples | int32 count + FVolumeLightingSample |
| LowQualitySamples | int32 count + FVolumeLightingSample |

FVolumeLightingSample is 132 bytes: FVector Position, f32 Radius, 27 × f32 lighting (third-order
spherical harmonics, 9 coefficients each for R, G, B), FColor PackedSkyBentNormal, f32
DirectionalLightShadowing. The sample arrays are kept packed in the JSON.

## FLightComponentMapBuildData

int32 ShadowMapChannel; FMatrix WorldToLight (16 × f32); int32 ShadowMapSizeX, ShadowMapSizeY;
int32 count + 16-bit float depth samples (packed).

## Cubemap captures (fourth map)

Stock UE 4.17 ends after LightBuildData. These files have a fourth map whose value is
int32, f32, int32 count + bytes, int32 count + bytes. In
`Maps/bx/umap/bx_02/bx_02_ENV_Night_BuiltData.uasset` it has 68 entries of 262,160 bytes each:
the int32 is 64 and the first array has 262,128 bytes = 6 faces × (64² + 32² + ... + 1) texels
× 8 bytes, a full mip chain of 16-bit float RGBA cube faces (texels end in `00 3C`, 1.0 as a
half float); the second array is empty. The JSON names what the bytes show - `cubemapSize`,
`capturedData` - and leaves the float and the second array as `unk_f32` and `unk_bytes`.
`FReflectionCaptureMapBuildData` of UE 4.19 has this shape (CubemapSize, AverageBrightness,
FullHDRCapturedData, EncodedHDRCapturedData); that this map is a backport of it was not
verified in the executable, so those names are not used.

# Materials: cooked inline shader maps

Code: `tools/lib/pkg_classes/material.js`. Evidence: `Material`,
`MaterialInstanceConstant`, `LandscapeMaterialInstanceConstant` in
`audit/package_manifest.json`; the global shader cache in `audit/loose_files.json`.

**Status: decoded and round-trip exact. The shader classes' parameter bindings are decoded with
layouts derived from the executable (`shader_parameters.md`); what remains uninterpreted are the
bindings of shader types the executable does not contain (listed there).**

## Material

After the common export framing:

```
int32 count of resources
per resource:  uint32 bCooked, uint32 bValid, FMaterialShaderMap when valid
```

## MaterialInstanceConstant, LandscapeMaterialInstanceConstant

Only instances with their own static permutation have native data (1,391 of 24,364 and 347 of
7,510 exports): `FStaticParameterSet`, then the same resource list as Material.

FStaticParameterSet: three arrays -
static switches { FName, uint32 Value, uint32 bOverride, guid },
component masks { FName, uint32 R, G, B, A, uint32 bOverride, guid },
terrain layer weights { FName, int32 WeightmapIndex, uint32 bOverride, guid }.

## FMaterialShaderMap

| Field | Type |
|---|---|
| ShaderMapId | see below |
| Platform | int32 |
| FriendlyName | FString |
| UniformExpressionSet | see below |
| compilation flags | 9 × uint32 bool |
| DebugDescription | FString ("Compiling <material>: Platform=PCD3D_SM5, Usage=...") |
| Shaders | int32 count + { FName shader type, shader record } |
| ShaderPipelines | int32 count + { FName pipeline type, int32 count + { FName type, shader record } } |
| MeshShaderMaps | int32 count + { FName vertex factory type, Shaders, ShaderPipelines } |

The nine booleans are kept as an unnamed list (`compilationOutputFlags`). Stock 4.17
`FMaterialCompilationOutput` has nine members in this place (bRequiresSceneColorCopy,
bNeedsSceneTextures, bUsesEyeAdaptation, bModifiesMeshPosition, bUsesWorldPositionOffset,
bNeedsGBuffer, bUsesGlobalDistanceField, bUsesPixelDepthOffset, bUsesSceneDepthLookup); the
executable has no names, and matching each position to a member was not verified, so the JSON
does not claim it. The same goes for the fifth uniform-expression array (`unk_expressions_4`),
which is empty in every shader map and so offers nothing to identify it by.

FMaterialShaderMapId: uint32 Usage, guid BaseMaterialId, int32 QualityLevel, int32
FeatureLevel, FStaticParameterSet, int32 count + guid (ReferencedFunctions), int32 count +
guid (ReferencedParameterCollections), three dependency arrays of { FName type, 20-byte source
hash } (shader types, shader pipeline types, vertex factory types), 20-byte
TextureReferencesHash, 20-byte BasePropertyOverridesHash.

FUniformExpressionSet: expression arrays in this order - vector, scalar, 2D texture, cube
texture, a fifth array stock 4.17 does not have (`unk_expressions_4`, empty wherever seen),
then int32 count + guid (ParameterCollections), then per-frame scalar, per-frame vector,
per-frame previous scalar, per-frame previous vector. An expression is an FName type followed
by that type's fields:

| Type (`FMaterialUniformExpression...`) | Fields |
|---|---|
| Constant | FLinearColor Value, u8 ValueType |
| VectorParameter | FName, FLinearColor default |
| ScalarParameter | FName, f32 default |
| Texture, FlipBookTextureParameter | int32 TextureIndex, int32 SamplerSource |
| TextureParameter | FName, int32 TextureIndex, int32 SamplerSource |
| Time, RealTime | none |
| Sine | expression, uint32 bIsCosine |
| SquareRoot, Period, Periodic, Floor, Ceil, Frac, Abs, Saturate, Round, Sign, Truncate, Logarithm2, Logarithm10 | expression |
| FoldedMath | expression A, expression B, u8 Op, uint32 ValueType |
| AppendVector | A, B, uint32 NumComponentsA |
| Min, Max, Fmod | A, B |
| TrigMath | X, Y, u8 Op |
| Clamp | Input, Min, Max |
| ComponentSwizzle | X, int8 IndexR, IndexG, IndexB, IndexA, NumElements |

A type that is not in this table makes the export fall back to undecoded, so each row is
exercised only by exports that parse exactly with it. Types present in the data
(`uniform_expression_types` in `audit/package_facts.json`): Constant, VectorParameter,
ScalarParameter, Texture, TextureParameter, Time, RealTime, SquareRoot, Periodic, Floor, Ceil,
Frac, Abs, Saturate, Round, Sign, FoldedMath, AppendVector, Min, Max, Fmod, TrigMath, Clamp,
ComponentSwizzle. The rows for FlipBookTextureParameter, Sine, Period, Truncate, Logarithm2
and Logarithm10 are stock layouts that no file uses (unverified). The fifth expression array
is empty in all 5,364 shader maps.

All shader maps are valid, for platform 0 and feature level 3; shader targets have platform 0
and frequencies 0-5 (`values` in `audit/package_facts.json`).

## Shader record

```
int32  EndOffset        position in the virtual file "<.uasset><.uexp>" where this record ends
bytes  parameters       the shader class's own Serialize() output (bindings), see shader_parameters.md
common tail             FShader::SerializeBase
```

Common tail:

| Field | Type |
|---|---|
| OutputHash | 20 bytes |
| MaterialShaderMapHash | 20 bytes |
| ShaderPipeline | FName |
| VFType | FName |
| VFSourceHash | 20 bytes |
| Type | FName, equals the record's type |
| SourceHash | 20 bytes |
| Target | uint32 frequency, uint32 platform |
| uniform buffer parameters | int32 count + { FString struct name, uint16 BaseIndex, uint32 bIsBound } |
| Resource | FShaderResource |

FShaderResource: FName SpecificType, Target, 20-byte OutputHash, uint32 NumInstructions,
uint32 NumTextureSamplers, int32 UncompressedCodeSize, uint32 0 (code not shared), int32 count
+ code bytes.

Code bytes (D3D11 RHI): `FD3D11ShaderResourceTable` (uint32 ResourceTableBits and five uint32
arrays: ShaderResourceViewMap, SamplerMap, UnorderedAccessViewMap, ResourceTableLayoutHashes,
TextureMap), the **DXBC** container (third-party; signature `DXBC`, its length is the
container's own total-size field at offset 24), then optional data: { u8 key, int32 size,
data } entries and a final int32 holding the size of the optional block including itself.
Keys seen: `p` = 5 × u8 packed resource counts (global uniform buffer used, samplers, SRVs,
CBs, UAVs); `u` = int32 count + FString uniform buffer names.

### Finding the boundary between parameters and tail

The record gives no length for the parameters. The decoder takes the first position from
which the common tail parses and ends exactly at EndOffset with `Type` equal to the record's
type; the tail contains two self-delimiting arrays and the code array whose length must land
on EndOffset, so a false match would have to satisfy all of them. EndOffset itself was
checked on every record (it must fall inside the export, after the record start).

### Parameter bindings

`parameters` is the decoded block: a positional array following the type's layout in
`tools/lib/pkg_classes/shader_layouts.json`, which is derived from the executable
(`shader_parameters.md`, address table in `shader_parameters_table.md`). A block stays raw
(`parametersRaw`, or `parametersPartial` when only the FMeshMaterialShader part could be
read) only for the types the executable does not contain; those bytes are the
`shader parameters ...` entries under `undecoded_kinds` in the manifest.

## Global shader cache

`assets/Engine/GlobalShaderCache-PCD3D_SM5.bin`: uint32 `GSMB` (bytes `42 4D 53 47`), then the
same Shaders + ShaderPipelines lists, with every FName written as an FString because the file
has no name table, and EndOffset as a plain file offset. `tools/loose_files.js` decodes it to
`assets_converted/loose/Engine/GlobalShaderCache-PCD3D_SM5.bin.json` and rebuilds it
identically. Its parameter bindings are decoded with the same layouts; the exceptions (28
editor-only types and one mismatching record) are listed in `shader_parameters.md`.

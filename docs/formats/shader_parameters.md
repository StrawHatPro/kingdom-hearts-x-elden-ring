# Shader parameter bindings

Code: `tools/shader_layouts.js` (+ `tools/shader_layouts/`) derives the layouts from the
executable and writes `tools/lib/pkg_classes/shader_layouts.json`;
`tools/lib/pkg_classes/material.js` decodes with them. Address table (every Serialize used):
`shader_parameters_table.md` (generated). Evidence of the result: `shader parameters` under
`undecoded_kinds` in `audit/package_manifest.json` and the global shader cache row in
`audit/loose_files.json`; counts per type in `audit/package_facts.json`.

All addresses below are virtual addresses in `base/KH3.exe` (image base 0x140000000); the
decompiled C is `build/decomp/<chunk>.jsonl`.

## Where the bytes are

In a shader record (`material.md`) the bytes between EndOffset and the common tail are what the
shader object's virtual `Serialize(FArchive&)` writes. The executable has no symbols, so a
decoded block is a **positional array** in the order the executable serializes the members;
`shader_layouts.json` gives, per shader type, that order as `[kind, member offset]`. Member
offsets identify the fields; no member names are claimed. The three parameter structs are
named after the stock engine structs of the same shape.

## How a type's layout is found

1. **Type name → registration.** The type name (`FName` in the record) exists as a UTF-16
   string in the executable; the static initializer that references it constructs the
   `FShaderType` (e.g. `func_140327CA0` for `FHitProxyPS` calls `f_1431CDA10(&object, kind,
   name, source file, function name, frequency, ConstructSerializedType, ...)`; a few types have
   the constructor inlined and store the same pointers directly).
2. **ConstructSerializedType → vtable.** That function allocates the shader object
   (`f_14151C990(size)`) and stores its vtable (e.g. `func_14359B750`: size 0x2C8, vtable
   0x14731F458), possibly through an out-of-line constructor.
3. **Serialize = vtable slot 2** (`[vtable+0x10]`; e.g. `func_1433126C0` for `FHitProxyPS`).
   A missing decompilation there is a `jmp` thunk, followed with the disassembler.
4. **Flattening.** Serialize functions are chains of calls: `f(this, Ar, ...)` (the base
   class's Serialize), `f(Ar, this + off)` (a member's `operator<<`) and `f(this + off, Ar)`.
   The automatic reader (`tools/shader_layouts/expand.js`) follows them, including constant
   loops over member arrays, down to the primitive serializers below. A function it cannot
   read mechanically is reported and has to be entered by hand in
   `tools/shader_layouts/manual.js`; the run ends with 0 unresolved functions.

Result: 1,587 type names (of the 1,617 that occur in the data) map to 282 Serialize functions;
306 distinct layouts including the per-shader Scaleform ones.

## Primitive serializers

| Kind | Function | What it reads (file order) |
|---|---|---|
| `param` | 1431D8D20 | FShaderParameter: uint16 → +2, uint16 → +4, uint16 → +0 (stock: BaseIndex, NumBytes, BufferIndex). Value `[a, b, c]` in file order |
| `res` | 1431D8E40 | FShaderResourceParameter: uint16 → +0, uint16 → +2 (stock: BaseIndex, NumResources) |
| `ub` | 14329F550 | FShaderUniformBufferParameter: uint16 → +4, 32-bit bool → +6 (stock: BaseIndex, bIsBound) |
| `ubArr` | 1435C6800 | int32 count + `ub` |
| `paramArr` | 1435C6650 | int32 count + `param` |
| `dbgSet` | 1435C6B70 | six int32, stored to +0, +4, +0x10, +0x14, +8, +0xC |
| `bytes` | 142D998B0 | int32 count + bytes |
| `fstr` | 142CE4810 | FString |
| `name` | archive virtual +0x38 | FName (name-table index + number in a package, FString in the global shader cache) |
| `u32`, `b32` | inline | 4 bytes |
| `vfref` | 1431D8F00 | vertex factory parameter reference, below |
| `typedList` | 143600C90 | handler-typed list, below |

## FMaterialShader::Serialize (1435B4200, read by hand)

`ub`@0xC0, `ubArr`@0xC8, the composite at 0x118 (`f_14350BC20`, 67 `res` and one `ub`),
`res`@0x224, `res`@0x228, `dbgSet`@0x230, then - identical bytes in the saving and loading
branches - `name` (+0x260), `u32`@0x248, `u32`@0x24C, `bytes`@0x250 (the uniform buffer layout:
name, two sizes, resource list), `fstr`@0x270, `res`@0x22C, four `paramArr` at 0xD8, 0xE8, 0xF8,
0x108, and the `typedList` at 0x280. FMeshMaterialShader::Serialize (1435B4D70) adds `vfref`@0x290
and `param`@0x2B8.

## Vertex factory parameters (`vfref`, 1431D8F00)

FName of the vertex factory type (`f_1431D21B0`), uint8 shader frequency, 20 bytes
(`f_142D99970`), int32 file offset of the end of the parameters, then - when the type's
ConstructShaderParameters returned an object for that frequency - that object's Serialize
(`[vtable+0x10]`). When it returned null nothing follows (the block is empty: 0 bytes in every
such record). The parameter classes are found like shader types: vertex factory type name →
registration (`f_1431CDC60(&object, name, shader file, five flags, ConstructShaderParameters,
...)`) → the vtable stored for each frequency → Serialize. The 25 vertex factory types of the
data and their per-frequency classes are in the table document.

## Typed parameter list (143600C90)

int32 count; per entry uint32, int32 handler index, then the handler's own Serialize
(`[vtable+0x30]`). The handlers are the ten objects that `func_14360BBE0` appends to the table
at 0x149D0C090; their layouts (one to nineteen `param`/`res`/`ub`) are in the table document.
This list is not part of stock UE 4.17. In the data the uint32 is 0x1CEDCAFE in every entry (the value the loader also presets) and the handlers that occur are 0, 2, 3, 4 and 9 (`typed_parameter_handlers` in `audit/package_facts.json`).

## Scaleform shaders

`RHI::VertexShaderImpl<...>` (1438BB980): 28 `param` from +200, four `u32` from +0x170.
`RHI::FragShaderImpl<...>` (1438BBA40) is driven by data: the shader object's descriptor
(`[this+0xC0]`, a static pointer stored by each type's ConstructSerializedType) points at a
table of 28 uniforms, 10 bytes each (int16 location, int16 array size at +6, uint8 kind at +8).
A uniform with a negative location is skipped; kind 0 is a `param` at +200 + 6·i; otherwise the
function writes, per array element j, two `res` at (location + j + 0x5C)·4 and
(location + j + 0x60)·4 (the branch taken when the global at 0x149D1AC14 is 0, which is the
platform value all shader targets in the data carry). The 194 tables are read from the
executable's data section by `tools/shader_layouts/scaleform.js`.

## What stays uninterpreted, and why

| Case | Records | Reason |
|---|---|---|
| 30 type names with no string in the executable (`typesNotInExecutable` in the JSON: `FLandscapeGrassWeightVS/PS` in materials; editor shaders such as `FPostProcessCompositeEditorPrimitivesPS*`, `FPostProcessSelectionOutlinePS*`, `FAtmosphere*`, `TMeshPaint*`, `FSimpleElement*PS` in the global cache) | all records of those types | the shipping executable has no such shader class, so it cannot say how they are laid out; its loader skips such a record through EndOffset. For the two material types the FMeshMaterialShader fields are still decoded (accepted only when their vertex-factory reference names the record's own vertex factory) and only the bytes after them stay raw (`parametersPartial.extraRaw`) |
| `FRayTracedShadowHeightfieldsPS` in the global shader cache | 1 | the record holds 68 parameter bytes; the executable's class (registration 1402F3F20, vtable 14731FCF8, Serialize 143312EC0) serializes 130. The cached shader was not built from this class layout |

Every decoded block is checked the only way the data allows: the layout must consume the
block exactly (and a `vfref`'s end offset must fall on the end of its vertex-factory
parameters), otherwise the block stays raw.

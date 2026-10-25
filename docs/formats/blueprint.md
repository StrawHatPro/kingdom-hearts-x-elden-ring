# Blueprint classes: UField family

Code: `tools/lib/pkg_classes/ufield.js`, `tools/lib/kismet.js`. Evidence: the classes below
in `audit/package_manifest.json` (every export decoded and round-trip identical except those
in the six mismatched packages, see `package_json.md`).

All layouts follow the common export framing (properties - always just `None` for these
classes - and the object-guid flag), then:

## Properties (`*Property` classes)

| Field | Type | Notes |
|---|---|---|
| Next | package index | UField::Next, next field of the owner |
| ArrayDim | int32 | |
| PropertyFlags | uint64 | |
| RepNotifyFunc | FName | |

Stock UE 4.17 also has ElementSize and RepIndex in memory; neither is in the file: a plain
`IntProperty` export is 12 + 24 bytes for all 67,8xx exports.

Then per class:

| Class | Extra fields |
|---|---|
| Int, Float, Str, Name, Text, Int8/16/64, UInt16/32/64, Double | none |
| Object, WeakObject, LazyObject, AssetObject | PropertyClass (package index) |
| Class, AssetClass | PropertyClass, MetaClass |
| Struct | Struct |
| Array | Inner |
| Set | ElementProp |
| Map | KeyProp, ValueProp |
| Byte | Enum |
| Enum | Enum, UnderlyingProp |
| Bool | u8 BoolSize, u8 NativeBool |
| Delegate, MulticastDelegate | SignatureFunction |
| Interface | InterfaceClass |

## UStruct part (Function, classes, structs)

| Field | Type |
|---|---|
| Next | package index |
| SuperStruct | package index |
| Children | package index (first child field) |
| script | int32 in-memory size, int32 serialized size, bytecode (`kismet.md`) |

## Function, DelegateFunction

UStruct part, then uint32 FunctionFlags, int16 RepOffset only when `FunctionFlags & 0x40`
(FUNC_Net), package index EventGraphFunction, int32 EventGraphCallOffset.

## BlueprintGeneratedClass, WidgetBlueprintGeneratedClass, AnimBlueprintGeneratedClass

UStruct part, then:

| Field | Type |
|---|---|
| FuncMap | int32 count + { FName, package index } |
| ClassFlags | uint32 |
| ClassWithin | package index |
| ClassConfigName | FName |
| ClassGeneratedBy | package index (0 in cooked data) |
| Interfaces | int32 count + { package index Class, int32 PointerOffset, uint32 bImplementedByK2 } |
| bDeprecatedForceScriptOrder | uint32 bool |
| Dummy | FName (`None`) |
| bCooked | uint32 bool |
| ClassDefaultObject | package index |

The generated-class subclasses add no native data (their extra state is in properties).

## ScriptStruct, UserDefinedStruct

UStruct part + uint32 StructFlags.

## Enum, UserDefinedEnum

Next, int32 count + { FName, int64 value }, u8 CppForm. The display names of a
UserDefinedEnum are a `MapProperty` (Name → Text) in its properties.

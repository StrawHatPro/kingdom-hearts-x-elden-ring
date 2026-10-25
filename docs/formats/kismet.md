# Kismet (Blueprint VM) bytecode

Code: `tools/lib/kismet.js`. Every `Function` export's script (and the empty scripts of
classes and structs) is disassembled to an expression tree and assembled back; see `Function`
in `audit/package_manifest.json`.

## Container

```
int32  in-memory size of the script
int32  serialized size
bytes  serialized script: expressions until the serialized size is used up
```

The serialized form differs from the in-memory script in two operand kinds only:

| Operand | On disk | In memory |
|---|---|---|
| object pointer | int32 package index | 8 bytes |
| FName | int32 name index + int32 number | 12 bytes |

Jump targets and skip sizes are in-memory offsets. The disassembler therefore tracks the
in-memory offset of every expression (`at` in the JSON) and checks that the total equals the
recorded in-memory size - this holds for every script, which confirms the operand widths of
every opcode that occurs. On write the in-memory size is recomputed.

## Expression tree

`{ "op": name, "at": in-memory offset, ...operands }`. Operand kinds: `p` object pointer,
`n` name, `e` nested expression, `E` expressions up to and including an end marker, fixed-width
integers and floats, `s` zero-terminated ANSI string, `S` zero-terminated UTF-16 string.

| Code | Name | Operands |
|---|---|---|
| 00 / 01 / 02 / 48 | LocalVariable / InstanceVariable / DefaultVariable / LocalOutVariable | property:p |
| 04 | Return | value:e |
| 06 | Jump | target:u32 |
| 07 | JumpIfNot | target:u32, condition:e |
| 09 | Assert | line:u16, debugOnly:u8, condition:e |
| 0B | Nothing | |
| 0F | Let | property:p, variable:e, value:e |
| 12 / 19 / 1A | ClassContext / Context / Context_FailSilent | object:e, skip:u32, rvalueProperty:p, context:e |
| 13 / 2E | MetaCast / DynamicCast | class:p, value:e |
| 14 / 5F / 60 / 43 / 44 | LetBool / LetObj / LetWeakObjPtr / LetMulticastDelegate / LetDelegate | variable:e, value:e |
| 15 / 16 | EndParmValue / EndFunctionParms | |
| 17 | Self | |
| 18 | Skip | skip:u32, value:e |
| 1B | VirtualFunction | function:n, params:E EndFunctionParms |
| 1C / 68 | FinalFunction / CallMath | function:p, params:E EndFunctionParms |
| 63 | CallMulticastDelegate | signature:p, params:E EndFunctionParms |
| 1D / 1E / 24 / 2C / 35 / 36 | IntConst / FloatConst / ByteConst / IntConstByte / Int64Const / UInt64Const | value |
| 1F / 34 | StringConst / UnicodeStringConst | value:s / value:S |
| 20 / 21 | ObjectConst / NameConst | object:p / value:n |
| 22 / 23 / 2B | RotationConst / VectorConst / TransformConst | 3 / 3 / 10 × f32 |
| 25 / 26 / 27 / 28 / 2A / 2D | IntZero / IntOne / True / False / NoObject / NoInterface | |
| 29 | TextConst | u8 literal type: 0 empty; 1 localized: source:e, key:e, namespace:e; 2 invariant / 3 literal string: source:e; 4 string table: tableAsset:p, tableId:e, key:e |
| 2F / 30 | StructConst / EndStructConst | struct:p, serializedSize:i32, members:E |
| 31 / 32 | SetArray / EndArray | array:e, elements:E |
| 39 / 3A, 3B / 3C | SetSet / EndSet, SetMap / EndMap | target:e, count:i32, elements:E |
| 3D / 3E | SetConst / EndSetConst | innerProperty:p, count:i32, elements:E |
| 3F / 40 | MapConst / EndMapConst | keyProperty:p, valueProperty:p, count:i32, elements:E |
| 65 / 66 | ArrayConst / EndArrayConst | innerProperty:p, count:i32, elements:E |
| 38 | PrimitiveCast | castType:u8, value:e |
| 42 | StructMemberContext | property:p, struct:e |
| 4B | InstanceDelegate | function:n |
| 4C / 4D / 4F | PushExecutionFlow / PopExecutionFlow / PopExecutionFlowIfNot | target:u32 / - / condition:e |
| 4E | ComputedJump | target:e |
| 50 / 5A / 5E | Breakpoint / WireTracepoint / Tracepoint | |
| 51 | InterfaceContext | value:e |
| 52 / 54 / 55 | ObjToInterfaceCast / CrossInterfaceCast / InterfaceToObjCast | class:p, value:e |
| 53 | EndOfScript | |
| 5B | SkipOffsetConst | value:u32 |
| 5C / 62 | AddMulticastDelegate / RemoveMulticastDelegate | delegate:e, other:e |
| 5D | ClearMulticastDelegate | delegate:e |
| 61 | BindDelegate | function:n, delegate:e, object:e |
| 64 | LetValueOnPersistentFrame | property:p, value:e |
| 67 | AssetConst | value:e |
| 69 | SwitchValue | u16 case count, u32 end offset, index:e, per case { value:e, u32 next offset, result:e }, default:e |
| 6A | InstrumentationEvent | u8 type; type 4: name |
| 6B | ArrayGetByRef | array:e, index:e |
| FE | CallFunctionByPath_0xFE | function:n, params:E EndFunctionParms |

Opcode names are those of the stock UE 4.17 `EExprToken` enumeration; operand layouts are
those of the stock serializer and are confirmed by the size check above for each opcode
present in the data.

`audit/package_facts.json` (`kismet_opcodes`) counts the opcodes over all converted
functions (53,436 scripts, 11,443,639 expressions, superseded copies included). Sixty are
present: LocalVariable, InstanceVariable, DefaultVariable, LocalOutVariable, Return, Jump,
JumpIfNot, Nothing, Let, LetBool, LetObj, LetValueOnPersistentFrame, ClassContext, Context,
EndFunctionParms, Self, VirtualFunction, FinalFunction, CallMath, CallMulticastDelegate,
IntConst, FloatConst, ByteConst, StringConst, UnicodeStringConst, ObjectConst, NameConst,
RotationConst, VectorConst, TransformConst, True, False, NoObject, NoInterface, TextConst,
DynamicCast, PrimitiveCast, ObjToInterfaceCast, InterfaceContext, StructConst, EndStructConst,
SetArray, EndArray, ArrayConst, EndArrayConst, StructMemberContext, PushExecutionFlow,
PopExecutionFlow, PopExecutionFlowIfNot, ComputedJump, EndOfScript, SkipOffsetConst,
AddMulticastDelegate, RemoveMulticastDelegate, ClearMulticastDelegate, BindDelegate, AssetConst,
SwitchValue, ArrayGetByRef, CallFunctionByPath_0xFE. The other rows of the table are the stock
layouts and are **not exercised by any file** (unverified).

## The executable's native table

`tools/kismet_natives.js` → `audit/kismet_natives.json` lists what the executable registers in
the VM's native table (GNatives, 0x149C6E260; the interpreter dispatches through
`(&g_149C6E260)[opcode]`, e.g. in `func_142F4AEB0`). Each registration is a static initializer
of the form `lea rax,[handler]; mov [GNatives + 8·opcode],rax`. 79 opcodes are registered and
every one of them has a row above; the rows without a registration are Return and the end
markers (EndParmValue, EndStructConst, EndArray, EndSet, EndMap, EndSetConst, EndMapConst,
EndArrayConst) and Skip, which other handlers consume.

## Opcode 0xFE

Not a stock opcode. It occurs 14 times, in four functions of the live tree (for example
`ExecuteUbergraph_he_01_gimmick_workshop` in `Levels/he/he_01/umap/he_01_gimmick_workshop.umap`).
Its operand layout - a name and a parameter list, as `VirtualFunction` - is fixed by the
enclosing `Context` skip size (41 = 1 + 12 + 3 × 9 + 1) and by the script size check.

Its handler is GNatives[0xFE] = `func_142F60EB0` (stored by the initializer at 0x1402D1C0B).
The handler reads the 12-byte name from the script, converts it to a string
(`f_142DED220`), calls `f_142F797B0(UFunction::StaticClass(), nullptr, string, nullptr)` - the
first argument comes from `func_142E54480`, the class getter that registers the class named
`Function` of `/Script/CoreUObject` - and passes the object found to `f_142F4AEB0(this, Stack,
Result, Function)`. The stock handler for `VirtualFunction`, GNatives[0x1B] = `func_142F62AB0`,
ends in the same `f_142F4AEB0` but obtains the function with `f_142F51440(this, name)` (a
lookup on the context object). So 0xFE calls a function that is located by its object path
rather than by name on the object. The decoder's name `CallFunctionByPath_0xFE` describes that
(confidence: inferred from the handler; the engine's own name for it is not known).

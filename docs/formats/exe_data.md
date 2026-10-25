# Non-code sections of the executable

What `tools/datamap.js` identifies in `.rdata`, `.data` (+ `.bss`), `.pdata`, `_RDATA`, `.rsrc`
and `.reloc` of `base/KH3.exe`, and how each structure is recognised.

Every number here is copied from the output of `node tools/datamap.js`
(`tool_log:2026-10-08T05:32:33.493Z#datamap`, also stored in `audit/data_map_summary.json`).
The map itself is `audit/data_map.json`: 1,176,631 rows
`{start, end, section, kind, symbol, type, confidence, how}`, one JSON object per line, ordered by
address (263,737,326 bytes; it parses with a single `JSON.parse`).

The tool reads only the binary. It does not read `audit/symbol_coverage.json`, Ghidra exports or
any other audit file, and it does not modify `asm/` or the coverage map.

## 1. Result

The tool asserts that the rows tile each section's full extent `max(VirtualSize, SizeOfRawData)`
with no gap, no overlap and no row crossing a section end, and exits non-zero otherwise.

| Section | Bytes | Rows | Unknown bytes | Unknown % |
|---|---|---|---|---|
| `.rdata` | 44,870,144 | 963,327 | 1,447,043 | 3.225 |
| `.data` (977,920 file-backed + `.bss`) | 27,897,536 | 207,999 | 391,377 | 1.403 |
| `.pdata` | 3,801,088 | 2 | 0 | 0.000 |
| `_RDATA` | 2,048 | 30 | 1,376 | 67.188 |
| `.rsrc` | 136,192 | 56 | 5 | 0.004 |
| `.reloc` | 4,192,768 | 5,217 | 0 | 0.000 |

Counted from the written file: rows by `kind`: rodata 685,183; padding 271,911; bss 134,424; data 48,701; unknown 36,412.
By `confidence`: confirmed 370,001; inferred 715,824; guess 90,806.

`kind` follows the splitter's convention (`.rdata` and `_RDATA` are `rodata`; `.data`, `.pdata`,
`.rsrc`, `.reloc` are `data`; rows in the uninitialised part of `.data` are `bss`); `padding` and
`unknown` override it. The fine classification is in `type`.

Symbols are a prefix that reflects the type plus the start address (`str_`, `wstr_`, `vtbl_`,
`fptrs_`, `ptr_`, `unwind_`, `ehfuncinfo_`, `rtti_col_`, `D_`, `B_`, `pad_`, `unk_`, ...). No
semantic names are assigned.

## 2. Evidence sources

The first word of `how` names the source.

| Tag | Source | Strength |
|---|---|---|
| `dir` | A PE data directory, or a structure reached by following fields from one | exact |
| `pogo` | The linker's section-contribution list in the `IMAGE_DEBUG_TYPE_POGO` debug record (58 entries: `.idata$5`, `.CRT$XCU`, `.rdata$r`, `.xdata`, `.didat$7`, `.bss`, ...) | exact for extents |
| `reloc` | The base relocation table. Every absolute pointer in the image has a `DIR64` entry, so "this qword is a pointer" is a fact | exact for pointer-ness |
| `xref` | A RIP-relative operand of an instruction in `.text`, or the target of a relocated pointer | exact that the address is used; the object's extent is inferred |
| `content` | Byte patterns (printable text, zero fill) | weak alone; `guess` unless combined |

### Relocations

5,216 blocks, 2,074,589 `DIR64` entries, 853 `ABSOLUTE` entries (block padding), no other type.
All 2,074,589 relocated qwords lie in the data sections (none in `.text`). After the map is built
the tool checks where each one landed: all are inside pointer-typed rows and none straddles a row
boundary.

| Row type holding them | Relocated qwords |
|---|---|
| `code_pointer_table` | 1,949,276 |
| `crt_init_table` | 58,804 |
| `ue4_native_function_table` | 19,760 |
| `data_pointer` | 17,111 |
| `data_pointer_table` | 9,843 |
| `mixed_pointer_table` | 9,085 |
| `vtable_rtti` | 4,735 |
| `code_pointer` | 3,958 |
| `rtti_type_descriptor` | 789 |
| `rtti_vtable_locator_pointer` | 680 |
| `delay_import_address_table`, `delay_import_unload_table` | 269 each |
| `tls_directory` | 4 |
| `load_config_directory` | 3 |
| `guard_cf_check_pointer`, `guard_cf_dispatch_pointer`, `tls_callback_table` | 1 each |

### References from `.text`

`.text` is decoded with iced-x86: each of the 316,750 `.pdata` ranges from its start, and the
regions between them with the same rule as `tools/split.js` (skip fill bytes, accept a block only
if it decodes cleanly to a terminator with no pending forward branch).

- 20,946,920 instructions decoded; 113,446 gap blocks accepted, 230 rejected.
- 1,322,311 RIP-relative operands, 1,220,798 of them into the data sections.
- For each target the tool records whether the address is taken (`lea`) or accessed, the operand
  width, and whether the access is an SSE/x87 float form.
- 956 `.pdata` ranges stop decoding at an invalid instruction (data inside the range, e.g. jump
  tables in hand-written assembly); decoding of that range ends there. References collected from
  bytes decoded before that point, or from data that happens to decode, can be false. A false
  reference can only split an `unknown`/`bss` row or propose a string/scalar start; it cannot
  override `dir`, `reloc` or RTTI rows, which are claimed first.

### Claim order

Items are claimed in this order; a later claim that overlaps an earlier one is refused and
counted. In the final run no claim was refused. 46,479 claims were exact repeats of an existing
range (`eh_unwind_map` shared by several `FuncInfo`s after identical-data folding).

1. PE directory structures
2. unwind info and EH tables
3. RTTI
4. `.CRT$X*` / `.rtc$*` contributions
5. UE4 native-function tables, then all remaining relocated qwords as pointer runs
6. referenced strings, then referenced scalars
7. unreferenced strings
8. fill: padding, bss, unknown

## 3. PE structures

| Type | Count | Bytes | How it is found |
|---|---|---|---|
| `import_descriptor_table` | 1 | 920 | Import directory: 45 `IMAGE_IMPORT_DESCRIPTOR` + null |
| `import_lookup_table` | 45 | 6,712 | `OriginalFirstThunk` array per DLL, null-terminated |
| `import_address_table` | 45 | 6,712 | `FirstThunk` array per DLL. The union equals the IAT directory (checked) |
| `import_hint_name` | 1,009 | 23,998 | `IMAGE_IMPORT_BY_NAME` entries of normal and delay imports |
| `import_dll_name` | 45 | 793 | `IMAGE_IMPORT_DESCRIPTOR.Name` |
| `delay_import_descriptor_table` | 1 | 512 | Delay-import directory: 15 `ImgDelayDescr` + null |
| `delay_import_name_table` | 15 | 2,272 | `rvaINT` |
| `delay_import_address_table` | 15 | 2,272 | `rvaIAT`, in `.data`; slots are relocated pointers to load thunks |
| `delay_import_unload_table` | 15 | 2,272 | `rvaUnloadIAT` |
| `delay_import_bound_table` | 1 | 2,272 | `rvaBoundIAT`, see below |
| `delay_import_module_handle` | 15 | 120 | `rvaHmod`, 8 bytes each, in `.bss` |
| `delay_import_dll_name` | 15 | 240 | `rvaDLLName` |
| `export_directory` | 1 | 40 | Export directory |
| `export_address_table` / `export_name_pointer_table` / `export_ordinal_table` | 1 each | 1,072 / 1,072 / 536 | 268 functions, 268 names |
| `export_name` | 268 | 12,076 | Name strings |
| `export_dll_name` | 1 | 13 | `IMAGE_EXPORT_DIRECTORY.Name` |
| `debug_directory` | 1 | 84 | 3 `IMAGE_DEBUG_DIRECTORY` entries |
| `debug_codeview` | 1 | 37 | Type 2: `RSDS` record, `TresGame.pdb` |
| `debug_vc_feature` | 1 | 20 | Type 12 |
| `debug_pogo` | 1 | 1,112 | Type 13: the section-contribution list |
| `load_config_directory` | 1 | 256 | Load-config directory (`Size` field 256, `GuardFlags` 0x100) |
| `security_cookie` | 1 | 8 | `SecurityCookie` = 0x1487B4038 |
| `guard_cf_check_pointer`, `guard_cf_dispatch_pointer` | 1 each | 8 each | 0x145C0EA38, 0x145C0EA40 (contribution `.00cfg`) |
| `tls_directory` | 1 | 40 | `IMAGE_TLS_DIRECTORY64` |
| `tls_template` | 1 | 76 | `StartAddressOfRawData`..`EndAddressOfRawData` |
| `tls_index` | 1 | 4 | `AddressOfIndex`, in `.bss` |
| `tls_callback_table` | 1 | 16 | 1 callback + null |
| `runtime_function_table` | 1 | 3,801,000 | Exception directory: 316,750 `RUNTIME_FUNCTION` of 12 bytes, one row |
| `resource_directory` | 21 | 624 | `IMAGE_RESOURCE_DIRECTORY` nodes of the tree |
| `resource_data_entry` | 16 | 256 | `IMAGE_RESOURCE_DATA_ENTRY` |
| `resource_data` | 16 | 135,115 | Data blobs named by the entries |
| `base_reloc_block` | 5,216 | 4,192,612 | One row per `IMAGE_BASE_RELOCATION` block; the directory is consumed exactly |

Imports: 794 functions (51 by ordinal). Delay imports: 269 functions (3 by ordinal).

Delay-import bound IAT: the 15 `rvaBoundIAT` values are closer together than each DLL's slot
count, so reading them as `n + 1` slots each makes them overlap. The area is instead taken as the
single linker contribution `.didat$7` (2,272 bytes) that contains all 15 targets and is entirely
zero (nothing is bound).

## 4. Unwind information and exception-handling tables

Each `RUNTIME_FUNCTION.UnwindInfoAddress` names an `UNWIND_INFO` in `.rdata` (contribution
`.xdata`). Chains are followed. 162,449 distinct records, all version 1.

Record size = 4-byte header + unwind codes (padded to an even count) + one of: nothing; a chained
`RUNTIME_FUNCTION` (12 bytes); handler RVA + language-specific data. One row per record,
language-specific data included.

| Type | Count | Bytes | Contents after the unwind codes |
|---|---|---|---|
| `unwind_info` | 18,965 | 360,708 | nothing |
| `unwind_info_chained` | 69,030 | 1,433,908 | parent `RUNTIME_FUNCTION` |
| `unwind_info_cxx_eh` | 72,059 | 1,686,288 | handler RVA + `FuncInfo` RVA (+ `GS_HANDLER_DATA`) |
| `unwind_info_gs` | 2,279 | 65,288 | handler RVA + `GS_HANDLER_DATA` |
| `unwind_info_seh` | 31 | 1,172 | handler RVA + `SCOPE_TABLE` (+ `GS_HANDLER_DATA`); 34 scope records |
| `unwind_info_custom_handler` | 85 | 1,264 | handler RVA + data of unknown layout |

The layout of the language-specific data depends on the handler, so the 41 distinct handlers are
classified from their code:

| Handler | Unwind infos | Class | Evidence |
|---|---|---|---|
| 0x1458718EC | 46,134 | C++ frame handler | `jmp [IAT]` thunk to `VCRUNTIME140.dll!__CxxFrameHandler3` |
| 0x14586FFF0 | 25,925 | GS + C++ | calls another routine, then that thunk (shape of `__GSHandlerCheck_EH`) |
| 0x14586FF74 | 2,279 | GS only | its only call is to 0x14586FF94, the routine the two GS+EH handlers share (shape of `__GSHandlerCheck`) |
| 0x1458718F8 | 28 | SEH | `jmp [IAT]` thunk to `VCRUNTIME140.dll!__C_specific_handler` |
| 0x145870070 | 3 | GS + SEH | calls another routine, then that thunk (shape of `__GSHandlerCheck_SEH`) |
| 36 others | 85 | custom | ordinary code that calls none of the above (hand-written assembly) |

The names `__GSHandlerCheck*` are not asserted; only the data layout that follows from the call
shape is used. `GS_HANDLER_DATA` is 4 bytes, or 12 when its `HasAlignment` bit is set. For the 85
custom records the handler data is taken to run to the next unwind record (at most 64 bytes);
those rows are `inferred`.

C++ EH tables, reached from the `FuncInfo` RVA in the handler data (`dir>unwind` in `how`):

| Type | Count | Bytes | Layout |
|---|---|---|---|
| `eh_funcinfo` | 71,877 | 2,875,080 | 40 bytes, magic 0x19930522 in all 71,877; none with another magic |
| `eh_unwind_map` | 24,929 | 1,438,824 | `maxState` x 8 bytes {toState, action RVA} |
| `eh_ip2state_map` | 71,729 | 4,983,416 | `nIPMapEntries` x 8 bytes {IP RVA, state}; 622,927 entries |
| `eh_tryblock_map` | 155 | 3,440 | `nTryBlocks` x 20 bytes; 172 try blocks |
| `eh_handler_array` | 172 | 3,560 | `nCatches` x 20 bytes `HandlerType`; 178 catch handlers |

The 24,929 unwind maps are distinct ranges; 46,479 further `FuncInfo`s point at one of them.

## 5. RTTI

| Type | Count | Bytes | How it is found |
|---|---|---|---|
| `rtti_complete_object_locator` | 680 | 16,320 | 24 bytes at 4-byte alignment: signature 1, `pSelf` equals the record's own RVA, type descriptor name starts with `.?A` (same test as `tools/rtti.js`, which also finds 680) |
| `rtti_type_descriptor` | 789 | 43,142 | 16 bytes + decorated name + NUL, in `.data` (contribution `.data$r`) |
| `rtti_class_hierarchy_descriptor` | 646 | 10,336 | 16 bytes, named by a locator or a base class descriptor |
| `rtti_base_class_array` | 646 | 10,556 | `numBaseClasses` RVAs, plus the zero terminator when present |
| `rtti_base_class_descriptor` | 673 | 18,844 | 28 bytes, listed in a base class array |
| `rtti_vtable_locator_pointer` | 680 | 5,440 | `vtable[-1]`: a relocated qword whose target is a locator |
| `vtable_rtti` | 680 | 37,880 | the relocated pointers into `.text` that follow it; 4,735 slots (matches `tools/rtti.js`) |

Type descriptors come from three routes: named by a locator or base class descriptor (646),
named only by an EH `HandlerType.pType` (1), and found by shape alone (142: a relocated pointer to
the `type_info` vftable 0x147C82DF0, a null spare field and a name starting with `.`).
No locator is left without a vtable.

## 6. Pointer tables

Linker-ordered tables named by POGO:

| Type | Count | Bytes | What |
|---|---|---|---|
| `crt_init_table` | 19 | 472,520 | `.CRT$XCA`..`.CRT$XTZ` and `.rtc$*` contributions (the TLS callback slot inside `.CRT$XL*` is its own row). 58,804 slots are relocated pointers; `.CRT$XCU`, the C++ static-initialiser list, is 470,384 bytes of them |

UE4 native-function tables:

| Type | Count | Bytes | What |
|---|---|---|---|
| `ue4_native_function_table` | 568 | 158,080 | 2 or more consecutive 16-byte pairs {relocated pointer to a C identifier string in `.rdata`, relocated pointer into `.text`}; 9,880 pairs. 152 single pairs are left to the generic pointer rows. `inferred`: the shape is that of `FNativeFunctionRegistrar` tables, the interpretation rests on `tools/names.js` |

`tools/names.js` reports 569 tables; it requires the function pointer to be a function start in the
coverage map rather than any relocated `.text` pointer, so its run boundaries differ slightly.

Everything else that carries a relocation is grouped into runs of consecutive relocated qwords.
A run is cut where a qword's own address is referenced (by `lea`, a memory operand or another
pointer), because that is where a separately addressed object starts.

| Type | Count | Bytes | What |
|---|---|---|---|
| `code_pointer_table` | 27,911 | 15,594,208 | 2 or more pointers, all into `.text` |
| `code_pointer` | 3,958 | 31,664 | a single pointer into `.text` |
| `data_pointer_table` | 2,364 | 78,744 | 2 or more pointers, all into data sections |
| `data_pointer` | 17,111 | 136,888 | a single pointer into a data section |
| `mixed_pointer_table` | 3,008 | 72,680 | a run with both kinds |

Of the 1,957,096 code pointers in these runs, 699,993 point at the start of a `.pdata` function,
none points inside a `.pdata` function body, and 1,257,103 point at code that has no `.pdata`
entry (leaf functions need none); 1,246,197 of those targets are 16-byte aligned.

Vtables without RTTI (the engine and game code are compiled without it): 27,175 of the
`code_pointer_table` rows start at an address that `.text` takes with `lea`, as a constructor
does when it stores the vtable pointer, and have no pointer into a function body. They are named
`vtbl_<VA>` with confidence `inferred`; the remaining tables are `fptrs_<VA>`. That the bytes are
code pointers is `confirmed` either way.

## 7. Strings

A string row covers the characters and the terminator; alignment fill after it is a separate
padding row.

| Type | Count | Bytes | Rule |
|---|---|---|---|
| `string_utf16` | 222,483 | 9,511,168 | starts at a referenced address; UTF-16LE units all printable; NUL unit |
| `string_ascii` | 47,040 | 1,026,697 | starts at a referenced address; printable bytes (0x20-0x7E, tab, CR, LF); NUL |
| `string_utf16_unreferenced` | 26,551 | 2,233,894 | no reference found; ASCII-range units only |
| `string_ascii_unreferenced` | 20,399 | 230,960 | no reference found |

Referenced strings (`inferred`): minimum 4 characters. Strings are claimed before scalars because
the compiler copies short literals inline with `movups`/`movsd`/`mov` loads from the start and the
middle of the literal; such loads would otherwise look like numeric constants. One guard works
the other way: an address that is only ever read as a 4- or 8-byte float is not taken as ASCII
text unless the text is more than twice as long as the load (0.7f is the bytes `333?`).
2,576 of the UTF-16 strings contain units outside ASCII (accepted ranges: Latin-1 to U+27FF,
U+3000-U+9FFF, Hangul, CJK compatibility, full-width forms).

Short strings (`guess`), 5,244 of the referenced rows: 1 to 3 characters, accepted only if nothing
but zero fill follows up to the next taken address or item. When the address is only read, not
taken, at least 2 UTF-16 units or 3 bytes are required, since a small integer looks the same as a
one-character string.

Unreferenced strings (`guess`): text that neither a relocation nor a RIP-relative operand points
at. A brute-force search of `.text` for any 32-bit displacement resolving to a sample of these
addresses found none, so they are dead literals the linker kept, not missed references. Minimum
length 8 bytes / 6 units, or 4 when the text directly follows another string.

## 8. Scalars and constants

For a referenced address that is not text, when every RIP-relative operand naming it has the same
width, the row is that wide (`inferred`); the object may be the first element of a larger array.

| Type | Count | Bytes | Evidence |
|---|---|---|---|
| `float32` | 1,835 | 7,340 | all accesses are 4-byte SSE/x87 float forms |
| `float64` | 322 | 2,576 | all accesses are 8-byte float forms |
| `simd128_const` | 523 | 8,368 | 16-byte accesses, at least one a packed-float form |
| `scalar8` / `scalar16` / `scalar32` / `scalar64` / `scalar128` | 341 / 498 / 1,987 / 13,873 / 227 | 341 / 996 / 7,948 / 110,984 / 3,632 | integer-form accesses of that width (a 4- or 8-byte row with mixed float and integer accesses is also `scalarN`) |

13,715 of the 13,873 `scalar64` rows are 8-byte globals in the initialised part of `.data`.

## 9. Padding, bss, unknown

| Type | Count | Bytes | Meaning |
|---|---|---|---|
| `zero_padding` | 271,906 | 1,258,887 | zero bytes between items whose start nothing references. `inferred` under 16 bytes, `guess` otherwise (a long zero run could be unreferenced zero data) |
| `section_tail_padding` | 5 | 1,006 | raw bytes past `VirtualSize` (file alignment) |
| `bss_referenced` | 134,405 | 26,919,720 | uninitialised storage starting at a referenced address and running to the next one |
| `bss` | 3 | 28 | uninitialised storage whose start is not referenced |
| `unknown_referenced` | 4,779 | 1,073,853 | start is referenced, contents not identified; symbol `D_<VA>` |
| `unknown_referenced_zero` | 5,751 | 121,407 | the same, contents all zero |
| `unknown` | 25,882 | 644,541 | not referenced, not uniform fill; symbol `unk_<VA>` |

`.bss` starts at 0x1487C6B00, the start of the POGO `.bss` contribution. That is 256 bytes before
the end of the raw data of `.data`; those 256 file-backed bytes are zero (checked) and are
classified as bss. A `bss_referenced` row's extent is the distance to the next referenced address,
not a proven object size. `--no-bss-split` emits the gaps unsplit.

What the unknown bytes are, from inspection of the largest rows (not classified by the tool):

- lookup tables referenced from code, the largest being 262,144, 151,552 and 131,072 bytes
  (float tables, CRC/AES-style tables, Unicode property data);
- in `.data`, 8-byte integer fields that follow a single relocated pointer (structures made of a
  pointer plus counters);
- `_RDATA`: one referenced block of 1,376 bytes at the start of the section, followed by 16-byte
  SIMD constants.

## 10. Looked for and not found, or not confirmed

- Export forwarders: 0.
- Bound-import directory, COM descriptor, global-pointer and architecture directories: empty in
  the header.
- Control-flow-guard function table, address-taken IAT table, long-jump table: counts are 0 in
  the load config. Only the two guard function-pointer slots exist.
- `FuncInfo` with magic other than 0x19930522, and `pESTypeList`: 0.
- Version-2 unwind info, compressed (`__CxxFrameHandler4`) EH data: 0 (all 162,449 records are
  version 1).
- Named resources (`IMAGE_RESOURCE_DIR_STRING_U`): 0; all resource ids are numeric. 5 non-zero
  bytes between resource blobs are left `unknown`.
- Relocations of a type other than `DIR64`/`ABSOLUTE`, and relocations inside `.text`: 0.
- Layout of the handler data of the 36 assembly-language handlers: not decoded.
- Per-descriptor extents of the delay-import bound IAT: inconsistent, see section 3.
- Whether a `code_pointer_table` is a C++ vtable or another function-pointer array: only
  inferred from the `lea` reference; 736 tables have no such reference.
- The 1,257,103 code pointers to code without a `.pdata` entry cannot be checked against a
  function-start list derived from the binary alone.
- Floating-point pools as arrays: only individually referenced constants are typed.
- Image-relative (RVA) references from code or data to data (other than those in PE, unwind, EH
  and RTTI structures) are not tracked, so an object reached only that way appears unreferenced.

## 11. Running

```
node tools/datamap.js                 writes audit/data_map.json, audit/data_map_summary.json, logs the run
node tools/datamap.js --no-write      analysis and self-check only
node tools/datamap.js --no-bss-split  do not split .bss at referenced addresses
node tools/datamap.js --ref-cache=F   cache the .text scan in file F (development aid)
```

About 20 seconds, single process.

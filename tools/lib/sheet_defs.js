// Declarations of every method sheet: which file, which columns, and for each column its role
// (key | value | ref), type, ref target and note. tools/extract.js writes sheets/schema.json and
// sheets/index.json from this table, so a column exists in exactly one place.
//
// Column tuple: [name, role, type, ref target ('-' when none), note, cold]
//   type: str | int | num | addr (0x-hex address) | hex (0x-hex value) | yesno | list | json |
//         enum:a|b|c .  "-" (deliberately none) is legal in a column of any type.
//   ref target: one or more sheet kinds joined with "|"; a list cell is a list of refs.
//   cold: true for provenance columns that tools/q.js leaves out unless they are asked for.
// Row ids are the names the game itself uses (class names, addresses, asset paths); they are
// not rewritten to snake_case.
const K = 'key', V = 'value', R = 'ref';
const DEFS = {
  // ---- meta
  schema: { group: 'meta', file: 'schema.json', by: 'tools/extract.js', from: 'tools/lib/sheet_defs.js', cols: [
    ['id', K, 'str', '-', '<sheet>.<column>'], ['sheet', R, 'str', 'sheets', 'sheet kind'], ['column', V, 'str', '-', '-'],
    ['role', V, 'enum:key|value|ref', '-', 'key = unique row id; ref = must resolve in the target sheet'],
    ['type', V, 'str', '-', 'cell type; preflight checks every checked cell against it'],
    ['ref', V, 'str', '-', 'target sheet kind(s) of a ref column'], ['cold', V, 'yesno', '-', 'provenance column, left out of tools/q.js output unless named'],
    ['note', V, 'str', '-', 'what the column holds and where it comes from']] },
  sheets: { group: 'meta', file: 'index.json', by: 'tools/extract.js', from: 'tools/lib/sheet_defs.js', cols: [
    ['id', K, 'str', '-', 'sheet kind'], ['group', V, 'str', '-', 'directory under sheets/'], ['file', V, 'str', '-', 'file, or directory when the kind is split over several files'],
    ['built_by', V, 'str', '-', 'tool that writes it'], ['evidence', V, 'str', '-', 'what it is built from'],
    ['min_rows', V, 'int', '-', 'a bulk export with fewer rows than this is truncated (preflight E-L0-SHORT); "-" = no floor']] },
  // authored sheets: the only ones not derived from evidence. Rows are added with tools/sheet_add.js.
  doctrine: { group: 'meta', file: 'doctrine.json', by: 'tools/sheet_add.js (authored)', from: 'project rules and tool versions', external: true, cols: [
    ['id', K, 'str', '-', '-'], ['key', V, 'str', '-', '-'], ['value', V, 'str', '-', '-'], ['notes', V, 'str', '-', '-', true]] },
  decisions: { group: 'meta', file: 'decisions.json', by: 'tools/sheet_add.js (authored)', from: 'decisions of the user and of the sessions', external: true, cols: [
    ['id', K, 'str', '-', '-'], ['decision', V, 'str', '-', '-'], ['rationale', V, 'str', '-', '-'], ['alternatives', V, 'str', '-', 'what was not chosen'],
    ['reversibility', V, 'enum:easy|medium|hard', '-', '-'], ['by', V, 'enum:user|session', '-', 'who decided'], ['ts', V, 'str', '-', 'date']] },
  tokens: { group: 'meta', file: 'tokens.json', by: 'tools/sheet_add.js (authored)', from: 'token counts of agent runs as reported by the coordinating session', external: true, cols: [
    ['id', K, 'str', '-', '-'], ['stage', V, 'str', '-', 'what the run worked on'], ['total_tokens', V, 'int', '-', 'tokens the run used; unchecked = not reported'],
    ['input_tokens', V, 'int', '-', 'not reported so far'], ['output_tokens', V, 'int', '-', 'not reported so far'], ['cached_tokens', V, 'int', '-', 'not reported so far'],
    ['rows', V, 'int', '-', 'sheet rows the run finished; unchecked = not measured'], ['ts', V, 'str', '-', 'date of the run']] },
  kernel: { group: 'meta', file: 'kernel.json', by: 'tools/extract.js', from: 'tools/*.js, tools/*.ps1, tools/lib/*.js, tools/ghidra/*.java', cols: [
    ['id', K, 'str', '-', 'hand-written file under tools/'], ['kind', V, 'enum:tool|lib|ghidra', '-', '-'],
    ['purpose', V, 'str', '-', 'first comment line of the file; unchecked when the file has none'],
    ['version', V, 'str', '-', 'its VERSION constant; "-" when it has none']] },

  // ---- reflect: what the game registers with the engine's reflection system
  classes: { group: 'reflect', file: 'classes.json', by: 'tools/extract.js', from: 'audit/reflection.json (class records: one per StaticClass getter)', cols: [
    ['id', K, 'str', '-', 'class name'],
    ['name', V, 'str', '-', 'L"Name" argument of GetPrivateStaticClassBody'],
    ['cpp_name', V, 'str', '-', 'C++ class name: the literal the name argument points into (TEXT("ATresGameMode") + 1) where there is one, else the prefix UnrealHeaderTool enforces (A below AActor, U otherwise; the rule agrees with all 7,005 literals)', true], ['ord', V, 'int', '-', 'position of the record in audit/reflection.json'],
    ['module', R, 'str', 'modules', '/Script/<Module> argument of the registration call'],
    ['size', V, 'int', '-', 'sizeof, argument of the registration call'], ['class_flags', V, 'hex', '-', 'EClassFlags argument'],
    ['cast_flags', V, 'hex', '-', 'EClassCastFlags argument (StaticClassCastFlags)', true], ['config', V, 'str', '-', 'config name argument (StaticConfigName)', true],
    ['super', R, 'str', 'classes', 'parent class: the class returned by the SuperClassFn argument (Super::StaticClass); "-" for Object'],
    ['within', R, 'str', 'classes', 'ClassWithin: the class returned by the WithinClassFn argument'],
    ['system', R, 'str', 'systems', 'design/systems row whose rule matches the name'],
    ['properties', V, 'int', '-', 'properties rows owned directly'], ['functions', V, 'int', '-', 'functions rows owned'],
    ['header', V, 'str', '-', 'generated layout header; "-" when the class has no reflected member'],
    ['getter', R, 'addr', 'code', 'GetPrivateStaticClass: the function that calls GetPrivateStaticClassBody', true],
    ['register_natives', R, 'addr', 'code', 'StaticRegisterNatives function; "-" for core classes built without one', true],
    ['ref_addr', R, 'addr', 'code', 'Z_Construct_UClass_<Name>: the function that adds properties and functions', true]] },
  structs: { group: 'reflect', file: 'structs.json', by: 'tools/extract.js', from: 'audit/reflection.json (struct records)', cols: [
    ['id', K, 'str', '-', 'struct name'],
    ['name', V, 'str', '-', '-'], ['ord', V, 'int', '-', 'position of the record in audit/reflection.json'],
    ['module', R, 'str', 'modules', 'package the struct is outered to (the Z_Construct_UPackage getter whose result is the Outer argument)'],
    ['size', V, 'int', '-', 'sizeof argument of FindExistingStructIfHotReloadOrDynamic; equals the size its TCppStructOps constructor stores'],
    ['align', V, 'int', '-', 'alignment its TCppStructOps constructor stores; "-" when the struct is built without CppStructOps'],
    ['super', R, 'str', 'structs', 'base struct: the SuperStruct argument of the UScriptStruct constructor; "-" when it is null'],
    ['struct_flags', V, 'hex', '-', 'EStructFlags argument', true],
    ['properties', V, 'int', '-', 'properties rows owned directly'], ['header', V, 'str', '-', 'generated layout header; "-" when no member has an offset'],
    ['outer_getter', R, 'addr', 'code', 'function that returns the outer object (the package)', true],
    ['ref_addr', R, 'addr', 'code', 'registration function (Z_Construct_UScriptStruct_...)', true]] },
  enums: { group: 'reflect', file: 'enums.json', by: 'tools/extract.js', from: 'audit/reflection.json (enum records); sheets/schemas for where the cooked data uses them', cols: [
    ['id', K, 'str', '-', 'enum name'], ['module', R, 'str', 'modules', 'package the enum is outered to; "-" for a user-defined enum'],
    ['cpp_form', V, 'enum:regular|namespaced|enumclass', '-', 'UEnum::ECppForm argument of SetEnums'], ['values', V, 'int', '-', 'names the registration function adds'],
    ['values_seen', V, 'int', '-', 'distinct values that occur in data sheets'], ['used_by', R, 'list', 'data-fields', 'data fields typed with it'],
    ['asset', R, 'str', 'asset-files', 'user-defined enum: the cooked package that defines it (names and values from its decoded export); "-" for an enum the exe registers'],
    ['ref_addr', R, 'addr', 'code', 'registration function (Z_Construct_UEnum_...); "-" for a user-defined enum', true]] },
  'enum-values': { group: 'reflect', file: 'enum-values.json', by: 'tools/extract.js', from: 'audit/reflection.json (enum records); values of enum-typed cells in sheets/entities and sheets/systems', cols: [
    ['id', K, 'str', '-', '<Enum>/<value name>'], ['enum', R, 'str', 'enums', '-'], ['name', V, 'str', '-', 'value name as registered (and as stored in cooked data)'],
    ['value', V, 'int', '-', 'numeric value paired with the name in the registration function'], ['uses', V, 'int', '-', 'data cells holding it']] },
  'property-types': { group: 'reflect', file: 'property-types.json', by: 'tools/extract.js', from: 'audit/reflection.json object classes; sizes from the table in tools/lib/layout.js (UE 4.17 x64), verified by the layout static_asserts', cols: [
    ['id', K, 'str', '-', 'UProperty class'], ['size', V, 'int', '-', 'bytes of storage of one element; "-" when it depends on the property (struct size, enum underlying type)'],
    ['c_type', V, 'str', '-', 'C type the layout headers use'], ['uses', V, 'int', '-', 'properties rows of this type']] },
  'class-defaults': { group: 'reflect', file: 'class-defaults.json', by: 'tools/extract.js', from: 'audit/class_defaults.json (tools/class_defaults.js: the class constructors run in the emulator)', cols: [
    ['id', K, 'str', '-', '<Class>.<data field>: a property that some cooked object of the class does not serialize'],
    ['class', R, 'str', 'classes', 'class of the objects; every one of them has the class default object as archetype'],
    ['field', R, 'str', 'data-fields', 'the column of the data sheets'], ['property', R, 'str', 'properties', 'the reflected property (of the class or a super class)'],
    ['type', R, 'str', 'property-types', '-'], ['omitted', V, 'int', '-', 'objects in the data sheets that do not serialize it'],
    ['value', V, 'json', '-', 'value in the class default object, written as the data sheets write a serialized value of the property; unchecked when not recovered (see reason)'],
    ['rule', V, 'enum:zero|store|delta', '-', 'zero = no instruction of the constructor chain stores to the member and the object is zero-filled before construction; store = the chain stores constants, the same on every path; delta = tagged struct, equal to the default means no member listed; "-" when not recovered'],
    ['evidence', V, 'addr', '-', 'address of the instruction that stores the (first byte of the) value; "-" for zero and delta'],
    ['ctor', R, 'addr', 'code', 'the class constructor given to GetPrivateStaticClassBody (__DefaultConstructor), where the run starts', true],
    ['reason', V, 'str', '-', 'why no value was recovered; "-" when one was']] },
  properties: { group: 'reflect', file: 'properties.json', by: 'tools/extract.js', from: 'audit/reflection.json (property objects)', cols: [
    ['id', K, 'str', '-', '<owner>.<name>'], ['owner', R, 'str', 'classes|structs|functions|properties', 'class, struct, function (parameter) or container property (inner)'],
    ['owner_kind', V, 'enum:class|struct|function|property', '-', '-'], ['ord', V, 'int', '-', 'object index within its registration record'],
    ['name', V, 'str', '-', 'string passed to FName before the property is created'], ['type', R, 'str', 'property-types', 'which allocator made the object'],
    ['offset', V, 'int', '-', 'Offset argument of the property constructor; for a bool the byte the bitfield probe finds; "-" for container inners'],
    ['flags', V, 'hex', '-', 'EPropertyFlags literal'],
    ['mask', V, 'hex', '-', 'BoolProperty: bit mask within the byte at offset (0x1 for a plain bool); "-" for other types'],
    ['array_dim', V, 'int', '-', 'UProperty::ArrayDim: the value the registration function assigns after construction, 1 when it assigns none'],
    ['target', R, 'str', 'classes|structs|enums|functions', 'struct, class, enum or delegate signature whose getter result is passed to the constructor; "-" for plain values and enum-less bytes']] },
  functions: { group: 'reflect', file: 'functions.json', by: 'tools/extract.js', from: 'audit/reflection.json (function and delegate records)', cols: [
    ['id', K, 'str', '-', '<Class>.<Name>; </Script/Module>.<Name> for a delegate signature outered to a package'],
    ['owner', R, 'str', 'classes', 'class whose Z_Construct_UClass result is the Outer of the function; "-" when the outer is a package'], ['name', V, 'str', '-', '-'],
    ['kind', V, 'enum:function|delegatefunction', '-', '-'], ['flags', V, 'hex', '-', 'EFunctionFlags literal'],
    ['params_size', V, 'int', '-', 'size of the parameter struct'], ['params', V, 'int', '-', 'properties rows owned (parameters and return value)'],
    ['native', R, 'str', 'exec', 'exec function its class registers under the same name (native function table); "-" when FUNC_Native (0x400) is not set'],
    ['outer_getter', R, 'addr', 'code', 'function that returns the outer object', true],
    ['ref_addr', R, 'addr', 'code', 'registration function (Z_Construct_UFunction_...)', true]] },

  // ---- exe: facts about the executable recovered by this project's tools
  exec: { group: 'exe', file: 'exec.json', by: 'tools/extract.js', from: 'audit/symbols.json (rows with script_name)', cols: [
    ['id', K, 'str', '-', '<Class>::exec<Name>; with @<address> when the name repeats'], ['ref_addr', R, 'addr', 'code', 'the exec function'],
    ['class', R, 'str', 'classes', 'class whose StaticRegisterNatives registers the table'], ['script_name', V, 'str', '-', 'string paired with the address in the native function table'],
    ['table', V, 'addr', '-', 'address of the native function table'], ['registered_by', R, 'addr', 'code', 'StaticRegisterNatives function that registers the table'],
    ['ref_conf', V, 'str', '-', 'confidence recorded with the name', true]] },
  names: { group: 'exe', file: 'names.json', by: 'tools/extract.js', from: 'audit/symbols.json', cols: [
    ['id', K, 'str', '-', 'symbol used in asm/ (func_<address>)'], ['ref_addr', R, 'addr', 'code', '-'], ['name', V, 'str', '-', 'recovered name'],
    ['basis', V, 'str', '-', 'kind of evidence: native function table, reflection registration, engine function, class constructor chain, string literal, jump thunk, import thunk, PE export or import data, Ghidra Function ID'], ['ref_conf', V, 'enum:confirmed|inferred', '-', '-'], ['evidence', V, 'str', '-', 'full evidence text', true]] },
  rtti: { group: 'exe', file: 'rtti.json', by: 'tools/extract.js', from: 'audit/rtti.json', cols: [
    ['id', K, 'str', '-', 'demangled class name'], ['mangled', V, 'str', '-', 'type descriptor name'], ['bases', R, 'list', 'rtti', 'base classes from the class hierarchy descriptor'],
    ['vtables', V, 'int', '-', 'vtables rows of the class']] },
  vtables: { group: 'exe', file: 'vtables.json', by: 'tools/extract.js', from: 'audit/rtti.json', cols: [
    ['id', K, 'addr', '-', 'address of the vtable'], ['class', R, 'str', 'rtti', '-'], ['this_offset', V, 'int', '-', 'offset of the subobject it belongs to'], ['slots', V, 'int', '-', 'function pointers counted']] },
  vfuncs: { group: 'exe', file: 'vfuncs.json', by: 'tools/extract.js', from: 'audit/vfuncs.json (tools/vtables.js: class vtables found by running the class constructors, and the RTTI vtables)', cols: [
    ['id', K, 'addr', '-', 'a function held by at least one vtable'], ['ref_addr', R, 'addr', 'code', 'the same address, as a function'],
    ['class', R, 'str', 'classes|rtti', 'the class that introduces it: its vtable holds the function at the slot and its super class\'s vtable holds another function there, or has no such slot; "-" when several classes do (identical code folded by the linker, shared stubs)'],
    ['owner_kind', V, 'enum:class|rtti', '-', 'class = reflected class (vtable from its constructor run); rtti = class with run-time type information; "-" when the introducing classes are of both kinds'],
    ['slot', V, 'int', '-', 'index in the vtable (8 bytes per slot); "-" when it is introduced at different slots'],
    ['how', V, 'enum:override|new|root', '-', 'override = replaces the super class\'s function at the slot; new = the slot does not exist in the super class; root = the class has no known super vtable; "-" when mixed'],
    ['introduced_by', V, 'int', '-', 'classes that introduce it'], ['vtables', V, 'int', '-', 'vtable slots that hold it, over all classes'],
    ['ref_conf', V, 'enum:confirmed|inferred', '-', 'confirmed = one introducing class; inferred = several (the owner cell is "-")']] },
  sections: { group: 'exe', file: 'sections.json', by: 'tools/extract.js', from: 'base/KH3.exe section table, audit/symbol_coverage.json, audit/data_map_summary.json', cols: [
    ['id', K, 'str', '-', 'section name'], ['start', V, 'addr', '-', '-'], ['end', V, 'addr', '-', '-'], ['bytes', V, 'int', '-', '-'],
    ['ranges', V, 'int', '-', 'coverage ranges that tile it'], ['unknown_bytes', V, 'int', '-', 'bytes in ranges of kind unknown']] },
  coverage: { group: 'exe', file: 'coverage.json', by: 'tools/extract.js', from: 'audit/symbol_coverage.json, audit/data_map_summary.json', cols: [
    ['id', K, 'str', '-', '<section>/<kind>'], ['section', R, 'str', 'sections', '-'], ['kind', V, 'str', '-', 'function, thunk, jumptable, padding, library, data, rodata, bss, unknown'],
    ['ranges', V, 'int', '-', '-'], ['bytes', V, 'int', '-', '-'],
    ['matched_ranges', V, 'int', '-', 'ranges with a C source that passed the differential test; "-" for data sections'], ['matched_bytes', V, 'int', '-', '-']] },
  'data-types': { group: 'exe', file: 'data-types.json', by: 'tools/extract.js', from: 'audit/data_map_summary.json (per_type)', cols: [
    ['id', K, 'str', '-', 'type tools/datamap.js gave a range of a data section'], ['ranges', V, 'int', '-', '-'], ['bytes', V, 'int', '-', '-']] },

  // ---- native: the code section as Ghidra sees it. Written by tools/native_sheets.js, not by extract.js.
  code: { group: 'native', file: 'code.json', by: 'tools/native_sheets.js', from: 'Ghidra function list', external: true, minRows: 360000, cols: [
    ['id', K, 'addr', '-', 'entry address of a function'], ['name', V, 'str', '-', "Ghidra's name; FUN_<addr> until named"], ['size', V, 'int', '-', 'bytes in the body'],
    ['callers', V, 'int', '-', 'references to the entry'], ['callees', V, 'int', '-', 'distinct functions it calls'], ['thunk', V, 'yesno', '-', 'yes when it only forwards to another function']] },
  gaps: { group: 'native', file: 'gaps.json', by: 'tools/native_sheets.js', from: 'Ghidra listing', external: true, cols: [
    ['id', K, 'addr', '-', 'start of a run of the code section that is not in any function'], ['length', V, 'int', '-', '-'], ['bytes', V, 'str', '-', 'first bytes, hex'],
    ['after', V, 'str', '-', 'what ends just before it'], ['kind', V, 'str', '-', 'what it turned out to be; unchecked until explained']] },
  verify: { group: 'native', file: 'verify.json', by: 'tools/native_sheets.js', from: 'differential test results', external: true, minRows: 300000, cols: [
    ['id', R, 'addr', 'code', 'function'], ['verdict', V, 'enum:same|same-partial|differs|no-verdict|not-compiled|not-tested|unconfirmed', '-', 'result of running the C source against the original'],
    ['coverage', V, 'num', '-', 'share of the original instructions the test executed']] },
  decomp: { group: 'native', file: 'decomp.json', by: 'tools/native_sheets.js', from: 're/decomp/index.tsv', external: true, minRows: 360000, cols: [
    ['id', R, 'addr', 'code', 'function'], ['how', V, 'enum:ok|relaxed|listed', '-', 'how the decompiler output was obtained; unchecked = no file yet'], ['file', V, 'str', '-', 'decompiled C']] },

  // ---- impl: work sheets that overlap evidence sheets (tools/link.js)
  progress: { group: 'impl', file: 'progress.json', by: 'tools/link.js', from: 'native/code overlapped with exe/names, exe/exec, reflect/functions, native/decomp, native/verify, exe/strings, exe/calls', external: true, cols: [
    ['id', R, 'addr', 'code', 'function'],
    ['status', V, 'enum:identified|typed|named|decompiled|verified', '-', 'furthest stage reached (sheets/README.md); understood and ported cannot be claimed: nothing records them yet'],
    ['name', V, 'str', '-', 'recovered name (exe/names); unchecked = not named'], ['name_source', V, 'str', '-', 'exe/names.basis'],
    ['signature', R, 'str', 'functions', 'reflect/functions row that gives its parameters; unchecked = types not known'],
    ['how', V, 'enum:ok|relaxed|listed', '-', 'native/decomp.how; unchecked = no decompiled file'],
    ['verdict', V, 'str', '-', 'native/verify.verdict; unchecked = never compiled and compared'],
    ['strings', V, 'int', '-', 'exe/strings rows that name it in used_by'],
    ['entry_depth', V, 'int', '-', 'calls from the PE entry point (exe/calls); "-" = not reachable by direct calls; unchecked = no call graph yet'],
    ['score', V, 'num', '-', 'triage score, higher = read first (sheets/README.md)']] },
  calls: { group: 'exe', file: 'calls.json', by: 'tools/link.js', from: 're/calls.tsv (Ghidra call references)', external: true, cols: [
    ['id', R, 'addr', 'code', 'calling function'], ['callees', R, 'list', 'code', 'functions it calls directly']] },
  strings: { group: 'exe', file: 'strings/', by: 'tools/xref_sheets.js', from: 'audit/data_map.json string ranges, base/KH3.exe, RIP-relative operands of native/code functions', external: true, cols: [
    ['id', K, 'addr', '-', 'address of the first character'], ['encoding', V, 'enum:ascii|utf16', '-', '-'], ['bytes', V, 'int', '-', 'extent in the data map, terminator and padding included'],
    ['text', V, 'str', '-', '-'], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confidence of tools/datamap.js that the range is a string'],
    ['used_by', R, 'list', 'code', 'functions with an instruction that addresses it']] },
  constants: { group: 'exe', file: 'constants.json', by: 'tools/xref_sheets.js', from: 'audit/data_map.json scalar ranges, base/KH3.exe, RIP-relative operands of native/code functions', external: true, cols: [
    ['id', K, 'addr', '-', '-'], ['type', R, 'str', 'data-types', 'width and kind inferred from the instructions that read it'], ['bytes', V, 'int', '-', '-'], ['raw', V, 'str', '-', 'the bytes, hex, in memory order'],
    ['value', V, 'num', '-', 'float32/float64 as a number, scalar8/16/32 as unsigned; "-" where the bytes alone do not say (64 bits and wider, NaN, infinities)'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', '-'], ['used_by', R, 'list', 'code', 'functions with an instruction that addresses it']] },

  // ---- assets
  'asset-classes': { group: 'assets', file: 'classes.json', by: 'tools/extract.js', from: 'audit/asset_census_summary.json (by_class), audit/package_manifest.json (classes), audit/blueprint_class_bases.json', cols: [
    ['id', K, 'str', '-', 'class of cooked exports'], ['origin', V, 'enum:script|blueprint', '-', 'script = imported from /Script/<Module>; blueprint = defined by a cooked package'],
    ['module', R, 'str', 'modules', 'script package it is imported from; "-" for blueprint classes'],
    ['native_class', R, 'str', 'classes', 'the reflected class itself, or the native base of a blueprint class (audit/blueprint_class_bases.json); the nearest common native ancestor when several packages define the name with different bases'],
    ['exports', V, 'int', '-', 'exports under assets/ (census)'], ['bytes', V, 'int', '-', 'their serialized bytes'], ['packages', V, 'int', '-', 'packages containing one'],
    ['decoder', V, 'str', '-', 'tool that reads and rewrites the class; unchecked = none'], ['layout', V, 'str', '-', 'tagged properties only, or native data after them'],
    ['decoded_exports', V, 'int', '-', 'exports with zero uninterpreted bytes (package_manifest; counts superseded copies too)'],
    ['roundtrip_exports', V, 'int', '-', 'exports in packages that rebuild byte for byte'], ['uninterpreted_bytes', V, 'int', '-', 'bytes carried as raw hex'],
    ['third_party_bytes', V, 'int', '-', 'payloads kept verbatim in a named third-party format'], ['delegated_bytes', V, 'int', '-', 'bytes decoded by another converter'],
    ['status', V, 'enum:decoded|partial|undecoded', '-', 'decoded = uninterpreted_bytes is 0; undecoded = no export decodes']] },
  'asset-areas': { group: 'assets', file: 'areas.json', by: 'tools/extract.js', from: 'audit/asset_census.json (files)', cols: [
    ['id', K, 'str', '-', 'top-level content folder'], ['sheet', V, 'str', '-', 'file holding its asset-files rows'], ['files', V, 'int', '-', '-'], ['bytes', V, 'int', '-', '-'], ['packages', V, 'int', '-', '.uasset/.umap files']] },
  'asset-files': { group: 'assets', file: 'files/', by: 'tools/extract.js', from: 'audit/asset_census.json (files)', cols: [
    ['id', K, 'str', '-', 'path under assets/'], ['area', R, 'str', 'asset-areas', '-'], ['type', V, 'str', '-', 'container type: package, uexp, ubulk, or the extension'],
    ['bytes', V, 'int', '-', '-'], ['classes', R, 'list', 'asset-classes', 'classes of the exports of a package; "-" for other files']] },
  'asset-formats': { group: 'assets', file: 'formats.json', by: 'tools/extract.js', from: 'audit/loose_files.json (formats)', cols: [
    ['id', K, 'str', '-', 'format of files that are not packages'], ['extensions', V, 'list', '-', '-'], ['files', V, 'int', '-', '-'], ['bytes', V, 'int', '-', '-'],
    ['identified_by', V, 'str', '-', '-'], ['decoded', V, 'int', '-', 'files a tool parses'], ['roundtrip', V, 'int', '-', 'files rebuilt byte for byte from the parse']] },
  paks: { group: 'assets', file: 'paks.json', by: 'tools/extract.js', from: 'audit/asset_manifest.json (archives)', cols: [
    ['id', K, 'str', '-', 'archive path under the game root'], ['entries', V, 'int', '-', '-'], ['roundtrip_ok', V, 'int', '-', 'entries recompressed to the stored bytes'], ['sha1_ok', V, 'int', '-', 'entries whose index SHA-1 verifies'],
    ['stored_bytes', V, 'int', '-', '-'], ['extracted_bytes', V, 'int', '-', '-'], ['manifest', V, 'str', '-', 'per-entry rows', true]] },
  movies: { group: 'assets', file: 'movies.json', by: 'tools/extract.js', from: 'audit/usm_manifest.json (files)', cols: [
    ['id', K, 'str', '-', '.usm path'], ['bytes', V, 'int', '-', '-'], ['chunks', V, 'int', '-', '-'], ['streams', V, 'list', '-', 'stream ids'],
    ['structural', V, 'yesno', '-', 'every byte is accounted for by the parsed structure'], ['roundtrip', V, 'yesno', '-', 'rebuilt byte for byte from chunks and elementary streams']] },

  // ---- design
  modules: { group: 'design', file: 'modules.json', by: 'tools/extract.js', from: 'audit/reflection.json packages, audit/asset_census_summary.json import packages', cols: [
    ['id', K, 'str', '-', '/Script/<Module>'], ['classes', V, 'int', '-', 'reflect/classes rows'], ['structs', V, 'int', '-', 'reflect/structs rows'], ['enums', V, 'int', '-', 'reflect/enums rows'], ['functions', V, 'int', '-', 'reflect/functions rows of its classes and package'],
    ['natives', V, 'int', '-', 'exe/exec rows of those classes'], ['asset_classes', V, 'int', '-', 'assets/classes rows imported from it'], ['asset_exports', V, 'int', '-', 'their exports'],
    ['origin', V, 'enum:game|engine|plugin|middleware', '-', 'whose code the module is; unchecked where no evidence decides it (the evidence cell says why)'],
    ['evidence', V, 'str', '-', 'where the module name occurs, and what decides the origin', true]] },
  systems: { group: 'design', file: 'systems.json', by: 'tools/extract.js', from: 'RULES of tools/docs_gen.js applied to reflect/classes; one row per other module', cols: [
    ['id', K, 'str', '-', '-'], ['ord', V, 'int', '-', 'rules apply in this order; the first match decides'], ['title', V, 'str', '-', '-'],
    ['module', R, 'str', 'modules', 'module whose classes the rule applies to'], ['match', V, 'str', '-', 'regular expression on the class name; "-" = every class of the module'],
    ['classes', V, 'int', '-', '-'], ['structs', V, 'int', '-', 'Tres-named structs matching the rule'], ['functions', V, 'int', '-', '-'], ['natives', V, 'int', '-', '-'],
    ['blueprints', V, 'int', '-', 'cooked blueprint classes whose native base is in the system'],
    ['roots', R, 'list', 'classes', 'classes of the system whose parent class is outside it: the tops of the subtrees that make up the system'],
    ['evidence', V, 'str', '-', 'how the row was derived', true]] },

  // ---- data: the game's own tables (rows live in sheets/entities and sheets/systems)
  'data-structs': { group: 'data', file: 'structs.json', by: 'tools/extract.js', from: 'sheets/schemas/*.schema.json, audit/sheets.json', cols: [
    ['id', K, 'str', '-', 'row/<RowStruct> or object/<Class>: the kind of the data sheets'], ['kind', V, 'enum:row|object', '-', '-'], ['name', V, 'str', '-', '-'],
    ['reflect', R, 'str', 'structs|classes|asset-files', 'the same struct or class in the exe; for a user-defined struct the cooked package that defines it'], ['fields', V, 'int', '-', '-'], ['tables', V, 'int', '-', 'data sheets holding rows of it'], ['rows', V, 'int', '-', '-'],
    ['schema', V, 'str', '-', 'JSON Schema used by tools/sheets_check.js'], ['header', V, 'str', '-', 'generated struct']] },
  'data-fields': { group: 'data', file: 'fields.json', by: 'tools/extract.js', from: 'sheets/schemas/*.schema.json', cols: [
    ['id', K, 'str', '-', '<struct id>.<field>'], ['struct', R, 'str', 'data-structs', '-'], ['ord', V, 'int', '-', 'serialized order'], ['name', V, 'str', '-', 'column of the data sheets'],
    ['ue_name', V, 'str', '-', 'property name stored in the cooked tag'], ['ue_type', R, 'str', 'property-types', 'property type stored in the cooked tag'],
    ['enum', R, 'str', 'enums', 'enumName of the tag'], ['struct_name', V, 'str', '-', 'structName of the tag'], ['inner', V, 'str', '-', 'innerType of an array or map tag'], ['value_type', V, 'str', '-', 'valueType of a map tag'],
    ['optional', V, 'yesno', '-', 'yes when some object leaves it out (the class default applies)'], ['c_type', V, 'str', '-', 'type in the generated struct'], ['comment', V, 'str', '-', 'comment in the generated struct']] },
  'data-tables': { group: 'data', file: 'tables.json', by: 'tools/extract.js', from: 'audit/sheets.json, sheets/entities, sheets/systems', cols: [
    ['id', K, 'str', '-', 'data sheet file under sheets/'], ['kind', V, 'enum:datatable|objects', '-', '-'], ['struct', R, 'str', 'data-structs', 'row struct, or class of the main asset'],
    ['classes', R, 'list', 'data-structs', 'kinds of rows in the file'], ['rows', V, 'int', '-', '-'], ['source', V, 'str', '-', 'cooked .uasset it compiles back to', true], ['archive', V, 'str', '-', 'pak it comes from', true]] },

  // ---- gen: which rows become which code
  targets: { group: 'gen', file: 'targets.json', by: 'tools/extract.js', from: 'tools/gen.js', cols: [
    ['id', K, 'str', '-', '-'], ['rows_from', R, 'list', 'sheets', 'sheets whose rows are the input'], ['row', V, 'str', '-', 'which rows produce a strut'],
    ['needs', V, 'list', '-', 'columns that must be checked for a row to produce its strut'], ['strut', V, 'str', '-', 'what one row becomes'], ['output', V, 'str', '-', 'file pattern'],
    ['files', V, 'int', '-', 'files it writes'], ['verified_by', V, 'str', '-', 'gate that compiles or checks the output']] },
  // ---- port: asset packs built for reimplementations outside the game (local build output, never distributed)
  'sora-assets': { group: 'port', file: 'sora-assets.json', by: 'tools/sora_pack.js', from: 'build/sora_pack (manifest.json, validation.json): the pawn blueprint, AnimBlueprint, anim set, blend spaces and material instances of p_ex001 under assets_converted/', external: true, cols: [
    ['id', K, 'str', '-', 'file of the pack, relative to build/sora_pack'], ['kind', V, 'enum:mesh|mesh+animations|texture|animation|face-animation|data|manifest|sound', '-', '-'],
    ['logical', V, 'list', '-', 'animation: rows of the TresAnimSet that play it (Name, or Name@sample for a blend-space sample); texture: its role; "-" otherwise'],
    ['source', R, 'str', 'asset-files', 'cooked package the item was converted from'], ['source_sha256', V, 'str', '-', 'sha256 of the .uexp of the package (the export data)', true],
    ['bytes', V, 'int', '-', 'size of the packed file'], ['sha256', V, 'str', '-', 'sha256 of the packed file', true],
    ['duration', V, 'num', '-', 'animation: SequenceLength in seconds'], ['frames', V, 'int', '-', 'animation: NumFrames'],
    ['loop', V, 'yesno', '-', 'animation: whether the player states play it looping (docs/sora-animation-spec.md)'],
    ['root_motion', V, 'yesno', '-', 'animation: whether the track of the root bone kh_sk moves or turns'],
    ['validation', V, 'str', '-', 'result of the checks of tools/sora_pack.js for this file'], ['evidence', V, 'str', '-', 'the game data that names the source asset']] },
  // (party stream C) the node classes of the friends' AI pack and how kh-ai implements them (tools/party_ai_sheet.js; docs/party-contracts.md C)
  'party-ai-nodes': { group: 'port', file: 'party-ai-nodes.json', by: 'tools/party_ai_sheet.js', from: 'build/sora_pack/party/ai/index.json (node-class census of the trees / EQS queries reachable from n_ex001_BT / n_ex002_BT), sheets/exe/vfuncs (own behaviour overrides), the decompiled C read for crates/kh-ai/src/nodes', external: true, cols: [
    ['id', K, 'str', '-', 'node class, or param.<name> for a kh-ai parameter not read from the game'], ['kind', V, 'enum:task|decorator|service|composite|eqs-generator|eqs-test|other|param', '-', '-'],
    ['instances', V, 'int', '-', 'instances in the packed trees / queries'], ['functions', R, 'list', 'code', 'the class\'s own behaviour functions (vfuncs slots 72..74, 86..91, 96..101)'],
    ['evidence', V, 'str', '-', 'what kh-ai implements and from what', true], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'inferred = decompiled C read or stock UE 4.17; guess = from the class name / fields'],
    ['implemented', V, 'yesno', '-', 'kh-ai has an implementation (tests/real_trees.rs reachability: zero unknown)'], ['note', V, 'str', '-', '-']] },
  // (party stream A) Donald's and Goofy's packs and their 14 + 14 weapons (tools/party_pack.js; docs/party-contracts.md A.1)
  'party-assets': { group: 'port', file: 'party-assets.json', by: 'tools/party_pack.js', from: 'build/sora_pack/party (party.json, n_ex00x/manifest.json, validation.json): the pawn blueprints, AnimBlueprints, anim sets, KBD user data (KineDriver / Bonamik), weapon blueprints and material instances of n_ex001 / n_ex002 under assets_converted/', external: true, cols: [
    ['id', K, 'str', '-', 'file of the pack, relative to build/sora_pack/party'], ['kind', V, 'enum:mesh|texture|animation|face-animation|data|manifest', '-', '-'],
    ['logical', V, 'list', '-', 'animation: anim-set rows that play it (Name, Name@sample, face:Name, CM_Body:Name ...); weapon mesh: its equip item; "-" otherwise'],
    ['source', R, 'str', 'asset-files', 'cooked package the item was converted from'], ['source_sha256', V, 'str', '-', 'sha256 of the .uexp of the package (the export data)', true],
    ['bytes', V, 'int', '-', 'size of the packed file'], ['sha256', V, 'str', '-', 'sha256 of the packed file', true],
    ['duration', V, 'num', '-', 'animation: SequenceLength in seconds'], ['frames', V, 'int', '-', 'animation: NumFrames'],
    ['loop', V, 'yesno', '-', 'animation: "no" (an NPC state gives the play mode)'],
    ['root_motion', V, 'yesno', '-', 'animation: whether the track of the root bone kh_sk moves or turns'],
    ['validation', V, 'str', '-', 'result of the checks of tools/party_pack.js for this file'], ['evidence', V, 'str', '-', 'the game data that names the source asset']] },
  // authored rows kept in tools/sora_movement_sheet.js: the fixed parameters of Sora's on-foot movement and jump (docs/sora-movement-spec.md, section 8)
  'sora-movement': { group: 'port', file: 'sora-movement.json', by: 'tools/sora_movement_sheet.js', from: 'constructors and state code of base/KH3.exe read in disassembly, class default objects of Blueprints/Player/p_ex001, converted animations, ini files', external: true, cols: [
    ['id', K, 'str', '-', '<group>.<parameter>'], ['value', V, 'json', '-', 'number or boolean in KH3 units; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'cm, s, deg and combinations; "-" for a ratio or flag'], ['source', V, 'str', '-', 'where the value is stored: instruction address of the constructor or code, or asset path and property'],
    ['evidence', V, 'str', '-', 'how the value is used and how it was verified', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = value and use read at instruction level, or in a class default object / ini; inferred = derived from confirmed values, animation data, or stock engine behaviour identified by shape; guess = not determined (value unchecked)'],
    ['note', V, 'str', '-', '-']] },
  // authored rows kept in tools/sora_movement_sheet.js: the fixed parameters of Sora's field camera (docs/sora-camera-spec.md, section 9)
  'sora-camera': { group: 'port', file: 'sora-camera.json', by: 'authored (docs/sora-camera-spec.md)', from: 'constructors and camera code of base/KH3.exe read in disassembly, class default object of Blueprints/Camera/Setting/p_ex001/BP_TresCameraNormal_ex001, skeleton and ini files', external: true, cols: [
    ['id', K, 'str', '-', '<group>.<parameter>'], ['value', V, 'json', '-', 'number, boolean or name in KH3 units; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'cm, s, deg, 1/s and combinations; "-" for a ratio, flag or name'], ['source', V, 'str', '-', 'where the value is stored: instruction address of the constructor or code, or asset path and property'],
    ['evidence', V, 'str', '-', 'how the value is used and how it was verified', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = value and use read at instruction level or in a class default object / ini; inferred = decompiled C, stock engine knowledge or derived; guess = not determined'],
    ['note', V, 'str', '-', '-']] },
  // authored rows of Sora's magic, shotlock / focus / airstep and item systems (docs/sora-magic-spec.md); written by build/sora_tmp/magic/merge.js from the research strands
  ...Object.fromEntries([
    ['sora-magic-spells', 'the cast path: request gates, TresPlayerState_FireMagic sub-steps, animations and notify times, magic set entries, combo rules'],
    ['sora-magic-projectiles', 'the spell actors: per spell and tier the blueprint class defaults, components, motion, hit shapes and attack rows'],
    ['sora-magic-mp', 'MP storage and cost, MP charge, the magic damage and cure formulas, abilities and status effects'],
    ['sora-magic-shotlocks', 'focus gauge, shotlock aim mode, the Kingdom Key shotlocks and their projectiles, airstep'],
    ['sora-magic-items', 'item use: state, timing, pouch, effect amounts'],
  ].map(([kind, what]) => [kind, { group: 'port', file: kind + '.json', by: 'authored (docs/sora-magic-spec.md)', from: 'code of base/KH3.exe (decompiled C, constants read in disassembly), cooked packages and data tables of the game: ' + what, external: true, cols: [
    ['id', K, 'str', '-', '<group>.<parameter>'], ['value', V, 'json', '-', 'number, boolean, name or list in KH3 units; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'cm, s, deg and combinations, int, count, index; "-" for a ratio, flag or name'], ['source', V, 'str', '-', 'where the value is stored: instruction address or asset path and property'],
    ['evidence', V, 'str', '-', 'how the value is used and how it was verified', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level or in a class default object / data table; inferred = decompiled C, derived or stock engine knowledge; guess = not determined (value unchecked)'],
    ['note', V, 'str', '-', '-']] }])),
  // authored rows kept in tools/sora_actions_sheet.js: Sora's action / traversal states (docs/sora-actions-spec.md)
  'sora-states': { group: 'port', file: 'sora-states.json', by: 'tools/sora_actions_sheet.js', from: 'vtables and constructors of the TresPlayerState_* classes in base/KH3.exe, the state registration in the TresPlayerPawnBase / TresPlayerPawnSora constructors, reflect/enum-values ETresStateID', external: true, cols: [
    ['id', K, 'str', '-', 'state class'], ['state_id', V, 'int', '-', 'ETresStateID written to MyStateID (+0x58) by the constructor chain; unchecked = not determined'],
    ['state_enum', V, 'str', '-', 'ETresStateID name without TSID_STATE_'], ['super', V, 'str', '-', 'parent class'], ['size', V, 'int', '-', 'object size'],
    ['vtable', V, 'str', '-', 'vtable address; unchecked = not verified from the constructor'],
    ['ref_reset', R, 'addr', 'code', 'vtable slot 95'], ['ref_begin', R, 'addr', 'code', 'vtable slot 96'], ['ref_tick', R, 'addr', 'code', 'vtable slot 97'], ['ref_end', R, 'addr', 'code', 'vtable slot 98'],
    ['substeps', V, 'int', '-', 'number of m_nSubStep values the state uses; unchecked = not counted'],
    ['registered_by', V, 'str', '-', 'constructor that registers the class with the state queue of the player pawn Sora, or "not registered for Sora"'],
    ['scope', V, 'enum:implemented-in-stage-1|specified|specified-partial|stub', '-', 'specified = begin / tick / end in docs/sora-actions-spec.md; specified-partial = architecture, entry conditions, animations and key numbers only; stub = entry points or name only'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confidence of the row (id, addresses): confirmed = constructor and vtable read; inferred = from the vtable audit and the constructor scan'],
    ['note', V, 'str', '-', '-']] },
  'sora-transitions': { group: 'port', file: 'sora-transitions.json', by: 'tools/sora_actions_sheet.js', from: 'the controller input update 0x1413F1AB0, the request interface of the player pawn (vtable 0x14685EB18) and the state ticks of base/KH3.exe', external: true, cols: [
    ['id', K, 'str', '-', '<from>><to>[.<variant>]'], ['from', V, 'str', '-', 'state id name(s) the transition starts in; * = any'], ['to', V, 'str', '-', 'state id name it leads to'],
    ['request', V, 'str', '-', 'who asks: controller call (button), a state tick, or another system'], ['gate', V, 'str', '-', 'predicate that must hold; "-" = none'],
    ['how', V, 'str', '-', 'queue operation and what is written to the new state'], ['ref_addr', R, 'addr', 'code', 'function that performs or decides the transition; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level; inferred = decompiled C'], ['note', V, 'str', '-', '-']] },
  'sora-actions': { group: 'port', file: 'sora-actions.json', by: 'tools/sora_actions_sheet.js', from: 'constructors and state code of base/KH3.exe, class default objects of Blueprints/Player/p_ex001, converted animations (root tracks and notifies)', external: true, cols: [
    ['id', K, 'str', '-', '<state>.<parameter>'], ['value', V, 'json', '-', 'number, boolean, name or list in KH3 units; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'cm, s, deg and combinations; "-" for a ratio, flag, id or name'], ['source', V, 'str', '-', 'where the value is stored: instruction address of the constructor or code, or asset path and property'],
    ['evidence', V, 'str', '-', 'how the value is used and how it was verified', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = value and use read at instruction level, or in a class default object / animation asset; inferred = decompiled C or derived; guess = not determined (value unchecked)'],
    ['note', V, 'str', '-', '-']] },
  // authored rows kept in tools/sora_enemy_reaction_sheet.js: what a generic enemy does when a hit lands (docs/sora-enemy-reaction-spec.md; crate kh-react)
  'sora-enemy-reaction': { group: 'port', file: 'sora-enemy-reaction.json', by: 'tools/sora_enemy_reaction_sheet.js', from: 'class default object of Blueprints/Enemy/EnemyCommonParam over the constructors of TresDamageCommonParam / TresEnemyCommonParam, the damage-state handlers of UTresAICharState_Damage / UTresEnemyState_Damage in base/KH3.exe, and the lengths and root tracks of the enemies\' own reaction clips', external: true, cols: [
    ['id', K, 'str', '-', '<group>.<parameter>'], ['value', V, 'json', '-', 'number in KH3 units; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'cm, s, cm/s, cm/s2, deg, 1/s; "-" for a ratio'], ['source', V, 'str', '-', 'where the value is stored: instruction address of the constructor or code, or asset path and property'],
    ['evidence', V, 'str', '-', 'how the value is used and how it was verified', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = value and use read at instruction level or in a class default object; inferred = decompiled C, a median over the enemies\' assets, or stock engine knowledge; guess = not determined (value unchecked)'],
    ['note', V, 'str', '-', '-']] },
  // Sora's melee combat (docs/sora-combat-spec.md); all eight are written by tools/sora_combat_sheet.js: code facts are authored
  // rows there, data rows are read from the game's tables, blueprints and animation clips on every run
  'sora-combat-params': { group: 'port', file: 'sora-combat-params.json', by: 'tools/sora_combat_sheet.js', from: 'attack state, pawn, controller and damage code of base/KH3.exe read in disassembly; class default objects of Blueprints/Player/p_ex001', external: true, cols: [
    ['id', K, 'str', '-', '<group>.<parameter>'], ['value', V, 'json', '-', 'number, boolean or name in KH3 units; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'cm, s, cm/s, 1/s, count; "-" for a ratio, flag or name'], ['source', V, 'str', '-', 'where the value is stored: instruction address of the constructor or code, or asset path and property'],
    ['evidence', V, 'str', '-', 'how the value is used and how it was verified', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = value and use read at instruction level or in a class default object; inferred = decompiled C, or the value is confirmed and its use was not read; guess = not determined'],
    ['note', V, 'str', '-', '-']] },
  'sora-combat-attacks': { group: 'port', file: 'sora-combat-attacks.json', by: 'tools/sora_combat_sheet.js', from: 'TresPlayerState_Attack 0x1413378B0 (attack kind -> animation and per-kind settings) and the clips of p_ex001_AnimSet (length, root track, notifies)', external: true, cols: [
    ['id', K, 'str', '-', 'kind_<hex attack kind>'], ['kind', V, 'int', '-', 'attack kind: the value of state+0x1B0 / pawn+0xE10'], ['label', V, 'str', '-', 'what the decision code uses it for'],
    ['anim', V, 'str', '-', 'TresAnimSet name played (mode 2: one-shot with root motion)'], ['clip', V, 'str', '-', 'sequence under Character/pc/p_ex001/anm/rt/body'],
    ['phase', V, 'enum:ground|air|either', '-', 'where the decision code selects it'], ['role', V, 'enum:combo|approach|crowd|finisher|rise|down|launch|counter|special', '-', '-'],
    ['requires', V, 'str', '-', 'ability (ETresAbilityKind name and value) the code tests; "-" when none'],
    ['root_len', V, 'num', '-', 'state+0x1D8: divisor of the root-motion scale (cm); "-" = the kind does not use root scaling'], ['stop_dist', V, 'num', '-', 'state+0x24C: homing stops inside this distance (cm)'],
    ['z_gain', V, 'num', '-', 'state+0x14C: vertical homing gain (1/s)'], ['air_mode', V, 'yesno', '-', 'state+0x1DF: gravity off and Flying mode while the vertical channel runs'],
    ['keep_face', V, 'yesno', '-', 'state+0x1F4 bit 4: faces the target for the whole attack instead of 0.2 s'],
    ['length', V, 'num', '-', 'SequenceLength (s)'], ['frames', V, 'int', '-', 'NumFrames'], ['root_motion', V, 'yesno', '-', 'bEnableRootMotion'],
    ['root_fwd', V, 'num', '-', 'root track: forward travel at the last frame (cm, actor space)'], ['root_up', V, 'num', '-', 'root track: vertical travel at the last frame (cm)'],
    ['enable_input', V, 'num', '-', 'time of the first TresEnableInput notify (s): the next request is accepted from here (and only after the first hit window opened)'],
    ['enable_change', V, 'num', '-', 'time of the first TresEnableChange notify (s): a queued attack starts here'],
    ['traction', V, 'list', '-', 'TresTraction windows "start-end" (s)'], ['face_at', V, 'list', '-', 'TresFaceAt times (s)'], ['attack_rows', V, 'list', '-', 'attack-data rows named by the clip\'s hit windows (sora-combat-attack-data ids)'],
    ['ref_addr', R, 'addr', 'code', 'function that holds the kind\'s settings'], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = settings read at instruction level and clip data read from the asset'], ['note', V, 'str', '-', '-']] },
  'sora-combat-hits': { group: 'port', file: 'sora-combat-hits.json', by: 'tools/sora_combat_sheet.js', from: 'TresAnimNotifyState_Attack instances of the melee clips (assets_converted/animations)', external: true, cols: [
    ['id', K, 'str', '-', '<anim>#<n>'], ['anim', V, 'str', '-', 'TresAnimSet name'], ['clip', V, 'str', '-', '-'], ['start', V, 'num', '-', 'window start (s)'], ['end', V, 'num', '-', 'window end (s)'],
    ['start_f60', V, 'num', '-', 'start in 60 Hz frames'], ['end_f60', V, 'num', '-', 'end in 60 Hz frames'], ['group', V, 'str', '-', 'CollisionGrpName: shape group (sora-combat-collisions.group)'],
    ['attack_row', V, 'str', '-', 'm_AttackDataIDName: row of p_ex001_AttackData'], ['hit_se', V, 'str', '-', 'm_WeaponHitSEAliasID (ETresSoundAliasLabel_WeaponHit)'],
    ['interval', V, 'num', '-', 'AttackInterval (s): re-hit interval of the window; "-" = one hit per target'], ['extra', V, 'str', '-', 'other serialized properties of the notify'], ['source', V, 'str', '-', 'notify and asset', true]] },
  'sora-combat-collisions': { group: 'port', file: 'sora-combat-collisions.json', by: 'tools/sora_combat_sheet.js', from: 'TresAtkCollComponent.CollisionShapesSrc of Blueprints/Player/p_ex001/w_so_KeybladeBase and p_ex001_Pawn', external: true, cols: [
    ['id', K, 'str', '-', '<owner>/<group>[#n]'], ['owner', V, 'str', '-', 'blueprint that owns the component'], ['group', V, 'str', '-', 'm_GrpName'], ['shape', V, 'str', '-', 'ETresCollision'],
    ['size_x', V, 'num', '-', 'm_Size.X (radius for spheres and capsules)'], ['size_y', V, 'num', '-', '-'], ['size_z', V, 'num', '-', 'm_Size.Z (CAPSULE1: length)'], ['socket1', V, 'str', '-', 'm_SocketName1'], ['socket2', V, 'str', '-', 'm_SocketName2 (CAPSULE2: second end)'],
    ['loc1', V, 'json', '-', 'm_RelativeLocation1'], ['loc2', V, 'json', '-', 'm_RelativeLocation2'], ['rot', V, 'json', '-', 'm_RelativeRocation (pitch, yaw, roll)'], ['attach1', V, 'str', '-', 'ETresAtkCollLocationAttachType: NORMAL = the component owner\'s mesh (weapon), OWNER_ATTACHED_MESH = the wielder\'s mesh'],
    ['attach2', V, 'str', '-', '-'], ['map_hit', V, 'str', '-', 'ETresAtkCollMapHitType'], ['sweep', V, 'yesno', '-', 'not m_bDisableSweep'], ['profile', V, 'str', '-', 'collision profile'], ['default_row', V, 'str', '-', 'm_DefaultAttackDataIDName'],
    ['phys', V, 'yesno', '-', 'm_bIsPhysAttackCollision'], ['source', V, 'str', '-', '-', true]] },
  // (party stream D6b) The team finishers' shot actors (Unison Fire / Blizzard / Thunder, Fusion Spin): one row per attack shape. Written by tools/sora_combat_shots_sheet.js, generated into build/gen/rust/sora_combat_shots.rs (crates/kh-core combat/hit.rs)
  'sora-combat-shots': { group: 'port', file: 'sora-combat-shots.json', by: 'tools/sora_combat_shots_sheet.js', from: 'Blueprints/Player/p_ex001/p_ex001_ProjectileSet and the shot blueprints with their parents (Blueprints/Projectile/Unison, MagicFire / MagicBlizzard / MagicThunder bases); spawn rule from attack-state slots 137 0x14134CCE0 / 138 0x14134C710', external: true, cols: [
    ['id', K, 'str', '-', '<ProjectileName>#<shape index>'], ['shot', V, 'str', '-', 'ProjectileName of p_ex001_ProjectileSet'], ['class', V, 'str', '-', 'blueprint class'], ['group', V, 'str', '-', 'm_GrpName'],
    ['row', V, 'str', '-', 'm_DefaultAttackDataIDName (sora-combat-attack-data)'], ['shape', V, 'str', '-', 'ETresCollision'], ['mesh', V, 'str', '-', 'm_Mesh of a CONVEX shape'], ['life', V, 'num', '-', 'm_MainLife (s)'], ['no_destroy', V, 'yesno', '-', 'm_bNoDestroyOnHit'],
    ['size', V, 'json', '-', 'm_Size'], ['inc_size', V, 'json', '-', 'm_IncSize (/s)'], ['max_size', V, 'json', '-', 'm_IncMaxSize'], ['size_curve', V, 'json', '-', 'm_SizeVectorCurve X keys (time, value, ...; linear); [] = none'], ['loc', V, 'json', '-', 'm_RelativeLocation1'],
    ['scale', V, 'json', '-', 'm_Scale (only CONVEX keeps it: 0x140647950)'], ['inc_scale', V, 'json', '-', 'm_IncScale (/s)'], ['max_scale', V, 'json', '-', 'm_IncMaxScale'], ['interval', V, 'num', '-', 'm_CollAutoActivate m_AttackInterval of the group (0: each actor once)'], ['map_hit', V, 'str', '-', 'ETresAtkCollMapHitType'],
    ['particle', V, 'str', '-', 'ParticleComp Template ("" none)'], ['particle_loc', V, 'json', '-', 'ParticleComp RelativeLocation'], ['hit_effect', V, 'str', '-', 'm_HitEffects entry of the shape group m_EffectGrpName: m_PawnHitEffect'], ['hit_se', V, 'str', '-', 'that entry: m_PawnHitSEAsset'],
    ['hit_offset', V, 'num', '-', 'm_fHitEffectRandomOffsetRadius (cm)'], ['fire_effect', V, 'str', '-', 'MyFireEffects[0].MyParticleSystem'], ['fire_se', V, 'str', '-', 'MyFireEffects[0].MySoundCue'],
    ['spawn', V, 'enum:ROOT_UP_100|ROOT_ATTACHED_GOOFY', '-', 'where the attack state puts the actor'], ['ref_addr', R, 'addr', 'code', 'the attack-state slot that spawns it'], ['source', V, 'str', '-', '-', true]] },
  'sora-combat-attack-data': { group: 'port', file: 'sora-combat-attack-data.json', by: 'tools/sora_combat_sheet.js', from: 'sheets/entities/DataTable/Player/Attack/p_ex001_AttackData.json (rows default, kb_*, revenge_*, rdc_*, blow_counter*, atk_barriercracker, divefall_attack*, lm_*)', external: true, cols: [
    ['id', K, 'str', '-', 'row name as the notifies give it'], ['power', V, 'num', '-', 'm_Power: multiplier of (strength - defense)'], ['damage_kind', V, 'str', '-', 'ETresDamageKind: the reaction asked of the victim'], ['attribute', V, 'str', '-', 'ETresDamageAttribute'],
    ['power_scale', V, 'num', '-', 'm_DamagePowerScale: knockback scale'], ['parabola_angle', V, 'num', '-', '-'], ['move_length', V, 'num', '-', '-'], ['brake', V, 'num', '-', '-'], ['knockback', V, 'str', '-', 'ETresAtkHitKnockbackType: where the knockback direction comes from'],
    ['exec_rate', V, 'num', '-', 'm_DamageExecRate (percent)'], ['effect_time', V, 'num', '-', '-'], ['bad_status', V, 'str', '-', '-'], ['bad_gen_rate', V, 'num', '-', '-'], ['bad_time', V, 'num', '-', '-'], ['bad_param', V, 'num', '-', '-'], ['team_check', V, 'str', '-', '-'],
    ['reaction_power', V, 'int', '-', 'm_ReactionPowerValue'], ['scrum', V, 'yesno', '-', '-'], ['armor_attack', V, 'int', '-', 'm_ArmorAttackPower'], ['form_point', V, 'int', '-', 'm_FormPoint: formchange gauge gain'], ['magic', V, 'yesno', '-', 'uses magic power instead of strength'],
    ['invalid_guard', V, 'yesno', '-', 'm_bIsIvalidGuard'], ['rapid_fire', V, 'yesno', '-', '-'], ['killer', V, 'yesno', '-', '-'], ['attraction', V, 'yesno', '-', '-'], ['counter', V, 'yesno', '-', '-'], ['weak_guard', V, 'yesno', '-', '-'],
    ['reflect_level', V, 'int', '-', '-'], ['reflect_reaction', V, 'str', '-', '-'], ['revenge_point', V, 'num', '-', '-'], ['phys_force', V, 'str', '-', '-'], ['source', V, 'str', '-', '-', true]] },
  'sora-combat-rules': { group: 'port', file: 'sora-combat-rules.json', by: 'tools/sora_combat_sheet.js', from: 'TresPlayerState_Attack decision code 0x14134C420 / 0x14134ACA0 / 0x1413378B0 read in disassembly', external: true, cols: [
    ['id', K, 'str', '-', 'rule_<nn>'], ['order', V, 'int', '-', 'order in which the code tests it'], ['phase', V, 'enum:pre|special|next|finisher|post|forced', '-', '-'], ['condition', V, 'str', '-', '-'], ['result', V, 'str', '-', '-'],
    ['kind', V, 'str', '-', 'attack kind when the rule fixes one; "-" otherwise'], ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', '-']] },
  'sora-combat-reactions': { group: 'port', file: 'sora-combat-reactions.json', by: 'tools/sora_combat_sheet.js', from: 'TakeDamage 0x1405BB850, reaction request 0x141439A50, TresPlayerState_Damage and the pawn\'s damage animation tables', external: true, cols: [
    ['id', K, 'str', '-', '-'], ['damage_kind', V, 'int', '-', 'ETresDamageKind value; "-" when the row is not tied to one'], ['name', V, 'str', '-', '-'], ['situation', V, 'str', '-', '-'], ['anim', V, 'str', '-', 'TresAnimSet name(s)'],
    ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', '-'], ['note', V, 'str', '-', '-']] },
  'sora-combat-notifies': { group: 'port', file: 'sora-combat-notifies.json', by: 'tools/sora_combat_sheet.js', from: 'notify instances of the melee clips; native Notify handlers of base/KH3.exe', external: true, cols: [
    ['id', K, 'str', '-', 'notify class'], ['instances', V, 'int', '-', 'instances in the melee clips'], ['iface_slot', V, 'str', '-', 'UTresAnimInterface offset the notify calls on the pawn (forwarded to the current state); "-" = not read'],
    ['ref_addr', R, 'addr', 'code', 'native Notify / NotifyBegin of the class; "-" = not read'], ['effect', V, 'str', '-', 'what the game does with it; "-" = not read']] },
  // pass 2 of the combat research (docs/sora-combat-spec.md revision 2): tables of the six strands, authored rows in
  // tools/lib/sora_combat_pass2.json, written by tools/sora_combat_sheet.js; id = a natural key or the row number in table order
  'sora-combat-can-attack': { group: 'port', file: 'sora-combat-can-attack.json', by: 'tools/sora_combat_sheet.js', from: 'CanAttack 0x141426330 (request interface slot 44): the condition per current state id (docs/sora-combat-spec.md 3.2; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['state_ids', V, 'str', '-', '-'], ['condition', V, 'str', '-', '-'], ['addr', V, 'str', '-', '-'], ['conf', V, 'str', '-', '-']] },
  'sora-combat-try-attack': { group: 'port', file: 'sora-combat-try-attack.json', by: 'tools/sora_combat_sheet.js', from: 'TryAttack 0x141437330 (slot 45): queue operation and writes per current state id (docs/sora-combat-spec.md 3.3; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['state_ids', V, 'str', '-', '-'], ['queueing', V, 'str', '-', '-'], ['writes', V, 'str', '-', '-'], ['addr', V, 'str', '-', '-'], ['conf', V, 'str', '-', '-']] },
  'sora-combat-attack-exits': { group: 'port', file: 'sora-combat-attack-exits.json', by: 'tools/sora_combat_sheet.js', from: 'what can end a TresPlayerState_Attack, with the gate that decides it (docs/sora-combat-spec.md 5.3; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['exit', V, 'str', '-', '-'], ['gate', V, 'str', '-', '-'], ['condition', V, 'str', '-', '-'], ['mechanism', V, 'str', '-', '-'], ['conf', V, 'str', '-', '-']] },
  'sora-combat-attack-end': { group: 'port', file: 'sora-combat-attack-end.json', by: 'tools/sora_combat_sheet.js', from: 'TresPlayerState_Attack::End 0x141335120 as ordered steps (docs/sora-combat-spec.md 5.4; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['order', V, 'int', '-', '-'], ['condition', V, 'str', '-', '-'], ['action', V, 'str', '-', '-'], ['addr', V, 'str', '-', '-'], ['conf', V, 'str', '-', '-']] },
  'sora-combat-controller-order': { group: 'port', file: 'sora-combat-controller-order.json', by: 'tools/sora_combat_sheet.js', from: 'order of the combat calls in the controller input update 0x1413F1AB0 (docs/sora-combat-spec.md 3.1.2; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['order', V, 'int', '-', '-'], ['call', V, 'str', '-', '-'], ['slot_or_addr', V, 'str', '-', '-'], ['conf', V, 'str', '-', '-']] },
  'sora-combat-tick-delta': { group: 'port', file: 'sora-combat-tick-delta.json', by: 'tools/sora_combat_sheet.js', from: 'displacement of one movement tick during an attack, per movement mode (docs/sora-combat-spec.md 6.1.3; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['mode', V, 'str', '-', '-'], ['delta', V, 'str', '-', '-'], ['addr', R, 'addr', 'code', '-'], ['conf', V, 'str', '-', '-']] },
  'sora-combat-velocity-flags': { group: 'port', file: 'sora-combat-velocity-flags.json', by: 'tools/sora_combat_sheet.js', from: 'bits of the movement component flag byte +0xC18 (direct-velocity record and body-push switches) (docs/sora-combat-spec.md 6.1.2; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['bit', V, 'int', '-', '-'], ['meaning', V, 'str', '-', '-'], ['writers', V, 'str', '-', '-'], ['readers', V, 'str', '-', '-']] },
  'sora-combat-air-timeline': { group: 'port', file: 'sora-combat-air-timeline.json', by: 'tools/sora_combat_sheet.js', from: 'movement-mode timeline of an air-mode attack (docs/sora-combat-spec.md 6.6; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['order', V, 'int', '-', '-'], ['event', V, 'str', '-', '-'], ['effect', V, 'str', '-', '-'], ['addr', V, 'str', '-', '-'], ['conf', V, 'str', '-', '-']] },
  'sora-combat-push-weight': { group: 'port', file: 'sora-combat-push-weight.json', by: 'tools/sora_combat_sheet.js', from: 'body-push weight by push level (UTresBodyCollComponent 0x140637430) (docs/sora-combat-spec.md 6.5; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['case', V, 'str', '-', '-'], ['weight', V, 'num', '-', '-'], ['conf', V, 'str', '-', '-']] },
  'sora-combat-prim-flags': { group: 'port', file: 'sora-combat-prim-flags.json', by: 'tools/sora_combat_sheet.js', from: 'flag bytes +0x984..+0x987 of UTresAtkCollPrimitive (docs/sora-combat-spec.md 8.0; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['byte', V, 'str', '-', '-'], ['bit', V, 'int', '-', '-'], ['meaning', V, 'str', '-', '-'], ['set_by', V, 'str', '-', '-'], ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'str', '-', '-']] },
  'sora-combat-activation': { group: 'port', file: 'sora-combat-activation.json', by: 'tools/sora_combat_sheet.js', from: 'the 0x70-byte activation struct passed from the attack notify to Activate 0x140625500 (docs/sora-combat-spec.md 8.2.1; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['offset', V, 'str', '-', '-'], ['type', V, 'str', '-', '-'], ['content', V, 'str', '-', '-'], ['source', V, 'str', '-', '-'], ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'str', '-', '-']] },
  'sora-combat-reaction-rules': { group: 'port', file: 'sora-combat-reaction-rules.json', by: 'tools/sora_combat_sheet.js', from: 'Classify 0x14061FE20: victim result and attacker result by the body reaction type of the hit shape (docs/sora-combat-spec.md 8.2.6; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['body_type', V, 'str', '-', '-'], ['condition', V, 'str', '-', '-'], ['victim_result', V, 'str', '-', '-'], ['attacker_result', V, 'str', '-', '-'], ['no_react', V, 'yesno', '-', '-'], ['recorded', V, 'yesno', '-', '-'], ['clear_bad_status', V, 'yesno', '-', '-'], ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'str', '-', '-']] },
  'sora-combat-knockback-dir': { group: 'port', file: 'sora-combat-knockback-dir.json', by: 'tools/sora_combat_sheet.js', from: 'knockback direction by m_KnockbackType (0x14061EA30) (docs/sora-combat-spec.md 8.7; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['type', V, 'int', '-', '-'], ['name', V, 'str', '-', '-'], ['direction', V, 'str', '-', '-'], ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'str', '-', '-']] },
  'sora-combat-damage-info': { group: 'port', file: 'sora-combat-damage-info.json', by: 'tools/sora_combat_sheet.js', from: 'TresDamageInfo as delivered inside FTresPointDamageEvent (+0xA8) (docs/sora-combat-spec.md 8.7; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['offset', V, 'str', '-', '-'], ['field', V, 'str', '-', '-'], ['source', V, 'str', '-', '-'], ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'str', '-', '-']] },
  'sora-combat-shape-flags': { group: 'port', file: 'sora-combat-shape-flags.json', by: 'tools/sora_combat_sheet.js', from: 'the keyblade shape groups with the flags their shape data produces (0x140642CA0) (docs/sora-combat-spec.md 8.1; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s group'], ['group', V, 'str', '-', '-'], ['shape', V, 'str', '-', '-'], ['hits_world_static', V, 'yesno', '-', '-'], ['ignore_ground', V, 'yesno', '-', '-'], ['phys', V, 'yesno', '-', '-'], ['fixed_location', V, 'yesno', '-', '-'], ['size_rule', V, 'str', '-', '-'], ['used_by_base_combo', V, 'yesno', '-', '-']] },
  'sora-combat-hit-effects': { group: 'port', file: 'sora-combat-hit-effects.json', by: 'tools/sora_combat_sheet.js', from: 'hit effect and hit sound of the Kingdom Key per case (chooser 0x140628960 on w_so010_Wep m_HitEffects) (docs/sora-combat-spec.md 8.6; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['case', V, 'str', '-', '-'], ['effect', V, 'str', '-', '-'], ['sound', V, 'str', '-', '-'], ['ref_conf', V, 'str', '-', '-']] },
  'sora-combat-attacker-events': { group: 'port', file: 'sora-combat-attacker-events.json', by: 'tools/sora_combat_sheet.js', from: 'everything the attacker side does per hit, in order (docs/sora-combat-spec.md 8.5; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['when', V, 'str', '-', '-'], ['action', V, 'str', '-', '-'], ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'str', '-', '-']] },
  'sora-combat-lockon-fields': { group: 'port', file: 'sora-combat-lockon-fields.json', by: 'tools/sora_combat_sheet.js', from: 'lock-on state on TresPlayerControllerSora (docs/sora-combat-spec.md 7.1; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s offset'], ['offset', V, 'str', '-', '-'], ['type', V, 'str', '-', '-'], ['init', V, 'str', '-', '-'], ['accessor', V, 'str', '-', '-'], ['meaning', V, 'str', '-', '-'], ['evidence', V, 'str', '-', '-']] },
  'sora-combat-lockon-slots': { group: 'port', file: 'sora-combat-lockon-slots.json', by: 'tools/sora_combat_sheet.js', from: 'virtual functions of TresLockonTargetComponent (vtable 0x146628970) the targeting code calls (docs/sora-combat-spec.md 7.9; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['slot', V, 'int', '-', '-'], ['vt_offset', V, 'str', '-', '-'], ['address', R, 'addr', 'code', '-'], ['meaning', V, 'str', '-', '-'], ['evidence', V, 'str', '-', '-']] },
  'sora-combat-lockon-hud': { group: 'port', file: 'sora-combat-lockon-hud.json', by: 'tools/sora_combat_sheet.js', from: 'HUD calls of the lock-on code (docs/sora-combat-spec.md 7.11; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['call', V, 'str', '-', '-'], ['address', R, 'addr', 'code', '-'], ['when', V, 'str', '-', '-'], ['effect', V, 'str', '-', '-'], ['evidence', V, 'str', '-', '-']] },
  'sora-combat-damage-params': { group: 'port', file: 'sora-combat-damage-params.json', by: 'tools/sora_combat_sheet.js', from: 'PlayerDamageCommonParam (TresDamageCommonParam): native default, asset value and the value the player uses (docs/sora-combat-spec.md 9.3; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s name'], ['name', V, 'str', '-', '-'], ['offset', V, 'str', '-', '-'], ['native_default', V, 'num', '-', '-'], ['asset_value', V, 'num', '-', '-'], ['player_value', V, 'num', '-', '-'], ['read_by_slot', V, 'int', '-', '-']] },
  'sora-combat-damage-handlers': { group: 'port', file: 'sora-combat-damage-handlers.json', by: 'tools/sora_combat_sheet.js', from: 'TresPlayerState_Damage: damage kind -> handler, with the exits each handler allows (docs/sora-combat-spec.md 9.3; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s handler'], ['damage_kinds', V, 'str', '-', '-'], ['handler', V, 'str', '-', '-'], ['slot', V, 'int', '-', '-'], ['ref_addr', R, 'addr', 'code', '-'], ['attack_out', V, 'str', '-', '-'], ['jump_out', V, 'str', '-', '-'], ['aerial_recovery', V, 'str', '-', '-'], ['clips', V, 'str', '-', '-'], ['ref_conf', V, 'str', '-', '-']] },
  'sora-combat-damage-clips': { group: 'port', file: 'sora-combat-damage-clips.json', by: 'tools/sora_combat_sheet.js', from: 'clips of the hit reactions with length, change point and the play mode the handler passes (docs/sora-combat-spec.md 9.4; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s anim'], ['anim', V, 'str', '-', '-'], ['clip', V, 'str', '-', '-'], ['length_s', V, 'num', '-', '-'], ['enable_change_s', V, 'num', '-', '-'], ['play_mode', V, 'int', '-', '-'], ['role', V, 'str', '-', '-']] },
  'sora-combat-hurt-shapes': { group: 'port', file: 'sora-combat-hurt-shapes.json', by: 'tools/sora_combat_sheet.js', from: 'the 13 TresBodyCollPrimitive templates of p_ex001_PawnBase (Sora\'s hurt, push and camera bodies) (docs/sora-combat-spec.md 9.10; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s variable'], ['variable', V, 'str', '-', '-'], ['group_name', V, 'str', '-', '-'], ['ignore_none_group', V, 'yesno', '-', '-'], ['shape', V, 'str', '-', '-'], ['size_x', V, 'int', '-', '-'], ['size_y', V, 'int', '-', '-'], ['size_z', V, 'int', '-', '-'], ['socket', V, 'str', '-', '-'], ['loc_x', V, 'int', '-', '-'], ['loc_y', V, 'int', '-', '-'], ['loc_z', V, 'int', '-', '-'], ['rot_pitch', V, 'int', '-', '-'], ['rot_yaw', V, 'int', '-', '-'], ['rot_roll', V, 'int', '-', '-'], ['damage', V, 'yesno', '-', '-'], ['push', V, 'yesno', '-', '-'], ['camera', V, 'yesno', '-', '-'], ['reaction_type', V, 'str', '-', '-'], ['template_export', V, 'str', '-', '-']] },
  'sora-combat-hp-rules': { group: 'port', file: 'sora-combat-hp-rules.json', by: 'tools/sora_combat_sheet.js', from: 'ApplyHP 0x1405A2A00: the rules applied to new = HP - Floor(amount), in order (docs/sora-combat-spec.md 9.7; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['order', V, 'int', '-', '-'], ['condition', V, 'str', '-', '-'], ['result', V, 'str', '-', '-'], ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'str', '-', '-']] },
  'sora-combat-hit-blocks': { group: 'port', file: 'sora-combat-hit-blocks.json', by: 'tools/sora_combat_sheet.js', from: 'everything that can stop a hit on Sora from damaging or staggering him (docs/sora-combat-spec.md 9.6; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['source', V, 'str', '-', '-'], ['storage', V, 'str', '-', '-'], ['set_by', V, 'str', '-', '-'], ['tested_in', V, 'str', '-', '-'], ['effect', V, 'str', '-', '-'], ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'str', '-', '-']] },
  'sora-combat-form-clips': { group: 'port', file: 'sora-combat-form-clips.json', by: 'tools/sora_combat_sheet.js', from: 'clips of the formchange, the Second Form finishers and the finish commands with their gameplay notifies (docs/sora-combat-spec.md 10.4; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s anim'], ['anim', V, 'str', '-', '-'], ['clip', V, 'str', '-', '-'], ['length_s', V, 'num', '-', '-'], ['root_motion', V, 'str', '-', '-'], ['root_fwd_cm', V, 'num', '-', '-'], ['root_up_cm', V, 'int', '-', '-'], ['enable_input_s', V, 'str', '-', '-'], ['enable_change_s', V, 'str', '-', '-'], ['traction_s', V, 'str', '-', '-'], ['face_at_s', V, 'str', '-', '-'], ['hit_windows', V, 'int', '-', '-'], ['other_notify_classes', V, 'str', '-', '-']] },
  'sora-combat-form-commands': { group: 'port', file: 'sora-combat-form-commands.json', by: 'tools/sora_combat_sheet.js', from: 'rows of Load/Common/TresCmdData (TresCommandKindData) used by attack, formchange, finish and grand-magic commands (docs/sora-combat-spec.md 10.1; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s enum'], ['kind', V, 'int', '-', '-'], ['enum', V, 'str', '-', '-'], ['Type', V, 'int', '-', '-'], ['Cate', V, 'int', '-', '-'], ['Attr', V, 'int', '-', '-'], ['Local', V, 'int', '-', '-'], ['Param0', V, 'int', '-', '-'], ['Param1', V, 'int', '-', '-'], ['Param2', V, 'int', '-', '-'], ['Param3', V, 'int', '-', '-'], ['LocKey', V, 'str', '-', '-']] },
  'sora-combat-finish-levels': { group: 'port', file: 'sora-combat-finish-levels.json', by: 'tools/sora_combat_sheet.js', from: 'finish level -> command -> per-command tick of TresPlayerState_StyleFinish (docs/sora-combat-spec.md 10.6; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s enum'], ['level', V, 'int', '-', '-'], ['command', V, 'int', '-', '-'], ['enum', V, 'str', '-', '-'], ['state_slot', V, 'int', '-', '-'], ['function', R, 'addr', 'code', '-'], ['clips', V, 'str', '-', '-'], ['extra_commands', V, 'str', '-', '-'], ['evidence', V, 'str', '-', '-']] },
  'sora-combat-form-abilities': { group: 'port', file: 'sora-combat-form-abilities.json', by: 'tools/sora_combat_sheet.js', from: 'abilities added while wear form TRES_WEAR_FORM_05 (Second Form) is worn (docs/sora-combat-spec.md 10.5; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s ability'], ['wear_form', V, 'str', '-', '-'], ['ability', V, 'str', '-', '-'], ['ability_id', V, 'int', '-', '-'], ['copies', V, 'int', '-', '-'], ['source', V, 'str', '-', '-']] },
  'sora-combat-queue-classes': { group: 'port', file: 'sora-combat-queue-classes.json', by: 'tools/sora_combat_sheet.js', from: 'where each kind of command goes in the HUD command part (classifier 0x141619E60) (docs/sora-combat-spec.md 11.1; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'row number in table order'], ['cmd_type', V, 'int', '-', '-'], ['cate', V, 'json', '-', '-'], ['kinds', V, 'str', '-', '-'], ['hud_class', V, 'json', '-', '-'], ['container', V, 'str', '-', '-'], ['counted', V, 'str', '-', '-'], ['hud_countdown', V, 'str', '-', '-'], ['producer', V, 'str', '-', '-'], ['sora_only', V, 'str', '-', '-']] },
  'sora-combat-form-fields': { group: 'port', file: 'sora-combat-form-fields.json', by: 'tools/sora_combat_sheet.js', from: 'style-gauge and formchange fields of the player controller (docs/sora-combat-spec.md 10.1; authored rows in tools/lib/sora_combat_pass2.json)', external: true, cols: [
    ['id', K, 'str', '-', 'the row\'s offset'], ['offset', V, 'str', '-', '-'], ['type', V, 'str', '-', '-'], ['name', V, 'str', '-', '-'], ['accessor', V, 'str', '-', '-'], ['meaning', V, 'str', '-', '-']] },
  // audio of the player character (docs/sora-audio-spec.md): written by tools/sora_audio_pack.js from audit/sora_port/audio
  'sora-sounds': { group: 'port', file: 'sora-sounds.json', by: 'tools/sora_audio_pack.js', from: 'audit/sora_port/audio (animation notifies, auto-SE table, voice groups and weapon sets of p_ex001 read from the converted assets; sounds played from native code and blueprints), resolved against the SEAD banks under assets_converted/audio', external: true, cols: [
    ['id', K, 'str', '-', '<route>.<trigger>[#n]: one row per trigger and sound it can play'], ['route', V, 'str', '-', 'how the sound is triggered: notify.* (animation notify), autose.* (auto-SE table entry), voice.group, weapon.*, code.* (native code or blueprint)'],
    ['trigger', V, 'str', '-', 'notify class and animation asset, table entry, or the code path'], ['clip', V, 'str', '-', 'animation sequence the notify sits in; "-" otherwise'],
    ['time', V, 'json', '-', 'notify trigger time in seconds; "-" otherwise'], ['duration', V, 'json', '-', 'notify state duration in seconds; "-" otherwise'],
    ['sound', V, 'str', '-', 'SQEXSEADSound asset (voices by their /Localization/ja/ path), or the auto-SE lookup key; "-" when the trigger plays nothing'],
    ['bank', V, 'str', '-', 'bank in build/sora_pack/audio/manifest.json (path under Content/Sound, no extension); "-" when not packed'], ['sound_index', V, 'json', '-', 'index of the sound in the bank'],
    ['selection', V, 'str', '-', 'how this sound is chosen among the rows of the same trigger'], ['spatial', V, 'str', '-', '2D or 3D as the trigger plays it'],
    ['attach', V, 'str', '-', 'socket or bone the sound is placed at, and whether it follows'], ['volume', V, 'json', '-', 'volume multiplier given by the trigger'], ['pitch', V, 'json', '-', 'pitch multiplier given by the trigger'],
    ['condition', V, 'str', '-', 'condition of the trigger', true], ['packed', V, 'str', '-', 'yes when the sound is in the audio pack'],
    ['evidence', V, 'str', '-', 'asset = read from converted game data; instruction / decompiled C for code rows'], ['ref_addr', R, 'addr', 'code', 'function that plays or resolves the sound; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = asset data or instruction level; inferred = decompiled C or derived'], ['note', V, 'str', '-', '-', true]] },
  // authored rows kept in tools/sora_sound_rules_sheet.js: the constants of the SEAD playback model as far as Sora needs it
  'sora-sound-rules': { group: 'port', file: 'sora-sound-rules.json', by: 'tools/sora_sound_rules_sheet.js', from: 'SEAD driver and UE plugin code of base/KH3.exe, the SEAD configuration in WindowsEngine.ini, class default objects and auto-SE assets of p_ex001', external: true, cols: [
    ['id', K, 'str', '-', '<group>.<parameter>'], ['value', V, 'json', '-', 'number, boolean or name; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 's, ms, cm, m, m/s, ratio, count and combinations; "-" for a flag or name'], ['source', V, 'str', '-', 'where the value is stored: instruction address, config section, or asset path and property'],
    ['evidence', V, 'str', '-', 'how the value is used and how it was verified', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level, or in the config / an asset; inferred = decompiled C only or stock engine behaviour; guess = not determined'],
    ['note', V, 'str', '-', '-']] },
  // authored rows kept in tools/sora_ui_sheet.js: HUD and menus of the Sora port (docs/sora-ui-spec.md)
  'sora-ui-elements': { group: 'port', file: 'sora-ui-elements.json', by: 'tools/sora_ui_sheet.js', from: 'Scaleform movies of UI/Scaleform decoded by tools/sora_ui_pack.js (timelines, AS3), UI part classes of base/KH3.exe (TresUIP_*) read in decompiled C and disassembly, tools/lib/sora_ui_iface.json', external: true, cols: [
    ['id', K, 'str', '-', '<screen group>.<element>, or iface.<Movie>.<method> for one method of an AS3 document class'], ['screen', R, 'str', 'sora-ui-screens', 'screen the element belongs to'],
    ['movie', V, 'str', '-', 'movie of the UI pack (build/sora_pack/ui/movies/<movie>.json); "-" when none'], ['symbol', V, 'str', '-', 'instance path and symbol class in the movie, or the native members bound to it'],
    ['driver', V, 'str', '-', 'native function (name, vtable slot, address) that feeds the element; unchecked = not found'], ['invoke', V, 'str', '-', 'AS3 method(s) or frame labels the driver uses, with argument meaning'],
    ['data', V, 'str', '-', 'game value the element shows'], ['rule', V, 'str', '-', 'the exact rule (formula, thresholds, frames); unchecked = not determined (never guessed)'],
    ['layout', V, 'str', '-', 'position on the 1920x1080 stage (from the pack layout files)'], ['anim', V, 'str', '-', 'frame count and frame labels (1-based frames at 60 fps)'],
    ['evidence', V, 'enum:instr|decomp|asset|ue|asset+decomp|asset+instr', '-', 'instr = read at instruction level; decomp = decompiled C only; asset = read from the movie or a data asset; ue = stock UE 4.17', true],
    ['ref_addr', R, 'addr', 'code', 'function that drives the element; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = rule read in the movie and at instruction level / in the data; inferred = decompiled C or derived; guess = not determined (rule unchecked)'],
    ['note', V, 'str', '-', '-']] },
  'sora-ui-screens': { group: 'port', file: 'sora-ui-screens.json', by: 'tools/sora_ui_sheet.js', from: 'the same evidence as sora-ui-elements; reflected members of TresUIManager, TresCockpit and the TresUIP_* classes', external: true, cols: [
    ['id', K, 'str', '-', '-'], ['kind', V, 'enum:hud|menu|system', '-', '-'], ['movie', V, 'str', '-', 'package path under UI/Scaleform; "-" when none'], ['native_class', V, 'str', '-', 'class that owns the movie'],
    ['opened_by', V, 'str', '-', 'owner object / what opens the screen'], ['flow', V, 'str', '-', 'sub screens and how they are reached; unchecked = not determined'], ['input', V, 'str', '-', 'input the screen consumes; unchecked = not determined'],
    ['sounds', V, 'str', '-', 'sounds it requests; unchecked = not determined'], ['evidence', V, 'enum:instr|decomp|asset|ue|asset+decomp|asset+instr', '-', 'as in sora-ui-elements', true], ['ref_addr', R, 'addr', 'code', 'constructor or main function of the native class; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'as in sora-ui-elements'], ['note', V, 'str', '-', '-']] },
  'sora-ui-params': { group: 'port', file: 'sora-ui-params.json', by: 'tools/sora_ui_sheet.js', from: 'HUD code of base/KH3.exe read in disassembly, AS3 constants of the movies, DataTable rows of p_ex001', external: true, cols: [
    ['id', K, 'str', '-', '<group>.<parameter>'], ['value', V, 'json', '-', 'number; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'px, frames (1/60 s), s, HP and so on; "-" for a ratio or count'], ['source', V, 'str', '-', 'where the value is stored: instruction, AS3 constant, or asset row'],
    ['evidence', V, 'str', '-', 'how the value is used and how it was verified', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level, in an AS3 constant or in a data row; inferred = decompiled C or derived; guess = not determined'],
    ['note', V, 'str', '-', '-']] },
  // Sora's character progression and status systems (docs/sora-stats-spec.md); all six are written by tools/sora_stats_sheet.js:
  // data rows are read from the game's tables and from base/KH3.exe on every run, code facts are authored rows there
  'sora-levels': { group: 'port', file: 'sora-levels.json', by: 'tools/sora_stats_sheet.js', from: 'sheets/entities/DataTable/Player/LevelUp/p_ex001_LevelUpData.json (99 rows); use of the row read in 0x1405832D0 / 0x140576E70 / 0x140582FD0', external: true, cols: [
    ['id', K, 'str', '-', 'level 1..99 (row name of the table)'], ['exp_total', V, 'int', '-', 'm_Exp: total EXP at which the level is reached'], ['exp_from_prev', V, 'int', '-', 'm_Exp minus m_Exp of the previous level (0 at level 1)'],
    ['exp_to_next', V, 'int', '-', 'EXP from this level to the next (0 at level 99)'], ['attack', V, 'int', '-', 'm_AttackPower: Strength of the level (absolute)'], ['magic', V, 'int', '-', 'm_MagicPower (absolute)'],
    ['defense', V, 'int', '-', 'm_DefensePower (absolute)'], ['ap', V, 'int', '-', 'm_AbilityPoint (absolute)'], ['d_attack', V, 'int', '-', 'difference to the previous level, as shown by the level-up panel'],
    ['d_magic', V, 'int', '-', '-'], ['d_defense', V, 'int', '-', '-'], ['d_ap', V, 'int', '-', '-'],
    ['ability_a', V, 'str', '-', 'm_AbilityKind1: ability granted on ability path 0 (ETresPlayerAbilityGrowthType TYPE_A); "-" = none'], ['ability_b', V, 'str', '-', 'm_AbilityKind2 (path 1, TYPE_B)'], ['ability_c', V, 'str', '-', 'm_AbilityKind3 (path 2, TYPE_C)'],
    ['source', V, 'str', '-', 'table row', true], ['ref_addr', R, 'addr', 'code', 'function that reads the row'], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level or in a data table; inferred = decompiled C only, or derived; guess = not determined']] },
  'sora-abilities': { group: 'port', file: 'sora-abilities.json', by: 'tools/sora_stats_sheet.js', from: 'reflect/enum-values ETresAbilityKind; sheets/entities/Load/Tres/TresAbilityData.json; level, victory bonus, equipment, upgrade, meal and start tables; ability-query call sites decoded from base/KH3.exe; effects read in the named functions', external: true, cols: [
    ['id', K, 'str', '-', 'ETresAbilityKind name'], ['enum_value', V, 'int', '-', 'value of the enum = id used by the code'], ['row', V, 'str', '-', 'row of TresAbilityData; "-" = none'], ['name', V, 'str', '-', 'English name (kh3_system locres); "-" = no text'],
    ['category', V, 'str', '-', 'm_Category: ACTION, MOVE or SUPPORT'], ['equip_type', V, 'str', '-', 'm_EquipType'], ['ap', V, 'int', '-', 'm_UseAP per equipped copy'], ['use_mp', V, 'int', '-', 'm_UseMP (party command abilities; -1 = all MP)'],
    ['max_equip', V, 'int', '-', 'm_MaxEquip of the row (not a cap on owned copies: see the spec)'], ['ui_priority', V, 'int', '-', 'm_UIPriority: order in the ability menu'],
    ['owner', V, 'enum:sora|cuisine|party|unused', '-', 'sora = Sora can own or wear it; cuisine = only as a full-course meal bonus; party = party members only; unused = no source in the data'],
    ['sources', V, 'list', '-', 'how it is obtained: start set, level per path (A:.. B:.. C:..), victory bonus rows, keyblades, upgrades, armour, accessories, meal'],
    ['copies', V, 'int', '-', 'copies Sora owns with the start set, level 99 and every victory bonus (equipment not counted)'], ['system', V, 'str', '-', 'system that implements the effect'],
    ['effect', V, 'str', '-', 'effect as implemented in the code, with the function addresses'], ['help_text', V, 'str', '-', 'the description shown by the game (English); "-" = none'],
    ['hooks', R, 'list', 'code', 'functions that query the ability'], ['hook_sites', V, 'list', '-', '<function>@<call site>:<query kind> for every constant-id query (has, count, level, party-has, party-count)'],
    ['status', V, 'enum:full|partial|hook-only|none', '-', 'full = effect and numbers read in code; partial = hook and qualitative effect read; hook-only = call sites known, effect from the help text; none = no code reference found'],
    ['evidence', V, 'str', '-', 'where the row comes from', true], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level or in a data table; inferred = decompiled C only, or derived; guess = not determined'], ['test', V, 'str', '-', 'how to observe the effect in a test level; "-" = not applicable'],
    // (X1, 2026-10-15) the port's own column: excluded = never granted, listed or loaded (sheets/decisions.json dec029: Attraction Flow is not ported)
    ['port', V, 'enum:in|excluded|ours', '-', 'in = the port has the ability as KH3 does; excluded = the port never grants, lists, equips or loads it (user decision dec029); ours = (XSI) a port ability of XS_SORA_ABILITIES.md (our design, value 256..)']] },
  'sora-stats-rules': { group: 'port', file: 'sora-stats-rules.json', by: 'tools/sora_stats_sheet.js', from: 'status, EXP, damage and HP / MP code of base/KH3.exe (decompiled C; constants and the damage function read in disassembly); base and level tables of p_ex001', external: true, cols: [
    ['id', K, 'str', '-', '<group>.<parameter>'], ['value', V, 'json', '-', 'number, boolean, name, list or formula text; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'HP, MP, FP, AP, EXP, s, level, tier; "-" for a ratio, flag, name or formula'], ['source', V, 'str', '-', 'where the value is stored or computed: function address, instruction address or data table and field'],
    ['evidence', V, 'str', '-', 'how the value is used and how it was verified', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level or in a data table; inferred = decompiled C only, or derived; guess = not determined'],
    ['note', V, 'str', '-', '-']] },
  'sora-difficulty': { group: 'port', file: 'sora-difficulty.json', by: 'tools/sora_stats_sheet.js', from: 'game-level tests in base/KH3.exe: 0x140575C80, 0x1405864C0, 0x140632CB0, 0x140642820, 0x140576130, 0x140E92280, 0x1405AB250; TresChrInitData', external: true, cols: [
    ['id', K, 'str', '-', 'ETresGameLevelID name'], ['game_level', V, 'int', '-', 'value stored at GW+0x7DF0'], ['base_hp', V, 'int', '-', 'Sora max HP before bonuses'], ['base_mp', V, 'int', '-', 'Sora max MP before bonuses'],
    ['hp_up', V, 'list', '-', 'amount of HP_UP3 / 5 / 10 / 15 / 30'], ['mp_up', V, 'list', '-', 'amount of MP_UP3 / 5 / 10 / 20'], ['exp_rule', V, 'str', '-', 'EXP awarded for an enemy reward exp'],
    ['player_damage', V, 'num', '-', 'factor on damage dealt by the player side'], ['enemy_damage', V, 'num', '-', 'factor on damage dealt by the other side'], ['enemy_attack_add', V, 'int', '-', 'added to the enemy attack stat at spawn'],
    ['ap_add', V, 'int', '-', 'AP added to Sora at record init'], ['mp_charge_time', V, 'num', '-', 'MP charge duration in seconds before haste'], ['start_abilities', V, 'list', '-', 'abilities added to the start set'],
    ['note', V, 'str', '-', '-'], ['ref_addr', R, 'addr', 'code', '-'], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level or in a data table; inferred = decompiled C only, or derived; guess = not determined']] },
  // (XSI, 2026-10-16) the XS abilities (build/sora_tmp/party/XS_SORA_ABILITIES.md, approved): authored in tools/sora_xs_sheet.js
  'sora-enum-ext': { group: 'port', file: 'sora-enum-ext.json', by: 'tools/sora_xs_sheet.js (authored)', from: 'our design: the port\x27s own values of reflected enums (the XS abilities, ETresAbilityKind 256..), added by tools/gen_port.js after the game\x27s values', external: true, cols: [
    ['id', K, 'str', '-', '<enum>/<name>'], ['enum', V, 'str', '-', 'the reflected enum (sheets/reflect/enum-values) the value is added to'], ['name', V, 'str', '-', '<enum>::<NAME>, as the game names its own values'],
    ['enum_value', V, 'int', '-', 'the value; never one the game has'], ['source', V, 'str', '-', 'who chose it and where it is described', true], ['note', V, 'str', '-', '-']] },
  'sora-xs-abilities': { group: 'port', file: 'sora-xs-abilities.json', by: 'tools/sora_xs_sheet.js (authored)', from: 'our design (XS_SORA_ABILITIES.md, approved by the user 2026-10-16) and the KH3 rows it names (TresShootFlowData, pawn defaults, SoKc projectile)', external: true, cols: [
    ['id', K, 'str', '-', '<ability>.<constant>'], ['value', V, 'json', '-', 'a number'], ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'OUR DESIGN with its XS section, or the KH3 data it is read from'],
    ['evidence', V, 'str', '-', 'what the code does with it', true], ['ref_addr', V, 'str', '-', '"-": no game code reads these rows'], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice or a value read from the game data'], ['note', V, 'str', '-', '-']] },
  'sora-xs-battle-levels': { group: 'port', file: 'sora-xs-battle-levels.json', by: 'tools/sora_xs_sheet.js (authored)', from: 'our design: Hidden Potential\x27s battle level of an Elden Ring boss / Giant body by region depth (the bands of sora-er-boss-rewards small.tier_cap)', external: true, cols: [
    ['id', K, 'str', '-', 'depth.<0..4> = a region band; default = no region known'], ['value', V, 'json', '-', 'battle level 1..99'], ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'OUR DESIGN'],
    ['evidence', V, 'str', '-', 'the regions of the band and the floor it gives', true], ['ref_addr', V, 'str', '-', '"-"'], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice'], ['note', V, 'str', '-', '-']] },
  // (DF, 2026-10-15) KH3 Critical mode's start abilities from the victory-bonus rows flagged m_InitAbilityEquipOnCriticalMode
  'sora-critical-start': { group: 'port', file: 'sora-critical-start.json', by: 'tools/sora_critical_sheet.js', from: 'sheets/entities/Load/Tres/TresVBonusData.json (m_InitAbilityEquipOnCriticalMode, m_AbilitySora1), the skip in 0x140586280 case 1, sora-abilities (AP)', external: true, cols: [
    ['id', K, 'str', '-', 'victory-bonus row name, or <m_Critical* list>.<ability>'], ['group', V, 'enum:init|vbonus', '-', 'init = TresChrInitData m_CriticalEquipAbility / m_CriticalHaveAbility (granted by Status::from_loadout); vbonus = a flagged victory-bonus row'], ['ability', V, 'str', '-', 'ETresAbilityKind name (m_AbilitySora1)'], ['mode', V, 'enum:EQUIP|HAVE', '-', 'm_InitAbilityEquipOnCriticalMode: EQUIP = owned and equipped, HAVE = owned only'],
    ['ap', V, 'int', '-', 'm_UseAP of the ability'], ['name', V, 'str', '-', 'English name'], ['order', V, 'int', '-', 'row order (the equip order at a Critical start)'], ['port', V, 'enum:in|excluded', '-', '(DF, dec032) excluded = KH3 data kept, not part of the port Critical start'],
    ['source', V, 'str', '-', 'table and row', true], ['ref_addr', R, 'addr', 'code', 'the Critical skip of the row\x27s ability'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'inferred: the start grant is not located in the code'], ['note', V, 'str', '-', '-']] },
  'sora-equipment': { group: 'port', file: 'sora-equipment.json', by: 'tools/sora_stats_sheet.js', from: 'sheets/entities/Load/Tres/TresEquipItemData.json (Sora keyblades, armour, accessories), ItemSynthesis/TresItemWeaponEnhanceData.json (Kingdom Key), TresVBonusData.json and the UTresChrInitData constructor (slot counts)', external: true, cols: [
    ['id', K, 'str', '-', 'm_KeyName of the equipment row; <key>.upgrade.<n>; slot.<kind>'], ['kind', V, 'enum:keyblade|keyblade-upgrade|armour|accessory|slot', '-', '-'], ['row', V, 'str', '-', 'table row; "-" for slot rows'],
    ['name', V, 'str', '-', 'English name; "-" = not resolved'], ['ap', V, 'int', '-', 'm_AP added to max AP; for slot rows the slot count'], ['attack', V, 'int', '-', 'm_AttackPlus'], ['magic', V, 'int', '-', 'm_MagicPlus'], ['defense', V, 'int', '-', 'm_DefensePlus'],
    ['resist', V, 'list', '-', 'm_AttrResist*: physical, fire, blizzard, thunder, water, aero, dark, no-type (percent)'], ['abilities', V, 'list', '-', 'm_AppendAbility: abilities active while worn (no AP cost)'],
    ['source', V, 'str', '-', 'table or code', true], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level or in a data table; inferred = decompiled C only, or derived; guess = not determined'], ['note', V, 'str', '-', '-']] },
  'sora-loadout-99': { group: 'port', file: 'sora-loadout-99.json', by: 'tools/sora_stats_sheet.js', from: 'level, base, victory bonus, start and keyblade tables of the game and the formulas of sora-stats-rules; rows with source "test configuration" are choices of the port', external: true, cols: [
    ['id', K, 'str', '-', '<parameter>[.<opening choice>.<mode>]'], ['value', V, 'json', '-', 'number, boolean, name, list or formula text; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'HP, MP, FP, AP, EXP, s, level, tier; "-" for a ratio, flag, name or formula'], ['source', V, 'str', '-', 'where the value is stored or computed: function address, instruction address or data table and field'],
    ['evidence', V, 'str', '-', 'how the value is used and how it was verified', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level or in a data table; inferred = decompiled C only, or derived; guess = not determined'],
    ['note', V, 'str', '-', '-']] },
  // (magic implementer) the shortcut sets of the level-99 test configuration: authored here, copied into sora-loadout-99 by tools/sora_stats_sheet.js
  'sora-shortcuts': { group: 'port', file: 'sora-shortcuts.json', by: 'authored (magic implementer)', from: 'a choice of the test level, not game data: a new game has all 12 shortcut slots empty and only the Customize menu writes them (docs/sora-magic-spec.md J 3.1)', external: true, cols: [
    ['id', K, 'str', '-', 'shortcuts.a / .b / .c = Shortcut Set A / B / C'], ['value', V, 'json', '-', 'four command names (ETresCommandKind tails, "" = empty) in the order right, top, left, bottom face button'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'the slot order', true], ['ref_addr', V, 'str', '-', '"-": no code reads these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice'],
    ['note', V, 'str', '-', '-']] },
  // (integration implementer) what the test level puts into the item and equipment stock: authored here, read by the test app at start
  'sora-test-stock': { group: 'port', file: 'sora-test-stock.json', by: 'authored (integration implementer)', from: 'a choice of the test level, not game data: the level-99 loadout has an empty stock, so the Items and Equipment screens of the camp menu had no content (build/sora_tmp/IMPL_INTEGRATION_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'stock.items = battle items in stock; stock.equipment = spare armour / accessories in stock'], ['value', V, 'json', '-', 'object: item name (ItemKind::name) or equipment row id -> count'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'where the names come from', true], ['ref_addr', V, 'str', '-', '"-": no game code reads these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice'],
    ['note', V, 'str', '-', '-']] },
  // (fixes pass 1) the level 1 / 20 / 99 test loadouts the test app cycles through: authored here, read by the test app
  // (party, stream F) the party's level-99 test loadout and the camp menu's spare staffs / shields: authored, read by the test app
  'party-test': { group: 'port', file: 'party-test.json', by: 'authored (party stream F)', from: 'a choice of the test level (party-port-plan.md open note N2, user decision 2), not game data; each row says what game data it takes (build/sora_tmp/party/IMPL_F_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'party.level, <chara>.loadout, stock.weapons, spawn.offsets'], ['value', V, 'json', '-', 'object, see the row'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'the game data the row takes', true], ['ref_addr', V, 'str', '-', 'function the row cites, or "-"'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice'],
    ['note', V, 'str', '-', '-']] },
  'sora-test-levels': { group: 'port', file: 'sora-test-levels.json', by: 'authored (fixes pass 1)', from: 'a choice of the test level by request of the user, not game data: the levels and magic tiers the run-time level toggle of the test app cycles through (build/sora_tmp/IMPL_FIXES1_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'level.<N>: one test loadout'], ['value', V, 'json', '-', 'object: level (1..99), magic_tier (1 base, 2 -ra, 3 -ga), story_bonuses (0 = none; 1 = all of the default loadout), new_game (1 = the true start of a new game, sheet sora-newgame: the other two are then not used)'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'how the loadout is built', true], ['ref_addr', V, 'str', '-', '"-": no game code reads these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice'],
    ['note', V, 'str', '-', '-']] },
  // (new game) what a new game starts Sora with, in the row form of sora-loadout-99: authored here from the game's start data, compiled into crates/kh-host (save.rs) and read by the test app
  'sora-newgame': { group: 'port', file: 'sora-newgame.json', by: 'authored (new-game implementer)', from: 'the start of a new game as the game\x27s data gives it: TresChrInitData m_PlayerSora (weapon, start abilities), level row 1, the base row, the victory bonuses of the opening level ew_01 (the choice row and Vbonus_082 Fire + Water), the Kingdom Key\x27s shotlocks (docs/sora-stats-spec.md section 14, build/sora_tmp/IMPL_NEWGAME_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'a loadout row id (crates/kh-core/src/status/loadout.rs lists them)'], ['value', V, 'json', '-', 'number, name, list or map, as in sora-loadout-99'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the data table and field, or the script, the value comes from'],
    ['evidence', V, 'str', '-', 'how it was established', true], ['ref_addr', V, 'str', '-', 'function or instruction that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read in a data table or script; inferred = derived by the rules of sora-stats-rules'],
    ['note', V, 'str', '-', '-']] },
  // (Elden Ring gameplay hookup) the numbers of the Elden Ring host: damage conversion both ways, target range, weight classes; authored here, read by crates/kh-er at Sora's start
  'sora-er-host': { group: 'port', file: 'sora-er-host.json', by: 'authored (Elden Ring gameplay hookup)', from: 'choices of the Elden Ring host, not data of either game: how Sora\x27s damage numbers map to Elden Ring hit points and back, which characters are targets, their weight classes (build/sora_tmp/er/IMPL_ER4_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'damage.out.* / damage.in.* = the conversions; target.* = enumeration, weight classes and the class table (target.npc.<cNNNN | NpcParam id>); react.* = KH3 reaction kind -> Elden Ring dmgLevel'], ['value', V, 'json', '-', 'a number; an object for a weight class (max_radius_cm, max_height_cm, push_level, puppet) or a reaction (level, air_level, poise); "puppet" | "armoured" for a target.npc row'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'what the choice rests on', true], ['ref_addr', V, 'str', '-', '"-": no game code reads these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice'],
    ['note', V, 'str', '-', '-']] },
  // (ER party S3) the AI allies' invisible Elden Ring bodies: which Spirit Ash rows, the spawn / respawn / HP / parking rules, the go/no-go test (F1 redirection removed by the user 2026-10-15); authored, read by crates/kh-er/src/allybody.rs at the pool's start
  'party-er-bodies': { group: 'port', file: 'party-er-bodies.json', by: 'authored (ER party phase stream S3)', from: 'choices of the Elden Ring party host, not data of either game: how the AI allies get bodies the game\x27s enemies target (build/sora_tmp/er/party/ER_PARTY_RESEARCH.md part 1, IMPL_S3_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'body.* = the pool (routes, Spirit Ash rows, spawn / respawn, HP, teams, parking); owner.* = stage-2 bullets; test.* = the go/no-go test'], ['value', V, 'json', '-', 'a number, a string, a list, or null (the run-time default named in the evidence)'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'what the choice rests on', true], ['ref_addr', V, 'str', '-', '"-": no game code reads these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'guess = a host choice not yet seen in the game'],
    ['note', V, 'str', '-', '-']] },
  // (AGGRO) whom Elden Ring's enemies attack: the threat model's weights, the holds and switches, the hate hits; authored, read by crates/kh-er/src/party/aggro.rs at the director's start and at every reload of the control file
  'party-er-aggro': { group: 'port', file: 'party-er-aggro.json', by: 'authored (stream AGGRO)', from: 'choices of the Elden Ring party host, not data of either game: how every engaged enemy picks a party member to attack (build/sora_tmp/party/IMPL_AGGRO_STATE.md); the abilities\x27 own rules stay in party-don-progression (aggro.*) and Goofy\x27s kit', external: true, cols: [
    ['id', K, 'str', '-', 'aggro.* = the director (weights, windows, holds, hate hits)'], ['value', V, 'json', '-', 'a number'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'what the choice rests on', true], ['ref_addr', V, 'str', '-', '"-": no game code reads these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'guess = a host choice not yet seen in the game'],
    ['note', V, 'str', '-', '-']] },
  // (Elden Ring sound / effects) Elden Ring ground material -> KH3 footstep surface, output level, effect scale; authored, read by crates/kh-er/src/fxmap.rs at Sora's start
  // (party stream E) Donald's and Goofy's sounds, voices and effects (tools/party_audio_pack.js; docs/party-contracts.md E)
  'party-sounds': { group: 'port', file: 'party-sounds.json', by: 'tools/party_audio_pack.js', external: true, from: 'the n_ex001 / n_ex002 anim sets (notifies of every clip), FsTable_Donald / _Goofy, the 14 + 14 weapon blueprints sound sets, Sound/SE/_Data/SeAttach of every party effect, the n_ex001 spell actor blueprints; packed by tools/sora_audio_pack.js (party rows of audit/sora_port/audio/party_rows.json)', cols: [
    ['id', K, 'str', '-', '<owner>.notify.<clip>.<n> | <owner>.autose.<surface>.<slot> | <owner>.<weapon>.swing|hit.<n> | code.<category>.<trigger>'],
    ['owner', V, 'str', '-', 'n_ex001 (Donald) | n_ex002 (Goofy); "-" for the shared effect / spell actor rows'],
    ['route', V, 'str', '-', 'the route of the audio pack trigger (notify.*, autose.*, weapon.*, code.*)'], ['trigger', V, 'str', '-', 'notify class and animation asset, table entry, or the code path'],
    ['logical', V, 'str', '-', 'anim-set rows that play the clip ("|" separated); "-" otherwise'], ['time', V, 'json', '-', 'notify time in seconds; "-" otherwise'],
    ['sound', V, 'str', '-', 'the SQEXSEADSound asset(s) ("|" separated), the auto-SE lookup, or "-"'], ['packable', V, 'str', '-', 'yes = every named sound resolves to a bank sound; no / empty otherwise'],
    ['evidence', V, 'str', '-', 'asset = read from the converted packages'], ['note', V, 'str', '-', '-']] },
  'party-voice': { group: 'port', file: 'party-voice.json', by: 'tools/party_audio_pack.js', external: true, from: 'Blueprints/Npc/n_ex00x/DataTable/n_ex00x_VoiceGroupSet (TresVoiceGroupSet)', cols: [
    ['id', K, 'str', '-', '<owner>.<group>.<number>'], ['owner', V, 'str', '-', 'n_ex001 | n_ex002'], ['group', V, 'str', '-', 'm_GroupName'], ['number', V, 'int', '-', 'm_GroupNo'],
    ['voice_ja', V, 'str', '-', 'm_pVoice (the /Localization/ja/ reference)'], ['voice_en', V, 'str', '-', 'its English twin'], ['exists', V, 'yesno', '-', 'the asset exists'],
    ['play_on_group_not_found', V, 'yesno', '-', 'm_bPlayOnGroupNotFound'], ['evidence', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the voice group set asset']] },
  'party-effects': { group: 'port', file: 'party-effects.json', by: 'tools/party_audio_pack.js', external: true, from: 'every particle system the party uses (Effects/NPC/n_ex00x, the weapons, notifies, magic sets, spell actors, team-attack link pawns) and the n_ex001 spell actor classes', cols: [
    ['id', K, 'str', '-', 'particle system name, or spell actor class (*_C)'], ['asset', V, 'str', '-', '/Game path of the system or blueprint'],
    ['used_by', V, 'str', '-', 'who references it (first six), or the actor class roles'], ['uses', V, 'int', '-', 'number of users / roles'], ['evidence', V, 'str', '-', '-']] },
  // (party stream B) Donald and Goofy's simulation (docs/party-port-plan.md stream B; crates/kh-party includes build/gen/rust/party_data.rs)
  'party-params': { group: 'port', file: 'party-params.json', by: 'tools/party_sheet.js', external: true, from: 'n_ex00x base rows and pawn class defaults, the BT action blueprints over their native constructors, FriendDamageCommonParam over the TresDamageCommonParam constructor, the spell projectiles, UTresNpcState_Die', cols: [
    ['id', K, 'str', '-', '<chara>.<group>.<parameter> | act.<chara>.<asset>.<field> | damage.* | revive.* | proj.*'], ['value', V, 'json', '-', 'number in KH3 units; unchecked = not determined'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'asset path and property, or constructor / code address'],
    ['evidence', V, 'str', '-', 'how the value is used', true], ['ref_addr', V, 'str', '-', 'function that reads the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read in a class default object or data table; inferred = decompiled C'],
    ['note', V, 'str', '-', '-']] },
  'party-levels': { group: 'port', file: 'party-levels.json', by: 'tools/party_sheet.js', external: true, from: 'DataTable/Npc/LevelUp/n_ex001_LevelUpData and n_ex002_LevelUpData (TresChrLevelUpDataTable)', cols: [
    ['id', K, 'str', '-', '<chara>.<level>'], ['chara', V, 'str', '-', 'n_ex001 | n_ex002'], ['level', V, 'int', '-', '1..99'], ['exp', V, 'int', '-', 'm_Exp: total EXP for the level'],
    ['strength', V, 'int', '-', 'm_AttackPower'], ['magic', V, 'int', '-', 'm_MagicPower'], ['defense', V, 'int', '-', 'm_DefensePower'], ['ap', V, 'int', '-', 'm_AbilityPoint']] },
  'party-attacks': { group: 'port', file: 'party-attacks.json', by: 'tools/party_sheet.js', external: true, from: 'DataTable/Npc/Attack/n_ex001_AttackDataTable and n_ex002_AttackDataTable (TresAttackDataTable)', cols: [
    ['id', K, 'str', '-', '<chara>.<row>'], ['chara', V, 'str', '-', '-'], ['row', V, 'str', '-', 'row name'], ['power', V, 'num', '-', 'm_Power'], ['damage_kind', V, 'str', '-', 'm_DamageKind'], ['attribute', V, 'str', '-', 'm_DamageAttribute'],
    ['power_scale', V, 'num', '-', '-'], ['parabola_angle', V, 'num', '-', '-'], ['move_length', V, 'num', '-', '-'], ['brake', V, 'num', '-', '-'], ['knockback', V, 'str', '-', '-'], ['effect_time', V, 'num', '-', '-'],
    ['bad_status', V, 'str', '-', '-'], ['bad_rate', V, 'num', '-', '-'], ['bad_time', V, 'num', '-', '-'], ['team_check', V, 'str', '-', '-'], ['reaction_power', V, 'int', '-', '-'], ['magic', V, 'json', '-', 'm_bIsMagicAttack'], ['reflect_level', V, 'int', '-', '-']] },
  'party-states': { group: 'port', file: 'party-states.json', by: 'tools/party_sheet.js', external: true, from: 'constructors and state functions of the TresNpcState_* / TresAICharState_* classes (build/sora_tmp/party/decomp_index.tsv)', cols: [
    ['id', K, 'str', '-', 'kh_party::NpcStateId'], ['class', V, 'str', '-', 'the game class'], ['ctor', R, 'addr', 'code', 'constructor'], ['what', V, 'str', '-', 'the functions read and what the state does', true]] },
  'party-weapons': { group: 'port', file: 'party-weapons.json', by: 'tools/party_sheet.js', external: true, from: 'Load/Tres/TresEquipItemData rows I03021..I03034 (Donald) and I03041..I03054 (Goofy)', cols: [
    ['id', K, 'str', '-', 'item row'], ['key', V, 'str', '-', 'm_KeyName'], ['chara', V, 'str', '-', '-'], ['name', V, 'str', '-', 'm_Comment (Japanese)'], ['strength', V, 'int', '-', 'm_AttackPlus'], ['magic', V, 'int', '-', 'm_MagicPlus'],
    ['defense', V, 'int', '-', 'm_DefensePlus'], ['abilities', V, 'str', '-', 'm_AppendAbility, space separated'], ['model', V, 'str', '-', 'w_do0x0 / w_gu0x0']] },
  // (party stream D) the team attacks
  'party-team': { group: 'port', file: 'party-team.json', by: 'tools/party_team_pack.js', external: true, from: 'build/sora_tmp/party/team_attacks.json, Load/Common/TresCmdData, the state-event camera blueprints (BP_TresPlayerStateEvent_donald, _goofy_shoot, _ShieldGuardian), Camera/sora/Skill CameraAnims, Blueprints/FriendLink link pawns, p_ex001_AttackData FR_* rows', cols: [
    ['id', K, 'str', '-', 'team attack'], ['command', V, 'int', '-', 'ETresCommandKind'], ['sora_state', V, 'str', '-', 'TresPlayerState_* (state id) or the attack kind'], ['npc_state', V, 'str', '-', 'TresNpcState_* (kh_party fr_id)'],
    ['link_pawn', V, 'str', '-', 'FriendLink pawn / shot'], ['clips', V, 'str', '-', 'clips of Sora and the friends'], ['cameras', V, 'str', '-', 'CameraAnims / orbit cameras and their triggers'], ['rows', V, 'str', '-', 'attack-data rows'],
    ['ref_addr', R, 'addr', 'code', 'main function'], ['evidence', V, 'str', '-', 'what was read'], ['note', V, 'str', '-', 'departures']] },
  'party-abilities': { group: 'port', file: 'party-abilities.json', by: 'tools/party_sheet.js', external: true, from: 'Load/Tres/TresAbilityData: the rows Donald and Goofy equip', cols: [
    ['id', K, 'str', '-', 'm_KeyName'], ['ap', V, 'int', '-', 'm_UseAP'], ['mp', V, 'int', '-', 'm_UseMP (-1 = all MP)']] },
  // (prize) KH3's prize orbs: movement, life, attraction, values; the test level's drop tables; the Elden Ring host's one drop rule. Written by tools/sora_prize_sheet.js, generated into build/gen/rust/sora_prizes.rs (crates/kh-prize)
  'sora-prizes': { group: 'port', file: 'sora-prizes.json', by: 'tools/sora_prize_sheet.js', from: 'the prize actors and the prize manager of the game (docs/sora-prize-spec.md); rows test.* are choices of the test level and rows er.* choices of the Elden Ring host, not game data', external: true, cols: [
    ['id', K, 'str', '-', 'move.* / spawn.* / life.* / bounce.* / rest.* / magnet.* / get.* / kind.* / box.* / table.* = the game; test.* = the test level; er.* = the Elden Ring drop rule'], ['value', V, 'json', '-', 'a number or a list of numbers; null = unchecked'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'where the value was read, or who chose it'],
    ['evidence', V, 'str', '-', 'how the game uses it', true], ['ref_addr', V, 'str', '-', 'the function that reads it; "-": data or a choice'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read from data, an immediate, or an authored choice; inferred = decompiled C only'],
    ['note', V, 'str', '-', '-']] },
  // (boss finish) KH3's boss-finish presentation: slow motion, camera cuts, blur, white-out, sound. Written by tools/sora_bossfinish_sheet.js, generated into build/gen/rust/sora_bossfinish.rs (crates/kh-core combat/finish.rs)
  'sora-bossfinish': { group: 'port', file: 'sora-bossfinish.json', by: 'tools/sora_bossfinish_sheet.js', from: 'the boss maps\x27 level blueprints (128 packages call TresStartBossDefeatEffect; reference graph tt_01_camera_ms060), the camera shake asset, the sound configuration and the natives they call (docs/sora-bossfinish-spec.md); rows port.* and battle.* are choices of the port, not game data', external: true, cols: [
    ['id', K, 'str', '-', 'speed.* = the global game speed; cut.* = the three camera cuts; shake.* / blur.* / white.* / after.* / sound.* = the rest of the script; port.* / battle.* = the port'], ['value', V, 'json', '-', 'a number, a list of numbers or a word; null = unchecked'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'where the value was read, or who chose it'],
    ['evidence', V, 'str', '-', 'how the game uses it', true], ['ref_addr', V, 'str', '-', 'the function that reads it; "-": data or a choice'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read from data, an immediate, or an authored choice; inferred = decompiled C only; guess = not read'],
    ['note', V, 'str', '-', '-']] },
  // (loot) KH3's items a Sora can hold: battle and camp items, armour, accessories, synthesis materials, with the item id a prize box carries. Written by tools/sora_item_table.js; both hosts read the file itself (kh-host/src/items.rs)
  'sora-item-table': { group: 'port', file: 'sora-item-table.json', by: 'tools/sora_item_table.js', from: 'sheets/entities/Load/Tres/TresItemData.json (categories BATTLEITEM, CAMPITEM, PROTECTOR, ACCESSORY, MATERIAL) and the English string table kh3_item', external: true, cols: [
    ['id', K, 'str', '-', 'm_KeyName (ITEM_POTION, PRT_ITEM01, ACC_ITEM01, MAT_ITEM01 ...): for armour and accessories the id of the sora-equipment row'], ['item_id', V, 'int', '-', 'the item id: category x 256 + index of the row name I<cc><nnn> (0x101 Potion, 0x701 Blazing Shard); what a prize box holds'],
    ['kind', V, 'enum:battle|camp|armour|accessory|material', '-', 'ETresItemCategory'], ['grade', V, 'int', '-', 'm_ItemRank as a grade: 0 none, 1 RANK03 (Shard), 2 RANK02 (Stone), 3 RANK01 (Gem), 4 RANK00 (Crystal, the ores)'],
    ['name', V, 'str', '-', 'English name (m_NameTextID); "-" = no string'], ['family', V, 'str', '-', 'materials: the name without its grade word (Blazing, Mythril ...); "-" otherwise'],
    ['buy', V, 'int', '-', 'm_BuyPrice, munny'], ['sell', V, 'int', '-', 'm_SellPrice, munny'], ['max', V, 'int', '-', 'm_MaxNum: the stock maximum'],
    ['box', V, 'enum:heal|material|foodstuff|rare', '-', 'the prize box an item of this id comes in (item box spawn 0x141473110: by the high byte of the id)'], ['can_sell', V, 'int', '-', 'm_bCanSell'],
    ['row', V, 'str', '-', 'row name of TresItemData', true], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read from the data table'], ['note', V, 'str', '-', '-']] },
  // (loot) Elden Ring item -> KH3 item: the rules by category / goods type / rarity / effect, the pools, explicit per-id overrides, the lot-roll settings. AUTHORED and meant to be edited (docs/sora-er-items.md); read at run time by crates/kh-er/src/lootmap.rs
  'sora-er-items': { group: 'port', file: 'sora-er-items.json', by: 'authored (first version: build/sora_tmp/loot/make_er_items_sheet.js, one-shot)', from: 'choices of the Elden Ring host; Elden Ring param field names and ids from public documentation (marked guess until verified in the game); KH3 item keys from sheets/port/sora-item-table.json', external: true, cols: [
    ['id', K, 'str', '-', 'luck.* / lot.* / give.* / rarity.* / material.* / armour.* / nothing.* = settings; pool.* = KH item pools; rule.<nnn>.<name> = a mapping rule (lowest number first); id.<table>.<param id> = an explicit override'], ['value', V, 'json', '-', 'a number, a list, a rule object {cat, goods_type, rarity, effect, consumable, ids, give} or a give text'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it / where the field is documented'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": no game code was read for these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'guess = a host choice, or public documentation not yet verified in the game'],
    ['note', V, 'str', '-', '-']] },
  // (SHOP) the KH shop that stands in for Elden Ring's merchants' buy / sell windows: the area tier, the stock and prices, the sell rule, the cut of the kill munny, Elden Ring's online items, the smiths; generated by build/sora_tmp/party/SHOP/make_shop_sheet.js; read at run time by crates/kh-er (shopkh.rs), compiled into crates/kh-host/src/shop.rs as the fallback
  'sora-er-shop': { group: 'port', file: 'sora-er-shop.json', by: 'build/sora_tmp/party/SHOP/make_shop_sheet.js (host choices; the smiths and the talk numbers measured in the game\x27s own talk scripts)', from: 'choices of the Elden Ring host; Elden Ring\x27s own talk scripts and EquipParamGoods (read-only copies in build/sora_tmp/er/conv/game); KH3 item keys from sheets/port/sora-item-table.json (build/sora_tmp/party/IMPL_SHOP_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'shop.* / sell.* / price.* / munny.* / online.* / smith.* / talk.* = settings; stock.<KH item key> = a stock row'], ['value', V, 'json', '-', 'a number, a list or a stock row {from, base}'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it / where it was read'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": no game code was read for these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'guess = a host choice'],
    ['note', V, 'str', '-', '-']] },
  // (ER party S1) the Elden Ring party host's numbers: the breadcrumb nav, the cast budget, the ladder jump (LADDER2), the stuck / catch-up fallbacks; authored, read by crates/kh-er/src/party/glue.rs when the party host is made
  'party-er-host': { group: 'port', file: 'party-er-host.json', by: 'authored (ER party S1, build/sora_tmp/er/party/IMPL_S1_STATE.md)', from: 'choices of the Elden Ring party host, not data of either game: how the friends follow the leader\x27s trail, how many casts they may make, when KH3\x27s warp is the fallback (the user\x27s decision U12)', external: true, cols: [
    ['id', K, 'str', '-', 'nav.* = the breadcrumb trail and its routes; casts.* = the nav\x27s cast budget; ladder.* = the party\x27s ladder jump (LADDER2); stuck.* / catchup.* / warp.* = the fallbacks'], ['value', V, 'json', '-', 'a number'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": no game code reads these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice'],
    ['note', V, 'str', '-', '-']] },
  // (party K1) the friends' follow speed (KH3's FollowPlayer locomotion definition) and the AI locomotion's port rules (catch-up, speed-matched loop, blocked guard, idle turn); authored, read by crates/kh-party/src/locomotion.rs through build/gen/rust/party_follow.rs
  'party-follow': { group: 'port', file: 'party-follow.json', by: 'authored (party K1, build/sora_tmp/er/party/IMPL_K1_STATE.md)', from: 'follow.* = UTresNpcLocomotionDefinitionLand_FollowPlayer (ctor 0x141203230, speed rule 0x1412196C0, tick 0x141215A00; n_ex00x_Move1 has that native class and overrides none of them); k1.* = port design of this stream (the user: the AI must keep up, no foot sliding, no running in place, no turn without a turn clip)', external: true, cols: [
    ['id', K, 'str', '-', 'follow.* = KH3\x27s FollowPlayer numbers; k1.* = the port\x27s own rules'], ['value', V, 'json', '-', 'a number'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the constructor / literal it was read from, or PORT DESIGN'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', 'the function, "-" for a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read in the code or an authored port choice; inferred = a read whose meaning is inferred'],
    ['note', V, 'str', '-', '-']] },
  // (party X3D) Donald's playable kit (XD_PLAYABLE_KITS.md section 1, approved 2026-10-15): finishers, X moves, Aero Float, Grand Magic, Spell Volley, his own abilities; authored by build/sora_tmp/party/x3d/make_sheet.js, read by crates/kh-party/src/pad/donald*.rs through build/gen/rust/party_kit_donald.rs
  'party-kit-donald': { group: 'port', file: 'party-kit-donald.json', by: 'authored (party X3D, build/sora_tmp/party/IMPL_X3D_STATE.md)', from: 'the approved XD design (KH3 has no playable Donald: every rule is the port\x27s) over KH3 pieces: his clips (n_ex001 anim set), the unison actors (stream D6), the link pawns\x27 meteors / missiles (stream D), Sora\x27s shotlock numbers (sora-magic-shotlocks)', external: true, cols: [
    ['id', K, 'str', '-', 'feel.* / combo.* / fin.* finishers / hop.* block.* blink.* slide.* flutter.* land.* the X moves / float.* glide.* / grand.* / volley.* / ability.* and the abilities\x27 numbers'], ['value', V, 'json', '-', 'a number, or an attack row name'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the KH3 data it was read from, or PORT DESIGN (XD / X3D)'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', 'the function, "-" for a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read in the data or an authored port choice'],
    ['note', V, 'str', '-', '-']] },
  // (party X3G) Goofy's playable kit (XD_PLAYABLE_KITS.md section 2, approved 2026-10-15): the shield guard, Perfect Guard and the counter chain, Hop / Shield Surf / air guard / Shield Charge, the solo Fusion finishers and Bombardier Dive, the Tornado / Turbo / Discus tiers, Tornado Copter, Shield Guardian mode, Discus Barrage, his own abilities; authored by build/sora_tmp/party/x3g/make_sheet.js, read by crates/kh-party/src/pad/goofy*.rs through build/gen/rust/party_kit_goofy.rs
  'party-kit-goofy': { group: 'port', file: 'party-kit-goofy.json', by: 'authored (party X3G, build/sora_tmp/party/IMPL_X3G_STATE.md)', from: 'the approved XD design (KH3 has no playable Goofy: every rule is the port\x27s) over KH3 pieces: his clips (n_ex002 anim set), the Trinity Guard / Goofy Bombardier link pawns (stream D3), the Fusion Spin actor (stream D6b), Sora\x27s attack rows and shotlock numbers', external: true, cols: [
    ['id', K, 'str', '-', 'feel.* / guard.* perfect.* bash.* bash2.* heavy.* air_bash.* payback.* the shield / hop.* surf.* air_guard.* charge.* the X moves / fin.* dive.* finishers / tier.* tornado.* turbo.* discus.* the tiers / copter.* mode.* barrage.* / ability.* and the abilities\x27 numbers'], ['value', V, 'json', '-', 'a number, an attack row, an effect / sound asset or a name'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the KH3 data it was read from, or PORT DESIGN (XD / X3G)'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', 'the function, "-" for a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read in the data or an authored port choice'],
    ['note', V, 'str', '-', '-']] },
  // (GOOFY-BAL) the friend leaders' balance (the user 2026-10-16 23:50; build/sora_tmp/party/BAL_DESIGN.md), read by crates/kh-party/src/balance.rs
  'party-balance': { group: 'port', file: 'party-balance.json', by: 'authored (stream BAL, build/sora_tmp/party/IMPL_BAL_STATE.md)', from: "PORT DESIGN (KH3 has no playable Donald / Goofy): ratios of a friend leader's stats to Sora's, and the rule of Elden Ring's hits on him", external: true, cols: [
    ['id', K, 'str', '-', 'n_ex00x.lead.* the per-character ratios / lead.* the rules'], ['value', V, 'json', '-', 'a number or a bool'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', "PORT DESIGN (stream BAL) and the user's words"],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored port choice'],
    ['note', V, 'str', '-', '-']] },
  'party-balance-levels': { group: 'port', file: 'party-balance-levels.json', by: 'tools/party_balance_sheet.js', external: true, from: "sheets/port/sora-levels.json (Sora's attack / magic / defense per level) times the ratios of sheets/port/party-balance.json", cols: [
    ['id', K, 'str', '-', '<chara>.<level>'], ['chara', V, 'str', '-', 'n_ex001 | n_ex002'], ['level', V, 'int', '-', '1..99'],
    ['strength', V, 'int', '-', "the leader's Strength before his weapon"], ['magic', V, 'int', '-', "the leader's Magic before his weapon (0: no magic)"], ['defense', V, 'int', '-', "the leader's Defense before weapon and armour"],
    ['source', V, 'str', '-', 'how the row was made', true]] },
  // (GOOFY-GG) Goofy's GUARD gauge (the user 2026-10-16 23:40; build/sora_tmp/party/IMPL_GG_STATE.md), read by crates/kh-party/src/guard.rs and the HUD bar (kh-ui hud_guard.rs)
  'party-guard': { group: 'port', file: 'party-guard.json', by: 'build/sora_tmp/party/gg/tools/make_guard_sheets.js (once; the sheet is the source)', from: "PORT DESIGN (KH3 has no GUARD gauge): the meter's refill / break rules (the X3G shield gauge's numbers carried over) and the silver HUD bar's look", external: true, cols: [
    ['id', K, 'str', '-', 'guard.* the meter / hud.* the bar'], ['value', V, 'json', '-', 'a number, a bool, an RGBA array or a text'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', "PORT DESIGN (stream GG) and the user's words"],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored port choice'],
    ['note', V, 'str', '-', '-']] },
  'party-guard-levels': { group: 'port', file: 'party-guard-levels.json', by: 'build/sora_tmp/party/gg/tools/make_guard_sheets.js (once; stream BAL / the user retune the values)', from: 'PORT DESIGN (the user: GUARD "possible to increase via levelling up"): GUARD max of Goofy per level', external: true, cols: [
    ['id', K, 'str', '-', '<chara>.<level>'], ['chara', V, 'str', '-', 'n_ex002'], ['level', V, 'int', '-', '1..99'],
    ['guard_max', V, 'num', '-', 'GUARD max points at this level'], ['source', V, 'str', '-', 'how the row was made', true]] },
  // (DON1) the Donald overhaul's shared sheets (documentation/DONALD_OVERHAUL.md; build/sora_tmp/party/DON_PLAN.md), read by crates/kh-party/src/donald/ (compiled) and kh-partyhost donald.rs (again at run time)
  'party-don-core': { group: 'port', file: 'party-don-core.json', by: 'build/sora_tmp/party/don/make_sheets.js (once; the sheet is the source, the user retunes it)', from: "THE USER'S DESIGN (documentation/DONALD_OVERHAUL.md) and DON1's readings where it leaves a number open (ref_conf inferred)", external: true, cols: [
    ['id', K, 'str', '-', 'attune.* res.* arcane.* fight.* wet.* chill.* frozen.* big.* burn.* reveal.* exposed.* blind.* launch.* react.* echo.* cost.* color.*'], ['value', V, 'json', '-', 'a number'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the doc section, or PORT DESIGN (DON1)'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = the doc names the number; inferred = DON1 chose it'],
    ['note', V, 'str', '-', '-']] },
  'party-don-spells': { group: 'port', file: 'party-don-spells.json', by: 'build/sora_tmp/party/don/make_sheets.js (once; the sheet is the source, stream DON-MAGIC / the user retune the values)', from: "THE USER'S DESIGN (documentation/DONALD_OVERHAUL.md 4.3, 6): every castable spell and level, its MP prices and rules; effect numbers DON1's where the doc leaves them open", external: true, cols: [
    ['id', K, 'str', '-', 'l1.<element>.<tier> | l2.<element>.<level> | l3.<element> | hy.<a>_<b> | y.<line>.<level> | rel.<element>'], ['name', V, 'str', '-', 'the menu name'],
    ['element', V, 'str', '-', 'fire | blizzard | thunder | water | aero | light | none'], ['element2', V, 'str', '-', 'a hybrid\'s second element, else "-"'],
    ['line', V, 'enum:l1|l2|l3|hy|y|rel', '-', 'the spell line'], ['level', V, 'int', '-', 'tier / level'],
    ['cost_free', V, 'num', '-', 'MP unattuned (fixed, like Sora; Y rows: % of max MP; -1 = never)'], ['cost_att', V, 'num', '-', 'MP attuned (fixed; Y rows: % of max MP)'], ['cost_quick', V, 'num', '-', 'MP within 3 s of Resonance 3 (-1 = none)'],
    ['full_power', V, 'json', '-', 'true = needs full-power attunement'], ['once_per_period', V, 'json', '-', 'true = once per attunement period'], ['arcane', V, 'int', '-', 'Arcane Flow charges (Y)'],
    ['cast', V, 'num', '-', 's'], ['power', V, 'num', '-', 'per line: see the source'], ['extra', V, 'num', '-', 'per spell: see the source'],
    ['radius', V, 'num', '-', 'cm'], ['range', V, 'num', '-', 'cm'], ['angle', V, 'num', '-', 'deg'], ['push', V, 'num', '-', 'cm'], ['duration', V, 'num', '-', 's'], ['count', V, 'int', '-', 'orbs, rays, meteors, ticks ...'],
    ['applies', V, 'str', '-', 'statuses: burn | chill:N | frozen | wet | reveal | blind | launch | -'], ['target', V, 'str', '-', 'enemy | ally | self | party | area_self | area_target'],
    ['source', V, 'str', '-', 'the doc section and what the numbers mean', true]] },
  // (DON-UI) the look of Donald's HUD pieces and his menu's switches (documentation/DONALD_OVERHAUL.md 7), read by crates/kh-party/src/donald/hud.rs (compiled) and kh-partyhost custom.rs (DonLook)
  'party-don-hud': { group: 'port', file: 'party-don-hud.json', by: 'build/sora_tmp/party/DON-UI/tools/make_hud_sheet.js (once; the sheet is the source, the user retunes it)', from: "THE USER'S DESIGN (documentation/DONALD_OVERHAUL.md 6.1, 6.3, 6.5, 7, 9) placed on KH3's own HUD movies (PORT DESIGN, DON-UI)", external: true, cols: [
    ['id', K, 'str', '-', 'menu.* ring.* pip.* icon.* ult.* worn.* blink.* star.* zetta.* status.*'], ['value', V, 'json', '-', 'a number'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the doc section, or PORT DESIGN (DON-UI)'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = the doc names the rule; inferred = DON-UI chose the number'],
    ['note', V, 'str', '-', '-']] },
  // (DON-PROG) Donald's progression: ability AP / levels / test order, the aggro / aura / AI numbers (crates/kh-party/src/donald/ability_info.rs, compiled; kh-partyhost donald.rs, again at run time)
  'party-don-progression': { group: 'port', file: 'party-don-progression.json', by: 'build/sora_tmp/party/DON-PROG/make_prog_sheet.js (once; the sheet is the source, the user retunes it)', from: "THE USER'S DESIGN (documentation/DONALD_OVERHAUL.md 4.2, 5.4, 9, 10, 11) and DON-PROG's readings where it leaves a number open", external: true, cols: [
    ['id', K, 'str', '-', 'ability.<name> | aggro.* | aura.* | ai.* | kh3.*'], ['ap', V, 'int', '-', 'the AP of an ability (0 for the other rows)'],
    ['level', V, 'int', '-', 'the level he learns it at (0 = not by level)'], ['priority', V, 'int', '-', 'the loadout order of the test level (1 first, 0 = not by default)'],
    ['value', V, 'num', '-', 'the number of a rule (0 for the ability rows)'], ['unit', V, 'str', '-', '-'],
    ['source', V, 'str', '-', 'the doc section, or PORT DESIGN (DON-PROG)', true]] },
  // (DON-MAGIC) the spell-specific numbers of Donald's casts (crates/kh-party/src/donald/magic_fx.rs)
  'party-don-magic': { group: 'port', file: 'party-don-magic.json', by: 'build/sora_tmp/party/DON-MAGIC/make_magic_sheet.js (once; the sheet is the source, the user retunes it)', from: "THE USER'S DESIGN (documentation/DONALD_OVERHAUL.md 4.3, 4.5, 6) and DON-MAGIC's readings where it leaves a number open (ref_conf inferred); KH3 clip times", external: true, cols: [
    ['id', K, 'str', '-', 'cast.* boost.* size.* bolt.* hit.* l1.* flare.* freeze.* orbs.* flood.* barrier.* banish.* <ultimate>.* <hybrid>.* <arcane>.* rel.* steam.* conduct.* wildfire.* burn.* blind.*'], ['value', V, 'json', '-', 'a number'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the doc section, KH3, or PORT DESIGN (DON-MAGIC)'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = the doc names the number; inferred = DON-MAGIC chose it'],
    ['note', V, 'str', '-', '-']] },
  // (ZETTA) Zettaflare as the KH3 event kg702b shows it (crates/kh-party/src/donald/zettaflare.rs; build/sora_tmp/party/IMPL_ZETTA_STATE.md)
  'party-don-zetta': { group: 'port', file: 'party-don-zetta.json', by: 'build/sora_tmp/party/ZETTA/tools/make_zetta_sheets.js (from the KH3 cutscene data; the sheet is the source, the user retunes it)', from: "KH3 cutscene_kg01_kg702 matinee kg702b (Donald's Zettaflare) and ZETTA's port design where play differs from the event (ref_conf inferred)", external: true, cols: [
    ['id', K, 'str', '-', 'zetta.*'], ['value', V, 'json', '-', 'a number'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the KH3 event, or PORT DESIGN (ZETTA)'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": an event time or a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read from the event; inferred = ZETTA chose it'],
    ['note', V, 'str', '-', '-']] },
  'party-don-zetta-fx': { group: 'port', file: 'party-don-zetta-fx.json', by: 'build/sora_tmp/party/ZETTA/tools/make_zetta_sheets.js (read from the KH3 cutscene data)', from: "KH3 cutscene_kg01_kg702 matinee kg702b: Donald's clips, the effect attach tracks, the placed emitters with their trigger / move tracks, the SE stream and the voice track", external: true, cols: [
    ['id', K, 'str', '-', 'clip.* | fx.<frame>.<effect> | se.* | voice.*'], ['use', V, 'int', '-', '1 = played; 0 = a camera cheat of the event (kept for the record)'], ['ult', V, 'int', '-', '1 = also played by the ultimate casts (the circle, the aura, the clips, his effort; tinted to the element)'],
    ['kind', V, 'enum:clip|fx|se|voice|look|shot_scale|se_kill|se_spike', '-', '(ZETTA round 5c) se_kill: a sound played at its time on the target only when Zettaflare killed it (the silhouette breaking); se_spike: a sound the rock spikes play as they erupt and break (start = the most seconds it plays; scale = volume); shot_scale (round 5c): a per-shot size override (asset = the row id it scales, or a prefix ending in *; socket = the camera shot; scale = the factor, applied at the cut); look (round 5): the event look for kh_core::host::cinematic (offset = exposure_ev, key_ev, bloom_threshold; fade = the blend, s)'], ['start', V, 'num', '-', 'matinee s'], ['end', V, 'num', '-', 'matinee s; -1 = by itself'],
    ['asset', V, 'str', '-', 'clip sequence / effect name / sound asset path'], ['frame', V, 'enum:none|donald_socket|weapon|target_socket|world_donald|world_target|beam|beam_socket|beam_stream|end|end_beam|end_impact|end_glow|end_aura|beam_head', '-', '(ZETTA round 5c) end_aura: the aura on Donald (his body and wand, toward the camera, shrinking with the beam); beam_head: the launch head at the beam reach; (round 5, the ending) end: Donald feet at the ending, X along the beam line; end_beam: the ending beam start (his staff), X along it in 3D'],
    ['socket', V, 'str', '-', '-'], ['offset', V, 'json', '-', 'cm in the frame (X forward, Y right, Z up)'], ['rotation', V, 'json', '-', 'pitch, yaw, roll (deg) in the frame'],
    ['path', V, 'json', '-', 'move track keys (matinee s, x, y, z) in the frame'], ['end_mode', V, 'str', '-', 'loop | once | deactivate | fadeout | loopend | none'],
    ['fade', V, 'num', '-', 's'], ['scale', V, 'num', '-', '-'], ['source', V, 'str', '-', 'the matinee track it was read from', true]] },
  'party-don-zetta-cam': { group: 'port', file: 'party-don-zetta-cam.json', by: 'build/sora_tmp/party/ZETTA/tools/make_zetta_cam.js (read from the KH3 cutscene camera data)', from: "KH3 cutscene_kg01_kg702 TresMBCameraData_3 (the matinee kg702b's MB camera clips): Donald's shots 860 / 890 / 910 and the target shots 920 / 930 / 940 of the Zettaflare, the reaction shots left out; (round 5) the ending's 960 / 970 from cutscene_kg01_kg702b TresMBCameraData_1 (the matinee kg702c)", external: true, cols: [
    ['id', K, 'str', '-', '<shot>.<matinee s>'], ['shot', V, 'str', '-', 'shot860 | shot890 | shot910 | shot920 | shot930 | shot940 | shot960 | shot970 (the ending, kg702c)'], ['frame', V, 'enum:donald|target|beam|subject|end', '-', 'end (the ending: Donald feet at its start, X along the beam line), Donald (his feet, his facing), the target (its centre, X from Donald to it), the beam (the target centre, X from his staff to it in 3D) or subject (the target shots: around the target where it is, the beam axes, scaled to its size)'],
    ['part', V, 'enum:channel|cast|end', '-', 'channel: its time from the start of the channel shot; cast: matinee time of the cast part; end: the ending (kg702c), matinee time on the same clock'], ['time', V, 'num', '-', 'matinee s'],
    ['position', V, 'json', '-', 'cm in the frame (X forward, Y right, Z up)'], ['rotation', V, 'json', '-', 'pitch, yaw, roll (deg) in the frame'], ['fov', V, 'num', '-', 'horizontal deg'],
    ['source', V, 'str', '-', 'the camera clip it was read from', true]] },
  // (NF) KH3's near-camera / see-through fade of the characters (TresCameraManager's shut-out fade; crates/kh-camera/src/camfade.rs; build/sora_tmp/party/IMPL_NF_STATE.md)
  'sora-camera-fade': { group: 'port', file: 'sora-camera-fade.json', by: 'build/sora_tmp/party/NF/tools/make_camfade_sheets.js (once; the sheet is the source)', from: 'KH3 code (FUN_1405D87F0, FUN_1405C9660, FUN_1405EFBE0, FUN_1405FBFF0, TresCameraManager 0x1405E2970, actor defaults 0x143EA0D80) and the translated masked material shaders; port design for the leader and the limbs', external: true, cols: [
    ['id', K, 'str', '-', 'camfade.*'], ['value', V, 'json', '-', 'a number, a bool or a text'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the KH3 address or PORT DESIGN and whose words'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read in the code or an authored port choice'],
    ['note', V, 'str', '-', '-']] },
  'sora-camera-fade-volumes': { group: 'port', file: 'sora-camera-fade-volumes.json', by: 'build/sora_tmp/party/NF/tools/make_camfade_sheets.js (re-made from the blueprints per character id)', from: 'TresBodyCollPrimitive components with m_bEnableCamera of Blueprints/{Player,Npc}/<id>/<id>_PawnBase (their SCS nodes: socket, location, rotation)', external: true, cols: [
    ['id', K, 'str', '-', '<chara>.<component>'], ['chara', V, 'str', '-', 'p_ex001 / n_ex001 / n_ex002 / ...'], ['bone', V, 'str', '-', 'AttachToName on TresCharMesh0'],
    ['shape', V, 'enum:SPHERE|CAPSULE|BOX|CONVEX', '-', 'MyShapeType (class default SPHERE)'], ['radius', V, 'num', '-', 'MyShapeSize.X (cm)'],
    ['half_height', V, 'num', '-', 'CAPSULE: MyShapeSize.Z (cm, the radius included); 0 = sphere'], ['offset', V, 'json', '-', 'RelativeLocation (cm)'],
    ['rotation', V, 'json', '-', 'RelativeRotation (pitch, yaw, roll)'], ['function', V, 'enum:FADE|RESET', '-', 'm_OverlapCameraFunction (class default FADE)'],
    ['source', V, 'str', '-', 'the blueprint export', true]] },
  // (GOOFY-HR) the hit reactions of every playable character (crates/kh-party/src/hurt.rs; build/sora_tmp/party/IMPL_HR_STATE.md)
  'party-hurt': { group: 'port', file: 'party-hurt.json', by: 'stream HR (authored once; the sheet is the source)', from: 'KH3 code (Damage Setup 0x141365930, the exits rule 0x14135E188, the hit-effect chooser 0x140628960) and port design', external: true, cols: [
    ['id', K, 'str', '-', 'hurt.*'], ['value', V, 'json', '-', 'a number or a bool'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'the KH3 address or PORT DESIGN and whose words'],
    ['evidence', V, 'str', '-', 'what the row does', true], ['ref_addr', V, 'str', '-', '"-": a port rule'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read in the code or an authored port choice; inferred = chosen from the data'],
    ['note', V, 'str', '-', '-']] },
  // (ER party S8) the unlock scenes (Goofy vs Leonine Misbegotten, Sora vs Margit, Donald at the Limgrave Finger Reader), the character select, Donald's lines (OUR TEXT, for the user to review), the unlock celebration; written by build/sora_tmp/er/party/s8/make_sheet.js, read by crates/kh-er/src/unlock/
  'party-er-unlocks': { group: 'port', file: 'party-er-unlocks.json', by: 'build/sora_tmp/er/party/s8/make_sheet.js (authored; Elden Ring ids and places parsed from the install)', from: 'Elden Ring\x27s own map and event data (map/mapstudio/*.msb, event/*.emevd, msg TalkMsg / NpcName), read-only from the user\x27s install (copies in build/sora_tmp/er/party/s8/game); the words are OUR TEXT for the user to review; the rest are host choices (build/sora_tmp/er/party/IMPL_S8_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'scene.<chara> = an unlock scene (ids, flags, places); stage.* / puppet.* = the staged fight; anim.* / fx.* = the angry loop and the celebration; text.* / talk.* = the words (OUR TEXT)'], ['value', V, 'json', '-', 'a number, a list, a string or an object (the evidence names its fields)'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'where it was read, or who chose it'],
    ['evidence', V, 'str', '-', 'the parsed record or what the row does', true], ['ref_addr', V, 'str', '-', '"-": Elden Ring data, no KH3 code'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = parsed from the game\x27s data or an authored choice; inferred = chosen by measurement, to be seen in the game; guess = not read'],
    ['note', V, 'str', '-', '-']] },
  'sora-er-fx': { group: 'port', file: 'sora-er-fx.json', by: 'authored (Elden Ring sound and effects; build/sora_tmp/er/fx/make_fx_sheet.js)', from: 'choices of the Elden Ring host, not data of either game: which KH3 footstep surface an Elden Ring ground material sounds as, how loud Sora is, how bright his effects are (build/sora_tmp/er/IMPL_ER_FX_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'surface.er.<Elden Ring hit material> / surface.default = the footstep surface; sound.* = output; vfx.* = effect drawing'], ['value', V, 'json', '-', 'a number; null = unchecked (the default applies)'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'what the choice rests on', true], ['ref_addr', V, 'str', '-', '"-": no game code reads these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice; inferred = rests on community names'],
    ['note', V, 'str', '-', '-']] },
  // (ER77) KH3 Sora's voice events -> the user's own KH1 Sora clips while the KH1 outfit is worn. AUTHORED (first version: build/sora_tmp/er/er77/make_outfit_voice_sheet.js, one-shot); tools/sora_outfit_voice.js -> build/sora_pack/audio/outfits/<outfit>/voice.json, read by kh-audio (outfit.rs)
  'sora-outfit-voice': { group: 'port', file: 'sora-outfit-voice.json', by: 'authored (first version: build/sora_tmp/er/er77/make_outfit_voice_sheet.js, one-shot)', from: 'choices of the Elden Ring host and the user (2026-10-15): which KH1 clip stands for which KH3 voice event; KH3 voice events from the audio pack\x27s triggers (build/sora_pack/audio/manifest.json)', external: true, cols: [
    ['id', K, 'str', '-', 'outfit.<outfit> = settings; clip.<outfit>.<clip> = one clip file; voice.<outfit>.<event> = one KH3 voice event -> the clips'], ['value', V, 'json', '-', 'settings object | { file } | { outfit, clips: [clip ids], pick: random (no immediate repeat) | step (by combo step), kind: mapped | stand_in | user | silent (clips []), match: [keys: group:<voice group>:<unit>[#<element>] | anim:<logical animation> | asset:<KH3 voice asset name>] } (first match by kind of key: group, then anim, then asset)'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'which KH3 voice events the row covers', true], ['ref_addr', V, 'str', '-', '"-": no game code reads these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice; inferred = the partner of a team attack told from which partner\x27s anim set holds the animation'],
    ['note', V, 'str', '-', '-']] },
  // (boss rewards, IMPL_ER31) which Elden Ring boss gives Sora what (magic, abilities, slots, max HP / MP): AUTHORED and meant to be edited (docs/sora-er-boss-rewards.md); written by build/sora_tmp/er/er31/make_sheet.js, read at run time by crates/kh-host/src/bossrewards.rs
  'sora-er-boss-rewards': { group: 'port', file: 'sora-er-boss-rewards.json', by: 'build/sora_tmp/er/er31/make_sheet.js (authored)', from: 'choices of the Elden Ring host: the rows the user fixed (Margit -> Thunder, Godrick -> Cure, Maliketh -> Glide) and proposals; KH3 reward names from the victory-bonus table (sheets/entities/Load/Tres/TresVBonusData.json); Elden Ring defeat flags and chr ids from memory of public lists (guess until the log of the game verifies them)', external: true, cols: [
    ['id', K, 'str', '-', 'boss.<row id> = one boss (the row id is what the save records and what the key boss.grant takes); rule.slot_order = the slots of the Great Rune bosses by count; small.* = the general rule for every boss that is not a row (one item, tier caps by region, a stat boost every Nth)'], ['value', V, 'json', '-', '{ name, region, order, path: required | main | optional, flag: the defeat event flag or null, flag_conf: verified | guess, chr: the character model id (cNNNN) or null, chr_conf, area: the map area the chr id counts in or null, user: [rewards the user fixed], proposal: [rewards proposed] }; a reward is magic:<spell name of any tier> | ability:<game name> | hp:<3|5|10|15|30> | mp:<3|5|10|20> | slot:<item|armor|accessory|great_rune>'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'who chose the rewards'],
    ['evidence', V, 'str', '-', 'why this boss gives this, and how the row is applied', true], ['ref_addr', V, 'str', '-', '"-": no game code was read for these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'guess = an identifier from memory, not yet verified in the game; confirmed = an authored rule'],
    ['note', V, 'str', '-', 'where the identifiers come from']] },
  // (story events, IMPL_ER33) the events Elden Ring ties to resting at a site of grace (Melina's accord, her invitation, her talks) and the game's own conditions for each; written by build/sora_tmp/er/er33/make_story_sheet.js, read at run time (and compiled in as the defaults) by crates/kh-er/src/story.rs
  'sora-er-story': { group: 'port', file: 'sora-er-story.json', by: 'build/sora_tmp/er/er33/make_story_sheet.js (authored from the parsed scripts)', from: 'Elden Ring\x27s own event scripts (event/common.emevd, map emevd) and talk scripts (script/talk/m00_00_00_00.talkesdbnd), read from the user\x27s install and listed under build/sora_tmp/er/er33/txt (build/sora_tmp/er/IMPL_ER33_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'story.<event> = an event made by itself (kind auto); story.talk.<name> = a condition under which the save-point menu shows "Talk to Melina" (kind menu)'], ['value', V, 'json', '-', '{ kind: auto (made unasked: only the accord) | menu (an entry of the game\x27s grace menu, selected for the player) | self (she speaks by herself at a rest: offered in the save-point menu once a session, never made unasked; IMPL_ER36), method: B driven (talkdrive.rs: the game\x27s real rest under a black cover), done_any: flags that end it, need_all / need_none / need_any: event flags, rise_any: flags that must be seen turning on in the session, graces: the grace entity ids it can happen at ([] = any), graces_free: those of them without the count, count_flags + count_min: how many of these discovery flags must be on }'],
    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'where the conditions were read'],
    ['evidence', V, 'str', '-', 'the script event / talk state the row was parsed from, and what was measured', true], ['ref_addr', V, 'str', '-', '"-": Elden Ring script data, no KH3 code'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = parsed from the game\x27s script; inferred = partly public knowledge from memory'],
    ['note', V, 'str', '-', 'what was not read, and why the values are what they are']] },
  'sora-er-config': { group: 'port', file: 'sora-er-config.json', by: 'authored (Elden Ring: KH3 Config screen -> Elden Ring options; build/sora_tmp/er/er21/make_config_sheet.js)', from: 'choices of the Elden Ring host, not data of either game: which Elden Ring option (a field of its GameSettings) each row of KH3\x27s Config screen drives, and how the values map (build/sora_tmp/er/IMPL_ER21_STATE.md)', external: true, cols: [
    ['id', K, 'str', '-', 'config.<KH3 option> (kh_core::config::ConfigOption)'], ['value', V, 'json', '-', '{ er: the GameSettings field or "-", offset: its byte offset in GameSettings, state: live | unverified | kh-only }; only a live row is read and written'],
    ['unit', V, 'str', '-', 'the two value ranges'], ['source', V, 'str', '-', 'who chose it'],
    ['evidence', V, 'str', '-', 'what the mapping rests on', true], ['ref_addr', V, 'str', '-', '"-": no game code was read for these rows'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = an authored choice (kh-only rows); inferred = the field\x27s place is the SDK\x27s, its effect was not seen in the game'],
    ['note', V, 'str', '-', '-']] },
  // the material instances of Sora's mesh sections and of the Kingdom Key, and the camp menu's character render (tools/sora_material_pack.js)
  'sora-materials': { group: 'port', file: 'sora-materials.json', by: 'tools/sora_material_pack.js', from: 'the decoded packages of the material instances on the sections of p_ex001_0 and w_so010_0 (parent chain, parameters, the compiled shader map of the chain: uniform expressions, texture slots, base-pass and depth-only pixel shaders), the textures they name, and /Game/UI/Blueprints/CampTop/p_ex001_UIActor (the camp menu model: capture, lights, anim set)', external: true, cols: [
    ['id', K, 'str', '-', '<mesh>.<slot name> = the material instance of a mesh section; <mesh>.<slot>#<what> = a fact about it: #root, #owner (the package whose compiled shader map it runs), #shading_model, #blend_mode, #two_sided, #base_pass / #opacity_mask (translation state), #scalar:<name>, #vector:<name>, #texture<slot>:<parameter>, #default_scalar / #default_vector (read by the shader, not set by the chain); menu.<what> = the camp menu character render'],
    ['value', V, 'json', '-', 'asset path, number, "r g b a", enum or state text'], ['unit', V, 'str', '-', 'asset | scalar | rgba | enum | bool | state | px | deg | cm | scale'],
    ['source', V, 'str', '-', 'the decoded package the row was read from', true], ['evidence', V, 'str', '-', 'asset = read from the cooked data'],
    ['ref_addr', V, 'str', '-', '"-": no code address (asset data)'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read from the asset; guess = a texture slot that could not be resolved'],
    ['note', V, 'str', '-', 'who set the value in the chain, texture file / colour space / size in the pack, or what the row means']] },
  // Sora's weapon and visual effects (docs/sora-vfx-spec.md); all four are written by tools/sora_vfx_sheet.js from the research
  // row files kept under audit/sora_port/vfx (asset data mined from the decoded packages, code facts with their addresses)
  'sora-effects': { group: 'port', file: 'sora-effects.json', by: 'tools/sora_vfx_sheet.js', from: 'audit/sora_port/vfx/rows_effects.json: notify instances of the clips of p_ex001_AnimSet, attach-data assets (ead_*), effect tables of the pawn, weapon, projectile and load-asset blueprints, physical-material effect tables; effect-attach code of base/KH3.exe', external: true, cols: [
    ['id', K, 'str', '-', 'anim.<clip>.<notify>.<entry>, weapon.*, pawn.*, magic.*, physmat.*, ...'], ['category', V, 'str', '-', 'locomotion, keyblade_appear, melee, guard, magic, cure, shotlock, item, levelup, formchange, damage, abnormal, lowhp, death, prize, link_attraction, form_pointer, other'],
    ['trigger_kind', V, 'enum:anim_notify|anim_notify_state|component_table|load_asset|data_table|projectile|code|unknown', '-', 'what requests the effect'],
    ['trigger', V, 'str', '-', 'notify class, or property path of the table / asset that holds the effect, or the code path'], ['clip', V, 'str', '-', 'animation clip (file name under Character/pc/p_ex001/anm) for notify rows; "-" otherwise'],
    ['logical', V, 'list', '-', 'rows of p_ex001_AnimSet that play the clip; "-" otherwise'], ['time', V, 'num', '-', 'notify trigger time in the clip, seconds'], ['duration', V, 'num', '-', 'notify state duration, seconds; "-" for instant triggers'],
    ['effect', V, 'str', '-', 'object path of the ParticleSystem; "-" for a pure trigger without effect (Shot, ThrowWeapon, SetWeaponVisible, decal materials); unchecked = no effect found / not determined'],
    ['kk_base', V, 'enum:yes|no|conditional|unknown|na', '-', 'whether base-form Sora with the Kingdom Key reaches the row: yes; no (other keyblade / form: effect group or clip of that form); conditional (only under the effect group named in params.group_gate); unknown; na (entry without effect). Rule: docs/sora-vfx-spec.md 7.4'],
    ['attach', V, 'json', '-', '{mode, socket, end_socket, offset, rotation, scale, absolute, to_attach, detach_at_end} as far as the data says; "-" when the trigger has no placement data'],
    ['params', V, 'json', '-', 'the other properties of the entry: group_id, end_type, when_change_animation, fades, delays, play_number, colour, kk_scope, subcategory, group_gate (basis of kk_base)', true],
    ['source', R, 'str', 'asset-files', 'cooked package that holds the trigger'], ['evidence', V, 'str', '-', 'export and property the row was read from', true], ['ref_addr', R, 'addr', 'code', 'function that executes the trigger; "-" when not identified'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read from the named export / property; inferred = data chain joined by a rule read as decompiled C; guess = not determined'], ['note', V, 'str', '-', '-']] },
  'sora-weapon': { group: 'port', file: 'sora-weapon.json', by: 'tools/sora_vfx_sheet.js', from: 'audit/sora_port/vfx/rows_weapon.json: TresWeaponBase / TresEquipmentComponent code of base/KH3.exe, class default objects of Blueprints/Player/p_ex001/w_so010_Wep, w_so_KeybladeBase, w_soNNN_Equip, the weapon mesh, skeleton, Bonamik and anim set under Character/wep/w_so010, item tables', external: true, cols: [
    ['id', K, 'str', '-', '<group>.<parameter>'], ['kind', V, 'enum:param|socket|component|animation|keyblade|behaviour', '-', 'param = fixed value; socket / component / animation = asset facts of the Kingdom Key; behaviour = rule read in code; keyblade = pointer row of one keyblade (value = {name, blueprint, mesh, formchange variants})'], ['value', V, 'json', '-', 'number, boolean, name, asset path or object; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'cm, s, deg, asset, name, ...; "-" for a ratio, flag or object'], ['source', V, 'str', '-', 'where the value is stored: asset path and property, or instruction address'],
    ['evidence', V, 'str', '-', 'evidence mark ([I] instruction level, [C] decompiled C, [A] asset data, [U] stock UE 4.17) and how the value is used', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level or in a class default object / asset; inferred = decompiled C, stock engine behaviour or derived; guess = not determined (value unchecked)'],
    ['note', V, 'str', '-', '-']] },
  'sora-vfx-params': { group: 'port', file: 'sora-vfx-params.json', by: 'tools/sora_vfx_sheet.js', from: 'audit/sora_port/vfx/rows_fxsys.json: TresEffectAttachComponent, ParticleSystemComponent and pawn effect code of base/KH3.exe, attach-data assets of Sora and the Kingdom Key', external: true, cols: [
    ['id', K, 'str', '-', '<group>.<parameter>'], ['kind', V, 'enum:const|asset', '-', 'const = constant of the effect runtime read in code; asset = value of an effect asset'], ['value', V, 'json', '-', 'number, boolean, name, asset path or object; unchecked = not determined (never guessed)'],
    ['unit', V, 'str', '-', 'cm, s, deg, asset, name, ...; "-" for a ratio, flag or object'], ['source', V, 'str', '-', 'where the value is stored: asset path and property, or instruction address'],
    ['evidence', V, 'str', '-', 'evidence mark ([I] instruction level, [C] decompiled C, [A] asset data, [U] stock UE 4.17) and how the value is used', true], ['ref_addr', R, 'addr', 'code', 'function that reads or applies the value; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read at instruction level or in a class default object / asset; inferred = decompiled C, stock engine behaviour or derived; guess = not determined (value unchecked)'],
    ['note', V, 'str', '-', '-']] },
  'sora-vfx-modules': { group: 'port', file: 'sora-vfx-modules.json', by: 'tools/sora_vfx_sheet.js', from: 'audit/sora_port/vfx/rows_modules.json (census of build/sora_tmp/vfx/cascade/census.js over the 516 particle systems of build/sora_pack/vfx and over all 10,409 systems of Content/Effects) and sqex_modules.json (semantics of the Square Enix modules)', external: true, cols: [
    ['id', K, 'str', '-', 'class name of a particle module / type data / distribution / material expression, or feature:<name>=<value>'], ['origin', V, 'enum:ue4|sqex', '-', 'stock UE 4.17 class or Square Enix extension'],
    ['category', V, 'enum:module|typedata|distribution|required-feature|material-blend|material-expression|other', '-', '-'], ['sora_effects', V, 'int', '-', 'particle systems of the Sora set that use it'], ['sora_emitters', V, 'int', '-', 'emitters (LOD 0) that use it'],
    ['sora_instances', V, 'int', '-', 'instances in the Sora set'], ['game_instances', V, 'int', '-', 'instances in all particle systems of the game'],
    ['support', V, 'enum:exported|approximated|ignored', '-', 'what tools/sora_vfx_pack.js does: exported = every value in the neutral JSON with known semantics; approximated = values carried verbatim, behaviour not specified; ignored = editor data'],
    ['semantics_conf', V, 'str', '-', 'Square Enix classes: how the behaviour text was obtained (decompiled, decompiled-partial, reflection+name, reflection-only); "-" for stock classes'], ['note', V, 'str', '-', 'one-line meaning']] },
  // authored rows kept in tools/sora_input_layout_sheet.js: default pad and keyboard / mouse layout (build/sora_tmp/actions/p2_input/SPEC.md)
  'sora-input-layout': { group: 'port', file: 'sora-input-layout.json', by: 'tools/sora_input_layout_sheet.js', from: 'key assignments and key reads in the machine code of base/KH3.exe (pad table 0x1486DAA00, key table 0x1487D7790 and its defaults 0x140EDF630, key emulation registration 0x141074520, UI pad poll 0x14158E390, controller input update 0x1413F1AB0), Config/DefaultInput.ini', external: true, cols: [
    ['id', K, 'str', '-', 'game action'], ['fkey_addr', V, 'str', '-', 'address of the FKey the reading code uses: an EKeys static (fixed), one of the two game key variables, or a key-table slot'],
    ['pad_default', V, 'str', '-', 'default pad key (EKeys name, Xbox label) with i.GamePadDecideType = 0; "-" = no pad key'], ['key_default', V, 'str', '-', 'default keyboard / mouse main key; "-" = none'],
    ['key_sub', V, 'str', '-', 'default keyboard / mouse sub key; "-" = none (the slot holds the invalid key)'], ['semantics', V, 'str', '-', 'how the reading code uses the key: just-pressed, held, released, axis, timed; "-" = not a key; unchecked = not determined'],
    ['source', V, 'str', '-', 'where the default is assigned: static initialiser, settings function, key-table default, ini line', true], ['consumer', V, 'str', '-', 'what reads the key and what it starts; unchecked = not determined', true],
    ['ref_addr', R, 'addr', 'code', 'main consumer function; "-" when none'],
    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = key, default and semantics read at instruction level; inferred = key read at instruction level, consumer logic from decompiled C; guess = consumer not determined'],
    ['note', V, 'str', '-', '-']] },
};

const schemaRows = () => Object.entries(DEFS).flatMap(([kind, d]) => d.cols.map(([name, role, type, ref, note, cold]) => [`${kind}.${name}`, kind, name, role, type, ref, cold ? 'yes' : 'no', note]));
const indexRows = () => Object.entries(DEFS).map(([kind, d]) => [kind, d.group, `sheets/${d.group === 'meta' ? '' : d.group + '/'}${d.file}`, d.by, d.from, d.minRows ?? '-']);
const columnsOf = (kind) => DEFS[kind].cols.map((c) => c[0]);
const fileOf = (kind) => (DEFS[kind].group === 'meta' ? DEFS[kind].file : `${DEFS[kind].group}/${DEFS[kind].file}`);

module.exports = { DEFS, schemaRows, indexRows, columnsOf, fileOf };

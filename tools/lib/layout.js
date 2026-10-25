// Emission rules of the reflection layout headers (build/gen/include/generated/reflection/<Name>.h).
// One implementation, two callers: tools/reflection_gen.js feeds it from audit/reflection.json,
// tools/gen.js feeds it from the sheets; check.js compares both against the files on disk, so the
// sheets and the evidence cannot drift apart unnoticed.
//
// A record is { kind: 'class'|'struct', name, module, size, super, ref_addr, members: [...] } and a
// member is { ord, name, type, offset, flags, mask, array_dim, inner_type, target, target_size }:
//   type         UProperty class;  inner_type: class of the inner property (array element, enum underlying type)
//   mask         bit mask of a BoolProperty within the byte at `offset`
//   array_dim    UProperty::ArrayDim (C array members)
//   target_size  sizeof the struct a StructProperty holds
// Every member gets a static_assert(offsetof); a record whose total size is known gets trailing
// padding and a static_assert(sizeof), so a member that does not fit in its owner fails to compile.
const { GEN, REFLECT_BANNER: BANNER } = require('./genout'); // build/gen: generated output is disposable (doctrine D6)
const DIR = `${GEN}/reflection`;
const ident = (s) => { const x = String(s).replace(/[^A-Za-z0-9_]/g, '_'); return /^[0-9]/.test(x) ? '_' + x : x || '_'; };
const HX = (n) => '0x' + n.toString(16).toUpperCase();
const hx = (n) => '0x' + n.toString(16);

// Storage size and C type per property class (UE 4.17, x64). null size = depends on the property.
// The sizes are checked by the compiler through every generated layout: a wrong one makes a member
// overlap the next reflected member (counted as a size conflict) or overrun sizeof(owner).
const TYPES = {
  FloatProperty: [4, 'float'], IntProperty: [4, 'int32_t'], UInt32Property: [4, 'uint32_t'], ByteProperty: [1, 'uint8_t'], Int8Property: [1, 'int8_t'],
  Int16Property: [2, 'int16_t'], UInt16Property: [2, 'uint16_t'], Int64Property: [8, 'int64_t'], UInt64Property: [8, 'uint64_t'], DoubleProperty: [8, 'double'],
  NameProperty: [8, 'kh3_FName'], StrProperty: [16, 'kh3_FString'], ArrayProperty: [16, 'kh3_TArray'], ObjectProperty: [8, 'void *'], ClassProperty: [8, 'void *'],
  WeakObjectProperty: [8, 'kh3_FWeakObjectPtr'], InterfaceProperty: [16, 'kh3_FScriptInterface'], DelegateProperty: [16, 'kh3_FScriptDelegate'],
  MulticastDelegateProperty: [16, 'kh3_TArray'], TextProperty: [24, 'kh3_FText'],
  BoolProperty: [1, 'uint8_t'],                 // the byte that holds the bit; the mask is in the member comment
  MapProperty: [80, 'kh3_TMap'], SetProperty: [80, 'kh3_TSet'],
  LazyObjectProperty: [28, 'kh3_FLazyObjectPtr'], AssetObjectProperty: [32, 'kh3_FAssetPtr'], AssetClassProperty: [32, 'kh3_FAssetPtr'],
};
const TYPES_H = BANNER + `#ifndef KH3_REFLECTION_TYPES_H
#define KH3_REFLECTION_TYPES_H
#include <stdint.h>
#include <stddef.h>
// Engine value types as stored in memory (UE 4.17, x64). Sizes are checked below and, indirectly,
// by every generated layout: a wrong size would make a member overlap the next reflected one.
#pragma pack(push, 1)
typedef struct { int32_t comparison_index; int32_t number; } kh3_FName;
typedef struct { void *data; int32_t num; int32_t max; } kh3_TArray;
typedef kh3_TArray kh3_FString;
typedef struct { int32_t object_index; int32_t serial_number; } kh3_FWeakObjectPtr;
typedef struct { void *object; void *interface_pointer; } kh3_FScriptInterface;
typedef struct { kh3_FWeakObjectPtr object; kh3_FName function_name; } kh3_FScriptDelegate;
typedef struct { void *data; void *reference_controller; uint32_t flags; uint32_t pad_0x14; } kh3_FText;
typedef struct { uint8_t bytes[80]; } kh3_TSet;   // TSparseArray (56) + hash (16) + hash size (8)
typedef struct { uint8_t bytes[80]; } kh3_TMap;   // a TSet of pairs
typedef struct { kh3_FWeakObjectPtr weak_ptr; int32_t tag_at_last_test; uint32_t guid[4]; } kh3_FLazyObjectPtr;
typedef struct { kh3_FWeakObjectPtr weak_ptr; int32_t tag_at_last_test; uint32_t pad_0xC; kh3_FString asset_long_pathname; } kh3_FAssetPtr;
#pragma pack(pop)
_Static_assert(sizeof(kh3_FName) == 8, "FName");
_Static_assert(sizeof(kh3_TArray) == 16, "TArray");
_Static_assert(sizeof(kh3_FWeakObjectPtr) == 8, "FWeakObjectPtr");
_Static_assert(sizeof(kh3_FScriptInterface) == 16, "FScriptInterface");
_Static_assert(sizeof(kh3_FScriptDelegate) == 16, "FScriptDelegate");
_Static_assert(sizeof(kh3_FText) == 24, "FText");
_Static_assert(sizeof(kh3_TSet) == 80, "TSet");
_Static_assert(sizeof(kh3_TMap) == 80, "TMap");
_Static_assert(sizeof(kh3_FLazyObjectPtr) == 28, "FLazyObjectPtr");
_Static_assert(sizeof(kh3_FAssetPtr) == 32, "FAssetPtr");
#endif
`;

// Size and C type of one element of a member: [size|null, ctype|null]
function elementOf(m, types) {
  if (m.type === 'EnumProperty') return (m.inner_type && types[m.inner_type]) || [null, null];
  if (m.type === 'StructProperty') return [typeof m.target_size === 'number' ? m.target_size : null, null];
  return types[m.type] || [null, null];
}

// records in emission order -> { files: { path: text }, stats, conflicts }
function emit(records, types = TYPES) {
  const out = {};
  const stats = { layouts: 0, members: 0, typed_members: 0, opaque_members: 0, size_conflicts: 0, shared_offsets: 0, bool_members: 0, members_without_offset: 0, sized_layouts: 0, members_beyond_size: 0, members_below_super_size: 0, name_collisions: 0 };
  const conflicts = [], beyond = [], belowSuper = [];
  const used = new Map();
  for (const r of records) {
    const direct = r.members;
    if (!direct.length || !r.name) continue;
    const fields = direct.filter((o) => typeof o.offset === 'number').sort((a, b) => a.offset - b.offset || a.ord - b.ord);
    const loose = direct.filter((o) => typeof o.offset !== 'number');
    stats.members_without_offset += loose.length;
    let cname = ident(r.name);
    if (used.has(cname)) { stats.name_collisions++; cname += '_' + String(r.ref_addr).slice(2); }
    used.set(cname, 1);
    const sized = typeof r.size === 'number';
    const L = [BANNER.trimEnd(), `// ${r.kind} ${r.name}${r.module ? '  (' + r.module + ')' : ''}${r.super && r.super !== '-' ? ' : ' + r.super : ''}`,
      `// Offsets and names are the constants the game's own registration function ${r.ref_addr} passes to the engine.`,
      sized ? `// sizeof(${r.name}) = ${HX(r.size)} per its registration. Members the engine does not reflect are not listed; the gaps are padding here.` : '// Members the engine does not reflect are not listed; the gaps are padding here. Total size is not recorded.',
      `#ifndef KH3_REFLECTION_${cname.toUpperCase()}_H`, `#define KH3_REFLECTION_${cname.toUpperCase()}_H`, '#include "../reflection_types.h"', '', '#pragma pack(push, 1)', `typedef struct ${cname} {`];
    const asserts = [];
    let cur = 0, lastOffset = -1;
    const seen = new Map();
    for (let i = 0; i < fields.length; i++) {
      const o = fields[i];
      const next = fields.slice(i + 1).find((x) => x.offset > o.offset);
      const gap = next ? next.offset - o.offset : sized && r.size > o.offset ? r.size - o.offset : null;
      const dim = typeof o.array_dim === 'number' && o.array_dim > 1 ? o.array_dim : 1;
      let [size, ctype] = elementOf(o, types);
      let fname = ident(o.name ?? `unk_${hx(o.offset)}`);
      if (seen.has(fname)) fname += `_${hx(o.offset)}`;
      seen.set(fname, 1);
      const note = `${o.type}${o.type === 'ArrayProperty' && o.inner_type ? ' of ' + o.inner_type : ''}${o.target && o.target !== '-' && !/Delegate/.test(o.type) ? ' ' + o.target : ''}${typeof o.mask === 'number' ? ' mask ' + HX(o.mask) : ''}, flags ${o.flags}`;
      if (typeof r.super_size === 'number' && o.offset < r.super_size) { stats.members_below_super_size++; if (belowSuper.length < 40) belowSuper.push({ struct: r.name, member: o.name, offset: o.offset, super: r.super, super_size: r.super_size }); }
      if (sized && o.offset >= r.size) { // a member that starts outside its owner: reported, never hidden
        stats.members_beyond_size++; if (beyond.length < 40) beyond.push({ struct: r.name, member: o.name, offset: o.offset, size: r.size });
        L.push(`    /* ${hx(o.offset)} lies beyond sizeof: ${fname} (${note}) */`);
        continue;
      }
      if (o.offset === lastOffset) { L.push(`    /* ${hx(o.offset)} also: ${fname} (${note}) */`); stats.shared_offsets++; if (o.type === 'BoolProperty') stats.bool_members++; continue; }
      if (o.offset < cur) { // previous member's assumed size runs into this one: a size in TYPES is wrong for this case
        stats.size_conflicts++;
        if (conflicts.length < 40) conflicts.push({ struct: r.name, member: o.name, offset: o.offset, previous_end: cur });
        L.push(`    /* ${hx(o.offset)} overlaps the previous member: ${fname} (${note}) */`);
        continue;
      }
      if (o.offset > cur) L.push(`    uint8_t pad_${HX(cur)}[${HX(o.offset - cur)}];`);
      stats.members++;
      if (o.type === 'BoolProperty') stats.bool_members++;
      const total = size === null ? null : size * dim;
      if (total !== null && (gap === null || total <= gap)) {
        if (ctype === null) L.push(`    uint8_t ${fname}[${HX(total)}]; // ${hx(o.offset)} ${note}${dim > 1 ? `; ${dim} elements of ${HX(size)} bytes` : ''}`);
        else L.push(`    ${ctype}${ctype.endsWith('*') ? '' : ' '}${fname}${dim > 1 ? `[${dim}]` : ''}; // ${hx(o.offset)} ${note}`);
        cur = o.offset + total; stats.typed_members++;
      } else {
        if (total !== null) { stats.size_conflicts++; if (conflicts.length < 40) conflicts.push({ struct: r.name, member: o.name, type: o.type, assumed_size: total, gap }); }
        const n = gap ?? 1;
        L.push(`    uint8_t ${fname}[${HX(n)}]; // ${hx(o.offset)} ${note}; ${gap === null ? 'size not known, last reflected member' : next ? 'extent to the next reflected member' : 'extent to the end of the owner'}`);
        cur = o.offset + n; stats.opaque_members++;
      }
      asserts.push(`_Static_assert(offsetof(${cname}, ${fname}) == ${HX(o.offset)}, "${cname}.${fname}");`);
      lastOffset = o.offset;
    }
    for (const m of loose) L.push(`    /* ${m.type} ${ident(m.name ?? 'unnamed')}: offset not recovered */`);
    if (sized && cur < r.size) L.push(`    uint8_t pad_${HX(cur)}[${HX(r.size - cur)}];`);
    L.push(`} ${cname};`, '#pragma pack(pop)', '', ...asserts);
    if (sized && cur <= r.size) { L.push(`_Static_assert(sizeof(${cname}) == ${HX(r.size)}, "sizeof ${cname}");`); stats.sized_layouts++; }
    L.push('', '#endif', '');
    if (!asserts.length) continue;
    out[`${DIR}/${cname}.h`] = L.join('\n');
    stats.layouts++;
  }
  const names = Object.keys(out).map((p) => p.slice(DIR.length + 1)).sort();
  out[`${GEN}/reflection_types.h`] = TYPES_H;
  out[`${GEN}/reflection.h`] = BANNER + '#ifndef KH3_REFLECTION_H\n#define KH3_REFLECTION_H\n' + names.map((n) => `#include "reflection/${n}"`).join('\n') + '\n#endif\n';
  return { files: out, stats, conflicts, beyond, belowSuper };
}

module.exports = { BANNER, DIR, TYPES, TYPES_H, ident, emit };

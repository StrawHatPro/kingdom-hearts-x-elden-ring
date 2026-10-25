// From audit/reflection.json generate:
//   build/gen/include/generated/reflection/<Name>.h   one layout struct per reflected class/struct:
//        members at the offsets the game's registration code passes to the engine (bool members at
//        the byte the engine's bitfield probe finds, with their mask), explicit padding everywhere
//        else, a static_assert on offsetof for every member and on sizeof for the whole
//   build/gen/include/generated/reflection_types.h    the engine value types the members use
//   audit/script_functions.json                 script-visible functions with their parameters
//   audit/reflection_layouts.json               counts and consistency figures
//   node tools/reflection_gen.js [--check]      --check: write nothing, exit 1 if stale
// The emission rules are in tools/lib/layout.js (shared with tools/gen.js, which feeds them from
// the sheets instead).
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ROOT, logRun, writeJson } = require('./lib/log');
const layout = require('./lib/layout');

const VERSION = '2.0.0';
const CHECK = process.argv.includes('--check');
const recs = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'reflection.json'), 'utf8'));
const sha256 = (s) => crypto.createHash('sha256').update(s).digest('hex');
const isProp = (o) => /Property$/.test(o.object_class || '');

const classSize = new Map(recs.filter((r) => r.kind === 'class').map((r) => [r.name, r.owner.size]));
const structSize = new Map(recs.filter((r) => r.kind === 'struct').map((r) => [r.name, r.struct.size]));
const functions = [];
const records = [];
let script_functions = 0, parameters = 0;
for (const r of recs) {
  const objs = r.objects;
  const top = r.kind === 'class' ? null : objs[r.top];
  if (r.kind === 'function' || r.kind === 'delegatefunction') {
    const params = objs.filter((o) => isProp(o) && o.outer === top.id).map((o) => {
      const f = o.property_flags ? BigInt(o.property_flags) : 0n;
      return { name: o.name, type: o.object_class, offset: o.offset ?? null, flags: o.property_flags || null, out: !!(f & 0x100n), return: !!(f & 0x400n),
        ...(o.mask !== undefined ? { mask: o.mask } : {}), ...(o.target && o.target !== '-' ? { target: o.target.name } : {}) };
    }).sort((a, b) => (a.offset ?? 1e9) - (b.offset ?? 1e9));
    functions.push({ class: r.owner.name || null, package: r.owner.package || null, name: r.name, kind: r.kind,
      function_flags: r.function.flags, params_size: r.function.params_size, params, registration_function: r.registration_function });
    script_functions++; parameters += params.length;
    continue;
  }
  if (r.kind !== 'class' && r.kind !== 'struct') continue;
  // members: properties attached directly to the struct object, or to the class
  const inner = (o) => objs.find((x) => x.outer === o.id);
  const members = objs.filter((o) => isProp(o) && (top ? o.outer === top.id : o.outer === undefined)).map((o) => {
    const t = o.target && o.target !== '-' ? o.target : null;
    const i = inner(o);
    return { ord: o.id, name: o.name, type: o.object_class, offset: o.offset ?? null, flags: o.property_flags, mask: o.mask ?? null, array_dim: o.array_dim ?? 1,
      inner_type: i ? i.object_class : null, target: t ? t.name : null, target_size: t && t.kind === 'struct' ? structSize.get(t.name) ?? null : null };
  });
  const sup = r.kind === 'class' ? r.owner.super : r.struct.super;
  records.push({ kind: r.kind, name: r.name, module: r.owner.package || null, size: r.kind === 'class' ? r.owner.size : r.struct.size, super: sup,
    super_size: sup && sup !== '-' ? (r.kind === 'class' ? classSize : structSize).get(sup) ?? null : null, ref_addr: r.registration_function, members });
}
const { files: out, stats: ls, conflicts, beyond, belowSuper } = layout.emit(records);
const stats = { records: recs.length, ...ls, script_functions, parameters };

const fnText = '[\n' + functions.map((f) => JSON.stringify(f)).join(',\n') + '\n]\n';
if (CHECK) {
  const stale = Object.entries(out).filter(([p, s]) => !fs.existsSync(path.join(ROOT, p)) || fs.readFileSync(path.join(ROOT, p), 'utf8') !== s).map(([p]) => p);
  if (stale.length) { console.log(`reflection headers are out of date: ${stale.length} stale`, stale.slice(0, 5)); process.exit(1); }
  console.log(`reflection headers are current (${Object.keys(out).length} files)`);
} else {
  fs.rmSync(path.join(ROOT, layout.DIR), { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
  for (const [p, s] of Object.entries(out)) { fs.mkdirSync(path.dirname(path.join(ROOT, p)), { recursive: true }); fs.writeFileSync(path.join(ROOT, p), s); }
  fs.writeFileSync(path.join(ROOT, 'audit', 'script_functions.json'), fnText);
  const OUT = path.join(ROOT, 'audit', 'reflection_layouts.json');
  writeJson(OUT, { generated_by: 'tools/reflection_gen.js', ...stats, example_size_conflicts: conflicts, members_beyond_size_examples: beyond, members_below_super_size_examples: belowSuper,
    headers_sha256: sha256(Object.keys(out).sort().map((p) => sha256(out[p])).join('')) });
  logRun({ tool: 'reflection_gen', version: VERSION, outputs: [OUT, path.join(ROOT, 'audit', 'script_functions.json')] });
  console.log(JSON.stringify(stats, null, 1));
  console.log('conflicts:', JSON.stringify(conflicts.slice(0, 8)));
  console.log('beyond size:', JSON.stringify(beyond.slice(0, 8)));
  console.log('below super size:', JSON.stringify(belowSuper.slice(0, 8)));
}

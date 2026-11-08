// (SPIRITSPRING) EVERY SPIRITSPRING OF ELDEN RING (base game + DLC), read from the game's own map data: the MSB regions of
// type 46 `MountJump` (their Japanese names "騎乗大ジャンプ" = "mounted big jump"), with the height their type data gives
// (f32 JumpHeight, metres) and the type 52 `MountJumpFall` region over each. No row is written by hand.
//
// How Elden Ring uses them (READ, build/sora_tmp/spiritspring/STATE.md): Torrent's behaviour script c0000.hks asks env(366)
// (the MountJump height the body stands in, cm) when the ride's jump is pressed, sets Ride_HighJump_Height = h + 700 and
// rises (RideJumpHighLoop) until it is that far above the take-off. So the launch height is JumpHeight x 100 + 700 cm.
// No region is switched by an event (activation part -1 in all of them, no EMEVD names one): live while the map is loaded.
//
// Input: the read-only 2.7.2.0 MSB copies in build/sora_tmp/weapons/P9a/game/map/mapstudio (P9a's extraction).
// Output: sheets/port/kh-er-spiritsprings.json (read by crates/kh-er/src/spiritspring.rs) and
//         build/sora_tmp/spiritspring/inventory.csv.
// usage: node tools/kh_spiritsprings.js [--check]
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.join(__dirname, '..');
const T = path.join(ROOT, 'build/sora_tmp');
const MSB = require(path.join(T, 'weapons/P9a/tools/msblib.js'));
const MSB_DIR = path.join(T, 'weapons/P9a/game/map/mapstudio');
const OUT = path.join(ROOT, 'sheets/port/kh-er-spiritsprings.json');
const CSV = path.join(T, 'spiritspring/inventory.csv');
const CHECK = process.argv.includes('--check');

/** Torrent's rise past the spring's own height (c0000.hks: `Ride_HighJump_Height = env(366) + 700`), cm. */
const TORRENT_EXTRA_CM = 700;
const MOUNT_JUMP = 46, MOUNT_JUMP_FALL = 52;
const SHAPES = { 0: 'point', 1: 'circle', 2: 'sphere', 3: 'cylinder', 4: 'rect', 5: 'box', 6: 'composite' };

/** The region's raw words the reader does not give: activation part (entity data + 0) and the type data (+0x58). */
function extra(file) {
  const b = fs.readFileSync(file);
  const str = (o) => { let e = o; while (b.readUInt16LE(e)) e += 2; return b.slice(o, e).toString('utf16le'); };
  const lists = {};
  let o = 16;
  while (o) {
    const cnt = b.readInt32LE(o + 4);
    const name = str(Number(b.readBigInt64LE(o + 8)));
    const offs = [];
    for (let k = 0; k < cnt - 1; k++) offs.push(Number(b.readBigInt64LE(o + 16 + k * 8)));
    lists[name] = offs;
    o = Number(b.readBigInt64LE(o + 16 + (cnt - 1) * 8));
  }
  return (lists.POINT_PARAM_ST || []).map((p) => {
    const ed = Number(b.readBigInt64LE(p + 0x50)), td = Number(b.readBigInt64LE(p + 0x58));
    return { activation_part: ed ? b.readInt32LE(p + ed) : -1, t0: td ? b.readFloatLE(p + td) : null, t4: td ? b.readInt32LE(p + td + 4) : null };
  });
}

const round = (x) => Math.round(x * 1000) / 1000;
const files = fs.readdirSync(MSB_DIR).filter((f) => f.endsWith('.msb')).sort();
const found = [];
for (const f of files) {
  const map = f.slice(0, -4);
  const m = MSB.read(path.join(MSB_DIR, f));
  if (!m.regions.some((r) => r.type === MOUNT_JUMP)) continue;
  const x = extra(path.join(MSB_DIR, f));
  const falls = m.regions.filter((r) => r.type === MOUNT_JUMP_FALL);
  for (const r of m.regions.filter((q) => q.type === MOUNT_JUMP)) {
    const e = x[r.index];
    if (e.activation_part !== -1) throw new Error(`${map} region ${r.index}: an activation part (${e.activation_part}): this reader does not handle it`);
    // the fall region over it: the nearest of its map within 6 m horizontally
    const near = falls.map((q) => ({ q, d: Math.hypot(q.pos[0] - r.pos[0], q.pos[2] - r.pos[2]) })).filter((z) => z.d < 6).sort((a, b) => a.d - b.d)[0];
    found.push({
      map, maps: [map], index: r.index, entity: r.entity, msb_name: r.name, shape: SHAPES[r.shape] || String(r.shape), pos: r.pos, rot: r.rot, dims: r.dims,
      height_m: round(e.t0), launch_cm: Math.round(e.t0 * 100) + TORRENT_EXTRA_CM, unk_t04: e.t4,
      fall: near ? { index: near.q.index, entity: near.q.entity, msb_name: near.q.name, shape: SHAPES[near.q.shape] || String(near.q.shape), pos: near.q.pos, dims: near.q.dims } : null,
    });
  }
}
// a map's `_10` variant (the same tile) repeating a spring exactly: one row, both maps named
const rows = [];
const key = (s) => JSON.stringify([s.map.replace(/_\d\d$/, ''), s.entity, s.pos, s.dims, s.height_m]);
for (const s of found) {
  const same = rows.find((r) => key(r) === key(s));
  if (same) same.maps.push(s.map);
  else rows.push(s);
}
const id = (s) => `spring.${s.map}.${s.index}`;
const sheetRows = rows.map((s) => {
  const ev = `READ (2.7.2.0 copy): ${s.maps.join(' + ')} region #${s.index} "${s.msb_name}" type ${MOUNT_JUMP} MountJump, ${s.shape} at (${s.pos.join(', ')}) dims ${JSON.stringify(s.dims)}, type data JumpHeight ${s.height_m} m (unk ${s.unk_t04}), activation part -1; launch = ${s.height_m} x 100 + ${TORRENT_EXTRA_CM} cm (c0000.hks env(366) + 700)${s.fall ? `; fall region #${s.fall.index} "${s.fall.msb_name}" ${s.fall.shape} ${JSON.stringify(s.fall.dims)}` : ''}`;
  return [id(s), s, 'metres (MSB) / cm (launch)', 'map/mapstudio/*.msb, action/script/c0000.hks', ev, '-', 'confirmed', s.maps.length > 1 ? `also in ${s.maps.slice(1).join(', ')}` : '-'];
});
const text = '{"sheet":"kh-er-spiritsprings","group":"port","columns":["id","value","unit","source","evidence","ref_addr","ref_conf","note"],"rows":[\n' + sheetRows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n';
const csv = ['id,maps,region,entity,msb_name,shape,x,y,z,radius,height_m,launch_cm,unk_t04,fall_region,fall_dims'].concat(rows.map((s) => [id(s), s.maps.join(' '), s.index, s.entity, `"${s.msb_name}"`, s.shape, ...s.pos, s.dims[0], s.height_m, s.launch_cm, s.unk_t04, s.fall ? s.fall.index : '', s.fall ? `"${s.fall.dims.join(' ')}"` : ''].join(','))).join('\n') + '\n';
if (CHECK) {
  const was = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf8') : '';
  console.log(was === text ? 'kh-er-spiritsprings: up to date' : 'kh-er-spiritsprings: STALE (run node tools/kh_spiritsprings.js)');
  process.exit(was === text ? 0 : 1);
}
fs.writeFileSync(OUT, text);
fs.mkdirSync(path.dirname(CSV), { recursive: true });
fs.writeFileSync(CSV, csv);
const dlc = rows.filter((s) => s.map.startsWith('m61')).length;
console.log(`kh-er-spiritsprings: ${rows.length} springs (${rows.length - dlc} base game, ${dlc} DLC) from ${found.length} MountJump regions in ${new Set(found.map((s) => s.map)).size} maps; heights ${Math.min(...rows.map((s) => s.height_m))} .. ${Math.max(...rows.map((s) => s.height_m))} m; ${rows.filter((s) => s.fall).length} with a fall region`);

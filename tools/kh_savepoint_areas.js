// (INTERACT-MARK, the user 2026-11-05: "do option 1") THE SAVE-POINT AREAS THAT DIFFER FROM KH3'S, AS THE USER'S CHOICE:
// writes sheets/port/kh-savepoint-areas.json. One row today: the Roundtable Hold's Table of Lost Grace (m11_10_00_00,
// grace 11101950), whose grace stands on the floor INSIDE the round table, so KH3's reactor capsule (g_ex_SavePoint
// TresReactor0: radius 110, height 180, 40 cm under the root) barely reaches the table top among the broken swords. The
// user chose to raise that one save point's area to table level, reachable from the table's edge.
//
// MEASURED in the map data (read-only copies of the 2.7.2.0 archives, build/sora_tmp/weapons/P8a/game):
//   m11_10_00_00.msb: the grace asset AEG099_060_9000 (entity 11101950), the grace's character c1000_9000 (the place
//     savepoint.rs builds the save point at), the grace's player point (entity 11101950 - 970 = c0000_9000: where its
//     warp and rest put the player: the side he comes from);
//   m11_10_00_00.nvmhktbnd (P8a/navpolys.json, the walkable faces): the table top's height (the walkable ring 2.6 .. 3.4 m
//     around the grace), the table top's edge and the floor beside it on the line toward the player point.
// THE RULE (the user's option 1): the capsule keeps KH3's radius (110) and height (180); its BASE is the table top; its
// centre is on that line, between the edge and the grace, placed so that the overlap test (kh_host::savepoint, his
// capsule radius + 110) takes a leader standing on the table top at its edge and not one standing on the floor beside it,
// for every leader's KH3 capsule radius (Sora 60, Goofy 40, Donald 30; their world body keeps 32 cm, sora-er-host
// `collision.radius`, from the table's side). The recover capsule keeps KH3's relation to the reactor (the same centre).
// Usage: node tools/kh_savepoint_areas.js [--check]
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const P8A = path.join(ROOT, 'build/sora_tmp/weapons/P8a');
const OUT = path.join(ROOT, 'sheets/port/kh-savepoint-areas.json');
const M = require(path.join(ROOT, 'build/sora_tmp/weapons/P9a/tools/msblib.js'));
const G = require(path.join(ROOT, 'build/sora_tmp/party/SHOP/goods.js'));

const KH3 = { reactor: { radius_cm: 110, height_cm: 180, offset_cm: -40 }, recover: { radius_cm: 50, height_cm: 140, offset_cm: -40 } };
const LEADER_RADII_CM = { sora: 60, goofy: 40, donald: 30 }; // sora-movement capsule.radius, party-params n_ex002 / n_ex001
const BODY_M = 0.32; // sora-er-host collision.radius (cm / 100)

function roundtable() {
  const msb = M.read(path.join(P8A, 'game/map/mapstudio/m11_10_00_00.msb'));
  const polys = require(path.join(P8A, 'navpolys.json'));
  const ents = new Set(G.rows('BonfireWarpParam.param').map((r) => r.data.readUInt32LE(8)));
  const grace = msb.parts.find((p) => p.entity === 11101950 && ents.has(p.entity));
  const chr = msb.parts.find((p) => p.model === 'c1000' && Math.hypot(p.pos[0] - grace.pos[0], p.pos[2] - grace.pos[2]) < 0.5);
  const point = msb.parts.find((p) => p.entity === grace.entity - 970);
  const g = grace.pos;
  const heights = (x, z) => {
    const out = [];
    for (const p of polys) {
      let inside = false;
      for (let i = 0, j = p.length - 1; i < p.length; j = i++) {
        const [xi, , zi] = p[i], [xj, , zj] = p[j];
        if ((zi > z) !== (zj > z) && x < ((xj - xi) * (z - zi)) / (zj - zi) + xi) inside = !inside;
      }
      if (!inside) continue;
      const [a, b, c] = p;
      const d = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
      const l1 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / d, l2 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / d;
      out.push(l1 * a[1] + l2 * b[1] + (1 - l1 - l2) * c[1]);
    }
    return out;
  };
  const top = [];
  for (let a = 0; a < 360; a += 5) for (let r = 2.6; r <= 3.4001; r += 0.2) top.push(...heights(g[0] + r * Math.cos(a * Math.PI / 180), g[2] + r * Math.sin(a * Math.PI / 180)).filter((y) => y > g[1] + 0.5));
  top.sort((a, b) => a - b);
  const tableTop = top[top.length >> 1];
  const dist = Math.hypot(point.pos[0] - g[0], point.pos[2] - g[2]);
  const dir = [(point.pos[0] - g[0]) / dist, (point.pos[2] - g[2]) / dist];
  let edge = 0, floorAt = null, floorY = null;
  for (let r = 2.0; r <= 8.0; r += 0.01) {
    const h = heights(g[0] + r * dir[0], g[2] + r * dir[1]);
    if (h.some((y) => Math.abs(y - tableTop) < 0.15)) edge = r;
    const f = h.find((y) => y < tableTop - 0.5);
    if (f !== undefined && floorAt === null && r > edge) { floorAt = r; floorY = f; }
  }
  // the centre's interval: a leader on the table top with his body against nothing at the edge (centre edge - body) is in
  // for the SMALLEST capsule; one on the floor against the table's side (centre edge + body) is out for the LARGEST
  const rMin = Math.min(...Object.values(LEADER_RADII_CM)) / 100, rMax = Math.max(...Object.values(LEADER_RADII_CM)) / 100;
  const reach = KH3.reactor.radius_cm / 100;
  const lo = edge - BODY_M - reach - rMin, hi = edge + BODY_M - reach - rMax;
  const along = Math.round(((lo + hi) / 2) * 100) / 100;
  const centre = [g[0] + dir[0] * along, g[2] + dir[1] * along];
  const offset = [centre[0] - chr.pos[0], centre[1] - chr.pos[2]];
  // (the user's correction 2026-11-05: "you should be able to access round table hold save point from the floor next to the
  // table") THE TABLE'S EDGE ALL AROUND, from the grace character (the save point's own centre), every 5 degrees of KH3's yaw
  // (KH3 x = Elden Ring z, KH3 y = Elden Ring x: the direction (er x, er z) = (sin a, cos a)): the last point on the table
  // top before the floor, on the walkable navmesh
  const ring = [];
  for (let a = 0; a < 360; a += 5) {
    const s = Math.sin(a * Math.PI / 180), c = Math.cos(a * Math.PI / 180);
    let e = 0;
    for (let rr = 2.0; rr <= 8.0; rr += 0.01) {
      const h = heights(chr.pos[0] + rr * s, chr.pos[2] + rr * c);
      if (h.some((y) => Math.abs(y - tableTop) < 0.15)) e = rr;
      else if (h.some((y) => y < tableTop - 0.5) && e > 0) break;
    }
    ring.push(Math.round(e * 100) / 100);
  }
  const floorOver = Math.round((floorY - chr.pos[1]) * 1000) / 1000;
  const value = {
    map: 'm11_10_00_00', grace: grace.entity, grace_character: chr.name,
    reactor: { radius_cm: KH3.reactor.radius_cm, height_cm: KH3.reactor.height_cm },
    recover: { radius_cm: KH3.recover.radius_cm, height_cm: KH3.recover.height_cm },
    centre_from_grace_character_m: offset.map((v) => Math.round(v * 1000) / 1000),
    base_over_grace_character_m: Math.round((tableTop - chr.pos[1]) * 1000) / 1000,
    // the floor band all around: standing on the floor (or the table top) within KH3's own reactor radius of the table's
    // edge, at floor height with KH3's capsule height; KH3's recover radius likewise (the user's correction)
    ring: { edge_m_by_kh_yaw: ring, step_deg: 5, floor_over_grace_character_m: floorOver, reach_cm: { reactor: KH3.reactor.radius_cm, recover: KH3.recover.radius_cm }, height_cm: KH3.reactor.height_cm },
    // the measurements the tests stand on (the same frame: metres from the grace character)
    measured: { grace_asset_m: [g[0] - chr.pos[0], g[1] - chr.pos[1], g[2] - chr.pos[2]].map((v) => Math.round(v * 1000) / 1000), toward_player_point: dir.map((v) => Math.round(v * 10000) / 10000), table_top_edge_m: Math.round(edge * 100) / 100, floor_m: floorY === null ? null : Math.round((floorY - chr.pos[1]) * 1000) / 1000, body_m: BODY_M, leader_radii_cm: LEADER_RADII_CM },
  };
  const evidence = `MEASURED (read-only 2.7.2.0 copies, tools/kh_savepoint_areas.js): msb ${grace.name} (entity ${grace.entity}) at (${g.map((v) => v.toFixed(3)).join(', ')}); its character ${chr.name} at (${chr.pos.map((v) => v.toFixed(3)).join(', ')}) (savepoint.rs builds the save point there); its player point ${point.name} (entity ${grace.entity - 970}) at (${point.pos.map((v) => v.toFixed(3)).join(', ')}), ${dist.toFixed(2)} m along (${dir.map((v) => v.toFixed(4)).join(', ')}); navmesh (nvmhktbnd, ${polys.length} faces): the TABLE TOP at y ${tableTop.toFixed(3)} (median of ${top.length} samples of the walkable ring 2.6 .. 3.4 m around the grace) = ${(tableTop - chr.pos[1]).toFixed(3)} m over the character; on the line to the player point the table top ends at ${edge.toFixed(2)} m and the floor (y ${floorY && floorY.toFixed(3)}) begins at ${floorAt && floorAt.toFixed(2)} m. KH3's own capsule (g_ex_SavePoint TresReactor0, pack savepoint/savepoint.json): radius ${KH3.reactor.radius_cm}, height ${KH3.reactor.height_cm}, ${-KH3.reactor.offset_cm} cm under the root (92 cm over the character): it spans the character's -38 .. +142 cm, the table top is +${Math.round((tableTop - chr.pos[1]) * 100)}: inside only within about 1.0 m of the middle, among the swords (seen in the game 2026-11-05: 'not at one' at 1.9 / 2.6 m, OFFERED at 1.7 m after a jump over the swords). THE CENTRE: ${along.toFixed(2)} m from the grace toward the player point, the middle of [${lo.toFixed(2)}, ${hi.toFixed(2)}) = [edge - body - 110 - smallest leader radius, edge + body - 110 - largest) (leaders ${JSON.stringify(LEADER_RADII_CM)} cm, body ${BODY_M * 100} cm): a leader on the table top at its edge is in, whoever leads. THEN THE USER'S CORRECTION (2026-11-05, verbatim: "you should be able to access round table hold save point from the floor next to the table"): THE FLOOR BAND ALL AROUND: the table's edge measured every 5 degrees from the grace character on the same navmesh (${Math.min(...ring).toFixed(2)} .. ${Math.max(...ring).toFixed(2)} m: not a perfect circle), the floor at ${floorOver.toFixed(3)} m over the character; the Save is offered while the leader's own position (his capsule's axis) is within KH3's reactor radius (110 cm) of that edge and his capsule overlaps the slab from the floor up KH3's capsule height (180 cm) (the heal: KH3's recover radius, 50 cm); 1.5 m out from the edge and beyond: not offered, whoever leads. The slab also covers the table top (1 m over the floor), so the table top is in as well; the dais of swords in the middle is not`;
  return { id: 'area.11101950', value, unit: 'cm / metres (Elden Ring axes: x, z from the grace character; base = the capsule\'s bottom over it)', source: 'THE USER\'S CHOICE 2026-11-05 ("do option 1": the Roundtable Hold save point\'s area raised to table level, reachable from the table\'s edge; then the correction "you should be able to access round table hold save point from the floor next to the table": the floor band all around; the inside-only rule unchanged everywhere else); placed by the rules above from the measured map data', evidence, ref_addr: '-', ref_conf: 'confirmed', note: 'crates/kh-er/src/savepoint_area.rs applies it to that grace\'s save point only (reactor and recover capsules: KH3\'s sizes, this centre and base; the floor band: kh_host::savepoint::Ring, KH3\'s radii as the reach from the table\'s edge); Elden Ring\'s own grace rows 6100 .. 6103 stay switched off (gracelock.rs); no green marker on it (reactmark.rs: a save point is never marked)' };
}

const rows = [roundtable()];
const text = '{"sheet":"kh-savepoint-areas","group":"port","columns":["id","value","unit","source","evidence","ref_addr","ref_conf","note"],"rows":[\n' + rows.map((r) => JSON.stringify([r.id, r.value, r.unit, r.source, r.evidence, r.ref_addr, r.ref_conf, r.note])).join(',\n') + '\n]}\n';
if (process.argv.includes('--check')) {
  const was = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf8') : '';
  console.log(was === text ? 'kh-savepoint-areas: up to date' : 'kh-savepoint-areas: STALE (run node tools/kh_savepoint_areas.js)');
  process.exit(was === text ? 0 : 1);
}
fs.writeFileSync(OUT, text);
console.log(OUT, JSON.stringify(rows[0].value));

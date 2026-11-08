// (EMOTE-PROMPTS, the user 2026-11-07: "for elden ring quests that require the use of an emote, if the emote is known, give a
// Y prompt in the area that the emote would work in Elden Ring (use Elden Ring's own data) that when activated triggers the
// even as if the emote was used") EVERY ELDEN RING EVENT THAT WAITS ON THE PLAYER PERFORMING A GESTURE, READ FROM THE GAME'S
// OWN DATA: writes sheets/port/kh-er-gesture-events.json (read by crates/kh-er/src/gestureprompt.rs) and
// build/sora_tmp/emote_prompts/inventory.csv. State note: build/sora_tmp/emote_prompts/STATE.md.
//
// HOW ELDEN RING KNOWS A GESTURE WAS PERFORMED (read in the data, read-only 2.7.2.0 copies):
// - GestureParam row = gesture id; `itemId` = its name (GoodsName); `msgAnimId` = its animation (a00, 80000 + ..).
// - The gesture's animation puts a SpEffect on the player in a window of its TAE (c0000 a00.tae, event type 67 "while in
//   the window" / 66 "once"). That SpEffect is what the scripts read:
//   EMEVD 4[5] IfCharacterHasSpEffect(cond, 10000, sp, 1, ..) in the condition group of a wait; talk ESD function 61
//   (the player has SpEffect sp) in a state's condition.
// - Every EMEVD event instance (2000[0] / 2000[6] with its arguments put in) is walked from its first instruction along
//   every path (flag gotos / skips / ends forked; flags the path sets kept) up to the wait whose MAIN condition holds the
//   gesture SpEffect; a row is one alternative of that MAIN condition (its flags, the player's region 3[2] or radius 3[3],
//   other SpEffects of the player, the own-world test 3[26]) plus what the path needed to get there.
// - A talk ESD gesture check is read from its state's condition (distance f1, angle f10, the flags it reads); the
//   character's enabling flags come from its map event (named in ESD_EXTRA with their evidence).
// Usage: node tools/kh_gesture_events.js [--check]
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const T = path.join(ROOT, 'build/sora_tmp');
const OUT = path.join(ROOT, 'sheets/port/kh-er-gesture-events.json');
const CSV = path.join(T, 'emote_prompts/inventory.csv');
const EVD = require(path.join(T, 'weapons/P9a/tools/evd.js'));
const MSB = require(path.join(T, 'weapons/P9a/tools/msblib.js'));
const REG = require(path.join(T, 'emote_prompts/tools/reg.js'));
const PD = require(path.join(T, 'emote_prompts/tools/pdef.js'));
const MSB_DIR = path.join(T, 'weapons/P9a/game/map/mapstudio');
const TAE = path.join(T, 'party/SUMMON/arrival_re/dump/c0000.anibnd/a00.tae');
const ESD_DIR = path.join(T, 'er/conv/game/esd');
const ESD_TOOL = path.join(T, 'er/er33/tools/esd.js');
const TXT = path.join(T, 'er/conv/game/txt/all');
const CHECK = process.argv.includes('--check');

// ---------------------------------------------------------------- names
function tsv(files) {
  const m = new Map();
  for (const f of files) for (const l of fs.readFileSync(path.join(TXT, f), 'utf8').split('\n')) { const t = l.indexOf('\t'); if (t > 0 && !m.has(+l.slice(0, t))) m.set(+l.slice(0, t), l.slice(t + 1).trim()); }
  return m;
}
const GOODS = tsv(['GoodsName.tsv', 'GoodsName_dlc01.tsv']);
const NPCN = tsv(['NpcName.tsv', 'NpcName_dlc01.tsv']);

// ---------------------------------------------------------------- gestures and the SpEffects their animations put on
const gp = REG.param('GestureParam');
const GESTURES = gp.rows.map((r) => ({ id: r.id, item: gp.buf.readInt32LE(r.at + 4), anim: gp.buf.readInt32LE(r.at + 8) }));
for (const g of GESTURES) g.name = GOODS.get(g.item) || `gesture ${g.id}`;
const spp = REG.param('SpEffectParam'), SPF = PD.fields('SP_EFFECT_PARAM_ST');
const endurance = (id) => { const r = spp.rows.find((x) => x.id === id); return r ? PD.read(spp.buf, r.at, SPF.effect_endurance) : null; };
/** [{anim, sp, type, start, end}] of a00.tae (type 66 / 67 SpEffect events) */
function taeSpEffects() {
  const b = fs.readFileSync(TAE), out = [];
  const u32 = (o) => b.readUInt32LE(o), u64 = (o) => Number(b.readBigUInt64LE(o)), f32 = (o) => b.readFloatLE(o);
  const n = u32(0x54), list = u64(0x58);
  for (let k = 0; k < n; k++) {
    const id = u64(list + k * 16), at = u64(list + k * 16 + 8), ev = u64(at), ne = u32(at + 0x20);
    for (let e = 0; e < ne; e++) {
      const h = ev + e * 24, data = u64(h + 16), type = u32(data);
      if (type === 66 || type === 67) out.push({ anim: id, sp: b.readInt32LE(u64(data + 8)), type, start: f32(u64(h)), end: f32(u64(h + 8)) });
    }
  }
  return out;
}
const TAE_SP = taeSpEffects();
const IN_GESTURE = 100200; // every gesture animation's whole-length "in a gesture" effect (not a gesture's own)
/** sp -> [{gesture, tae}] for the SpEffects only a gesture animation puts on */
const BY_SP = new Map();
for (const t of TAE_SP) {
  const g = GESTURES.find((x) => x.anim === t.anim || (t.anim > x.anim && t.anim < x.anim + 10 && !GESTURES.some((y) => y.anim === t.anim)));
  if (!g || t.sp === IN_GESTURE) continue;
  if (!BY_SP.has(t.sp)) BY_SP.set(t.sp, []);
  BY_SP.get(t.sp).push({ g, t });
}

// ---------------------------------------------------------------- EMEVD instance walk
const maps = EVD.maps();
const inits = []; // {map, slot, event, args: Buffer}
for (const m of maps) for (const ev of EVD.load(m)) for (const i of ev.ins) {
  if (i.bank === 2000 && (i.id === 0 || i.id === 6)) inits.push({ map: m, ev: ev.id, slot: i.id === 0 ? i.words[0] : 0, event: i.words[1], args: Buffer.from(i.hex, 'hex').slice(8) });
}
function argBytes(ins, init) {
  const buf = Buffer.from(ins.hex, 'hex');
  for (const p of ins.params) init.args.copy(buf, p.tgt, p.src, p.src + p.len);
  return buf;
}
const i8 = (b, o) => b.readInt8(o), u8 = (b, o) => b[o], i32 = (b, o) => b.readInt32LE(o), f32 = (b, o) => b.readFloatLE(o);

/** a literal: {flag, on} ; an atom: {k:'flag'|'region'|'radius'|'sp'|'own'|'other', ...} */
function atomOf(ins, a, ownFlag) {
  const key = `${ins.bank}[${ins.id}]`;
  if (key === '3[0]') { const type = u8(a, 2); const flag = type === 2 ? ownFlag : i32(a, 4); return { k: 'flag', flag, on: u8(a, 1) === 1, raw: key }; }
  if (key === '3[2]') return { k: 'region', entity: i32(a, 4), region: i32(a, 8), inside: u8(a, 1) === 1, raw: key };
  if (key === '3[3]') return { k: 'radius', entity: i32(a, 4), other: i32(a, 8), radius: Math.round(f32(a, 12) * 1000) / 1000, inside: u8(a, 1) === 1, raw: key };
  if (key === '4[5]') return { k: 'sp', chr: i32(a, 4), sp: i32(a, 8), has: u8(a, 12) === 1, raw: key };
  if (key === '3[26]') return { k: 'own', not_own: u8(a, 1) === 1, raw: key };
  return { k: 'other', raw: key, words: [...Array(Math.floor(a.length / 4)).keys()].map((j) => i32(a, j * 4)) };
}
const isCond = (ins) => ins.bank >= 0 && ins.bank <= 5;
const isFlagOnly = (f) => f.items.every((x) => (x.items ? isFlagOnly(x) : x.k === 'flag'));
const neg = (f) => (f.items ? { op: f.op === 'and' ? 'or' : 'and', items: f.items.map(neg) } : { ...f, on: !f.on });
/** formula -> DNF: list of conjunctions (lists of atoms) */
function dnf(f) {
  if (!f.items) return [[f]];
  if (f.op === 'or') return f.items.flatMap(dnf);
  let acc = [[]];
  for (const it of f.items) { const d = dnf(it); const next = []; for (const a of acc) for (const c of d) next.push(a.concat(c)); acc = next; }
  return acc;
}
/** flag-only formula -> CNF clauses (lists of {flag,on}) */
const cnf = (f) => dnf(neg(f)).map((conj) => conj.map((l) => ({ flag: l.flag, on: !l.on })));

function walk(ev, init, mapName) {
  const ownFlag = ev.id + init.slot;
  const labels = new Map(); ev.ins.forEach((ins, k) => { if (ins.bank === 1014) labels.set(ins.id, k); });
  const rows = [], notes = new Set();
  const stack = [{ pc: 0, known: new Map(), clauses: [], groups: new Map(), steps: 0 }];
  const clone = (s) => ({ pc: s.pc, known: new Map(s.known), clauses: s.clauses.slice(), groups: new Map([...s.groups].map(([k, v]) => [k, v.slice()])), steps: s.steps });
  const groupF = (s, g) => ({ op: g < 0 ? 'or' : 'and', items: (s.groups.get(g) || []).map((x) => (x.sub !== undefined ? groupF(s, x.sub) : x)) });
  /** require the flag-only formula f (true); false: contradicts what the path knows */
  function require(s, f) {
    for (const cl of cnf(f)) {
      const live = cl.filter((l) => !s.known.has(l.flag) || s.known.get(l.flag) === l.on);
      if (!live.length) return false;
      if (live.some((l) => s.known.get(l.flag) === l.on)) continue;
      if (live.length === 1) s.known.set(live[0].flag, live[0].on); else s.clauses.push(live);
    }
    return true;
  }
  const flagF = (flag, on) => ({ op: 'and', items: [{ k: 'flag', flag, on }] });
  while (stack.length) {
    const s = stack.pop();
    let alive = true;
    while (alive && s.pc < ev.ins.length) {
      if (++s.steps > 400) { notes.add('path too long'); alive = false; break; }
      const ins = ev.ins[s.pc], a = argBytes(ins, init), key = `${ins.bank}[${ins.id}]`;
      const fork = (jumpTo, condTrue) => { const t = clone(s); if (require(t, condTrue)) { t.pc = jumpTo; stack.push(t); } if (!require(s, neg(condTrue))) alive = false; else s.pc++; };
      if (isCond(ins) && key !== '0[0]') {
        const at = atomOf(ins, a, ownFlag), g = i8(a, 0);
        if (g === 0) { s.groups.set(0, [at]); } else { if (!s.groups.has(g)) s.groups.set(g, []); s.groups.get(g).push(at); s.pc++; continue; }
      }
      if (key === '0[0]') {
        const res = i8(a, 0), st = u8(a, 1), g = i8(a, 2);
        const item = st === 1 ? { sub: g } : { sub: g, negated: true };
        if (st !== 1) notes.add('negated group');
        if (res !== 0) { if (!s.groups.has(res)) s.groups.set(res, []); s.groups.get(res).push(item); s.pc++; continue; }
        s.groups.set(0, [item]);
      }
      if (s.groups.has(0)) {
        // a WAIT on MAIN
        const f = groupF(s, 0);
        s.groups = new Map();
        const alts = dnf(f).filter((conj) => conj.some((x) => x.k === 'sp' && x.chr === 10000 && x.has && BY_SP.has(x.sp)));
        if (alts.length) { for (const conj of alts) rows.push({ conj, known: new Map(s.known), clauses: s.clauses.map((c) => c.slice()), pc: s.pc }); alive = false; break; }
        if (isFlagOnly(f) && dnf(f).length === 1) for (const l of dnf(f)[0]) s.known.set(l.flag, l.on); // it held then (an either-or wait tells nothing)
        s.pc++; continue;
      }
      if (ins.bank === 1014 || ins.bank === 1001 || ins.bank === 2000 || ins.bank >= 2001) {
        if (key === '2003[66]' || key === '2003[69]') { const type = u8(a, 0); s.known.set(type === 2 ? ownFlag : i32(a, 4), u8(a, 8) === 1); }
        s.pc++; continue;
      }
      if (key === '1003[2]' || key === '1003[1]' || key === '1003[101]') {
        const n = u8(a, 0), st = u8(a, 1) === 1, type = u8(a, 2), flag = type === 2 ? ownFlag : i32(a, 4);
        const target = key === '1003[2]' ? ev.ins.length : key === '1003[1]' ? s.pc + 1 + n : labels.get(n);
        if (target === undefined) { notes.add('missing label'); alive = false; break; }
        fork(target, flagF(flag, st)); continue;
      }
      if (key === '1000[2]' || key === '1000[1]' || key === '1000[101]' || key === '1000[7]' || key === '1000[8]') {
        const n = u8(a, 0), st = u8(a, 1) === 1, g = i8(a, 2), f = groupF(s, g);
        s.groups.delete(g);
        if (!isFlagOnly(f)) { notes.add(`${key} on a non-flag group: both ways`); const t = clone(s); t.pc = key === '1000[1]' || key === '1000[7]' ? s.pc + 1 + n : key === '1000[101]' ? labels.get(n) : ev.ins.length; stack.push(t); s.pc++; continue; }
        const target = key === '1000[2]' || key === '1000[8]' ? ev.ins.length : key === '1000[101]' ? labels.get(n) : s.pc + 1 + n;
        fork(target, st ? f : neg(f)); continue;
      }
      if (key === '1000[103]') { s.pc = labels.get(u8(a, 0)); if (s.pc === undefined) alive = false; continue; }
      if (key === '1000[3]') { s.pc += 1 + u8(a, 0); continue; }
      if (key === '1000[4]') { alive = false; break; }
      if (key === '1003[14]') { s.pc++; continue; } // the network approval wait (always passes offline)
      // any other flow instruction: both ways (skip its first byte / go on), noted
      notes.add(`${key} both ways`);
      const t = clone(s); t.pc = s.pc + 1 + u8(a, 0); stack.push(t); s.pc++;
    }
  }
  return { rows, notes: [...notes] };
}

// ---------------------------------------------------------------- MSB lookups
const msbCache = new Map();
function msb(map) { if (!msbCache.has(map)) { const f = path.join(MSB_DIR, map + '.msb'); msbCache.set(map, fs.existsSync(f) ? MSB.read(f) : null); } return msbCache.get(map); }
const SHAPES = { 0: 'point', 1: 'circle', 2: 'sphere', 3: 'cylinder', 4: 'rect', 5: 'box', 6: 'composite' };
const npp = REG.param('NpcParam'), NPF = PD.fields('NPC_PARAM_ST');
function npcName(npcId) { const r = npp.rows.find((x) => x.id === npcId); return r ? NPCN.get(PD.read(npp.buf, r.at, NPF.name_id)) || null : null; }
function partOf(map, entity) { const m = msb(map); return m && m.parts.find((p) => p.entity === entity); }

// ---------------------------------------------------------------- rows from EMEVD
const rows = [];
const notes = [];
const flagLits = (known, conj) => {
  const all = new Map(known);
  for (const x of conj) if (x.k === 'flag') all.set(x.flag, x.on);
  return [...all].sort((p, q) => p[0] - q[0]).map(([f, on]) => [f, on]);
};
const seen = new Set();
for (const m of maps) for (const ev of EVD.load(m)) {
  if (!ev.ins.some((i) => i.bank === 4 && i.id === 5)) continue;
  const myInits = inits.filter((x) => x.event === ev.id && (x.map === m || m.startsWith('common')));
  const hasGesture = ev.ins.some((i) => i.bank === 4 && i.id === 5 && (BY_SP.has(i.words[2]) || i.params.some((p) => p.tgt === 8)));
  if (!hasGesture) continue;
  if (!myInits.length) { if (ev.ins.some((i) => i.bank === 4 && i.id === 5 && BY_SP.has(i.words[2]))) notes.push(`${m} event ${ev.id}: waits on a gesture SpEffect but no map initializes it (an unused event): no row`); continue; }
  for (const init of myInits) {
    const w = walk(ev, init, m);
    for (const r of w.rows) {
      const sps = r.conj.filter((x) => x.k === 'sp' && x.chr === 10000);
      const gsp = sps.find((x) => x.has && BY_SP.has(x.sp));
      const others = sps.filter((x) => x !== gsp).map((x) => [x.sp, x.has]);
      const where = [];
      for (const x of r.conj) {
        if (x.k === 'region' && x.entity === 10000) {
          const reg = msb(init.map) && msb(init.map).regions.find((q) => q.entity === x.region);
          where.push(reg ? { kind: 'region', map: init.map.replace(/_\d\d$/, '_00'), entity: x.region, inside: x.inside, shape: SHAPES[reg.shape] || String(reg.shape), pos: reg.pos, rot: reg.rot, dims: reg.dims, msb_name: reg.name } : { kind: 'region', map: init.map, entity: x.region, inside: x.inside, missing: true });
        }
        if (x.k === 'radius' && x.entity === 10000) {
          const part = partOf(init.map, x.other);
          where.push({ kind: 'near', map: init.map.replace(/_\d\d$/, '_00'), entity: x.other, radius_m: x.radius, inside: x.inside, max_angle_deg: null, npc: part ? npcName(part.npc) : null, msb_part: part ? part.name : null });
        }
      }
      const unknown = r.conj.filter((x) => x.k === 'other' || (x.k === 'region' && x.entity !== 10000) || (x.k === 'radius' && x.entity !== 10000));
      const own = r.conj.some((x) => x.k === 'own' && !x.not_own);
      const g = BY_SP.get(gsp.sp)[0];
      const row = {
        source: 'emevd', map: init.map, event: ev.id, slot: init.slot, own_flag: ev.id + init.slot,
        gesture: g.g.id, gesture_name: g.g.name, gesture_item: g.g.item, anim: g.g.anim,
        sp_effect: gsp.sp, sp_endurance: endurance(gsp.sp), tae: { anim: g.t.anim, type: g.t.type, start: +g.t.start.toFixed(4), end: +g.t.end.toFixed(4) },
        where, flags: flagLits(r.known, r.conj), any_flags: r.clauses.map((c) => c.map((l) => [l.flag, l.on])),
        player_sp: others, own_world: own, unknown: unknown.map((x) => x.raw), path_notes: w.notes,
        wait_at: `${init.map} event ${ev.id} instruction #${r.pc}`,
      };
      const dedupe = JSON.stringify({ ...row, map: row.map.replace(/_\d\d$/, ''), wait_at: '' });
      const twin = rows.find((x) => x._key === dedupe);
      if (twin) { if (init.map !== twin.map && !twin.also_maps.includes(init.map)) twin.also_maps.push(init.map); continue; }
      if (seen.has(dedupe)) continue;
      seen.add(dedupe);
      rows.push({ ...row, also_maps: [], _key: dedupe });
    }
  }
}

// ---------------------------------------------------------------- rows from talk ESD (function 61 on a gesture SpEffect)
// The character's enabling / done flags are not in its talk script: read from its map event, named here with the evidence.
const ESD_EXTRA = {
  425006100: {
    flags: [[4565, true], [4909, false], [2049442702, false], [2049442703, false]],
    evidence: 'm61_49_44_00 event 2049440701 (InitializeEvent args X12 4565 / X20 4909 / X24 2049442703): the character is enabled only while 4565 is ON and 4909, 2049442703 are OFF; the ESD success path sets 2049449205 then 2049442702 (event 2049442801 waits on 2049442702 to start the duel), so 2049442702 OFF = not yet triggered',
  },
};
const esdTool = require('child_process');
for (const dir of fs.readdirSync(ESD_DIR)) {
  for (const f of fs.readdirSync(path.join(ESD_DIR, dir)).filter((x) => x.endsWith('.esd'))) {
    const text = esdTool.execFileSync(process.execPath, [ESD_TOOL, path.join(ESD_DIR, dir, f)], { encoding: 'utf8', maxBuffer: 64 << 20 });
    const lines = text.split('\n');
    lines.forEach((l, k) => {
      for (const mm of l.matchAll(/(?:^|[ [;])61 (\d+) call\/1 ; 1 ==/g)) {
        const sp = +mm[1];
        if (!BY_SP.has(sp)) continue;
        // the same state's earlier conditions: distance (f1) and angle (f10) put in registers
        let st = k; while (st > 0 && !/^ state /.test(lines[st])) st--;
        const block = lines.slice(st, k + 1).join('\n');
        const dist = block.match(/ 1 call\/0 setreg\d ([\d.]+)f? </), ang = block.match(/(?:^|[ [;])10 call\/0 setreg\d ([\d.]+)f? </);
        const talk = +f.replace(/^t|\.esd$/g, '');
        let part = null, map = null;
        for (const m of fs.readdirSync(MSB_DIR).filter((x) => x.endsWith('.msb'))) { const mm2 = msb(m.slice(0, -4)); const p = mm2 && mm2.parts.find((q) => q.talk === talk && q.type === 2); if (p) { part = p; map = m.slice(0, -4); break; } }
        const g = BY_SP.get(sp)[0], extra = ESD_EXTRA[talk] || { flags: [], evidence: 'no map event named' };
        rows.push({
          source: 'esd', map, event: talk, slot: 0, own_flag: 0,
          gesture: g.g.id, gesture_name: g.g.name, gesture_item: g.g.item, anim: g.g.anim,
          sp_effect: sp, sp_endurance: endurance(sp), tae: { anim: g.t.anim, type: g.t.type, start: +g.t.start.toFixed(4), end: +g.t.end.toFixed(4) },
          where: [{ kind: 'near', map, entity: part ? part.entity : 0, radius_m: dist ? +dist[1] : 0, inside: true, max_angle_deg: ang ? +ang[1] : null, npc: part ? npcName(part.npc) : null, msb_part: part ? part.name : null }],
          flags: extra.flags, any_flags: [], player_sp: [], own_world: false, unknown: [/52 call\/0 ; 0 ==/.test(l) ? 'talk f52() == 0' : null].filter(Boolean), path_notes: [extra.evidence],
          wait_at: `${dir}/${f} state line ${k + 1}: ${l.trim().slice(0, 160)}`, also_maps: [],
        });
      }
    });
  }
}

// ---------------------------------------------------------------- the sheet and the inventory
// ---------------------------------------------------------------- the user's rules on single rows
// A SpEffect of the player a row's script also waits on can be GIVEN with the gesture SpEffect for the press window instead of
// being required (the script's own condition then passes unchanged): the user's decision, quoted per row.
const USER_RULES = {
  1039482610: { grant: [486], note: 'THE USER 2026-11-07 (via the coordinator, verbatim): "yes drop the crown requirement": the Glintstone Crown SpEffect 486 (resident effect of protectors 830000-834000, 836000) is not required; it is put on the body with the 485 of Erudition for the press window' },
};
for (const r of rows) {
  r.extra_speffects = [];
  const rule = USER_RULES[r.event];
  if (!rule) continue;
  for (const sp of rule.grant) if (r.player_sp.some(([s, has]) => s === sp && has)) { r.player_sp = r.player_sp.filter(([s]) => s !== sp); r.extra_speffects.push(sp); }
  r.user_rule = rule.note;
}
rows.sort((p, q) => p.gesture - q.gesture || p.event - q.event);
const baseId = (r) => `gesture.${r.source}.${r.event}${r.where[0] && r.where[0].kind === 'near' && rows.filter((x) => x.event === r.event).length > 1 ? '.' + r.where[0].entity : ''}`;
const ids = new Map();
for (const r of rows) { const b = baseId(r); const n = [...ids.values()].filter((v) => v === b || v.startsWith(b + '.path')).length; ids.set(r, n ? `${b}.path${n + 1}` : b); }
const id = (r) => ids.get(r);
const sheetRows = rows.map((r) => {
  const v = { ...r }; delete v._key;
  const w = r.where.map((x) => (x.kind === 'region' ? `${x.shape} region ${x.entity} (${x.msb_name || '?'}) of ${x.map}` : `within ${x.radius_m} m of ${x.npc || 'character'} (entity ${x.entity})${x.max_angle_deg ? `, angle < ${x.max_angle_deg} deg` : ''}`)).join(' and ');
  const ev = `READ (2.7.2.0 copies): ${r.wait_at}; gesture ${r.gesture} "${r.gesture_name}" (GestureParam itemId ${r.gesture_item}, msgAnimId ${r.anim}) puts SpEffect ${r.sp_effect} on the player (a00.tae anim ${r.tae.anim} event type ${r.tae.type}, ${r.tae.start}-${r.tae.end} s; SpEffectParam effectEndurance ${r.sp_endurance}); ${w}; flags ${JSON.stringify(r.flags)}${r.any_flags.length ? ' any-of ' + JSON.stringify(r.any_flags) : ''}${r.player_sp.length ? '; player SpEffects ' + JSON.stringify(r.player_sp) : ''}${r.extra_speffects.length ? '; GIVEN with the gesture SpEffect for the press window (the script still waits on them): ' + JSON.stringify(r.extra_speffects) : ''}${r.own_world ? '; own world (3[26])' : ''}${r.unknown.length ? '; unread: ' + r.unknown.join(', ') : ''}${r.path_notes.length ? '; ' + r.path_notes.join('; ') : ''}`;
  return [id(r), v, 'ids / metres', r.source === 'emevd' ? 'event/*.emevd, map/mapstudio/*.msb, regulation.bin GestureParam, chr/c0000.anibnd a00.tae' : 'script/talk talk ESD, map/mapstudio/*.msb, regulation.bin GestureParam, chr/c0000.anibnd a00.tae', ev, '-', r.unknown.length ? 'inferred' : 'confirmed', [r.also_maps.length ? `also in ${r.also_maps.join(', ')}` : '', r.user_rule || ''].filter(Boolean).join('; ') || '-'];
});
const text = '{"sheet":"kh-er-gesture-events","group":"port","columns":["id","value","unit","source","evidence","ref_addr","ref_conf","note"],"rows":[\n' + sheetRows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n';
const q = (s) => `"${String(s).replace(/"/g, '""')}"`;
const csv = ['gesture_id,gesture,sp_effect,extra_speffects,source,map,event,where,flags,any_flags,player_sp,own_world,unread,wait_at'].concat(rows.map((r) => [r.gesture, q(r.gesture_name), r.sp_effect, q(r.extra_speffects.join(' ')), r.source, [r.map, ...r.also_maps].join(' '), r.event,
  q(r.where.map((x) => (x.kind === 'region' ? `${x.shape} ${x.entity} at ${x.pos} rot ${x.rot} dims ${x.dims} (${x.msb_name})` : `near ${x.entity} ${x.npc || ''} r ${x.radius_m}${x.max_angle_deg ? ' angle<' + x.max_angle_deg : ''}`)).join(' & ')),
  q(r.flags.map(([f, on]) => `${f}=${on ? 'ON' : 'OFF'}`).join(' ')), q(r.any_flags.map((c) => c.map(([f, on]) => `${f}=${on ? 'ON' : 'OFF'}`).join('|')).join(' & ')), q(r.player_sp.map(([s, h]) => `${s}${h ? '' : ' absent'}`).join(' ')), r.own_world, q(r.unknown.join(' ')), q(r.wait_at)].join(','))).join('\n') + '\n';
if (CHECK) {
  const was = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf8') : '';
  console.log(was === text ? 'kh-er-gesture-events: up to date' : 'kh-er-gesture-events: STALE (run node tools/kh_gesture_events.js)');
  process.exit(was === text ? 0 : 1);
}
fs.writeFileSync(OUT, text);
fs.writeFileSync(CSV, csv);
console.log(`kh-er-gesture-events: ${rows.length} rows (${GESTURES.length} gestures, ${BY_SP.size} gesture SpEffects: ${[...BY_SP].map(([s, l]) => `${s}<-${l.map((x) => x.g.id).join('/')}`).join(' ')})`);
for (const n of notes) console.log('  ' + n);
for (const r of rows) console.log(`  ${id(r)}: ${r.gesture_name} -> ${r.where.map((x) => x.kind + ' ' + x.entity).join(' & ')} flags ${JSON.stringify(r.flags)} any ${JSON.stringify(r.any_flags)} sp ${JSON.stringify(r.player_sp)} unread ${r.unknown.join(',')} notes ${r.path_notes.join(' | ').slice(0, 120)}`);

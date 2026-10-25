// Writes the four sheets of Sora's weapon and visual effects (docs/sora-vfx-spec.md):
//   sheets/port/sora-effects.json      one row per trigger -> effect (animation notify entry, component table row, load-asset
//                                      property, projectile, code call) that base-form Sora with the Kingdom Key can reach,
//                                      plus pointer rows for other keyblades / forms that sit in the same clips and tables
//   sheets/port/sora-weapon.json       the Keyblade system: parameters, sockets, components, weapon animations, behaviour
//                                      rows and one pointer row per keyblade
//   sheets/port/sora-vfx-params.json   fixed parameters of the effect runtime (attach component, fades, group constants)
//   sheets/port/sora-vfx-modules.json  census of the Cascade subset: one row per particle module / type data / distribution /
//                                      feature with usage counts (Sora's effect set and game-wide) and what the pack does with it
// The rows are research results kept as evidence files under audit/sora_port/vfx/ (row arrays written by the research
// strands; "--import" refreshes them from build/sora_tmp/vfx/<strand>/ when those scratch folders still exist). This tool
// only normalises them: column order, "-" for not-applicable cells, null for undetermined cells (never a guess), the
// group gate of animation-notify rows (column kk_base, rule in docs/sora-vfx-spec.md section 7.4) and the confidence
// of the Square Enix module semantics.
//
//   node tools/sora_vfx_sheet.js            write the sheets
//   node tools/sora_vfx_sheet.js --import   first copy the strand row files into audit/sora_port/vfx/
//   node tools/sora_vfx_sheet.js --check    exit 1 if a sheet on disk differs
'use strict';
const fs = require('fs'), path = require('path');
const VERSION = '1.0.0';
const R = path.resolve(__dirname, '..');
const AUD = path.join(R, 'audit', 'sora_port', 'vfx');
const TMP = path.join(R, 'build', 'sora_tmp', 'vfx');
const IMPORTS = { // evidence file <- strand scratch file
  'rows_effects.json': 'fxinv/rows_effects.json',
  'rows_weapon.json': 'weapon/rows_weapon.json',
  'rows_fxsys.json': 'fxsys/rows_fxsys.json',
  'rows_modules.json': 'cascade/rows_modules.json',
  'sqex_modules.json': 'fxsys/sqex_modules.json',
  'notify_classes.json': 'fxsys/notify_classes.json',
  'attach_structs.json': 'fxsys/attach_structs.json',
  'physmat_effects.json': 'fxsys/physmat_effects.json',
  'weapon_effects_seen.json': 'weapon/effects_seen.json',
};
if (process.argv.includes('--import')) {
  fs.mkdirSync(AUD, { recursive: true });
  for (const [dst, src] of Object.entries(IMPORTS)) {
    const s = path.join(TMP, src);
    if (!fs.existsSync(s)) { console.log('import: missing ' + src + ' (kept the evidence file)'); continue; }
    fs.copyFileSync(s, path.join(AUD, dst));
  }
}
const load = (f) => JSON.parse(fs.readFileSync(path.join(AUD, f), 'utf8'));
const dash = (v) => (v === null || v === undefined || v === '' ? '-' : v);
const addr = (v) => { const m = /^0x([0-9a-fA-F]{9})$/.exec(String(v || '')); return m ? '0x' + m[1].toUpperCase() : '-'; }; // one function address of the code sheet, or "-"

// ------------------------------------------------------------------ sora-effects
// Group gate (docs/sora-vfx-spec.md 7.4). An inline attach entry of an animation notify spawns only when its GroupID is -1
// or equals a current animation group of the pawn's effect-attach component; the play API sets that group on every play.
// Groups the player states pass with the plain Keyblade (weapon kind 1): 0 everywhere, except the ones listed here.
const FORM_KIND_GROUPS = new Set([1823, 1824, 1900, 2010, 2100, 2200, 3400, 3410]); // passed only for form-change weapon kinds
const KK_PLAY_GROUPS = { // group -> { clips (logical names) that are played with it, condition }
  313: { names: /^WallKick/, cond: 'WallKickF/R/L are played with effect group 0x139' },
  521: { names: /^(Guard|AirGuard|GuardHit)/, cond: 'guard in water (Guard flag 0x40) is played with effect group 0x209' },
};
function gate(r) {
  const [, category, kind, trigger, , logical, , , effect, , params] = r;
  const p = params || {}; const scope = p.kk_scope; const g = p.group_id;
  const isAttach = /^anim_notify/.test(kind) && /AttachEffect/.test(trigger);
  if (!isAttach || g === undefined || g === null) {
    if (scope === 'kk') return ['yes', null];
    if (scope === 'second_form' || scope === 'other_form' || scope === 'other_keyblade') return ['no', null];
    return ['unknown', null];
  }
  if (effect === null) return ['na', 'entry without effect'];
  const names = Array.isArray(logical) ? logical : [];
  if (g === 0 || g === -1) {
    if (scope === 'kk') return ['yes', `GroupID ${g}: spawns under the default effect group 0`];
    if (scope === 'second_form') return ['no', 'GroupID 0, but the clip belongs to the Second Form moveset (played only in that form)'];
    if (scope === 'other_form') return ['no', 'GroupID 0, but the clip belongs to another form'];
    if (scope === 'other_keyblade') return ['unknown', 'GroupID 0: not gated by group. The effect is named for another keyblade and sits in a clip (shotlock / StyleChange variant) that the Kingdom Key is not known to play: clip selection, not the group, decides'];
    return ['unknown', null];
  }
  if (FORM_KIND_GROUPS.has(g)) return ['no', `GroupID ${g} is an effect group the states pass only for form-change weapon kinds; the plain Keyblade plays with group 0`];
  const k = KK_PLAY_GROUPS[g];
  if (k) return [names.some((n) => k.names.test(n)) ? 'conditional' : 'unknown', k.cond];
  if (g >= 1300 && g <= 1445) return ['conditional', `GroupID ${g} = TresPlayerMagicSet m_EffectGroup: spawns when the cast animation is played with that spell's group`];
  if (g === 1585) return ['conditional', 'GroupID 1585 (0x631): the shotlock state plays its kind-1 animation with this group'];
  if (scope === 'other_keyblade') return ['no', `GroupID ${g} != 0 next to another keyblade's effect: not a group any base-form state is known to pass`];
  return ['unknown', `GroupID ${g}: no state is known to play this clip with that group`];
}
function effectsSheet() {
  const rows = load('rows_effects.json');
  const out = rows.map((r) => {
    const [id, category, kind, trigger, clip, logical, time, duration, effect, attach, params, source, evidence, refAddr, refConf, note] = r;
    const [kk, basis] = gate(r);
    const p = { ...(params || {}) }; if (basis) p.group_gate = basis;
    const pure = effect === null && /Shot$|ThrowWeapon|SetWeaponVisible|SetActorVisible|SetMeshVisible|m_CheckFootSteps|DecalMaterial|TrailMaterial/.test(trigger);
    const anim = /^anim_notify/.test(kind);
    return [id, category, kind, trigger, dash(clip), Array.isArray(logical) && logical.length ? logical : '-',
      anim ? time : dash(time), kind === 'anim_notify_state' ? duration : dash(duration),
      pure ? '-' : effect, kk, attach === null ? (pure || !anim ? '-' : null) : attach, p, dash(source), evidence, addr(refAddr), refConf, dash(note)];
  });
  return [['id', 'category', 'trigger_kind', 'trigger', 'clip', 'logical', 'time', 'duration', 'effect', 'kk_base', 'attach', 'params', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'], out];
}

// ------------------------------------------------------------------ sora-weapon / sora-vfx-params
const PARAM_COLS = ['id', 'kind', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'];
function paramSheet(file) {
  return [PARAM_COLS, load(file).map((r) => { const o = r.slice(0, 9); o[3] = dash(o[3]); o[6] = addr(o[6]); o[8] = dash(o[8]); if (o[7] === 'guess') o[2] = null; return o; })];
}

// ------------------------------------------------------------------ sora-vfx-modules
function modulesSheet() {
  const rows = load('rows_modules.json');
  const sem = new Map(); const sq = load('sqex_modules.json'); for (const m of (Array.isArray(sq) ? sq : sq.modules || Object.values(sq))) sem.set(m.class, m.confidence || 'reflection-only');
  const out = rows.map((r) => { const [id, origin, category, se, sm, si, gi, support, note] = r; return [id, origin, category, se, sm, si, gi, support, origin === 'sqex' && sem.has(id) ? sem.get(id) : '-', dash(note)]; });
  return [['id', 'origin', 'category', 'sora_effects', 'sora_emitters', 'sora_instances', 'game_instances', 'support', 'semantics_conf', 'note'], out];
}

// ------------------------------------------------------------------ write
const SHEETS = { 'sora-effects': effectsSheet, 'sora-weapon': () => paramSheet('rows_weapon.json'), 'sora-vfx-params': () => paramSheet('rows_fxsys.json'), 'sora-vfx-modules': modulesSheet };
let bad = 0;
for (const [name, make] of Object.entries(SHEETS)) {
  const [cols, rows] = make();
  const ids = new Set(); for (const r of rows) { if (ids.has(r[0])) { console.error(name + ': duplicate id ' + r[0]); bad = 1; } ids.add(r[0]); }
  const out = `{"sheet":"${name}","group":"port","columns":${JSON.stringify(cols)},"rows":[\n${rows.map((r) => JSON.stringify(r)).join(',\n')}\n]}\n`;
  const file = path.join(R, 'sheets', 'port', name + '.json');
  if (process.argv.includes('--check')) { const same = fs.existsSync(file) && fs.readFileSync(file, 'utf8') === out; console.log(same ? name + ': ' + rows.length + ' rows, current' : name + ': sheet differs from tools/sora_vfx_sheet.js ' + VERSION); if (!same) bad = 1; continue; }
  fs.writeFileSync(file, out);
  let extra = '';
  if (name === 'sora-effects') { const c = {}; for (const r of rows) c[r[9]] = (c[r[9]] || 0) + 1; extra = ' kk_base ' + JSON.stringify(c); }
  console.log(name + ': ' + rows.length + ' rows written (tools/sora_vfx_sheet.js ' + VERSION + ')' + extra);
}
process.exit(bad);

// Writes the sheets of Sora's character progression and status systems (docs/sora-stats-spec.md):
//   sheets/port/sora-levels.json       one row per level 1..99: EXP, stats, AP and the ability granted on each of the three paths
//   sheets/port/sora-abilities.json    one row per ETresAbilityKind value: data row, sources, effect, hook addresses, test idea
//   sheets/port/sora-stats-rules.json  parameters and formulas of the status object, EXP, HP / MP / focus runtime rules
//   sheets/port/sora-difficulty.json   one row per game level (Beginner, Standard, Proud, Critical)
//   sheets/port/sora-equipment.json    Sora's keyblades (with the Kingdom Key upgrade rows), armour, accessories and slot counts
//   sheets/port/sora-loadout-99.json   the level-99 test configuration (stats per opening choice, abilities, magic, shotlocks, items)
// Data rows are read on every run from the game's tables under sheets/entities and from base/KH3.exe (the ability-query call
// sites are found by decoding the executable: calls through the ability slots of the pawn vtable and to the ability helpers,
// with the ability id recovered from EDX). Code facts (effects, formulas) are authored rows below, each with its address.
// Unknown values are null or "-", never guessed. ref_conf: confirmed = read at instruction level or in a data table;
// inferred = decompiled C only, or derived; guess = not determined.
//
//   node tools/sora_stats_sheet.js          write the sheets
//   node tools/sora_stats_sheet.js --check  exit 1 if a sheet on disk differs
const fs = require('fs'), path = require('path');
const VERSION = '1.0.0';
const R = path.resolve(__dirname, '..');
const pe = require('./lib/pe.js');
const iced = require(path.join(R, 'node_modules/iced-x86'));
const XSI = require('./sora_xs_sheet.js'); // (XSI) the XS abilities, their level slots and boss rows

// ------------------------------------------------------------------ game tables
const table = (rel) => JSON.parse(fs.readFileSync(path.join(R, 'sheets/entities', rel), 'utf8'));
const short = (v) => (typeof v === 'string' ? v.replace(/^E[A-Za-z0-9_]+::/, '') : v);
const items = (v) => (v && v.items ? v.items.map(short) : []);
const T_LEVEL = 'DataTable/Player/LevelUp/p_ex001_LevelUpData.json';
const T_BASE = 'DataTable/Player/Base/p_ex001_BaseParamData.json';
const T_ABIL = 'Load/Tres/TresAbilityData.json';
const T_VB = 'Load/Tres/TresVBonusData.json';
const T_EQUIP = 'Load/Tres/TresEquipItemData.json';
const T_ENH = 'Load/Tres/ItemSynthesis/TresItemWeaponEnhanceData.json';
const T_INIT = 'Load/Common/TresChrInitData.json';
const T_MEAL = 'Load/Tres/TresFullcourseAbilityList.json';
const T_ITEM = 'Load/Tres/TresItemData.json';
const levelRows = table(T_LEVEL).rows, baseRow = table(T_BASE).rows[0], abilRows = table(T_ABIL).rows, vbRows = table(T_VB).rows;
const equipRows = table(T_EQUIP).rows, enhRows = table(T_ENH).rows, mealRows = table(T_MEAL).rows, itemRows = table(T_ITEM).rows;
const initObj = (table(T_INIT).rows || table(T_INIT).objects)[0];
const initList = (name) => { const p = initObj.m_PlayerSora.props.find((x) => x.tag.name === name); return p ? (p.value.items ? p.value.items.map(short) : p.value) : []; };

// enum ETresAbilityKind (sheets/reflect/enum-values)
const enumVals = (() => {
  const j = JSON.parse(fs.readFileSync(path.join(R, 'sheets/reflect/enum-values.json'), 'utf8'));
  const ci = (n) => j.columns.indexOf(n); const out = [];
  for (const r of j.rows) if (r[ci('enum')] === 'ETresAbilityKind') out.push([short(r[ci('name')]), r[ci('value')]]);
  // (XSI, 2026-10-16) the port's own values (sheets/port/sora-enum-ext via tools/sora_xs_sheet.js): the XS abilities 256..
  for (const a of XSI.ABILITIES) out.push([a[0], a[1]]);
  return out.sort((a, b) => a[1] - b[1]);
})();
const idOf = new Map(enumVals.map(([n, v]) => [n, v])), nameOf = new Map(enumVals.map(([n, v]) => [v, n]));

// English text (assets_converted/loose/.../Localization/Game/en); "-" when the pack is not on disk or the key is absent
function locres(name) {
  const f = path.join(R, 'assets_converted/loose/KINGDOM HEARTS III/Content/Localization/Game/en', name + '.locres.json');
  const m = new Map(); if (!fs.existsSync(f)) return m;
  const j = JSON.parse(fs.readFileSync(f, 'utf8')); const strs = j.strings || j.stringTable || [];
  for (const ns of j.namespaces) for (const k of ns.keys) {
    let s = k.string !== undefined ? k.string : strs[k.stringIndex];
    if (s && typeof s === 'object') s = s.s !== undefined ? s.s : (s.string && s.string.s !== undefined ? s.string.s : null);
    if (typeof s === 'string') m.set(ns.namespace.s + '.' + k.key.s, s);
  }
  return m;
}
const TXT = locres('kh3_system'), TXT_ITEM = locres('kh3_item');
const clean = (s) => String(s).replace(/<br>/g, ' ').replace(/<[^>]*>/g, '').replace(/\s+/g, ' ').trim();

// ------------------------------------------------------------------ ability query call sites in base/KH3.exe
// pawn vtable slots (offset = slot * 8) and helper functions, all identified in docs/sora-stats-spec.md section 4.1
const VSLOTS = { 0x14e0: 'has', 0x1500: 'count', 0x1508: 'level', 0x28e0: 'count' };
const DIRECT = { 0x1405a03c0: 'count', 0x140e8aab0: 'count', 0x1405ace20: 'has', 0x140580810: 'count', 0x140588690: 'has',
  0x1414358b0: 'has', 0x141424130: 'count', 0x1414242f0: 'level', 0x140eabab0: 'party-has', 0x140ea88e0: 'party-count' };
// functions that only forward a variable id (the API itself, blueprint thunks, debug commands) are not hooks
const NOT_HOOKS = new Set(['0x1405A03C0', '0x140580810', '0x140588690', '0x1405ACE20', '0x1405A4EF0', '0x1405BCB70', '0x1405A5AB0', '0x141424130',
  '0x1414242F0', '0x1414358B0', '0x140EA88E0', '0x140EABAB0', '0x1405A4620', '0x1405A41E0', '0x1405A4220']);
function scanHooks() {
  const img = pe.load(); const base = Number(img.imageBase); const b = img.b; const off = (va) => img.rvaToOff(va - base);
  const fns = [];
  for (const l of fs.readFileSync(path.join(R, 'sheets/native/code.json'), 'utf8').split('\n')) { const m = l.match(/^\["(0x[0-9A-Fa-f]+)","[^"]*",(\d+)/); if (m) fns.push([parseInt(m[1], 16), +m[2]]); }
  fns.sort((a, c) => a[0] - c[0]);
  const fnOf = (va) => { let lo = 0, hi = fns.length - 1; while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (fns[mid][0] <= va) lo = mid; else hi = mid - 1; } return fns[lo]; };
  // game module range only (player, pawn, UI, game-instance code); engine code has no ability queries
  const LO = 0x140400000, HI = 0x141D00000; const o0 = off(LO), o1 = off(HI);
  const sites = new Map();
  const add = (va, kind) => { const f = fnOf(va); if (va >= f[0] + f[1]) return; if (!sites.has(f[0])) sites.set(f[0], { size: f[1], list: [] }); sites.get(f[0]).list.push([va, kind]); };
  for (const dS of Object.keys(VSLOTS)) {
    const pat = Buffer.alloc(4); pat.writeUInt32LE(+dS); let i = o0;
    while ((i = b.indexOf(pat, i)) >= 0 && i < o1) {
      const modrm = b[i - 1], reg = (modrm >> 3) & 7;
      if (b[i - 2] === 0xFF && (modrm & 0xC0) === 0x80 && (modrm & 7) !== 4 && (reg === 2 || reg === 4)) add(LO + (i - o0) - 2, VSLOTS[+dS]);
      i += 1;
    }
  }
  for (let i = o0; i < o1 - 5; i++) { if (b[i] !== 0xE8 && b[i] !== 0xE9) continue; const va = LO + (i - o0); const t = va + 5 + b.readInt32LE(i + 1); if (DIRECT[t]) add(va, DIRECT[t]); }
  const Reg = iced.Register, Mn = iced.Mnemonic, OpK = iced.OpKind; const ins = new iced.Instruction();
  const RDX = new Set([Reg.RDX, Reg.EDX, Reg.DX, Reg.DL]);
  const hooks = new Map(); // ability id -> [[fnHex, siteHex, kind]]
  for (const [fva, info] of [...sites.entries()].sort((a, c) => a[0] - c[0])) {
    const fnHex = '0x' + fva.toString(16).toUpperCase(); if (NOT_HOOKS.has(fnHex)) continue;
    const want = new Map(info.list); const o = off(fva);
    const dec = new iced.Decoder(64, b.subarray(o, o + info.size), iced.DecoderOptions.None); dec.ip = BigInt(fva);
    let edx = null;
    while (dec.canDecode) {
      dec.decodeOut(ins); const ip = Number(ins.ip);
      if (want.has(ip) && edx !== null && nameOf.has(edx) && edx !== 0) { if (!hooks.has(edx)) hooks.set(edx, []); hooks.get(edx).push([fnHex, '0x' + ip.toString(16).toUpperCase(), want.get(ip)]); }
      if (ins.mnemonic === Mn.Call) { edx = null; continue; }
      if (ins.opCount >= 1 && ins.op0Kind === OpK.Register && RDX.has(ins.op0Register) && ins.mnemonic !== Mn.Cmp && ins.mnemonic !== Mn.Test) {
        const k = ins.op1Kind;
        if (ins.mnemonic === Mn.Mov && (k === OpK.Immediate32 || k === OpK.Immediate8 || k === OpK.Immediate32to64 || k === OpK.Immediate64 || k === OpK.Immediate16)) edx = Number(ins.immediate(1) & 0xffffffffn);
        else edx = null;
      }
    }
  }
  return hooks;
}
const HOOKS = scanHooks();

// ------------------------------------------------------------------ authored: effect of every ability
// key: [system, status, ref_conf, effect, test idea, extra hook functions the scan cannot attribute (variable id at the site)]
// status: full = effect and numbers read in code; partial = hook and qualitative effect read, the mechanics belong to another
// spec or were not read to the end; hook-only = call sites known, effect from the game's help text; none = no code reference.
const DMG = '0x140632240';
const A = {
  // ---- movement
  AIR_RECOVERY: ['movement', 'partial', 'inferred', 'Aerial recovery request while blown away: request check 0x14142A3D0 (case 1) needs the damage state flag (+0x8E) and the ability; second gate in the damage reaction code 0x14143D8F0.', 'get launched, press jump: recovery only with the ability', []],
  HIGHJUMP: ['movement', 'full', 'confirmed', 'Held jump = jump kind 1: launch speed from pawn vt+0x1AA0 (0x141434320) = MyJumpPowerHighLv3 1950 cm/s instead of MyJumpPowerNormal 1480; kind selection 0x141434010. MyJumpPowerHigh / Lv2 (1625 / 1790) are not read by the player vtable.', 'hold jump: apex height with and without', ['0x141434320']],
  DOUBLEFLIGHT: ['movement', 'full', 'confirmed', 'Second jump in the air = jump kind 3, launch speed MyJumpPowerDouble 2300 cm/s (0x141434010); request gates in 0x14142A3D0. Also needs the script action flag GW+0x95B0 bit 0 (0x1414358B0).', 'press jump in the air', []],
  SUPERJUMP: ['movement', 'partial', 'inferred', 'Flowmotion jump = jump kind 2, launch speed MyJumpPowerSuper 2500 cm/s (0x141434010), requested from the pole / wall flowmotion states (cases 0x14, 0x16 of 0x14142A3D0) and 0x14143D8F0. Needs GW+0x95B0 bit 7.', 'jump out of a pole spin or wall kick', []],
  SUPERSLIDE: ['movement', 'full', 'confirmed', 'Flowmotion air dash (state 21). Duration by equipped count (pawn vt+0x1AA8, 0x1414354B0): fewer than 2 -> MySuperSlideTime 0.2 s, 2 -> 0.35 s, 3 or more -> 0.5 s, at MySuperSlideSpeed 4000 cm/s; request gate 0x14142E320. Needs GW+0x95B0 bit 7.', 'dash distance per equipped count', []],
  GLIDE: ['movement', 'partial', 'inferred', 'Hold jump in the air -> glide state 4 (MyGlideSpeed 1500 cm/s, MyGlideGravityScale 0.001, MyGlideTurnRate 0.03); request gate 0x141429390. The second copy allowed by m_MaxEquip is not queried by a level call.', 'hold jump while falling', []],
  DODGE: ['movement', 'partial', 'inferred', 'Dodge Roll request (state 16): gate 0x141427CC0 (ground states 8, 11, 12, 18, 19 and the default path).', 'guard button with stick input on the ground', []],
  AIRSLIDE: ['movement', 'full', 'confirmed', 'Air dash (state 20). Root-motion speed scale by equipped count (pawn vt+0x1B90, 0x141432EB0): fewer than 2 -> 0.725, 2 -> 1.075, 3 or more -> 1.425; request gates 0x141425780. Needs GW+0x95B0 bit 1.', 'slide distance per equipped count', []],
  AIRDODGE: ['movement', 'partial', 'inferred', 'Aerial Dodge: the air evade taken by 0x141425780 / 0x141436D20 when Air Slide is not available or in battle states. Needs GW+0x95B0 bit 1.', 'guard button with stick input in the air during a fight', []],
  POLE_SPIN: ['movement', 'partial', 'inferred', 'Flowmotion: air slide / aerial dodge into a vertical pole starts state 24 (TryPoleTurn 0x14143F0C0 tests id 0x5F for a pole).', 'slide into a pole', []],
  POLE_SWING: ['movement', 'partial', 'inferred', 'Flowmotion: horizontal bar variant of TryPoleTurn 0x14143F0C0 (id 0x60).', 'slide into a bar', []],
  WALL_KICK: ['movement', 'partial', 'inferred', 'Flowmotion wall kick (state 23): gate 0x14142F990, also needs GW+0x95B0 bit 7.', 'slide into a wall', []],
  REFLECT_GUARD: ['guard', 'partial', 'inferred', 'Block: guard request 0x1414294C0 refuses guard kind 1 (the plain block) without the ability.', 'press guard: block only with the ability', ['0x1414294C0']],
  // ---- counters / reprisals
  BLOW_COUNTER: ['counter', 'partial', 'inferred', 'Payback Strike: TresPlayerState_AirSlide begin 0x141332930 (the aerial recovery path) arms follow-up command 2 with attack kind 0x27.', 'attack right after an aerial recovery', []],
  GUARD_COUNTER: ['counter', 'partial', 'inferred', 'Counter Slash, one of the three reprisals offered after a successful block; reprisal kind is chosen in 0x1414145C0 (controller+0x8A4 = 3 or 4) and executed by the command dispatcher 0x141437330. No constant-id query of id 4 exists: it is the default reprisal kind.', 'block, then attack', ['0x1414145C0']],
  REVENGEIMPACT: ['counter', 'partial', 'inferred', 'Counter Impact reprisal: tested in 0x1414145C0 (reprisal availability and kind) and in the dispatcher 0x141437330.', 'block, then attack with only this reprisal equipped', []],
  REVENGEDIVE: ['counter', 'partial', 'inferred', 'Counter Kick reprisal: tested in 0x1414145C0 and in the dispatcher 0x141437330.', 'block, then attack with only this reprisal equipped', []],
  REVENGE_EX: ['counter', 'partial', 'inferred', 'Final Blow: follow-up after a reprisal (current command 0xDD..0xE0) armed in 0x141341B10 / 0x141343B00.', 'attack again after a reprisal', []],
  RISKDODGE: ['counter', 'partial', 'inferred', 'Risk Dodge (state 17) after a guard break: gates 0x14142CD20, 0x1413ED1E0, 0x1413C44C0, 0x141433D40.', 'jump right after a guard break', []],
  RD_COUNTER: ['counter', 'none', 'guess', 'No code reference and no name text: unused.', '-', []],
  COUNTER_UP: ['damage', 'full', 'confirmed', 'Reprisal Boost: attacks flagged counter (damage info +0x51 bit 1): damage += ceil(0.5 * count * damage).', 'reprisal damage with and without', []],
  CRITICAL_COUNTER: ['critical', 'full', 'confirmed', 'Critical Counter: a block within CriticalModeJustGuardTime 0.1333 s of the hit marks the guard as just guard (0x141414170, 0x1414145C0, 0x141414B70 set controller+0x878 bit 8 and the state flag); attacks flagged counter + just guard ((+0x51 & 6) == 6) deal damage x2 (0x140632240).', 'block just before the hit, reprisal damage doubles', []],
  // ---- combo
  SLASH_UPPER: ['combo', 'partial', 'inferred', 'Rising Spiral: at combo start the equipped count is passed to pawn vt+0x27B0 (0x14134ACA0, 0x141432780); 0x1414343E0 counts how many of the two combo modifiers are equipped.', 'press jump during a ground combo', []],
  AIR_ROLL_BEAT: ['combo', 'partial', 'inferred', 'Groundbreaker: at combo start the equipped count is passed to pawn vt+0x27C0 (0x14134ACA0, 0x141432780).', 'press jump during an air combo', []],
  AIR_DOWN: ['combo', 'partial', 'inferred', 'Falling Slash (attack kind 0xE) replaces the air attack 0xF when the target is at least 250 cm below, within 400 cm horizontally and grounded (0x14134ACA0 and the formchange combo states).', 'attack from above a grounded target', []],
  TRIPPLE_SLASH: ['finisher', 'partial', 'inferred', 'Speed Slash finisher (kind 5). Finisher pool size 0x141433A60 = 1 + (13 or 14 equipped) + (15 or 110 equipped); choice in 0x14134ACA0.', 'finish a combo next to a target', []],
  CHARGE_THRUST: ['finisher', 'partial', 'inferred', 'Triple Rush finisher (kind 6): taken instead of Speed Slash when the target is farther than 400 cm, when only one target is locked, or when Speed Slash is not equipped (0x14134ACA0).', 'finish a combo from a distance', []],
  MAGICFLUSH: ['finisher', 'partial', 'inferred', 'Magic Flash finisher: second finisher slot of 0x141433A60 / 0x141435090 (shared with Last Charge).', 'second finisher of a combo', []],
  FINAL_HEAVEN: ['finisher', 'partial', 'inferred', 'Last Charge finisher (Re Mind): counted with Magic Flash in the finisher pool (0x141433A60, 0x141435090).', 'second finisher of a combo', []],
  SONIC_SLASH: ['flowmotion-attack', 'partial', 'inferred', 'Flowmotion attack choice 0x1413325F0: Sonic Slash when the target is farther than 800 cm.', 'attack out of flowmotion from far', []],
  SONIC_DOWN: ['flowmotion-attack', 'partial', 'inferred', 'Flowmotion attack choice 0x1413325F0: Sonic Cleave (kind 0x37) when the target is more below than away.', 'attack out of flowmotion above a target', []],
  TURN_CUTTER: ['flowmotion-attack', 'partial', 'inferred', 'Flowmotion attack choice 0x1413325F0: Buzz Saw when two or more targets are in range.', 'attack out of flowmotion near a group', []],
  SUMMERSALT: ['flowmotion-attack', 'partial', 'inferred', 'Somersault: attack request during a bar swing (PoleTurn state 0x1413AA820, id 0x5E).', 'attack while swinging on a bar', []],
  QUICK_SHAFT: ['combo', 'partial', 'inferred', 'Quick Slash: attack kind 1 -> 9 and 0x10 -> 0x18 at combo step selection (0x14134ACA0, 0x1413378B0, form combo states).', 'first ground attack next to a target', []],
  FLASH_STEP: ['combo', 'partial', 'inferred', 'Flash Step: attack kind 2 -> 10 and 0x11 -> 0x19 (0x14134ACA0, 0x1413378B0).', 'first attack on a distant target', []],
  RADIAL_CUT: ['combo', 'partial', 'inferred', 'Radial Blaster: attack kind 3 -> 11 and 0x12 -> 0x1A (0x14134ACA0, 0x1413378B0).', 'first attack among several targets', []],
  AERIAL_SWEEP: ['combo', 'partial', 'inferred', 'Aerial Sweep: attack kind 8 -> 0x1F (0x14134ACA0).', 'attack a target high above', []],
  AERIAL_DIVE: ['combo', 'partial', 'inferred', 'Aerial Dive: attack kind 0xF -> 0x20 (0x14134ACA0, 0x1413378B0).', 'air attack on a target below', []],
  COMBO_PLUS: ['combo', 'full', 'inferred', 'Max combo length (pawn vt+0x1D00, 0x141433570) = 4 + equipped count while grounded (base Sora, no formchange; formchanges use fixed 3 / 5 / 6 / 10).', 'count the hits before the finisher', ['0x141433570']],
  AIRCOMBO_PLUS: ['combo', 'full', 'inferred', 'Max combo length = 4 + equipped count while airborne (0x141433570).', 'count the air hits before the finisher', ['0x141433570']],
  COMBO_MASTER: ['combo', 'partial', 'inferred', 'Combo continues when a swing misses: tested in the attack request paths 0x141426330, 0x14142B3F0, 0x1414426E0.', 'whiff an attack, the combo counter keeps going', []],
  COMBO_UP: ['damage', 'full', 'confirmed', 'Combo Boost: physical ground attacks (info +0x50 bit 7 clear): damage += ceil(0.05 * count * c * damage), c = combo value of the hit (info +0x54, i8).', 'damage of the 4th combo hit with and without', [DMG]],
  AIRCOMBO_UP: ['damage', 'full', 'confirmed', 'Air Combo Boost: the same for hits flagged aerial (info +0x50 bit 7): id = 0x1F + bit.', 'damage of the 4th air combo hit', [DMG]],
  CHARGE_BERSERK: ['cuisine', 'full', 'inferred', 'Berserk Charge: Strength +1 while the MP charge runs (0x140580E70, via the actor interface vt+0xF8) and combos do not end while charging (attack state begin 0x141333210).', 'empty MP, combo length and damage', []],
  AUTO_FINISH: ['situation', 'hook-only', 'inferred', 'Auto-Finish: listed by the HUD ability-state query 0x141616B10 / 0x141669950; the timer-expiry trigger was not read.', 'let a formchange timer run out in battle', []],
  // ---- damage dealt
  FIRE_UP: ['magic', 'full', 'confirmed', 'Attribute 1 (fire) hits: damage += ceil(0.2 * count * damage).', 'Fire damage with 0 / 1 copies', [DMG]],
  BLIZZARD_UP: ['magic', 'full', 'confirmed', 'Attribute 2 (blizzard) hits: damage += ceil(0.2 * count * damage).', 'Blizzard damage', [DMG]],
  THUNDER_UP: ['magic', 'full', 'confirmed', 'Attribute 3 (thunder) hits: damage += ceil(0.2 * count * damage).', 'Thunder damage', [DMG]],
  WATER_UP: ['magic', 'full', 'confirmed', 'Attribute 4 (water) hits: damage += ceil(0.2 * count * damage).', 'Water damage', [DMG]],
  AERO_UP: ['magic', 'full', 'confirmed', 'Attribute 5 (aero) hits: damage += ceil(0.2 * count * damage).', 'Aero damage', [DMG]],
  MAGIC_COMBO_UP: ['magic', 'full', 'confirmed', 'Magic Galvanizer: magic hits (info +0x50 bit 0): damage += ceil(0.1 * count * c * damage), c = combo value of the hit (info +0x54).', 'damage of the 3rd spell of a magic combo', []],
  FULLMP_BURST: ['magic', 'full', 'confirmed', 'Full MP Blast: hits flagged full-MP (info +0x51 bit 4): damage += ceil(0.5 * count * damage).', 'first spell at full MP', []],
  HEARTLESS_BUSTER: ['party', 'full', 'confirmed', 'Target bio type 1: damage = ceil((1 + 0.3 * count) * damage). Party weapon ability.', '-', []],
  NOBODY_BUSTER: ['party', 'full', 'confirmed', 'Target bio type 2: damage = ceil((1 + 0.3 * count) * damage). Party weapon ability.', '-', []],
  JOIN_FORCE: ['party', 'full', 'confirmed', 'Double Team (party members): +5 to the attack stat before the defence subtraction when the target is in the attacker team\'s shared target list.', '-', []],
  // ---- magic / MP
  MP_HASTE: ['mp', 'full', 'confirmed', 'MP charge rate (0x1405AB4D0) = 1 + 0.1 * count(MP_HASTE) + 0.2 * count(MP_HASTERA) + 0.3 * count(MP_HASTEGA).', 'time the recharge', []],
  MP_HASTERA: ['mp', 'full', 'confirmed', 'See MP_HASTE: +0.2 per copy.', 'time the recharge', []],
  MP_HASTEGA: ['mp', 'full', 'confirmed', 'See MP_HASTE: +0.3 per copy.', 'time the recharge', []],
  MP_SAVE: ['mp', 'full', 'confirmed', 'MP Thrift (0x1405A4B90): cost = max(cost - floor(0.2 * count * cost + 0.9), 1).', 'MP after one Fire', []],
  MAGIC_COMBO_SAVE: ['mp', 'full', 'confirmed', 'Magic Combo Thrift (0x1405A4B90): cost = max(cost - combo counter, 1).', 'MP cost of consecutive casts', []],
  MP_SAFETY: ['mp', 'full', 'inferred', 'With it a spell whose cost exceeds the current MP is refused (ability use 0x1405A4EF0, command availability 0x14142F330) unless Extra Cast applies; Cure (cost 255) is never blocked. Shown by the HUD query 0x141616B10.', 'cast with too little MP', []],
  MP_LEAVE: ['mp', 'full', 'confirmed', 'Extra Cast (0x1405BCDC0): when the cost exceeds the current MP and MP > 0, MP is set to 0 without starting the charge; the next cast starts it.', 'cast with too little MP, then once more', []],
  ENDLESS_MAGIC: ['magic', 'partial', 'inferred', 'Sets bit 15 of the FireMagic state flags at Begin (0x14136A8E0): the magic combo never ends with a finisher.', 'cast the same spell many times in a row', []],
  AUTO_LOCK_MAGIC: ['magic', 'partial', 'inferred', 'Sets bit 14 of the FireMagic state flags at Begin (0x14136A8E0): a cast without lock-on locks on to a target automatically.', 'cast without lock-on', []],
  LEAF_VEIL: ['survival', 'partial', 'inferred', 'Leaf Bracer: while a Cure cast runs the damage-invalid bit is set (pawn vt+0x2068(1), cleared at the end): FireMagic 0x14136F130 for Sora, 0x141220930 / 0x141225450 / 0x1411A9250 / 0x1411AD570 for party members.', 'get hit during Cure', []],
  WIZZARD_STAR: ['mp', 'full', 'confirmed', 'Wizard\'s Ruse (0x1405BCDC0): after MP is spent, with probability (count + 1) / 8, HP += MP spent.', 'cast repeatedly, HP sometimes rises', []],
  MAGIC_TIME: ['situation', 'full', 'confirmed', 'Grand Magic Extender (0x14140DA40): the timer of a new grand-magic situation command is multiplied by 1 + 0.2 * count (cap 99.9 s).', 'grand magic timer length', []],
  GRAND_MAGIC: ['cuisine', 'partial', 'inferred', 'More Grand Magic: the grand-magic roll threshold of 0x1413ED500 (10, or 20 for the second tier) is multiplied by 1.3.', 'frequency of grand magic commands', []],
  MAGIC_ROULETTE: ['situation', 'partial', 'inferred', 'Magic Roulette (keyblade ability): 0x1413ED500 offers a -za command without the usual requirement.', '-', []],
  FIRAGAN: ['cuisine', 'partial', 'inferred', 'Firaza weight in the grand-magic roll 0x1413ED500: +15 per count.', '-', []],
  BLIZZAGAN: ['cuisine', 'partial', 'inferred', 'Blizzaza weight in the grand-magic roll 0x1413ED500: +15 per count.', '-', []],
  THUNDAGAN: ['cuisine', 'partial', 'inferred', 'Thundaza weight in the grand-magic roll 0x1413ED500: +15 per count.', '-', []],
  WATAGAN: ['cuisine', 'partial', 'inferred', 'Waterza weight in the grand-magic roll 0x1413ED500: +15 per count (also a keyblade ability).', '-', []],
  AEROGAN: ['cuisine', 'partial', 'inferred', 'Aeroza weight in the grand-magic roll 0x1413ED500: +15 per count.', '-', []],
  CURAGAN: ['cuisine', 'hook-only', 'inferred', 'Curaza situation command: generated in 0x1413ED500 (query with a non-constant id).', '-', ['0x1413ED500']],
  // ---- survival
  LAST_LEAVE: ['survival', 'full', 'inferred', 'Second Chance (0x1405A2A00): a hit that would bring HP below 1 leaves 1 HP when HP before the hit was above 1.', 'take a lethal hit at HP >= 2', []],
  COMBO_LEAVE: ['survival', 'full', 'inferred', 'Withstand Combo (0x1405A2A00): a lethal hit leaves 1 HP while the damage timer pawn+0x848 runs (set to 0.6 s by every hit, counted down in 0x1405AF5B0 only while the ability is equipped) or while the state queue holds TSID_STATE_DAMAGE (19).', 'take two lethal hits within 0.6 s', []],
  DEFENDER: ['survival', 'full', 'inferred', 'Defense +2 when HP% <= 25 with 1 copy, <= 50 with 2, <= 75 with 3, always with 4 (0x140581830).', 'defence value at low HP', []],
  CRITICAL_HALF: ['survival', 'full', 'confirmed', 'Damage Control: at HP% <= 25 damage taken = ceil(damage * 0.5) with one copy, ceil(damage / 3) with two or more (0x140632240; same rule for percent damage in 0x1405A3970).', 'damage taken at low HP', []],
  DAMAGE_ASPIR: ['mp', 'full', 'inferred', 'Damage Syphon (0x1405B58D0): MP += ceil(HP lost / max HP * 100 * count); not while charging.', 'MP after taking a hit', []],
  FIRE_ASPIR: ['mp', 'full', 'inferred', 'Hit of attribute 1: MP += ceil(power * 3 * count) (power = attack power of the hit, 1 for percent damage) (0x1405B58D0).', '-', ['0x1405B58D0']],
  BLIZZARD_ASPIR: ['mp', 'full', 'inferred', 'Hit of attribute 2: MP += ceil(power * 3 * count) (0x1405B58D0).', '-', ['0x1405B58D0']],
  THUNDER_ASPIR: ['mp', 'full', 'inferred', 'Hit of attribute 3: MP += ceil(power * 3 * count) (0x1405B58D0).', '-', ['0x1405B58D0']],
  WATER_ASPIR: ['mp', 'full', 'inferred', 'Hit of attribute 4: MP += ceil(power * 3 * count) (0x1405B58D0).', '-', ['0x1405B58D0']],
  AERO_ASPIR: ['mp', 'full', 'inferred', 'Hit of attribute 5: MP += ceil(power * 3 * count) (0x1405B58D0).', '-', ['0x1405B58D0']],
  DARK_ASPIR: ['mp', 'full', 'inferred', 'Hit of attribute 6: MP += ceil(power * 3 * count) (0x1405B58D0).', '-', []],
  WALK_REGENE: ['mp', 'full', 'inferred', 'MP Walker (0x1405AF5B0): after 5 s of continuous movement, +1 MP every 1 / (count + 1) s; not while charging.', 'run for 10 s with MP missing', []],
  WALK_HEALING: ['survival', 'full', 'inferred', 'HP Walker (0x1405AF5B0): after 5 s of continuous movement, +1 HP every 1 / (count + 1) s.', 'run for 10 s with HP missing', []],
  GUARD_REGENE: ['mp', 'full', 'inferred', 'Block Replenisher: a successful block calls Recover(0, 3, 0) = MP +3 (attack state 0x141331AF0; also tested in the guard paths 0x14143CAF0, 0x1414BD9C0).', 'MP after a block', []],
  BURN_GUARD: ['status', 'full', 'inferred', 'Bad status 17 (burn) is refused: resist test 0x140589610 maps the status to ability 0x3F.', '-', ['0x140589610']],
  CLOUD_GUARD: ['status', 'full', 'inferred', 'Bad status 15 (cloud) is refused (0x140589610, ability 0x40).', '-', ['0x140589610']],
  SNEEZE_GUARD: ['status', 'full', 'inferred', 'Bad status 13 (sneeze) is refused (0x140589610, ability 0x41).', '-', ['0x140589610']],
  FREEZE_GUARD: ['status', 'full', 'inferred', 'Bad status 7 (freeze) is refused (0x140589610, ability 0x42).', '-', ['0x140589610']],
  DISCHARGE_GUARD: ['status', 'full', 'inferred', 'Bad status 16 (discharge) is refused (0x140589610, ability 0x43).', '-', ['0x140589610']],
  STUN_GUARD: ['status', 'full', 'inferred', 'Bad status 12 (stun) is refused (0x140589610, ability 0x44).', '-', []],
  // ---- prizes / items / luck
  PRIZE_DRAW: ['prize', 'full', 'confirmed', 'Treasure Magnet (0x1414794C0): prize pick-up radius by count 0 / 1 / 2+ = 300 / 600 / 900 cm (squared table at 0x1469AB6D8).', 'distance at which prizes fly in', []],
  MAGIC_DRAW: ['prize', 'partial', 'inferred', 'Magic Treasure Magnet: hits get the magic-draw flag in the hit dispatch 0x140618940; prizes of enemies killed that way are drawn from any distance (0x1412A18A0).', 'kill with magic from far', []],
  MASTER_DRAW: ['prize', 'full', 'inferred', 'Master Treasure Magnet (0x1414794C0): no distance limit for the prize draw.', 'prizes fly in from anywhere', []],
  LUCK_UP: ['prize', 'hook-only', 'inferred', 'Lucky Strike: party-wide count read by the drop code 0x140E1DE40, 0x141480420, 0x1417896B0; the rate formula was not read.', '-', []],
  ITEM_UP: ['item', 'full', 'inferred', 'Item Boost: healing item effect x (1 + 0.5 * count) (0x141263950 for command kinds 0x3A..0x3D, 0x42, 0x43; item effect 0x1405BCBE0 / 0x1405BCF10).', 'Potion heal amount', []],
  HARVEST: ['prize', 'hook-only', 'inferred', 'Harvester: party-wide count added to the ingredient pick-up count in 0x14112A760.', '-', []],
  HP_CONVERTER: ['prize', 'full', 'inferred', 'Prize kind 0 (HP small) becomes 1 (HP large) (0x141472DC0).', 'HP prizes are all large', ['0x141472DC0']],
  MP_CONVERTER: ['prize', 'full', 'inferred', 'Prize kind 2 (MP small) becomes 3 (MP large) (0x141472DC0).', 'MP prizes are all large', ['0x141472DC0']],
  FP_CONVERTER: ['prize', 'full', 'inferred', 'Prize kind 7 (focus small) becomes 8 (focus large) (0x141472DC0).', 'focus prizes are all large', ['0x141472DC0']],
  MUNNY_CONVERTER: ['prize', 'full', 'inferred', 'Munny prize kind 4 becomes 5, 5 becomes 6 (0x141472DC0).', '-', ['0x141472DC0']],
  MILLIONAIRE: ['cuisine', 'full', 'inferred', 'Rags to Riches (0x141472DC0): prize kinds 0, 2, 7 become munny 4; kinds 1, 3, 4 become 5; 5 and 6 become 6.', '-', ['0x141472DC0']],
  PRIZE_FEEVER: ['cuisine', 'hook-only', 'inferred', 'Prize Proliferator: party-wide test in the prize spawn code 0x141480420 / 0x141480FB0 (help text: +50% prizes).', '-', []],
  SHARE_PRIZE: ['party', 'partial', 'inferred', 'Share Prizes: 0x141259A50 / 0x14147A070 let a prize the collector cannot use go to a friend.', '-', []],
  EXPZERO: ['exp', 'full', 'inferred', 'Zero EXP (0x140632CB0): the awarded EXP is 0 while equipped.', 'kill an enemy, EXP does not change', []],
  EXP_BARGAIN: ['cuisine', 'full', 'inferred', 'EXP Incentive (0x140632CB0): exp += ceil((0.5 + 0.1 * max(battle level - level, 0)) * exp).', '-', []],
  // ---- HUD / system
  LIBRA: ['hud', 'partial', 'inferred', 'Scan: the target HP gauge of the HUD is shown only with it (0x14165A050).', 'lock on: enemy HP bar visible', []],
  FRIEND_AID: ['hud', 'hook-only', 'inferred', 'Assist Friends: listed by the HUD query 0x141616B10 / 0x141669950 (lets items and Cure target friends).', '-', []],
  BATTLE_GRAPHER: ['system', 'partial', 'inferred', 'Frontline Photographer: reaction request 0x141439A50 keeps the camera mode when staggered.', '-', []],
  CHARISMA_CHEF: ['system', 'hook-only', 'inferred', 'Chef Extraordinaire: read by the four cooking minigames (0x1414A3520, 0x1414A3AB0, 0x1414A3FF0, 0x1414A4520).', '-', []],
  BEST_CONDITION: ['cuisine', 'partial', 'inferred', 'Top Condition: party-wide count in the battle start function 0x140EA31F0 (team HP set to full).', '-', []],
  OVER_TIME: ['cuisine', 'partial', 'inferred', 'Overtime: read by the HUD situation-command code (0x14161AB00, 0x141631600, 0x141650EE0, 0x141662140, 0x1416552C0): a new command extends the timers of the others.', '-', []],
  LUNCH_TIME: ['cuisine', 'none', 'guess', 'Cuisine Extender (+50% meal duration by the help text): no code reference found by the scans.', '-', []],
  POWER_LUNCH: ['cuisine', 'full', 'confirmed', 'Hearty Meal: every meal bonus (max HP, max MP, Strength, Magic, Defense) becomes ceil(bonus * 1.25) (0x140583990, 0x140583B40, 0x140580E70, 0x1405836E0, 0x140581830).', '-', []],
  OVERCOME: ['cuisine', 'full', 'inferred', 'Hidden Potential: Strength and Magic become at least EnemyBaseStatus[battle level].m_BaseDefensePower + ceil(0.15 * battle level + 1.5) (0x140580E70, 0x1405836E0).', '-', []],
  // ---- formchange / attraction / link / focus / team
  FORM_UP: ['situation', 'partial', 'inferred', 'Situation Boost: +0.2 on the situation-gauge gain factor (0x1413E9000).', 'hits needed for a formchange', []],
  FORM_TIME: ['situation', 'full', 'confirmed', 'Formchange Extender (0x14140DA40, site 0x14140DB76): the timer of a new formchange command is multiplied by 1 + 0.2 * count.', 'formchange command timer', ['0x14140DA40']],
  ATTRACTION_TIME: ['situation', 'full', 'inferred', 'Attraction Extender (0x14140DA40): attraction command (kinds 0xBA..0xBF) timer x (1 + 0.2 * count).', '-', []],
  ATTRACTION_UP: ['situation', 'full', 'inferred', 'Attraction Enhancer (0x141432610): chance of the green attraction marker = 0.2 + 0.05 * count (x0.5 in Critical unless a flag), 0 with Critical Converter; attacker count also feeds 0x1406301E0 in the damage apply path.', '-', []],
  LINK_BOOST: ['situation', 'full', 'inferred', 'Link Extender (0x1413F9990): the link timer runs at 0.8x speed.', '-', []],
  FOCUS_ASPIR: ['focus', 'partial', 'confirmed', 'Focus Syphon: adds the free shotlock (0x141411B80; HUD 0x141616B10); absorb rate by count 1 / 2 / 3 = MyFocusAspirRateLv1..3 = 1.4 / 1.9 / 2.4 (0x141433C90). Needs GW+0x95B0 bit 4.', 'lock on with focus 0', []],
  FOCUS_BOOST: ['focus', 'none', 'guess', 'No code reference and no name text: unused.', '-', []],
  POWER_CURE: ['situation', 'full', 'inferred', 'Cure Converter (FireMagic shot 0x14137BB30): a Cure adds (15 * count + 20) * MP / max MP to the situation gauge; 0x1413E9000 lets command 0x7D count for the gauge.', '-', []],
  CRITICAL_CHARGE: ['critical', 'partial', 'inferred', 'Critical Recharge (0x1413E9000): while the MP charge runs the gauge gain uses CriticalChargeStylePointAddRate 1.3.', 'gauge gain while recharging', []],
  // (CC) every AbilityCount(0x6A) site of the exe read (byte scan "mov edx,6Ah" before vt+0x1500): six, all here.
  CRITICAL_CONVERTER: ['critical', 'full', 'inferred', 'Critical Converter: RollGrandMagic 0x1413ED500 returns false when equipped (0x1413EDFED), so AwardAtMax 0x1413F0ED0 offers the formchange at every full gauge, next to any grand magic (port: magic/grand.rs roll_full). The rest is attraction markers, excluded with Attraction Flow (dec029): 0x141432610 chance 0; 0x1413F66D0 marks no enemy for attraction kinds other than 0xBA; 0x1405AF8A0 / 0x1405BDFF0 clear the enemy marker (+0x628 / +0x62C).', 'fill the gauge with magic: the formchange command comes with the grand magic', []],
  UNISON_FIRE: ['team', 'partial', 'inferred', 'Team finisher chance (0x141430A60): (sum of the equipped counts of ids 190..194) * MyFriendComboFinishAddRate 20 * 10, further scaled by the distance bands 800 / 550 / 350 cm.', '-', []],
  UNISON_BLIZZARD: ['team', 'partial', 'inferred', 'See UNISON_FIRE (0x141430A60, id 0xBF).', '-', ['0x141430A60']],
  UNISON_THUNDER: ['team', 'partial', 'inferred', 'See UNISON_FIRE (0x141430A60, id 0xC0).', '-', ['0x141430A60']],
  FUSION_SPIN: ['team', 'partial', 'inferred', 'See UNISON_FIRE (0x141430A60, id 0xC1).', '-', ['0x141430A60']],
  FUSION_ROCKET: ['team', 'partial', 'inferred', 'See UNISON_FIRE (0x141430A60, id 0xC2).', '-', ['0x141430A60']],
  HYPER_HEALING: ['party', 'full', 'inferred', 'Party members: KO time divided by min(count, 3) + 1 (0x1412413C0) and revive HP = ceil(max HP * 25% * (min(count, 3) + 1)) (0x141264140).', '-', []],
  COMBI_UP: ['party', 'partial', 'inferred', 'More Team Attacks: count read in the team-attack roll of the party pawn (0x1412793F0).', '-', []],
  COMBI_TIME: ['party', 'full', 'inferred', 'Team Attack Extender (0x14140DA40): team attack command timer x (1 + 0.5 * count of the friend).', '-', []],
  BEST_COMBI: ['party', 'partial', 'inferred', 'Team Effort: party pawn 0x1411B4E80 requests a team attack command at battle start.', '-', []],
};

// ------------------------------------------------------------------ sora-levels
const LV = levelRows.map((r) => ({ n: +r._row_name, exp: r.m_Exp, atk: r.m_AttackPower, mag: r.m_MagicPower, def: r.m_DefensePower, ap: r.m_AbilityPoint,
  a: short(r.m_AbilityKind1), b: short(r.m_AbilityKind2), c: short(r.m_AbilityKind3) })).sort((x, y) => x.n - y.n);
// (XSI, 2026-10-16) the six slots X1 emptied (Attraction Enhancer / Extender, dec029) hold the XS abilities (XS 4.1, our design)
for (const r of LV) { const slot = XSI.LEVEL_SLOTS[r.n]; if (!slot) continue; r.xsi = []; for (const k of ['a', 'b', 'c']) if (slot[k]) { r.xsi.push(k.toUpperCase() + ' ' + r[k] + ' -> ' + slot[k]); r[k] = slot[k]; } }
const noneDash = (s) => (s === 'NONE' ? '-' : s);
const LEVELS = LV.map((r, i) => { const p = i ? LV[i - 1] : null;
  return [String(r.n), r.exp, p ? r.exp - p.exp : 0, i + 1 < LV.length ? LV[i + 1].exp - r.exp : 0, r.atk, r.mag, r.def, r.ap,
    p ? r.atk - p.atk : 0, p ? r.mag - p.mag : 0, p ? r.def - p.def : 0, p ? r.ap - p.ap : 0, noneDash(r.a), noneDash(r.b), noneDash(r.c),
    'sheets/entities/' + T_LEVEL + ' row ' + r.n + (r.xsi ? '; (XSI) our design, XS_SORA_ABILITIES.md 4.1: ' + r.xsi.join(', ') : ''), '0x1405832D0', 'confirmed']; });
const L99 = LV[98];

// ------------------------------------------------------------------ ability sources
const src = new Map(); const addSrc = (a, s, own) => { if (!a || a === 'NONE') return; if (!src.has(a)) src.set(a, { list: [], copies: 0 }); const e = src.get(a); e.list.push(s); if (own) e.copies += 1; };
for (const a of initList('m_EquipAbility')) addSrc(a, 'start (equipped)', true);
for (const a of initList('m_HaveAbility')) addSrc(a, 'start (owned)', true);
for (const a of initList('m_CriticalEquipAbility')) addSrc(a, 'start in Critical (equipped)', false);
for (const a of initList('m_CriticalHaveAbility')) addSrc(a, 'start in Critical (owned)', false);
{ const per = new Map(); for (const r of LV) for (const [k, col] of [['a', 'A'], ['b', 'B'], ['c', 'C']]) { const a = r[k]; if (a === 'NONE') continue; if (!per.has(a)) per.set(a, { A: [], B: [], C: [] }); per.get(a)[col].push(r.n); }
  for (const [a, p] of per) { addSrc(a, 'level ' + ['A', 'B', 'C'].map((c) => c + ':' + p[c].join('+')).join(' '), false); src.get(a).copies += p.A.length; } }
const VB_SKIP = /debug_(Status|Magic|ItemSlot)|Select/;
for (const r of vbRows) { if (VB_SKIP.test(r._row_name)) continue; for (const a of [short(r.m_AbilitySora1), short(r.m_AbilitySora2)]) addSrc(a, 'victory bonus ' + r._row_name + ' (' + r.m_FlagName + ')', true); }
for (const r of equipRows) { const k = r.m_KeyName; const who = /^WEP_KEYBLADE_SO/.test(k) ? 'keyblade ' : /^PRT_/.test(k) ? 'armour ' : /^ACC_/.test(k) ? 'accessory ' : 'party weapon '; for (const a of items(r.m_AppendAbility)) addSrc(a, who + k, false); }
for (const r of enhRows) addSrc(short(r.m_AppendAbility), 'upgrade ' + short(r.m_WeaponID) + ' level ' + r.m_WeaponLevel, false);
for (const r of mealRows) addSrc(short(r.m_AbilityPlus), 'full-course meal bonus (level ' + r.m_Level + ')', false);
// (XSI) the Elden Ring bosses of the XS rows (sheets/port/sora-er-boss-rewards.json; our design, XS 4.2)
for (const [boss, b] of Object.entries(XSI.BOSS_ROWS)) addSrc(b.add, 'Elden Ring boss ' + boss, false);

// ------------------------------------------------------------------ sora-abilities
// (X1, 2026-10-15) THE USER: "remove all attraction related abilities on Sora, they are useless since we don't even have
// attractions" (sheets/decisions.json dec029; Attraction Flow is excluded from the port). Column `port` = excluded: kh-core
// never grants, lists, equips or loads them; the loadout below does not own them. CRITICAL_CONVERTER stays (it also forces
// the result of RollGrandMagic 0x1413ED500, so it is not attraction-only).
const PORT_EXCLUDED = new Set(['ATTRACTION_TIME', 'ATTRACTION_UP']);
const abilByKey = new Map(abilRows.map((r) => [r.m_KeyName, r]));
const ABILITIES = [];
for (const [name, val] of enumVals) {
  if (name === 'NONE' || /_MAX$/.test(name)) continue;
  const row = abilByKey.get(name.replace(/^__/, ''));
  const s = src.get(name) || { list: [], copies: 0 };
  // (XSI, 2026-10-16) a port value (no TresAbilityData row) and the two KH3 rows the XS abilities switch on
  const xs = XSI.ABILITIES.find((a) => a[0] === name), on = XSI.SWITCHED_ON[name];
  if (xs) {
    ABILITIES.push([name, val, '-', xs[2], xs[4], 'ABILITY', xs[3], 0, 1, 900 + (val - 256), 'sora', s.list, s.copies, xs[5], '(XSI, our design, XS_SORA_ABILITIES.md ' + xs[6] + ') ' + xs[7], xs[8], [], [], 'full',
      'our design: tools/sora_xs_sheet.js (XS_SORA_ABILITIES.md ' + xs[6] + ', approved by the user 2026-10-16); the code: crates/kh-core/src/abilities/xs.rs', 'confirmed', '-', 'ours']);
    continue;
  }
  const owner = on ? 'sora' : val >= 199 ? 'party' : s.list.some((x) => /^(start|level|victory|keyblade|upgrade WEP_KEYBLADE|armour|accessory)/.test(x)) ? 'sora' : s.list.some((x) => /^full-course/.test(x)) ? 'cuisine' : s.list.length ? 'party' : 'unused';
  const a = A[name] || (val >= 199 ? ['party', 'none', 'guess', 'Party-member command ability (MP cost in m_UseMP); not part of Sora\'s set and not analysed.', '-', []] : ['-', 'none', 'guess', 'Not analysed.', '-', []]);
  const scan = HOOKS.get(val) || [];
  const fnSet = [...new Set([...scan.map((h) => h[0]), ...a[5]])].sort();
  const text = row ? TXT.get(row.m_NameTextID) : null; const help = row ? TXT.get(String(row.m_NameTextID).replace('.name_', '.detail_')) : null;
  ABILITIES.push([name, val, row ? row._row_name : '-', text ? clean(text) : '-', row ? short(row.m_Category) : '-', row ? short(row.m_EquipType) : '-',
    on ? on.ap : row ? row.m_UseAP : '-', row ? row.m_UseMP : '-', row ? row.m_MaxEquip : '-', row ? row.m_UIPriority : '-', owner, s.list.length ? s.list : [], s.copies,
    a[0], on ? a[3] + ' (XSI) ' + on.note : a[3], help && help !== text ? clean(help) : '-', fnSet, scan.map((h) => h[0] + '@' + h[1] + ':' + h[2]), a[1],
    'effect: authored from the functions named in it; hooks: decoded from base/KH3.exe on this run (' + scan.length + ' constant-id sites); data: sheets/entities/' + T_ABIL + (on ? '; (XSI) AP and sources: our design, XS_SORA_ABILITIES.md ' + on.section : ''), a[2], a[4], PORT_EXCLUDED.has(name) ? 'excluded' : 'in']);
}

// ------------------------------------------------------------------ sora-stats-rules (authored)
const B = { hp: baseRow.m_MaxHitPoint, mp: baseRow.m_MaxMagicPoint, fp: baseRow.m_MaxFocusPoint };
const RULES = [
  // objects
  ['object.game_instance', 'FUN_141522B30() -> UTresGameInstance (GI); global work GW = GI + 0x870', '-', '0x141522B30, 0x140EA8620', 'pawn vt+0x8C0 returns GI, player pawn vt+0x8C8 returns GW', '0x140EA8620', 'inferred', 'offsets below are given from GW'],
  ['object.game_level', 'GW+0x7DF0 i32 (GI+0x8660)', '-', 'getter 0x140EA85E0; writers 0x140E8CF90, 0x140E908A0', 'ETresGameLevelID 0 Beginner, 1 Standard, 2 Proud, 3 Critical', '0x140EA85E0', 'confirmed', 'head of a 0x20-byte block copied as a whole by the save code'],
  ['object.exp', 'GW+0x7E00 i32 (GI+0x8670)', '-', 'add 0x140E864B0, set 0x140E960E0 (both take GW+0x7DF0 and write +0x10)', 'Sora\'s total EXP; level is derived from it', '0x140E864B0', 'inferred', 'clamped to 0..9999999'],
  ['object.battle_level', 'GW+0x7E09 u8 (GI+0x8679)', '-', 'getter 0x140EA85D0', 'index (1..99) into EnemyBaseStatus', '0x140EA85D0', 'inferred', '-'],
  ['object.growth_type', 'GW+0x7E0C u8', '-', 'setter 0x140E97490', 'ETresPlayerGrowthType from the first opening choice; no reader exists in the executable', '0x140E97490', 'confirmed', 'field scan of the game module: written only'],
  ['object.ability_growth_type', 'GW+0x7E0D u8', '-', 'setter 0x140E97490; read at 0x14058339C and 0x14059EFF0', '0 / 1 / 2 selects m_AbilityKind1 / 2 / 3 of the level row', '0x1405832D0', 'confirmed', '-'],
  ['object.action_flags', 'GW+0x95B0 u8', '-', 'tested by 0x1414358B0, 0x141424130, 0x1414242F0', 'bit0 gates DOUBLEFLIGHT, bit1 AIRSLIDE / AIRDODGE, bit4 FOCUS_ASPIR, bit7 SUPERJUMP / SUPERSLIDE (also WALL_KICK in 0x14142F990); set by the BP_SetAbility* script functions', '0x1414358B0', 'inferred', 'the port must treat all bits as set in a free test level'],
  ['object.save_record', 'GW+0x9640 + index * 0x9C0 (16 records), second array at GW+0x9572AC', '-', '0x140E86C10, 0x140E87AA0', 'per-character persistent record; Sora = index of chr id 1 (0x140E98E90)', '0x140E86C10', 'inferred', 'layout in the spec, section 6'],
  ['object.base_param', 'pawn+0x4D0 UTresChrBaseParam (0x178 bytes)', '-', 'ctor 0x140571980; player init 0x140575C80', '+0x40 level, +0x44 next-level EXP, +0x48 this-level EXP, +0x50 / 54 / 58 max HP / MP / focus, +0x5C / 60 / 64 attack / magic / defense, +0x68 AP, +0x6C / 6E i16 HP / MP bonus, +0x130 save record, +0x138 meal, +0x140 weapon row, +0x148 equipment rows', '0x140575C80', 'inferred', '-'],
  // base values and level
  ['base.max_hp', B.hp, 'HP', 'sheets/entities/' + T_BASE + ' m_MaxHitPoint', 'copied to base param +0x50 by 0x140575C80', '0x140575C80', 'confirmed', '-'],
  ['base.max_mp', B.mp, 'MP', 'sheets/entities/' + T_BASE + ' m_MaxMagicPoint', 'base param +0x54', '0x140575C80', 'confirmed', '-'],
  ['base.max_focus', B.fp, 'FP', 'sheets/entities/' + T_BASE + ' m_MaxFocusPoint', 'base param +0x58; max focus = +0x58 + i16(+0x70) (0x1405838E0); no code adds to +0x70 for Sora', '0x1405838E0', 'confirmed', '-'],
  ['base.row_stats_overridden', true, '-', 'row m_AttackPower 2, m_MagicPower 6, m_DefensePower 2, m_AbilityPoint 2', 'for a character with a level table the level row replaces them (0x140576E70): they never reach the game', '0x140576E70', 'inferred', '-'],
  ['level.from_exp', 'highest L in 1..99 with exp >= row[L].m_Exp', '-', '0x140582FD0', 'row name = decimal level; m_Exp is the cumulative total', '0x140582FD0', 'inferred', 'several levels can be gained by one award: the level is recomputed, not incremented'],
  ['level.row_is_absolute', true, '-', '0x1405832D0 -> 0x140576E70', 'attack, magic, defense and AP of the row are stored as they are (base param +0x5C, +0x60, +0x64, +0x68)', '0x140576E70', 'inferred', '-'],
  ['level.max', 99, 'level', '0x140576E70 clamp, 0x140582FD0 loop bound', '-', '0x140576E70', 'inferred', '-'],
  ['level.99.exp', L99.exp, 'EXP', 'sheets/entities/' + T_LEVEL + ' row 99', 'next-level EXP (base param +0x44) is 0 at level 99', '0x140576E70', 'confirmed', '-'],
  ['level.99.attack', L99.atk, '-', 'row 99 m_AttackPower', '-', '0x1405832D0', 'confirmed', 'level 1: ' + LV[0].atk],
  ['level.99.magic', L99.mag, '-', 'row 99 m_MagicPower', '-', '0x1405832D0', 'confirmed', 'level 1: ' + LV[0].mag],
  ['level.99.defense', L99.def, '-', 'row 99 m_DefensePower', '-', '0x1405832D0', 'confirmed', 'level 1: ' + LV[0].def],
  ['level.99.ap', L99.ap, 'AP', 'row 99 m_AbilityPoint', '-', '0x1405832D0', 'confirmed', 'level 1: ' + LV[0].ap],
  // getters
  ['stat.max_hp', 'base(+0x50) + bonus(i16 +0x6C) + meal HP', 'HP', '0x140583990 (pawn vt+0x1820 0x1405AB200 jumps into it)', 'meal HP = [meal+0x1C], ceil(x * 1.25) with POWER_LUNCH; no equipment term, no cap', '0x140583990', 'inferred', 'SetHitPoint clamps current HP to 0..max'],
  ['stat.max_mp', 'base(+0x54) + bonus(i16 +0x6E) + meal MP', 'MP', '0x140583B40 (pawn vt+0x1828)', 'meal MP = [meal+0x20]', '0x140583B40', 'inferred', '-'],
  ['stat.attack', '((level attack or level-1 attack with PRO code 0) + boost) [x3 with EZ code 0] + weapon.attack + sum(armour, accessory attack); OVERCOME floor; + meal; + 1 with CHARGE_BERSERK while charging; + 5 * buff level', '-', '0x140580E70 (pawn vt+0x1918 0x1405AB800)', 'boost = save record +0x980 (Strength Boost items); buff level = base param +0x88 (0..3, set each tick by 0x140594EF0)', '0x140580E70', 'inferred', 'equipment rows 0..2 (the keyblade slots) are not summed: only the current weapon row +0x140'],
  ['stat.magic', 'the same shape with level magic, boost +0x981, row field +0x28, meal +0x28; no berserk, no buff term', '-', '0x1405836E0 (pawn vt+0x1938 0x1405AB8C0)', '-', '0x1405836E0', 'inferred', '-'],
  ['stat.defense', '(level defense, or 0 with PRO code 1) + boost(+0x982) + meal + weapon.defense + sum(armour, accessory defense) + DEFENDER', '-', '0x140581830 (pawn vt+0x1928 0x1405AB8A0)', 'DEFENDER: +2 when HP% <= 25 / 50 / 75 / 100 for 1 / 2 / 3 / 4 copies', '0x140581830', 'inferred', '-'],
  ['stat.ap_max', 'level AP(+0x68) + boost(save record +0x983) + weapon.AP + sum(armour, accessory AP)', 'AP', '0x1405808B0', 'boost byte is set to m_CriticalUseAPUpNum (30) at record init in Critical (0x140E92280) and raised by AP Boost items', '0x1405808B0', 'inferred', 'AP in use = sum of m_UseAP of equipped copies (menu code not read)'],
  ['stat.resist', 'base(+0xBC + attr * 4) + 100 - ceil(100 * product(1 - row.resist[attr] / 100))', '%', '0x1405850B0', 'product over the weapon row and equipment rows 3..18; damage factor = clamp(100 - resist, 0, 200) / 100', '0x1405850B0', 'inferred', 'Sora base resists are all 0'],
  ['stat.ceil_floor', 'ceil(x) = -(cvtss2si(-0.5 - 2x) >> 1); floor(x) = cvtss2si(2x - 0.5) >> 1', '-', 'engine idiom (FMath::CeilToInt / FloorToInt SSE forms)', 'every rounding in this sheet uses one of the two', '0x140632240', 'confirmed', 'round-to-nearest-even conversion'],
  // bonuses
  ['bonus.hp_up', [3, 5, 10, 15, 30], 'HP', '0x1405864C0 cases 1..5', 'ETresVictoryBonusKind HP_UP3 / 5 / 10 / 15 / 30 at game level < 3', '0x1405864C0', 'inferred', 'Critical: 1 / 2 / 5 / 7 / 15'],
  ['bonus.mp_up', [3, 5, 10, 20], 'MP', '0x1405864C0 cases 8..11', 'MP_UP3 / 5 / 10 / 20 at game level < 3', '0x1405864C0', 'inferred', 'Critical: 1 / 2 / 5 / 10'],
  ['bonus.recount', 'bonus totals are recomputed from the set victory-bonus flags', '-', '0x140E90EE0 (counts at save record +0x960 + kind) and 0x140E8CE20 (sum)', 'the flags live in the game-flag set GW+0x13280; nothing is accumulated in the record', '0x140E90EE0', 'inferred', '-'],
  ['bonus.apply_now', 'TresGetVictoryBonus: max HP / MP bonus is refreshed and current HP / MP rise by the same amount', '-', 'pawn vt+0x1520 0x1405A06B0', '-', '0x1405A06B0', 'inferred', '-'],
  // exp
  ['exp.enemy_reward', 'ceil(m_ExpRate * EnemyBaseStatus[battle level].m_BaseExp)', 'EXP', '0x140576130 (enemy base param +0x4C)', 'm_ExpRate of the enemy base-param row; table Load/Common/EnemyBaseStatus.json', '0x140576130', 'inferred', '-'],
  ['exp.award', '0 if dead or EXPZERO; Beginner exp += ceil(0.25 * exp); Critical exp = ceil(0.75 * exp); EXP_BARGAIN bonus; min(exp, 999)', 'EXP', '0x140632CB0', 'called by GiveExp 0x140E9E830 with the player pawn', '0x140632CB0', 'inferred', 'Standard and Proud: unchanged'],
  ['exp.total_cap', 9999999, 'EXP', '0x140E864B0', '-', '0x140E864B0', 'inferred', '-'],
  ['exp.level_up', 'after the add: for the player and each of the 6 party slots, if 0 < next-level EXP <= total EXP: refresh the base param (pawn vt+0x1960), then show the level-up panel', '-', '0x140E9E830 -> 0x140EBE400', 'panel rows: kind 1 Strength delta, 2 Magic delta, 3 Defense delta, 4 AP delta, 7 new ability (0x140583470 gives the deltas between the rows of level-1 and level)', '0x140EBE400', 'inferred', 'HP and MP are not refilled by a level-up; level 99 fires achievement 0x2D'],
  ['exp.hud_popup', 'EXP number at the enemy position (lifetime 3.0 s) and the next-level counter = next-level EXP - total EXP', '-', '0x141575840, 0x141589570', '-', '0x140E9E830', 'inferred', '-'],
  // hp / mp runtime
  ['hp.set', 'm_HitPoint (pawn+0x5FC) = clamp(v, 0, max HP)', 'HP', 'pawn vt+0x1840 0x1405B9700', 'then the status listener (pawn+0x448) vt+0x220(old HP)', '0x1405B9700', 'inferred', '-'],
  ['hp.damage_apply', 'HP - floor(damage); survival rules; SetHitPoint; damaged notify (vt+0x1198)', '-', '0x1405A2A00', 'order: kill rule of the attack (+0x66), LAST_LEAVE, COMBO_LEAVE, timer reset, invincible checks', '0x1405A2A00', 'inferred', '-'],
  ['hp.combo_leave_timer', 0.6, 's', '0x1405A2A00 writes 0x3F19999A to pawn+0x848 after every hit', 'counted down by 0x1405AF5B0', '0x1405A2A00', 'inferred', '-'],
  ['hp.pinch', 'HP <= max HP * 25 / 100 (integer division)', '-', 'pawn vt+0x2538 0x1405AD600', 'the cockpit (0x1415849A0) starts the HP alert sound (system SE 9, volume 0.8) while it holds', '0x1405AD600', 'inferred', 'the same 25% threshold is used by CRITICAL_HALF and DEFENDER through the HP percent getter'],
  ['mp.charge_time', 50, 's', 'ATresPlayerPawnBase ctor 0x141423BD4 (MyMpChargeTime, pawn+0x60C)', 'see docs/sora-magic-spec.md part C section 3', '0x1405AB250', 'confirmed', '+20 s in Critical'],
  ['prize.values', { hp_small: 1, hp_large: 5, mp_small: 3, mp_large: 5, focus_small: 3, focus_large: 5, munny: [1, 5, 10], mp_small_charge_s: 2, mp_large_charge_s: 6 }, '-', 'pawn vt+0x1898 0x1405AB950', 'while the MP charge runs an MP prize shortens the timer instead', '0x1405AB950', 'inferred', '-'],
  // damage
  ['damage.base', 'max(attacker stat - defender Defense, 1), clamped by the defender\'s m_DamageMin / m_DamageMax rules, times power, ceil', '-', '0x140632240', 'stat = Strength, or Magic when the hit is flagged magic (info +0x50 bit 0)', '0x140632240', 'confirmed', 'Sora row: m_DamageMin 0, m_DamageMax 255'],
  ['damage.min_rule', 'defender m_DamageMin <= 0: min = 1 + ceil(0.1 * battle level) for defender kinds 1 and 5 (enemies), else 13 above battle level 70, else 1; base = max(base, min)', '-', '0x1406325A0..0x14063261A', 'Sora as defender (kind 0, m_DamageMin 0): min 1, or 13 above battle level 70', '0x140632240', 'confirmed', '-'],
  ['damage.max_rule', 'if max > 0 and base > max: base = min(base, max(floor((max + stat - defense) / 2), min))', '-', '0x14063261E..0x140632663', 'for defender kinds 1 and 5 above battle level 70 a second cap max*2 (max*6 with code-menu option B0) applies', '0x140632240', 'confirmed', 'Sora: m_DamageMax 255'],
  ['damage.cap_player', 80, '-', '0x1406326B5 (f32 80.0), test 0x1405891A0', 'defender kinds 0, 3 and 4 (the player and friends): base = min(base, 80) unless min >= 80; applied before the power factor', '0x140632240', 'confirmed', 'so one hit on Sora deals at most ceil(80 * power) before the resist, difficulty and ability steps'],
  ['damage.resist', 'ceil(clamp(100 - resist, 0, 200) / 100 * damage)', '-', '0x140632772', '-', '0x140632240', 'confirmed', '-'],
  ['damage.difficulty', 'ceil(damage * factor(attacker side, game level))', '-', '0x140642820', 'see sora-difficulty', '0x140642820', 'confirmed', '-'],
];

// ------------------------------------------------------------------ sora-difficulty
const DIFF = [
  ['BEGINNER', 0, B.hp, B.mp, [3, 5, 10, 15, 30], [3, 5, 10, 20], 'exp + ceil(0.25 * exp)', 1.25, 0.5, 0, 0, 50, [], 'Standard values except EXP x1.25 and the two damage factors', '0x140642820', 'confirmed'],
  ['STANDARD', 1, B.hp, B.mp, [3, 5, 10, 15, 30], [3, 5, 10, 20], 'exp', 1, 1, 0, 0, 50, [], 'reference', '0x140642820', 'confirmed'],
  ['PROUD', 2, B.hp, B.mp, [3, 5, 10, 15, 30], [3, 5, 10, 20], 'exp', 1, 1.5, 0, 0, 50, [], 'enemy damage x1.5 only', '0x140642820', 'confirmed'],
  ['CRITICAL', 3, Math.floor(B.hp / 2), Math.floor(B.mp / 2), [1, 2, 5, 7, 15], [1, 2, 5, 10], 'ceil(0.75 * exp)', 1, 2, 5, initObj.m_PlayerSora.props.find((x) => x.tag.name === 'm_CriticalUseAPUpNum').value, 70,
    [...initList('m_CriticalEquipAbility'), ...initList('m_CriticalHaveAbility')], 'base HP / MP halved (floor) at 0x140575D73; every HP / MP bonus roughly halved (table 0x1405864C0); enemy attack stat +5 (0x140575ECF); AP +30 (0x140E92280); MP charge +20 s (0x1405AB250); abilities granted through a victory bonus row with m_InitAbilityEquipOnCriticalMode are not granted again (0x140586280)', '0x140575C80', 'inferred'],
];

// ------------------------------------------------------------------ sora-equipment
const itemByKey = new Map(itemRows.map((r) => [r.m_KeyName, r]));
const itemName = (k) => { const r = itemByKey.get(k); if (!r) return '-'; const t = TXT_ITEM.get(r.m_NameTextID) || TXT.get(r.m_NameTextID); return t ? clean(t) : '-'; };
const EQUIP = [];
const resist = (r) => [r.m_AttrResistPhysical, r.m_AttrResistFire, r.m_AttrResistBlizzard, r.m_AttrResistThunder, r.m_AttrResistWater, r.m_AttrResistAero, r.m_AttrResistDark, r.m_AttrResistNoType];
for (const r of equipRows) {
  const k = r.m_KeyName; const kind = /^WEP_KEYBLADE_SO/.test(k) ? 'keyblade' : /^PRT_/.test(k) ? 'armour' : /^ACC_/.test(k) ? 'accessory' : null; if (!kind) continue;
  EQUIP.push([k, kind, r._row_name, itemName(k), r.m_AP, r.m_AttackPlus, r.m_MagicPlus, r.m_DefensePlus, resist(r), items(r.m_AppendAbility), 'sheets/entities/' + T_EQUIP, 'confirmed',
    k === 'WEP_KEYBLADE_SO_00' ? 'Kingdom Key: item id 0x301, ETresItemDefWeapon::WEP_KEYBLADE00, start weapon (m_Weapons of m_PlayerSora) and the fallback when no slot holds a weapon (0x140E86C10)' : '-']);
}
{ let atk = 0, mag = 0; const kk = equipRows.find((r) => r.m_KeyName === 'WEP_KEYBLADE_SO_00');
  for (const r of enhRows.filter((x) => short(x.m_WeaponID) === 'WEP_KEYBLADE00').sort((x, y) => x.m_WeaponLevel - y.m_WeaponLevel)) { atk += r.m_AttackPlus; mag += r.m_MagicPlus;
    EQUIP.push(['WEP_KEYBLADE_SO_00.upgrade.' + r.m_WeaponLevel, 'keyblade-upgrade', r._row_name, 'Kingdom Key +' + r.m_WeaponLevel, 0, kk.m_AttackPlus + atk, kk.m_MagicPlus + mag, 0, resist(kk),
      [...items(kk.m_AppendAbility), ...enhRows.filter((x) => short(x.m_WeaponID) === 'WEP_KEYBLADE00' && x.m_WeaponLevel <= r.m_WeaponLevel && short(x.m_AppendAbility) !== 'NONE').map((x) => short(x.m_AppendAbility))],
      'sheets/entities/' + T_ENH + ' (row adds attack ' + r.m_AttackPlus + ', magic ' + r.m_MagicPlus + ')', 'inferred', 'cumulative totals; the function that builds the upgraded row (override map read by 0x14112EBD0) was not read']); } }
const cnt = (re) => vbRows.filter((r) => !VB_SKIP.test(r._row_name) && (re.test(short(r.m_BonusSora1)) || re.test(short(r.m_BonusSora2)))).length;
EQUIP.push(['slot.keyblade', 'slot', '-', '-', 3, 0, 0, 0, [], [], 'record init 0x140E92280: slot 0 open, slots 1 and 2 opened for the player kind', 'inferred', 'value column ap = slot count; save record +0x80, 8 bytes per slot {i32 item id, u8 flags bit0 open, bit1 locked}; current slot index at record +6']);
EQUIP.push(['slot.armour', 'slot', '-', '-', 1 + cnt(/^DEF_SLOT_UP1$/), 0, 0, 0, [], [], 'UTresChrInitData ctor 0x140571C26 (m_InitProtectorSlot 1) + ' + cnt(/^DEF_SLOT_UP1$/) + ' DEF_SLOT_UP1 rows of TresVBonusData', 'inferred', 'ap = slot count with every story bonus; start 1; save record +0x98, max 8']);
EQUIP.push(['slot.accessory', 'slot', '-', '-', 1 + cnt(/^ACC_SLOT_UP1$/), 0, 0, 0, [], [], 'UTresChrInitData ctor 0x140571C32 (m_InitAccessorySlot 1) + ' + cnt(/^ACC_SLOT_UP1$/) + ' ACC_SLOT_UP1 rows', 'inferred', 'ap = slot count with every story bonus; start 1; save record +0xD8, max 8']);
EQUIP.push(['slot.item', 'slot', '-', '-', 3 + cnt(/^ITEM_SLOT_UP1$/), 0, 0, 0, [], [], 'UTresChrInitData ctor 0x140571C41 (m_InitItemSlot 3) + ' + cnt(/^ITEM_SLOT_UP1$/) + ' ITEM_SLOT_UP1 rows (2 of them Re Mind)', 'inferred', 'ap = slot count with every story and Re Mind bonus; start 3; save record +0x118, max 8']);

// ------------------------------------------------------------------ sora-loadout-99 (test configuration)
const vbSum = (re, tab) => vbRows.filter((r) => re.test(r._row_name)).reduce((s, r) => s + [short(r.m_BonusSora1), short(r.m_BonusSora2)].reduce((t, k) => t + (tab[k] || 0), 0), 0);
const HP_STD = { HP_UP3: 3, HP_UP5: 5, HP_UP10: 10, HP_UP15: 15, HP_UP30: 30 }, HP_CRIT = { HP_UP3: 1, HP_UP5: 2, HP_UP10: 5, HP_UP15: 7, HP_UP30: 15 };
const MP_STD = { MP_UP3: 3, MP_UP5: 5, MP_UP10: 10, MP_UP20: 20 }, MP_CRIT = { MP_UP3: 1, MP_UP5: 2, MP_UP10: 5, MP_UP20: 10 };
const STORY = /^Vbonus_0\d\d$|^VBonus_Minigame/, STORY_NOSEL = { test: (n) => STORY.test(n) && !/^Vbonus_0(79|80|81)$/.test(n) }, DLC = /^VBonus_DLC_0/;
const CHOICE = { vitality: 'Vbonus_079', balance: 'Vbonus_080', wisdom: 'Vbonus_081' };
const kk = equipRows.find((r) => r.m_KeyName === 'WEP_KEYBLADE_SO_00');
const LOAD = [];
const push = (id, value, unit, source, evidence, ref, conf, note) => LOAD.push([id, value, unit, source, evidence, ref, conf, note]);
push('level', 99, 'level', 'test configuration', 'total EXP ' + L99.exp + ' = row 99 of the level table', '0x140582FD0', 'confirmed', '-');
push('exp', L99.exp, 'EXP', 'sheets/entities/' + T_LEVEL + ' row 99', 'GW+0x7E00', '0x140E864B0', 'confirmed', 'any value from ' + L99.exp + ' to 9999999 is level 99');
push('game_level', 'STANDARD', '-', 'test configuration', 'GW+0x7DF0 = 1', '0x140EA85E0', 'confirmed', 'rows *.critical give the Critical variant');
push('choice.default', 'balance + ability path A', '-', 'test configuration', 'the data marks no default: the opening always asks; balance is the middle entry (growth type 2, bonus row Vbonus_080) and path A is m_AbilityKind1; at level 99 all three paths own the same 28 level abilities', '0x140E97490', 'inferred', 'first choice 0 / 1 / 2 = vitality / balance / wisdom sets ETresPlayerGrowthType 0 / 2 / 1; second choice n sets the ability path n (blueprint Levels/ew/ew_01 gameflow)');
for (const [name, row] of Object.entries(CHOICE)) {
  const sel = new RegExp('^' + row + '$');
  for (const [mode, hp0, mp0, ht, mt] of [['standard', B.hp, B.mp, HP_STD, MP_STD], ['critical', Math.floor(B.hp / 2), Math.floor(B.mp / 2), HP_CRIT, MP_CRIT]]) {
    const hpS = vbSum(STORY_NOSEL, ht), mpS = vbSum(STORY_NOSEL, mt), hpC = vbSum(sel, ht), mpC = vbSum(sel, mt), mpD = vbSum(DLC, mt);
    push('hp.' + name + '.' + mode, hp0 + hpS + hpC, 'HP', 'base ' + hp0 + ' + story bonuses ' + hpS + ' + choice ' + hpC, 'rows of sheets/entities/' + T_VB + ' with a Sora HP bonus; amounts from 0x1405864C0', '0x140583990', 'inferred', 'without story bonuses: ' + (hp0 + hpC));
    push('mp.' + name + '.' + mode, mp0 + mpS + mpC + mpD, 'MP', 'base ' + mp0 + ' + story bonuses ' + mpS + ' + choice ' + mpC + ' + Re Mind ' + mpD, 'rows with a Sora MP bonus', '0x140583B40', 'inferred', 'without Re Mind: ' + (mp0 + mpS + mpC) + '; without any story bonus: ' + (mp0 + mpC));
  }
}
push('focus', B.fp, 'FP', 'sheets/entities/' + T_BASE + ' m_MaxFocusPoint', 'no bonus source found', '0x1405838E0', 'confirmed', 'the same in every mode');
push('strength', L99.atk + kk.m_AttackPlus, '-', 'level 99 row ' + L99.atk + ' + Kingdom Key ' + kk.m_AttackPlus, 'no boosts, no armour or accessories, no meal', '0x140580E70', 'inferred', 'fully upgraded Kingdom Key: ' + (L99.atk + kk.m_AttackPlus + 5));
push('magic', L99.mag + kk.m_MagicPlus, '-', 'level 99 row ' + L99.mag + ' + Kingdom Key ' + kk.m_MagicPlus, '-', '0x1405836E0', 'inferred', 'fully upgraded Kingdom Key: ' + (L99.mag + kk.m_MagicPlus + 5));
push('defense', L99.def, '-', 'level 99 row', '-', '0x140581830', 'inferred', '+2 with DEFENDER at low HP');
push('ap', L99.ap, 'AP', 'level 99 row', 'Critical: +30', '0x1405808B0', 'inferred', 'AP Boost items raise the record byte +0x983');
push('weapon', 'WEP_KEYBLADE_SO_00', '-', 'sheets/entities/' + T_INIT + ' m_PlayerSora.m_Weapons', 'Kingdom Key in keyblade slot 0, slots 1 and 2 empty', '0x140E92280', 'confirmed', 'ability from the weapon: ' + items(kk.m_AppendAbility).join(', ') + ' (no AP cost: counted in bits 18..24 of the ability word)');
push('armour', [], '-', 'test configuration', 'empty slots', '-', 'confirmed', '-');
push('accessories', [], '-', 'test configuration', 'empty slots', '-', 'confirmed', '-');
// abilities: everything Sora owns at level 99 with every victory bonus
// (XSI, 2026-10-16) the port's own abilities (column port = ours) are NOT listed here: the deployed DLL reads this sheet at
// run time and refuses a name it does not know. Status::from_loadout gives a loadout the XS abilities of its level path
// from the level table (kh-core status/mod.rs, xsi_level_slots).
const owned = ABILITIES.filter((r) => r[10] === 'sora' && r[12] > 0 && !PORT_EXCLUDED.has(r[0]) && r[22] !== 'ours').map((r) => ({ name: r[0], copies: r[12], ap: r[6], pri: r[9] })).sort((x, y) => x.pri - y.pri);
push('abilities.owned', owned.map((o) => o.name + (o.copies > 1 ? ' x' + o.copies : '')), '-', 'start set + 26 level abilities + Sora victory bonuses (story, minigames, Re Mind); KH3 has 28 level abilities: the port-excluded ATTRACTION_TIME / ATTRACTION_UP are not owned (dec029)', owned.length + ' abilities, ' + owned.reduce((s, o) => s + o.copies, 0) + ' copies; the three level paths give the same set at level 99', '0x140E90EE0', 'inferred', 'copies beyond 3 cannot be stored (3 have / equip bit pairs per ability)');
const apAll = owned.reduce((s, o) => s + o.copies * o.ap, 0);
push('abilities.ap_all', apAll, 'AP', 'sum of m_UseAP over all owned copies', 'more than the ' + L99.ap + ' AP of level 99: not everything can be equipped without AP Boost items', '0x1405808B0', 'inferred', '-');
// default equipped set: fits the level-99 AP without boosts
const OFF = { EXPZERO: 9, MP_SAFETY: 9, REVENGEIMPACT: 9, REVENGEDIVE: 9, LINK_BOOST: 9, UNISON_FIRE: 9, UNISON_BLIZZARD: 9, UNISON_THUNDER: 9,
  FUSION_SPIN: 9, FUSION_ROCKET: 9, FRIEND_AID: 9, MAGIC_TIME: 9, FORM_TIME: 9, AUTO_FINISH: 9, MASTER_DRAW: 9, FOCUS_ASPIR: 2, PRIZE_DRAW: 1,
  // (XSI) the XS abilities of level path A (Spiral Rush 12, Diving Strike 22) are owned, not equipped: the player installs them
  ...Object.fromEntries(XSI.ABILITIES.map((a) => [a[0], 9])), CRITICAL_COUNTER: 9, OVERCOME: 9 };
const eq = owned.map((o) => ({ name: o.name, n: Math.max(0, o.copies - (OFF[o.name] || 0)), ap: o.ap })).filter((o) => o.n > 0);
const apEq = eq.reduce((s, o) => s + o.n * o.ap, 0);
if (apEq > L99.ap) throw new Error('default equipped set needs ' + apEq + ' AP, level 99 has ' + L99.ap);
push('abilities.equipped', eq.map((o) => o.name + (o.n > 1 ? ' x' + o.n : '')), '-', 'test configuration', 'every owned copy except: EXPZERO (would stop EXP), MP_SAFETY (changes the default MP rule), the two extra reprisals (only one can be installed), the link / team / formchange-timer abilities (no partner systems in the test level), Master Treasure Magnet, two of three Focus Syphon and one of two Treasure Magnet', '0x140E8AAB0', 'inferred', 'AP in use ' + apEq + ' of ' + L99.ap);
push('abilities.equipped_ap', apEq, 'AP', 'sum of m_UseAP of abilities.equipped', '-', '0x1405808B0', 'inferred', '-');
push('abilities.critical_extra', [...initList('m_CriticalEquipAbility'), ...initList('m_CriticalHaveAbility')], '-', 'sheets/entities/' + T_INIT + ' m_CriticalEquipAbility, m_CriticalHaveAbility', 'only when game_level is CRITICAL', '0x140E92280', 'inferred', 'AP 5 + 3 + 1');
const mel = (k) => vbRows.filter((r) => STORY.test(r._row_name) && [short(r.m_BonusSora1), short(r.m_BonusSora2)].includes(k)).length;
push('magic.known', { fire: mel('MELEM_FIRE'), blizzard: mel('MELEM_BLIZZARD'), thunder: mel('MELEM_THUNDER'), water: mel('MELEM_WATER'), aero: mel('MELEM_AERO'), cure: mel('MELEM_CURE') }, 'tier', 'count of MELEM_* victory bonuses per element in sheets/entities/' + T_VB, 'tier 3 = -ga; the magic system is specified in docs/sora-magic-spec.md', '0x140E90EE0', 'inferred', 'counts at save record +0x960 + kind (24..29)');
push('shotlocks', ['TEXT_SHOOTFLOW_KB00_00 (King of Hearts)', 'TEXT_SHOOTFLOW_KB00_01 (Ragnarok)'], '-', 'sheets/entities/Load/Tres/TresKeybladeAbilityData.json WEP_KEYBLADE00 m_ShootflowAbilitys', 'plus the Focus Syphon shotlock while FOCUS_ASPIR is equipped', '-', 'confirmed', 'formchange of the Kingdom Key: TRES_WEAR_FORM_05 (Second Form)');
push('item_slots', 3 + cnt(/^ITEM_SLOT_UP1$/), 'slots', 'm_InitItemSlot 3 + ITEM_SLOT_UP1 bonuses', '-', '0x140E98BE0', 'inferred', 'without Re Mind: ' + (3 + cnt(/^ITEM_SLOT_UP1$/) - 2));
push('items', ['ITEM_POTION', 'ITEM_POTION', 'ITEM_HIGHPOTION', 'ITEM_HIGHPOTION', 'ITEM_ETHER', 'ITEM_ETHER'], '-', 'test configuration', 'key names of sheets/entities/' + T_ITEM + '; a choice, not a game default (m_Items of m_PlayerSora is empty)', '-', 'confirmed', 'item effects: docs/sora-magic-spec.md part E');
for (const it of ['ITEM_POTION', 'ITEM_HIGHPOTION', 'ITEM_ETHER']) if (!itemByKey.has(it)) throw new Error('loadout item not in TresItemData: ' + it);

// (magic implementer) The shortcut sets of the test configuration are AUTHORED in their own sheet,
// sheets/port/sora-shortcuts.json (rows shortcuts.a / .b / .c), and carried into the loadout sheet here, so that
// regenerating the loadout never loses them and the loaders keep reading one file. `node tools/sora_stats_sheet.js
// --no-write` does not exist: ANY run of this tool without --check rewrites its six sheets.
{
  const file = path.join(R, 'sheets/port/sora-shortcuts.json');
  if (fs.existsSync(file)) for (const row of JSON.parse(fs.readFileSync(file, 'utf8')).rows) {
    if (!/^shortcuts\.[abc]$/.test(row[0]) || !Array.isArray(row[1]) || row[1].length !== 4) throw new Error('sora-shortcuts: bad row ' + JSON.stringify(row[0]));
    LOAD.push(row);
  }
}

// ------------------------------------------------------------------ write
const SHEETS = [
  ['sora-levels', ['id', 'exp_total', 'exp_from_prev', 'exp_to_next', 'attack', 'magic', 'defense', 'ap', 'd_attack', 'd_magic', 'd_defense', 'd_ap', 'ability_a', 'ability_b', 'ability_c', 'source', 'ref_addr', 'ref_conf'], LEVELS],
  ['sora-abilities', ['id', 'enum_value', 'row', 'name', 'category', 'equip_type', 'ap', 'use_mp', 'max_equip', 'ui_priority', 'owner', 'sources', 'copies', 'system', 'effect', 'help_text', 'hooks', 'hook_sites', 'status', 'evidence', 'ref_conf', 'test', 'port'], ABILITIES],
  ['sora-stats-rules', ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'], RULES],
  ['sora-difficulty', ['id', 'game_level', 'base_hp', 'base_mp', 'hp_up', 'mp_up', 'exp_rule', 'player_damage', 'enemy_damage', 'enemy_attack_add', 'ap_add', 'mp_charge_time', 'start_abilities', 'note', 'ref_addr', 'ref_conf'], DIFF],
  ['sora-equipment', ['id', 'kind', 'row', 'name', 'ap', 'attack', 'magic', 'defense', 'resist', 'abilities', 'source', 'ref_conf', 'note'], EQUIP],
  ['sora-loadout-99', ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'], LOAD],
];
let bad = 0;
for (const [name, columns, rows] of SHEETS) {
  const ids = new Set(); for (const r of rows) { if (r.length !== columns.length) throw new Error(name + ' row width: ' + r[0]); if (ids.has(r[0])) throw new Error(name + ' duplicate id: ' + r[0]); ids.add(r[0]); }
  const out = JSON.stringify({ sheet: name, group: 'port', columns }).replace(/\}$/, ',"rows":[\n') + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n';
  const file = path.join(R, 'sheets/port', name + '.json');
  if (process.argv.includes('--check')) { const same = fs.existsSync(file) && fs.readFileSync(file, 'utf8') === out; console.log(same ? name + ': ' + rows.length + ' rows, current' : name + ': sheet differs from tools/sora_stats_sheet.js ' + VERSION); if (!same) bad = 1; continue; }
  fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, out);
  console.log(name + ': ' + rows.length + ' rows written (tools/sora_stats_sheet.js ' + VERSION + ')');
}
if (!process.argv.includes('--check')) {
  const sora = ABILITIES.filter((r) => r[1] < 199); const by = {};
  for (const r of sora) { by[r[13]] = by[r[13]] || { n: 0, full: 0, partial: 0, hook: 0, none: 0 }; by[r[13]].n++; by[r[13]][r[18] === 'hook-only' ? 'hook' : r[18]]++; }
  console.log('abilities by system (count: full / partial / hook-only / none): ' + Object.keys(by).sort().map((k) => k + ' ' + by[k].n + ': ' + by[k].full + '/' + by[k].partial + '/' + by[k].hook + '/' + by[k].none).join('; '));
}
process.exit(bad);

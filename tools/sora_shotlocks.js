// (SHOT-A) KH3's Keyblade shotlocks (ShootFlow kinds other than the Kingdom Key's) as data for kh-core magic/shotlocks/*:
// sheets/port/sora-shotlocks.json, an id / value sheet in the sora-magic-* format, read by kh-core magic/tables.rs
// `Sheet::Shotlocks`. documentation/KH3_WEAPONS_PLAN.md 1.3 / 4 (SHOT packages), build/sora_tmp/weapons/SHOT-A/STATE.md.
//
//   node tools/sora_shotlocks.js           write the sheet; then node tools/extract.js and node tools/gen_port.js
//   node tools/sora_shotlocks.js --check   fail when the sheet on disk differs from what would be written
//
// Rows written here (the framework; every SHOT package reads them):
//   kind.k<N>.*        every ETresShootFlowKind 1..44: TresShootFlowData (name, LocKey, Fp, Param0), the per-kind set-up of
//                      TresPlayerState_ShootFlow::Begin 0x1413A5CD0 (update delegate slot / address, projectile FName +0xB8,
//                      effect group +0xC0, voice index +0xC4), the family (one Rust module per update function), the
//                      Keyblade(s) that list it and its ability (TEXT_SHOOTFLOW_*: TresAbilityData name / help ids).
//   select.*           SelectKind 0x141411B80: the base kind per weapon type (pawn vt+0x12C8 = ETresItemDefWeapon value),
//                      the MAX kind per weapon type (Sora only), the alternative MAX kinds of 0x1413ECC80, its constants.
//   keyblade.<KEY>.*   the weapon type of each Keyblade (kh3-keyblades weapon_def_value).
//   projset.<w>.<name> each weapon's projectile set (Blueprints/Player/p_ex001/<w>_ProjSet): projectile name -> class.
//   proj.<key>.*       every projectile class of Blueprints/Projectile/ShootFlow and their respawn successors: the class
//                      defaults magic/projectile.rs `StageDef::load_in` reads (the property names of sora-magic-projectiles).
// Rows of the families: tools/shotlocks/<family>.js (see tools/shotlocks/lib.js).
'use strict';
const fs = require('fs');
const path = require('path');
const lib = require('./shotlocks/lib.js');
const { P, readJson, must, row, blueprint, projectileSet } = lib;
const VERSION = '1.0.0';
const OUT = P('sheets', 'port', 'sora-shotlocks.json');

const rows = [];
const add = (...a) => rows.push(row(...a));
const sheetRows = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };

// ------------------------------------------------------------------------------------------------ kinds
// Begin 0x1413A5CD0's switch on +0x8C: the update delegate (thunk -> vtable offset of TresPlayerState_ShootFlow, vtable
// 0x14693CA58, read from base/KH3.exe), the projectile FName stored at +0xB8, the effect group +0xC0 and the voice +0xC4.
// Unlisted kinds (5, 37, 44) take the default branch: update vt+0x3D0, "SF_SHOOTINGSTAR".
const SLOT = { 0x3a8: 0x1413BADB0, 0x3d0: 0x1413BA090, 0x3d8: 0x1413B6320, 0x3e0: 0x1413B6960, 0x3e8: 0x1413B8FC0, 0x408: 0x1413B8C90, 0x410: 0x1413B88F0, 0x448: 0x1413B78B0, 0x468: 0x1413B73D0, 0x488: 0x1413B9910, 0x498: 0x1413B5E30, 0x4b8: 0x1413BC550, 0x4d0: 0x1413B9600, 0x4e0: 0x1413B7B40, 0x4f0: 0x1413BBEE0, 0x500: 0x1413BB3C0, 0x508: 0x1413B7F10, 0x518: 0x1413B6D50, 0x528: 0x1413B9C30 };
// The Rust module (kh-core magic/shotlocks/<family>.rs; "volley" = states/shoot_flow.rs's King of Hearts / Ragnarok code,
// "syphon" = its Focus Syphon) per update function.
const FAMILY = { 0x408: 'volley', 0x410: 'volley', 0x448: 'syphon', 0x468: 'drain', 0x488: 'pellet', 0x3d0: 'meteor', 0x3d8: 'barrage', 0x3e0: 'cannon', 0x498: 'avatar', 0x4b8: 'rush', 0x4d0: 'laser', 0x4e0: 'gate', 0x3a8: 'burst', 0x4f0: 'vacuum', 0x500: 'ultima', 0x3e8: 'aqua', 0x508: 'inception', 0x518: 'kind43', 0x528: 'kind39' };
const K = (slot, proj, group, voice) => ({ slot, proj: proj || '-', group: group || 0, voice: voice || 0 });
const KINDS = {
  1: K(0x408, 'KC_SHOT', 0x631), 2: K(0x410, 'KC_RAGNAROK'), 3: K(0x468, 'HE_DRAIN'), 4: K(0x488), 5: K(0x3d0, 'SF_SHOOTINGSTAR'), 6: K(0x3d8, 'DW_BARRAGE'),
  7: K(0x3e0, 'BZ_CANNON'), 8: K(0x498), 9: K(0x4b8), 10: K(0x468, 'RA_DRAIN'), 11: K(0x4d0), 12: K(0x408, 'MI_SHOT', 0x7a3), 13: K(0x4e0, 'MI_GATE'),
  14: K(0x3a8, 'FZ_BURST', 0x7d5), 15: K(0x4f0, 'FZ_VACUUMWAVE'), 16: K(0x3a8, 'CA_BURST'), 17: K(0x4f0, 'CA_VACUUMWAVE'), 18: K(0x3d0, 'BX_SHOOT', 0x645),
  19: K(0x4e0, 'BX_GATE', 0x645), 20: K(0x408, 'PO_SHOOT', 0x72b), 21: K(0x3d8, 'PO_BARRAGE', 0x71e), 22: K(0x3e0, 'PO_CANNON'), 23: K(0x3d0, 'RE_SHOOT', 0x86c),
  24: K(0x488, null, 0x86d), 25: K(0x498, null, 0x76c, 1), 26: K(0x4b8, null, 0x76c, 1), 27: K(0x3a8, 'SL_SHOT', 0x938), 28: K(0x410, 'SL_RAGNAROK', 0x938),
  29: K(0x500, 'UW_ULTIMA'), 30: K(0x408, 'SN_SHOT', 0x631), 31: K(0x410, 'SN_RAGNAROK'), 32: K(0x408, 'MS_SHOT', 0x631), 33: K(0x410, 'MS_RAGNAROK'),
  34: K(0x408, 'SE_SHOT', 0x631), 35: K(0x410, 'SE_RAGNAROK'), 36: K(0x448, null, 0, 0x35), 37: K(0x3d0, 'SF_SHOOTINGSTAR'), 38: K(0x3e8), 39: K(0x528),
  40: K(0x3d0, 'LF_SHOOT', 0x834), 41: K(0x3a8, 'DF_BURST', 0x898), 42: K(0x508, 'SF_LIGHTINCEPTION'), 43: K(0x518), 44: K(0x3d0, 'SF_SHOOTINGSTAR'),
};
const SFD = new Map(readJson(P('sheets', 'entities', 'Load', 'Common', 'TresShootFlowData.json')).rows.map((r) => [Number(r.id), r]));
const ABIL = readJson(P('sheets', 'entities', 'Load', 'Tres', 'TresAbilityData.json')).rows.filter((r) => /^TEXT_SHOOTFLOW_/.test(r.m_KeyName || ''));
const KB3 = sheetRows('kh3-keyblades');
const KBS = sheetRows('sora-keyblades').filter((r) => r.kind === 'keyblade');
const hex = (n) => '0x' + n.toString(16).toUpperCase();
const BEGIN = '0x1413A5CD0';
for (const [kStr, k] of Object.entries(KINDS)) {
  const kind = Number(kStr);
  const d = SFD.get(kind);
  must(d, 'TresShootFlowData has no row ' + kind);
  const id = (p) => `kind.k${kind}.${p}`;
  const src = `sheets/entities/Load/Common/TresShootFlowData.json row ${kind}`;
  add(id('name'), d.Name.s, '-', src + ' Name', 'the game\'s Japanese name of the kind', '-', 'confirmed');
  add(id('loc_key'), d.LocKey || '-', '-', src + ' LocKey', 'the command name text (kh3_system.command.*)', '-', 'confirmed');
  add(id('fp'), d.Fp, 'int', src + ' Fp', 'the focus shown by the HUD (the aim mode spends focus by aim time, magic spec F)', '-', 'confirmed');
  add(id('param0'), d.Param0, 'count', src + ' Param0', 'max locks (also sora-magic-shotlocks aim2.table.k<N>.param0)', '0x1414165E0', 'confirmed');
  add(id('update_slot'), k.slot, 'offset', BEGIN + ' case ' + kind, 'the per-kind update delegate +0x170 (thunk -> vtable offset of TresPlayerState_ShootFlow 0x14693CA58)', BEGIN, 'confirmed');
  add(id('update'), hex(SLOT[k.slot]), 'addr', 'vtable 0x14693CA58 +' + hex(k.slot) + ' (base/KH3.exe)', 'the update function the delegate calls every Tick (0x1413B0D10)', hex(SLOT[k.slot]), 'confirmed');
  add(id('family'), FAMILY[k.slot], '-', 'derived from the update function', 'the kh-core module that ports it (magic/shotlocks/<family>.rs; volley / syphon: states/shoot_flow.rs)', '-', 'confirmed');
  add(id('proj_name'), k.proj, '-', BEGIN + ' case ' + kind, 'the projectile FName at +0xB8 (resolved by the weapon\'s projectile set); "-" = none set by Begin', BEGIN, 'confirmed');
  add(id('effect_group'), k.group, 'int', BEGIN + ' case ' + kind, 'the effect group +0xC0 every Play of the state passes (0 = Reset\'s)', BEGIN, 'confirmed');
  add(id('voice'), k.voice, 'int', BEGIN + ' case ' + kind, 'the voice index +0xC4 handed to the anim interface (vt+0x1F0) after each Play', BEGIN, 'confirmed');
  // The Keyblades that list it (m_ShootflowAbilitys via kh3-keyblades) and its ability row.
  const ability = ABIL.find((a) => a.m_NameTextID === d.LocKey && a.m_KeyName && KB3.some((kb) => (kb.shotlocks || []).includes(a.m_KeyName)));
  const holders = ability ? KB3.filter((kb) => (kb.shotlocks || []).includes(ability.m_KeyName)) : [];
  if (ability) {
    const kb = holders[0];
    const en = kb.shotlock_names[kb.shotlocks.indexOf(ability.m_KeyName)];
    add(id('ability'), ability.m_KeyName, '-', 'sheets/entities/Load/Tres/TresAbilityData.json row ' + ability._row_name + ' (m_NameTextID = LocKey)', 'the ETresTextAbilityKind the Keyblade lists in m_ShootflowAbilitys: a display ability (m_UseAP 0, category ACTION); the shotlock is the held Keyblade\'s (SelectKind reads only the weapon type)', '-', 'confirmed');
    add(id('name_id'), ability.m_NameTextID, '-', 'TresAbilityData ' + ability._row_name + ' m_NameTextID', 'the shotlock\'s name text', '-', 'confirmed');
    add(id('help_id'), ability.m_HelpTextID, '-', 'TresAbilityData ' + ability._row_name + ' m_HelpTextID', 'the shotlock\'s help text', '-', 'confirmed');
    add(id('name_en'), en, '-', 'sheets/port/kh3-keyblades shotlock_names (WD)', 'the English name', '-', 'confirmed');
    add(id('keyblades'), holders.map((h) => h.id), '-', 'sheets/port/kh3-keyblades shotlocks', 'the Keyblades whose m_ShootflowAbilitys list it', '-', 'confirmed');
  }
}

// ------------------------------------------------------------------------------------------------ SelectKind 0x141411B80
// base: `switch (pawn vt+0x12C8(-1))` when ctrl+0xA34 is 0; MAX (flag 0x40 set, Sora: pawn vt+0xD70 == 1): the second
// switch. Weapon types 3 and 8 pick between two MAX kinds by 0x1413ECC80 (true -> the first).
const BASE = { 1: 1, 2: 3, 3: 5, 4: 8, 5: 10, 6: 12, 7: 14, 8: 20, 9: 18, 10: 16, 11: 23, 12: 25, 13: 40, 14: 41, 15: 29, 16: 30, 17: 32, 18: 27, 19: 34 };
const MAX = { 1: 2, 2: 4, 3: 7, 4: 9, 5: 11, 6: 13, 7: 15, 8: 22, 9: 19, 10: 17, 11: 24, 12: 26, 13: 42, 14: 42, 16: 31, 17: 33, 18: 28, 19: 35 };
const MAX_ALT = { 3: 6, 8: 21 };
const SEL = '0x141411B80';
for (let w = 1; w <= 19; w++) {
  add(`select.base.w${w}`, BASE[w], 'kind', SEL + ' first switch case ' + w + (w === 3 ? ' (the default branch)' : ''), 'the kind while aiming, from the weapon type (pawn vt+0x12C8)', SEL, 'confirmed');
  add(`select.max.w${w}`, MAX[w] === undefined ? BASE[w] : MAX[w], 'kind', SEL + ' second switch case ' + w + (MAX[w] === undefined ? ' (no case: the base kind stays)' : ''), 'the kind once the locks reach Param0 of the base kind (flag 0x40), Sora only (pawn vt+0xD70 == 1)', SEL, 'confirmed');
  if (MAX_ALT[w]) add(`select.max_alt.w${w}`, MAX_ALT[w], 'kind', SEL + ' case ' + w + ': 7 - (0x1413ECC80 != 0) / 0x16 - (..)', 'the MAX kind when 0x1413ECC80 is true', '0x1413ECC80', 'confirmed');
}
const ALT = '0x1413ECC80';
add('select.alt.max_locked_targets', 2, 'count', ALT + ': `2 < ctrl+0xA1C` returns false', 'the alternative needs at most this many locked targets (and at least one)', ALT, 'confirmed');
add('select.alt.up', 1000, 'cm', ALT + ': fStack_20 + 1000.0', 'the sweep ends this far above the first locked target\'s component (vt+0xB88), plus the pawn\'s half height (vt+0x548)', ALT, 'confirmed');
add('select.alt.radius', 30, 'cm', ALT + ': vt+0x2750(.., 0x41F00000 = 30.0, 1)', 'the sphere radius of the sweep 0x141431630 ("Warp" query, channels 0 / 1 / 0x12 blocking, Sora and the target ignored): clear = true', '0x141431630', 'confirmed');
add('select.alt.from_up', 100, 'cm', 'pawn vt+0x2110 = 0x141448550 (the reference point, magic spec F: location + (0, 0, 100))', 'the sweep starts at Sora\'s reference point', '0x141448550', 'inferred', 'sora-magic-shotlocks aim2.lock.entry_ref_height');
add('select.override_ctrl_a34', 0, 'kind', SEL + ': uVar4 = ctrl+0xA34 when non-zero', 'a kind override on the controller; no writer found by the SHOT-A pass: 0 (the weapon type decides)', SEL, 'inferred');

for (const kb of KB3) {
  must(kb.weapon_def_value > 0, kb.id + ': no weapon_def_value');
  add(`keyblade.${kb.id.toLowerCase()}.weapon_type`, kb.weapon_def_value, 'int', 'sheets/port/kh3-keyblades ' + kb.id + ' weapon_def_value (' + kb.weapon_def + ')', 'ETresItemDefWeapon: what pawn vt+0x12C8 returns with the Keyblade held', '0x1405AA8A0', 'confirmed');
}

// ------------------------------------------------------------------------------------------------ projectile sets
const WEAPONS = [...new Set(KBS.map((r) => (r.projectile_set || '').split('/').pop().replace('_ProjSet', '')).filter((w) => /^w_so\d+$/.test(w)))].sort();
const CLASSES = new Set();
for (const w of WEAPONS) {
  for (const [name, cls] of projectileSet(w)) {
    add(`projset.${w}.${name.toLowerCase()}`, cls, '-', 'Blueprints/Player/p_ex001/' + w + '_ProjSet', 'the projectile class the weapon spawns for this name (pawn vt+0x1450 -> equipment -> projectile manager)', '0x1405BDBF0', 'confirmed');
  }
}
const SHOOTFLOW_DIR = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Blueprints', 'Projectile', 'ShootFlow');
for (const f of fs.readdirSync(SHOOTFLOW_DIR).filter((f) => f.endsWith('.uasset.json')).sort()) CLASSES.add(f.replace('.uasset.json', ''));

// ------------------------------------------------------------------------------------------------ projectiles
const tail = (v) => String(v || '').split('::').pop();
const v3 = (o, d) => (o ? [o.x || 0, o.y || 0, o.z || 0] : d || [0, 0, 0]);
const keyOf = (cls) => cls.replace(/_C$/, '').replace(/^TresProj/, '').toLowerCase();
const done = new Set();
function addProj(cls) {
  const name = cls.replace(/_C$/, '');
  if (done.has(name)) return;
  done.add(name);
  const pkg = '/Game/Blueprints/Projectile/ShootFlow/' + name;
  const bp = blueprint(pkg);
  const c = bp.cdo, mv = bp.comps.ProjectileComp || {}, coll = bp.comps.TresAtkColl0 || {}, rootc = bp.comps.TresRoot || {}, pc = bp.comps.ParticleComp || {};
  const key = keyOf(name);
  const src = (what) => 'Blueprints/Projectile/ShootFlow/' + name + ' : ' + what;
  const p = (prop, value, unit, what, evidence, conf, note) => add(`proj.${key}.${prop}`, value, unit, src(what), evidence || 'the class default (magic/projectile.rs StageDef::load_in)', '-', conf || 'confirmed', note);
  const nat = (prop, value, unit, what, ctor) => add(`proj.${key}.${prop}`, value, unit, 'native default (' + ctor + '); ' + what + ' not set by ' + name, 'the class default (magic/projectile.rs StageDef::load_in)', ctor.split(' ').pop(), 'confirmed');
  p('class', name + '_C', '-', 'class', 'the blueprint class (ProjectileView::class, the effect seam name)');
  p('native_class', bp.parent || 'TresProjectileBase', '-', 'super class', 'the native class: TresProj_Ragnarok adds the helix (magic/projectile.rs ProjClass::Ragnarok)');
  p('main_life', c.m_MainLife || 0, 's', 'Default__' + name + '_C.m_MainLife');
  if (c.m_MaxLifeDistance !== undefined) p('max_life_distance', c.m_MaxLifeDistance, 'cm', 'm_MaxLifeDistance');
  p('no_destroy_on_hit', !!c.m_bNoDestroyOnHit, '-', 'm_bNoDestroyOnHit (false when absent: native)');
  p('explode_on_time_out', !!c.m_bExplodeOnTimeOut, '-', 'm_bExplodeOnTimeOut (false when absent: native)');
  if (c.m_AutoStuckShutdownTime !== undefined) p('auto_stuck_shutdown_time', c.m_AutoStuckShutdownTime, 's', 'm_AutoStuckShutdownTime');
  if (c.m_OverrideFinishAttackDataID !== undefined) p('override_finish_attack_data_id', String(c.m_OverrideFinishAttackDataID), '-', 'm_OverrideFinishAttackDataID');
  if (c.m_OverrideSituationAttackDataID !== undefined) p('override_situation_attack_data_id', String(c.m_OverrideSituationAttackDataID), '-', 'm_OverrideSituationAttackDataID');
  p('re_spawn_projectile_on_time_out', !!c.m_bReSpawnProjectileOnTimeOut, '-', 'm_bReSpawnProjectileOnTimeOut (the successor\'s master enable)');
  if (c.m_RespawnCheckType !== undefined) p('respawn_check_type', tail(c.m_RespawnCheckType), '-', 'm_RespawnCheckType');
  const respawn = c.m_ReSpawnProjectileClass ? String(c.m_ReSpawnProjectileClass) : 'null';
  p('re_spawn_projectile_class', respawn, '-', 'm_ReSpawnProjectileClass');
  if (c.m_ReSpawnRotInheritType !== undefined) p('re_spawn_rot_inherit_type', tail(c.m_ReSpawnRotInheritType), '-', 'm_ReSpawnRotInheritType');
  p('re_spawn_take_over_atk_target', !!c.m_bReSpawnTakeOverAtkTarget, '-', 'm_bReSpawnTakeOverAtkTarget');
  p('re_spawn_take_over_atk_coll_hit_list', !!c.m_bReSpawnTakeOverAtkCollHitList, '-', 'm_bReSpawnTakeOverAtkCollHitList');
  p('re_spawn_take_over_atk_coll_finish_flag', !!c.m_bReSpawnTakeOverAtkCollFinishFlag, '-', 'm_bReSpawnTakeOverAtkCollFinishFlag');
  if (c.m_bPermissionLockOn !== undefined) p('permission_lock_on', !!c.m_bPermissionLockOn, '-', 'm_bPermissionLockOn');
  // (SHOT-FIX) attach-to-hit (Explode slot 537 0x141473E30), the life after Shutdown (0x141475670 -> +0x758), the delayed
  // fire (StartFire 0x141474970): read by magic/projectile.rs (attach / linger / fire delay)
  p('attach_to_hit_actor', !!c.m_bAttachToHitActor, '-', 'm_bAttachToHitActor (false when absent: native)', 'Explode attaches the actor to the actor it hit; it rides on it for m_ExplodeLife (magic/projectile.rs explode)');
  if (c.m_ExplodeLife !== undefined) p('explode_life', c.m_ExplodeLife, 's', 'm_ExplodeLife'); else nat('explode_life', 0, 's', 'm_ExplodeLife', 'TresProjectileBase native 0 (docs/sora-magic-spec.md B; read by Shutdown) 0x141475670');
  p('do_delay_fire', !!c.m_bDoDelayFire, '-', 'm_bDoDelayFire (false when absent: native)', 'StartFire waits m_FireDelayTimeMin (random up to Max) before DoFire (magic/projectile.rs start_fire)');
  p('fire_delay_time_min', c.m_FireDelayTimeMin || 0, 's', 'm_FireDelayTimeMin (0 when absent)');
  p('fire_delay_time_max', c.m_FireDelayTimeMax || 0, 's', 'm_FireDelayTimeMax (0 when absent)');
  // Movement (UTresProjectileMovementComponent ctor 0x141470D50: InitialSpeed = MaxSpeed = 2000, everything else 0 / false).
  if (mv.InitialSpeed !== undefined) p('initial_speed', mv.InitialSpeed, 'cm/s', 'ProjectileComp.InitialSpeed'); else nat('initial_speed', 2000, 'cm/s', 'InitialSpeed', 'UTresProjectileMovementComponent ctor 0x141470D50');
  if (mv.MaxSpeed !== undefined) p('max_speed', mv.MaxSpeed, 'cm/s', 'ProjectileComp.MaxSpeed'); else nat('max_speed', 2000, 'cm/s', 'MaxSpeed', 'UTresProjectileMovementComponent ctor 0x141470D50');
  p('accel', mv.m_Accel || 0, 'cm/s2', 'ProjectileComp.m_Accel (0 when absent: native)');
  p('move_start_delay', mv.m_fMoveStartDelay || 0, 's', 'ProjectileComp.m_fMoveStartDelay (0 when absent)');
  p('enable_map_hit_normal_check', !!mv.m_bEnableMapHitNormalCheck, '-', 'ProjectileComp.m_bEnableMapHitNormalCheck (false when absent: native)');
  if (mv.m_CheckMapHitNormal !== undefined) p('check_map_hit_normal', mv.m_CheckMapHitNormal, 'deg', 'ProjectileComp.m_CheckMapHitNormal');
  p('enable_homing', !!mv.m_bEnableHoming, '-', 'ProjectileComp.m_bEnableHoming (false when absent: native)');
  p('homing_start_delay', mv.m_HomingStartDelay || 0, 's', 'ProjectileComp.m_HomingStartDelay');
  p('homing_min_dist', mv.m_HomingMinDist || 0, 'cm', 'ProjectileComp.m_HomingMinDist');
  p('homing_turn_max', mv.m_HomingTurnMax || 0, 'deg/s', 'ProjectileComp.m_HomingTurnMax');
  p('homing_turn_accel', mv.m_HomingTurnAccel || 0, 'deg/s2', 'ProjectileComp.m_HomingTurnAccel');
  p('homing_max_turn_max', mv.m_HomingMaxTurnMax || 0, 'deg/s', 'ProjectileComp.m_HomingMaxTurnMax');
  if (mv.m_HomingMaxAngle !== undefined) p('homing_max_angle', mv.m_HomingMaxAngle, 'deg', 'ProjectileComp.m_HomingMaxAngle');
  p('enable_spin', !!mv.m_bEnableSpin, '-', 'ProjectileComp.m_bEnableSpin (false when absent: native ctor 0x141470EDE)');
  if (mv.m_Spin) p('spin', [mv.m_Spin.pitch || 0, mv.m_Spin.yaw || 0, mv.m_Spin.roll || 0], 'deg/tick', 'ProjectileComp.m_Spin (pitch, yaw, roll)');
  if (mv.m_SpinMaxTime !== undefined) p('spin_max_time', mv.m_SpinMaxTime, 's', 'ProjectileComp.m_SpinMaxTime');
  if (mv.ProjectileGravityScale !== undefined) p('projectile_gravity_scale', mv.ProjectileGravityScale, '-', 'ProjectileComp.ProjectileGravityScale');
  const rootShape = (rootc.CollisionShapesSrc || [])[0];
  p('root_radius', rootShape && rootShape.Size ? rootShape.Size.x || 0 : 0, 'cm', 'TresRoot.CollisionShapesSrc[0].Size.x (0: no root shape)');
  // Attack shapes (TresAtkColl0.CollisionShapesSrc) and the hit effects.
  const shapes = coll.CollisionShapesSrc || [];
  p('hit_shape_count', shapes.length, 'count', 'TresAtkColl0.CollisionShapesSrc');
  shapes.forEach((s, i) => {
    const q = (prop, value, unit, what) => p(`shape${i}_${prop}`, value, unit, `TresAtkColl0.CollisionShapesSrc[${i}].${what}`);
    q('type', tail(s.m_ShapeType) || 'SPHERE', '-', 'm_ShapeType');
    q('attack_id', String(s.m_DefaultAttackDataIDName || 'None'), '-', 'm_DefaultAttackDataIDName');
    q('group', String(s.m_GrpName || 'None'), '-', 'm_GrpName');
    q('size', v3(s.m_Size), 'cm', 'm_Size');
    q('inc_size', v3(s.m_IncSize), 'cm/s', 'm_IncSize');
    q('inc_max_size', v3(s.m_IncMaxSize), 'cm', 'm_IncMaxSize');
    q('scale', v3(s.m_Scale, [1, 1, 1]), '-', 'm_Scale');
    q('inc_scale', v3(s.m_IncScale), '1/s', 'm_IncScale');
    q('inc_max_scale', v3(s.m_IncMaxScale), '-', 'm_IncMaxScale');
    q('offset', v3(s.m_RelativeLocation1), 'cm', 'm_RelativeLocation1');
    q('map_hit_type', tail(s.m_MapHitType) || 'IGNORE_GROUND', '-', 'm_MapHitType');
    if (s.m_SizeVectorCurve) q('size_curve', String(s.m_SizeVectorCurve), '-', 'm_SizeVectorCurve (asset; keys not extracted by SHOT-A)');
  });
  const he = (coll.m_HitEffects || [])[0] || {};
  p('hit_effect', he.m_PawnHitEffect ? String(he.m_PawnHitEffect) : '', '-', 'TresAtkColl0.m_HitEffects[0].m_PawnHitEffect ("" = none)');
  p('hit_sound', he.m_PawnHitSEAsset ? String(he.m_PawnHitSEAsset) : '', '-', 'TresAtkColl0.m_HitEffects[0].m_PawnHitSEAsset ("" = none)');
  p('map_hit_effect', he.m_MapHitEffect ? String(he.m_MapHitEffect) : '', '-', 'TresAtkColl0.m_HitEffects[0].m_MapHitEffect ("" = none)');
  p('particle', pc.Template ? String(pc.Template) : '', '-', 'ParticleComp.Template ("" = none): the body effect');
  const fire = (c.MyFireEffects || [])[0];
  p('fire_effect', fire && fire.MyParticleSystem ? String(fire.MyParticleSystem) : '', '-', 'MyFireEffects[0].MyParticleSystem ("" = none)');
  // TresProj_Ragnarok's helix (native ctor 0x1414700D0: the values of sora-magic-shotlocks kc2.proj_kc_ragnarok.rolling_*,
  // unless the blueprint overrides them).
  if (bp.parent === 'TresProj_Ragnarok') {
    const R = { rolling_speed: ['m_RollingSpeed', 180, 'deg/s'], rolling_speed_add: ['m_RollingSpeedAdd', 1800, 'deg/s2'], rolling_speed_max: ['m_RollingSpeedMax', 1200, 'deg/s'], rolling_radius: ['m_RollingRadius', 100, 'cm'], rolling_radius_add: ['m_RollingRadiusAdd', -200, 'cm/s'], rolling_radius_max: ['m_RollingRadiusMax', 15, 'cm'] };
    for (const [prop, [field, def, unit]] of Object.entries(R)) {
      if (c[field] !== undefined) p(prop, c[field], unit, field); else nat(prop, def, unit, field, 'TresProj_Ragnarok ctor 0x1414700D0');
    }
    add(`proj.${key}.rolling_radius_open_rate`, 200, 'cm/s', 'TresProj_Ragnarok Tick 0x141484643 f32 200', 'the helix radius opening rate (magic/projectile.rs helix)', '0x1414845E0', 'confirmed');
  }
  if (respawn !== 'null' && respawn !== 'None') addProj(respawn);
}
for (const cls of CLASSES) addProj(cls);

// ------------------------------------------------------------------------------------------------ family rows
const FAMILY_DIR = P('tools', 'shotlocks');
for (const f of fs.readdirSync(FAMILY_DIR).filter((f) => f.endsWith('.js') && f !== 'lib.js').sort()) {
  const fam = require(path.join(FAMILY_DIR, f));
  const out = fam(lib);
  must(Array.isArray(out), f + ': must return an array of rows');
  for (const r of out) { must(Array.isArray(r) && r.length === 8, f + ': a row is not lib.row(...)'); rows.push(r); }
}

// ------------------------------------------------------------------------------------------------ write
const ids = new Set();
for (const r of rows) { must(!ids.has(r[0]), 'duplicate row ' + r[0]); ids.add(r[0]); }
rows.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
const sheet = { sheet: 'sora-shotlocks', group: 'port', columns: ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'], rows };
void VERSION;
const text = JSON.stringify(sheet, null, 1) + '\n';
if (process.argv.includes('--check')) {
  const old = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf8') : '';
  if (old !== text) { console.error('sora-shotlocks.json is out of date: run node tools/sora_shotlocks.js'); process.exit(1); }
  console.log('sora-shotlocks.json up to date (' + rows.length + ' rows)');
} else {
  fs.writeFileSync(OUT, text);
  console.log('wrote ' + path.relative(lib.ROOT, OUT) + ' (' + rows.length + ' rows)');
}

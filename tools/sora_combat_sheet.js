// Writes the sheets of Sora's melee combat (docs/sora-combat-spec.md):
//   sheets/port/sora-combat-params.json       fixed parameters read from base/KH3.exe (constructors, state and damage code)
//   sheets/port/sora-combat-attacks.json      one row per attack kind of TresPlayerState_Attack that Sora + Kingdom Key can reach
//   sheets/port/sora-combat-hits.json         one row per TresAnimNotifyState_Attack window of those clips
//   sheets/port/sora-combat-collisions.json   the hit shapes (TresAtkCollComponent.CollisionShapesSrc of the keyblade and the pawn)
//   sheets/port/sora-combat-attack-data.json  the rows of p_ex001_AttackData the melee system uses
//   sheets/port/sora-combat-rules.json        the combo decision procedure as ordered rules
//   sheets/port/sora-combat-reactions.json    Sora's hit reactions: damage kind -> state handler and animation names
//   sheets/port/sora-combat-notifies.json     the anim-notify classes of the combat clips and what they do
// plus the pass-2 sheets (revision 2 of the spec): the tables of tools/lib/sora_combat_pass2.json, one sheet each
//   sheets/port/sora-combat-<table>.json      (32 sheets: request path, movement, hit application, lock-on, reactions, Second Form)
// and the pass-2 parameter rows of the same file, appended to sora-combat-params (a row whose id already exists keeps its
// VALUE: the tool refuses a pass-2 row that would change it; text, address and confidence are taken from pass 2).
// and, with --assets, build/sora_tmp/combat/asset_requests.json (every animation, notify class, effect and sound the
// combat clips and the keyblade reference, for the asset pack).
//
// Rows that state code facts are authored below (address of the function that holds the fact); rows that state data
// are read from the game's own files every time: sheets/entities/DataTable/Player/Attack/p_ex001_AttackData.json,
// assets_converted/packages (blueprints), assets_converted/animations (clips, notifies, root track). Unknown = null.
//
//   node tools/sora_combat_sheet.js [--assets] [--check]
const fs = require('fs'), path = require('path');
const R = path.resolve(__dirname, '..');
const P = path.join;
const sp = require('./sora_pack.js');
const VERSION = '2.0.0';
const PASS2 = JSON.parse(fs.readFileSync(path.join(__dirname, 'lib', 'sora_combat_pass2.json'), 'utf8'));
const CONTENT = sp.CONTENT;
const ANIMSET = '/Game/Character/pc/p_ex001/anm/p_ex001_AnimSet.p_ex001_AnimSet';
const MESH = '/Game/Character/pc/p_ex001/mdl/p_ex001_0.p_ex001_0';
const MESH_YAW = 270; // TresCharMesh0 relative yaw (docs/sora-animation-spec.md): mesh +Y is actor forward
const PKG = 'assets_converted/packages/KINGDOM HEARTS III/Content/';
const C = 'confirmed', I = 'inferred', G = 'guess';
const f32 = (hex) => { const b = Buffer.alloc(4); b.writeUInt32LE(hex >>> 0); return Math.round(b.readFloatLE(0) * 1e6) / 1e6; };
const r4 = (x) => Math.round(x * 1e4) / 1e4;
const strip = (v) => (typeof v === 'string' ? v.replace(/^[A-Za-z0-9_]+::/, '') : v);

// ---------------------------------------------------------------------------------------------------- parameters
// id, value, unit, source, evidence, ref_addr, ref_conf, note
const D = '0x14134ACA0', PLAY = '0x1413378B0', TRAC = '0x141349CC0', MET = '0x141330D30', DMG = '0x140632240', CTOR = '0x141423210', TGT = '0x141404E50', LINIT = '0x1414162A0';
const params = [
  // combo length
  ['combo.base_max', 4, 'count', 'ATresPlayerPawnBase slot 928 0x141433570: return AbilityCount(28 or 29) + 4 (default branch: no formchange command kind, character not Aqua / Riku / Mickey)', 'ability 28 COMBO_PLUS when FUN_1405ADAA0(pawn) is true (on ground), else 29 AIRCOMBO_PLUS; debug line "Combo:%d/%d" of 0x141333B80 prints state+0x128 / this value', '0x141433570', I, 'decompiled C, constants not re-read in the disassembly'],
  ['combo.finishers_base', 1, 'count', 'ATresPlayerPawnBase slot 1253 0x141433A60: 1, +1 with ability 13 or 14, +1 with ability 15 or 110', 'number of finisher steps at the end of the combo; a normal step is chosen while combo < max - finishers', '0x141433A60', I, 'decompiled C'],
  ['combo.second_form_max', 6, 'count', '0x141433570: command kind 0x94..0x98 (CHANGE_L1..L5) returns 6', 'controller slot 846 returns the current formchange command kind', '0x141433570', I, 'Second Form = CHANGE_L1 (148)'],
  ['combo.second_form_finishers', 3, 'count', '0x141433A60: command kind 0x94..0x98 returns 3', '-', '0x141433A60', I, '-'],
  ['swim.exit_delay', 0.133333, 's', 'TresPlayerState_Attack::Tick 0x141341340: comiss with 0.133333 on the timer state+0x254', 'after the first hit window opened (state+0x164) and outside water: time until state+0x234 bit 7, the CANCEL bit, is raised (also raised at TresEnableChange; never by the timer for the last finisher or kinds with bit 8). Guard, dodge roll, air slide and the swim exit test bit 7', '0x141341340', C, 'the id is historical (revision 1 read it as the swim exit only); same constant as attack.cancel_delay; not a combo buffer'],
  // decision thresholds (0x14134ACA0, disassembly)
  ['decide.far_dist', 400, 'cm', D + ': movss xmm6,[145D02630h] = 400, compared with state+0x1CC', 'dist > 400 -> reach 1 (approach attack); also the range limit of rise / air-down / finisher choices', D, C, 'dist = horizontal distance to the lock-on point - 50, floored at 0'],
  ['decide.crowd_count', 2, 'count', D + ': cmp [rsi+1C4h],1 / jle', 'more than 1 lock-on target within 400 cm (counted by the controller target update) -> reach 2 (crowd attack)', D, C, 'count = controller+0x95C'],
  ['decide.crowd_radius', 400, 'cm', TGT + ': if (dist3D < 400) ++controller+0x95C', 'raw 3D distance from the search origin (root + 100 up + facing * 30), counted before the Enemy / priority / line-of-sight tests', TGT, C, 'same constant as target.crowd_range'],
  ['decide.dist_offset', 50, 'cm', MET + ': subss 50 from the horizontal distance, clamp at 0', 'state+0x1CC', MET, C, '-'],
  ['decide.air_below_dz', -100, 'cm', D + ': comiss with -100 on state+0x1D0 (target Z - self Z)', 'airborne and target more than 100 cm below -> falling attack (kind 0x0F) or air-down (0x0D / 0x0E)', D, C, '-'],
  ['decide.air_down_dz', -250, 'cm', D + ': comiss with -250 at 0x14134B2B8 and 0x14134B40B', 'air-down needs ability 12, dz <= -250, target on ground, previous kind not 0x0D / 0x0E, dist <= 400', D, C, '-'],
  ['decide.rise_dz', 160, 'cm', 'ATresPlayerPawnBase ctor ' + CTOR + ': mov dword [pawn+0BA8h],43200000h at 0x14142380E', 'TargetMetrics ' + MET + ' sets state+0x1E0 when dz > pawn+0xBA8; then kind 8 (rise) if dist <= 400 and the previous kind is not 8, 0x17, 0x1F', MET, C, '-'],
  ['decide.rise_speed', 700, 'cm/s', D + ': DirZ = pawn+0xBAC * 200 (0x14134B3AA); pawn+0xBAC = 3.5 (ctor 0x141423818)', 'upward traction speed of the rise attack (state+0x1D4)', D, C, 'derived 3.5 * 200'],
  ['decide.ground_probe_range', 800, 'cm', 'PreDecide 0x14134C420: ground probe 0x14063A650 with range 800', 'bAir = false only when the probe hits within 30 cm and the floor normal test passes', '0x14134C420', C, 'the probe is the sphere sweep of docs/sora-movement-spec.md 4.5'],
  ['decide.ground_probe_max', 30, 'cm', 'PreDecide 0x14134C420: comiss with 30', '-', '0x14134C420', C, '-'],
  ['decide.launch_air_dz', 100, 'cm', D + ': comiss with 100 at 0x14134B06D', 'launch request in the air with AIR_ROLL_BEAT available and dz < 100 -> kind 0x1D, else 0x1C', D, C, 'launch request = jump button during a combo, see spec 3.4'],
  // facing and traction
  ['face.rate', 12, '1/s', 'Facing 0x14134D0E0: alpha = dt * 12; 1.0 on the tick where the FaceAt bit is consumed', 'pawn slot 438 Turn(yaw, alpha) toward the lock-on point; yaw only unless state+0x1F4 bit 5', '0x14134D0E0', C, '-'],
  ['face.timer', 0.2, 's', 'Facing 0x14134D0E0: slot 127 (enable, 0.2) when the FaceAt bit (state+0x84 bit 4) is consumed', 'facing stays on for this time unless state+0x1F4 bit 4 (whole attack)', '0x14134D0E0', C, '-'],
  ['traction.speed', 800, 'cm/s', TRAC + ': target speed 800 when state+0x150 == 0 and dist > state+0x24C', 'horizontal homing speed while the TresTraction notify window is open', TRAC, C, 'state+0x150 overrides it (kind 0x17: 800; flag 0x1000: 2000)'],
  ['traction.stop_dist', 160, 'cm', PLAY + ': mov dword [rsi+24Ch],43200000h (default per attack)', 'homing target speed becomes 0 at or inside this distance to the lock-on point (clamped distance, no -50)', TRAC, C, 'per-kind overrides in sora-combat-attacks.stop_dist'],
  ['traction.rate_on', 0.2, '-', TRAC + ': rate 0.2 (xmm13) while homing', 'v += (target - v) * min(rate * dt * 60, 1); snap when |delta| < 0.0003 (slot 135 0x1413364D0)', TRAC, C, '-'],
  ['traction.rate_off_ground', 0.5, '-', TRAC + ': rate 0.5 after the window (ground attack)', 'target speed 0', TRAC, C, '-'],
  ['traction.rate_off_air', 0.1, '-', TRAC + ': rate 0.1 after the window when state+0x1DF (air mode) and not the first air attack', '-', TRAC, C, '-'],
  ['traction.first_air_scale', 1.5, '-', 'ATresPlayerPawnBase ctor ' + CTOR + ': mov dword [pawn+0BB0h],3FC00000h at 0x141423822', 'first attack of a combo started in the air: speed and rate multiplied by pawn+0xBB0', TRAC, C, 'unless state+0x234 bit 18'],
  ['traction.dist_clamp', 800, 'cm', TRAC + ': horizontal distance clamped to 0..800', '-', TRAC, C, '-'],
  ['traction.dz_clamp', 500, 'cm', TRAC + ': dz clamped to -500..500', '-', TRAC, C, '-'],
  ['traction.z_gain', 3.0, '1/s', PLAY + ': mov dword [rsi+14Ch],40400000h (default per attack)', 'vertical homing speed = clamp(dz) * gain while the window is open (air mode attacks without fixed DirZ)', TRAC, C, '4.5 for kinds 0x14, 0x15, 0x34, 0x36'],
  ['traction.z_initial_gain', 5, '1/s', MET + ': DirZ = clamp(dz * 5, -1200, 1200) when |dz| > 30', 'initial state+0x1D4', MET, C, '-'],
  ['traction.z_initial_max', 1200, 'cm/s', MET + ': clamp +-1200', '-', MET, C, '-'],
  ['traction.z_decay', 10, '1/s', TRAC + ': after the window DirZ -= dt * DirZ * 10 until it crosses 0', '-', TRAC, C, '-'],
  ['air.gravity_scale', 0.1, '-', PLAY + ' tail: pawn slot 714 (GravityScale = 0.1) when state+0x1DF', 'air-mode attacks: gravity disabled (movement+0xA3C bit 1) and movement mode 5 (Flying) while the Z channel is active', PLAY, C, 'restored at the end of the traction window: gravity on, mode 3 (Falling)'],
  ['air.hold_time', 0.6, 's', PLAY + ' tail: mov dword [rsi+18Ch],3F19999Ah when state+0x1DF', 'state+0x18C: arming value of the cool-downs End starts when an air attack whiffed (cleared to 0 by a landed hit)', PLAY, C, 'not a hover time; same constant as attack.air.cooldown_arm; End 0x141335120 reads it'],
  ['root.scale_rule', 'dist / root_len', '-', TRAC + ': FUN_1405A1D30(pawn+0x440, scaleXY, 1.0, 0) with scaleXY = clamped distance / state+0x1D8', 'only when state+0x234 bit 1; 1.0 with no target; 1.0 again when the traction window closes', TRAC, C, 'no clamp anywhere on the path (root_scale.clamp); horizontal only; 0 when the target is exactly above / below'],
  // hit confirmation
  ['hitstop.small', 0.02, 's', 'ATresPlayerPawnBase ctor ' + CTOR + ': MyHitStopTimeSmall (pawn+0x1574) at 0x141423AA3', 'reflected property; not overridden in p_ex001_PawnBase / p_ex001_Pawn', CTOR, C, 'NOT APPLIED by the game: the only reader is pawn slot 1374 0x141444090, which nothing calls (gain.hitstop_applied = 0); the port must not add a hit-stop'],
  ['hitstop.big', 0.05, 's', 'ATresPlayerPawnBase ctor ' + CTOR + ': MyHitStopTimeBig (pawn+0x1578) at 0x141423AAD', '-', CTOR, C, 'NOT APPLIED by the game (see hitstop.small)'],
  // formchange gauge (style points)
  ['style.max', 100, 'count', 'ATresPlayerPawnBase ctor ' + CTOR + ': MyStylePointMax (pawn+0xCA8) at 0x141423BDE', 'reflected property', CTOR, C, 'formchange gauge'],
  ['style.add_rate', 1.0, '-', 'ctor ' + CTOR + ': MyStylePointAddRate (pawn+0xCAC) at 0x141423BE8', 'multiplier on TresAttackDataTable.m_FormPoint in AddStylePoint 0x1413E9000 (granted only when the victim lost HP)', CTOR, C, 'same constant as form.gauge.add_rate'],
  ['style.add_rate_lv0', 1.0, '-', 'ctor: MyStylePointAddRateLv0 (pawn+0xCB0)', 'level = Param1 of the current form command row: 0 outside a form', CTOR, C, '-'],
  ['style.add_rate_lv1', 0.8, '-', 'ctor: MyStylePointAddRateLv1 (pawn+0xCB4)', 'level 1 = in Second Form', CTOR, C, '-'],
  ['style.add_rate_lv2', 0.55, '-', 'ctor: MyStylePointAddRateLv2 (pawn+0xCB8)', '-', CTOR, I, '-'],
  ['style.dec_start_time', 20, 's', PKG + 'Blueprints/Player/p_ex001/p_ex001_PawnBase.uasset.json Default__p_ex001_PawnBase_C MyStylePointDecStartTime', 'native default 10 (ctor 0x141423C64)', CTOR, C, '-'],
  ['style.dec_interval', 1.0, 's', 'ctor: MyStylePointDecInterval (pawn+0xCE4) at 0x141423C6E', '-', CTOR, C, '-'],
  ['style.dec', 5, 'count', PKG + 'Blueprints/Player/p_ex001/p_ex001_PawnBase.uasset.json MyStylePointDec', 'native default 10 (ctor 0x141423C78)', CTOR, C, '-'],
  ['guard.hold_time', 3, 's', PKG + 'Blueprints/Player/p_ex001/p_ex001_PawnBase.uasset.json MyGuardHoldTime', '-', CTOR, I, 'guard state is specified by another agent'],
  ['reaction.cooldown', 1.5, 's', PKG + 'Blueprints/Player/p_ex001/p_ex001_PawnBase.uasset.json MyReactionCoolDownTime', 'controller slot 676 0x141415A50 replaces its argument by 1.0 only when this property is 1.0; for Sora (1.5) the argument is the command cool-down ctrl+0x8C8', CTOR, C, 'same constant as command.reaction_cooldown_time'],
  // damage formula (0x140632240, disassembly)
  ['damage.min_base', 1, '-', DMG + ': x = max(A, 0) - D; if x < 1: x = 1', 'A = attacker strength (magic power for a magic attack), D = defender defense', DMG, C, '-'],
  ['damage.epsilon', 0.000001, '-', DMG + ': x = CeilToInt((x - 1e-6) * (power - 1e-6))', 'power = TresAttackDataTable.m_Power of the row', DMG, C, '-'],
  ['damage.resist_max_percent', 200, '-', DMG + ': m = clamp(100 - resist[attribute], 0, 200) * 0.01', 'defender attribute resistance (status+0xBC[attr] with equipment), FUN_1405850B0', DMG, C, '-'],
  ['damage.combo_up_per_step', 0.05, '-', DMG + ': bonus = step * AbilityCount(31 or 32) * 0.05', 'COMBO_UP (ground) / AIRCOMBO_UP (aerial flag): x += CeilToInt(bonus * x); step = TresDamageInfo+0x54', DMG, C, '-'],
  ['damage.counter_up', 0.5, '-', DMG + ': counter attack flag and AbilityCount(69): x += CeilToInt(n * 0.5 * x)', 'COUNTER_UP', DMG, C, '-'],
  ['damage.critical_counter', 2, '-', DMG + ': counter + just-guard flags and ability 104: x *= 2', 'CRITICAL_COUNTER', DMG, C, '-'],
  ['damage.attribute_up', 0.2, '-', DMG + ': attribute 1..5 and AbilityCount(32 + attribute): x += CeilToInt(n * 0.2 * x)', 'FIRE_UP .. AERO_UP', DMG, C, 'not reached by physical attacks'],
  ['damage.buster', 0.3, '-', DMG + ': x = CeilToInt((n * 0.3 + 1) * x) for ability 101 vs status+0x7D == 1, ability 102 vs == 2', 'HEARTLESS_BUSTER / NOBODY_BUSTER', DMG, C, '-'],
  ['damage.rapid_fire_resist_field', '0xDC', '-', DMG + ': rapid-fire flag: x = CeilToInt(clamp(100 - status+0xDC, 0, 100) * 0.01 * x)', '-', DMG, C, '-'],
  ['damage.zero_result', 0.1, '-', DMG + ': returns 0.1 when the result is 0', 'so that TakeDamage does not discard the hit', DMG, C, '-'],
  ['damage.random', 'none', '-', DMG + ': no random number is drawn in the formula', 'TresAttackDataTable.m_DamageExecRate (0..100) is a separate roll in 0x140618940 before the formula', DMG, C, 'the roll is the engine xorshift128 (hit.exec_rate_roll_scale); never rolled for Sora: every row has rate 100'],
  // targeting
  ['target.near_range', 900, 'cm', LINIT + ': mov dword [ctrl+970h],44610000h', TGT + ': half of it gates non-"Enemy" targets; priority override below it', LINIT, C, '-'],
  ['target.min_dist', 150, 'cm', LINIT + ': mov dword [ctrl+974h],43160000h', TGT + ': a target whose surface distance is below it is scored with dist*0.01 + 150', LINIT, C, '-'],
  ['target.max_range', 3500, 'cm', LINIT + ': mov dword [ctrl+978h],455AC000h', TGT + ': the best candidate becomes the soft target only when nearer', LINIT, C, '-'],
  ['target.switch_range', 2400, 'cm', LINIT + ': mov dword [ctrl+9C8h],45160000h', TGT + ': candidates for target switching', LINIT, I, 'decompiled C for the use'],
  ['target.facing_weight', 1.8, '-', TGT + ': score = (1.8 - dot(searchDir, dirToTarget2D)) * dist', 'lowest score wins among equal priority, line-of-sight test 0x1413F0750 last', TGT, C, 'searchDir = the facing of Sora (root forward, flattened); not the stick, not the camera'],
  ['target.origin_push', 30, 'cm', TGT + ': origin = pawn position + searchDir * 30', '-', TGT, I, 'decompiled C'],
];

// ---------------------------------------------------------------------------------------------------- attack kinds
// kind, label, anim (ground) , anim (air or null), phase, role, requires, rootLenHex|null, extra: { air_mode, keep_face, stop_dist, z_gain, flags }
const K = (kind, label, anim, phase, role, requires, rootHex, x = {}) => ({ kind, label, anim, phase, role, requires, rootHex, ...x });
const attacks = [
  K(0x01, 'ground 1 (normal)', '1301', 'ground', 'combo', '-', null),
  K(0x02, 'ground approach (far)', '1302', 'ground', 'approach', '-', null),
  K(0x03, 'ground crowd', '1303', 'ground', 'crowd', '-', 0x434F0000, { note: 'root length and root-scale mode (traction channel off) only when dist < 300' }),
  K(0x04, 'ground finisher', '1304', 'ground', 'finisher', '-', 0x4402C000),
  K(0x05, 'finisher: triple slash', '1305', 'ground', 'finisher', 'TRIPPLE_SLASH (13)', null),
  K(0x06, 'finisher: charge thrust', '1306', 'ground', 'finisher', 'CHARGE_THRUST (14)', null),
  K(0x07, 'finisher: magic flash', '1307', 'ground', 'finisher', 'MAGICFLUSH (15)', null),
  K(0x08, 'rise (target above)', '1308', 'either', 'rise', '-', null, { air_mode: true, note: 'DirZ fixed 700 cm/s (state+0x1DD)' }),
  K(0x09, 'quick shaft (replaces 1)', 'M_1359', 'ground', 'combo', 'QUICK_SHAFT (107)', 0x43B40000, { keep_face: true, note: 'state+0x25C = 0.2333 s' }),
  K(0x0A, 'flash step (replaces 2)', 'M_1360', 'ground', 'approach', 'FLASH_STEP (108)', 0x442A0000, { keep_face: true, note: 'slot 142 (effect group on) called at start' }),
  K(0x0B, 'radial cut (replaces 3)', 'M_1361', 'ground', 'crowd', 'RADIAL_CUT (109)', 0x43820000, { keep_face: true }),
  K(0x0C, 'finisher: final heaven', 'M_1362', 'ground', 'finisher', 'FINAL_HEAVEN (110)', 0x43E10000, { keep_face: true }),
  K(0x0D, 'air down', '1318', 'air', 'down', 'AIR_DOWN (12)', 0x43480000, { air_mode: true, keep_face: true, stop_dist: 0x437A0000, note: 'dive: state+0x1DE; ends on ground contact with the shot handler 0x141344580 (shockwave)' }),
  K(0x0E, 'air down (finisher)', '1318', 'air', 'finisher', 'AIR_DOWN (12)', 0x43480000, { air_mode: true, keep_face: true, stop_dist: 0x437A0000, note: 'attack row KB_DOWN_FIN (0x141343800)' }),
  K(0x0F, 'air falling attack (target below)', '1310', 'air', 'combo', '-', 0x43CF0000, { air_mode: true, keep_face: true }),
  K(0x10, 'air 1 (normal)', '1311', 'air', 'combo', '-', 0x43520000, { air_mode: true, keep_face: true }),
  K(0x11, 'air approach (far)', '1312', 'air', 'approach', '-', 0x441A8000, { air_mode: true, keep_face: true }),
  K(0x12, 'air crowd', '1313', 'air', 'crowd', '-', 0x43520000, { air_mode: true, keep_face: true, note: 'root length only when dist < 300 (traction channel off then)' }),
  K(0x13, 'air finisher', '1314', 'air', 'finisher', '-', 0x44098000, { air_mode: true, keep_face: true }),
  K(0x14, 'air finisher: triple slash', '1315', 'air', 'finisher', 'TRIPPLE_SLASH (13)', 0x43D78000, { air_mode: true, keep_face: true, z_gain: 0x40900000, note: 'state+0x23C bit 3: the Z channel re-arms at each traction window' }),
  K(0x15, 'air finisher: charge thrust', '1316', 'air', 'finisher', 'CHARGE_THRUST (14)', 0x44348000, { air_mode: true, keep_face: true, z_gain: 0x40900000, note: 'state+0x23C bit 3' }),
  K(0x16, 'air finisher: magic flash', '1317', 'air', 'finisher', 'MAGICFLUSH (15)', 0x43A18000, { air_mode: true, keep_face: true }),
  K(0x17, 'air 2 (rise clip, after 0x0F / 0x10 / 0x20)', '1308', 'air', 'combo', '-', null, { air_mode: true, keep_face: true, note: 'traction speed state+0x150 = 800' }),
  K(0x18, 'air quick shaft (replaces 0x10)', 'M_1363', 'air', 'combo', 'QUICK_SHAFT (107)', 0x43870000, { air_mode: true, keep_face: true, note: 'state+0x25C = 0.2333 s' }),
  K(0x19, 'air flash step (replaces 0x11)', 'M_1364', 'air', 'approach', 'FLASH_STEP (108)', 0x442A0000, { air_mode: true, keep_face: true }),
  K(0x1A, 'air radial cut (replaces 0x12)', 'M_1365', 'air', 'crowd', 'RADIAL_CUT (109)', 0x43820000, { air_mode: true, keep_face: true }),
  K(0x1B, 'air finisher: final heaven', 'M_1366', 'air', 'finisher', 'FINAL_HEAVEN (110)', 0x43E10000, { air_mode: true, keep_face: true }),
  K(0x1C, 'launch: slash upper', '1319', 'ground', 'launch', 'SLASH_UPPER (10)', null, { air_mode: true, keep_face: true, note: 'requested with the jump button during a combo (TryLaunch 0x14143EAE0)' }),
  K(0x1D, 'launch: air roll beat', '1320', 'air', 'launch', 'AIR_ROLL_BEAT (11)', null, { air_mode: true, keep_face: true }),
  K(0x1F, 'aerial sweep (replaces 8)', '1309', 'either', 'rise', 'AERIAL_SWEEP (111)', null, { air_mode: true }),
  K(0x20, 'aerial dive (replaces 0x0F)', 'M_1367', 'air', 'combo', 'AERIAL_DIVE (112)', 0x440C0000, { air_mode: true, keep_face: true }),
  K(0x21, 'guard counter: revenge slash', 'RevengeSlash', 'either', 'counter', '-', 0x42CC0000, { note: 'forced kind from Guard (TryAttack 0x141437330) while the counter window controller+0x8A0 > 0 (1.0 s after a successful guard)' }),
  K(0x22, 'chase attack (revenge ex)', 'ChaseAttack', 'either', 'counter', 'REVENGE_EX (7)', 0x43AF0000, { air_mode: true, note: 'Begin 0x141333210 forces it when state+0x234 bit 19 (m_pRevengePawn in a reaction state)' }),
  K(0x23, 'guard counter: revenge impact', 'RevengeImpact', 'either', 'counter', 'REVENGEIMPACT (5)', null),
  K(0x24, 'guard counter: revenge dive', 'RevengeDive', 'either', 'counter', 'REVENGEDIVE (6)', 0x44160000, { air_mode: true, stop_dist: 0x43340000 }),
  K(0x27, 'blow counter', 'BlowCounter', 'either', 'counter', 'BLOW_COUNTER (2)', 0x447A0000, { air_mode: true, note: 'forced kind out of AIR_SLIDE after an Aerial Recovery (AirSlide+0xB0 = 0x27; reaction command BLOWCOUNTER offered for 2.0 s): spec 9.5' }),
  K(0x28, 'risk dodge counter: hook upper', 'HookUpper', 'either', 'counter', 'RD_COUNTER (9)', 0x44160000, { air_mode: true }),
  K(0x29, 'risk dodge counter: back raid', 'BackRaid', 'either', 'counter', 'RD_COUNTER (9)', null, { air_mode: true }),
  K(0x2A, 'risk dodge counter: back rush', 'BackRush', 'either', 'counter', 'RD_COUNTER (9)', 0x43A50000, { air_mode: true }),
  K(0x2B, 'risk dodge counter: warp slash', 'WarpSlash', 'either', 'counter', 'RD_COUNTER (9)', null, { air_mode: true }),
  K(0x35, 'dive-fall attack', 'DiveFallAttack', 'air', 'special', '-', null, { air_mode: true, note: 'forced from Jump when its flag bit 14 is set (dive fall); DirZ -8000' }),
  // (combat implementer, 2026-10-12) the wall-run attacks: Decide rule 4 (0x14134ACA0: state+0x1EC bit 3, set by TryAttack out of WALLRUN)
  // picks 0x31..0x34 by the target's side (state+0x1C8, TargetMetrics 0x141330D30); PlayAttack cases 0x31..0x34 (decompiled C)
  K(0x31, 'wall-run attack: off the wall, target ahead / above', 'M_1144', 'air', 'special', '-', null, { note: 'Decide: side 0 (or target more than 400 above within 400): DirZ fixed 1000' }),
  K(0x32, 'wall-run attack: target on side 1', 'M_1147', 'air', 'special', '-', 0x4414C000, { note: 'PlayAttack case 0x32: root-scale channel on, root length 595' }),
  K(0x33, 'wall-run attack: target on side 2', 'M_1146', 'air', 'special', '-', 0x4414C000, { note: 'PlayAttack case 0x33: root-scale channel on, root length 595' }),
  K(0x34, 'wall-run attack: target below', 'M_1145', 'air', 'special', '-', null, { z_gain: 0x40900000, stop_dist: 0x42700000, note: 'Decide: target more than 250 below within 400 (or side 3: DirZ fixed -1000); PlayAttack case 0x34: m_bFaceAt off, z gain 4.5, stop distance 60' }),
  // (combat implementer, fourth pass 2026-10-12) the attack out of a slope slide: TryAttack case 10 sets state+0x1EC bit 5; the head of
  // PlayAttack turns it into kind 0x2F (or 0x10 when the target is more than 30 cm above); out of HOPPING (bit 4) it is always 0x10
  K(0x2F, 'slope-slide attack', 'M_1143', 'air', 'special', '-', null, { air_mode: true, note: 'PlayAttack case 0x2F (0x14133A160): root scale XY = clamp(dist / 815, 0.1, 2.5), state+0x18C = 0.3 (then 0.6 by the air-mode tail), air mode; row RUN_SLOPE' }),
  K(0x39, 'flow attack: somersault (out of a bar swing)', '1184', 'air', 'special', 'SUMMERSALT (94)', null, { air_mode: true, note: 'forced by TryAttack out of POLE_TURN with PoleTurn+0xB0 bit 0; PlayAttack case 0x39: no target, no facing, all traction channels off, state+0x234 bit 8, +0x21C = 3' }),
  // (magic implementer) the airstep's follow-up: forced by TresPlayerState_AthleticFlow's enemy stop (0x141346600 sub 7: a+0x1E8 = 0x36)
  // the flow kinds are refined by Attack slot 124 0x1413325F0 at the head of PlayAttack (decompiled C): from 0x36 / 0x37: 2+ enemies and
  // TURN_CUTTER -> 0x38; else dist > 800 and SONIC_SLASH -> 0x36; else dist < |dz| and SONIC_DOWN -> 0x37; a flow kind whose ability
  // (0x36 SONIC_SLASH 91, 0x37 SONIC_DOWN 92, 0x38 TURN_CUTTER 93, 0x39 SUMMERSALT 94) is missing becomes an ordinary air attack by reach
  K(0x36, 'flow attack: sonic slash (after an airstep arrival at an enemy)', '1181', 'air', 'special', 'SONIC_SLASH (91)', null, { z_gain: 0x40900000, note: 'forced kind from ATHLETICFLOW (magic spec H 4.7); PlayAttack case 0x36: root scale XY = clamp(dist / 850, 0.1, 0.7058824), state+0x18C = 0.3, state+0x190 bit 0; no air mode (state+0x1DF stays 0)' }),
  K(0x37, 'flow attack: sonic cleave (target more below than away)', '1182', 'air', 'special', 'SONIC_DOWN (92)', null, { air_mode: true, stop_dist: 0x437A0000, note: 'PlayAttack case 0x37: dive (state+0x1DE), DirZ state+0x1D4 = -1000, state+0x148 = 5.0, state+0x23C bit 4, state+0x234 bit 3, state+0x190 bit 0 (the last four: not read further)' }),
  K(0x38, 'flow attack: buzz saw (two or more enemies)', '1183', 'air', 'special', 'TURN_CUTTER (93)', null, { air_mode: true, note: 'PlayAttack case 0x38: root scale XY = clamp(dist / 1000, 0.1, 2.5), state+0x190 bit 0; attack camera mode 5' }),
  // Second Form (TresPlayerState_ComboLimit::Decide 0x141362AB0, spec 10.5): finisher kinds by finish index; all keep facing the target
  K(0x7A, 'Second Form ground finisher 1: Explosion (finish index 0 or >= 3)', 'M_1412', 'ground', 'finisher', 'form 0x94..0x98', null, { keep_face: true, note: 'rows LM_EXPLOSION / LM_EXPLOSION2' }),
  K(0x7B, 'Second Form ground finisher 2: Stride Break (finish index 1)', 'M_1411', 'ground', 'finisher', 'form 0x94..0x98', null, { keep_face: true, note: 'rows LM_STRIDEBREAK / LM_STRIDEBREAK2' }),
  K(0x7C, 'Second Form ground finisher 3: Ripple Drive (finish index 2)', 'M_1413', 'ground', 'finisher', 'form 0x94..0x98', null, { keep_face: true, note: 'row LM_RIPPLEDRIVE' }),
  K(0x7D, 'Second Form air finisher 1: Hurricane Period (finish index 0 or >= 3)', 'M_1414', 'air', 'finisher', 'form 0x94..0x98', null, { air_mode: true, keep_face: true, note: 'rows LM_HURRICANEPERIOD / LM_HURRICANEPERIOD2' }),
  K(0x7E, 'Second Form air finisher 2: Aerial Finish (finish index 1)', 'M_1415', 'air', 'finisher', 'form 0x94..0x98', null, { air_mode: true, keep_face: true, note: 'rows LM_AERIALFINISH x3 / LM_AERIALFINISH2' }),
  K(0x7F, 'Second Form air finisher 3: Magnet Splash (finish index 2)', 'M_1416', 'air', 'finisher', 'form 0x94..0x98', null, { air_mode: true, keep_face: true, note: 'rows LM_MAGNESPLASH / LM_MAGNESPLASH2' }),
  // (XSI, 2026-10-16) THE XS ABILITIES' ATTACKS (build/sora_tmp/party/XS_SORA_ABILITIES.md; OUR DESIGN on KH3 clips and rows):
  // kinds 0xE0.. are the port's own (KH3's attack kinds end at 0xD0); crates/kh-core/src/abilities/xs/ forces them
  K(0xE0, '(XSI) Sonic Dash: one dash (Second Form Sonic Blade M_1425)', 'M_1425', 'air', 'special', 'SONIC_DASH (ours 257)', null, { note: 'XS 2.2: forced by the air dodge with a lock 4 - 15 m away; root length scaled to sonic.dash_length, gravity off for sonic.dash_time' }),
  K(0xE1, '(XSI) Sonic Dash: the launch after the 2nd dash (M_1426)', 'M_1426', 'air', 'special', 'SONIC_DASH (ours 257)', null, { keep_face: true, note: 'XS 2.2: an attack press within sonic.follow_window of the 2nd dash' }),
  K(0xE2, '(XSI) Ancient Light (Second Form LM_UNIONDISCODE M_1435)', 'M_1435', 'either', 'special', 'ANCIENT_LIGHT (ours 267)', null, { air_mode: true, keep_face: true, note: 'XS 2.13: the situation command LM_UNIONDISCODE (174) outside the form; invincible throughout; 25 % cap per use' }),
  // (XSI) the clips of other anim sets (tools/xsi_pack.js: build/sora_tmp/xsi/pack, the overlay of the pack's manifest)
  K(0xE3, '(XSI) Spiral Rush (Light Form M416)', 'M_M416', 'ground', 'finisher', 'SPIRAL_RUSH (ours 258)', null, { keep_face: true, note: 'XS 2.3: the ground finisher with the target 5 - 12 m away' }),
  K(0xE4, '(XSI) Zantetsuken (Light Form M411)', 'M_M411', 'ground', 'finisher', 'ZANTETSUKEN (ours 259)', null, { keep_face: true, note: 'XS 2.4: the ground finisher with Attack held; the draw stance held at 0.30 s' }),
  K(0xE5, '(XSI) Event Horizon (Light Form M413)', 'M_M413', 'ground', 'finisher', 'EVENT_HORIZON (ours 260)', null, { keep_face: true, note: 'XS 2.5: takes the ground Magic Flash (kind 7)' }),
  K(0xE6, '(XSI) Giant Stamp (Hyper Hammer 3416)', 'M_3416', 'ground', 'finisher', 'GIANT_STAMP (ours 261)', null, { keep_face: true, note: 'XS 2.6: the ground finisher against a Heavy / Giant body' }),
  K(0xE7, '(XSI) Chandelier (Blizzard Blades 7419)', 'M_7419', 'either', 'special', 'CHANDELIER (ours 262)', null, { air_mode: true, keep_face: true, note: 'XS 2.7: a Blizzard-family Finish End' }),
  K(0xE8, '(XSI) Myriad: the Ultimate Form summon (StyleChange9 9401), then 9419', 'StyleChange9', 'either', 'special', 'MYRIAD (ours 268)', null, { air_mode: true, keep_face: true, note: 'XS 2.14: the finish command of Second Form; M_9419 follows the summon; the form ends with it' }),
  K(0xE9, '(XSI) Light Wave, tap (Re Mind SoKcShoot 1813)', 'SoKcShoot', 'either', 'special', 'LIGHT_WAVE (ours 264)', null, { air_mode: true, keep_face: true, note: 'XS 2.9: Y during MP Charge' }),
  K(0xEA, '(XSI) Light Wave, hold (Re Mind SoKcIrradiate 1814)', 'SoKcIrradiate', 'either', 'special', 'LIGHT_WAVE (ours 264)', null, { air_mode: true, keep_face: true, note: 'XS 2.9: Y held during MP Charge: six waves' }),
  K(0xEB, '(XSI) Rage Raid (Rage Form D420)', 'M_D420', 'either', 'counter', 'RAGE_RAID (ours 265)', null, { keep_face: true, note: 'XS 2.11: the reaction command Raid after a block / dodge against a revenge' }),
  K(0xEC, '(XSI) Riskcharge (Rage Form D419)', 'M_D419', 'either', 'special', 'RISKCHARGE (ours 266)', null, { note: 'XS 2.12: the situation command Riskcharge at 25 % HP' }),
  K(0xED, '(XSI) Crimson Blast (Rage Form D417)', 'M_D417', 'ground', 'finisher', 'RISKCHARGE (ours 266)', null, { keep_face: true, note: 'XS 2.12: the ground finisher for riskcharge.duration after a Riskcharge' }),
];

// ---------------------------------------------------------------------------------------------------- decision rules
// order, phase, condition, result, kind ('-' = several), evidence
const rules = [
  ['pre', 'every Begin (PreDecide 0x14134C420)', 'bAir = true; if !(pawn+0xD80 bit 0): ground probe range 800: hit and dist <= 30 -> bAir = false, then a floor slope test (FUN_14063C870, pitch < -35 -> bAir = true); numEnemies = controller slot 726; lastKind = pawn+0xE10; TargetMetrics: dist, dz, targetOnGround, targetHigh (dz > 160)', '-', '0x14134C420', C],
  ['pre', 'combo == 0, or lastKind is 0x1C / 0x1D', 'reach = 1 if dist > 400, else 2 if numEnemies > 1 (else unchanged: 0 on a fresh state, copied from the previous state in a chain); first = true', '-', D, C],
  ['pre', 'combo == 0', 'lastKind = 0; pawn+0xE50 = AbilityCount(SLASH_UPPER 10); pawn+0xE54 = AbilityCount(AIR_ROLL_BEAT 11)', '-', D, C],
  ['special', 'state+0x1EC bit 3 (wall-run attack request) and the equipped weapon type is 1, 0x16 or 0x17', 'kind 0x31..0x34 by dz / dist / side (wall-run attacks M_1144..M_1147); out of scope', '-', D, C],
  ['special', 'state+0x1EC bit 2 (launch request)', 'kind = 0x1C if pawn+0xE50 == 1 and not (pawn+0xE54 == 1 and bAir and dz < 100), else 0x1D; the used stock is set to 2; combo = 0; pawn+0xE18 = 0', '-', D, C],
  ['special', 'controller+0x920 != 0', 'kind 0x1E (sneeze); finish flags set', '0x1E', D, C],
  ['special', 'state+0x1EC bit 7 (friend unison request)', 'kind 0x3F..0x43 from pawn+0x1464 (0x16..0x1A)', '-', D, C],
  ['next', 'combo < max - finishers, or state+0x234 bit 21: targetHigh and lastKind not in {8, 0x17, 0x1F} and dist <= 400', 'kind 8; DirZ = 700; fixed DirZ', '8', D, C],
  ['next', '... else: bAir and dz < -100', 'kind 0x0D if AIR_DOWN (12) and dz <= -250 and targetOnGround and lastKind not in {0x0D, 0x0E} and dist <= 400, else 0x0F', '-', D, C],
  ['next', '... else by lastKind: 1, 9, 0x18', 'base = 3 if reach == 1 else 2', '-', D, C],
  ['next', '... lastKind 2, 0x0A, 0x11, 0x19', 'base = 1 if reach == 1 else 3', '-', D, C],
  ['next', '... lastKind 3, 0x0B, 0x12, 0x1A', 'base = 1 if reach == 1 else 2', '-', D, C],
  ['next', '... lastKind 8, 0x1F', 'base = 1', '1', D, C],
  ['next', '... lastKind 0x0F, 0x10, 0x20', 'base = 8', '8', D, C],
  ['next', '... lastKind 0x17', 'base = 3', '3', D, C],
  ['next', '... any other lastKind, first == true', 'base = 2 if reach == 1; 3 if reach == 2; else 1', '-', D, C],
  ['next', '... any other lastKind, first == false, by combo % 3', '0: base = reach + 1; 1: base = 2 if reach == 0 else 1; 2: base = 2 if reach == 2 else 3', '-', D, C],
  ['next', 'after the base choice', 'if bAir or (combo > 0 and targetHigh): kind = base + 0x0F; pawn finish count (pawn+0xE14) = 0', '-', D, C],
  ['finisher', 'combo >= max - finishers and not state+0x234 bit 21', 'n = pawn+0xE14; bFinish = true; if n >= finishers - 1 or combo >= max - 1: "finish end" (state+0x141, pawn+0xE48 bit 5: no further attack request is accepted)', '-', D, C],
  ['finisher', '... bAir and dz < -100 and AIR_DOWN (12) and dz <= -250 and targetOnGround and lastKind not in {0x0D, 0x0E} and dist <= 400', 'kind 0x0E; pawn+0xE14 = n + 1', '0x0E', D, C],
  ['finisher', '... n == 0', 'no TRIPPLE_SLASH / CHARGE_THRUST: 4. Else 5; with CHARGE_THRUST: 6 unless TRIPPLE_SLASH is also equipped and dist <= 400 and numEnemies != 1', '-', D, C],
  ['finisher', '... n == 1', 'slot B: 4 if TRIPPLE_SLASH or CHARGE_THRUST is equipped; else 7 with MAGICFLUSH (0x0C instead when FINAL_HEAVEN is also equipped and numEnemies < 2); else 0x0C with FINAL_HEAVEN; else 0', '-', D, C],
  ['finisher', '... n == 2', 'slot C: the MAGICFLUSH / FINAL_HEAVEN choice of slot B when slot B was taken by 4; else 0', '-', D, C],
  ['finisher', 'after the finisher choice', 'if bAir or targetHigh: kind += 0x0F; pawn+0xE14 = n + 1', '-', D, C],
  ['post', 'kind 1 / 2 / 3 with QUICK_SHAFT (107) / FLASH_STEP (108) / RADIAL_CUT (109)', 'kind 9 / 0x0A / 0x0B', '-', D, C],
  ['post', 'kind 0x10 / 0x11 / 0x12 with the same abilities', 'kind 0x18 / 0x19 / 0x1A', '-', D, C],
  ['post', 'kind 8 with AERIAL_SWEEP (111)', 'kind 0x1F', '0x1F', D, C],
  ['post', 'kind 0x0F with AERIAL_DIVE (112)', 'kind 0x20', '0x20', D, C],
  ['forced', 'state+0x1E8 != 0 (set by the request function: guard counter 0x21 / 0x23 / 0x24, barrier cracker 0x25, risk-dodge counters, dive-fall 0x35, slide attacks 0x36..0x39)', 'PlayAttack 0x1413378B0 uses it instead of the decided kind (0x0F / 0x10 / 0x11 / 0x12 are upgraded by the abilities above)', '-', PLAY, I],
];

// ---------------------------------------------------------------------------------------------------- reactions
const reactions = [
  // id, damage_kind, name, situation, anim, handler, evidence, note
  ['gate', '-', '-', 'any', '-', '0x141439A50', C, 'interface slot 88: refuses when slot 87 says no, the controller timer +0x8F4 > 0, pawn+0xE60 bit 17, or (light kinds only) super armor / attack kinds 0x36..0x39; else clears the queue, pushes DAMAGE (19) and calls its setup slot 114'],
  ['setup', '-', '-', 'any', '-', '0x141365930', I, 'copies TresDamageInfo to state+0x98; state+0x88 = damage kind (the sub-step starts at 0; the handler is a delegate chosen by a jump table: sheet sora-combat-damage-handlers); HP < 1 turns SMALL into BLOW (scale 1, brake 0); hit direction, variant = hit counter & 1 (+0x140); +0x138 = 0.2 s = delay before Aerial Recovery (not an invincibility)'],
  ['small.ground.front', 1, 'SMALL', 'ground, direction 0', '0201 / 0205 (variant)', '0x141435200', I, 'direction index 0..3 and variant chosen by the Damage state; table in the pawn'],
  ['small.ground.back', 1, 'SMALL', 'ground, direction 1', '0202 / 0206 (variant)', '0x141435200', I, '-'],
  ['small.ground.dir2', 1, 'SMALL', 'ground, direction 2', '0203', '0x141435200', I, '-'],
  ['small.ground.dir3', 1, 'SMALL', 'ground, direction 3', '0204', '0x141435200', I, '-'],
  ['small.air', 1, 'SMALL', 'air, direction 0..3', '0207 / 0208 / 0209 / 0210', '0x141435200', I, '-'],
  ['blow', 2, 'BLOW', 'direction 0..3, phase 0..2', '0211..0213 / 0214..0216 / 0217..0219 / 0220..0222', '0x1414333A0', I, 'three phases per direction'],
  ['down', '-', '-', 'index 0..3', 'M_0223 / M_0224 / M_0225 / M_0226', '0x141433F10', I, 'WALL-HIT clips of Blow / Parabola (event interface slot 9 0x141356930: speed >= 50, angle within 30 deg of the wall normal); the id is historical'],
  ['heavy', '-', '-', 'handlers', 'M_0227 / M_0228 / M_0230 / M_0237 / 0213 / Land / JumpNormalDownLoop / CatchEnd', '0x14135D740', I, 'Damage state slots 116..124 (0x14135D740, 0x14135B300, 0x14135EEA0, 0x14135C410, 0x14135CAB0, 0x14135A870, 0x14135B150, 0x14135E260, 0x14135BDA0): one per reaction family; read in pass 2: spec 9.4, sheets sora-combat-damage-handlers / -damage-clips / -damage-params'],
  ['freeze', '-', 'bad status', 'pawn slot 1048 true', '-', '0x141439A50', I, 'state BADSTAT_FREEZE (123), time from TresDamageInfo+0x38'],
  ['burn', '-', 'bad status', 'pawn slot 1049 true and damage kind NONE', '-', '0x141439A50', I, 'state BADSTAT_BURN (124)'],
  ['recover', 13, 'RECOVER', 'any', '-', '0x1405BB850', I, 'heals: HP += CeilToInt(amount); no reaction'],
  ['without', 12, 'WITHOUT', 'any', '-', '0x1405BB850', I, 'damage without reaction (FUN_14064E7C0)'],
  ['kill', 14, 'KILL', 'any', '-', '0x1405BB850', I, 'amount = current HP'],
  ['die', '-', '-', 'HP < 1 after the hit', '-', '0x14143A190', I, 'unless pawn slot 575 0x14142F640 allows survival (item count check 0xA01); interface slot 129 pushes DIE (59). Second Chance / Withstand Combo act before, in ApplyHP 0x1405A2A00 (sheet sora-combat-hp-rules)'],
];

// ---------------------------------------------------------------------------------------------------- notify classes
const notifyInfo = {
  TresAnimNotifyState_Attack: ['+0x200 begin / +0x208 end', '0x140561F40', 'begin: pawn enables the collision group with the attack-data row; the Attack state handler 0x1413459E0 sets state+0x164 (first hit window opened: a cast may be requested from now on; an attack also needs TresEnableInput and a landed hit of this chain), clears state+0x1F4 bit 4 and state+0x234 bit 3, counts the hit window (slot 115: state+0x12C / +0x138). end 0x140564B10: group off (the hit list stays until the next activation)'],
  TresAnimNotifyState_Traction: ['+0x258 begin / +0x260 end', '0x140564520', 'state+0x84 bit 2 (m_bEnableTraction) on / off: the homing window'],
  TresAnimNotify_EnableInput: ['+0x240', '0x14055F9A0', 'state+0x84 bit 5: the next attack / launch request is accepted from here (if the first hit window has opened and a hit of this chain has landed, or with Combo Master); Tick also sets it at TresEnableChange'],
  TresAnimNotify_EnableChange: ['+0x238', '0x14055F920', 'state+0x84 bit 0 (m_bEnableChange): a queued next attack starts now; Attack state handler 0x141342DF0 also calls the controller (FUN_1414142D0(controller, 0): controller+0x878 bit 9 off); slot 128 ends the hover of an air-mode attack and Tick raises the cancel bit'],
  TresAnimNotify_FaceAt: ['+0x298', '0x14055FED0', 'state+0x84 bit 4: the facing code snaps to the target on its next tick and re-reads the target metrics'],
  TresAnimNotifyState_AttackEnableMapHitReflect: ['-', '0x1405620F0', 'window in which a weapon hit on world geometry recoils the attack (state REFLECT): sets primitive+0x987 bit 1 (0x140625C00 / 0x1406250E0); spec 8.2.5 step 9 and 8.5.1'],
  TresAnimNotify_LandAnimPose: ['-', '0x140560320', 'stores the landing pose used by the Jump / Idle states (docs/sora-animation-spec.md)'],
  TresAnimNotify_LastAnimPose: ['-', '-', 'stores the last pose kind (battle stance)'],
  TresAnimNotify_Shot: ['+0x228', '0x140561B70', 'Attack state handler 0x141344580: spawns the attack\'s projectile / shockwave (air down: DOWNIMPACT)'],
  TresAnimNotify_Timing: ['-', '0x140561CC0', 'generic timing event with m_Param (flash step, back raid)'],
  TresAnimNotify_ThrowWeapon: ['-', '-', 'back raid: throws the keyblade (TresWeaponMovementComponent homing)'],
  TresAnimNotify_PlayWeaponSwingSE: ['-', '0x1405615F0', 'swing sound: m_SoundAliasID SWING1..6 of the weapon sound alias set (enumset_swing_w_so010)'],
  TresAnimNotify_PlayVoice: ['-', '-', 'random voice from m_RandomVoiceAssets (weights)'],
  TresAnimNotifyState_AttachEffect: ['-', '-', 'trail / swing effect attached to the weapon or owner for the window'],
  TresAnimNotify_AttachEffect: ['-', '-', 'one-shot effect'],
  TresAnimNotifyState_BpEvent: ['-', '-', 'blueprint event window (camera / state event component)'],
};

// ---------------------------------------------------------------------------------------------------- data readers
function loadPkgFlat(rel) {
  const d = JSON.parse(fs.readFileSync(P(R, PKG, rel + '.uasset.json'), 'utf8'));
  const imp = (i) => { let im = d.imports[-i - 1], s = im.objectName; while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; s = im.objectName + '.' + s; } return s; };
  const ref = (v) => (v === 0 ? null : v < 0 ? imp(v) : 'export:' + d.exports[v - 1].objectName);
  const val = (p) => { if (p.type === 'ObjectProperty') return ref(p.value); if (p.type === 'StructProperty') return Array.isArray(p.value) ? flat(p.value) : p.value;
    if (p.type === 'ArrayProperty') { const v = p.value; if (v && v.items) { if (p.innerType === 'StructProperty') return v.items.map((it) => (Array.isArray(it) ? flat(it) : it)); if (p.innerType === 'ObjectProperty') return v.items.map(ref); return v.items; } return v; } return strip(p.value); };
  const flat = (props) => { const o = {}; for (const p of props || []) o[p.name + (p.arrayIndex ? '[' + p.arrayIndex + ']' : '')] = val(p); return o; };
  const props = (e) => flat((e.data && e.data.props) || e.props);
  const imports = d.imports.filter((im) => im.outerIndex < 0 && d.imports[-im.outerIndex - 1].className === 'Package').map((im) => ({ cls: im.className, path: d.imports[-im.outerIndex - 1].objectName + '.' + im.objectName }));
  return { d, props, imports };
}
function attackData() {
  const j = JSON.parse(fs.readFileSync(P(R, 'sheets/entities/DataTable/Player/Attack/p_ex001_AttackData.json'), 'utf8'));
  const keep = /^(default|kb_|revenge_|rdc_|blow_counter|atk_barriercracker|divefall_attack|lm_|ff_sonic_|run_slope|run_wall_|unison_|fusion_|fr_|sclf_|schm_giantstamp|scsk_chandelier|scsw_myriad|an_counter|an_riskcharge|an_crimsonblast|sf_reflectivelaser|sf_lfshoot)/; // (XSI) sclf_ .. sf_lfshoot: the XS abilities' KH3 rows // unison_ / fusion_ / fr_: party stream D (the team attacks' shots) // run_*: fourth combat pass (the wall-run and slope-slide attacks' rows)
  const yn = (b) => (b ? 'yes' : 'no');
  // (XSI) the XS rows go after the others: the indices of the rows before them stay what they were
  const xsi = /^(sclf_|schm_giantstamp|scsk_chandelier|scsw_myriad|an_counter|an_riskcharge|an_crimsonblast|sf_reflectivelaser|sf_lfshoot)/;
  return [...j.rows.filter((r) => keep.test(r.id) && !xsi.test(r.id)), ...j.rows.filter((r) => xsi.test(r.id))].map((r) => [r._row_name, r.m_Power, strip(r.m_DamageKind).replace('TRES_DMG_KIND_', ''), strip(r.m_DamageAttribute).replace('TRES_DMG_ATTR_', ''), r.m_DamagePowerScale, r.m_DamageParabolaAngle, r.m_DamageMoveLength, r.m_DamageBrakeParam,
    strip(r.m_KnockbackType).replace('TRES_AHKBT_', ''), r.m_DamageExecRate, r.m_DamageEffectTime, strip(r.m_BadStatusKind).replace('TRES_BADSTAT_', ''), r.m_BadStatusGenRate, r.m_BadStatusEffectTime, r.m_BadStatusEffectParam, strip(r.m_TeamCheckType),
    r.m_ReactionPowerValue, yn(r.m_bIsScrumAttack), r.m_ArmorAttackPower, r.m_FormPoint, yn(r.m_bIsMagicAttack), yn(r.m_bIsIvalidGuard), yn(r.m_bIsRapidFire), yn(r.m_bIsKillerAttack), yn(r.m_bIsAttractionDamage), yn(r.m_bIsCounterAttack), yn(r.m_bIsEnableWeakGuardAttack),
    r.m_ReflectLevel, strip(r.m_ReflectReaction).replace(/TRES_ATK_R[RS]_/, ''), r.m_RevengePoint, strip(r.m_PhysForceLv).replace('TPDF_', ''), 'sheets/entities/DataTable/Player/Attack/p_ex001_AttackData.json row ' + r.id]);
}
function collisions() {
  const out = [];
  for (const [owner, rel] of [['w_so_KeybladeBase', 'Blueprints/Player/p_ex001/w_so_KeybladeBase'], ['p_ex001_Pawn', 'Blueprints/Player/p_ex001/p_ex001_Pawn']]) {
    const pk = loadPkgFlat(rel); const seen = {};
    for (const e of pk.d.exports) { if (e.class !== 'TresAtkCollComponent') continue;
      for (const s of pk.props(e).CollisionShapesSrc || []) { const n = (seen[s.m_GrpName] = (seen[s.m_GrpName] || 0) + 1); const v3 = (o) => (o ? [o.x, o.y, o.z] : null);
        out.push([owner + '/' + s.m_GrpName + (n > 1 ? '#' + n : ''), owner, s.m_GrpName, s.m_ShapeType, s.m_Size.x, s.m_Size.y, s.m_Size.z, s.m_SocketName1, s.m_SocketName2, v3(s.m_RelativeLocation1), v3(s.m_RelativeLocation2),
          s.m_RelativeRocation ? [s.m_RelativeRocation.pitch, s.m_RelativeRocation.yaw, s.m_RelativeRocation.roll] : null, s.m_AttachType1, s.m_AttachType2, s.m_MapHitType, s.m_bDisableSweep ? 'no' : 'yes', (s.m_CollisionProfileName || {}).Name || '-', s.m_DefaultAttackDataIDName || '-', s.m_bIsPhysAttackCollision ? 'yes' : 'no',
          PKG + rel + '.uasset.json export ' + e.objectName + ' CollisionShapesSrc']); } } }
  return out;
}
let _bones = null, _set = null;
function clip(name) {
  if (!_set) { _set = new Map(sp.loadAnimSet(ANIMSET).rows.map((r) => [r.name, r.asset])); _bones = sp.buildSkeleton(sp.readGlb(P(R, 'assets_converted', 'meshes', CONTENT, sp.gamePathToContent(MESH) + '.glb')).json); }
  // (XSI) the XS abilities' clips of other anim sets (tools/xsi_pack.js CLIPS)
  const XSI = { M_M416: 'M416', M_M411: 'M411', M_M413: 'M413', M_3416: '3416', M_7419: '7419', M_9419: '9419', M_D417: 'D417', M_D419: 'D419', M_D420: 'D420', SoKcShoot: '1813', SoKcIrradiate: '1814' };
  const asset = _set.get(name) || (XSI[name] ? '/Game/Character/pc/p_ex001/anm/rt/body/' + XSI[name] + '.' + XSI[name] : null); if (!asset) return null;
  const content = sp.gamePathToContent(asset);
  if (!fs.existsSync(P(R, 'assets_converted', 'animations', CONTENT, content + '.anim.json'))) return { asset, content, sequence: false };
  const a = sp.loadAnimation(asset, _bones);
  const tr = a.chans.find((c) => c.bone === 0 && c.path === 'translation');
  let fwd = 0, up = 0, upMin = 0, upMax = 0;
  if (tr) { const cy = Math.cos(MESH_YAW * Math.PI / 180), sy = Math.sin(MESH_YAW * Math.PI / 180); let t0;
    for (let i = 0; i < a.numFrames; i++) { const t = a.numFrames > 1 ? i * a.length / (a.numFrames - 1) : 0; const p = sp.sampleChan(tr, t)(tr.ue_values); if (i === 0) t0 = p; const d = [p[0] - t0[0], p[1] - t0[1], p[2] - t0[2]]; fwd = d[0] * cy - d[1] * sy; up = d[2]; upMin = Math.min(upMin, up); upMax = Math.max(upMax, up); } }
  return { asset, content, sequence: true, length: a.length, frames: a.numFrames, rootMotion: !!(a.props && a.props.bEnableRootMotion === true), fwd: r4(fwd), up: r4(up), upMin: r4(upMin), upMax: r4(upMax), notifies: a.notifies };
}

// ---------------------------------------------------------------------------------------------------- build
function build() {
  const codeIds = new Set(); for (const l of fs.readFileSync(P(R, 'sheets/native/code.json'), 'utf8').split('\n')) { const m = l.match(/^\["(0x[0-9A-Fa-f]+)"/); if (m) codeIds.add(m[1].toUpperCase().replace('0X', '0x')); }
  const chk = (a, where) => { if (a !== '-' && !codeIds.has(a)) throw new Error(`${where}: ${a} is not a function start in sheets/native/code`); return a; };
  const S = {};
  // pass-2 parameter rows: appended; an id that exists keeps its value (the texts, address and confidence of pass 2 replace the old ones)
  const allParams = params.map((r) => r.slice()), byId = new Map(allParams.map((r) => [r[0], r]));
  for (const { strand, row } of PASS2.params) { if (row.length !== 8) throw new Error('pass 2 row ' + row[0] + ': 8 cells expected');
    const old = byId.get(row[0]);
    if (old) { if (JSON.stringify(old[1]) !== JSON.stringify(row[1])) throw new Error(`pass 2 (strand ${strand}) would change the value of ${row[0]}: ${JSON.stringify(old[1])} -> ${JSON.stringify(row[1])}; change the first-pass row on purpose`); for (let i = 2; i < 8; i++) old[i] = row[i]; }
    else { const r = row.slice(); allParams.push(r); byId.set(r[0], r); } }
  S['sora-combat-params'] = { cols: ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'], rows: allParams.map((r) => { chk(r[5], r[0]); return r; }) };
  const atkRows = [], hitRows = [], usedAnims = new Set(), notifyCount = {};
  const hitsOf = (anim, c) => c.notifies.filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n, i) => {
    const p = n.properties; const extra = Object.keys(p).filter((k) => !/^(CollisionGrpName|m_AttackDataIDName|m_WeaponHitSEAliasID|AttackInterval)$/.test(k)).map((k) => k + '=' + JSON.stringify(p[k])).join('; ');
    return [anim + '#' + (i + 1), anim, path.basename(c.content), r4(n.trigger_time), r4(n.end_trigger_time), Math.round(n.trigger_time * 6000) / 100, Math.round(n.end_trigger_time * 6000) / 100, p.CollisionGrpName || '-', p.m_AttackDataIDName || '-', p.m_WeaponHitSEAliasID ? strip(p.m_WeaponHitSEAliasID) : '-', p.AttackInterval !== undefined ? r4(p.AttackInterval) : '-', extra || '-', 'notify ' + n.name + ' of ' + c.asset];
  });
  for (const a of attacks) {
    const c = clip(a.anim); if (!c || !c.sequence) throw new Error('clip of ' + a.anim + ' missing');
    usedAnims.add(a.anim);
    const N = (cls) => c.notifies.filter((n) => n.class === cls);
    const first = (cls) => { const n = N(cls)[0]; return n ? r4(n.trigger_time) : '-'; };
    const hits = hitsOf(a.anim, c);
    atkRows.push(['kind_' + a.kind.toString(16).padStart(2, '0'), a.kind, a.label, a.anim, path.basename(c.content), a.phase, a.role, a.requires, a.rootHex === null ? '-' : f32(a.rootHex), a.stop_dist ? f32(a.stop_dist) : 160, a.z_gain ? f32(a.z_gain) : 3,
      a.air_mode ? 'yes' : 'no', a.keep_face ? 'yes' : 'no', r4(c.length), c.frames, c.rootMotion ? 'yes' : 'no', c.fwd, c.up, first('TresAnimNotify_EnableInput'), first('TresAnimNotify_EnableChange'),
      N('TresAnimNotifyState_Traction').map((n) => r4(n.trigger_time) + '-' + r4(n.end_trigger_time)), N('TresAnimNotify_FaceAt').map((n) => String(r4(n.trigger_time))), [...new Set(hits.map((h) => h[8]))], chk(PLAY, 'attacks'), C, a.note || '-']);
  }
  // hit windows: the attack clips plus the other melee clips that carry attack notifies
  // the first ten keep the row order of revision 1; the rest are the finish-command clips of pass 2 (spec 10.6)
  const extraClips = ['M_1411', 'M_1412', 'M_1413', 'M_1414', 'M_1415', 'M_1416', 'M_1421', 'M_1422', 'M_1425', 'StyleChangeEnd',
    // (StyleChangeEasy is a second AnimSet name of clip 1403 = StyleChangeEnd: not listed, its window would be a duplicate)
    'M_1423', 'M_1424', 'M_1426', 'M_1427', 'M_1428', 'M_1429', 'M_1430', 'M_1431', 'M_1432', 'M_1433', 'M_1434', 'M_1435'];
  const seenClip = new Set();
  for (const name of [...attacks.filter((a) => a.kind < 0x7A).map((a) => a.anim), ...extraClips]) { if (seenClip.has(name)) continue; seenClip.add(name); const c = clip(name); if (!c || !c.sequence) continue; usedAnims.add(name); hitRows.push(...hitsOf(name, c)); for (const n of c.notifies) notifyCount[n.class] = (notifyCount[n.class] || 0) + 1; }
  S['sora-combat-attacks'] = { cols: ['id', 'kind', 'label', 'anim', 'clip', 'phase', 'role', 'requires', 'root_len', 'stop_dist', 'z_gain', 'air_mode', 'keep_face', 'length', 'frames', 'root_motion', 'root_fwd', 'root_up', 'enable_input', 'enable_change', 'traction', 'face_at', 'attack_rows', 'ref_addr', 'ref_conf', 'note'], rows: atkRows };
  S['sora-combat-hits'] = { cols: ['id', 'anim', 'clip', 'start', 'end', 'start_f60', 'end_f60', 'group', 'attack_row', 'hit_se', 'interval', 'extra', 'source'], rows: hitRows };
  S['sora-combat-collisions'] = { cols: ['id', 'owner', 'group', 'shape', 'size_x', 'size_y', 'size_z', 'socket1', 'socket2', 'loc1', 'loc2', 'rot', 'attach1', 'attach2', 'map_hit', 'sweep', 'profile', 'default_row', 'phys', 'source'], rows: collisions() };
  S['sora-combat-attack-data'] = { cols: ['id', 'power', 'damage_kind', 'attribute', 'power_scale', 'parabola_angle', 'move_length', 'brake', 'knockback', 'exec_rate', 'effect_time', 'bad_status', 'bad_gen_rate', 'bad_time', 'bad_param', 'team_check', 'reaction_power', 'scrum', 'armor_attack', 'form_point', 'magic', 'invalid_guard', 'rapid_fire', 'killer', 'attraction', 'counter', 'weak_guard', 'reflect_level', 'reflect_reaction', 'revenge_point', 'phys_force', 'source'], rows: attackData() };
  S['sora-combat-rules'] = { cols: ['id', 'order', 'phase', 'condition', 'result', 'kind', 'ref_addr', 'ref_conf'], rows: rules.map((r, i) => ['rule_' + String(i + 1).padStart(2, '0'), i + 1, r[0], r[1], r[2], r[3], chk(r[4], 'rules'), r[5]]) };
  S['sora-combat-reactions'] = { cols: ['id', 'damage_kind', 'name', 'situation', 'anim', 'ref_addr', 'ref_conf', 'note'], rows: reactions.map((r) => { chk(r[5], 'reactions ' + r[0]); return r; }) };
  S['sora-combat-notifies'] = { cols: ['id', 'instances', 'iface_slot', 'ref_addr', 'effect'], rows: Object.keys(notifyCount).sort().map((cls) => { const i = notifyInfo[cls] || ['-', '-', '-']; return [cls, notifyCount[cls], i[0], chk(i[1], 'notifies ' + cls), i[2]]; }) };
  // pass-2 tables: one sheet each, rows as authored (address columns are checked by preflight through the sheet definitions)
  for (const [name, t] of Object.entries(PASS2.tables)) { if (S[name]) throw new Error('sheet name taken: ' + name);
    for (const r of t.rows) if (r.length !== t.cols.length) throw new Error(name + ' ' + r[0] + ': cell count');
    S[name] = { cols: t.cols, rows: t.rows }; }
  return { S, usedAnims };
}
function sheetText(name, s) { return `{"sheet":"${name}","group":"port","columns":${JSON.stringify(s.cols)},"rows":[\n` + s.rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n'; }

function assetRequests(usedAnims) {
  // everything the combat clips and the weapon reference: read from the import tables of the packages
  const more = ['Guard', 'GuardHit', 'AirGuard', 'AirGuardHit', 'AttackReflect', 'AttackAirReflect', 'Reflec', 'AirReflec', 'AirRecovery', 'IdleB', 'IdleLockon', 'BattleStart', 'BattleEnd', 'RunBattleStart', 'RunBattleEnd', 'StyleChange', 'StyleChangeLoop', 'StyleChangeEnd',
    'BadStatSneeze', 'BadStatAirSneeze', 'BadStatStunStart', 'BadStatStunLoop', 'BadStatStunEnd', 'BadStatBurnStart', 'BadStatBurnLoop', 'BadStatBurnEnd', 'CatchStart', 'CatchLoop', 'CatchEnd',
    ...Array.from({ length: 22 }, (_, i) => '02' + String(i + 1).padStart(2, '0')), 'M_0223', 'M_0224', 'M_0225', 'M_0226', 'M_0227', 'M_0228', 'M_0230', 'M_0237',
    'M_1411', 'M_1412', 'M_1413', 'M_1414', 'M_1415', 'M_1416', 'M_1421', 'M_1422', 'M_1423', 'M_1424', 'M_1425'];
  const names = [...new Set([...usedAnims, ...more])];
  const anims = [], notifyClasses = {}, effects = {}, sounds = {}, other = {};
  const add = (map, key, by) => { (map[key] = map[key] || new Set()).add(by); };
  const classify = (im, by) => { const p = im.path;
    if (/^\/Script\//.test(p)) { if (/AnimNotify/.test(p) && !/Default__/.test(p)) add(notifyClasses, p.split('.').pop(), by); return; }
    if (/^\/Game\/Effects\//.test(p)) add(effects, p, by); else if (/^\/Game\/Sound\//.test(p) || /Voice/.test(p)) add(sounds, p, by); else if (!/\/anm\/rt\/body\/|_Skeleton/.test(p)) add(other, p, by); };
  for (const n of names) { const c = clip(n); if (!c) { anims.push({ anim_name: n, clip: null, source: null, note: 'not in p_ex001_AnimSet' }); continue; }
    anims.push({ anim_name: n, clip: path.basename(c.content), source: c.asset, class: c.sequence ? 'AnimSequence' : 'BlendSpace', length_s: c.sequence ? r4(c.length) : null });
    if (!c.sequence) continue;
    for (const nn of c.notifies) if (nn.class) add(notifyClasses, nn.class, n);
    try { for (const im of loadPkgFlat(c.content).imports) classify(im, n); } catch (e) { /* package json missing */ } }
  for (const rel of ['Blueprints/Player/p_ex001/w_so010_Wep', 'Blueprints/Player/p_ex001/w_so_KeybladeBase']) for (const im of loadPkgFlat(rel).imports) classify(im, path.basename(rel));
  const pawn = loadPkgFlat('Blueprints/Player/p_ex001/p_ex001_PawnBase'); const cdo = pawn.props(pawn.d.exports.find((e) => /^Default__/.test(e.objectName)));
  for (const k of ['m_pGuardReflectEffect', 'm_pJustGuardEffect', 'm_pCriticalCounterHitEffect', 'm_CmnAuraEffect', 'm_CmnBadStatesEffect', 'm_pAthleticHitEffect']) if (cdo[k]) add(effects, cdo[k], 'p_ex001_PawnBase.' + k);
  const list = (m) => Object.keys(m).sort().map((k) => ({ path: k, referenced_by: [...m[k]].sort() }));
  const out = { generator: 'tools/sora_combat_sheet.js ' + VERSION, note: 'Assets referenced by Sora\'s melee combat with the Kingdom Key (base form and Second Form finishers, hit reactions, guard, bad status). Sound entries are asset paths from the import tables (weapon alias sets, voice assets, SE assets named by notifies); the alias ids used by notifies are in swing_alias_ids / hit_alias_ids.',
    animations: anims, notify_classes: Object.keys(notifyClasses).sort().map((k) => ({ class: k, used_by: notifyClasses[k].size })), effects: list(effects), sounds: list(sounds), other: list(other), swing_alias_ids: [], hit_alias_ids: [] };
  const sw = new Set(), hi = new Set();
  for (const n of names) { const c = clip(n); if (!c || !c.sequence) continue; for (const nn of c.notifies) { const p = nn.properties || {}; if (p.m_SoundAliasID) sw.add(strip(p.m_SoundAliasID)); if (p.m_WeaponHitSEAliasID) hi.add(strip(p.m_WeaponHitSEAliasID)); } }
  out.swing_alias_ids = [...sw].sort(); out.hit_alias_ids = [...hi].sort();
  return out;
}

function main() {
  const check = process.argv.includes('--check');
  const { S, usedAnims } = build(); let diff = 0;
  for (const [name, s] of Object.entries(S)) { const file = P(R, 'sheets/port', name + '.json'), text = sheetText(name, s);
    if (check) { if (!fs.existsSync(file) || fs.readFileSync(file, 'utf8') !== text) { console.log('differs: ' + name); diff++; } }
    else { fs.writeFileSync(file, text); console.log(`sora_combat_sheet: sheets/port/${name}.json  ${s.rows.length} rows`); } }
  if (process.argv.includes('--assets')) { const o = assetRequests(usedAnims), f = P(R, 'build/sora_tmp/combat/asset_requests.json'); fs.mkdirSync(path.dirname(f), { recursive: true }); fs.writeFileSync(f, JSON.stringify(o, null, 1));
    console.log(`sora_combat_sheet: ${path.relative(R, f)}  animations ${o.animations.length}, notify classes ${o.notify_classes.length}, effects ${o.effects.length}, sounds ${o.sounds.length}, other ${o.other.length}`); }
  if (check && diff) process.exit(1);
}
if (require.main === module) main(); else module.exports = { build, VERSION };

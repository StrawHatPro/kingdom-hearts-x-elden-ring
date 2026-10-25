// (XSI, 2026-10-16) The 16 abilities of build/sora_tmp/party/XS_SORA_ABILITIES.md (approved by the user with the 9
// recommended answers; "don't start with critical counter": Critical Mode's start set does not give it, so Critical
// Counter is a level C 18 / Godskin Noble ability like the others). ATTRACTION FLOW stays excluded (dec029).
//
// Writes three AUTHORED sheets (our design where marked "ours"; KH3 data where a game source is named):
//   sheets/port/sora-enum-ext.json        the port's own ETresAbilityKind values (256..): tools/gen_port.js adds them to
//                                         the generated enum; tools/sora_stats_sheet.js gives each its sora-abilities row
//   sheets/port/sora-xs-abilities.json    every number of the 16 abilities (one row = one constant; gen_port ->
//                                         build/gen/rust/sora_xs_abilities.rs, XsAbilityParams / XS_ABILITY_PARAMS)
//   sheets/port/sora-xs-battle-levels.json  Hidden Potential's "battle level" of an Elden Ring enemy by region depth
//                                         (OUR DESIGN: the small-boss region bands of sora-er-boss-rewards small.tier_cap)
// and exports ABILITIES (the catalogue rows) and LEVEL_SLOTS for tools/sora_stats_sheet.js.
//
//   node tools/sora_xs_sheet.js           write the sheets
//   node tools/sora_xs_sheet.js --check   exit 1 if a sheet on disk differs
'use strict';
const fs = require('fs'), path = require('path');
const R = path.resolve(__dirname, '..');
const XS = 'build/sora_tmp/party/XS_SORA_ABILITIES.md';
const OURS = 'OUR DESIGN (XS proposal, approved by the user 2026-10-16; not data of KH3)';

// ------------------------------------------------------------------ the abilities
// [game name, value, English name, AP, category, system, XS section, effect, help text, sources]
// Values from 256 up: TRES_ABI_KIND_MAX (244) and ETresAbilityKind_MAX (245) stay the game's sentinels; the game's
// ability word array has 0x200 entries (status/ability_set.rs ABILITY_SLOTS).
const ABILITIES = [
  ['DIVING_STRIKE', 256, 'Diving Strike', 2, 'ACTION', 'movement', '2.1', 'While falling with 6 m or more below him and an enemy in KH3\'s dive cone (MyDiveFallLockonRangeXY 1000 / LimitZ 3500), A starts KH3\'s DiveFallAttack (1050): the dive pose is held while he homes on the target at 2500 cm/s, then the BEAT hit, the shock ring and the 5.0 VERTICALROLL END hit (+10 % per full 5 m fallen beyond 6 m, at most +50 %). Invincible from the press to the END hit. After 1 s of free fall with 10 m below he tips head-first (DiveFallStart 0048 -> DiveFallLoop 0049).', 'Press the Attack button while falling from a height to dive onto an enemy below.'],
  ['SONIC_DASH', 257, 'Sonic Dash', 3, 'ACTION', 'air', '2.2', 'In the air, locked on a target 4 to 15 m away: X with the stick toward it flashes through it (Second Form\'s Sonic Blade dash M_1425, LM_SONICRAVE 1.2 SMALL, 1000 cm in 0.30 s, gravity off, invincible 0 - 0.30 s). After the 2nd dash, A within 0.4 s plays M_1426 (LM_SONICRAVE_END 1.5 TOSS). Uses the Air Slide charges.', 'Dash through a locked-on target in midair with the Dodge button.'],
  ['SPIRAL_RUSH', 258, 'Spiral Rush', 3, 'ACTION', 'finisher', '2.3', 'Ground finisher when the target is 5 to 12 m away: Light Form\'s drill charge M416 (900 cm forward; SCLF_SPIRALRUSH_START 0.3, SCLF_SPIRALRUSH 0.6, SCLF_SPIRALRUSH_END 1.0 VERTICALROLL).', 'Charge at a distant target with a drilling finisher that launches it.'],
  ['ZANTETSUKEN', 259, 'Zantetsuken', 4, 'ACTION', 'finisher', '2.4', 'Hold Attack on a ground finisher: Light Form\'s iaido M411 holds its draw stance at 0.30 s while Attack stays down (at most 1.5 s); release to cut (SCLF_ZANTETSUKEN 1.5 SMALL; held 0.8 s or more: power x1.6 and reaction power 2).', 'Hold the Attack button on a finisher, then release it to cut through the enemy.'],
  ['EVENT_HORIZON', 260, 'Event Horizon', 4, 'ACTION', 'finisher', '2.5', 'Takes Magic Flash\'s place in the ground finisher choice: Light Form\'s light ring M413 (SCLF_EVENTHORIZON 0.5 BEAT magic, SCLF_EVENTHORIZON_END 1.8 SIDESPIN magic; ring radius 450 cm).', 'Finish a combo with a ring of light that scales with Magic.'],
  ['GIANT_STAMP', 261, 'Giant Stamp', 5, 'ACTION', 'finisher', '2.6', 'Ground finisher against a Heavy or Giant body: the Keyblade becomes the Hyper Hammer for one slam (3416, SCHM_GIANTSTAMP 2.0 TOSS armour 10 rp 5; shockwave 400 cm; invincible 1.20 - 1.53 s).', 'Slam a large foe with a giant hammer.'],
  ['CHANDELIER', 262, 'Chandelier', 3, 'ACTION', 'magic', '2.7', 'A Blizzard-family Finish End becomes Blizzard Blades\' falling ice chandelier (7419, SCSK_CHANDELIER 0.5 + SCSK_CHANDELIER_END 5.0 BLOW, blizzard, FREEZE; power x0.6 / x0.8 / x1.0 by tier).', 'Your Blizzard finish brings down a chandelier of ice.'],
  ['SHOTLOCK_SPECTRAL_RAYS', 263, 'Spectral Rays', 3, 'ACTION', 'shotlock', '2.8', 'A second shotlock, chosen on the Customize screen: Ever After\'s refracting lasers (TresShootFlowData row 11 sf_laser_0, M_1483, up to 28 locks, focus cost 20; SF_REFLECTIVELASER 0.8 SMALL per lock, each beam refracts once to the nearest other lock at x0.5). The shotlock nerf of the Elden Ring host applies unchanged.', 'Allows the Spectral Rays shotlock to be chosen in Customize.'],
  ['LIGHT_WAVE', 264, 'Light Wave', 4, 'ACTION', 'magic', '2.9', 'While MP Charge runs the situation slot offers Light Wave once per charge: tap Y = SoKcShoot 1813 and one wave, hold Y = SoKcIrradiate 1814 and 6 waves 0.30 s apart aimed with the stick (SoKc_LightWave_Projectile 7000 cm/s, piercing, 35 m; row SF_LFSHOOT 0.8 TOSS magic x (1 + 0.25 per -ga spell)). The MP Charge gets 10 s longer.', 'Fire waves of light while your MP recharges.'],
  ['RAGE_RAID', 265, 'Rage Raid', 4, 'ACTION', 'counter', '2.11', 'After a successful Block, Dodge Roll, Risk Dodge or Air Slide against an armoured body inside its revenge armour (4 s), the reaction command Raid (kh3_system.command.antiraid): Rage Form\'s counter D420 (AN_COUNTER 0.2 x n, AN_COUNTER_END 1.0 TOSS armour 10 rp 5; invincible 0 - 1.10 s); the END hit clears the body\'s revenge and its revenge armour.', 'Counter a foe that breaks out of your combo.'],
  ['RISKCHARGE', 266, 'Riskcharge', 5, 'ACTION', 'finisher', '2.12', 'At 25 % HP or less the situation slot offers Riskcharge (kh3_system.situation.riskcharge): Rage Form\'s D419 (invincible), 20 % of the current HP (never lethal), charge level +1 (at most 3); for 10 s the ground finisher is Crimson Blast D417 whose shot uses _IMPACT / _RC1 / _RC2 / _RC3 by level. One use per 15 s.', 'Trade HP for power when in a pinch.'],
  ['ANCIENT_LIGHT', 267, 'Ancient Light', 6, 'ACTION', 'spectacle', '2.13', 'With a full formchange gauge and the lock on a boss or Giant body, the situation slot also offers Ancient Light (LM_UNIONDISCODE 174, kh3_system.situation.lmex_ux): the whole gauge is spent on Second Form\'s M_1435 (LM_UNIONDISCODE x5 0.2 TOSS, 2 0.25 MAGNET, 3 0.1, 4 6.0 BLOW) instead of entering the form. Invincible throughout. At most 25 % of the target\'s maximum HP per use.', 'Spend a full Formchange gauge on a finisher of light against a powerful foe.'],
  ['MYRIAD', 268, 'Myriad', 7, 'ACTION', 'spectacle', '2.14', 'Second Form\'s finish command becomes Ultimate Form\'s finale: the 9401 summon swaps in the Ultima form weapon, then 9419 (SCSW_MYRIAD 0.3 BEAT multi-hit, SCSW_MYRIAD_END 6.0 VERTICALROLL armour 10 rp 5) and the form ends. Invincible throughout. At most 25 % of the target\'s maximum HP per use.', 'Second Form ends with the Ultima Weapon\'s finale.'],
  ['BREAK_BOOST', 269, 'Break Boost', 4, 'SUPPORT', 'passive', '2.16', 'Finishers, reprisals and launchers deal +1 reaction power to STRONG bodies; +25 % Elden Ring poise damage from rows with reaction power 1 or more; the open window after an endurance break lasts 12 s instead of KH3\'s 10 s. The revenge limit and armour, the endurance lockout and the chain cap are untouched.', 'Break the guard of powerful foes faster and keep it open longer.'],
];
// KH3 rows the abilities switch on (no new value): CRITICAL_COUNTER 104 (only in KH3's Critical start set), OVERCOME 179
// (KH3 AP 0, only in Wear Form 05's form-ability list).
const SWITCHED_ON = {
  CRITICAL_COUNTER: { ap: 5, section: '2.10', sources: ['level C:18', 'Elden Ring boss godskin_noble'], note: 'KH3 gives it only in Critical Mode\'s start set; the port: level C 18 and Godskin Noble (XS 2.10). DF removed it from the port\'s Critical start (dec032), the user: "don\'t start with critical counter". ER addition (ours): a just guard against a STRONG body takes reaction power 2 off its endurance.' },
  OVERCOME: { ap: 5, section: '2.15', sources: ['Elden Ring boss mimic_tear'], note: 'KH3 row AP 0 and no player source (Wear Form 05 only); the port: AP 5, Mimic Tear (XS 2.15). Active only against powerful foes (bosses and Giant bodies: Targets::foe_level), with the battle level of sora-xs-battle-levels in Elden Ring.' },
};
// Level slots X1 emptied (XS 4.1): level -> path letter -> ability.
const LEVEL_SLOTS = { 12: { a: 'SPIRAL_RUSH' }, 18: { c: 'CRITICAL_COUNTER' }, 22: { a: 'DIVING_STRIKE', b: 'DIVING_STRIKE', c: 'DIVING_STRIKE' }, 44: { b: 'LIGHT_WAVE' } };
// Boss rows (XS 4.2): boss row -> what the proposal list gains / loses.
const BOSS_ROWS = {
  tree_sentinel: { add: 'SPIRAL_RUSH' }, agheel: { add: 'SONIC_DASH' }, smarag: { add: 'SHOTLOCK_SPECTRAL_RAYS' }, dragonkin_nokstella: { add: 'BREAK_BOOST' },
  mimic_tear: { add: 'OVERCOME' }, astel: { add: 'LIGHT_WAVE' }, elemer: { add: 'GIANT_STAMP' }, godskin_noble: { add: 'CRITICAL_COUNTER' },
  morgott: { add: 'ANCIENT_LIGHT' }, mohg_omen: { add: 'RAGE_RAID' }, mohg_lord: { add: 'RISKCHARGE', remove: 'FOCUS_ASPIR' }, loretta_haligtree: { add: 'CHANDELIER' },
  rellana: { add: 'ZANTETSUKEN' }, metyr: { add: 'EVENT_HORIZON' }, consort_radahn: { add: 'MYRIAD' },
};

// ------------------------------------------------------------------ the numbers (one row per constant)
// [id, value, unit, source, evidence, ref_addr, ref_conf, note]
const K = (id, value, unit, source, evidence, note = '-', ref = '-', conf = 'confirmed') => [id, value, unit, source, evidence, ref, conf, note];
const ours = (id, value, unit, section, evidence, note = '-') => K(id, value, unit, `${OURS}, ${XS} ${section}`, evidence, note);
const PARAMS = [
  // 2.1 Diving Strike
  ours('dive.min_drop', 600, 'cm', '2.1', 'the attack press starts Diving Strike only with at least this much drop below Sora (the ground probe); XS Q8'),
  K('dive.lockon_xy', 1000, 'cm', 'KH3 pawn default MyDiveFallLockonRangeXY (docs/sora-combat-spec.md 7.4)', 'targets farther than this horizontally are not in the dive cone', '-', '-', 'confirmed'),
  K('dive.lockon_limit_z', 3500, 'cm', 'KH3 pawn default MyDiveFallLockonLimitZ (docs/sora-combat-spec.md 7.4)', 'targets more than this below Sora (or above him) are not in the dive cone', '-', '-', 'confirmed'),
  ours('dive.pose_fall_time', 1.0, 's', '2.1', 'seconds of free fall (Jump sub-step 4) before Sora tips head-first by himself'),
  ours('dive.pose_min_height', 1000, 'cm', '2.1', 'drop below him needed for the head-first pose'),
  ours('dive.pose_steer_speed', 300, 'cm/s', '2.1', 'horizontal speed the stick steers the head-first fall at'),
  ours('dive.home_speed', 2500, 'cm/s', '2.1', 'speed he homes on the target at while the dive pose of DiveFallAttack is held'),
  ours('dive.hold_time', 0.9, 's', '2.1', 'clip time of DiveFallAttack (1050) whose pose is held during the homing (before the BEAT window at 0.967 s)'),
  ours('dive.contact', 150, 'cm', '2.1', 'distance to the target centre (or height above the ground) at which the held pose is released'),
  ours('dive.bonus_step', 500, 'cm', '2.1', 'every full step fallen beyond dive.min_drop adds dive.bonus_per_step to the END hit\'s power'),
  ours('dive.bonus_per_step', 0.1, '-', '2.1', 'power bonus per step'),
  ours('dive.bonus_max', 0.5, '-', '2.1', 'largest power bonus'),
  ours('dive.home_timeout', 4.0, 's', '2.1', 'a dive that has met neither its target nor the ground after this long is released (a safety)'),
  // 2.2 Sonic Dash
  ours('sonic.range_min', 400, 'cm', '2.2', 'nearest locked target a dash is offered for'),
  ours('sonic.range_max', 1500, 'cm', '2.2', 'farthest locked target a dash is offered for'),
  ours('sonic.stick_cos', 0.5, 'cos', '2.2', 'the stick must point within acos(this) of the direction to the target'),
  ours('sonic.stick_min', 0.5, '-', '2.2', 'the stick must be tilted at least this far'),
  ours('sonic.dash_length', 1000, 'cm', '2.2', 'length of one dash (M_1425 travels 1099 cm of root motion: scaled to this)'),
  ours('sonic.dash_time', 0.3, 's', '2.2', 'duration of the dash part of M_1425 (its attack and traction windows: 0 - 0.30 s); gravity off and invincible meanwhile'),
  ours('sonic.follow_window', 0.4, 's', '2.2', 'after the 2nd dash, an attack press within this time plays the M_1426 launch'),
  // 2.3 Spiral Rush
  ours('spiral.range_min', 500, 'cm', '2.3', 'nearest target for which the ground finisher is Spiral Rush'),
  ours('spiral.range_max', 1200, 'cm', '2.3', 'farthest target'),
  // 2.4 Zantetsuken
  ours('zantetsuken.hold_min', 0.1, 's', '2.4', 'the attack button held this long when the finisher begins makes it Zantetsuken (a tap is the normal finisher). 0.1 s, not 0.2: the queued finisher begins about 0.17 s after a press made late in the input window (test-level trace 2026-10-16), so 0.2 s needed a press before the window opened'),
  ours('zantetsuken.stance_time', 0.3, 's', '2.4', 'clip time of M411 whose pose is held while the button stays down'),
  ours('zantetsuken.hold_max', 1.5, 's', '2.4', 'longest hold of the stance'),
  ours('zantetsuken.full_charge', 0.8, 's', '2.4', 'hold time of a full charge'),
  ours('zantetsuken.full_power', 1.6, '-', '2.4', 'power factor of a full charge (1.5 x 1.6 = 2.4, Magic Flash\'s power in one hit)'),
  ours('zantetsuken.full_rp', 2, 'rp', '2.4', 'reaction power of a full charge (the row has 1)'),
  // 2.5 Event Horizon
  ours('event_horizon.radius', 450, 'cm', '2.5', 'radius of the light ring around Sora (its two attack windows)'),
  // 2.6 Giant Stamp
  ours('giant_stamp.radius', 400, 'cm', '2.6', 'shockwave radius around the impact (the group StampL, which the Kingdom Key does not have)'),
  ours('giant_stamp.ahead', 150, 'cm', '2.6', 'the impact point is this far in front of Sora'),
  ours('giant_stamp.invincible_from', 1.2, 's', '2.6', 'clip time the invincibility starts (top of the jump)'),
  ours('giant_stamp.invincible_to', 1.53, 's', '2.6', 'clip time it ends (the impact)'),
  // 2.7 Chandelier
  ours('chandelier.scale_1', 0.6, '-', '2.7', 'power factor of a Blizzard Finish End'),
  ours('chandelier.scale_2', 0.8, '-', '2.7', 'Blizzara'),
  ours('chandelier.scale_3', 1.0, '-', '2.7', 'Blizzaga'),
  ours('chandelier.radius', 350, 'cm', '2.7', 'radius of the chandelier\'s hits around the target (the group FinishSK)'),
  ours('chandelier.ahead', 400, 'cm', '2.7', 'without a target the chandelier falls this far in front of Sora'),
  ours('chandelier.end_time', 1.6, 's', '2.7', 'clip time of 7419 the chandelier crashes (the end of its SCSK_CHANDELIER window): the SCSK_CHANDELIER_END hit (no notify of the clip names that row)'),
  // 2.8 Spectral Rays
  K('spectral.kind', 11, 'ETresShootFlowKind', 'KH3 TresShootFlowData row 11 "ラプ反射" = kh3_system.command.sf_laser_0 "Spectral Rays"', 'the shotlock kind of Spectral Rays', '-', '-', 'confirmed'),
  ours('spectral.refract_scale', 0.5, '-', '2.8', 'damage factor of a beam\'s refraction to the nearest other lock'),
  ours('spectral.beam_from', 1.6, 's', '2.8', 'clip time of M_1483 the beams start at (its SF_REFLECTIVELASER_RANGE window: 1.60 - 3.60 s)'),
  ours('spectral.beam_to', 3.6, 's', '2.8', 'clip time the last beam leaves'),
  ours('spectral.hit_radius_share', 0.5, '-', '2.8', 'a beam hit sphere at the target centre has this share of the target radius'),
  ours('spectral.hit_radius_min', 30, 'cm', '2.8', 'and at least this radius'),
  ours('spectral.hit_life', 0.0333, 's', '2.8', 'a beam hit sphere lives this long (two 60 Hz steps)'),
  // 2.9 Light Wave
  ours('light_wave.tap_max', 0.25, 's', '2.9', 'a press of Y released within this is a tap (one wave); held longer = the volley'),
  ours('light_wave.volley', 6, 'waves', '2.9', 'waves of the held version'),
  ours('light_wave.interval', 0.3, 's', '2.9', 'seconds between waves of the volley'),
  K('light_wave.speed', 7000, 'cm/s', 'KH3 SoKc_LightWave_Projectile InitialSpeed', 'wave speed', '-', '-', 'confirmed'),
  ours('light_wave.range', 3500, 'cm', '2.9', 'a wave lives 0.5 s (35 m at 7000 cm/s)'),
  ours('light_wave.radius', 120, 'cm', '2.9', 'collision radius of a wave'),
  ours('light_wave.ga_bonus', 0.25, '-', '2.9', 'power factor added per -ga spell learned'),
  ours('light_wave.charge_extra', 10, 's', '2.9', 'the MP Charge gets this much longer (the MP Haste family still shortens it)'),
  K('light_wave.shot_time', 0.34, 's', 'KH3 clip SoKcShoot 1813: its TresEnableInput', 'the tap\'s wave leaves at this clip time', '-', '-', 'confirmed'),
  ours('light_wave.command', 248, 'ETresCommandKind', '2.9', 'the situation slot entry of Light Wave: KH3\'s unused Reserve248 (the HUD names it Light Wave)'),
  ours('light_wave.height', 120, 'cm', '2.9', 'a wave leaves this high above the feet (the Keyblade at chest height)'),
  ours('light_wave.offset', 80, 'cm', '2.9', 'a wave leaves this far in front of Sora'),
  ours('light_wave.hit_radius_min', 30, 'cm', '2.9', 'smallest sphere of the one-step hit a wave puts on a target it passes'),
  ours('light_wave.hit_life', 0.0333, 's', '2.9', 'life of that hit (one 30 fps frame)'),
  ours('light_wave.stick_min', 0.5, '-', '2.9', 'the stick pushed this far aims the waves (else the lock-on target, else the facing)'),
  ours('chandelier.end_life', 0.1, 's', '2.7', 'life of the crash hit SCSK_CHANDELIER_END'),
  // (XW-b) the chandelier itself: KH3's StylePawn w_so088 (native ATresStylePawnSKChandelier, accompany pawn of
  // TRES_CMD_KIND_FINISH_SK in w_so080_Equip form 07), which plays its own M_7423 (Character/wep/w_so088/anm/rt/7423)
  K('chandelier.pawn_attack_start', 5.0, 's', 'KINGDOM HEARTS III/Content/Character/wep/w_so088/anm/rt/7423.uasset (TresAnimNotifyState_Attack SCSK_CHANDELIER_END 5.0 .. 5.6 s)', 'asset: the chandelier\'s clip time its crash hit begins; the pawn is shown so that this falls on chandelier.end_time of 7419 (its clip time = 7419\'s + pawn_attack_start - end_time)'),
  K('chandelier.pawn_length', 5.6333, 's', 'KINGDOM HEARTS III/Content/Character/wep/w_so088/anm/rt/7423.uasset (SequenceLength 5.6333, 170 frames)', 'asset: the chandelier\'s clip: it is not drawn past its end (its anim BP has no other row)'),
  K('chandelier.pawn_max_time', 10, 's', 'KINGDOM HEARTS III/Content/Blueprints/StylePawn/w_so088.uasset (Default__w_so088_C m_Param_MaxTime; class default 30, ATresAccompanyPawnBase ctor 0x1413E6850)', 'asset: the pawn\'s longest life'),
  // 2.10 Critical Counter
  ours('critical_counter.strong_rp', 2, 'rp', '2.10', 'a just guard against a STRONG body takes this off its endurance (no damage)'),
  // 2.11 Rage Raid
  ours('rage_raid.window', 4, 's', '2.11', 'Raid is offered after a successful block / dodge against a body inside this many seconds of its revenge (react.revenge.armour of sora-er-host)'),
  ours('rage_raid.offer_time', 2, 's', '2.11', 'how long the Raid reaction command stays'),
  ours('rage_raid.invincible_to', 1.1, 's', '2.11', 'Rage Raid is invincible from 0 to this clip time'),
  ours('rage_raid.near', 600, 'cm', '2.11', 'a Dodge Roll, Risk Dodge or Air Slide counts against a body in its revenge this near'),
  K('rage_raid.command', 13, 'ex kind', 'KH3 ETresCommandKind ANTIRAID 13 (kh3_system.command.antiraid "Raid")', 'the reaction command Raid', '-', '-', 'confirmed'),
  // 2.12 Riskcharge
  ours('riskcharge.hp_share', 0.25, '-', '2.12', 'Riskcharge is offered at this share of max HP or less'),
  ours('riskcharge.cost', 0.2, '-', '2.12', 'share of the current HP it costs (never lethal)'),
  ours('riskcharge.max_level', 3, 'levels', '2.12', 'highest charge level'),
  ours('riskcharge.duration', 10, 's', '2.12', 'seconds the finisher is Crimson Blast'),
  ours('riskcharge.cooldown', 15, 's', '2.12', 'seconds between two uses'),
  K('riskcharge.effect_group_base', 9, 'GroupID', 'KH3 clip D419: its AttachEffect rows carry GroupID 10 / 11 / 12 (vp_ant_rsk_crg_00 / _01, vp_ant_rsk_rsn00, vp_ant_in_sho01)', 'the play of M_D419 takes effect group base + the charge level (1 .. 3)', '-', '-', 'inferred'),
  K('light_form.effect_group', 2100, 'GroupID', 'KH3 clips M416 / M411 / M413: the Oathkeeper (w_so200) rows carry GroupID 2100, the Oblivion (w_so210) rows 2200', 'the plays of Spiral Rush, Zantetsuken and Event Horizon take this effect group (the Oathkeeper-coloured set, XS 2.3)', '-', '-', 'confirmed'),
  K('riskcharge.blast_time', 1.62, 's', 'KH3 clip D417: its TresAnimNotify_Shot', 'Crimson Blast\'s shot leaves at this clip time', '-', '-', 'confirmed'),
  ours('riskcharge.blast_radius', 300, 'cm', '2.12', 'radius of the shot\'s blast'),
  ours('riskcharge.blast_ahead', 400, 'cm', '2.12', 'the blast is this far in front of Sora (or on the target when nearer)'),
  ours('riskcharge.blast_life', 0.3, 's', '2.12', 'the blast hits for this long'),
  // 2.13 / 2.14 the spectacle caps (Q7)
  ours('spectacle.cap_share', 0.25, '-', '1 and 2.13 / 2.14', 'one use of Ancient Light or Myriad deals at most this share of the target\'s maximum HP (XS Q7)'),
  ours('myriad.change_radius', 550, 'cm', '2.14', 'the Ultimate Form summon\'s blast around Sora (group ChangeSW of 9401)'),
  ours('myriad.radius', 500, 'cm', '2.14', 'the multi-hit of SCSW_MYRIAD around Sora (group SW_Myriad)'),
  ours('myriad.end_radius', 700, 'cm', '2.14', 'the 6.0 VERTICALROLL of SCSW_MYRIAD_END around Sora (group SW_Myriad2)'),
  // 2.13 Ancient Light's shapes of the groups the Kingdom Key lacks (L2_Pillar1..5, L2_Dark: another Keyblade's)
  ours('ancient.pillar_ring', 220, 'cm', '2.13', 'the five pillars stand on a ring of this radius around the target'),
  ours('ancient.pillar_radius', 180, 'cm', '2.13', 'radius of one pillar'),
  ours('ancient.pillar_height', 800, 'cm', '2.13', 'height of one pillar'),
  ours('ancient.no_target_ahead', 400, 'cm', '2.13', 'without a target the ring stands this far in front of Sora'),
  ours('ancient.dark_radius', 600, 'cm', '2.13', 'the sphere of L2_Dark around Sora'),
  // 2.16 Break Boost
  ours('break_boost.rp', 1, 'rp', '2.16', 'reaction power added to finishers, reprisals and launchers against STRONG bodies'),
  ours('break_boost.poise', 0.25, '-', '2.16', 'Elden Ring poise damage factor added for rows with reaction power 1 or more'),
  ours('break_boost.open_time', 12, 's', '2.16', 'seconds a broken endurance stays open (KH3: 10)'),
];

// ------------------------------------------------------------------ Hidden Potential's battle levels (OUR DESIGN)
// Region depth = the band of sora-er-boss-rewards small.tier_cap / kh-er lootmap depth_of_block: 0 Limgrave and Weeping
// Peninsula, 1 Liurnia / Caelid, 2 Altus / Mt. Gelmir / Leyndell, 3 Mountaintops / Farum Azula / Haligtree / the
// underground, 4 Shadow of the Erdtree. KH3's floor: EnemyBaseStatus[bl].m_BaseDefensePower + ceil(0.15 bl + 1.5)
// (0x140580E70): 15 -> 19, 30 -> 28, 45 -> 38, 60 -> 48, 75 -> 58 (Sora's level-99 Strength is 62 before his weapon).
const LEVELS = [[0, 15, 'Limgrave, Weeping Peninsula'], [1, 30, 'Liurnia, Caelid'], [2, 45, 'Altus Plateau, Mt. Gelmir, Leyndell'], [3, 60, 'Mountaintops, Farum Azula, Haligtree, the underground'], [4, 75, 'Shadow of the Erdtree']];
const BATTLE_LEVELS = [
  ...LEVELS.map(([d, bl, names]) => [`depth.${d}`, bl, 'battle level', `${OURS}, ${XS} 2.15 (the open item: the region band of the small-boss tier table)`, `Hidden Potential's battle level of a boss or Giant body in region depth ${d} (${names}); the Strength / Magic floor is EnemyBaseStatus[${bl}].m_BaseDefensePower + ceil(0.15 x ${bl} + 1.5)`, '-', 'confirmed', 'depth bands of sora-er-boss-rewards small.tier_cap']),
  ['default', 1, 'battle level', `${OURS}`, 'a host that does not know the region (the test level): the target\'s own level, else this (1: no floor above Sora\'s own stats)', '-', 'confirmed', '-'],
];

function sheet(name, cols, rows) { return { sheet: name, group: 'port', columns: cols, rows }; }
const PCOLS = ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'];
function build() {
  return {
    'sora-enum-ext': sheet('sora-enum-ext', ['id', 'enum', 'name', 'enum_value', 'source', 'note'], ABILITIES.map((a) => [`ETresAbilityKind/${a[0]}`, 'ETresAbilityKind', `ETresAbilityKind::${a[0]}`, a[1], `${OURS}, ${XS} ${a[6]}`, `${a[2]} (AP ${a[3]})`])),
    'sora-xs-abilities': sheet('sora-xs-abilities', PCOLS, PARAMS),
    'sora-xs-battle-levels': sheet('sora-xs-battle-levels', PCOLS, BATTLE_LEVELS),
  };
}
// One row per line, as the other port sheets.
function text(s) { return `{"sheet":${JSON.stringify(s.sheet)},"group":"port","columns":${JSON.stringify(s.columns)},"rows":[\n${s.rows.map((r) => JSON.stringify(r)).join(',\n')}\n]}\n`; }

function main() {
  const check = process.argv.includes('--check');
  let bad = 0;
  for (const [name, s] of Object.entries(build())) {
    const file = path.join(R, 'sheets', 'port', name + '.json');
    const t = text(s);
    if (check) { if (!fs.existsSync(file) || fs.readFileSync(file, 'utf8') !== t) { console.log(`DIFFERS ${name}`); bad++; } continue; }
    fs.writeFileSync(file, t);
    console.log(`wrote sheets/port/${name}.json (${s.rows.length} rows)`);
  }
  if (bad) process.exit(1);
}
module.exports = { ABILITIES, SWITCHED_ON, LEVEL_SLOTS, BOSS_ROWS, PARAMS, BATTLE_LEVELS };
if (require.main === module) main();

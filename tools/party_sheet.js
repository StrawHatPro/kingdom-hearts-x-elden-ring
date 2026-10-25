// Stream B of the party port (docs/party-port-plan.md, docs/party-contracts.md section B): the
// party simulation's sheets and their Rust.
//
//   node tools/party_sheet.js
//
// Writes (Spreadsheet Method: every row cites its asset / address; a value that was not read is
// null and stops the generator):
//   sheets/port/party-params.json   scalar parameters (base rows, pawn class defaults, action
//                                   definition class defaults over their native constructors,
//                                   KO / revive, friend damage parameters ...)
//   sheets/port/party-levels.json   n_ex001 / n_ex002 LevelUpData (99 rows each)
//   sheets/port/party-attacks.json  n_ex001 / n_ex002 AttackDataTable rows
//   sheets/port/party-states.json   the NPC states the crate runs: class, constructor, functions read
//   build/gen/rust/party_data.rs    the four sheets as Rust, plus the minimal clip table
//                                   (lengths and notifies of every AnimSequence row of the two body
//                                   anim sets) that kh-party uses until stream A's generated anim
//                                   tables exist (contract B.7)
'use strict';
const fs = require('fs');
const path = require('path');
const sp = require('./sora_pack.js');

const ROOT = path.join(__dirname, '..');
const SHEETS = path.join(ROOT, 'sheets', 'port');
const GEN = process.env.KH_GEN_RUST_DIR || path.join(ROOT, 'build', 'gen', 'rust');
const ENT = path.join(ROOT, 'sheets', 'entities');
const PKG = path.join(ROOT, 'assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');

const readJson = (p) => JSON.parse(fs.readFileSync(p, 'utf8'));
const table = (rel) => readJson(path.join(ENT, rel + '.json')).rows;

// ---- class default objects of blueprints (export "Default__<name>_C")
function cdo(contentPath) {
  const d = readJson(path.join(PKG, contentPath + '.uasset.json'));
  const name = 'Default__' + path.basename(contentPath) + '_C';
  const e = d.exports.find((x) => x.objectName === name);
  if (!e) throw new Error('no CDO ' + name);
  const out = {};
  for (const p of (e.data && e.data.props) || []) out[p.name] = p.type === 'StructProperty' && Array.isArray(p.value) ? Object.fromEntries(p.value.map((q) => [q.name, q.value])) : p.value;
  return out;
}
function exportProps(contentPath, objectName) {
  const d = readJson(path.join(PKG, contentPath + '.uasset.json'));
  const e = d.exports.find((x) => x.objectName === objectName);
  if (!e) throw new Error('no export ' + objectName + ' in ' + contentPath);
  return Object.fromEntries(((e.data && e.data.props) || []).map((p) => [p.name, p.value]));
}

// ---- params
const P = [];
const row = (id, value, unit, source, evidence, ref_addr = '-', ref_conf = 'confirmed', note = '-') => P.push([id, value, unit, source, evidence, ref_addr, ref_conf, note]);

for (const [ch, uid] of [['n_ex001', 11], ['n_ex002', 12]]) {
  const b = table(`DataTable/Npc/Base/${ch}_BaseDataTable`)[0];
  const src = `DataTable/Npc/Base/${ch}_BaseDataTable row Default`;
  row(`${ch}.base.max_hp`, b.m_MaxHitPoint, 'HP', src + ' m_MaxHitPoint', 'base max HP before HP_UP victory bonuses');
  row(`${ch}.base.max_mp`, b.m_MaxMagicPoint, 'MP', src + ' m_MaxMagicPoint', 'max MP');
  row(`${ch}.base.damage_min`, b.m_DamageMin, 'HP', src + ' m_DamageMin', 'defender floor in CalcDamage 0x140632240 (stats spec 7)', '0x140632240');
  row(`${ch}.base.damage_max`, b.m_DamageMax, 'HP', src + ' m_DamageMax', 'defender per-hit CAP in CalcDamage 0x140632240: one hit takes at most this much', '0x140632240');
  const pawn = cdo(`Blueprints/Npc/${ch}/${ch}_PawnBase`);
  row(`${ch}.mp_charge_time`, pawn.MyMpChargeTime ?? null, 's', `Blueprints/Npc/${ch}/${ch}_PawnBase CDO MyMpChargeTime`, 'MP charge length after MP runs out (plan 2.8)', '-', pawn.MyMpChargeTime == null ? 'guess' : 'confirmed');
  // (B5) m_DangleOffsetPos (pawn vt+0xE88 0x140541BA0): the hang point from the ledge contact, and the jump's ledge probe height
  const dof = pawn.m_DangleOffsetPos || null;
  row(`${ch}.dangle.offset_x`, dof ? dof.x : null, 'cm', `Blueprints/Npc/${ch}/${ch}_PawnBase CDO m_DangleOffsetPos.X`, 'Dangle: hang point = ledge contact + Rot(yaw) x the offset (FUN_140638900); the space test radius |x| - 10', '0x140541BA0');
  row(`${ch}.dangle.offset_z`, dof ? dof.z : null, 'cm', `Blueprints/Npc/${ch}/${ch}_PawnBase CDO m_DangleOffsetPos.Z`, 'Dangle: hang point below the ledge top; the jump probes ledges at 275 - Z - 150 above the feet (0x140446070 / 0x140433FD0)', '0x140541BA0');
  // The root: ATresPawnBase ctor 0x1405C0550 makes the root a UTresRootComponent (MyRoot, not ACharacter's
  // 34 / 88 capsule: TresPawnBase derives from APawn); its body is built from CollisionShapesSrc[0] by
  // 0x1405DF310 (Sphyl radius = max(Size.X, 1), total half height Size.Z, centre RelativeLocation), as Sora's.
  const root = exportProps(`Blueprints/Npc/${ch}/${ch}_PawnBase`, 'TresRoot').CollisionShapesSrc.items[0];
  const rs = Object.fromEntries(root.map((q) => [q.name, q.value]));
  if (rs.ShapeType !== 'ETresCollision::CAPSULE1') throw new Error(ch + ' root shape ' + rs.ShapeType);
  const rsrc = `Blueprints/Npc/${ch}/${ch}_PawnBase export TresRoot CollisionShapesSrc[0] (CAPSULE1)`;
  row(`${ch}.capsule.radius`, Math.max(rs.Size.x, 1), 'cm', rsrc + ' Size.X', 'root body capsule radius (UTresRootComponent body build 0x1405DF310: max(Size.X, 1))', '0x1405DF310');
  row(`${ch}.capsule.half_height`, rs.Size.z, 'cm', rsrc + ' Size.Z', 'root capsule total half height (0x1405DF310: Sphyl length 2 * (Size.Z - Size.X))', '0x1405DF310');
  row(`${ch}.capsule.center_z`, rs.RelativeLocation.z, 'cm', rsrc + ' RelativeLocation.Z', 'root capsule centre above the actor origin (0x1405DF310)', '0x1405DF310');
  const cap = exportProps(`Blueprints/Npc/${ch}/${ch}_PawnBase`, 'PhysObjHitCollision');
  row(`${ch}.physobj.radius`, cap.CapsuleRadius, 'cm', `${ch}_PawnBase PhysObjHitCollision CapsuleRadius`, 'the physics-object hit capsule (ATresNpcPawnBase ctor 0x141254FF0 "PhysObjHitCollision", attached to the mesh): pushes physics objects, not the body', '0x141254FF0');
  row(`${ch}.physobj.half_height`, cap.CapsuleHalfHeight, 'cm', `${ch}_PawnBase PhysObjHitCollision CapsuleHalfHeight`, 'physics-object hit capsule', '0x141254FF0');
  const mv = exportProps(`Blueprints/Npc/${ch}/${ch}_PawnBase`, 'TresCharMovement0');
  row(`${ch}.move.gravity_scale`, mv.GravityScale, '-', `${ch}_PawnBase TresCharMovement0 GravityScale`, 'movement component gravity scale');
  row(`${ch}.move.max_walk_speed`, mv.MaxWalkSpeed, 'cm/s', `${ch}_PawnBase TresCharMovement0 MaxWalkSpeed`, 'MaxWalkSpeed');
  row(`${ch}.revive_time`, uid === 11 ? 40 : 35, 's', 'UTresNpcState_Die Begin 0x1412413C0: 30 (0x41F00000), unique id 11 -> 40 (0x42200000), 12 -> 35 (0x420C0000)', 'KO timer before the revive; divided by 1 + min(count HYPER_HEALING, 3)', '0x1412413C0', 'inferred');
  const mv1 = cdo(`Blueprints/Npc/${ch}/BT/${ch}_Move1`);
  row(`${ch}.loco.move1.min_speed`, mv1.m_MinSpeed, 'cm/s', `${ch}_Move1 m_MinSpeed`, 'locomotion definition: walk floor');
  row(`${ch}.loco.move1.max_speed`, mv1.m_MaxSpeed, 'cm/s', `${ch}_Move1 m_MaxSpeed`, 'locomotion definition: run');
  row(`${ch}.loco.move1.run_loop_interp`, mv1.m_RunLoopInterpolationTime, 's', `${ch}_Move1 m_RunLoopInterpolationTime`, 'blend into the run loop');
  // (B4) the slope-slide locomotion (pawn m_SlopeSlide = Blueprints/Npc/<ch>/BT/<ch>_SlopeSlide, TresNpcLocomotionDefinitionLand_SlopeSlide)
  const ssl = cdo(`Blueprints/Npc/${ch}/BT/${ch}_SlopeSlide`);
  row(`${ch}.loco.slope_slide.min_speed`, ssl.m_MinSpeed, "cm/s", `${ch}_SlopeSlide m_MinSpeed`, "locomotion definition of the slope slide (pawn m_SlopeSlide +0xB58): walk floor");
  row(`${ch}.loco.slope_slide.max_speed`, ssl.m_MaxSpeed, "cm/s", `${ch}_SlopeSlide m_MaxSpeed`, "locomotion definition of the slope slide: the MaxWalkSpeed the slide runs at (SlopeSlide tick 0x14126C950: MaxAcceleration = MaxWalkSpeed x 60)");
  // (B4) the 2D WalkRun rows' blend parameters (Character/npc/<ch>/anm/rt/body/WalkRunN BlendParameters[0] / [1]):
  // X = the yaw delta move direction - facing, clamped to [Min, Max] (UTresAICharState_Run 0x14045AAF0: anim vt+0x128
  // reads the X range, vt+0x120(X, |Velocity|)), Y = the speed.
  {
    const wr = readJson(path.join(PKG, `Character/npc/${ch}/anm/rt/body/WalkRunN.uasset.json`));
    const bp = wr.exports.map((e) => ((e.data && e.data.props) || []).filter((p) => p.name === 'BlendParameters')).find((l) => l.length);
    const mm = (i, k) => bp[i].value.find((q) => q.name === k).value;
    const src = `Character/npc/${ch}/anm/rt/body/WalkRunN BlendParameters`;
    row(`${ch}.walkrun.x_min`, mm(0, 'Min'), 'deg', src + '[0].Min', 'blend X (yaw delta) lower bound; 0x14045AAF0 clamps X to it', '0x14045AAF0');
    row(`${ch}.walkrun.x_max`, mm(0, 'Max'), 'deg', src + '[0].Max', 'blend X (yaw delta) upper bound', '0x14045AAF0');
    row(`${ch}.walkrun.y_min`, mm(1, 'Min'), 'cm/s', src + '[1].Min', 'blend Y (speed) lower bound', '0x14045AAF0');
    row(`${ch}.walkrun.y_max`, mm(1, 'Max'), 'cm/s', src + '[1].Max', 'blend Y (speed) upper bound', '0x14045AAF0');
  }
}
// turn rates (none is overridden by n_ex00x_PawnBase / the NPC constructors 0x141254FF0 / 0x1411BA820 / 0x1411BAB80)
row('turn.fixed_point_speed', 360, 'deg/s', 'ATresAIPawnBase ctor 0x140529E70 m_FixedPointTurnSpeed (+0xA2C)', 'Turn state without a turn clip: vt+0xDE8 turn at this rate (UTresAICharState_Turn tick 0x140437890 sub-step 3)', '0x140437890');
row('turn.idle_max_rotation_speed', 180, 'deg/s', 'ATresAIPawnBase ctor 0x140529E70 m_IdleMaxRotationSpeed (+0xA34), m_bUseIdleMaxRotationSpeed true', 'Idle: the per-tick rotation is clamped to dt x this (UTresAICharState_Idle tick 0x140445880)', '0x140445880');
row('turn.fixed_turn_max_rotation_speed', 0, 'deg/s', 'ATresAIPawnBase ctor 0x140529E70 m_bUseFixedTurnMaxRotationSpeed false (m_FixedTurnMaxRotationSpeed 180 unused)', 'Turn clip rotation is not clamped (0 = off; 0x140437890 sub-steps 1 / 2)', '0x140437890');
row('turn.attack_rate', 360, 'deg/s', 'pawn vt+0x2408 = 0x140544270 (returns 360.0) for TresNpcPawnBase / n_ex001 / n_ex002', 'TurnToTarget window of AttackBase (tick 0x1404404B0: vt+0xDE0 toward the attack target at this rate)', '0x1404404B0');
row('turn.loco_max_rotation_yaw', 180, 'deg/s', 'UTresNpcLocomotionDefinitionLand(_FollowPlayer) ctor 0x141203230 m_MaxRotationSpeed.Yaw (n_ex00x_Move1 does not override it)', 'AI move: the heading turns at most this fast (velocity steering 0x1404F2590)', '0x1404F2590');
// TresChrInitEquip struct defaults: TCppStructOps<FTresChrInitEquip> vtable 0x146D4DE00, Construct 0x141DA4C50 -> 0x140570AC0
row('reflect.speed', 750, 'cm/s', 'UTresAICharState_Reflect Init 0x14043E620 +0xA0 = 750 (0x443B8000)', 'the recoil: MaxWalkSpeed and the move speed (Tick 0x140441FF0)', '0x140441FF0');
row('reflect.braking', 3072, 'cm/s^2', 'UTresAICharState_Reflect Init 0x14043E620 +0xA4 = 3072 (0x45400000)', 'BrakingDecelerationWalking / Flying during the recoil, ground friction 0', '0x140441FF0');
row('move.jump_up_gravity_scale', 1, '-', 'UTresCharMovementComponent ctor 0x140570EB0 m_JumpUpGravityScale (+0xBE8) = 1.0; not set by UTresAICharMovementComponent ctor 0x140512380 nor n_ex00x_PawnBase TresCharMovement0', 'the NPC rise gravity scale (the jump solver FUN_1405431C0 and PhysFalling)', '0x140570EB0');
row('move.max_jump_duration_time', 0, 's', 'UTresCharMovementComponent ctor 0x140570EB0 m_MaxJumpDurationTime (+0xBEC) = 0.0 (not overridden)', 'no jump-duration gravity phase for the NPCs', '0x140570EB0');
row('move.jump_duration_gravity_scale', 1, '-', 'UTresCharMovementComponent ctor 0x140570EB0 m_JumpDurationGravityScale (+0xBF0) = 1.0 (not overridden)', 'jump-duration gravity scale of the NPCs', '0x140570EB0');
// the Warp state (UTresNpcState_Warp: Init 0x141266C10, Begin 0x14125BAE0, Tick 0x14126DD90, EQS callback 0x141274600)
row('warp.wait', 2, 's', 'ATresNpcPawnBase tick 0x14127B980: the warp request (every +0xC98 = 2.0 s) writes Warp +0x88 = 2.0 (0x40000000)', 'Begin: time limit +0x8C = +0x88; the point found: wait +0x98 = +0x88, limit 2 x +0x88', '0x14127B980');
row('warp.recheck', 0.1, 's', 'UTresNpcState_Warp tick 0x14126DD90: +0x9C reset to 0.1 (0x3DCCCCCD); Init 0x141266C10 starts it at random x 0.1', 'the path-to-the-leader re-check period', '0x14126DD90');
row('warp.threshold', 3000, 'cm', 'UTresNpcState_Warp Begin 0x14125BAE0: +0xA0 = 3000 (0x453B8000) when the friend component +0x314 <= 0', 'a path to the leader shorter than this ends the warp (no move)', '0x14125BAE0');
row('warp.threshold_battle', 1000, 'cm', 'UTresNpcState_Warp Begin 0x14125BAE0: +0xA0 = 1000 (0x447A0000) while the friend component +0x314 > 0 (the battle-start window)', 'the same threshold just after a battle starts', '0x14125BAE0');
row('warp.battle_window', 3, 's', 'UTresFriendComponent +0x312 / +0x314: the 3 s window after the battle starts (stream C2 read)', 'the battle-start window of the warp threshold', '0x14125BAE0', 'inferred');
// (B4) traversal states: UTresNpcState_SlopeSlide / SuperSlide / RailSlide / Hopping (sheet party-states), the probes of the AI Run tick 0x140442430
row("slide.probe_radius", 50, "cm", "FUN_14063C870: sphere 50 (0x42480000) swept from location +50 to -50, channels 0 / 1 / 0x12", "the NPC slope probe (start mode 0 from the Run tick, continue mode 1 from the SlopeSlide tick)", "0x14063C870");
row("slide.start_angle", -25, "deg", "FUN_14063C870 mode 0: fall angle <= -25 (or a SQEX_bEnableForceSlopeSlide floor)", "a flagged floor descending at 25 deg or more starts the slide", "0x14063C870");
row("slide.start_dot", 0.642788, "-", "FUN_14063C870 mode 0: dot(facing, fall line) >= 0.642788 (cos 50)", "facing downhill to start", "0x14063C870");
row("slide.continue_angle", -20, "deg", "FUN_14063C870 mode 1: fall angle <= -20", "the slide continues on a floor descending 20 deg or more", "0x14063C870");
row("slide.continue_dot", 0.173648, "-", "FUN_14063C870 mode 1: dot >= 0.173648 (cos 80)", "facing within 80 deg of the fall line keeps the slide", "0x14063C870");
row("slide.ahead", 50, "cm", "FUN_14063C870: the second sweep 50 cm along (pitch = fall angle, yaw = facing)", "the floor ahead must also be steep enough", "0x14063C870");
row("slide.pitch_add", 25, "deg", "SlopeSlide tick 0x14126C950: target pitch = slope pitch (+0x90) + 25 in sub-step 0", "the body pitch toward the slope (not drawn: the port has yaw only)", "0x14126C950");
row("slide.turn_rate", 6, "1/s", "SlopeSlide tick 0x14126C950: pawn vt+0xDB0 (0x1405DDD90) alpha = dt x 6", "the facing eases toward the AI direction +0x8C", "0x14126C950");
row("slide.accel_factor", 60, "1/s", "SlopeSlide tick 0x14126C950: MaxAcceleration = MaxWalkSpeed (vt+0x1580) x 60 (vt+0x1640)", "the slide reaches its speed within one 60 Hz tick", "0x14126C950");
row("slide.hop_ahead", 120, "cm", "SlopeSlide tick 0x14126C950 / Run tick 0x140442430: hop probe at location + Rot x (120, 0, 0), radius 100", "hop points ahead of a slide / run", "0x14126C950");
row("slide.hop_radius", 100, "cm", "vt+0x2350 radius 100 (0x42C80000) from the SlopeSlide / Run ticks", "hop probe radius", "0x14126C950");
row("slide.rail_range", 50, "cm", "vt+0xEA0 range 50 (0x42480000) from the SlopeSlide / Run ticks", "rail probe range on the ground", "0x14126C950");
row("superslide.speed", 1500, "cm/s", "ATresNpcPawnBase ctor 0x141254FF0: m_SuperSlideSpeed +0xB30 = 1500 (0x44BB8000); n_ex00x_PawnBase do not override it", "SuperSlide tick 0x14126D3B0: MaxWalkSpeed = vt+0x1AB0 (+0xB30) every tick", "0x14126D3B0");
row("superslide.turn_rate", 6, "1/s", "SuperSlide tick 0x14126D3B0: vt+0xDB0 alpha dt x 6 (+0xA8 rate unset)", "the facing eases toward the AI direction", "0x14126D3B0");
row("superslide.stop_angle", 75, "deg", "SuperSlide tick 0x14126D3B0: |wrap(+0x8C - yaw)| >= 75 -> vt+0x16E8 (StopMovement)", "a sharp direction change stops the slide speed", "0x14126D3B0");
row("superslide.air_accel", 3888.0002, "cm/s^2", "SuperSlide tick 0x14126D3B0: in the air Velocity += AirControl x dir x 3888.0002 x dt, clamped to MaxWalkSpeed in XY", "air steering", "0x14126D3B0");
row("superslide.input_hold", 0.2, "s", "SuperSlide tick 0x14126D3B0: +0x94 = 0.2 (0x3E4CCCCD) on an input tick", "input latch time", "0x14126D3B0");
row("rail.warp_delay", 1, "s", "RailSlide EQS callback 0x141250A00: +0x19C = 1.0", "the wait after the point is found before the off-rail warp", "0x141250A00");
row("rail.warp_limit", 2, "s", "RailSlide EQS callback 0x141250A00: +0x198 = 2.0", "the off-rail warp gives up after this", "0x141250A00");
row("rail.warp_probe_range", 150, "cm", "RailSlide tick 0x14124E890: vt+0xEA0 range 150 (0x43160000) at the warp point (segment +30 up)", "a rail at the warp point is taken", "0x14124E890");
row("rail.leave_distance", 500, "cm", "RailSlide slot 145 0x141252210: target = location + forward(XY) x 500, projected by FUN_140542040 (1000)", "the end-of-rail jump target", "0x141252210");
row("rail.leave_height_min", 400, "cm", "RailSlide slot 145 0x141252210: JumpCodeDriven +0x9C = 400 (0x43C80000), +0xA8 = 0x101", "minimum apex of the end-of-rail jump (FUN_140578330 h_min)", "0x141252210");
row("rail.leave_height", 100, "cm", "UTresAICharState_JumpCodeDriven Init 0x14043E470: +0x98 = 100 (0x42C80000)", "apex above the higher end (FUN_140578330 h0)", "0x14043E470");
row("hop.accel_factor", 5.4, "-", "Hopping Begin 0x141242050: MaxAcceleration = +0xB8 x 5.4", "hop acceleration", "0x141242050");
row("hop.next_min", 0.85, "-", "Hopping tick 0x14124C010 sub 2: input magnitude > 0.85 searches the next point", "hop on / off", "0x14124C010");
row("hop.turn_min", 0.2, "-", "Hopping tick 0x14124C010 sub 2: magnitude > 0.2 and the state time >= 0.1 (FUN_1405A0B20)", "turning on the point", "0x14124C010");
row("hop.wait_min", 0.1, "s", "Hopping tick 0x14124C010 sub 2: FUN_1405A0B20 >= 0.1", "no input taken in the first 0.1 s on a point", "0x14124C010");
row("hop.turn_rate", 6, "1/s", "Hopping tick 0x14124C010: vt+0xDB0 alpha dt x 6", "facing toward the input yaw", "0x14124C010");
row("hop.search_start", 150, "cm", "Hopping tick 0x14124C010 sub 2: d = 150, 200 .. < 400 along the input yaw, radius 100, the highest point", "the next hop point", "0x14124C010");
row("hop.search_limit", 400, "cm", "Hopping tick 0x14124C010 sub 2 loop end (d < 400)", "the next hop point", "0x14124C010");
row("hop.search_step", 50, "cm", "Hopping tick 0x14124C010 sub 2 step 50", "the next hop point", "0x14124C010");
row("hop.search_radius", 100, "cm", "Hopping tick 0x14124C010 sub 2: vt+0x2350 radius 100", "the next hop point", "0x14124C010");
// (B5) the friends' jump / fall: UTresAICharState_NavLinkJumpCodeDriven (crate::jump)
row("jump.h0", 100, "cm", "UTresAICharState_JumpCodeDriven Init 0x14043E470: +0x98 = 100 (0x42C80000), +0x9C = 0", "FUN_140578330's apex above the higher end of a code-driven jump", "0x14043E470");
row("jump.dangle_height", 500, "cm", "JumpCodeDriven launch 0x140433FD0: ATresNpcPawnBase +0xB34 (ctor 0x141254FF0: 0x43FA0000), 500 for other pawns", "a code-driven target more than this above him searches the arc for a ledge (Dangle)", "0x140433FD0");
row("jump.rail_range", 150, "cm", "UTresAICharState_Jump tick 0x1404408A0: vt+0xEA0 range 150 (0x43160000), segment location -> +30 Z", "a rail is taken in the air", "0x1404408A0");
row("jump.turn_rate", 6, "1/s", "UTresAICharState_Jump 0x14045A5E0 phases 2 / 4: vt+0xDB0 alpha dt x 6", "the facing eases toward +0xA0 in the air", "0x14045A5E0");
row("jump.air_accel", 1000000, "cm/s^2", "0x14045A5E0 phases 2 / 4: MaxAcceleration vt+0x1640(1000000.0) with AddMovementInput(Rot(0, +0xA0, 0), 1)", "the air steering while +0xA4 / +0xA8", "0x14045A5E0");
row("jump.roll_height", 2000, "cm", "JumpCodeDriven sub-steps 0x140459D70 case 4: +0x148 < 2000 (and with input < 800) -> no RollLand", "a fall this high lands with RollLand", "0x140459D70");
row("jump.roll_height_input", 800, "cm", "0x140459D70 case 4: +0xA4 and +0x148 >= 800", "a fall this high with input lands with RollLand", "0x140459D70");
row("jump.stuck_period", 0.5, "s", "0x14045A5E0 phase 2: +0x100 + 0.5 < world time -> a height sample", "the stuck check samples the height twice a second", "0x14045A5E0");
row("jump.stuck_tolerance", 5, "cm", "0x14045A5E0 phase 2: 10 samples all within 5.0 of their mean, Velocity.Z <= 0", "stuck in the air (on an edge)", "0x14045A5E0");
row("jump.escape_time", 4, "s", "0x14045A5E0: phase 4 with +0x124 = 4.0 (0x40800000)", "the escape steering lasts this long or until landing", "0x14045A5E0");
row("jump.escape_speed", 500, "cm/s", "UTresAICharState_Jump 0x1404386F0: vt+0x1570(500) (0x43FA0000)", "the escape hop's MaxWalkSpeed (jump 3 x half height, the longest of 8 rays)", "0x1404386F0");
row("jump.arc_step", 0.05, "s", "JumpCodeDriven launch 0x140433FD0: the ledge search steps the arc by 0.05 s", "ledge search along a code-driven jump", "0x140433FD0");
// (B5) the climb probe FUN_140636BF0 (the non-player branch of FUN_140634E60) and UTresNpcState_Climbing (crate::climb)
row("climb.probe_ahead", 70, "cm", "FUN_140636BF0: the axis at the location + forward(XY) x 70", "climb probe", "0x140636BF0");
row("climb.probe_top", 260, "cm", "FUN_140636BF0: from Z + 260", "climb probe", "0x140636BF0");
row("climb.probe_drop", 180, "cm", "FUN_140636BF0: down 180 (to Z + 80)", "climb probe", "0x140636BF0");
row("climb.probe_radius", 35, "cm", "FUN_140636BF0: PxSphereGeometry {0, 35.0 (0x420C0000)} for FUN_144526380 (channels 0 / 1 / 0x12)", "climb probe sphere", "0x140636BF0");
row("climb.height_min", 45, "cm", "FUN_140636BF0: 45 <= impact Z - location Z", "lowest ledge he climbs", "0x140636BF0");
row("climb.height_max", 225, "cm", "FUN_140636BF0: impact Z - location Z <= 225", "highest ledge he climbs", "0x140636BF0");
row("climb.slope_max", 35, "deg", "FUN_140636BF0: -(atan2 of the impact normal's fall line) x 57.295776 <= 35", "the top no steeper than this", "0x140636BF0");
row("climb.facing_dot", 0.707107, "-", "FUN_140636BF0: dot(normalize2D(impact - location), forward) >= 0.707107", "the ledge within 45 deg of the facing", "0x140636BF0");
row("climb.high", 150, "cm", "FUN_140636BF0: height > 150 -> out +0x24 (ClimbingH, sub 1)", "high climb", "0x140636BF0");
row("climb.middle", 100, "cm", "FUN_140636BF0: 100 < height <= 150 -> out +0x25 (ClimbingMR)", "middle climb", "0x140636BF0");
row("climb.lift_low", 0.1, "-", "UTresNpcState_Climbing tick 0x141249650 sub 0: +0xB0 = 0.1 (0x3DCCCCCD)", "the traction lift's share of the gap per 60 Hz tick (low)", "0x141249650");
row("climb.lift_middle", 0.25, "-", "0x141249650 sub 0: +0xB0 = 0.25 (0x3E800000)", "the traction lift (middle)", "0x141249650");
row("climb.lift_high", 0.3, "-", "0x141249650 sub 0: +0xB0 = 0.3 (0x3E99999A)", "the traction lift (high)", "0x141249650");
// (B5) UTresNpcState_Dangle over UTresCharState_Dangle and the ledge probes of the jump
row("dangle.probe_top", 275, "cm", "0x140446070 / 0x140433FD0: up = (275 - m_DangleOffsetPos.Z) - 150", "the jump's ledge probe height", "0x140446070");
row("dangle.probe_below", 150, "cm", "0x140446070 / 0x140433FD0: (275 - Z) - 150", "the jump's ledge probe height", "0x140446070");
row("dangle.probe_down", 50, "cm", "0x140446070 / 0x140433FD0: FUN_140638740 / 900 down = 50 (0x42480000)", "the jump's ledge probe depth", "0x140446070");
row("dangle.probe_forward_npc", 0, "cm", "(party I1) 0x140446070 (FUN_140638740 5th argument 0.0) / 0x140433FD0 (FUN_140638900 7th argument 0.0)", "attempt 3 of FUN_140638900 (the only one for an NPC): no forward offset", "0x140638900");
row("dangle.probe_radius_scale_npc", 1.5, "-", "(party I1) 0x140446070: max(root +0x288, +0x28C) x 1.5; 0x140433FD0: fStack_228 (the same max) x 1.5", "the attempt-3 sphere radius = 1.5 x the capsule radius", "0x140638900");
row("dangle.begin_backoff", 10, "cm", "UTresCharState_Dangle Begin 0x141508DE0: Rot x (-10, 0, 0)", "the hang point tested from 10 cm behind", "0x141508DE0");
row("dangle.input_min", 0.5, "-", "iface vt+0x118 0x141270D90 state 6: magnitude > 0.5 -> +0xB8 / +0xB4", "the AI move counts as input", "0x141270D90");
row("dangle.cooldown", 0.6, "s", "UTresCharState_Dangle tick 0x14150F220: vt+0x2378(0.6 (0x3F19999A), 1)", "pawn +0x6F8 while hanging / on letting go", "0x14150F220");
row("dangle.climb_angle_ai", 90, "deg", "0x14150F220: an input pawn (vt+0x1A20: m_CharInputType PAD / DEBUG_PAD) climbs under 45 and lets go from 135; the friends (TRES_CHAR_INPUT_AI 0) climb under 90, else let go", "the hang's input rule for the AI friends", "0x14150F220");
row("situation.coop_range", 500, "cm", "n_ex002 SituationAction 0x1411CB9F0 phase 2: |coop actor - leader|^2 <= 250000 (XY)", "Goofy offers the coop situation command (0xCE) within it", "0x1411CB9F0");
row("situation.coop_turn", 180, "deg/s", "n_ex002 SituationAction 0x1411CB9F0 phase 1: vt+0xDE0(leader, 180 (0x43340000), 1)", "Goofy turns to the leader during GCoopStart", "0x1411CB9F0");
row('init.protector_slot_default', 1, 'slots', 'FTresChrInitEquip ctor 0x140570AC0: +0x11 m_InitProtectorSlot = 1', 'armour slots of a record that does not serialize the count', '0x140570AC0');
row('init.accessory_slot_default', 1, 'slots', 'FTresChrInitEquip ctor 0x140570AC0: +0x28 m_InitAccessorySlot = 1', 'accessory slots of a record that does not serialize the count', '0x140570AC0');
row('init.item_slot_default', 3, 'slots', 'FTresChrInitEquip ctor 0x140570AC0: +0x40 m_InitItemSlot = 3', 'item slots of a record that does not serialize the count', '0x140570AC0');
row('revive.default_time', 30, 's', 'UTresNpcState_Die Begin 0x1412413C0 (0x41F00000)', 'KO timer of a friend that is neither Donald nor Goofy', '0x1412413C0', 'inferred');
row('revive.hyper_healing_cap', 3, 'copies', 'UTresNpcState_Die Begin 0x1412413C0 and ATresNpcPawnBase 0x141264140: min(count(0xC3 HYPER_HEALING), 3)', 'timer / (1 + n); revive HP x (1 + n)', '0x1412413C0', 'inferred');
row('revive.hp_percent', 25, '%', 'ATresNpcPawnBase 0x141264140 (pawn slot 1075): max(1, RoundToInt(maxHP(vt+0x1820) * (n + 1) * 25 * 0.01))', 'HP a KO\'d friend gets back when the timer runs out (vt+0x1820 read as max HP: [C], slot not named)', '0x141264140', 'inferred');
row('revive.cure_half', 1, 'flag', 'kh-core status/runtime.rs Ally.dead (magic spec I 5)', 'a heal on a KO\'d friend is halved, rounded up; the Die tick gets up as soon as HP > 0 (0x141249F80 sub-step 1)', '0x141249F80', 'inferred');

// friend damage parameters: FriendDamageCommonParam CDO over the UTresDamageCommonParam constructor 0x14064C130
const fd = cdo('Blueprints/Npc/FriendDamageCommonParam');
const ctor = { m_SmallDamagePower: 450, m_SmallDamageBrake: 0.95, m_SmallDamageStartFallingSpeed: 100, m_AirSmallDamageVelocityZ: 0, m_BlowDamageVelocityZ: 125, m_BlowDamagePower: 5000, m_BlowDamageBrake: 0.85, m_AirBlowDamageVelocityZ: 125, m_AirBlowDamagePower: 2000, m_AirBlowDamageBrake: 0.85, m_AirBlowDamageStartBrakeTime: 0, m_BlowDamageStartFallingSpeed: 100, m_TossDamagePower: 1500, m_TossDamageFlightDuration: 1, m_TossDamageFallGravityScale: 1, m_LaunchDamagePower: null, m_LaunchDamageFlightDuration: null, m_LaunchDamageFallGravityScale: null, m_BeatDamagePower: 1500, m_ParabolaDamagePower: 1500, m_ParabolaDamageAngle: 45, m_ParabolaDamageBrake: 0.98, m_ParabolaDamageLimitBrake: 300 };
const dmgMap = [['small_power', 'm_SmallDamagePower'], ['small_brake', 'm_SmallDamageBrake'], ['small_start_falling_speed', 'm_SmallDamageStartFallingSpeed'], ['small_air_velocity_z', 'm_AirSmallDamageVelocityZ'], ['blow_ground_power', 'm_BlowDamagePower'], ['blow_ground_velocity_z', 'm_BlowDamageVelocityZ'], ['blow_ground_brake', 'm_BlowDamageBrake'], ['blow_air_power', 'm_AirBlowDamagePower'], ['blow_air_velocity_z', 'm_AirBlowDamageVelocityZ'], ['blow_air_brake', 'm_AirBlowDamageBrake'], ['blow_air_start_brake_time', 'm_AirBlowDamageStartBrakeTime'], ['blow_start_falling_speed', 'm_BlowDamageStartFallingSpeed'], ['toss_power', 'm_TossDamagePower'], ['toss_flight_duration', 'm_TossDamageFlightDuration'], ['toss_fall_gravity_scale', 'm_TossDamageFallGravityScale'], ['launch_power', 'm_LaunchDamagePower'], ['launch_flight_duration', 'm_LaunchDamageFlightDuration'], ['launch_fall_gravity_scale', 'm_LaunchDamageFallGravityScale'], ['beat_power', 'm_BeatDamagePower'], ['parabola_power', 'm_ParabolaDamagePower'], ['parabola_angle', 'm_ParabolaDamageAngle'], ['parabola_brake', 'm_ParabolaDamageBrake'], ['parabola_limit_brake', 'm_ParabolaDamageLimitBrake']];
for (const [id, prop] of dmgMap) {
  const v = fd[prop] ?? ctor[prop];
  row('damage.' + id, v ?? null, '-', fd[prop] != null ? `Blueprints/Npc/FriendDamageCommonParam CDO ${prop}` : `UTresDamageCommonParam constructor 0x14064C130 ${prop} (not in the friend CDO)`, 'kh-react DamageParams field of the friends\' damage state (UTresNpcState_Damage : UTresAICharState_Damage, plan 2.7)', '0x14064C130', v == null ? 'guess' : 'confirmed');
}

// wall hit: TresNpcState_Damage (vtable 0x146837C20) keeps UTresAICharState_Damage's getters: slot 150 = 0x14043A5D0
// (returns 30.0), slot 151 = 0x14043A5E0 (returns 50.0); the enemies' 0x140DD1C10 / 0x140DD1C90 read EnemyCommonParam
for (const [id, k, d] of [['burn_move_speed', 'm_BadStatBurnMoveSpeed', 480], ['burn_gravity_scale', 'm_BadStatBurnGravityScale', 5], ['burn_jump_height_max', 'm_BadStatBurnJumpHeightMax', 350], ['burn_jump_height_min', 'm_BadStatBurnJumpHeightMin', 50]]) row('damage.' + id, fd[k] ?? d, '-', fd[k] != null ? 'Blueprints/Npc/FriendDamageCommonParam CDO ' + k : 'UTresDamageCommonParam ctor 0x14064C130 ' + k, 'the NPC burn hop (getters 0x141246730 / 0x1412467B0 read +0xC0 / +0xC4)', '0x141246730');
row('damage.wall_hit_angle', 30, 'deg', 'UTresAICharState_Damage slot 150 0x14043A5D0 (movss constant 30.0), not overridden by TresNpcState_Damage', 'a blow ends on a wall when knock . normal <= -cos(angle)', '0x14043A5D0');
row('damage.wall_hit_speed', 50, 'cm/s', 'UTresAICharState_Damage slot 151 0x14043A5E0 (movss constant 50.0), not overridden by TresNpcState_Damage', 'a wall hit needs a horizontal speed above it', '0x14043A5E0');

// action definitions: class defaults of the BT action blueprints over their native constructors
const ACT = [
  ['n_ex001', 'Attack1_Beat'], ['n_ex001', 'Attack2_Swing'], ['n_ex001', 'Attack3_Jump'], ['n_ex001', 'Attack4_MagFire'], ['n_ex001', 'Attack5_MagBlizzard'], ['n_ex001', 'Attack6_MagThunder'], ['n_ex001', 'Attack7_MagAero'], ['n_ex001', 'Action2_MagCure'],
  ['n_ex002', 'Attack1_Push'], ['n_ex002', 'Attack2_Sweep'], ['n_ex002', 'Attack3_Jump'], ['n_ex002', 'Attack6_Tornado'], ['n_ex002', 'Attack7_Turbo'], ['n_ex002', 'Attack8_Discus'],
];
// native constructor defaults (decompiled C): base 0x14121BBF0, melee 0x14123F050, ranged 0x14123F0D0, jump base 0x14121BFA0,
// tornado 0x1411BB8E0, turbo 0x1411BB980, discus 0x1411BBA30
const NATIVE = {
  base: { m_MaxDistance: 500, m_YawTolerance: 90, m_StartYawMax: 135, 'm_AnimNotifyTractionParam.m_TractionDisMin': 0, 'm_AnimNotifyTractionParam.m_TractionDisMax': 100, 'm_AnimNotifyTractionParam.m_TractionMaxSpeed': 800, 'm_AnimNotifyTractionParam.m_TractionAcc': 8000, m_MinDistance: 0 },
  melee: { m_MinHeight: 0, m_MaxHeight: 100 },
  ranged: { m_YawTolerance: 30, m_ShotPitchMax: 45, m_ShotPitchMin: 0, m_ShotYawMinMax: 30 },
  jump: { m_JumpHeight: 150, m_JumpSpeed: 860, m_LimitHeight: 500, m_LimitMinHeight: 300, m_GravityScale: 2.5, m_MinMoveDistance: 1000, m_MaxMoveDistance: 1500, m_JumpAttackEnableZ: 50, m_JumpAttackRange: 80, m_JumpAttackUpRangeZ: 50, m_JumpAttackDownRangeZ: 50, m_JumpAttackDecelMoveSpeed: 3000, m_JumpAttackMinMoveSpeed: 0, m_JumpHomingSpeedEnable: 0, m_JumpHomingSpeed: 45, m_JumpHomingYawMaxEnable: 0, m_JumpHomingYawMax: 0, m_EnableSpeedCorrectionUp: 1, m_EnableSpeedCorrectionDown: 1 },
  tornado: { m_fMoveTime: 8, m_fMovemSpeed: 400, m_ExclusionNum: 3, m_fFlyHeight: 150, m_fFlySpeed: 150 },
  turbo: { m_fMoveTime: 8, m_fMoveSpeed: 680, m_fRangeMin: 50, m_fTurnAngle: 15, m_fTurnRate: 3, m_fSlalomTime: 0.7, m_fTargetHeight: 90, m_FallHeight: 150 },
  discus: { m_bEnableTargetShot: 1, m_fAttackTime: 5, m_fStartSpeed: 2000, m_fAttackFadeTime: 0.5, m_rHomingDisableYaw: 90, m_rHomingDisablePitch: 70, m_NpcPitchOffset: 0, m_NpcPitchTolerance: 0 },
};
const nativeOf = (asset) => {
  const kinds = ['base'];
  if (/Beat|Swing|Push|Sweep/.test(asset)) kinds.push('melee');
  if (/Mag|Discus/.test(asset)) kinds.push('ranged');
  if (/Jump/.test(asset)) kinds.push('jump');
  if (/Tornado/.test(asset)) kinds.push('tornado');
  if (/Turbo/.test(asset)) kinds.push('turbo');
  if (/Discus/.test(asset)) kinds.push('discus');
  return Object.assign({}, ...kinds.map((k) => NATIVE[k]));
};
const flat = (o, pre = '') => Object.entries(o).flatMap(([k, v]) => (v && typeof v === 'object' && !Array.isArray(v) ? flat(v, pre + k + '.') : [[pre + k, v]]));
for (const [ch, asset] of ACT) {
  const c = Object.fromEntries(flat(cdo(`Blueprints/Npc/${ch}/BT/${ch}_${asset}`)));
  const merged = { ...nativeOf(asset), ...c };
  for (const [k, v0] of Object.entries(merged)) {
    const v = typeof v0 === 'boolean' ? (v0 ? 1 : 0) : v0;
    if (typeof v !== 'number') continue;
    const id = `act.${ch}.${asset}.${k.replace(/^m_/, '').replace(/\.m_/g, '.')}`;
    row(id, v, '-', k in c ? `Blueprints/Npc/${ch}/BT/${ch}_${asset} CDO ${k}` : `native constructor default (${k})`, 'action definition number (plan 2.4)');
  }
}

// Donald's spell projectiles: class defaults of Blueprints/Npc/n_ex001/n_ex001_ProjectileMag* over their parents
// (Blueprints/Projectile/Magic*/TresProj_Mag*Base) over the native defaults (plan 2.4)
const SPELL_BASE = { Fire: 'MagicFire/TresProj_MagFireBase', Blizzard: 'MagicBlizzard/TresProj_MagBlizzardBase', Thunder: 'MagicThunder/TresProj_MagThunderBase', Aero: 'MagicAero/TresProj_MagAeroBase', Cure: 'MagicCure/TresProj_MagCureBase' };
// TresProjectileMovementComponent ctor 0x141470D50: InitialSpeed = MaxSpeed = 2000; homing off, the rest 0
const MOVE_NATIVE = { InitialSpeed: 2000, MaxSpeed: 2000, m_Accel: 0, m_bEnableHoming: false, m_HomingStartDelay: 0, m_HomingMinDist: 0, m_HomingTurnMax: 0, m_HomingTurnAccel: 0, m_HomingMaxTurnMax: 0 };
const SHAPE_KIND = { SPHERE: 0, CAPSULE1: 1, CONVEX: 2 };
function shapesOf(pth) {
  const d = readJson(path.join(PKG, pth + '.uasset.json'));
  const e = d.exports.find((x) => x.objectName === 'TresAtkColl0');
  const p = e && e.data && e.data.props && e.data.props.find((q) => q.name === 'CollisionShapesSrc');
  return p ? p.value.items.map((it) => Object.fromEntries(it.map((q) => [q.name, q.value]))) : null;
}
function moveOf(pth) { try { return exportProps(pth, 'ProjectileComp'); } catch (e) { return {}; } }
for (const spell of ['Fire', 'Blizzard', 'Thunder', 'Aero', 'Cure']) {
  const pth = `Blueprints/Npc/n_ex001/n_ex001_ProjectileMag${spell}`;
  const base = `Blueprints/Projectile/${SPELL_BASE[spell]}`;
  const c = cdo(pth), cb = cdo(base);
  const id = `proj.${spell.toLowerCase()}`;
  const life = c.m_MainLife ?? cb.m_MainLife;
  if (life != null) row(`${id}.life`, life, 's', `${c.m_MainLife != null ? pth : base} CDO m_MainLife`, 'projectile life');
  row(`${id}.no_destroy_on_hit`, (c.m_bNoDestroyOnHit ?? cb.m_bNoDestroyOnHit) ? 1 : 0, 'flag', `${pth} / ${base} CDO m_bNoDestroyOnHit`, '1 = the actor lives on after a hit (one hit per target)');
  for (const k of ['m_AppendCure', 'm_BonusAppendCure']) if (c[k] != null) row(`${id}.${k.replace(/^m_/, '')}`, c[k], 'HP', `${pth} CDO ${k}`, 'ATresProj_MagicCure 0x1414742A0: the target gets AppendCure + max(1, MagicPower); the others in DetectRange BonusAppendCure + max(1, MagicPower)', '0x1414742A0');
  if (spell === 'Cure') row(`${id}.DetectRange`, 300, 'cm', 'ATresProj_MagicCure ctor 0x14146FD60 m_DetectRange (not overridden)', 'the bonus heal reaches the target\'s team within this distance of the target', '0x1414742A0');
  const mo = moveOf(pth), mb = moveOf(base);
  if (Object.keys(mo).length || Object.keys(mb).length) for (const k of Object.keys(MOVE_NATIVE)) {
    const v = mo[k] ?? mb[k] ?? MOVE_NATIVE[k];
    const src = mo[k] != null ? `${pth} ProjectileComp ${k}` : mb[k] != null ? `${base} ProjectileComp ${k}` : `TresProjectileMovementComponent ctor 0x141470D50 default ${k}`;
    row(`${id}.${k.replace(/^m_b?/, '')}`, typeof v === 'boolean' ? (v ? 1 : 0) : v, /Turn/.test(k) ? 'deg/s' : /Delay/.test(k) ? 's' : 'cm/s', src, 'TresProjectileMovementComponent (homing 0x1414859E0: turn max grows by TurnAccel per s up to MaxTurnMax)', '0x141470D50');
  }
  const own = shapesOf(pth), sh = own || shapesOf(base) || [];
  sh.forEach((s, i) => {
    const src = `${own ? pth : base} TresAtkColl0 CollisionShapesSrc[${i}]`;
    const kind = SHAPE_KIND[String(s.m_ShapeType).replace(/.*::/, '')];
    if (kind == null) throw new Error(spell + ' shape ' + s.m_ShapeType);
    const r = (k, v, unit, what) => row(`${id}.shape${i}.${k}`, v, unit, src + ' ' + what, 'attack shape (kh-core ShapeDef::extent 0x140649E40: size + inc * t, capped at inc max; CONVEX tres_colltest01 = cylinder radius 100 x scale.x, z +-50 x scale.z)', '0x140649E40');
    r('kind', kind, '0 sphere / 1 capsule / 2 convex', 'm_ShapeType');
    r('size_x', s.m_Size.x, 'cm', 'm_Size.X'); r('size_z', s.m_Size.z, 'cm', 'm_Size.Z');
    r('inc_x', s.m_IncSize.x, 'cm/s', 'm_IncSize.X'); r('inc_max_x', s.m_IncMaxSize.x, 'cm', 'm_IncMaxSize.X');
    r('offset_z', s.m_RelativeLocation1.z, 'cm', 'm_RelativeLocation1.Z');
    r('scale_x', s.m_Scale.x, '-', 'm_Scale.X'); r('scale_z', s.m_Scale.z, '-', 'm_Scale.Z');
  });
  if (sh.length) row(`${id}.shapes`, sh.length, 'count', `${own ? pth : base} TresAtkColl0 CollisionShapesSrc`, 'number of attack shapes');
}
// n_ex001_MagicSet (TresPlayerMagicSet m_Units): the cast clip choice of FUN_14122D300 (unit +4 bit 0 m_bIsCastType:
// MagicCast / MagicAirCast, else MagicFire / MagicAirFire) and the spawn rule (m_bIsDirectPos: at the target position)
for (const u of exportProps('Blueprints/Npc/n_ex001/n_ex001_MagicSet', 'n_ex001_MagicSet').m_Units.items) {
  const o = Object.fromEntries(u.map((q) => [q.name, q.value]));
  const cmd = String(o.m_Command).replace(/.*TRES_CMD_KIND_/, '').toLowerCase();
  if (!/^(fire|blizzard|thunder|aero|cure)$/.test(cmd)) continue;
  const src = `Blueprints/Npc/n_ex001/n_ex001_MagicSet m_Units ${o.m_Command}`;
  row(`magicset.${cmd}.cast_type`, o.m_bIsCastType ? 1 : 0, 'flag', src + ' m_bIsCastType', '1 = MagicCast / MagicAirCast, 0 = MagicFire / MagicAirFire (FUN_14122D300 via FUN_141229630)', '0x14122D300');
  row(`magicset.${cmd}.direct_pos`, o.m_bIsDirectPos ? 1 : 0, 'flag', src + ' m_bIsDirectPos', '1 = the spell actor is put at the target position', '0x14122D300', 'inferred');
  row(`magicset.${cmd}.effect_group`, o.m_EffectGroup, 'id', src + ' m_EffectGroup', 'effect group of the cast (FUN_1405B8C30 with unit +8)', '0x14122D300');
}

{
  const s = shapesOfWeapon('Blueprints/Npc/n_ex002/w_gu_WepBase', 'Attack_Discus');
  row('proj.discus.radius', s.m_Size.x, 'cm', 'Blueprints/Npc/n_ex002/w_gu_WepBase TresAtkColl0 CollisionShapesSrc "Attack_Discus" m_Size.X (SPHERE at ATK_00)', 'the thrown shield\'s attack sphere');
  for (const [k, v] of Object.entries(exportProps('Blueprints/Npc/n_ex002/w_gu_WepBase', 'TresWeaponMovement0'))) if (typeof v === 'number' || typeof v === 'boolean') row('proj.discus.' + k.replace(/^m_b?/, ''), typeof v === 'boolean' ? (v ? 1 : 0) : v, /Turn/.test(k) ? 'deg/s' : 'cm/s', 'Blueprints/Npc/n_ex002/w_gu_WepBase TresWeaponMovement0 ' + k, 'the thrown shield TresWeaponMovementComponent (projectile law 0x1414729E0; homing 0x1414859E0)', '0x1411CD930');
}
function shapesOfWeapon(pth, grp) {
  const d = readJson(path.join(PKG, pth + '.uasset.json'));
  const e = d.exports.find((x) => x.objectName === 'TresAtkColl0');
  const it = e.data.props.find((q) => q.name === 'CollisionShapesSrc').value.items.map((x) => Object.fromEntries(x.map((q) => [q.name, q.value]))).find((x) => x.m_GrpName === grp);
  if (!it) throw new Error('no group ' + grp + ' in ' + pth);
  return it;
}

// ---- levels
const LV = [];
for (const ch of ['n_ex001', 'n_ex002']) for (const r of table(`DataTable/Npc/LevelUp/${ch}_LevelUpData`)) LV.push([`${ch}.${r._row_name}`, ch, +r._row_name, r.m_Exp, r.m_AttackPower, r.m_MagicPower, r.m_DefensePower, r.m_AbilityPoint]);

// ---- attacks
const AT = [];
const enumTail = (s) => String(s || '').replace(/^.*::/, '');
for (const ch of ['n_ex001', 'n_ex002']) for (const r of table(`DataTable/Npc/Attack/${ch}_AttackDataTable`)) {
  AT.push([`${ch}.${r._row_name}`, ch, r._row_name, r.m_Power, enumTail(r.m_DamageKind), enumTail(r.m_DamageAttribute), r.m_DamagePowerScale ?? 1, r.m_DamageParabolaAngle ?? 0, r.m_DamageMoveLength ?? 0, r.m_DamageBrakeParam ?? 0, enumTail(r.m_KnockbackType) || 'NORMAL', r.m_DamageEffectTime ?? 0, enumTail(r.m_BadStatusKind) || 'NONE', r.m_BadStatusGenRate ?? 0, r.m_BadStatusEffectTime ?? 0, enumTail(r.m_TeamCheckType) || 'DEFAULT', r.m_ReactionPowerValue ?? 0, !!r.m_bIsMagicAttack, r.m_ReflectLevel ?? 0]);
}

// ---- states
const ST = [
  ['Idle', 'TresNpcState_Idle', '0x141242140', 'idle / idle break / battle stance'],
  ['Move', 'TresNpcState_Run', '0x14042FBE0', 'AI move under a locomotion definition (UTresAICharState_Run ctor 0x14042FBE0)'],
  ['Turn', 'TresNpcState_Turn', '0x14042FCA0', 'turn in place (UTresAICharState_Turn ctor)'],
  ['Jump', 'TresNpcState_Jump', '0x14123FFB0', 'jump id 3 (ctor; Init 0x141247E10, Begin 0x1412422F0, Tick 0x14124D680, End 0x1412453E0): registered by ATresNpcPawnBase 0x141254FF0 but never requested for the friends (RequestJump 0x14054C2E0 takes 0x2B first): B5 NavLinkJump'],
  ['NavLinkJump', 'TresAICharState_NavLinkJumpCodeDriven', '0x14042FA70', 'B5: the friends\' jump / fall, TSID_STATE_NAVLINK_JUMP 0x2B (vtable 0x145C963E8: Init 0x14043E470, Begin 0x140433610, Tick 0x1404408A0, End 0x140435040, sub-steps 0x140459D70, air phase 0x14045A5E0, air probes 0x140446070, launch 0x140433FD0, stuck 0x140438BD0); RequestJump iface vt+0x510 0x14054C2E0 (fall, vt+0x398 0x140458D00), JumpCodeDriven vt+0x3A8 0x14054C6A0 (target, vt+0x390 0x140459150); clips 0x141263A00'],
  ['Climbing', 'TresNpcState_Climbing', '0x14123F7F0', 'B5: the vault, id 7 (vtable 0x146837448: Init 0x141247B10, Begin 0x141241270, Tick 0x141249650, End 0x141244460, CanChange 0x1412405A0); request iface vt+0x538 0x141276490 from the Run / SlopeSlide / SuperSlide ticks with the NPC probe FUN_140636BF0; clips ClimbingR / MR / H, SonicSlideClimbing(M / H), LightLandR / L'],
  ['Dangle', 'TresNpcState_Dangle', '0x14123F870', 'B5: the hang, id 6 over UTresCharState_Dangle (Init 0x14150DEA0, Begin 0x141508DE0, Tick 0x14150F220, End 0x14150BB80); request iface vt+0x208 0x141276580 from the jump air probe 0x140446070 after a ledge-aimed launch 0x140433FD0; clips GripStart / GripLoop / GripRise'],
  ['Action', 'TresAICharState_Action', '0x14042F390', 'one action definition running (ctor); AttackBase tick 0x1404404B0'],
  ['PlayMotion', 'TresAICharState_PlayMotion', '0x14042FAF0', 'play a named motion (ctor)'],
  ['Damage', 'TresNpcState_Damage', '0x14123F830', 'damage reaction over UTresAICharState_Damage (kh-react with FriendDamageCommonParam)'],
  ['Reflect', 'TresNpcState_Reflect', '0x141240040', 'attack repelled (ctor); tick 0x14124F080'],
  ['BadStatStun', 'TresNpcState_BadStatStun', '0x14123F770', 'stun'],
  ['BadStatBurn', 'TresNpcState_BadStatBurn', '0x14123F630', 'burn'],
  ['BadStatSneeze', 'TresNpcState_BadStatSneeze', '0x14123F6B0', 'sneeze'],
  ['Die', 'TresNpcState_Die', '0x14123F8B0', 'KO: Begin 0x1412413C0, Tick 0x141249F80, End 0x141244670, anim names 0x141246E00 (BadStatStun* clips)'],
  ['UseItem', 'TresNpcState_UseItem', '0x141255F30', 'item use (anim names 0x141263D30: ItemUse / ItemAirUse)'],
  ['Warp', 'TresNpcState_Warp', '0x141255F70', 'warp to the EQS point near Sora'],
  ['SlopeSlide', 'TresNpcState_SlopeSlide', '0x141266A80', 'B4: slope slide (Init 0x141266A80, Begin 0x14125B8A0, Tick 0x14126C950, End 0x14125E310); state id 10, requested by the Run tick 0x140442430 through iface vt+0x520 0x141276B10'],
  ['SuperSlide', 'TresNpcState_SuperSlide', '0x141266B90', 'B4: super slide (Init 0x141266B90, Begin 0x14125BA50, Tick 0x14126D3B0, End 0x14125E460); id 0x15, requested by iface vt+0x240 0x141271170 -> vt+0x248 0x141276B70'],
  ['RailSlide', 'TresNpcState_RailSlide', '0x141247F00', 'B4: rail slide over UTresCharState_RailSlide (Init 0x141247F00, Begin 0x1412423C0, Tick 0x14124E890 -> base 0x141510EE0, End 0x141245430, leave slot 145 0x141252210); id 0x16'],
  ['Hopping', 'TresNpcState_Hopping', '0x141247D60', 'B4: hop points (Init 0x141247D60, Begin 0x141242050, Tick 0x14124C010, End 0x141245270); id 9, requested by iface vt+0x530 0x141276660'],
  ['SituationAction', 'TresNpcState_SituationAction', '0x141247F40', 'B4: pawn situation 6 / 8 / 9 -> inner TalkPointWait / GimmickWait / NpcMotionPlay (Init 0x141247F40 -> 0x141247430, Begin 0x141242570, Tick 0x14124F1E0, End 0x1412454B0); Goofy n_ex002 override situation 7 GCoop (0x1411BD5D0 / 0x1411CB950 -> 0x1411CB9F0); id 0x93'],
];
const STATES = ST.map(([id, cls, ctorAddr, what]) => [id, cls, ctorAddr, what]);

// ---- weapons (TresEquipItemData I03021..I03034 Donald, I03041..I03054 Goofy)
const WP = table('Load/Tres/TresEquipItemData').filter((r) => /^I030[2-5]\d$/.test(r._row_name)).map((r) => [r._row_name, r.m_KeyName, +r._row_name.slice(1) < 3041 ? 'n_ex001' : 'n_ex002', r.m_Comment ? r.m_Comment.s : '', r.m_AttackPlus, r.m_MagicPlus, r.m_DefensePlus, (r.m_AppendAbility.items || []).map(enumTail).join(' '), String(r.m_EquipSubclass || '').replace(/^.*\/(w_[a-z]{2}\d{3})_Equip.*$/, '$1')]);
// ---- the party's abilities (TresAbilityData): AP and MP of the rows the friends use
const AB = table('Load/Tres/TresAbilityData').filter((r) => /DONALD_|GOOFY_|HYPER_HEALING|COMBI_|SHARE_PRIZE|PRIZE_DRAW|DEFENDER|HARVEST|LUCK_UP|MP_HASTE$|FIRE_UP|BLIZZARD_UP|THUNDER_UP|AERO_UP|DAMAGE_ASPIR|LAST_LEAVE|CRITICAL_HALF|COMBO_LEAVE/.test(String(r.m_KeyName))).map((r) => [String(r.m_KeyName), r.m_UseAP, r.m_UseMP]);

// ---- write sheets
const PCOLS = ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'];
const write = (name, columns, rows) => fs.writeFileSync(path.join(SHEETS, name + '.json'), JSON.stringify({ sheet: name, group: 'port', columns, rows }, null, 1) + '\n');
write('party-params', PCOLS, P);
write('party-levels', ['id', 'chara', 'level', 'exp', 'strength', 'magic', 'defense', 'ap'], LV);
write('party-attacks', ['id', 'chara', 'row', 'power', 'damage_kind', 'attribute', 'power_scale', 'parabola_angle', 'move_length', 'brake', 'knockback', 'effect_time', 'bad_status', 'bad_rate', 'bad_time', 'team_check', 'reaction_power', 'magic', 'reflect_level'], AT);
write('party-states', ['id', 'class', 'ctor', 'what'], STATES);

// ---- clips (stand-in until stream A's tables)
const sets = readJson(path.join(ROOT, 'build', 'sora_tmp', 'party', 'animsets.json'));
function clip(gamePath) {
  const content = gamePath.replace(/^\/Game\//, '').replace(/\.[^/]*$/, '');
  const f = path.join(ROOT, 'assets_converted', 'animations', 'KINGDOM HEARTS III', 'Content', content + '.anim.json');
  if (!fs.existsSync(f)) return null;
  const side = readJson(f);
  if (side.class !== 'AnimSequence') return null;
  const s = Object.fromEntries(side.data.props.map((x) => [x.tag.name, x.tag.type === 'BoolProperty' ? x.tag.boolVal === 1 : x.value]));
  const pkg = sp.loadPkg(content);
  const notifies = [];
  for (const it of s.Notifies ? s.Notifies.items : []) {
    const o = Object.fromEntries(it.props.map((x) => [x.tag.name, x.value]));
    const idx = (o.Notify && o.Notify.object) || (o.NotifyStateClass && o.NotifyStateClass.object) || 0;
    const exp = idx > 0 ? pkg.exports[idx - 1] : null;
    if (!exp) continue;
    const props = exp.data && exp.data.props ? Object.fromEntries(exp.data.props.map((p) => [p.name, p.value])) : {};
    const start = +(o.LinkValue ?? o.DisplayTime ?? 0), dur = +(o.Duration ?? 0);
    const isState = !!(o.NotifyStateClass && o.NotifyStateClass.object);
    let data = 'ClipNotifyData::None';
    const cls = exp.class.replace(/_C$/, '');
    if (cls === 'TresAnimNotifyState_Attack') data = `ClipNotifyData::Attack { attack: ${JSON.stringify(String(props.m_AttackDataIDName || 'None'))}, collision: ${JSON.stringify(String(props.m_AttackCollisionGrpName || props.m_CollisionGroupName || 'None'))} }`;
    else if (isState) data = `ClipNotifyData::State { name: ${JSON.stringify(cls.replace(/^.*NotifyState_/, ''))} }`;
    else data = `ClipNotifyData::Raw(${JSON.stringify(cls.replace(/^.*Notify_/, ''))})`;
    notifies.push({ cls, start, end: isState ? start + Math.max(dur, 1e-4) : start, data });
  }
  notifies.sort((a, b) => a.start - b.start);
  return { sequence: path.basename(content), length: s.SequenceLength, rate: s.RateScale ?? 1, blend: s.m_InterpolationTime ?? 0.2, root: !!s.bEnableRootMotion, notifies };
}
const f32 = (x) => { const s = (+x).toString(); return /[.eE]/.test(s) ? s : s + '.0'; };
let rs = `// GENERATED by tools/party_sheet.js from sheets/port/party-{params,levels,attacks,states}.json and the anim sidecars - DO NOT EDIT\n\n`;
rs += `/// One scalar of party-params: (id, value).\npub static PARTY_PARAMS: &[(&str, f32)] = &[\n${P.filter((r) => typeof r[1] === 'number').map((r) => `    (${JSON.stringify(r[0])}, ${f32(r[1])}),`).join('\n')}\n];\n\n`;
rs += `/// party-levels: (chara uid, level, exp, strength, magic, defense, ap).\npub static PARTY_LEVELS: &[(u8, u32, i32, i32, i32, i32, i32)] = &[\n${LV.map((r) => `    (${r[1] === 'n_ex001' ? 11 : 12}, ${r[2]}, ${r[3]}, ${r[4]}, ${r[5]}, ${r[6]}, ${r[7]}),`).join('\n')}\n];\n\n`;
rs += `/// party-attacks rows.\npub static PARTY_ATTACKS: &[AttackRow] = &[\n${AT.map((r) => `    AttackRow { chara: ${r[1] === 'n_ex001' ? 11 : 12}, name: ${JSON.stringify(r[2])}, power: ${f32(r[3])}, damage_kind: ${JSON.stringify(r[4])}, attribute: ${JSON.stringify(r[5])}, power_scale: ${f32(r[6])}, parabola_angle: ${f32(r[7])}, move_length: ${f32(r[8])}, brake: ${f32(r[9])}, knockback: ${JSON.stringify(r[10])}, effect_time: ${f32(r[11])}, bad_status: ${JSON.stringify(r[12])}, bad_rate: ${f32(r[13])}, bad_time: ${f32(r[14])}, team_check: ${JSON.stringify(r[15])}, reaction_power: ${r[16] | 0}, magic: ${r[17]}, reflect_level: ${r[18] | 0} },`).join('\n')}\n];\n\n`;
for (const ch of ['n_ex001', 'n_ex002']) {
  const set = sets[`Character/npc/${ch}/anm/${ch}_AnimSet`];
  const seen = new Set(); const items = [];
  for (const r of set.rows) {
    if (!r.asset || typeof r.name !== 'string' || seen.has(r.name) || r.name === 'None') continue;
    const c = clip(r.asset); if (!c) continue;
    seen.add(r.name);
    const ns = c.notifies.map((n) => `ClipNotify { class: ${JSON.stringify(n.cls)}, start: ${f32(n.start)}, end: ${f32(n.end)}, data: ${n.data} }`).join(', ');
    items.push(`    ClipDef { name: ${JSON.stringify(r.name)}, sequence: ${JSON.stringify(c.sequence)}, length: ${f32(c.length)}, rate: ${f32(c.rate)}, blend_in: ${f32(c.blend)}, looping: ${/Loop|Idle|^Swim|Dive|Walk|RunLoop/.test(r.name) && !/End|Start/.test(r.name)}, root_motion: ${c.root}, notifies: &[${ns}], root: &[], socket: &[] },`);
  }
  rs += `/// ${ch}_AnimSet: every AnimSequence row (${items.length}); lengths, rates, notifies from the anim sidecars. Looping is the row-name convention (Loop / Idle), [D].\npub static ${ch.toUpperCase()}_MIN_CLIPS: &[ClipDef] = &[\n${items.join('\n')}\n];\n\n`;
}
fs.mkdirSync(GEN, { recursive: true });
fs.writeFileSync(path.join(GEN, 'party_data.rs'), rs);
console.log(`party_sheet: params ${P.length}, levels ${LV.length}, attacks ${AT.length}, states ${STATES.length}; ${path.join(GEN, 'party_data.rs')}`);
// (appended) weapons and abilities: sheets and Rust
write('party-weapons', ['id', 'key', 'chara', 'name', 'strength', 'magic', 'defense', 'abilities', 'model'], WP);
write('party-abilities', ['id', 'ap', 'mp'], AB);
fs.appendFileSync(path.join(GEN, 'party_data.rs'), `/// party-weapons: (item row, key, chara uid, strength, magic, defense, abilities, model).\npub static PARTY_WEAPONS: &[(&str, &str, u8, i32, i32, i32, &[&str], &str)] = &[\n${WP.map((r) => `    (${JSON.stringify(r[0])}, ${JSON.stringify(r[1])}, ${r[2] === 'n_ex001' ? 11 : 12}, ${r[4]}, ${r[5]}, ${r[6]}, &[${r[7].split(' ').filter(Boolean).map((x) => JSON.stringify(x)).join(', ')}], ${JSON.stringify(r[8])}),`).join('\n')}\n];\n\n/// party-abilities: (key, AP, MP; MP -1 = all).\npub static PARTY_ABILITIES: &[(&str, i32, i32)] = &[\n${AB.map((r) => `    (${JSON.stringify(r[0])}, ${r[1] | 0}, ${r[2] | 0}),`).join('\n')}\n];\n`);
console.log(`party_sheet: weapons ${WP.length}, abilities ${AB.length}`);

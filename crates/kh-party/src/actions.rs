//! The action definitions (plan 2.4) run inside `TresAICharState_Action` / `AttackBase`:
//! Donald's Beat / Swing / Jump / Fire / Blizzard / Thunder / Aero / Cure, Goofy's Push / Sweep /
//! Jump / Tornado / Turbo / Discus. Numbers come from the BT action blueprints' class defaults
//! over their native constructors (sheet party-params `act.*`); the sub-step machines of
//! Tornado (Begin 0x1411C2130, Tick 0x1411C61D0, End 0x1411C8490) and Turbo (Begin 0x1411C2530,
//! Tick 0x1411C6CB0, steering 0x1411D0430) are transcribed from the decompiled C; the melee base
//! is `UTresAICharState_AttackBase` (Begin 0x140433040, Tick 0x1404404B0).

use crate::command::{ActionDef, TargetRef};
use crate::ids::Chara;
use crate::pawn::{forward, friend_damage, horizontal, turn_rate, yaw_of, NpcPawn, Request, StepCtx, TurnKind};
use crate::projectile::{Projectile, ProjectileKind};
use crate::host::PartyHost;
use crate::tables;
use crate::view::PartyEventKind;
use glam::Vec3;
use kh_core::enums::{ETresDamageAttribute, ETresDamageKind};
use kh_core::host::{HitAttacker, HitInfo, HitRecord, HurtShape, TargetFlags, TargetId};
use kh_core::status::{Defender, ItemKind};
use kh_react::DamageParams;

/// The running action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionRun {
    pub def: Option<ActionDef>,
    pub item: Option<ItemKind>,
    pub target: TargetRef,
    /// `m_SubStep`.
    pub sub: u8,
    /// Timers of the definition (Tornado +0x1EC, Turbo +0x200 / slalom +0x1F8).
    pub timer: f32,
    pub timer2: f32,
    /// Turbo's current speed (+0x204), slalom phase (+0x1F4) and weave angle (+0x20C).
    pub speed: f32,
    pub phase: u8,
    pub weave: f32,
    /// Tornado: fly mode flag (+0x1F4).
    pub flying: bool,
    pub shot: bool,
    /// Discus: +0x20E the shield has hit something (set by the hit handler 0x1411CC550), the fade timer +0x218.
    pub caught: bool,
    pub success: bool,
    pub start: Vec3,
    /// `UTresNpcAttackDefinitionJumpBase` run fields (Jump actions only).
    pub jump: JumpRun,
    /// (party P3) Discus thrown by a pad-driven Goofy with nothing to throw at (our design, `on_shot`): the shield
    /// flies out and back to him, the catch waits for its return.
    pub free_throw: bool,
    /// (party X2) A SORA SPELL cast by the friend leader under this action (the command: 29 Fire .. 51 Curaga, a grand
    /// magic 110 ..); 0 = the member's own KH3 action. leader_magic.rs runs its Begin and Shot.
    pub sora_cmd: u8,
}

/// The fields of `UTresNpcAttackDefinitionJumpBase` the jump attack runs on (offsets of the definition;
/// Begin 0x141220750 clears them).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct JumpRun {
    /// +0x254: launched (the flight part of the tick runs); +0x255: face the goal each tick.
    pub launched: bool,
    pub facing: bool,
    /// +0x256: the "target above the jump height" launch: Flying mode, gravity off, own vertical speed.
    pub own_vertical: bool,
    /// +0x257: the "target far below" launch (m_GravityScale applied, the aim clamped to the move distances).
    pub down_far: bool,
    /// +0x258: the jump attack clip has started; +0x259 (set by Begin, cleared when the goal is passed).
    pub attacked: bool,
    pub b259: bool,
    /// +0x25A: decelerate the move speed (m_JumpAttackDecelMoveSpeed): set by the TresAnimNotify_Timing handler
    /// 0x14122DE40 (m_Param 0) of the jump clips M_1311 / M_1312, cleared by Begin 0x141220750.
    pub decel: bool,
    /// +0x25C the horizontal move speed; +0x260 / +0x264 the own vertical speed and its gravity.
    pub speed: f32,
    pub vz: f32,
    pub g: f32,
    /// +0x270: the actor's forward at the launch.
    pub launch_dir: Vec3,
    /// pawn +0xB98: the goal (the target's root, refreshed each tick while there is a target).
    pub goal: Vec3,
    /// The definition's m_bEnableChange (the EnableChange notify of the running clip).
    pub enable_change: bool,
}

impl ActionRun {
    pub fn new(def: ActionDef, target: TargetRef) -> ActionRun {
        ActionRun { def: Some(def), item: None, target, sub: 0, timer: 0.0, timer2: 0.0, speed: 0.0, phase: 0, weave: 0.0, flying: false, shot: false, caught: false, success: false, start: Vec3::ZERO, jump: JumpRun::default(), free_throw: false, sora_cmd: 0 }
    }

    pub fn item(item: ItemKind, target: TargetRef) -> ActionRun {
        ActionRun { item: Some(item), def: None, ..ActionRun::new(ActionDef::DonaldBeat, target) }
    }
}

/// The friends' damage parameters: `FriendDamageCommonParam` over the `TresDamageCommonParam`
/// constructor 0x14064C130 (sheet rows `damage.*`).
pub fn friend_damage_params(gravity_scale: f32) -> DamageParams {
    let p = |k: &str| tables::param(&format!("damage.{k}"));
    DamageParams {
        small_power: p("small_power"),
        small_brake: p("small_brake"),
        small_start_falling_speed: p("small_start_falling_speed"),
        small_air_velocity_z: p("small_air_velocity_z"),
        blow_ground_power: p("blow_ground_power"),
        blow_ground_velocity_z: p("blow_ground_velocity_z"),
        blow_ground_brake: p("blow_ground_brake"),
        blow_air_power: p("blow_air_power"),
        blow_air_velocity_z: p("blow_air_velocity_z"),
        blow_air_brake: p("blow_air_brake"),
        blow_air_start_brake_time: p("blow_air_start_brake_time"),
        blow_start_falling_speed: p("blow_start_falling_speed"),
        toss_power: p("toss_power"),
        toss_flight_duration: p("toss_flight_duration"),
        toss_fall_gravity_scale: p("toss_fall_gravity_scale"),
        launch_power: p("launch_power"),
        launch_flight_duration: p("launch_flight_duration"),
        launch_fall_gravity_scale: p("launch_fall_gravity_scale"),
        beat_power: p("beat_power"),
        parabola_power: p("parabola_power"),
        parabola_angle: p("parabola_angle"),
        parabola_brake: p("parabola_brake"),
        parabola_limit_brake: p("parabola_limit_brake"),
        // the state's slots 150 / 151: UTresAICharState_Damage 0x14043A5D0 (30 deg) / 0x14043A5E0 (50 cm/s),
        // kept by TresNpcState_Damage (the enemies' overrides read EnemyCommonParam instead).
        wall_hit_speed: p("wall_hit_speed"),
        wall_hit_angle: p("wall_hit_angle"),
        gravity: DamageParams::ENEMY.gravity,
        gravity_scale,
    }
}

fn asset(def: ActionDef) -> &'static str {
    def.asset()
}

/// The ability whose MP the action spends (`m_NpcAIInfo.m_AttackAbilityKind`).
pub(crate) fn ability(def: ActionDef) -> Option<&'static str> {
    Some(match def {
        ActionDef::DonaldFire => "DONALD_FIRE",
        ActionDef::DonaldBlizzard => "DONALD_BLIZZARD",
        ActionDef::DonaldThunder => "DONALD_THUNDER",
        ActionDef::DonaldAero => "DONALD_AERO",
        ActionDef::DonaldCure => "DONALD_CURE",
        ActionDef::GoofyTornado => "GOOFY_TORNADE",
        ActionDef::GoofyTurbo => "GOOFY_TURBO",
        ActionDef::GoofyDiscus => "GOOFY_FLIP",
        _ => return None,
    })
}

/// (party I1) `UTresActionDefinitionBase::m_ViableStates` (+0xA8) of the BT action assets [A]
/// (Blueprints/Npc/n_ex00x/BT/n_ex00x_Attack*/Action2_MagCure): every one lists TSID_STATE_AI_MOVE; the magic
/// (Fire / Blizzard / Thunder / Aero / Cure), Tornado, Turbo and Discus also TSID_STATE_AI_SWIM (the water
/// melee Attack8_WaterBeat / Attack9_WaterSwing / Attack4_WaterUpper / Attack5_WaterRush list only AI_SWIM).
pub fn viable_swim(def: ActionDef) -> bool {
    !matches!(def, ActionDef::DonaldBeat | ActionDef::DonaldSwing | ActionDef::DonaldJump | ActionDef::GoofyPush | ActionDef::GoofySweep | ActionDef::GoofyJump)
}

/// (party I1) The definition's CanStart (CDO vt+0x3C0 = 0x140518070): IDLE with nothing queued (the port
/// queues no state), else the running state's id in `m_ViableStates`. No running action is ever replaced by a
/// new one for the friends (FUN_140520410 needs the AI_ActionAbortTiming window and an abort id their trees
/// never set; RequestAction 0x140547B40). P's pad states (`Pad`) take their own actions (port design).
pub fn viable_in(_def: ActionDef, state: crate::NpcStateId) -> bool {
    use crate::NpcStateId as S;
    match state {
        S::Idle | S::Move | S::Pad(_) => true,
        _ => false,
    }
}

/// The clip a melee definition names (`m_AttackAnimData`) [A].
fn melee_clip(def: ActionDef) -> Option<&'static str> {
    Some(match def {
        ActionDef::DonaldBeat | ActionDef::GoofyPush => "M_1301",
        ActionDef::DonaldSwing | ActionDef::GoofySweep => "M_1302",
        _ => return None,
    })
}

/// Begin. False = the action cannot start (the command fails).
pub fn begin(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) -> bool {
    let Some(mut run) = p.action else { return false };
    let Some(def) = run.def else { return false };
    if def.chara() != p.chara {
        return false;
    }
    run.start = p.position();
    // (party X2) the friend leader's Sora spell parked by his menu: its own Begin (no KH3 ability gate: Sora's cost at
    // the Shot)
    if let Some(cast) = crate::leader_magic::take_cast(p, def) {
        return crate::leader_magic::begin(p, run, cast, cx, host);
    }
    // the ability gate FUN_1405A4EA0(pawn, kind, 1): equipped and payable, then MP is spent
    if let Some(key) = ability(def) {
        if !crate::pad::goofy::pays_nothing(p) /* (party X3G) Discus Barrage: the focus paid */ && !p.status.pay(key) {
            return false;
        }
    }
    let target_pos = p.target_point(run.target, cx, host);
    // (party P3) a pad-driven member's target at his own place (Cure on himself) gives no direction: no start turn
    // (atan2(0, 0) turned him toward the world's +X). The AI keeps the transcribed rule.
    let target_pos_turn = target_pos.filter(|t| p.pad.is_none() || horizontal(*t - p.position()).length() > 1.0);
    if let Some(t) = target_pos_turn {
        // m_bEnableStartYawMax / m_StartYawMax 135: the start turn toward the target.
        let want = yaw_of(horizontal(t - p.position()));
        let max = tables::aparam(p.chara, asset(def), "StartYawMax");
        let diff = crate::pawn::wrap180(want - p.yaw);
        p.yaw = crate::pawn::wrap180(p.yaw + diff.clamp(-max, max));
    } else if let Some(want) = p.pad.as_ref().filter(|d| d.stick_magnitude >= kh_core::params::SORA_PARAMS.input_stick_active_min).map(|d| d.stick_yaw) {
        // (party P3) a pad-driven member without a target starts toward the held stick (camera-relative), with the
        // same m_StartYawMax turn, as Sora's attacks face the stick when nothing is targeted (our design: KH3's
        // friends always act on a target)
        let max = tables::aparam(p.chara, asset(def), "StartYawMax");
        let diff = crate::pawn::wrap180(want - p.yaw);
        p.yaw = crate::pawn::wrap180(p.yaw + diff.clamp(-max, max));
    }
    p.cmc.stop_movement();
    match def {
        ActionDef::DonaldBeat | ActionDef::DonaldSwing | ActionDef::GoofyPush | ActionDef::GoofySweep => {
            // AttackBase Begin 0x140433040: StopMovement, play the clip with blend 0.1.
            p.play(melee_clip(def).unwrap(), 0.1);
        }
        ActionDef::DonaldJump | ActionDef::GoofyJump => {
            jump_begin(p, &mut run, cx, host);
        }
        ActionDef::DonaldFire | ActionDef::DonaldBlizzard => {
            // STAND-IN [?]: which magic clip each spell plays is in n_ex001_MagicSet (not decoded);
            // the thrown spells use MagicFire (1321), the others MagicCast (1341).
            p.play(if p.on_ground() { "MagicFire" } else { "MagicAirFire" }, 0.1);
            p.events.push(PartyEventKind::SpellCast { def });
            set_cast_voice(p, def, run.target, cx, host);
            cast_effect(p, def, host); // (party P3)
        }
        ActionDef::DonaldThunder | ActionDef::DonaldAero | ActionDef::DonaldCure => {
            p.play(if p.on_ground() { "MagicCast" } else { "MagicAirCast" }, 0.1);
            p.events.push(PartyEventKind::SpellCast { def });
            set_cast_voice(p, def, run.target, cx, host);
            cast_effect(p, def, host); // (party P3)
        }
        ActionDef::GoofyTornado => {
            // Begin 0x1411C2130: target, exclusion list cleared, M_1321, sub-step 0.
            p.play("M_1321", -1.0);
            run.sub = 0;
            run.timer = 0.0;
        }
        ActionDef::GoofyTurbo => {
            // Begin 0x1411C2530: retarget flag on, fly flag = !falling, slalom timer, M_1324.
            run.timer2 = tables::aparam(p.chara, "Attack7_Turbo", "fSlalomTime");
            run.phase = 0;
            run.weave = 0.0;
            p.play("M_1324", -1.0);
            run.sub = 0;
        }
        ActionDef::GoofyDiscus => {
            // Begin 0x1411C26C0: ability gate, timers +0x210 / +0x218 and the flags +0x20C / +0x20E
            // cleared; in the air (FUN_1405AD820) the pitch is levelled; M_1330 in the air, else M_1327.
            p.play(if p.on_ground() { "M_1327" } else { "M_1330" }, -1.0);
            run.sub = 0;
            run.timer = 0.0;
            run.timer2 = 0.0;
        }
    }
    p.action = Some(run);
    true
}

// ------------------------------------------------------------------ the jump attack
//
// `UTresAttack3_n_ex00x_Jump` are `UTresNpcAttackDefinitionJumpBase` with no code of their own
// (Z_Construct 0x141B59030 / 0x141B59140). JumpBase vtable: Begin slot 121 0x141220750, Tick 122
// 0x141222A00, End 123 0x1412253B0, launch slot 132 0x14122E840 -> 0x14122B940 (the clip's JumpStart
// notify through the anim interface vt+0x2A0, 0x140560120), start-jump-attack slot 159 0x14121DAE0,
// face-the-goal slot 165 0x14122A310, the goal slot 153 0x1412267E0 (the target's root, + m_TargetOffset
// 0). Numbers: the BP CDO over the ctor 0x14121BFA0 (sheet act.*.Attack3_Jump.*); clip names from the
// ctor: JumpNormalStart / JumpNormalRiseLoop / JumpNormalDownLoop / Land, attack Up M_1312, Down M_1311.

const JUMP_START: &str = "JumpNormalStart";
const JUMP_RISE: &str = "JumpNormalRiseLoop";
const JUMP_DOWN: &str = "JumpNormalDownLoop";
const JUMP_LAND: &str = "Land";
const JUMP_ATTACK_UP: &str = "M_1312";
const JUMP_ATTACK_DOWN: &str = "M_1311";

/// The definition's target (FUN_14122A090: the assigned target, else the AI's target): its root
/// (`aim` false, vt+0x4C8(.., 0, 0)) or its aim point (`aim` true, (.., 1, 0): the bounds centre).
fn jump_target(p: &NpcPawn, t: TargetRef, aim: bool, cx: &StepCtx, host: &PartyHost) -> Option<Vec3> {
    match t {
        TargetRef::Enemy(id) => host.targets.info(id).map(|i| if aim { i.center } else { i.position }),
        TargetRef::None => None,
        _ => p.target_point(t, cx, host),
    }
}

/// `FUN_1405431C0` via the movement's FUN_140578330 (the jump velocity to `target` from `start`):
/// apex = max(dz (if the target is above) + h0, h_min); vz = sqrt(-2 g s_up apex); the fall time with
/// the jump-duration phase (t_dur at s_dur, then 1); horizontal = distance / (rise + fall time).
/// `g` is GetGravityZ (negative); s_up / t_dur / s_dur are m_JumpUpGravityScale / m_MaxJumpDurationTime /
/// m_JumpDurationGravityScale of the movement component (+0xBE8 / +0xBEC / +0xBF0).
pub fn jump_velocity(start: Vec3, target: Vec3, h0: f32, h_min: f32, g: f32, s_up: f32, s_dur: f32, t_dur: f32) -> Vec3 {
    let dz = target.z - start.z;
    let mut apex = if dz > 0.0 { dz + h0 } else { h0 };
    if h_min > 0.0 && apex < h_min {
        apex = h_min;
    }
    let vz = (apex * -2.0 * g * s_up).sqrt();
    let fall = if dz <= 0.0 { dz.abs() + h0 } else { h0 };
    let rest = fall - t_dur * t_dur * g * s_dur * -0.5;
    let t_fall = if rest <= 0.0 { ((fall * -2.0) / (g * s_dur)).sqrt() } else { ((rest * -2.0) / g).sqrt() + t_dur };
    let t = (-1.0 / (g * s_up)) * vz + t_fall;
    let d = Vec3::new(target.x - start.x, target.y - start.y, 0.0);
    let n = d.normalize_or_zero();
    let s = d.length() / t;
    Vec3::new(n.x * s, n.y * s, vz)
}

/// Begin 0x141220750 (after the base Begin: the start turn): with a target, the move speed +0x25C =
/// m_JumpSpeed, the attack clip = Up, the flags cleared (+0x259 set), StopMovement, the goal = the
/// target's root. vt+0x16D8(1) (movement vt+0xA40) is not modelled [?].
fn jump_begin(p: &mut NpcPawn, run: &mut ActionRun, cx: &StepCtx, host: &PartyHost) {
    let a = asset(run.def.unwrap());
    run.sub = 0;
    run.jump = JumpRun::default();
    if let Some(goal) = jump_target(p, run.target, false, cx, host) {
        run.jump.speed = tables::aparam(p.chara, a, "JumpSpeed");
        run.jump.b259 = true;
        run.jump.goal = goal;
        p.cmc.stop_movement();
    }
}

/// The launch, slot 132 0x14122B940 (the JumpStart notify of JumpNormalStart).
pub fn on_jump_start(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let Some(mut run) = p.action else { return };
    if !matches!(run.def, Some(ActionDef::DonaldJump | ActionDef::GoofyJump)) || p.state != crate::states::NpcStateId::Action {
        return;
    }
    let a = asset(run.def.unwrap());
    let ap = |k: &str| tables::aparam(p.chara, a, k);
    let (jump_height, jump_speed, limit_height, limit_min) = (ap("JumpHeight"), ap("JumpSpeed"), ap("LimitHeight"), ap("LimitMinHeight"));
    let (min_move, max_move, gravity_scale) = (ap("MinMoveDistance"), ap("MaxMoveDistance"), ap("GravityScale"));
    let corr_up = ap("EnableSpeedCorrectionUp") != 0.0;
    let corr_down = ap("EnableSpeedCorrectionDown") != 0.0;
    let Some(goal) = jump_target(p, run.target, false, cx, host) else { return };
    let aim = jump_target(p, run.target, true, cx, host).unwrap_or(goal);
    let me = p.position();
    let fwd = forward(p.yaw);
    let j = &mut run.jump;
    if jump_height + me.z < goal.z {
        // the target is above the jump height: Flying, gravity off, own vertical speed to the aim height
        let g = p.cmc.gravity_z(&p.params);
        j.g = g;
        let h = (aim.z - me.z).min(limit_height);
        j.vz = (h * g * -2.0).sqrt();
        let t = j.vz / g.abs();
        let d = horizontal(aim - me);
        let n = d.normalize_or_zero();
        let apex = n * jump_speed * t;
        if d.length() < apex.length() && corr_up {
            // speed = sqrt(((dy/t)/ny)^2 + ((dx/t)/nx)^2): = sqrt(2) |d| / t as the game writes it
            // [inf: a zero component divides 0 by 0 in the game; taken as |d| / t here].
            let c = |dc: f32, nc: f32| if nc.abs() > 1e-6 { (dc / t) / nc } else { d.length() / t };
            let (sx, sy) = (c(d.x, n.x), c(d.y, n.y));
            j.speed = (sx * sx + sy * sy).sqrt();
        }
        p.cmc.gravity_off = true;
        p.cmc.set_flying();
        j.own_vertical = true;
    } else {
        let dist = horizontal(goal - me).length();
        let mut target = goal;
        if limit_min <= me.z - goal.z {
            // far below: the aim, pulled in to m_MaxMoveDistance / pushed out to m_MinMoveDistance along
            // the forward; m_GravityScale applied (pawn vt+0x1650).
            target = aim;
            if max_move <= dist {
                target -= fwd * (dist - max_move);
            }
            if dist <= min_move {
                target += fwd * (min_move - dist);
            }
            p.cmc.gravity_scale = gravity_scale;
            j.down_far = true;
        }
        let g = p.cmc.gravity_z(&p.params);
        let v = jump_velocity(me, target, 0.0, jump_height, g, p.params.gravity_jump_up_scale, p.params.gravity_jump_duration_scale, p.params.gravity_jump_duration_time);
        // vt+0x1A28 JumpZVelocity, vt+0x1570 MaxWalkSpeed = |v.xy|
        p.cmc.jump_z_velocity = v.z;
        let vxy = Vec3::new(v.x, v.y, 0.0).length();
        p.cmc.max_walk_speed = vxy;
        let use_vxy = if j.down_far {
            true
        } else {
            let n = horizontal(target - me).normalize_or_zero();
            let apex = n * jump_speed * (v.z / g.abs());
            horizontal(target - me).length() < apex.length() && corr_down
        };
        if use_vxy {
            j.speed = vxy;
        }
        // movement vt+0x618: DoJump (Velocity.Z = JumpZVelocity, Falling)
        p.cmc.do_jump();
    }
    j.launch_dir = fwd;
    j.launched = true;
    j.facing = true;
    p.action = Some(run);
}

/// The start-jump-attack test, slot 159 0x14121DAE0: not yet attacked, a target, the goal not passed,
/// within m_JumpAttackRange (FUN_141228B50: XY distance of the roots minus both bounding cylinders,
/// max(radius, half height) each), and m_JumpAttackEnableZ above the floor (trace 10000 down) or no
/// floor. The clip: Up (M_1312) while rising (sub-step 2) unless the target is more than
/// m_JumpAttackDownRangeZ below; above m_JumpAttackUpRangeZ always Up; else Down (M_1311). The move
/// speed becomes the movement's horizontal speed (vt+0x938 = 0x140583CA0, |+0x9E8|).
fn jump_attack_start(p: &mut NpcPawn, run: &mut ActionRun, cx: &StepCtx, host: &PartyHost) -> bool {
    if run.jump.attacked || jump_passed(p, run) {
        return false;
    }
    let a = asset(run.def.unwrap());
    let ap = |k: &str| tables::aparam(p.chara, a, k);
    let Some(goal) = jump_target(p, run.target, false, cx, host) else { return false };
    let (tr, th) = match run.target {
        TargetRef::Enemy(id) => host.targets.info(id).map(|i| (i.radius, i.half_height)).unwrap_or((0.0, 0.0)),
        _ => (p.params.capsule_radius, p.params.capsule_half_height),
    };
    let range = ap("JumpAttackRange");
    // FUN_141228B50(range, 1, 0): the XY distance less both actors' bounds radius, max(extent.x, extent.y) of
    // GetActorBounds (FUN_143E98810(actor, 1)): the capsule radius, not the half height (B4 re-read).
    let _ = th;
    let dist = horizontal(goal - p.position()).length() - (tr + p.params.capsule_radius);
    if dist > range {
        return false;
    }
    run.jump.facing = false;
    let start = p.position();
    let floor = host.world.line_trace(start, start - Vec3::Z * 10000.0);
    if let Some(h) = floor {
        if ap("JumpAttackEnableZ") > start.z - h.position.z {
            return false;
        }
    }
    let aim = jump_target(p, run.target, true, cx, host).unwrap_or(goal);
    let dz = aim.z - start.z;
    let clip = if dz <= ap("JumpAttackUpRangeZ") {
        if (dz < 0.0 && dz.abs() > ap("JumpAttackDownRangeZ")) || run.sub != 2 {
            JUMP_ATTACK_DOWN
        } else {
            JUMP_ATTACK_UP
        }
    } else {
        JUMP_ATTACK_UP
    };
    p.play(clip, -1.0);
    run.jump.enable_change = false;
    run.jump.speed = horizontal(p.cmc.velocity).length();
    run.jump.attacked = true;
    true
}

/// FUN_141229A60 with m_JumpHomingYawMaxEnable off: the goal is passed when the 3D direction to it
/// points behind the actor's forward or behind the launch direction.
fn jump_passed(p: &NpcPawn, run: &ActionRun) -> bool {
    let d = (run.jump.goal - p.position()).normalize_or_zero();
    forward(p.yaw).dot(d) <= 0.0 || run.jump.launch_dir.dot(d) <= 0.0
}

/// End, slot 123 0x1412253B0: gravity scale back (vt+0x1658), StopMovement, MaxWalkSpeed back
/// (vt+0x1578), gravity on, Walking on the ground else Falling.
pub fn jump_end(p: &mut NpcPawn) {
    p.cmc.gravity_scale = p.params.gravity_scale;
    p.cmc.stop_movement();
    p.cmc.direct_xy = None;
    p.cmc.direct_z = None;
    p.cmc.max_walk_speed = p.params.speed_run;
    p.cmc.gravity_off = false;
    if !p.on_ground() {
        p.cmc.set_falling();
    }
}

/// Is the running action a jump (for the End on an interrupt).
pub fn is_jump(p: &NpcPawn) -> bool {
    p.action.is_some_and(|r| matches!(r.def, Some(ActionDef::DonaldJump | ActionDef::GoofyJump)))
}

/// The Shot / ThrowWeapon notifies: the spell (or the shield) leaves.
pub fn on_shot(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let Some(mut run) = p.action else { return };
    if run.shot {
        return;
    }
    let Some(def) = run.def else { return };
    // (party X2) a Sora spell: Sora's actors from the leader's caster
    if run.sora_cmd != 0 {
        crate::leader_magic::on_shot(p, run, cx, host);
        return;
    }
    run.shot = true;
    let origin = p.capsule_centre() + forward(p.yaw) * p.params.capsule_radius;
    let target_pos = p.target_point(run.target, cx, host);
    let enemy = match run.target {
        TargetRef::Enemy(t) => t,
        _ => TargetId::NONE,
    };
    let kind = match def {
        ActionDef::DonaldFire => Some(ProjectileKind::Fire),
        ActionDef::DonaldBlizzard => Some(ProjectileKind::Blizzard),
        ActionDef::DonaldThunder => Some(ProjectileKind::Thunder),
        ActionDef::DonaldAero => Some(ProjectileKind::Aero),
        ActionDef::GoofyDiscus => Some(ProjectileKind::Discus),
        ActionDef::DonaldCure => {
            cure(p, run.target, cx);
            // (party P3) the Cure actor's MyFireEffects (n_ex001_ProjectileMagCure_C "fire": vn_ex001_cur_fir00) at the
            // cast point; it has no body template [A party_host.json]
            let at = kh_core::host::Attach::World(origin);
            p.seam.spawn(host.effects, &kh_core::host::EffectSpawn { id: kh_core::host::EffectId(kh_core::magic::projectile::FIRE_EFFECTS_ID), name: "n_ex001_ProjectileMagCure_C", at, rotation: Vec3::new(0.0, p.yaw, 0.0), scale: 1.0, persistent: false });
            None
        }
        _ => None,
    };
    if kind == Some(ProjectileKind::Discus) {
        // ThrowWeapon of the definition's anim interface, 0x1411CD930: +0x210 = m_fAttackTime, +0x20C = 1
        run.timer = tables::aparam(p.chara, "Attack8_Discus", "fAttackTime");
        let a = "Attack8_Discus";
        // from R_wep (the shield's socket: A's ATK_00 track, else in front of the body [inf])
        let f = forward(p.yaw);
        let right = Vec3::new(-f.y, f.x, 0.0);
        let start = socket_at(p).map(|v| p.position() + f * v.x + right * v.y + Vec3::Z * v.z).unwrap_or(origin);
        let mut dir = f;
        let mut homing = true;
        // m_bEnableTargetShot with a target: the aim point in the actor's frame as (pitch, yaw); homing off
        // beyond m_rHomingDisableYaw / m_rHomingDisablePitch; yaw clamped to +-m_ShotYawMinMax, pitch to
        // m_ShotPitchMin..m_ShotPitchMax; the direction = the actor's rotation + that (pitch, yaw).
        if tables::aparam(p.chara, a, "bEnableTargetShot") != 0.0 {
            if let Some(t) = target_pos {
                let d = t - start;
                let yaw = crate::pawn::wrap180(yaw_of(horizontal(d)) - p.yaw);
                let pitch = d.z.atan2(horizontal(d).length()).to_degrees();
                if yaw.abs() >= tables::aparam(p.chara, a, "rHomingDisableYaw") || pitch >= tables::aparam(p.chara, a, "rHomingDisablePitch") {
                    homing = false;
                }
                let ym = tables::aparam(p.chara, a, "ShotYawMinMax");
                let yaw = yaw.clamp(-ym, ym);
                let pitch = pitch.max(tables::aparam(p.chara, a, "ShotPitchMin")).min(tables::aparam(p.chara, a, "ShotPitchMax"));
                let (sp, cp) = pitch.to_radians().sin_cos();
                let (sy, cy) = (p.yaw + yaw).to_radians().sin_cos();
                dir = Vec3::new(cp * cy, cp * sy, sp);
            }
        }
        let mut pr = Projectile::discus(p.id, start, dir.normalize_or_zero(), homing, enemy, target_pos);
        // (party P3) THE FREE THROW (our design; KH3's AI Goofy always throws at a target): a pad-driven Goofy with no
        // target (or a target already gone) throws straight along his facing (turned to the held stick at Begin),
        // level, at the throw's speed (m_fStartSpeed under the shield's movement law); after half of m_fAttackTime
        // (or its first hit, or a wall) it flies back to him at its full speed and he catches it when it arrives (the
        // definition's catch clip, `discus_tick`). Without this the shield flew on for the whole 2 s (30 m) and was
        // taken back by the catch from wherever it was.
        if p.pad.is_some() && (enemy == TargetId::NONE || target_pos.is_none()) {
            let f = forward(p.yaw);
            pr.velocity = f * pr.speed;
            pr.homing_on = false;
            pr.target = TargetId::NONE;
            pr.target_point = None;
            pr.free = Some(tables::aparam(p.chara, a, "fAttackTime") * 0.5);
            run.free_throw = true;
            run.timer = tables::aparam(p.chara, a, "fAttackTime") * FREE_THROW_TIME;
        }
        p.events.push(PartyEventKind::ProjectileSpawned { kind: pr.kind.name(), at: pr.position });
        p.requests.push(Request::Projectile(pr));
        p.action = Some(run);
        return;
    }
    if let Some(kind) = kind {
        // m_bIsDirectPos (Thunder, Aero): put at the target's feet
        let ground = match run.target {
            TargetRef::Enemy(t) => host.targets.info(t).map(|i| i.position),
            TargetRef::Point(q) => Some(q),
            _ => None,
        };
        let mut pr = Projectile::spawn_at(kind, p.id, p.chara, origin, p.yaw, enemy, target_pos, ground);
        spell_actor_effects(p, &mut pr, origin, host);
        p.events.push(PartyEventKind::ProjectileSpawned { kind: kind.name(), at: pr.position });
        p.requests.push(Request::Projectile(pr));
    }
    p.action = Some(run);
}

/// (party P3) THE SPELL ACTOR'S EFFECTS. Donald's spells had no visible actor: the projectile's body-effect handle
/// (`Projectile::effect`, E.6) was never given, so the host followed nothing. As Sora's actors do (kh-core
/// magic/projectile.rs `spawn_fired_from`), two seam events with the actor's class: the class with the actor
/// (persistent: its body ParticleComp, following it through `party_fx::publish_projectiles`) and the class with
/// `FIRE_EFFECTS_ID` at the cast point, turned along the flight (one-shot: its MyFireEffects). Thunder / Aero are put at
/// the target (m_bIsDirectPos): their fire effect stays at Donald's cast point. The body stops when the actor ends
/// (`Party::step_projectiles`).
fn spell_actor_effects(p: &mut NpcPawn, pr: &mut Projectile, origin: Vec3, host: &mut PartyHost) {
    use kh_core::host::{Attach, EffectId, EffectSpawn};
    let Some(class) = pr.kind.class() else { return };
    let h = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(class)), name: class, at: Attach::World(pr.position), rotation: Vec3::ZERO, scale: 1.0, persistent: true });
    pr.effect = h.0;
    let d = pr.velocity.normalize_or_zero();
    let d = if d == Vec3::ZERO { forward(p.yaw) } else { d };
    let rotation = Vec3::new(d.z.clamp(-1.0, 1.0).asin().to_degrees(), d.y.atan2(d.x).to_degrees(), 0.0);
    p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::magic::projectile::FIRE_EFFECTS_ID), name: class, at: Attach::World(origin), rotation, scale: 1.0, persistent: false });
}

/// (party P3) THE CAST EFFECT: the spell's unit of `n_ex001_MagicSet` names its effect group (m_EffectGroup, sheet
/// `magicset.<spell>.effect_group`: Fire 1300, Blizzard 1310, Thunder 1320, Aero 1330, Cure 1340), played by the cast
/// (0x14122D300 -> FUN_1405B8C30 with the unit's +8) from `n_ex001_Magic_ead` ("詠唱" rows [A]: 1300 vn_ex001_fir_cal00,
/// 1310 vn_ex001_blz_cal00, 1320 vn_ex001_thd_cal00, 1330 vn_ex001_air_cal00, 1340 vn_ex001_cur_cal00; socket vfx_call
/// of the staff, ToAttach, WhenChangeAnimationType END / LOOPEND: it ends with the cast clip). The cast clips 1321 / 1331
/// / 1341 / 1351 carry no AttachEffect notify of their own, so without this no cast effect showed.
fn cast_effect(p: &mut NpcPawn, def: ActionDef, host: &mut PartyHost) {
    use kh_core::host::{Attach, EffectId, EffectSpawn, PartyMember};
    let (key, name) = match def {
        ActionDef::DonaldFire => ("fire", "vn_ex001_fir_cal00"),
        ActionDef::DonaldBlizzard => ("blizzard", "vn_ex001_blz_cal00"),
        ActionDef::DonaldThunder => ("thunder", "vn_ex001_thd_cal00"),
        ActionDef::DonaldAero => ("aero", "vn_ex001_air_cal00"),
        ActionDef::DonaldCure => ("cure", "vn_ex001_cur_cal00"),
        _ => return,
    };
    let group = tables::param(&format!("magicset.{key}.effect_group")) as u32;
    let at = Attach::party(PartyMember(p.id.0), Some("vfx_call"));
    p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(group), name, at, rotation: Vec3::ZERO, scale: 1.0, persistent: false });
}

/// Donald's Cure: `n_ex001_ProjectileMagCure` (m_AppendCure 120, m_BonusAppendCure 60, m_CureEffectType
/// D_CURE) run by `ATresProj_MagicCure` 0x1414742A0 (m_DetectRange 300 from its ctor 0x14146FD60):
/// - the target gets m_AppendCure + max(1, MagicPower(caster)) (FUN_1405836E0 on the projectile's
///   base param), halved rounding up when it is a KO'd NPC (state 59), heal effect 14 (D_CURE);
/// - every other pawn of the target's team (a friend or the player; the caster too) whose root is
///   within m_DetectRange of the target's root gets m_BonusAppendCure + max(1, MagicPower), halved
///   when KO'd, effect 15.
pub fn cure(p: &mut NpcPawn, target: TargetRef, cx: &StepCtx) {
    let magic = p.status.magic().max(1);
    let main = tables::param("proj.cure.AppendCure") as i32 + magic;
    let bonus = tables::param("proj.cure.BonusAppendCure") as i32 + magic;
    let range = tables::param("proj.cure.DetectRange");
    let target = if target == TargetRef::None { TargetRef::Member(p.id) } else { target };
    let at = match target {
        TargetRef::Sora => Some(cx.sora.position),
        // (party AIX) AI Sora (a friend leading) is an ally too
        TargetRef::Member(m) => cx.members.iter().find(|x| x.0 == m).map(|x| x.1).or_else(|| cx.sora.ai_sora.filter(|a| a.0 == m).map(|a| a.1)),
        _ => None,
    };
    give_heal(p, target, main, CURE_EFFECT_MAIN);
    let Some(at) = at else { return };
    if target != TargetRef::Sora && cx.sora.position.distance(at) <= range {
        give_heal(p, TargetRef::Sora, bonus, CURE_EFFECT_BONUS);
    }
    // (party AIX) the Cure's area reaches AI Sora as it reaches the members (D_CURE kind 15 on the others in range)
    if let Some((s, pos)) = cx.sora.ai_sora {
        if TargetRef::Member(s) != target && pos.distance(at) <= range {
            give_heal(p, TargetRef::Member(s), bonus, CURE_EFFECT_BONUS);
        }
    }
    for &(m, pos, _) in cx.members {
        if TargetRef::Member(m) != target && pos.distance(at) <= range {
            give_heal(p, TargetRef::Member(m), bonus, CURE_EFFECT_BONUS);
        }
    }
}

/// The heal effect numbers of D_CURE (0x1414742A0): 14 on the target, 15 on the others in range.
pub const CURE_EFFECT_MAIN: u8 = 14;
pub const CURE_EFFECT_BONUS: u8 = 15;

/// (party D5) The effect a heal of `kind` plays on the healed pawn: Recover 0x1405B57D0 hands the kind to
/// 0x1405B8C30(+0x440, 4, kind), whose switch maps 0xE / 0xF (D_CURE's 14 on the target, 15 on the others in range)
/// to group 0x58F = 1423 of the pawn's m_CmnBadStatesEffect (ead_com_abn0 EffectDataList[20] "ケアル周辺回復（ツタ）
/// （通常）": vm_cur_bds1 at vfx_abn_center; vp_wcu_bds1 under water, not used: no water state) [C, A]. Its sound is
/// the effect's SeAttach (NPC/n_ex001/vm_cur_bds1: se00301_040), through the effect seam. (The same switch also plays
/// the common recovery group 0xC12 once per kind change and other kinds' groups: not part of this.) (name, socket, group).
pub const fn recover_effect(kind: u8) -> Option<(&'static str, &'static str, u32)> {
    match kind {
        14 | 15 => Some(("vm_cur_bds1", "vfx_abn_center", 0x58F)),
        // (GOOFY-IT) the item kinds of the same switch (ItemEffect's Recover kind: Potion 3 ... Panacea 0x15), kh-core's
        // traced table, which Sora's own item effect uses too: one table for every pawn
        _ => kh_core::host::audio::recover_kind_effect(kind),
    }
}

/// (GOOFY-IT) THE ITEM'S PRESENTATION ON ITS TARGET, for every character (no per-character rule): ItemEffect 0x1405BE820
/// hands Recover its kind only when the item gives something (the AMOUNT it computes, not the gain: a Potion at full HP
/// still shows) or cures (Elixir / Megalixir / Panacea); an MP-charge Ether / Hi-Ether shows its kind too. A target with
/// no focus gauge (the friends: max focus 0) gets nothing from a Refocuser, so no effect. The Recover kind, or None.
pub fn item_effect_kind(item: ItemKind, target_has_focus: bool) -> Option<u8> {
    if matches!(item, ItemKind::Refocuser | ItemKind::HiRefocuser) && !target_has_focus {
        return None;
    }
    kh_core::host::audio::item_recover_kind(item as u32)
}

/// (GOOFY-IT) The effect spawn of a Recover `kind` on party member `member` (any member, any character: the effect, its
/// socket and its group come from the kind alone). None: the kind has no effect.
pub fn recover_spawn(member: kh_core::host::PartyMember, kind: u8) -> Option<kh_core::host::EffectSpawn> {
    let (name, socket, group) = recover_effect(kind)?;
    Some(kh_core::host::EffectSpawn { id: kh_core::host::EffectId(group), name, at: kh_core::host::Attach::party(member, Some(socket)), rotation: Vec3::ZERO, scale: 1.0, persistent: false })
}

fn give_heal(p: &mut NpcPawn, target: TargetRef, amount: i32, effect: u8) {
    // (party D5) the healed member's own heal effect (Sora's: the host, from `SoraHeal::effect`)
    if let (TargetRef::Member(m), Some(_)) = (target, recover_effect(effect)) {
        p.requests.push(Request::HealEffect { member: m, kind: effect });
    }
    if target == TargetRef::Member(p.id) {
        p.status.heal(amount);
    } else {
        p.requests.push(Request::Heal { target, amount, effect });
    }
}

/// An item's effect (kh-core's amounts: magic spec E I5).
pub fn item_effect(p: &mut NpcPawn, item: ItemKind, target: TargetRef) {
    use kh_core::status::runtime::ITEM_UP_PER_COPY;
    // (B4) the item path FUN_1405BCBE0 -> FUN_1405BD190 -> ItemEffect 0x1405BE820 on the target: the USER's
    // ITEM_UP count (vt+0x1500(0x28)) raises the amount; the target's Die state halves it (applied by the target).
    let boost = 1.0 + ITEM_UP_PER_COPY * p.status.count(kh_core::enums::ETresAbilityKind::ItemUp) as f32;
    let target = if target == TargetRef::None { TargetRef::Member(p.id) } else { target };
    // (GOOFY-IT) the item's effect (and its sound, the effect's SeAttach) on its target, as Recover plays it for every
    // pawn: before, only Sora's own items showed one (kh-core magic/items.rs); a friend's item healed in silence
    match target {
        TargetRef::Member(m) => {
            if let Some(kind) = item_effect_kind(item, false) {
                p.requests.push(Request::HealEffect { member: m, kind });
            }
        }
        TargetRef::Sora => {
            if let Some(kind) = item_effect_kind(item, true) {
                p.requests.push(Request::SoraEffect { kind });
            }
        }
        _ => {}
    }
    match target {
        TargetRef::Member(id) if id == p.id => {
            let mult = if p.status.ko() { 0.5 * boost } else { boost };
            p.status.item_effect(item, mult);
        }
        TargetRef::Member(_) => p.requests.push(Request::Item { target, item, boost }),
        _ => {
            // Sora: his own Status applies ItemEffect; the amounts with this user's boost go as a heal.
            let (hp, mp) = sora_item_amounts(item, boost);
            if hp > 0 {
                p.requests.push(Request::Heal { target, amount: hp, effect: 1 });
            }
            if mp > 0 {
                p.requests.push(Request::Mp { target, amount: mp });
            }
        }
    }
}

/// An item's HP / MP on Sora with the user's ITEM_UP boost (his own Status applies ItemEffect; kh-core's amounts).
/// (party AIX) shared by the player branch above and by AI Sora (`SoraView::ai_sora`, Party::step).
pub(crate) fn sora_item_amounts(item: ItemKind, boost: f32) -> (i32, i32) {
    use kh_core::status::math::round_half_up;
    use kh_core::status::runtime::{ETHER_MP, HI_POTION_HP, POTION_HP};
    match item {
        ItemKind::Potion => (round_half_up(boost * POTION_HP), 0),
        ItemKind::HiPotion | ItemKind::MegaPotion => (round_half_up(boost * HI_POTION_HP), 0),
        ItemKind::Ether => (0, round_half_up(boost * ETHER_MP)),
        ItemKind::HiEther | ItemKind::MegaEther => (0, 999),
        ItemKind::Elixir | ItemKind::Megalixir => (9999, 9999),
        _ => (0, 0),
    }
}

/// MainProc of the action. True = the action has ended.
pub fn tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) -> bool {
    let Some(mut run) = p.action else { return true };
    let Some(def) = run.def else { return true };
    let dt = cx.dt;
    let target_pos = p.target_point(run.target, cx, host);
    // TresAnimNotifyState_TurnToTarget: turn while the window is open (AttackBase Tick 0x1404404B0
    // with m_bEnableTurnToTarget; the rate is pawn vt+0x2408 = 0x140544270: 360 deg/s for the NPC pawns).
    if let Some(t) = target_pos {
        // (party P3) a pad-driven Goofy's Turbo slide is steered by the stick alone (`pad_turbo_steer`): M_1325's
        // TurnToTarget window (its whole length) does not pull him to the target against the player
        let player_steers = p.pad.is_some() && def == ActionDef::GoofyTurbo && run.sub == 1;
        if p.anim.window_open("TurnToTarget") && !player_steers {
            let rate = turn_rate(TurnKind::Attack);
            p.turn_toward(t, dt, rate);
        }
        // TresAnimNotifyState_Traction: close in with m_AnimNotifyTractionParam (max speed /
        // acceleration; stop inside TractionDisMax) [D: the traction handler is not read].
        if p.anim.window_open("Traction") {
            let a = asset(def);
            let max = tables::aparam(p.chara, a, "AnimNotifyTractionParam.TractionMaxSpeed");
            let acc = tables::aparam(p.chara, a, "AnimNotifyTractionParam.TractionAcc");
            let stop = tables::aparam(p.chara, a, "AnimNotifyTractionParam.TractionDisMax");
            let d = horizontal(t - p.position());
            if d.length() > stop {
                let v = horizontal(p.cmc.velocity).length();
                let s = (v + acc * dt).min(max);
                p.cmc.direct_xy = Some(glam::Vec2::new(d.normalize().x * s, d.normalize().y * s));
            } else {
                p.cmc.direct_xy = Some(glam::Vec2::ZERO);
            }
        }
    }
    let done = match def {
        ActionDef::DonaldBeat | ActionDef::DonaldSwing | ActionDef::GoofyPush | ActionDef::GoofySweep => p.anim.finished,
        ActionDef::DonaldFire | ActionDef::DonaldBlizzard | ActionDef::DonaldThunder | ActionDef::DonaldAero | ActionDef::DonaldCure => {
            if p.anim.finished && !run.shot {
                on_shot(p, cx, host);
                run = p.action.unwrap_or(run);
            }
            p.anim.finished
        }
        ActionDef::DonaldJump | ActionDef::GoofyJump => {
            let r = jump_tick(p, &mut run, cx, host);
            if r {
                jump_end(p);
            }
            r
        }
        ActionDef::GoofyTornado => tornado_tick(p, &mut run, cx, host),
        ActionDef::GoofyTurbo => turbo_tick(p, &mut run, cx, host),
        ActionDef::GoofyDiscus => discus_tick(p, &mut run, dt),
    };
    if p.action.is_some() {
        p.action = Some(run);
    }
    done
}

/// `UTresAttack8_n_ex002_Discus` Tick 0x1411C7670 (`run.timer` = +0x210 the attack timer, set to
/// m_fAttackTime when the shield leaves; `run.timer2` = +0x218 the fade timer, m_fAttackFadeTime from
/// the first hit (0 on a wall hit); `run.shot` = +0x20C thrown; `run.caught` = +0x20E):
/// 0 -> (thrown: 2); 1: when it has hit and the fade has run out -> the catch; 2: the throw clip ends ->
/// the flight loop M_1328 (air M_1331), 3; 3: both timers count down (clamped); the attack time out,
/// or caught with the fade out -> the catch clip M_1329 (air M_1332), 4; 4 / 5: its end -> 6 (done).
/// (party P3) A free throw's safety limit, in m_fAttackTime: the catch comes when the shield is back; if it never
/// comes back (blocked), the definition's own catch after twice the attack time.
pub const FREE_THROW_TIME: f32 = 2.0;

/// (party P3) A pad-driven Goofy's targeted throw whose target is gone in flight becomes a free throw (it returns).
pub fn discus_make_free(p: &mut NpcPawn) {
    if let Some(mut run) = p.action.filter(|r| r.def == Some(ActionDef::GoofyDiscus) && !r.free_throw) {
        run.free_throw = true;
        run.timer = tables::aparam(p.chara, "Attack8_Discus", "fAttackTime") * FREE_THROW_TIME;
        p.action = Some(run);
    }
}

fn discus_tick(p: &mut NpcPawn, run: &mut ActionRun, dt: f32) -> bool {
    let a = "Attack8_Discus";
    let fade_max = tables::aparam(p.chara, a, "fAttackFadeTime");
    // (party P3) a pad-driven Goofy's air throw: the air flight loop becomes the ground one when he lands (same time)
    if p.pad.is_some() && run.sub == 3 && p.anim.clip.name == "M_1331" && p.on_ground() {
        let t = p.anim.time;
        p.play_looped("M_1328", 0.1);
        p.anim.time = t;
    }
    let catch = |p: &mut NpcPawn, run: &mut ActionRun| {
        p.play(if p.on_ground() { "M_1329" } else { "M_1332" }, -1.0);
        run.sub = 4;
    };
    // (+0x20E / +0x218 are set by the hit handler 0x1411CC550: `Party::discus_hit`.)
    match run.sub {
        0 | 1 | 2 => {
            if run.sub == 0 && run.shot {
                run.sub = 2;
            }
            if run.caught {
                run.timer2 = (run.timer2 - dt).clamp(0.0, fade_max);
                if run.timer2 <= 0.0 {
                    catch(p, run);
                    return false;
                }
            }
            if p.anim.finished {
                p.play_looped(if p.on_ground() { "M_1328" } else { "M_1331" }, -1.0); // (party P3) PlayAnim mode 3: it loops
                run.sub = 3;
            }
            false
        }
        3 => {
            if run.caught {
                run.timer2 = (run.timer2 - dt).clamp(0.0, fade_max);
            }
            let max = tables::aparam(p.chara, a, "fAttackTime") * if run.free_throw { FREE_THROW_TIME } else { 1.0 }; // (party P3)
            run.timer = (run.timer - dt).clamp(0.0, max);
            if run.timer > 0.0 && (!run.caught || run.timer2 > 0.0) {
                return false;
            }
            catch(p, run);
            false
        }
        4 | 5 => {
            run.sub = 5;
            p.anim.finished
        }
        _ => true,
    }
}

/// Tick, slot 122 0x141222A00. The flight part (while launched): the goal follows the target's root;
/// the jump attack may start (-> sub-step 6); the decel (+0x25A); in the air: goal passed -> stop
/// facing; facing -> snap the yaw to the goal (slot 165 0x14122A310, homing flags off:
/// SetActorRotation); velocity XY = forward x speed (vt+0x16C0), and with the own vertical speed
/// +0x260 += dt x +0x264, SetVelocityZ (vt+0x16D0) [inf: the decompiler lost those call arguments];
/// on the ground: StopMovement, not launched. Sub-steps: 0 JumpNormalStart -> 1; 1 its end: on the
/// ground -> 4, else JumpNormalRiseLoop -> 2; 2 Velocity.Z <= 0: (own vertical: gravity on, Falling)
/// JumpNormalDownLoop -> 4; 4 on the ground: Land, StopMovement -> 5; 5 its end -> done; 6 (attack)
/// Velocity.Z <= 0 -> (gravity on, Falling) 7; 7 the attack clip ends (or on the ground with
/// m_bEnableChange): in the air rising -> 2, falling -> DownLoop, 4; on the ground Land -> 5.
fn jump_tick(p: &mut NpcPawn, run: &mut ActionRun, cx: &StepCtx, host: &mut PartyHost) -> bool {
    let dt = cx.dt;
    let a = asset(run.def.unwrap());
    if let Some(g) = jump_target(p, run.target, false, cx, host) {
        run.jump.goal = g;
    }
    if run.jump.launched {
        if jump_attack_start(p, run, cx, host) {
            run.sub = 6;
        }
        if run.jump.decel {
            run.jump.speed = (run.jump.speed - dt * tables::aparam(p.chara, a, "JumpAttackDecelMoveSpeed")).max(tables::aparam(p.chara, a, "JumpAttackMinMoveSpeed"));
        }
        if !p.on_ground() {
            if jump_passed(p, run) {
                // (+0x258 set: anim interface vt+0x2F0 [?: not modelled])
                run.jump.facing = false;
                run.jump.b259 = false;
            }
            if run.jump.facing {
                let d = horizontal(run.jump.goal - p.position());
                if d.length_squared() > 1e-4 {
                    p.yaw = yaw_of(d);
                }
            }
            let f = forward(p.yaw) * run.jump.speed;
            p.cmc.set_velocity_xy(glam::Vec2::new(f.x, f.y));
            if run.jump.own_vertical {
                run.jump.vz += dt * run.jump.g;
                p.cmc.set_velocity_z(run.jump.vz);
            }
        } else {
            p.cmc.stop_movement();
            run.jump.launched = false;
        }
    }
    let fall_mode = |p: &mut NpcPawn, run: &mut ActionRun| {
        if run.jump.own_vertical {
            // vt+0x1728(1) gravity on, vt+0x17A0(3) Falling
            p.cmc.gravity_off = false;
            p.cmc.direct_z = None;
            p.cmc.set_falling();
            run.jump.own_vertical = false;
        }
    };
    let land = |p: &mut NpcPawn, run: &mut ActionRun| {
        p.play(JUMP_LAND, -1.0);
        p.cmc.stop_movement();
        p.cmc.direct_xy = None;
        run.jump.launched = false;
        run.sub = 5;
    };
    match run.sub {
        0 => {
            p.play(JUMP_START, -1.0);
            run.sub = 1;
            false
        }
        1 => {
            if !p.anim.finished {
                return false;
            }
            if p.on_ground() {
                run.sub = 4;
                return false;
            }
            p.play(JUMP_RISE, -1.0);
            run.sub = 2;
            false
        }
        2 => {
            if p.cmc.velocity.z > 0.0 {
                return false;
            }
            fall_mode(p, run);
            p.play(JUMP_DOWN, -1.0);
            run.sub = 4;
            false
        }
        4 => {
            if p.on_ground() {
                land(p, run);
            }
            false
        }
        5 => p.anim.finished,
        6 | 7 => {
            if run.sub == 6 && p.cmc.velocity.z <= 0.0 {
                fall_mode(p, run);
                run.sub = 7;
            }
            let change = p.on_ground() && run.jump.enable_change;
            if !p.anim.finished && !change {
                return false;
            }
            if !p.on_ground() {
                if p.cmc.velocity.z > 0.0 {
                    run.sub = 2;
                    return false;
                }
                p.play(JUMP_DOWN, -1.0);
                run.sub = 4;
            } else {
                land(p, run);
            }
            false
        }
        _ => true,
    }
}

/// `UTresAttack6_n_ex002_Tornado` Tick 0x1411C61D0.
fn tornado_tick(p: &mut NpcPawn, run: &mut ActionRun, cx: &StepCtx, host: &mut PartyHost) -> bool {
    let dt = cx.dt;
    let fly_height = tables::aparam(p.chara, "Attack6_Tornado", "fFlyHeight");
    let fly_speed = tables::aparam(p.chara, "Attack6_Tornado", "fFlySpeed");
    let move_speed = tables::aparam(p.chara, "Attack6_Tornado", "fMovemSpeed");
    // the hover: a floor trace 10000 cm down; below floor + FlyHeight -> flying mode, Z speed
    // FlySpeed; above it by more than FlySpeed x dt -> falling (mode 3) [C].
    if run.flying {
        let start = p.position() + Vec3::Z * 1.0;
        let floor = host.world.line_trace(start, start - Vec3::Z * 10000.0).map(|h| h.position.z);
        match floor {
            Some(fz) => {
                let want = fz + fly_height;
                let z = p.position().z;
                if z < want {
                    p.cmc.set_flying();
                    p.cmc.direct_z = Some(fly_speed);
                } else if z > want + fly_speed * dt {
                    p.cmc.set_falling();
                } else {
                    p.cmc.direct_z = Some(0.0);
                }
            }
            None => run.timer = 0.0,
        }
    }
    if run.sub == 1 && !crate::pad::goofy::tornado_steer(p, run, dt) /* (party X3G) tier 2+: the stick steers */ {
        // chase: velocity = MovemSpeed toward the target (3D direction, horizontal part applied;
        // yaw set to face it); >2 hits on one target -> retarget (exclusion, FUN_1411CF330).
        let alive = match run.target {
            TargetRef::Enemy(t) => host.targets.info(t).filter(|i| i.flags.has(TargetFlags::ALIVE)),
            _ => None,
        };
        match alive {
            Some(info) => {
                let d = info.position - p.position();
                let n = d.normalize_or_zero();
                p.yaw = yaw_of(horizontal(d));
                p.cmc.direct_xy = Some(glam::Vec2::new(n.x * move_speed, n.y * move_speed));
            }
            None => {
                if let Some(t) = retarget(p, host, run.target) {
                    run.target = TargetRef::Enemy(t);
                }
            }
        }
        if p.hits_this_action > 2 {
            if let Some(t) = retarget(p, host, run.target) {
                run.target = TargetRef::Enemy(t);
            }
            p.hits_this_action = 0;
        }
    }
    match run.sub {
        0 => {
            if p.anim.finished {
                p.play_looped("M_1322", -1.0); // (party P3) PlayAnim mode 1 (0x1411C61D0): it loops
                if p.on_ground() {
                    run.flying = true;
                }
                run.timer = tables::aparam(p.chara, "Attack6_Tornado", "fMoveTime");
                run.sub = 1;
            }
            false
        }
        1 => {
            run.timer = (run.timer - dt).max(0.0);
            if run.timer <= 0.0 {
                p.cmc.stop_movement();
                p.play("M_1323", -1.0);
                run.sub = 2;
            }
            false
        }
        2 => {
            if p.anim.finished {
                if !p.on_ground() {
                    p.cmc.set_falling();
                    run.flying = false;
                    p.play("JumpNormalDownLoop", -1.0);
                    run.sub = 3;
                } else {
                    run.flying = false;
                    return true;
                }
            }
            false
        }
        3 => {
            if p.on_ground() {
                p.play("Land", -1.0);
                run.sub = 4;
            }
            false
        }
        _ => p.anim.finished,
    }
}

/// `UTresAttack7_n_ex002_Turbo` Tick 0x1411C6CB0 with the steering 0x1411D0430.
fn turbo_tick(p: &mut NpcPawn, run: &mut ActionRun, cx: &StepCtx, host: &mut PartyHost) -> bool {
    let dt = cx.dt;
    let a = "Attack7_Turbo";
    match run.sub {
        0 => {
            if p.anim.finished {
                p.play_looped("M_1325", -1.0); // (party P3) PlayAnim mode 1 (0x1411C6CB0): it loops
                run.timer = tables::aparam(p.chara, a, "fMoveTime");
                run.speed = tables::aparam(p.chara, a, "fMoveSpeed");
                run.sub = 1;
            }
        }
        1 => {
            if p.pad.is_some() {
                pad_turbo_steer(p, dt); // (party P3) the player steers the slide
            } else {
                turbo_steer(p, run, cx, host);
            }
            run.timer = (run.timer - dt).max(0.0);
            if run.timer <= 0.0 {
                p.play("M_1326", -1.0);
                run.sub = 2;
            }
        }
        2 => {
            if p.anim.finished {
                if p.on_ground() {
                    return true;
                }
                p.cmc.set_falling();
                p.play("JumpNormalDownLoop", -1.0);
                run.sub = 3;
            }
        }
        3 => {
            if p.on_ground() {
                p.play("Land", -1.0);
                run.sub = 4;
            }
        }
        _ => return p.anim.finished,
    }
    if run.sub < 3 && p.on_ground() {
        // the ledge check: a drop of m_FallHeight (150) under the pawn ends the move (falling).
        let start = p.position() + Vec3::Z * 1.0;
        let fall = tables::aparam(p.chara, a, "FallHeight");
        let drop = host.world.line_trace(start, start - Vec3::Z * 10000.0).map(|h| start.z - 1.0 - h.position.z).unwrap_or(f32::MAX);
        if drop >= fall {
            p.cmc.set_falling();
            return true;
        }
        if run.sub == 1 {
            // velocity = forward x speed (vt+0x16B0).
            let f = forward(p.yaw);
            p.cmc.direct_xy = Some(glam::Vec2::new(f.x * run.speed, f.y * run.speed));
        }
    }
    false
}

/// (party P3) THE PAD'S TURBO STEERING (our design, the user's request: KH3's Goofy is never player-driven). While he
/// rides the shield (sub-step 1, M_1325) the left stick steers him directly: his yaw turns toward the stick's
/// camera-relative direction at the rate the game's own Turbo steering turns at (0x1411D0430 phase 0: `turn_toward`
/// with the NPC attack turn rate, 360 deg/s); the stick released, he keeps his heading (no slalom weave). Speed
/// (m_fMoveSpeed 900), duration (m_fMoveTime 6 s), the Attack_Body hits, the effects and the ledge check stay the game's
/// (`turbo_tick`). The AI's Goofy keeps `turbo_steer`.
fn pad_turbo_steer(p: &mut NpcPawn, dt: f32) {
    let Some((yaw, mag)) = p.pad.as_ref().map(|d| (d.stick_yaw, d.stick_magnitude)) else { return };
    if mag < kh_core::params::SORA_PARAMS.input_stick_active_min {
        return;
    }
    let rate = turn_rate(TurnKind::Attack);
    let diff = crate::pawn::wrap180(yaw - p.yaw);
    let step = rate * dt;
    p.yaw = crate::pawn::wrap180(p.yaw + diff.clamp(-step, step));
}

/// 0x1411D0430: phase 0 turns toward the target for m_fSlalomTime (0.7 s) and, once inside
/// m_fRangeMin, picks another target; phase 1 is one weave step: the angle +0x20C advances by
/// m_fTurnAngle (15 deg) and the yaw moves by m_fTurnRate (3 deg) toward the side the sine falls.
fn turbo_steer(p: &mut NpcPawn, run: &mut ActionRun, cx: &StepCtx, host: &mut PartyHost) {
    let a = "Attack7_Turbo";
    let dt = cx.dt;
    run.timer2 = (run.timer2 - dt).max(0.0);
    if run.phase == 0 {
        if let Some(t) = p.target_point(run.target, cx, host) {
            let dist = horizontal(t - p.position()).length();
            if dist > tables::aparam(p.chara, a, "fRangeMin") {
                let rate = turn_rate(TurnKind::Attack);
                p.turn_toward(t, dt, rate);
            } else if let Some(n) = retarget(p, host, run.target) {
                run.target = TargetRef::Enemy(n);
            }
        }
        if run.timer2 > 0.0 {
            return;
        }
        run.phase = 1;
    }
    let s0 = run.weave.to_radians().sin();
    run.weave += tables::aparam(p.chara, a, "fTurnAngle");
    if run.weave >= 360.0 {
        run.weave -= 360.0;
    }
    let s1 = run.weave.to_radians().sin();
    let rate = tables::aparam(p.chara, a, "fTurnRate");
    p.yaw = crate::pawn::wrap180(if s1 <= s0 { p.yaw - rate } else { p.yaw + rate });
    if run.timer2 > 0.0 {
        return;
    }
    run.phase = 0;
    run.timer2 = tables::aparam(p.chara, a, "fSlalomTime");
}

/// The nearest other living enemy (the exclusion search vt+0x4E8 / 0x4F0) [D: its EQS is C's].
fn retarget(p: &NpcPawn, host: &PartyHost, current: TargetRef) -> Option<TargetId> {
    let mut list = Vec::new();
    host.targets.enumerate(&mut list);
    let cur = match current {
        TargetRef::Enemy(t) => t,
        _ => TargetId::NONE,
    };
    list.iter()
        .filter(|i| i.id != cur && i.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE))
        .min_by(|a, b| a.position.distance(p.position()).total_cmp(&b.position.distance(p.position())))
        .map(|i| i.id)
}

/// The weapon shape radius (`TresAtkCollComponent.CollisionShapesSrc` of `w_do_WepBase` /
/// `w_gu_WepBase`: Donald SPHERE 40 at ATK_01, Goofy SPHERE 50 at ATK_00) [A].
pub fn weapon_radius(chara: Chara) -> f32 {
    match chara {
        Chara::Donald => 40.0,
        Chara::Goofy => 50.0,
    }
}

fn dist_point_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// One attack shape of a collision group, world space: segment `a`..`b` with `radius` (a == b: sphere),
/// or a CONVEX hull (`prism`, then `a`..`b` is its axis and `radius` its circumradius).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtkShape {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
    pub prism: Option<Prism>,
}

/// An upright regular prism: the hull of `Collision/tres_colltest01` (the StaticMesh's 20-gon: 40 distinct
/// vertices at radius 100, angles k x 18 deg, z +-50) under the shape's scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prism {
    pub centre: Vec3,
    /// Circumradius (the vertices' radius) and half height, cm; `yaw` the angle of vertex 0, degrees.
    pub radius: f32,
    pub half_height: f32,
    pub yaw: f32,
    pub sides: u32,
}

impl Prism {
    /// Distance from `p` to the prism (0 inside).
    pub fn distance(&self, p: Vec3) -> f32 {
        let d = p - self.centre;
        let dz = (d.z.abs() - self.half_height).max(0.0);
        let q = glam::Vec2::new(d.x, d.y);
        let n = self.sides.max(3);
        let step = std::f32::consts::TAU / n as f32;
        let v = |i: u32| {
            let a = self.yaw.to_radians() + step * i as f32;
            glam::Vec2::new(a.cos(), a.sin()) * self.radius
        };
        let mut outside = false;
        let mut best = f32::MAX;
        for i in 0..n {
            let (a, b) = (v(i), v(i + 1));
            let e = b - a;
            // outward normal of a counter-clockwise polygon
            let normal = glam::Vec2::new(e.y, -e.x);
            if (q - a).dot(normal) > 0.0 {
                outside = true;
            }
            let t = ((q - a).dot(e) / e.length_squared()).clamp(0.0, 1.0);
            best = best.min(q.distance(a + e * t));
        }
        let dxy = if outside { best } else { 0.0 };
        (dxy * dxy + dz * dz).sqrt()
    }

    /// Overlap with a capsule `a`..`b` of radius `r` (sampled along the segment, as `seg_seg_dist`).
    pub fn touches(&self, a: Vec3, b: Vec3, r: f32) -> bool {
        (0..=8).any(|i| self.distance(a.lerp(b, i as f32 / 8.0)) <= r)
    }
}

/// The attack-collision group a window opens (`TresAnimNotifyState_Attack.m_AttackCollisionGrpName`),
/// from the weapon blueprints' `TresAtkColl0.CollisionShapesSrc` (w_do_WepBase / w_gu_WepBase):
/// - Donald "None": SPHERE 40 at ATK_01 (+(-19,0,0) in socket space); "StickM": SPHERE 50 at ATK_01.
/// - Goofy "None" / "Attack_Discus": SPHERE 50 at ATK_00 (+(0,0,-10)); "Attack_Rush": CAPSULE1 50 x 100
///   at ATK_00 (+(0,0,60)) [inf: drawn as a sphere 50 at the socket, the capsule's axis needs the socket
///   rotation]; "Attack_Body" (Turbo): CAPSULE1 radius 80, length 92 at the owner mesh's "Center" socket
///   +(0,-12,0); "Attack_Tornado": CONVEX `Collision/tres_colltest01` (the 20-gon prism r 100, z +-50) at
///   the owner mesh's "kh_sk" with m_RelativeLocation1 (0,-65,0), m_RelativeRocation roll 90, m_Scale
///   (1.3, 1.3, 2.6) (m_Size is not used by CONVEX). kh_sk's ref pose (n_ex002_0) is Rx(-90) at the mesh
///   origin and the mesh is yawed 270 at the actor origin (n_ex002_PawnBase TresCharMesh0): the offset
///   becomes +65 up and the two rolls cancel: an upright prism, circumradius 130, half height 130, centre 65
///   above the actor origin, vertices at actor yaw + 270 + k x 18 deg [inf: kh_sk taken at its ref pose].
/// The socket-space offsets of the others are not applied (A's tracks carry the socket position only).
pub fn attack_shape(p: &NpcPawn, group: &str) -> AtkShape {
    let f = forward(p.yaw);
    let right = Vec3::new(-f.y, f.x, 0.0);
    let local = |v: Vec3| p.position() + f * v.x + right * v.y + Vec3::Z * v.z;
    let socket = socket_at(p).map(local);
    let sphere = |c: Vec3, r: f32| AtkShape { a: c, b: c, radius: r, prism: None };
    let fallback = |r: f32| p.capsule_centre() + f * (p.params.capsule_radius + r);
    match (p.chara, group) {
        (Chara::Goofy, "Attack_Body") => {
            let c = p.capsule_centre() + right * -12.0;
            AtkShape { a: c - Vec3::Z * 46.0, b: c + Vec3::Z * 46.0, radius: 80.0, prism: None }
        }
        (Chara::Goofy, "Attack_Tornado") => {
            let (r, hh, up) = TORNADO_HULL;
            let centre = p.position() + Vec3::Z * up;
            let prism = Prism { centre, radius: r, half_height: hh, yaw: p.yaw + 270.0, sides: 20 };
            AtkShape { a: centre - Vec3::Z * hh, b: centre + Vec3::Z * hh, radius: r, prism: Some(prism) }
        }
        (Chara::Donald, "StickM") => sphere(socket.unwrap_or_else(|| fallback(50.0)), 50.0),
        _ => {
            let r = weapon_radius(p.chara);
            sphere(socket.unwrap_or_else(|| fallback(r)), r)
        }
    }
}

/// Goofy's Attack_Tornado hull: (circumradius 100 x 1.3, half height 50 x 2.6, centre height 65), see
/// `attack_shape`.
pub const TORNADO_HULL: (f32, f32, f32) = (100.0 * 1.3, 50.0 * 2.6, 65.0);

pub(crate) fn seg_seg_dist(a0: Vec3, a1: Vec3, b0: Vec3, b1: Vec3) -> f32 {
    // sampled closest distance (both segments short): exact enough for overlap tests at cm scale
    let mut best = f32::MAX;
    for i in 0..=8 {
        let p = a0.lerp(a1, i as f32 / 8.0);
        best = best.min(dist_point_segment(p, b0, b1));
    }
    best
}

/// One hit test of the open attack window against the host's targets.
pub fn hit_test(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let Some((attack, group)) = p.anim.open_attack_group() else { return };
    let Some(row) = tables::attack_row(p.chara, attack) else { return };
    let shape = attack_shape(p, group);
    let centre = (shape.a + shape.b) * 0.5;
    let mut targets = std::mem::take(&mut p.scratch_targets);
    targets.clear();
    host.targets.enumerate(&mut targets);
    let mut shapes: Vec<HurtShape> = Vec::new();
    for info in &targets {
        if !info.flags.has(TargetFlags::HITTABLE) || p.hit_list.contains(&info.id) {
            continue;
        }
        shapes.clear();
        host.targets.hurt_shapes(info.id, &mut shapes);
        let touches = |s: &HurtShape| match shape.prism {
            Some(pr) => pr.touches(s.a, s.b, s.radius),
            None => seg_seg_dist(shape.a, shape.b, s.a, s.b) <= s.radius + shape.radius,
        };
        let Some(hs) = shapes.iter().find(|s| touches(s)) else { continue };
        p.hit_list.push(info.id);
        let _ = deliver(p, row, info.id, hs.part, centre, cx, host);
    }
    p.scratch_targets = targets;
}

fn socket_at(p: &NpcPawn) -> Option<Vec3> {
    let keys = p.anim.clip.socket;
    if keys.is_empty() {
        return None;
    }
    Some(socket_track_at(keys, p.anim.time))
}

/// The attack socket (A's `ClipDef::socket`: ATK_01 Donald's staff / ATK_00 Goofy's shield, actor
/// frame, per sampled frame) at `t`, linear between keys.
pub fn socket_track_at(keys: &[crate::anim::SocketKey], t: f32) -> Vec3 {
    let v = |k: &crate::anim::SocketKey| Vec3::new(k.x, k.y, k.z);
    if t <= keys[0].time {
        return v(&keys[0]);
    }
    for w in keys.windows(2) {
        if t <= w[1].time {
            let f = if w[1].time > w[0].time { (t - w[0].time) / (w[1].time - w[0].time) } else { 1.0 };
            return v(&w[0]).lerp(v(&w[1]), f);
        }
    }
    v(keys.last().unwrap())
}

/// Builds the hit record of a friend's attack row and hands it to the host.
/// True when the host accepted it.
pub fn deliver(p: &mut NpcPawn, row: &tables::AttackRow, target: TargetId, part: u32, at: Vec3, _cx: &StepCtx, host: &mut PartyHost) -> bool {
    let defender = Defender::of_target(host.targets.defense(target).as_ref());
    let damage = friend_damage(&p.status, row, &defender);
    let mut hit = HitRecord::empty(target);
    hit.part = part;
    hit.attack_name = row.name;
    hit.position = at;
    let dir = horizontal(at - p.position()).normalize_or_zero();
    hit.direction = if dir == Vec3::ZERO { forward(p.yaw) } else { dir };
    // (party X3G) a tier-2+ Goofy Tornado pulls: its hits knock inward
    if let Some(d) = crate::pad::goofy::hit_direction(p, row, host.targets.info(target).map(|i| i.position)) {
        hit.direction = d;
    }
    hit.damage = damage;
    hit.damage_kind = ETresDamageKind::from_game_name(row.damage_kind).unwrap_or(ETresDamageKind::Small);
    hit.attribute = ETresDamageAttribute::from_game_name(row.attribute).unwrap_or(ETresDamageAttribute::Physical);
    hit.attacker = HitAttacker { position: p.capsule_centre(), yaw: p.yaw, radius: p.params.capsule_radius, valid: true };
    hit.info = HitInfo { power: row.power, power_scale: row.power_scale, parabola_angle: row.parabola_angle, move_length: row.move_length, brake: row.brake, effect_time: row.effect_time, reaction_power: row.reaction_power, reflect_level: row.reflect_level as i8, ..HitInfo::DEFAULT };
    if row.magic || hit.attribute != ETresDamageAttribute::Physical {
        hit.info.flags50 |= HitInfo::FLAG50_MAGIC;
    }
    // (B4) Classify (0x14061FE20, the shared attack collision, as kh-core ports it) with the victim's body
    // (`Targets::body`): the victim / attacker results go into the record. no_clash (C 8.2.1): set unless the
    // row's m_ReflectLevel > 0 [inf: the "every later swing of a combo" rule is not applied to the friends].
    let body = host.targets.body(target, part);
    let shape = kh_core::combat::classify::Shape { no_clash: row.reflect_level <= 0, ..Default::default() };
    let (mut armor, mut endurance) = (None, None);
    let mut info = hit.info;
    let c = kh_core::combat::classify::classify(&body, kh_core::combat::classify::MapResponse::Other, &shape, hit.damage_kind, hit.attribute, &mut info, &mut |pw, weak| {
        armor = Some((pw, weak));
        i32::MIN
    }, &mut |pw| {
        endurance = Some(pw);
        i32::MIN
    });
    let c = if armor.is_some() || endurance.is_some() {
        let armor_left = armor.map(|(pw, weak)| host.targets.take_armor(target, pw, weak));
        let endurance_left = endurance.map(|pw| host.targets.take_reaction_power(target, pw));
        info = hit.info;
        kh_core::combat::classify::classify(&body, kh_core::combat::classify::MapResponse::Other, &shape, hit.damage_kind, hit.attribute, &mut info, &mut |_, _| armor_left.unwrap_or(0), &mut |_| endurance_left.unwrap_or(0))
    } else {
        c
    };
    hit.info = info;
    let res = host.targets.apply_hit(&hit);
    crate::hitfx::on_contact(p, row, at, &res, host); // (AUD) the hit's sound and effect (Sora's HitEffects rule; none before)
    // The friend's own contact (ATresNpcPawnBase notify slot 15 = 0x141271720, after the base 0x140549A80): an
    // attacker result REFLECT (3) or GUARD_REFLECT (4) -> RequestReflect (iface vt+0x448 = 0x141270B20: the state
    // registered (vt+0x440), not already in it; +0x88 = the result, +0x8C = the hit actor) and pawn vt+0x25A8
    // (the AI controller's message, not modelled). GUARD (5) does not repel.
    if matches!(c.attacker, kh_core::host::ReactionResult::Reflect | kh_core::host::ReactionResult::GuardReflect) {
        p.reflect_from = Some(host.targets.info(target).map_or(at, |i| i.position));
    }
    if res.accepted {
        p.hits_this_action += 1;
        if let Some(mut run) = p.action {
            run.success = true;
            p.action = Some(run);
        }
        p.events.push(PartyEventKind::Hit { target, damage: res.damage_dealt, killed: res.killed, attack: row.name });
        crate::pad::goofy::on_hit_landed(p, res.damage_dealt); // (party X3G) the melee cancel clock, the Shield Guardian gauge
    }
    res.accepted
}

/// The uid of a cure / item target for the voice-group number (Sora 1; a member its chara uid; 0 = himself / none).
pub fn target_uid(p: &NpcPawn, target: TargetRef, cx: &StepCtx) -> u8 {
    match target {
        TargetRef::Sora => 1,
        TargetRef::Member(m) if m == p.id => 0,
        TargetRef::Member(m) => cx.members.iter().find(|x| x.0 == m).map(|x| x.2).unwrap_or(0),
        _ => 0,
    }
}

/// E.4: the cast clip's own MagicCast / GiveRecovery group notifies play the line; the pawn only
/// sets the anim interface's voice group number (+0x1F0) before the clip (0x14122D300 [C]):
/// attack magic -> `magic_voice_number` (slot 1325), Cure on another character ->
/// `target_voice_number`, Cure on himself -> 0.
fn set_cast_voice(p: &mut NpcPawn, def: ActionDef, target: TargetRef, cx: &StepCtx, host: &mut PartyHost) {
    use kh_core::host::party::{magic_voice_number, target_voice_number};
    let m = kh_core::host::PartyMember(p.id.0);
    let n = if def == ActionDef::DonaldCure {
        match target_uid(p, target, cx) {
            0 => 0,
            uid => target_voice_number(m, uid, false, p.rng.next_u32()),
        }
    } else {
        magic_voice_number(m, p.rng.next_u32())
    };
    host.audio.party_voice_group_no(m, n);
}

/// The same for an item used on someone (UseItem; E.4).
pub fn set_item_voice(p: &mut NpcPawn, target: TargetRef, cx: &StepCtx, host: &mut PartyHost) {
    let m = kh_core::host::PartyMember(p.id.0);
    let n = match target_uid(p, target, cx) {
        0 => 0,
        uid => kh_core::host::party::target_voice_number(m, uid, false, p.rng.next_u32()),
    };
    host.audio.party_voice_group_no(m, n);
}

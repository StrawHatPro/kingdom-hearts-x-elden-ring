//! OWNER: the actions implementer. What the action / traversal states share
//! (docs/sora-actions-spec.md; Part II governs; "II.x" below): the pawn's and the controller's
//! fields of these states, the body-collision reaction type and what an incoming hit does with
//! it, the stance inputs (battle flag, weapon out, last pose, land pose), `Draw` / `Sheathe`,
//! and the name selectors. The states themselves are `states/{dodge_roll, guard, air_slide,
//! glide}.rs` and the type / landing / stance parts of `states/{jump, idle, run}.rs`.
//!
//! Constants: `sheets/port/sora-actions.json` -> `ActionParams` (`A` below).

use glam::Vec3;

use crate::anim::{AnimId, Notifies, NotifyClass, NotifyData, NotifyPhase};
use crate::enums::ETresAbilityKind as Ability;
use crate::host::{Attach, EffectId, EffectSpawn, EventKind, TargetId};
use crate::pawn::{Ctx, Pawn};
use crate::states::StateData;

include!(concat!(env!("KH_GEN_DIR"), "/sora_actions.rs"));

/// The world probes of the traversal states (tier 2).
pub(crate) mod probes;

/// The sheet's constants.
pub(crate) const A: ActionParams = ACTION_PARAMS;

/// `ETresBodyCollReactionType` (II.1 "Body-collision reaction types"): the values these states set.
pub mod reaction {
    pub const DEFAULT: u8 = 0;
    /// GUARD: a hit is guarded from every direction (the hit-entry Guard's `Body`).
    pub const GUARD: u8 = 3;
    /// GHOST_INVINCIBLE: the hit does not exist (Dodge Roll, Air Slide, Aerial Dodge, Recovery).
    pub const GHOST_INVINCIBLE: u8 = 11;
}

/// `ETresLandAnimPose` as the land selector 0x141434330 reads it (II.3).
pub mod land_pose {
    pub const NONE: u8 = 0;
    pub const NORMAL: u8 = 1;
    pub const LIGHT: u8 = 2;
    pub const LIGHT_R: u8 = 3;
    pub const LIGHT_L: u8 = 4;
}

/// The pawn's and the controller's fields of the action states. Plain data.
#[derive(Clone, Debug, PartialEq)]
pub struct PawnActions {
    /// pawn+0xE48 bit 0: the second jump is available (II.3 "Second jump": every writer listed).
    pub second_jump: bool,
    /// Bit 1: air slide available (cleared by `AirSlide::Begin`, back one pawn tick later).
    pub air_slide_ok: bool,
    /// Bit 2: air guard available.
    pub air_guard_ok: bool,
    /// Bit 3: flow jump allowed. Bit 4: Superslide out of a flow jump allowed.
    pub flow_jump: bool,
    pub superslide_ok: bool,
    /// pawn+0x619: the body-collision reaction type on the `Body` primitives.
    pub reaction: u8,
    /// Guard slot 116: the parts `Guard` / `GuardBody` are on and `Body` is off (a player guard).
    pub guard_parts: bool,
    /// pawn+0x630 bit 1: the battle flag, mirrored every frame from the host's engaged-enemy
    /// count plus the 1.0 s linger (II.3 "The inputs" 1).
    pub battle: bool,
    /// G+0x5C0: the linger timer.
    pub battle_linger: f32,
    /// G+0x8D4 bit 4: a forced battle flag (a host's switch; off).
    pub battle_forced: bool,
    /// equipment+0x2C8 bit 1: the weapon is out.
    pub weapon_out: bool,
    /// PORT ADDITION (IMPL_ER44, not the game's code): the host holds the Keyblade out (the Elden Ring host's hero
    /// light in the dark: Sora draws it and keeps it; the idle break does not put it away). `hold_applied`: what the
    /// step saw last (a release puts it away once, out of battle).
    pub hold_weapon: bool,
    pub hold_applied: bool,
    /// pawn+0x6E4: the last animation pose (0 NORMAL, 1 BATTLE).
    pub last_pose: u8,
    /// pawn+0x6E8: the land pose (`land_pose`), valid while `land_pose_play` is the playing
    /// instance: every play by name clears it (wrapper 0x1405A0E50).
    land_pose: u8,
    land_pose_play: u32,
    /// controller+0x89C: the just-guard timer (0.1333 s from a player guard's Begin with
    /// CRITICAL_COUNTER), counted down by controller slot 636 0x1413F85D0 (`-= dt`, floor 0: located by
    /// the combat merge, docs/sora-spec-reconciliation-2.md 2).
    pub just_guard_timer: f32,
    /// controller+0x8A0 / +0x8A4 / +0x8A8: the counter (reprisal) window, its command id (3, or
    /// 5 with REVENGEDIVE, else 4 with REVENGEIMPACT) and the attacker. The counter attacks
    /// themselves are the combat implementer's: this is the window they read.
    pub counter_window: f32,
    pub counter_command: i32,
    pub counter_attacker: TargetId,
    /// controller+0x878 bit 8: the guarded hit was a just guard (Critical Counter).
    pub critical: bool,
    /// The reaction command armed by an Aerial Recovery (AirSlide slot 113): (id, seconds left);
    /// id 2 = `BLOW_COUNTER` (Payback Strike), 2.0 s or until the state's End.
    pub reaction_command: Option<(i32, f32)>,
    /// The attacker of the latest hit on him (the weak pointer Damage+0x1C4), for Aerial Recovery.
    pub last_attacker: Option<(TargetId, Vec3)>,
    /// A hit-entry Guard is queued (the Guard that ends must not close the counter window).
    pub guard_hit_pending: bool,
    /// The script ability mask's FreeFlow bit (player data +0x95B0 bit 7, `BP_SetAbilityFreeFlow`;
    /// default on): gates the flow trigger (Superslide), the wall kick and the pole.
    pub flow_unlocked: bool,
    /// For hosts and tests: hits dropped by reaction type 11, hits guarded.
    pub hits_dodged: u32,
    pub hits_guarded: u32,
    /// controller+0xC50 / +0xC58: the risk-dodge offers the enemies' `TresAnimNotify_DamageCaution`
    /// registered (II.1 "Risk Dodge"; the host registers them: `Sora::offer_risk_dodge`).
    pub risk_offers: Vec<RiskOffer>,
    /// controller+0xC40 / +0xC48: the offer `vt+0x1CB0` chose on the Reflect entry (attacker, type).
    pub risk_chosen: Option<(TargetId, u8)>,
    /// The risk-dodge prompt is on the HUD (`FUN_141414170` mode 0 shown, 1 / 3 hidden).
    pub risk_prompt: bool,
    /// pawn vt+0x1010(0): the pawn-to-pawn body collision is off (Risk Dodge): the soft body push
    /// of the combat side skips him.
    pub body_collision_off: bool,
    /// For hosts and tests: risk dodges begun.
    pub risk_dodges: u32,
    /// The traversal fields of tier 2 (II.4 - II.6): see `Traversal`.
    pub traversal: Traversal,
}

/// One risk-dodge offer (0x14 bytes in the game: type at +0, the attacker's weak pointer).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiskOffer {
    /// `ETresRiskDodgeType`: 1 UNDERSLIDE, 2 OVERSLIDE, 3 SLIDETURN, 4 WARP.
    pub kind: u8,
    pub attacker: TargetId,
    /// Seconds the enemy's warning window stays open (the host's).
    pub left: f32,
}

/// The pawn's traversal fields of tier 2 (flowmotion and terrain states).
#[derive(Clone, Debug, PartialEq)]
pub struct Traversal {
    /// The script ability mask bit 8 (`BP_SetAbilityWallRun`, player data +0x95B0; default on).
    pub wall_run_unlocked: bool,
    /// pawn+0x6F8: the dangle cool-down (0.6 s, refreshed every hanging tick; II.6).
    pub dangle_cooldown: f32,
    /// The pole just left is excluded from the probe (II.4): its id and seconds left.
    pub pole_exclude: Option<(u32, f32)>,
    /// The rail just left (its id) and seconds until it may be taken again.
    pub rail_exclude: Option<(u32, f32)>,
    /// A pawn bumped this step by the running Run state (II.6 "Swaying": the caller of `TrySway`).
    pub sway_cooldown: f32,
    /// pawn+0x668 `m_LastHitHopComponent`: the hop point hit last (excluded from the probe).
    pub last_hop: Option<u32>,
    /// pawn+0xD80 bit 2: the slope-slide latch (II.6 "Request path").
    pub slope_latch: bool,
    /// `m_bEnableClimbing`: never set in retail (the vault is dead); a host's switch.
    pub climbing_enabled: bool,
    /// pawn+0xD80 bit 0 / bit 1: floating mode and its "special" variant (`BP_NotifySetFloatingMode`).
    pub floating: bool,
    pub floating_special: bool,
    /// For hosts and tests: how often each tier-2 state began (wall run, pole, rail, dangle,
    /// slope slide, sway, hop).
    pub begun: [u32; 8],
}

impl Default for Traversal {
    fn default() -> Traversal {
        Traversal { wall_run_unlocked: true, dangle_cooldown: 0.0, pole_exclude: None, rail_exclude: None, sway_cooldown: 0.0, last_hop: None, slope_latch: false, climbing_enabled: false, floating: false, floating_special: false, begun: [0; 8] }
    }
}

impl Default for PawnActions {
    /// The pawn constructor (0x141423AC0): bits 0 - 4 of pawn+0xE48 set.
    fn default() -> PawnActions {
        PawnActions {
            second_jump: true,
            air_slide_ok: true,
            air_guard_ok: true,
            flow_jump: true,
            superslide_ok: true,
            reaction: reaction::DEFAULT,
            guard_parts: false,
            battle: false,
            battle_linger: 0.0,
            battle_forced: false,
            weapon_out: false,
            hold_weapon: false,
            hold_applied: false,
            last_pose: 0,
            land_pose: land_pose::NONE,
            land_pose_play: 0,
            just_guard_timer: 0.0,
            counter_window: 0.0,
            counter_command: 0,
            counter_attacker: TargetId::NONE,
            critical: false,
            reaction_command: None,
            last_attacker: None,
            guard_hit_pending: false,
            flow_unlocked: true,
            hits_dodged: 0,
            hits_guarded: 0,
            risk_offers: Vec::new(),
            risk_chosen: None,
            risk_prompt: false,
            body_collision_off: false,
            risk_dodges: 0,
            traversal: Traversal::default(),
        }
    }
}

// ============================================================================ abilities, stance

pub(crate) fn has(s: &Pawn, kind: Ability) -> bool {
    s.status.has(kind)
}

/// `battleAnim` (pawn vt+0x1748 0x141435A10) for the plain Keyblade (weapon kind 1): the last
/// pose is BATTLE.
pub(crate) fn battle_anim(s: &Pawn) -> bool {
    s.actions.last_pose == 1
}

/// The pawn's land pose (vt+0x1788): the value a `TresLandAnimPose` notify stored, unless
/// anything was played since.
pub(crate) fn land_pose_of(s: &Pawn) -> u8 {
    if s.actions.land_pose != land_pose::NONE && s.actions.land_pose_play == s.anim.play_id {
        s.actions.land_pose
    } else {
        land_pose::NONE
    }
}

/// The land selector 0x141434330 `(pose, bWeaponOut)` (II.3): `B` = weapon out.
pub(crate) fn land_name(pose: u8, weapon_out: bool) -> Option<AnimId> {
    Some(match (pose, weapon_out) {
        (land_pose::NORMAL, false) => AnimId::Land,
        (land_pose::NORMAL, true) => AnimId::LandB,
        (land_pose::LIGHT, false) => AnimId::LightLand,
        (land_pose::LIGHT, true) => AnimId::LightLandB,
        (land_pose::LIGHT_R, false) => AnimId::LightLandR,
        (land_pose::LIGHT_R, true) => AnimId::LightLandRB,
        (land_pose::LIGHT_L, false) => AnimId::LightLandL,
        (land_pose::LIGHT_L, true) => AnimId::LightLandLB,
        _ => return None,
    })
}

/// The idle name selector 0x141433F80 (II.3): `IdleB` / `IdleLockon` need the weapon out AND
/// `battleAnim`; everything else is `IdleN` (there is no `W` idle).
pub(crate) fn idle_name(weapon_out: bool, battle_anim: bool, lock: bool) -> AnimId {
    match (weapon_out && battle_anim, lock) {
        (true, true) => AnimId::IdleLockon,
        (true, false) => AnimId::IdleB,
        _ => AnimId::IdleN,
    }
}

/// The animation-set suffix (Part I 9.1): `N` weapon away, `W` weapon out and not `battleAnim`,
/// `B` `battleAnim`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Suffix {
    N,
    W,
    B,
}

pub(crate) fn suffix(s: &Pawn) -> Suffix {
    if battle_anim(s) {
        Suffix::B
    } else if s.actions.weapon_out {
        Suffix::W
    } else {
        Suffix::N
    }
}

/// `Draw(bOut, bEffect = 1, rate = 1.0)` (pawn vt+0x1340 -> equipment vt+0x4A8): a no-op when
/// the weapon-out bit already equals `out`; else the bit is written AT ONCE and the weapon is
/// shown / hidden with its appear / disappear sequence (docs/sora-vfx-spec.md 2.4: effect
/// `VW_SO010_APP0` at once, the mesh 0.22 s later with the 0.06 s `OpacityFade` ramp). The
/// sequence itself is the host's: it gets the `WeaponVisible` event and the effect spawn.
pub(crate) fn draw(cx: &mut Ctx, out: bool) {
    if cx.pawn.actions.weapon_out == out {
        return;
    }
    cx.pawn.actions.weapon_out = out;
    cx.set_weapon_visible(out);
    const APPEAR: &str = "VW_SO010_APP0";
    cx.spawn_effect(EffectSpawn { at: Attach::Sora { bone: Some("R_wep_skt"), offset: Vec3::ZERO }, ..EffectSpawn::at_sora(EffectId(crate::notify::notify_asset_id(APPEAR)), APPEAR) });
}

/// The `Draw(1, 1, 1.0)` of Attack Begin (0x141333210; combat spec 5 "Begin"). With the PORT
/// OPTION `Tuning::weapon_instant_on_attack` (a user decision) the host is asked for the mesh at
/// once: on the draw itself, and on the first attack of a chain also when the weapon is already
/// out (its appear sequence may still be running). The appear effect is the game's either way.
pub(crate) fn draw_for_attack(cx: &mut Ctx, first_of_chain: bool) {
    let instant = cx.pawn.tuning.weapon_instant_on_attack;
    if cx.pawn.actions.weapon_out {
        if instant && first_of_chain {
            cx.pawn.event(crate::host::EventKind::WeaponVisible { visible: true, instant: true });
        }
        return;
    }
    if !instant {
        return draw(cx, true);
    }
    cx.pawn.actions.weapon_out = true;
    cx.pawn.event(crate::host::EventKind::WeaponVisible { visible: true, instant: true });
    const APPEAR: &str = "VW_SO010_APP0";
    cx.spawn_effect(EffectSpawn { at: Attach::Sora { bone: Some("R_wep_skt"), offset: Vec3::ZERO }, ..EffectSpawn::at_sora(EffectId(crate::notify::notify_asset_id(APPEAR)), APPEAR) });
}

/// Plays `name` with the play call of its stage-1 twin `like` (the `W` / `B` names take "the
/// same axes and play arguments as the `N` set": Part I 9.3); `name == like` is the stage-1
/// call itself, so the weapon-away paths stay bit-identical to the golden runs.
pub(crate) fn play_as(s: &mut Pawn, name: AnimId, like: AnimId) {
    if name == like {
        s.anim.play(name);
    } else {
        let data = like.data();
        s.anim.play_with(name, crate::anim::PlayArgs { looping: data.looping, root_motion: data.root_motion_bit, blend_in: data.blend_in });
    }
}

/// PORT ADDITION (IMPL_ER44): the host's hold on the Keyblade (`PawnActions::hold_weapon`), once a step before the
/// controller: held and not out -> `Draw(1)` (the game's appear effect and sequence); released while out of battle ->
/// `Draw(0)` (put away with the same sequence); in battle it stays out as the game has it.
pub(crate) fn host_hold(cx: &mut Ctx) {
    let (hold, applied, out) = (cx.pawn.actions.hold_weapon, cx.pawn.actions.hold_applied, cx.pawn.actions.weapon_out);
    if hold && !out {
        draw(cx, true);
    } else if !hold && applied && out && !cx.pawn.actions.battle && !battle_anim(cx.pawn) {
        draw(cx, false);
    }
    cx.pawn.actions.hold_applied = hold;
}

/// `Sheathe(bEffect, rate)` (pawn vt+0x1358): only for weapon kinds outside {1, 0x16, 0x17}: a
/// NO-OP for the plain Keyblade (II.3), kept so that the states read like the game's.
pub(crate) fn sheathe(_cx: &mut Ctx) {}

/// `SetReaction(pawn, type, name)` (0x1405B8B00).
pub(crate) fn set_reaction(s: &mut Pawn, kind: u8) {
    s.actions.reaction = kind;
}

/// `RestoreReaction(pawn, name)` (0x1405B60D0): the default type.
pub(crate) fn restore_reaction(s: &mut Pawn) {
    s.actions.reaction = reaction::DEFAULT;
}

/// The air-slide root scale by equipped `AIRSLIDE` copies (pawn vt+0x1B90 0x141432EB0).
pub(crate) fn air_slide_scale(s: &Pawn) -> f32 {
    match s.status.count(Ability::Airslide) {
        0 | 1 => A.airslide_root_scale_lv1,
        2 => A.airslide_root_scale_lv2,
        _ => A.airslide_root_scale_lv3,
    }
}

// ============================================================================ per-frame parts

/// The parts of the controller tick and of the pawn tick 0x141449CD0 that belong to these
/// states (called from the controller update, after the cool-downs were counted down):
/// - the availability bits 1 / 2 return when their timer is at 0 (0x14144A035 - 0x14144A0A9);
/// - the battle flag is mirrored from the game object (controller slot 630, II.3): engaged
///   enemies > 0, plus the 1.0 s linger, or the forced flag;
/// - the counter window, the just-guard timer and the reaction command count down.
pub(crate) fn pawn_tick(cx: &mut Ctx, dt: f32) {
    let engaged = cx.host.targets.engaged_count();
    let s = &mut *cx.pawn;
    if !(s.combat.air_slide_cooldown > 0.0) {
        s.actions.air_slide_ok = true;
    }
    if !(s.combat.air_guard_cooldown > 0.0) {
        s.actions.air_guard_ok = true;
    }
    let a = &mut s.actions;
    let battle = if engaged > 0 {
        a.battle_linger = A.stance_battle_linger;
        true
    } else {
        a.battle_linger = (a.battle_linger - dt).max(0.0);
        a.battle_linger > 0.0 || a.battle_forced
    };
    if battle != a.battle {
        a.battle = battle;
        s.event(EventKind::BattleMode { on: battle });
    }
    let a = &mut s.actions;
    a.just_guard_timer = (a.just_guard_timer - dt).max(0.0);
    if a.counter_window > 0.0 {
        a.counter_window -= dt;
        if !(a.counter_window > 0.0) {
            close_counter_window(s);
        }
    }
    if let Some((id, left)) = s.actions.reaction_command {
        s.actions.reaction_command = (left - dt > 0.0).then_some((id, left - dt));
    }
    // The risk-dodge offers live as long as the host's warning window.
    s.actions.risk_offers.retain_mut(|o| {
        o.left -= dt;
        o.left > 0.0
    });
    // The traversal cool-downs (pawn+0x6F8 and the port's own exclusions).
    let t = &mut s.actions.traversal;
    t.dangle_cooldown = (t.dangle_cooldown - dt).max(0.0);
    t.sway_cooldown = (t.sway_cooldown - dt).max(0.0);
    for slot in [&mut t.pole_exclude, &mut t.rail_exclude] {
        if let Some((id, left)) = *slot {
            *slot = (left - dt > 0.0).then_some((id, left - dt));
        }
    }
}

/// `FUN_141414170(controller, mode, bCritical)` (II.1 "Reflect"): the risk-dodge prompt of the
/// HUD (0x14158B260(hud, mode): 0 show, 1 hide, 3 used).
pub(crate) fn risk_prompt(s: &mut Pawn, mode: u8) {
    let shown = mode == 0;
    if s.actions.risk_prompt != shown || mode == 3 {
        s.actions.risk_prompt = shown;
        s.event(EventKind::RiskDodgePrompt { shown, used: mode == 3 });
    }
    if mode != 0 {
        // mode 1: FUN_1414142D0(controller, 0) and bit 8 cleared; any mode other than 0: bit 8 cleared.
        s.actions.critical = false;
    }
}

/// The pawn's own notify handlers (its anim interface, pawn+0x440): `TresLandAnimPose` -> slot
/// +0x2C8 (pawn+0x6E8), `TresLastAnimPose` -> slot +0x2C0 (pawn+0x6E4). The notify's value: the
/// constructor defaults are LIGHT (2) and NORMAL (0).
pub(crate) fn on_notifies(cx: &mut Ctx, notifies: &Notifies) {
    let s = &mut *cx.pawn;
    for event in &notifies.events {
        if event.phase != NotifyPhase::Instant {
            continue;
        }
        match (event.class(), event.def().data) {
            (NotifyClass::TresAnimNotifyLandAnimPose, NotifyData::LandAnimPose { pose }) => {
                s.actions.land_pose = match pose {
                    Some(name) if name.ends_with("LIGHT_R") => land_pose::LIGHT_R,
                    Some(name) if name.ends_with("LIGHT_L") => land_pose::LIGHT_L,
                    Some(name) if name.ends_with("NORMAL") => land_pose::NORMAL,
                    Some(name) if name.ends_with("NONE") => land_pose::NONE,
                    _ => land_pose::LIGHT,
                };
                s.actions.land_pose_play = s.anim.play_id;
            }
            (NotifyClass::TresAnimNotifyLastAnimPose, NotifyData::LastAnimPose { pose }) => {
                s.actions.last_pose = if pose.is_some_and(|name| name.ends_with("BATTLE")) { 1 } else { 0 };
            }
            _ => {}
        }
    }
}

/// The pawn's `MoveBlockedBy` (0x1405D2240) -> the running state's move-blocked handler. Air Slide
/// (0x1413432C0; not in water) and Superslide (0x1413DC940; more than 0.1 s into the slide and
/// still sliding): after the pole probe (tier 2: no poles) `TryWallKick(hit actor, hit component,
/// &ImpactNormal, bDirect 1, bKeep 0)`.
pub(crate) fn move_blocked(state: &mut StateData, cx: &mut Ctx) {
    // (`Cmc::last_block`: the first blocking hit of this tick, recorded by the combat side.)
    let Some((hit, velocity)) = cx.pawn.cmc.last_block else { return };
    let normal = hit.impact_normal;
    // The world-hit event of the other states (II.4 "Request: who calls TryWallRun"): Jump
    // (falling, the stick held this frame: 0x14139A730, bProbe 1) and Run (running with the stick
    // above 0.5: 0x1413B2BE0, bProbe 0); each after `TryHopping` failed. The caller finishes.
    let wall_run = match state {
        StateData::Jump(j) => j.wall_run_ok(cx.pawn, velocity.z).then_some(true),
        StateData::Run(r) => r.wall_run_ok(cx.pawn).then_some(false),
        _ => None,
    };
    if let Some(probe) = wall_run {
        if probes::pitch_of(normal).abs() <= A.wallrun_surface_pitch_max {
            let yaw = cx.pawn.yaw;
            if crate::states::hopping::try_hopping(state, cx, yaw, false) || crate::states::wall_run::try_wall_run(cx, &hit, probe) {
                cx.pawn.queue.do_finish();
            }
        }
        return;
    }
    // Air Slide / Superslide: the pole probe first (II.4: `OnHit` 0x141343408 / 0x1413DCA74).
    if matches!(state, StateData::AirSlide(_) | StateData::SuperSlide(_)) && crate::states::pole_turn::probe_and_try(state, cx) {
        return;
    }
    let kick = match state {
        StateData::AirSlide(_) => true,
        StateData::SuperSlide(s) => s.may_wall_kick(cx.pawn),
        _ => false,
    };
    if kick {
        crate::states::wall_kick::try_wall_kick(state, cx, normal);
    }
}

// ============================================================================ counter window

/// `FUN_1414145C0(controller, time, attacker, bSuccess, bCritical)`, the opening part (II.1):
/// needs GUARD_COUNTER (4), REVENGEIMPACT (5) or REVENGEDIVE (6).
pub(crate) fn open_counter_window(s: &mut Pawn, time: f32, attacker: TargetId, critical: bool) {
    let (counter, impact, dive) = (has(s, Ability::GuardCounter), has(s, Ability::Revengeimpact), has(s, Ability::Revengedive));
    if !(counter || impact || dive) {
        close_counter_window(s);
        return;
    }
    let a = &mut s.actions;
    a.counter_command = if dive { 5 } else if impact { 4 } else { 3 };
    a.counter_window = time;
    a.counter_attacker = attacker;
    if critical && s.status.count(Ability::CriticalCounter) > 0 {
        s.actions.critical = true;
    }
}

/// The closing part: +0x8A0 = 0, +0x8A4 = 0, weak pointers cleared, +0x878 &= ~0x100.
pub(crate) fn close_counter_window(s: &mut Pawn) {
    let a = &mut s.actions;
    (a.counter_window, a.counter_command, a.counter_attacker, a.critical) = (0.0, 0, TargetId::NONE, false);
}

// ============================================================================ incoming hits

// What a hit on Sora meets (his reaction type, his guard parts) is classified by the combat side:
// `combat::guard::classify_on_sora` reads `PawnActions::reaction` / `guard_parts` (the full table
// of the resolver 0x14061FE20) and hands a guarded hit to `states::guard::on_guarded_hit`.

/// The attacker's position now (the host's target if it still exists, else where it was).
pub(crate) fn attacker_position(cx: &Ctx) -> Option<Vec3> {
    let (id, position) = cx.pawn.actions.last_attacker?;
    Some(cx.target(id).map_or(position, |t| t.position))
}

//! `TresPlayerState_FireMagic` (id 18): the one state that casts every spell, with its requests
//! `CanUseCommand` / `CanMagic` / `TryMagic` (docs/sora-magic-spec.md part A; part I for what
//! the shot spawns; combat strand A 5 for the interleave with attacks). Addresses are the exe's.
//!
//! The spell is the command id (state+0xB0). The clip is one of the 16 `Magic*` names by
//! (cast / shot type, ground / air, moving, finisher or grand); the notify `TresAnimNotify_Shot`
//! releases the spell actor (`magic/projectile.rs`) from the Keyblade's muzzle socket `ATK_01`,
//! and the MP is deducted there.
//!
//! Literal readings and what is not here (each reported):
//! - swimming (flag 4, the underwater remap, the SWIM exit), weapon forms other than the
//!   Kingdom Key's, turn-type units: not reachable in the port;
//! - `Jump+0xF8` (a flow jump) refuses the cast; the friend "combo finish" roll (pawn vt+0x28C0)
//!   and the HUD hint 0x14140DF30 are not made;
//! - `SetMovementMode(Falling)` at the shot (A 1.4 step 11) is made on the ground too, as the
//!   code has it (0x14137C9FE: only "not swimming" is tested; pawn vt+0x17A0 = 0x14054EAF0 has no
//!   floor test): the movement tick of the same frame lands him again;
//! - the cast effect (pawn effect slot 1, `m_EffectGroup`) is one persistent effect event with
//!   the group as its id, stopped in End; drawing the Keyblade (pawn vt+0x1340) is a
//!   `WeaponVisible` event (the stance rules are the actions implementer's).

use glam::{Vec2, Vec3};

use super::common::{on_ground, turn};
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyPhase, Notifies, PlayArgs, SEQUENCES};
use crate::combat::decide::ComboAbilities;
use crate::enums::ETresAbilityKind;
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, SoundId, SoundPlay, TargetFlags, TargetId, TargetInfo};
use crate::magic::projectile::{self, SpawnArgs, StageDef};
use crate::magic::spells::{self, Element, SpellUnit};
use crate::magic::tables::Sheet;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};
use crate::status::MpCost;

/// A 1.4 step 11 as the code has it (decompiled C 0x14137BB30: `if !swimming: pawn vt+0x17A0(3)`;
/// neither the pawn's wrapper 0x14054EAF0 nor the component's `SetMovementMode` 0x140594A00
/// tests the floor): the shot of a GROUND cast puts him into Falling too; the falling physics
/// lands him again a few frames later (the capsule floats about 2 cm over the floor). Visible
/// consequence: a follow-up cast or attack requested inside those frames sees "not on ground"
/// and is the air variant. TO CONFIRM in the running game; `false` = Falling only in the air
/// (the first implementer's reading).
pub(crate) const FALLING_AT_GROUND_SHOT: bool = true;

/// The flag word state+0xD8 (A 1.1).
pub mod flag {
    pub const AIR: u32 = 0x1;
    pub const MOVING: u32 = 0x2;
    pub const SWIM: u32 = 0x4;
    pub const MOVE_REQUEST: u32 = 0x8;
    pub const FOLLOW_UP: u32 = 0x10;
    pub const TURN: u32 = 0x20;
    pub const FINISH: u32 = 0x40;
    pub const FINISH_END: u32 = 0x80;
    pub const SHOT: u32 = 0x100;
    pub const HOVER: u32 = 0x200;
    pub const GRAND: u32 = 0x400;
    pub const CURE: u32 = 0x800;
    pub const LEAF_VEIL: u32 = 0x1000;
    pub const GRAND_REQUEST: u32 = 0x2000;
    pub const AUTO_LOCK: u32 = 0x4000;
    pub const ENDLESS: u32 = 0x8000;
    pub const FULL_MP: u32 = 0x20000;
    pub const FULL_MP_DONE: u32 = 0x40000;
    pub const WATER_RING: u32 = 0x80000;
    pub const SHOT_FIRED: u32 = 0x200000;
    pub const AIM_PITCH: u32 = 0x400000;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FireMagic {
    pub(super) sub: u8,
    /// +0x90 `m_HomingTargetComponent`; +0x88 `m_pTargetPawn` is Sora himself (a Cure).
    pub target: TargetId,
    target_self: bool,
    /// +0x98, +0x9C, +0xA0: stick magnitude and world yaw of a moving cast.
    stick: f32,
    move_yaw: f32,
    /// +0xB0, +0xB4, +0xB8, +0xC4 and the Begin copies +0xBC, +0xC0.
    pub cmd: u8,
    pub combo_index: i32,
    counter_a: i32,
    counter_b: i32,
    a_begin: i32,
    b_begin: i32,
    /// +0xC8, +0xD0, +0xD4.
    face_yaw: f32,
    pub bullets: i32,
    pitch: f32,
    pub flags: u32,
    /// +0x84 bit 5.
    pub enable_input: bool,
    /// The counters come from the state before (queued behind an attack or a cast).
    chained: bool,
    /// The command of the cast queued behind this one (0 = none or not a cast).
    next_cmd: u8,
    /// The turn alpha factor is 1.0 in the first tick.
    first_tick: bool,
    /// The unit's `m_bIsDirectPos`, and the cast effect of pawn effect slot 1.
    direct_pos: bool,
    effect: Option<EffectHandle>,
}

#[allow(dead_code)]
impl FireMagic {
    /// Reset (0x14136ECF0).
    pub const RESET: FireMagic = FireMagic {
        sub: 0,
        target: TargetId::NONE,
        target_self: false,
        stick: 0.0,
        move_yaw: 0.0,
        cmd: 0,
        combo_index: 0,
        counter_a: 0,
        counter_b: 0,
        a_begin: 0,
        b_begin: 0,
        face_yaw: 0.0,
        bullets: 0,
        pitch: 0.0,
        flags: 0,
        enable_input: false,
        chained: false,
        next_cmd: 0,
        first_tick: true,
        direct_pos: false,
        effect: None,
    };

    fn has(&self, bits: u32) -> bool {
        self.flags & bits != 0
    }

    fn unit(&self) -> Option<SpellUnit> {
        spells::unit(self.cmd)
    }

    fn target_info(&self, cx: &Ctx) -> Option<TargetInfo> {
        cx.target(self.target).filter(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE))
    }

    /// `PitchToTarget` (0x141367C00) then `ClampPitch` (0x14137DF10).
    fn aim_pitch(&mut self, cx: &mut Ctx) {
        let own = cx.pawn.cmc.location;
        let mut pitch = 0.0;
        if let Some(t) = self.target_info(cx) {
            let d = t.center - own;
            if d.length_squared() > 1.0e-8 {
                let d = d.normalize();
                pitch = d.z.clamp(-1.0, 1.0).asin().to_degrees();
            }
        }
        cx.pawn.anim.set_axis(pitch);
        let s = Sheet::Spells;
        self.pitch = match cx.pawn.anim.axis_range() {
            Some((min, max)) => pitch.clamp(min, max),
            None if pitch.abs() > s.num("cast.pitch.no_blend_threshold") => s.num("cast.pitch.no_blend_value").copysign(pitch),
            None => 0.0,
        };
    }

    /// The FIREMAGIC row of the gate 0x141425510 (`CanStyleChange`, and the grand-magic exit of
    /// `CanMagic`): `!(+0xD8 & 0x10) && +0xD0 > 0`: no follow-up queued and a shot has left.
    pub fn shot_left_unchained(&self) -> bool {
        !self.has(flag::FOLLOW_UP) && self.bullets > 0
    }

    /// The guard gate out of a cast (slot 54; A 5.4), for the actions implementer.
    pub fn can_guard(&self) -> bool {
        !self.has(flag::FOLLOW_UP | flag::FINISH) && self.enable_input
    }

    /// The dodge-roll and air-slide gate (slots 57 / 68; A 5.4).
    pub fn can_dodge(&self, s: &Pawn) -> bool {
        s.queue.flags.enable_change && !self.has(flag::FOLLOW_UP | flag::FINISH) && !s.combat.finish_end
    }

    /// `CanRun` out of a cast (slot 38; A 5.4): only from a moving cast.
    pub fn can_run(&self, s: &Pawn) -> bool {
        self.flags & (flag::FOLLOW_UP | flag::MOVING) == flag::MOVING && s.queue.flags.enable_change && !s.combat.finish_end
    }

    /// `CanUseItem`'s row (E I2): no follow-up or finisher, input and change enabled.
    pub fn can_use_item(&self, s: &Pawn) -> bool {
        !self.has(flag::FOLLOW_UP | flag::FINISH) && self.enable_input && s.queue.flags.enable_change
    }

    /// `CanAttack`'s FIREMAGIC row (combat strand A 2; `T` and the final latch are the caller's).
    pub fn can_attack(&self, s: &Pawn, max_combo: i32) -> bool {
        s.queue.queued_id() != Some(StateId::Attack) && !self.has(flag::FOLLOW_UP) && self.bullets > 0 && self.combo_index < max_combo
    }

    /// `CanLaunch` adds: the cast must not be a finisher.
    pub fn can_launch(&self, s: &Pawn, max_combo: i32) -> bool {
        self.can_attack(s, max_combo) && !self.has(flag::FINISH)
    }

    /// `TryAttack` on a running cast (0x141437BEA): the follow-up is noted; returns (the next
    /// attack's combo, bAir).
    pub fn attack_follows(&mut self) -> (i32, bool) {
        self.flags |= flag::FOLLOW_UP;
        self.next_cmd = 0;
        (self.combo_index + 1, self.has(flag::AIR))
    }
}

fn combo_abilities(s: &Pawn) -> ComboAbilities {
    ComboAbilities::of(&s.status.abilities)
}

/// Pawn slot 981 (0x141432780) as the cast's End calls it: the chain is over.
fn combo_reset(s: &mut Pawn, cooldown: bool) {
    let abilities = combo_abilities(s);
    let combat = &mut s.combat;
    (combat.finish_count, combat.combo, combat.hit_count, combat.hit_flag, combat.finish_end, combat.last_kind) = (0, 0, 0, false, false, 0);
    combat.stocks = (abilities.slash_upper, abilities.air_roll_beat);
    if cooldown {
        combat.command_cooldown = Sheet::Spells.num("cast.cooldown.command");
    }
}

/// `CanUseCommand(cmd)` (slot 51 0x14142F330; A 2.2) for the magic commands: restrictions, the
/// pawn's command cool-down, and the MP rule ("not in MP charge"; MP Safety adds "enough MP").
pub(crate) fn can_use_command(s: &Pawn, cmd: u8) -> bool {
    let Some(unit) = spells::unit(cmd) else { return false };
    if s.combat.command_cooldown > 0.0 {
        return false;
    }
    if unit.grand {
        return true;
    }
    s.status.can_cast(unit.mp, s.combat.combo)
}

/// `CanMagic(cmd)` (slot 52 0x1414286F0; A 2.2).
pub(crate) fn can_magic(state: &StateData, s: &Pawn, cmd: u8) -> bool {
    // 1. can act, `CanUseCommand`, the latch pawn+0xE60 bit 17.
    if s.status.hp < 1 || !can_use_command(s, cmd) || s.magic.command_latch {
        return false;
    }
    // 2. Grand magic takes its own exit: nothing queued.
    if spells::is_grand(cmd) {
        // pawn vt+0x29F0(cmd) = 0x141425510 (jump table 0x1414256E0; combat spec 10.3): every
        // state passes except DANGLE, SHOOTERMODE, SHOOTFLOW, ATHLETICFLOW (and three vehicle
        // states); FIREMAGIC only with no follow-up and the shot out; USE_ITEM at m_bEnableChange.
        let state_ok = match state {
            StateData::ShootFlow(_) | StateData::AthleticFlow(_) => false,
            StateData::FireMagic(m) => m.shot_left_unchained(),
            StateData::UseItem(_) => s.queue.flags.enable_change,
            _ => true,
        };
        return s.queue.nothing_queued() && state_ok;
    }
    // 3. The next queued state.
    if matches!(s.queue.queued_id(), Some(StateId::Damage | StateId::Die)) {
        return false;
    }
    let abilities = combo_abilities(s);
    let max_combo = abilities.max_combo(on_ground(s));
    // 4. By the current state (jump table 0x141428FF0).
    match state {
        // IDLE 1, RUN 2, GLIDE 4 (FLOATING 5, SWAYING 11: no such states here): always.
        StateData::Idle(_) | StateData::Run(_) | StateData::Glide(_) => true,
        // Jump+0xF8 == 0.
        StateData::Jump(j) => !j.flow,
        // GUARD 15: nothing queued; Guard+0xDC != 0 (entered by a guarded hit); m_bEnableInput
        // (the hit clip's TresEnableInput, 0.2667 s). A player guard that caught nothing refuses.
        StateData::Guard(g) => s.queue.nothing_queued() && g.hit_entry && g.enable_input,
        // DODGEROLL 16: m_bEnableChange (0.4659 s).
        StateData::DodgeRoll(_) => s.queue.flags.enable_change,
        // BADSTAT_STUN 122: m_bEnableChange and nothing queued (the stub state never sets it).
        StateData::BadstatStun(_) => s.queue.flags.enable_change && s.queue.nothing_queued(),
        // AIR_SLIDE 20: (state+0xA8 & 0x80) == 0 and m_bEnableChange.
        StateData::AirSlide(a) => !a.aborted() && s.queue.flags.enable_change,
        // Interface vt+0x618 (0x141436AA0) refuses states 21..25 and 138 before the table
        // (SUPER_SLIDE, RAIL_SLIDE, WALL_KICK, POLE_TURN, ENEMY_FLOW); REFLECT 66, the
        // formchange states, SHOOTFLOW 41 and ATHLETICFLOW 42 have no row: the default below.
        // Attack family (combat strand A 5): no cast queued yet; nothing requested during this
        // attack; its first hit window has opened; combo < MaxCombo; the combo is not over.
        // NO m_bEnableInput test and NO hit-confirm test.
        StateData::Attack(a) => s.queue.queued_id() != Some(StateId::Firemagic) && !a.next_queued && a.swing_started && a.combo < max_combo && !s.combat.finish_end,
        StateData::FireMagic(m) => s.queue.queued_id() != Some(StateId::Firemagic) && !m.has(flag::FOLLOW_UP) && m.bullets > 0 && !s.combat.finish_end && (m.has(flag::ENDLESS) || m.combo_index < max_combo),
        StateData::UseItem(_) => s.queue.flags.enable_change,
        // DAMAGE: nothing queued; Damage+0x90 == 0; Damage+0x88 in {1, 2}; m_bEnableChange.
        StateData::Damage(d) => s.queue.nothing_queued() && matches!(d.kind.value(), 1 | 2) && s.queue.flags.enable_change,
        #[allow(unreachable_patterns)]
        _ => false,
    }
}

/// `TryMagic(targetPawn, targetComp, stickMag, stickYaw, bGrand)` (slot 53 0x14143AEB0; A 2.3).
/// The command is the pawn's pending command (pawn+0xE00, written by `ExecCommand`).
pub(crate) fn try_magic(state: &mut StateData, cx: &mut Ctx, target: TargetId, target_self: bool, stick: f32, stick_yaw: f32, grand: bool) -> bool {
    let s = &mut *cx.pawn;
    let cmd = s.magic.pending_cmd;
    if !can_magic(state, s, cmd) {
        return false;
    }
    let mut next = FireMagic { cmd, target, target_self, ..FireMagic::RESET };
    let mut air = !on_ground(s);
    let idx;
    let mut push = false;
    if grand {
        idx = 0;
    } else {
        match state {
            // 1, 2, 3, GLIDE 4 (FLOATING 5: no such state): the keep timer decides.
            StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) => {
                if s.magic.combo_keep > 0.0 {
                    idx = s.combat.combo + 1;
                    (next.counter_a, next.counter_b) = s.combat.carry;
                } else {
                    idx = 0;
                }
            }
            StateData::Attack(a) => {
                idx = a.combo + 1;
                a.next_queued = true;
                next.chained = true;
                push = true;
            }
            StateData::FireMagic(m) => {
                m.flags |= flag::FOLLOW_UP;
                m.next_cmd = cmd;
                idx = m.combo_index + 1;
                // A queued cast keeps the air flag of the request moment.
                next.chained = true;
                push = true;
            }
            _ => idx = 0,
        }
    }
    // GLIDE (0x14143B368 case 4): the move flag is forced: flags |= 8, +0x9C = actor yaw,
    // +0x98 = 1.0; air = 1 whatever the floor says (a glide carried along the ground).
    let forced_move = !grand && matches!(state, StateData::Glide(_));
    if forced_move {
        air = true;
        next.flags |= flag::MOVE_REQUEST;
        next.move_yaw = s.yaw;
        next.stick = 1.0;
    }
    if air {
        next.flags |= flag::AIR;
    }
    next.combo_index = idx;
    // The move request: Velocity.X^2 + Velocity.Y^2 > 40000 AND stick magnitude > 0.5.
    let sp = Sheet::Spells;
    if !forced_move && s.cmc.velocity.truncate().length_squared() > sp.num("cast.request.move_speed_sq_min") && stick > sp.num("cast.request.move_stick_min") {
        next.flags |= flag::MOVE_REQUEST;
        next.move_yaw = stick_yaw;
        next.stick = stick;
    }
    if grand {
        next.flags |= flag::GRAND_REQUEST;
    }
    s.combat.combo = idx;
    if push {
        s.queue.push_state(next);
    } else {
        s.queue.set_state(next);
    }
    true
}

/// The cast effect of an effect group (the Keyblade's `ead_wep_cal0` table; A 4.4).
fn cast_effect_name(group: i32) -> &'static str {
    match group {
        1301..=1306 | 501 => "vm_fir_cal0",
        1320..=1325 | 502 => "vm_blz_cal0",
        1340..=1345 => "vm_thd_cal0",
        1360..=1363 => "vm_wat_cal0",
        1400..=1405 | 503 => "vm_air_cal0",
        1420 | 1422 | 1424 => "vm_cur_cal0",
        _ => "ead_wep_cal0",
    }
}

/// The voice of the clip's `TresPlayVoice` notify: group `sora_magic`, index = the unit's voice
/// group (A 4.5). Names are `sora_magic:<index>`; the id is the index.
/// (ER77) The cast voice's seam name with the spell's element on the two SHARED cast lines (sora_magic 20: Fire /
/// Blizzard / Water, 21: Thunder / Aero): "sora_magic:20#fire". The audio side plays the same KH3 line (the tag is
/// dropped by kh-audio's routes); an outfit's own voice table (kh-audio outfit.rs) tells the element by it.
pub fn voice_name_tagged(index: i32, element: crate::magic::spells::Element) -> &'static str {
    use crate::magic::spells::Element as E;
    match (index, element) {
        (20, E::Fire) => "sora_magic:20#fire",
        (20, E::Blizzard) => "sora_magic:20#blizzard",
        (20, E::Thunder) => "sora_magic:20#thunder",
        (20, E::Water) => "sora_magic:20#water",
        (20, E::Aero) => "sora_magic:20#aero",
        (20, E::Cure) => "sora_magic:20#cure",
        (21, E::Fire) => "sora_magic:21#fire",
        (21, E::Blizzard) => "sora_magic:21#blizzard",
        (21, E::Thunder) => "sora_magic:21#thunder",
        (21, E::Water) => "sora_magic:21#water",
        (21, E::Aero) => "sora_magic:21#aero",
        (21, E::Cure) => "sora_magic:21#cure",
        _ => voice_name(index),
    }
}

pub fn voice_name(index: i32) -> &'static str {
    match index {
        2 => "sora_magic:2",
        3 => "sora_magic:3",
        4 => "sora_magic:4",
        5 => "sora_magic:5",
        6 => "sora_magic:6",
        7 => "sora_magic:7",
        8 => "sora_magic:8",
        9 => "sora_magic:9",
        10 => "sora_magic:10",
        11 => "sora_magic:11",
        12 => "sora_magic:12",
        20 => "sora_magic:20",
        21 => "sora_magic:21",
        31 => "sora_magic:31",
        53 => "sora_magic:53",
        _ => "sora_magic",
    }
}

/// The Keyblade's muzzle socket `ATK_01` now, in the world: from the playing clip's socket
/// tracks (actor space; the two heaviest samples of a blend space mixed by their weights).
/// `None`: the clip has no track (the pawn location is then used: A 1.4 step 7).
pub(crate) fn muzzle(pawn: &Pawn) -> Option<Vec3> {
    let request = pawn.anim.request();
    let weights = pawn.anim.name.sample_weights(request.axis);
    let mut sum = Vec3::ZERO;
    let mut total = 0.0;
    for (name, weight) in weights {
        if weight <= 0.0 {
            continue;
        }
        // (A sample without a socket track is left out of the mix.)
        let Some(sequence) = SEQUENCES.iter().find(|s| s.name == name) else { continue };
        let track = sequence.sockets;
        let Some(last) = track.len().checked_sub(1) else { continue };
        let f = if sequence.length > 0.0 { (pawn.anim.time / sequence.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
        let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
        let (a, b) = (track[i], track[(i + 1).min(last)]);
        let at = |k: usize| a[k] + (b[k] - a[k]) * t;
        sum += Vec3::new(at(3), at(4), at(5)) * weight;
        total += weight;
    }
    if total <= 0.0 {
        return None;
    }
    let local = sum / total;
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    Some(pawn.cmc.location + Vec3::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos, local.z))
}

impl FireMagic {
    /// `OnShot` (state+0x40 slot 69 0x14137BB30; A 1.4, I 0.1, I 3.1).
    fn on_shot(&mut self, cx: &mut Ctx) {
        let Some(unit) = self.unit() else { return };
        let sp = Sheet::Spells;
        // 1.
        let had = self.flags;
        self.flags = (self.flags & !(flag::AIM_PITCH | flag::TURN)) | flag::SHOT;
        if had & (flag::GRAND | flag::FINISH_END) != 0 {
            cx.pawn.status.damage_invalid = false;
        }
        let grand = self.has(flag::GRAND);
        // 2. "MP was full when the first shot left".
        if !self.has(flag::FULL_MP_DONE) {
            self.flags |= flag::FULL_MP_DONE;
            let cost = match unit.mp {
                MpCost::All => 255,
                MpCost::Points(p) => p,
            };
            if !grand && cost != 0 && cx.pawn.status.mp >= cx.pawn.status.max_mp() {
                self.flags |= flag::FULL_MP;
            }
        }
        let (yaw, origin) = (cx.pawn.yaw, cx.pawn.cmc.location);
        let facing = {
            let a = yaw.to_radians();
            Vec3::new(a.cos(), a.sin(), 0.0)
        };
        let target = if self.target_self { TargetId::NONE } else { self.target_info(cx).map_or(TargetId::NONE, |t| t.id) };
        // info +0x52..+0x55: the combo index, A, A at Begin, B at Begin (setters vt+0x1130 / 0x1140).
        let combo = [self.combo_index, self.counter_a, self.a_begin, self.b_begin];
        let common = |def: StageDef, position: Vec3, direction: Vec3| SpawnArgs { def, position, direction, cmd: self.cmd, shoot_flow: 0, finish: self.flags & flag::FINISH_END != 0, situation: grand, full_mp: self.flags & flag::FULL_MP != 0, combo, target, attached: None, fire: true };
        let Some(def) = StageDef::first(unit.proj) else {
            cx.pawn.note("a spell's first actor is not in the projectile sheet: nothing was spawned");
            return;
        };
        if unit.element == Element::Water {
            // 4. `ShotWater` 0x14137FFA0: the barrier, attached to the pawn's root at (50, 0, 100)
            // in his frame, pitched 60 up; Sora is invincible until End.
            let offset = sp.vec3("cast.shot.water_target_vec");
            let pitch = sp.num("cast.shot.water_pitch").to_radians();
            let direction = Vec3::new(facing.x * pitch.cos(), facing.y * pitch.cos(), pitch.sin());
            let (sin, cos) = yaw.to_radians().sin_cos();
            let position = origin + Vec3::new(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos, offset.z);
            // (vfx implementer 4) the fire effects play at the muzzle (the spawn's firePos argument).
            let fire_at = muzzle(cx.pawn).unwrap_or(position);
            projectile::spawn_fired_from(cx, &SpawnArgs { attached: Some(offset), ..common(def, position, direction) }, fire_at);
            self.flags |= flag::WATER_RING;
            cx.pawn.status.damage_invalid = true;
        } else {
            // 7. The spawn position: the muzzle of weapon 0, Z limited to actor Z + 163.
            let mut p = muzzle(cx.pawn).unwrap_or(origin);
            p.z = p.z.min(origin.z + sp.num("cast.shot.max_height_above_actor"));
            let pitch = self.pitch.to_radians();
            if !unit.direct_pos {
                // The wall guard: a trace from P - D L to P; on a hit P = the hit point.
                let d = Vec3::new(facing.x * pitch.cos(), facing.y * pitch.cos(), pitch.sin());
                let l = sp.num("cast.shot.wall_trace_min_len").max((p - origin).truncate().length());
                if let Some(hit) = cx.world.line_trace(p - d * l, p) {
                    p = hit.position;
                }
                projectile::spawn(cx, &common(def, p, d));
            } else {
                // Thunder, Aero: at the target's location, else 500 ahead.
                let mut t = origin + facing * sp.num("cast.shot.direct_default_range");
                if let Some(info) = self.target_info(cx).filter(|_| !self.target_self) {
                    t = info.center;
                    t.z = t.z.max(info.position.z);
                }
                // (vfx implementer 4) the actor stands at the target; its fire effects play at the muzzle P.
                // (DON-MAGIC tip) the user, 2026-10-22: "Sora's spells need to come from the tip of his keyblade": the actor's
                // fire effects play at the blade's TIP itself (socket ATK_01 at the shot frame), not at the tip held below
                // actor Z + 163 (that limit stays for a spawned bolt's position; a raised blade, Thunder's and Aero's, is above it)
                let tip = muzzle(cx.pawn).unwrap_or(p);
                projectile::spawn_fired_from(cx, &common(def, t, facing), tip);
            }
        }
        self.bullets += 1;
        // 9. Cure Converter (POWER_CURE 103): points to the formchange gauge by the MP spent.
        if self.has(flag::CURE) {
            if let Some(points) = crate::abilities::passive::cure_converter_points(&cx.pawn.status) {
                crate::magic::grand::on_damage(cx, points, self.cmd);
            }
        }
        // 10. The MP is deducted here.
        if !grand {
            let combo = cx.pawn.combat.combo;
            cx.pawn.status.spend_command_mp(unit.mp, combo);
        }
        // 11.
        self.flags |= flag::SHOT_FIRED;
        let s = &mut *cx.pawn;
        s.combat.hit_flag = true;
        s.cmc.gravity_off = false;
        // "if not swimming: SetMovementMode(Falling)": on the ground as well.
        if FALLING_AT_GROUND_SHOT || !on_ground(s) {
            s.cmc.set_falling();
        }
    }
}

impl PlayerState for FireMagic {
    const ID: StateId = StateId::Firemagic;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin` (0x14136A8E0; A 1.2).
    fn begin(&mut self, cx: &mut Ctx) {
        // 1. Draw the Keyblade.
        cx.set_weapon_visible(true);
        // 4. The counters: from the state this cast was queued behind (its End), else the request's.
        let passed = std::mem::take(&mut cx.pawn.combat.pass_on);
        if self.chained {
            (self.counter_a, self.counter_b) = passed;
        }
        (self.a_begin, self.b_begin) = (self.counter_a, self.counter_b);
        cx.pawn.combat.carry = (0, 0);
        let Some(unit) = self.unit() else { return };
        self.direct_pos = unit.direct_pos;
        // 5. Pawn effect slot 1 = the unit's effect group.
        if unit.effect_group >= 0 {
            self.effect = Some(cx.spawn_effect(EffectSpawn { id: EffectId(unit.effect_group as u32), name: cast_effect_name(unit.effect_group), at: Attach::Sora { bone: Some("R_wep_skt"), offset: Vec3::ZERO }, rotation: Vec3::ZERO, scale: 1.0, persistent: true }));
        }
        // 6. The flags.
        let s = &mut *cx.pawn;
        if s.status.has(ETresAbilityKind::AutoLockMagic) {
            self.flags |= flag::AUTO_LOCK;
        }
        if s.status.has(ETresAbilityKind::EndlessMagic) {
            self.flags |= flag::ENDLESS;
        }
        if unit.grand || self.has(flag::GRAND_REQUEST) {
            self.flags |= flag::GRAND;
        }
        if spells::is_cure(self.cmd) {
            self.flags |= flag::CURE;
            if s.status.has(ETresAbilityKind::LeafVeil) {
                self.flags |= flag::LEAF_VEIL;
            }
        }
        if self.has(flag::GRAND | flag::LEAF_VEIL) {
            s.status.damage_invalid = true;
        }
    }

    /// `Tick` (0x1413794B0; A 1.3).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let sp = Sheet::Spells;
        let Some(unit) = self.unit() else {
            cx.pawn.queue.do_chain();
            return;
        };
        // The target refresh: the controller's current target, unless Magic Lock-On or Cure.
        if !self.has(flag::AUTO_LOCK | flag::CURE) {
            self.target = cx.pawn.combat.lockon.current;
        }
        if self.target != TargetId::NONE && self.target_info(cx).is_none() {
            self.target = TargetId::NONE;
        }
        if self.sub == 0 {
            // bit 1 = bit 3.
            if self.has(flag::MOVE_REQUEST) {
                self.flags |= flag::MOVING;
            }
            let s = &mut *cx.pawn;
            if self.has(flag::CURE) {
                // Cure closes the combo at once.
                s.combat.finish_end = true;
            } else if !self.has(flag::ENDLESS) {
                let abilities = combo_abilities(s);
                let (max, nfin) = (abilities.max_combo(on_ground(s)), abilities.finishers());
                let idx = self.combo_index;
                if idx < max - nfin {
                    s.combat.finish_count = 0;
                } else {
                    let f = s.combat.finish_count;
                    self.flags |= flag::FINISH;
                    if f >= nfin - 1 || idx >= max - 1 {
                        s.combat.finish_end = true;
                        self.flags |= flag::FINISH_END;
                        s.status.damage_invalid = true;
                    }
                    s.combat.finish_count = f + 1;
                }
            }
            // (XSI) CHANDELIER (XS 2.7): a Blizzard-family Finish End becomes the falling ice chandelier.
            if self.has(flag::FINISH_END) && !self.has(flag::GRAND) && crate::abilities::xs::chandelier::take_finish_end(cx, self.cmd, unit.mp, self.combo_index, self.target) {
                cx.pawn.status.damage_invalid = false;
                return;
            }
            let s = &mut *cx.pawn;
            // A finish-end or grand cast is never a moving cast.
            if self.has(flag::FINISH_END | flag::GRAND) {
                self.flags &= !flag::MOVING;
            }
            let b = self.has(flag::FINISH_END | flag::GRAND);
            let air = self.has(flag::AIR | flag::SWIM);
            let moving = self.has(flag::MOVING);
            // The clip (0x141379D3A..).
            let anim = if self.has(flag::CURE) || unit.cast_type {
                match (moving, b && !self.has(flag::CURE), air) {
                    (true, _, false) => AnimId::MagicCastM,
                    (true, _, true) => AnimId::MagicAirCastM,
                    (false, true, false) => AnimId::MagicChant,
                    (false, true, true) => AnimId::MagicAirChant,
                    (false, false, false) => AnimId::MagicCast,
                    (false, false, true) => AnimId::MagicAirCast,
                }
            } else if moving {
                // delta = norm(+0x9C - actorYaw): |delta| < 50 -> MF, delta < 0 -> ML, else MR.
                let delta = wrap_degrees(self.move_yaw - s.yaw);
                let side = sp.num("cast.tick.move_anim_side_angle");
                match (delta.abs() < side, delta < 0.0, air) {
                    (true, _, false) => AnimId::MagicFireMF,
                    (true, _, true) => AnimId::MagicAirFireMF,
                    (false, true, false) => AnimId::MagicFireML,
                    (false, true, true) => AnimId::MagicAirFireML,
                    (false, false, false) => AnimId::MagicFireMR,
                    (false, false, true) => AnimId::MagicAirFireMR,
                }
            } else {
                match (b, air) {
                    (false, false) => AnimId::MagicFire,
                    (false, true) => AnimId::MagicAirFire,
                    (true, false) => AnimId::MagicShoot,
                    (true, true) => AnimId::MagicAirShoot,
                }
            };
            // Ground / air side effects.
            let hover_scale = sp.num("cast.tick.hover_gravity_scale");
            if !moving {
                s.cmc.stop_movement();
            } else {
                // Upward speed is removed; the horizontal velocity is kept as the request.
                if s.cmc.velocity.z > 0.0 {
                    s.cmc.velocity.z = 0.0;
                }
            }
            if !air {
                s.cmc.gravity_off = false;
            } else {
                s.cmc.gravity_scale = hover_scale;
                self.flags |= flag::HOVER;
                s.cmc.gravity_off = true;
            }
            // Play(name, slot 0, blendIn -1 (asset), mode 2, effect group, rate 1).
            s.anim.play_with(anim, PlayArgs::mode(2, -1.0));
            self.flags |= flag::AIM_PITCH | flag::TURN;
            self.face_yaw = s.yaw;
            self.aim_pitch(cx);
            self.sub = 1;
        } else {
            if self.has(flag::AIM_PITCH) {
                self.aim_pitch(cx);
            }
            let s = &mut *cx.pawn;
            // A queued Fire / Fira / Firaga chains at once.
            if self.has(flag::FOLLOW_UP) && s.queue.queued_id() == Some(StateId::Firemagic) && spells::is_fire_chain(self.next_cmd) {
                s.queue.flags.enable_change = true;
            }
            if s.anim.ended || (s.queue.flags.enable_change && self.has(flag::FOLLOW_UP)) {
                s.queue.do_chain();
            }
        }
        // ---- every tick, both sub-steps
        let s = &mut *cx.pawn;
        if !self.enable_input && self.has(flag::SHOT) && s.anim.time > sp.num("cast.tick.enable_input_time") {
            self.enable_input = true;
        }
        if self.has(flag::TURN) {
            if let Some(t) = cx.target(self.target).filter(|_| !self.target_self) {
                let d = (t.center - cx.pawn.cmc.location).truncate();
                if d.length_squared() > 1.0e-8 {
                    self.face_yaw = d.y.atan2(d.x).to_degrees();
                }
            }
            let k = if self.first_tick { sp.num("cast.tick.turn_alpha_first") } else { sp.num("cast.tick.turn_alpha") };
            turn(cx.pawn, self.face_yaw, k * dt * sp.num("cast.tick.turn_alpha_scale"));
        }
        self.first_tick = false;
        let s = &mut *cx.pawn;
        if self.has(flag::MOVING) {
            // XY velocity = direction(+0x9C) x GetMaxWalkSpeed().
            let a = self.move_yaw.to_radians();
            let speed = s.cmc.max_walk_speed;
            s.cmc.set_velocity_xy(Vec2::new(a.cos(), a.sin()) * speed);
        }
        if s.queue.flags.enable_change && self.has(flag::HOVER) {
            // ResetGravityScale, gravity enabled, SetMovementMode(Falling).
            self.flags &= !flag::HOVER;
            let p = *s.params();
            s.cmc.gravity_scale = p.gravity_scale;
            s.cmc.gravity_off = false;
            if s.cmc.is_flying() {
                s.cmc.set_falling();
            }
        }
    }

    /// `End` (0x14136CA50; A 1.5).
    fn end(&mut self, cx: &mut Ctx) {
        if let Some(handle) = self.effect.take() {
            cx.stop_effect(handle, false);
        }
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.gravity_off = false;
        s.cmc.gravity_scale = p.gravity_scale;
        s.cmc.direct_xy = None;
        if self.has(flag::WATER_RING | flag::LEAF_VEIL | flag::GRAND | flag::FINISH_END) {
            s.status.damage_invalid = false;
        }
        // The counters advance (0x141367F70): B += 1; A += 1 unless Endless Magic.
        self.counter_b += 1;
        if !self.has(flag::ENDLESS) {
            self.counter_a += 1;
        }
        let counters = (self.counter_a, self.counter_b);
        match s.queue.queued_id() {
            Some(StateId::Attack) | Some(StateId::Firemagic) => s.combat.pass_on = counters,
            _ => {
                // Anything else: the combo reset (with the command cool-down after a Finish End),
                // then pawn+0xE1C / +0xE20.
                combo_reset(s, self.has(flag::FINISH_END));
                s.combat.carry = counters;
            }
        }
        if self.has(flag::SHOT_FIRED) {
            if !self.has(flag::FINISH_END) {
                s.combat.command_cooldown = 0.0;
            }
        } else if !on_ground(s) {
            // Interrupted before the shot, in the air: the air-slide / air-guard cool-downs.
            s.combat.air_slide_cooldown = Sheet::Spells.num("cast.cooldown.air_slide_on_interrupt");
            s.combat.air_guard_cooldown = Sheet::Spells.num("cast.cooldown.air_guard_on_interrupt");
        }
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        let mut shot = false;
        let mut voice = false;
        for event in &notifies.events {
            match (event.class(), event.phase) {
                (NotifyClass::TresAnimNotifyShot, NotifyPhase::Instant) => shot = true,
                (NotifyClass::TresAnimNotifyPlayVoice, NotifyPhase::Instant) => voice = true,
                _ => {}
            }
        }
        if voice {
            // The voice group the state set for the clip's PlayVoice notify (anim interface vt+0x1F0).
            if let Some(unit) = self.unit() {
                let index = if self.has(flag::CURE) && self.target_self {
                    Sheet::Spells.list("cast.cure_voice.sora").get(1).copied().unwrap_or(31.0) as i32
                } else if self.has(flag::FINISH_END) {
                    unit.voice_finish
                } else {
                    unit.voice
                };
                // (audio implementer) the number on the seam: the generic "sora_magic" notify
                // event of this same notify then names the same unit.
                cx.set_voice_group_no(index);
                cx.play_sound(SoundPlay { id: SoundId(index as u32), name: voice_name_tagged(index, unit.element), at: Attach::Sora { bone: Some("atama"), offset: Vec3::ZERO }, volume: 1.0, pitch: 1.0, looping: false, spatial: true });
            }
        }
        if shot {
            // One Shot notify = one actor (I 0.1; the three samples of a blend space carry the
            // same notify: it is handled once per update).
            self.on_shot(cx);
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// For hosts and tests: the running cast (`Sora::cast`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CastView {
    pub cmd: u8,
    pub combo_index: i32,
    pub flags: u32,
    pub bullets: i32,
    pub pitch: f32,
    pub enable_input: bool,
    pub target: TargetId,
}

pub(crate) fn view(state: &StateData) -> Option<CastView> {
    match state {
        StateData::FireMagic(m) => Some(CastView { cmd: m.cmd, combo_index: m.combo_index, flags: m.flags, bullets: m.bullets, pitch: m.pitch, enable_input: m.enable_input, target: m.target }),
        _ => None,
    }
}

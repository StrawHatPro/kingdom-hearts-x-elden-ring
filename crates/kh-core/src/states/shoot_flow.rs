//! `TresPlayerState_ShootFlow` (id 41): the Kingdom Key's shotlock attacks King of Hearts
//! (`KC_SHOT`, kind 1), Ragnarok (`KC_RAGNAROK`, kind 2) and Focus Syphon (kind 36)
//! (docs/sora-magic-spec.md part G, which supersedes D S3). Addresses are the exe's.
//!
//! Invincible from Begin to End. Kinds 1 / 2: a start clip with a 500 cm lift (one swept move
//! on the second tick), then one homing projectile per lock, seven per tick, placed on a
//! rotating cone around the axis Sora -> blade tip for 0.5 s and launched together; an end clip.
//! Kind 36: a cast clip; 0.6 s after its Shot notify the focus returns `trunc(rate x locks)`.
//!
//! The lists of the game's state object (projectiles, effects) are kept on the pawn
//! (`PawnMagic::volley`): a state struct is `Copy`.
//!
//! Not here: the weapon presentation mode (pawn vt+0x1388) and the weapon glow of Ragnarok
//! (vt+0x1498): effect events only; the syphon's orbs are one effect event per target (their
//! per-tick motion, G 4 `MoveEffects`, is visual and left to the host).

use glam::Vec3;

use super::common::{ground_probe, on_ground, turn};
use super::fire_magic::{muzzle, voice_name};
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyPhase, Notifies, PlayArgs};
use crate::host::{Attach, EffectId, EffectSpawn, SoundId, SoundPlay, TargetFlags, TargetId};
use crate::magic::projectile::{self, SpawnArgs, StageDef};
use crate::magic::shotlock::{self, KIND_FOCUS_ASPIR, KIND_KC_RAGNAROK, KIND_KC_SHOT};
use crate::magic::spells::cmd;
use crate::magic::tables::Sheet;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShootFlow {
    sub: u8,
    /// +0x8C `ETresShootFlowKind`.
    pub kind: u8,
    /// +0x90, +0x94, +0x98, +0x9C.
    spawn_index: i32,
    pub n: i32,
    per_tick: i32,
    remaining: i32,
    /// +0xA0, +0xA4, +0xA8, +0xAC.
    ring_timer: f32,
    syphon_timer: f32,
    face_timer: f32,
    face_yaw: f32,
    /// +0x84 bit 4 (face request); +0xCC bits 0 (ring active / shot seen), 3 (facing).
    face_request: bool,
    ring_active: bool,
    facing: bool,
    /// +0xD0, +0xE4, +0xF8.
    first_target: TargetId,
    origin: Vec3,
    lift_target: Vec3,
    /// Begin finished the state at once (no locked target left).
    finished: bool,
    /// (XSI) Spectral Rays' beams fired so far (abilities/xs/spectral.rs).
    beams: i32,
}

impl ShootFlow {
    pub const RESET: ShootFlow = ShootFlow { sub: 0, kind: 0, spawn_index: 0, n: 0, per_tick: 0, remaining: 0, ring_timer: 0.0, syphon_timer: 0.0, face_timer: 0.0, face_yaw: 0.0, face_request: false, ring_active: false, facing: false, first_target: TargetId::NONE, origin: Vec3::ZERO, lift_target: Vec3::ZERO, finished: false, beams: 0 };

    fn ragnarok(&self) -> bool {
        self.kind == KIND_KC_RAGNAROK
    }

    /// `vt+0x398(h)` 0x1413AD550: the lift check.
    fn lift_check(&mut self, cx: &Ctx, h: f32) -> bool {
        let extra = Sheet::Shot.num("kc2.lift.sweep_extra_length");
        let (hit, mut distance, _) = ground_probe(cx.pawn, cx.world, h + extra);
        if hit {
            if !(distance < h) {
                return false;
            }
        } else {
            if !on_ground(cx.pawn) {
                return false;
            }
            distance = 0.0;
        }
        self.lift_target = cx.pawn.cmc.location;
        self.lift_target.z += h - distance;
        true
    }

    /// `vt+0x3A0(dt)` 0x1413C5400: in practice one swept move.
    fn lift_step(&mut self, cx: &mut Ctx, dt: f32) -> bool {
        let (w, s) = (cx.world, &mut *cx.pawn);
        let p = *s.params();
        let dz = self.lift_target.z - s.cmc.location.z;
        let k = Sheet::Shot.num("kc2.lift.speed_factor");
        s.cmc.stop_movement();
        if dz > 0.0 && dt * k * dz < dz {
            s.cmc.velocity.z = dt * k * dz;
            return false;
        }
        s.cmc.sweep_to_height(w, &p, self.lift_target.z);
        true
    }

    /// The volley set-up `vt+0x3F0` 0x1413C3330 (G 1.5).
    fn set_up(&mut self, cx: &mut Ctx) {
        let aim = &cx.pawn.magic.aim;
        self.n = aim.total;
        if self.n > 0 {
            self.first_target = aim.locked.first().map_or(TargetId::NONE, |e| e.target);
        }
        self.ring_active = true;
        self.per_tick = Sheet::Shot.int("kc2.volley.per_tick");
        self.remaining = self.n;
        // The blade tip (socket ATK_01), sampled once.
        self.origin = muzzle(cx.pawn).unwrap_or_else(|| shotlock::lock_point(cx));
        self.ring_timer = Sheet::Shot.num("kc2.volley.ring_time");
        self.facing = false;
        self.face_timer = 0.0;
    }

    /// The direction of formation slot `phi` degrees (G 1.7's closed form): on a cone of half
    /// angle 60 around the horizontal axis `a` (pawn -> blade tip).
    fn cone(&self, cx: &Ctx, phi: f32) -> Vec3 {
        let d = self.origin - shotlock::lock_point(cx);
        let flat = Vec3::new(d.x, d.y, 0.0);
        let a = if flat.length_squared() > 1.0e-8 { flat.normalize() } else { flat };
        let right = Vec3::new(-a.y, a.x, 0.0);
        let pitch = Sheet::Shot.num("kc2.volley.cone_pitch").to_radians();
        let (sin, cos) = phi.to_radians().sin_cos();
        a * pitch.cos() + (right * sin + Vec3::Z * cos) * pitch.sin()
    }

    /// The spawner `vt+0x3F8` / `vt+0x420` (G 1.6): at the end of every update tick.
    fn spawner(&mut self, cx: &mut Ctx) {
        if self.remaining <= 0 || self.n <= 0 {
            return;
        }
        let def = StageDef::shotlock(self.ragnarok());
        let targets: Vec<TargetId> = cx.pawn.magic.aim.locked.iter().map(|e| e.target).collect();
        for i in 0..self.per_tick {
            if self.remaining <= 0 {
                return;
            }
            // The index INSIDE the tick decides the spawn direction.
            let direction = self.cone(cx, 360.0 / self.n as f32 * i as f32);
            let mut target = self.first_target;
            if !targets.is_empty() {
                // locked[spawnIndex mod lockedTargets]; a dead entry gives NO target.
                let t = targets[self.spawn_index as usize % targets.len()];
                target = if cx.target(t).is_some_and(|info| info.flags.has(TargetFlags::ALIVE)) { t } else { TargetId::NONE };
            }
            let id = projectile::spawn(cx, &SpawnArgs { def, position: self.origin, direction, cmd: cmd::SHOOTFLOW, shoot_flow: self.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target, attached: None, fire: true });
            cx.pawn.magic.volley.push(id);
            self.spawn_index += 1;
            self.remaining -= 1;
        }
    }

    /// The formation `vt+0x400` / `vt+0x428` (G 1.7): first thing in sub-step 5.
    fn formation(&mut self, cx: &mut Ctx, dt: f32) {
        if !self.ring_active || cx.pawn.magic.volley.is_empty() || self.n <= 0 {
            return;
        }
        let s = Sheet::Shot;
        self.ring_timer -= dt;
        if self.ring_timer <= 0.0 {
            self.ring_active = false;
            self.ring_timer = 0.0;
        }
        let t = self.ring_timer;
        let sign = if self.ragnarok() { s.num("kc2.volley.ragnarok_spin_sign") } else { s.num("kc2.volley.kc_spin_sign") };
        let (bias, spin, rate, ring_time, delay) = (s.num("kc2.volley.ring_spin_bias"), s.num("kc2.volley.ring_spin_rate"), s.num("kc2.volley.ring_radius_rate"), s.num("kc2.volley.ring_time"), s.num("kc2.volley.launch_delay"));
        let ids = cx.pawn.magic.volley.clone();
        for (idx, id) in ids.iter().enumerate() {
            let phi = 360.0 / self.n as f32 * idx as f32 + sign * (bias - t) * spin;
            let position = self.origin + self.cone(cx, phi) * (ring_time - t) * rate;
            let Some(p) = cx.pawn.magic.projectiles.iter_mut().find(|p| p.id == *id && !p.dead) else { continue };
            let old = p.position;
            p.position = position;
            // SetActorRotation / SetDirection of the step just made; SetMoveStartDelay(t + 0.066666).
            p.set_direction(position - old);
            p.set_move_start_delay(t + delay);
        }
    }

    /// `PostTick` 0x1413C51D0 (G 1.3): the facing toward the first locked target.
    fn post_tick(&mut self, cx: &mut Ctx, dt: f32) {
        let rate = Sheet::Shot.num("kc2.state.face_alpha_per_s");
        let mut alpha = dt * rate;
        if std::mem::take(&mut self.face_request) {
            self.facing = true;
            self.face_timer = dt * rate;
            alpha = 1.0;
        }
        if self.facing {
            if let Some(t) = cx.target(self.first_target) {
                let d = t.center - shotlock::lock_point(cx);
                if d.truncate().length_squared() > 1.0e-8 {
                    self.face_yaw = d.y.atan2(d.x).to_degrees();
                }
            }
            turn(cx.pawn, self.face_yaw, alpha);
            self.face_timer -= dt;
            if self.face_timer <= 0.0 {
                self.facing = false;
            }
        }
    }

    /// Kinds 1 and 2 (updates 0x1413B8C90 / 0x1413B88F0; G 2.1, 2.2). Returns false when the
    /// state finished (no spawner call on that tick).
    fn volley_update(&mut self, cx: &mut Ctx, dt: f32) -> bool {
        let s = Sheet::Shot;
        let (charge, fire) = if self.ragnarok() { (AnimId::M_1481, AnimId::M_1482) } else { (AnimId::M_1473, AnimId::M_1474) };
        if self.sub == 0 {
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            pawn.anim.play_with(charge, PlayArgs::mode(2, -1.0));
            self.face_request = true;
            self.sub = 4;
            if self.lift_check(cx, s.num("kc2.lift.height")) {
                self.sub = 3;
            }
            return true;
        }
        if self.sub == 3 {
            if self.lift_step(cx, dt) {
                self.sub = 4;
            }
            // Falls through into the sub-step 4 code in the same tick.
        }
        if self.sub == 3 || self.sub == 4 {
            if cx.pawn.anim.ended {
                cx.pawn.cmc.stop_movement();
                cx.pawn.cmc.set_flying();
                self.set_up(cx);
                cx.pawn.anim.play_with(fire, PlayArgs::mode(2, -1.0));
                self.sub = 5;
            }
        } else if self.sub == 5 {
            self.formation(cx, dt);
            if cx.pawn.anim.ended {
                // 0x14140E500(ctrl, 0); the end clip; m_bEnableChange = false.
                cx.pawn.anim.play_with(AnimId::ShootFlowEnd, PlayArgs::mode(2, -1.0));
                cx.pawn.queue.flags.enable_change = false;
                self.sub = 2;
            }
        } else if self.sub == 2 && cx.pawn.anim.ended {
            cx.pawn.queue.do_chain();
            return false;
        }
        true
    }

    /// (XSI) Kind 11, SPECTRAL RAYS (XS 2.8; KH3's update of kind 11 was not read): `M_1483` plays (in the air with
    /// gravity off), the beams fire on the schedule of abilities/xs/spectral.rs, the state chains at the clip's end.
    fn spectral_update(&mut self, cx: &mut Ctx) {
        if self.sub == 0 {
            let pawn = &mut *cx.pawn;
            pawn.cmc.stop_movement();
            if !on_ground(pawn) {
                pawn.cmc.gravity_off = true;
                pawn.cmc.set_flying();
            }
            pawn.anim.play_with(AnimId::M_1483, PlayArgs::mode(2, -1.0));
            self.face_request = true;
            self.sub = 5;
            return;
        }
        self.beams = crate::abilities::xs::spectral::beams(cx, self.beams);
        if cx.pawn.anim.ended {
            cx.pawn.queue.do_chain();
        }
    }

    /// Kind 36, Focus Syphon (update 0x1413B78B0; G 4).
    fn syphon_update(&mut self, cx: &mut Ctx, dt: f32) {
        let s = Sheet::Shot;
        match self.sub {
            0 => {
                let pawn = &mut *cx.pawn;
                let clip = if on_ground(pawn) {
                    AnimId::MagicCast
                } else {
                    pawn.cmc.gravity_off = true;
                    pawn.cmc.stop_movement();
                    pawn.cmc.set_flying();
                    AnimId::MagicAirCast
                };
                pawn.anim.play_with(clip, PlayArgs::mode(2, -1.0));
                self.sub = 5;
            }
            5 => {
                if self.ring_active {
                    // StartEffects: one orb per locked target (`vp_fca_eng00`), effect 0xC26 on the enemy.
                    let targets: Vec<TargetId> = cx.pawn.magic.aim.locked.iter().map(|e| e.target).collect();
                    for target in targets {
                        cx.spawn_effect(EffectSpawn { id: EffectId(0xC26), name: "vp_fca_eng00", at: Attach::Target { target, offset: Vec3::ZERO }, rotation: Vec3::ZERO, scale: 1.0, persistent: false });
                    }
                    self.syphon_timer = s.num("kc2.syphon.grant_delay");
                    self.sub = 6;
                }
            }
            6 => {
                self.syphon_timer -= dt;
                if self.syphon_timer <= 0.0 {
                    // focus += max(1, trunc(rate x total locks)): Recover(0, 0, n, 0x16).
                    let total: i32 = cx.pawn.magic.aim.locked.iter().map(|e| e.lock_count).sum();
                    let rate = crate::abilities::passive::focus_syphon_rate(&cx.pawn.status).unwrap_or(0.0);
                    let n = ((rate * total as f32) as i32).max(s.int("kc2.syphon.grant_min"));
                    cx.pawn.status.recover(0, 0, n, s.int("kc2.syphon.recover_effect_kind") as u8);
                    self.sub = 2;
                }
            }
            _ => {}
        }
        if matches!(self.sub, 2 | 6) && cx.pawn.anim.ended {
            cx.pawn.queue.do_chain();
        }
    }
}

/// `TryShootFlow` (iface slot 95 0x1414413A0): the gate, then state 41 replaces the running one.
pub(crate) fn try_shoot_flow(state: &mut StateData, cx: &mut Ctx, kind: u8) -> bool {
    if cx.pawn.status.hp < 1 || matches!(state, StateData::Die(_) | StateData::ShootFlow(_) | StateData::AthleticFlow(_)) {
        return false;
    }
    cx.pawn.queue.set_state(ShootFlow { kind, ..ShootFlow::RESET });
    true
}

impl PlayerState for ShootFlow {
    const ID: StateId = StateId::Shootflow;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin` (0x1413A5CD0; G 1.2).
    fn begin(&mut self, cx: &mut Ctx) {
        cx.set_weapon_visible(true);
        cx.pawn.cmc.stop_movement();
        cx.pawn.status.damage_invalid = true;
        cx.pawn.magic.volley.clear();
        let aim = &cx.pawn.magic.aim;
        self.n = aim.total;
        if aim.locked.is_empty() {
            self.finished = true;
            cx.pawn.queue.do_chain();
            return;
        }
        self.first_target = aim.locked[0].target;
        self.per_tick = self.n;
        self.face_yaw = cx.pawn.yaw;
        // The start sounds of the charge clips are their notifies'; the effect group 0x631.
        if self.kind == KIND_KC_SHOT {
            cx.spawn_effect(EffectSpawn { id: EffectId(Sheet::Shot.int("kc2.state.kc_effect_group") as u32), name: "ead_shf_tame0", at: Attach::Sora { bone: Some("R_wep_skt"), offset: Vec3::ZERO }, rotation: Vec3::ZERO, scale: 1.0, persistent: false });
        }
    }

    /// `Tick` (0x1413B0D10; G 1.3).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        if self.finished {
            return;
        }
        if self.kind == KIND_FOCUS_ASPIR {
            self.syphon_update(cx, dt);
        } else if self.kind == crate::abilities::xs::spectral::KIND {
            self.spectral_update(cx); // (XSI)
        } else if self.volley_update(cx, dt) {
            self.spawner(cx);
        } else {
            return;
        }
        self.post_tick(cx, dt);
    }

    /// `End` (0x1413ABA70; G 1.8).
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.gravity_off = false;
        s.cmc.gravity_scale = p.gravity_scale;
        if on_ground(s) {
            s.cmc.stop_movement();
        } else if s.cmc.is_flying() {
            s.cmc.set_falling();
        }
        s.status.damage_invalid = false;
        // The projectiles live on; the controller's release-pending flag is cleared (slot 809).
        s.magic.volley.clear();
        shotlock::release_done(cx);
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        if self.kind != KIND_FOCUS_ASPIR {
            return;
        }
        for event in &notifies.events {
            match (event.class(), event.phase) {
                // The Shot handler (slot 69) for kind 0x24: "shot notify seen".
                (NotifyClass::TresAnimNotifyShot, NotifyPhase::Instant) => self.ring_active = true,
                (NotifyClass::TresAnimNotifyPlayVoice, NotifyPhase::Instant) => {
                    let index = Sheet::Shot.int("kc2.syphon.voice_index");
                    cx.set_voice_group_no(index); // (audio implementer) the number on the seam
                    cx.play_sound(SoundPlay { id: SoundId(index as u32), name: voice_name(index), at: Attach::Sora { bone: Some("atama"), offset: Vec3::ZERO }, volume: 1.0, pitch: 1.0, looping: false, spatial: true });
                }
                _ => {}
            }
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

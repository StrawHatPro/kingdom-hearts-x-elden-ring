//! The friends' projectiles in flight: Donald's spells (`n_ex001_ProjectileMag*`) and Goofy's
//! shield (Discus). Every number is a sheet row (party-params `proj.*`), read by
//! `tools/party_sheet.js` from the n_ex001 blueprints over their parents
//! (`TresProj_Mag*Base`) over the native defaults (TresProjectileMovementComponent ctor
//! 0x141470D50: InitialSpeed = MaxSpeed = 2000):
//! - Fire: SPHERE 20 (MAG_FIRE), 1000 -> 1500 cm/s at +100 cm/s^2, homing after 0.15 s with turn
//!   max 500 deg/s growing by 100 deg/s^2 up to 3000; ends on its first hit (respawn TPR_TYPE_HIT_ANY).
//! - Blizzard: SPHERE 38 (MAG_BLIZZARD), 1500 -> 2000 at +100, homing at once, turn 180 (accel 180,
//!   max 180); ends on a pawn hit (TPR_TYPE_HIT_PAWN; the respawned BlizzardBurn has no attack shape).
//! - Thunder (m_bIsDirectPos: put at the target position): life 0.3 s, no destroy on hit; CAPSULE1
//!   radius 50 + 600 t (max 200), half length 200, centre +200; and the CONVEX tres_colltest01 at
//!   scale (2, 2, 1.5) = a cylinder of radius 200, z +-75.
//! - Aero (direct pos): life 0.12 s, no destroy on hit, SPHERE 120 (its residue, the AeroCylinder,
//!   is a TresChrLauncher, not an attack).
//! - Discus (Goofy's shield, `Attack_Discus`: SPHERE 50 at ATK_00, HITMAP): thrown by the definition's anim-interface
//!   ThrowWeapon 0x1411CD930 (from R_wep along the actor's forward turned toward the target, pitch clamped to
//!   m_ShotPitchMin..Max 0..45 and yaw to +-m_ShotYawMinMax 30, at m_fStartSpeed 2000; homing on the target
//!   unless it was beyond m_rHomingDisableYaw 90 / m_rHomingDisablePitch 70), flying under the shield's
//!   TresWeaponMovementComponent (w_gu_WepBase TresWeaponMovement0: MaxSpeed 1500, m_Accel 100, homing turn
//!   360, min dist 100). The first hit (the hit handler 0x1411CC550 via the +0x48 interface) turns homing off
//!   (MyMovement vt+0x580 = SetHoming(false)) and starts the definition's fade m_fAttackFadeTime; a wall hit
//!   (normal more than 45 deg from up, FUN_14126C8E0) stops the shield with no fade. The shield is taken back by
//!   the catch clip of the definition (Tick 0x1411C7670 sub-step 4), not by flying home.
//! Homing is kh-core's TresProjectileMovementComponent law (0x1414859E0 / 0x141481160 `steer`).

use crate::ids::{Chara, MemberId};
use crate::tables;
use glam::Vec3;
use kh_core::host::TargetId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileKind {
    Fire,
    Blizzard,
    Thunder,
    Aero,
    Discus,
}

impl ProjectileKind {
    pub const fn name(self) -> &'static str {
        match self {
            ProjectileKind::Fire => "n_ex001_ProjectileMagFire",
            ProjectileKind::Blizzard => "n_ex001_ProjectileMagBlizzard",
            ProjectileKind::Thunder => "n_ex001_ProjectileMagThunder",
            ProjectileKind::Aero => "n_ex001_ProjectileMagAero",
            ProjectileKind::Discus => "n_ex002_Discus",
        }
    }

    /// (party P3) The spell actor's class as the effect side names it (`build/sora_pack/party/vfx/party_host.json`
    /// `projectiles`: its body ParticleComp template and MyFireEffects, tools/party_audio_pack.js); the shield has no
    /// actor class of its own (it is the weapon).
    pub const fn class(self) -> Option<&'static str> {
        match self {
            ProjectileKind::Fire => Some("n_ex001_ProjectileMagFire_C"),
            ProjectileKind::Blizzard => Some("n_ex001_ProjectileMagBlizzard_C"),
            ProjectileKind::Thunder => Some("n_ex001_ProjectileMagThunder_C"),
            ProjectileKind::Aero => Some("n_ex001_ProjectileMagAero_C"),
            ProjectileKind::Discus => None,
        }
    }

    /// (party P3) The pawn-hit effect of the actor's TresAtkCollComponent m_HitEffects (m_PawnHitEffect) [A:
    /// party_host.json role "hit"].
    pub const fn hit_effect(self) -> Option<&'static str> {
        match self {
            ProjectileKind::Fire => Some("vs_fir_hit0"),
            ProjectileKind::Blizzard => Some("vs_blz_hit0"),
            ProjectileKind::Thunder => Some("vs_thd_hit0"),
            ProjectileKind::Aero => Some("vm_air_hit0"),
            ProjectileKind::Discus => None,
        }
    }

    /// The attack-data row the projectile hits with (m_DefaultAttackDataIDName of its shapes) [A].
    pub const fn attack(self) -> &'static str {
        match self {
            ProjectileKind::Fire => "MAG_FIRE",
            ProjectileKind::Blizzard => "MAG_BLIZZARD",
            ProjectileKind::Thunder => "MAG_THUNDER",
            ProjectileKind::Aero => "MAG_AERO",
            ProjectileKind::Discus => "Attack8_Discus",
        }
    }

    /// The sheet key (`proj.<key>.*`).
    pub const fn key(self) -> &'static str {
        match self {
            ProjectileKind::Fire => "fire",
            ProjectileKind::Blizzard => "blizzard",
            ProjectileKind::Thunder => "thunder",
            ProjectileKind::Aero => "aero",
            ProjectileKind::Discus => "discus",
        }
    }

    /// n_ex001_MagicSet m_bIsDirectPos: the actor is put at the target position.
    pub fn direct_pos(self) -> bool {
        match self {
            ProjectileKind::Discus => false,
            k => tables::param(&format!("magicset.{}.direct_pos", k.key())) != 0.0,
        }
    }
}

/// One attack shape of a projectile, world space: segment `a`..`b` with `radius`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitShape {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projectile {
    pub kind: ProjectileKind,
    pub owner: MemberId,
    pub chara: Chara,
    pub position: Vec3,
    pub velocity: Vec3,
    pub speed: f32,
    pub max_speed: f32,
    pub accel: f32,
    /// The homing turn max now (deg/s): starts at m_HomingTurnMax, grows by m_HomingTurnAccel per s.
    pub turn: f32,
    pub homing_delay: f32,
    pub homing_on: bool,
    pub life: f32,
    pub age: f32,
    pub target: TargetId,
    pub target_point: Option<Vec3>,
    /// One hit per target (`m_bReSpawnTakeOverAtkCollHitList` keeps the list).
    pub hits: [TargetId; 8],
    /// The spell actor's body-effect handle (E.6 `publish_projectiles`); 0 = none yet. The host / E allocates it
    /// through `SeamHandles::for_member(owner).effect()` and writes it back with `Party::projectiles` (pub).
    pub effect: u32,
    pub alive: bool,
    pub returning: bool,
    /// Discus: it has hit something (the definition's +0x20E was set by this shield).
    pub caught: bool,
    /// (party P3) A pad-driven Goofy's free throw (no target, or its target lost; our design): seconds of the out
    /// leg left (straight on); then `returning` flies it back to him at its full speed until he catches it.
    pub free: Option<f32>,
}

impl Projectile {
    /// The shield thrown by `UTresAttack8_n_ex002_Discus`'s ThrowWeapon (0x1411CD930): `dir` is the
    /// launch direction (normalized), `homing` the throw's homing decision.
    pub fn discus(owner: MemberId, origin: Vec3, dir: Vec3, homing: bool, target: TargetId, target_point: Option<Vec3>) -> Projectile {
        let mut p = Projectile::spawn_at(ProjectileKind::Discus, owner, Chara::Goofy, origin, 0.0, target, target_point, None);
        p.velocity = dir * p.speed;
        p.homing_on = homing;
        p
    }
}

fn row(kind: ProjectileKind, k: &str) -> f32 {
    tables::param(&format!("proj.{}.{}", kind.key(), k))
}

impl Projectile {
    /// `origin` is the cast point; `target_point` the target's centre (homing aim) and `ground`
    /// the target's feet (the put point of a direct-pos spell).
    pub fn spawn(kind: ProjectileKind, owner: MemberId, chara: Chara, origin: Vec3, yaw: f32, target: TargetId, target_point: Option<Vec3>) -> Projectile {
        Projectile::spawn_at(kind, owner, chara, origin, yaw, target, target_point, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_at(kind: ProjectileKind, owner: MemberId, chara: Chara, origin: Vec3, yaw: f32, target: TargetId, target_point: Option<Vec3>, ground: Option<Vec3>) -> Projectile {
        let (speed, max_speed, accel, turn, delay, homing, life) = match kind {
            ProjectileKind::Discus => {
                let s = tables::aparam(Chara::Goofy, "Attack8_Discus", "fStartSpeed");
                (s, row(kind, "MaxSpeed"), row(kind, "Accel"), row(kind, "HomingTurnMax"), 0.0, true, f32::MAX)
            }
            k => (row(k, "InitialSpeed"), row(k, "MaxSpeed"), row(k, "Accel"), row(k, "HomingTurnMax"), row(k, "HomingStartDelay"), row(k, "EnableHoming") != 0.0, row(k, "life")),
        };
        let (sn, cs) = yaw.to_radians().sin_cos();
        let mut dir = Vec3::new(cs, sn, 0.0);
        if let Some(t) = target_point {
            let d = (t - origin).normalize_or_zero();
            if d != Vec3::ZERO {
                dir = d;
            }
        }
        let start = if kind.direct_pos() { ground.or(target_point).unwrap_or(origin + dir * 300.0) } else { origin };
        Projectile { kind, owner, chara, position: start, velocity: dir * speed, speed, max_speed, accel, turn, homing_delay: delay, homing_on: homing, life, age: 0.0, target, target_point, hits: [TargetId::NONE; 8], alive: true, returning: false, caught: false, effect: 0, free: None }
    }

    pub fn already_hit(&self, t: TargetId) -> bool {
        self.hits.contains(&t)
    }

    pub fn mark_hit(&mut self, t: TargetId) {
        if let Some(s) = self.hits.iter_mut().find(|h| **h == TargetId::NONE) {
            *s = t;
        }
    }

    /// Ends on a hit (Fire / Blizzard), or lives on (m_bNoDestroyOnHit: Thunder, Aero; the shield).
    pub fn ends_on_hit(&self) -> bool {
        match self.kind {
            ProjectileKind::Discus => false,
            k => row(k, "no_destroy_on_hit") == 0.0,
        }
    }

    /// The attack shapes now (ShapeDef::extent 0x140649E40 rule: size + inc x age, capped at inc max).
    pub fn shapes(&self, out: &mut Vec<HitShape>) {
        out.clear();
        if self.kind == ProjectileKind::Discus {
            let r = tables::param("proj.discus.radius");
            out.push(HitShape { a: self.position, b: self.position, radius: r });
            return;
        }
        let n = row(self.kind, "shapes") as usize;
        for i in 0..n {
            let s = |k: &str| row(self.kind, &format!("shape{i}.{k}"));
            let grow = |base: f32, inc: f32, max: f32| {
                let v = (base + inc * self.age).max(0.0);
                if max > 0.0 { v.min(max) } else { v }
            };
            let c = self.position + Vec3::Z * s("offset_z");
            let (sx, sz) = (s("scale_x"), s("scale_z"));
            match s("kind") as u8 {
                0 => out.push(HitShape { a: c, b: c, radius: grow(s("size_x"), s("inc_x"), s("inc_max_x")) * sx.min(sz) }),
                1 => {
                    // CAPSULE1: radius Size.X, half length Size.Z along Z.
                    let r = grow(s("size_x"), s("inc_x"), s("inc_max_x")) * sx.min(sz);
                    let h = (s("size_z") * sx.min(sz) - r).max(0.0);
                    out.push(HitShape { a: c - Vec3::Z * h, b: c + Vec3::Z * h, radius: r });
                }
                _ => {
                    // CONVEX tres_colltest01: cylinder radius 100 x scale.x, z +-50 x scale.z (as a capsule).
                    let r = 100.0 * sx;
                    let h = 50.0 * sz;
                    out.push(HitShape { a: c - Vec3::Z * h, b: c + Vec3::Z * h, radius: r });
                }
            }
        }
    }

    /// Kept for the hosts: the largest radius of the shapes now.
    pub fn radius(&self) -> f32 {
        let mut v = Vec::new();
        self.shapes(&mut v);
        v.iter().map(|s| s.radius).fold(0.0, f32::max)
    }

    /// Whether the shapes are live (all of them from the first frame).
    pub fn hitting(&self) -> bool {
        !self.returning || self.kind == ProjectileKind::Discus
    }

    /// Moves by `dt` toward `aim` (the target's centre now, if any). (`owner_pos` is unused: the shield does
    /// not fly home, the catch clip takes it back.)
    pub fn advance(&mut self, dt: f32, aim: Option<Vec3>, owner_pos: Vec3) {
        self.age += dt;
        if self.age >= self.life {
            self.alive = false;
            return;
        }
        // (party P3) THE FREE THROW'S RETURN (our design): straight back to him at the shield's full speed, never past
        // him (the catch takes it there, `Party::step_projectiles`)
        if let Some(left) = self.free {
            if !self.returning {
                self.free = Some(left - dt);
                if left - dt <= 0.0 {
                    self.returning = true;
                }
            }
            if self.returning {
                let to = owner_pos - self.position;
                let speed = self.max_speed.max(self.speed).max(1.0);
                let dir = to.normalize_or_zero();
                if to.length() <= speed * dt {
                    self.position = owner_pos;
                } else {
                    self.position += dir * speed * dt;
                }
                self.velocity = dir * speed;
                self.speed = speed;
                return;
            }
        }
        if self.speed <= 0.0 && self.velocity == Vec3::ZERO {
            return; // a static actor (Thunder, Aero)
        }
        if self.homing_on {
            if self.homing_delay > 0.0 {
                self.homing_delay = (self.homing_delay - dt).max(0.0);
            }
            if let (Some(a), true) = (aim, self.homing_delay <= 0.0) {
                if self.kind != ProjectileKind::Discus && self.turn > 0.0 {
                    let accel = row(self.kind, "HomingTurnAccel");
                    let max = row(self.kind, "HomingMaxTurnMax");
                    self.turn += dt * accel;
                    if max > 0.0 {
                        self.turn = self.turn.min(max);
                    }
                }
                let min = row(self.kind, "HomingMinDist");
                let delta = a - self.position;
                if min > 0.0 && delta.length_squared() <= min * min {
                    self.homing_on = false;
                } else {
                    let turn = if self.turn > 0.0 { (self.turn.to_radians() * dt).min(std::f32::consts::PI) } else { std::f32::consts::PI };
                    self.velocity = kh_core::magic::projectile::steer(self.velocity, delta, turn, -1.0);
                }
            }
        }
        // ComputeVelocity 0x1414729E0: v += normalize(v) * accel * dt, clamp to MaxSpeed; move v dt + (v' - v) dt / 2.
        let v0 = self.velocity;
        let mut v1 = v0;
        if self.accel > 0.0 && v0.length_squared() > 0.0 {
            v1 += v0.normalize() * self.accel * dt;
        }
        if self.max_speed > 0.0 && v1.length() > self.max_speed {
            v1 = v1.normalize() * self.max_speed;
        }
        self.position += v0 * dt + (v1 - v0) * dt * 0.5;
        self.velocity = v1;
        self.speed = v1.length();
    }
}

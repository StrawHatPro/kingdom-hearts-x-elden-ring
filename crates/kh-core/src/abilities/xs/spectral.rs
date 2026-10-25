//! (XSI) SPECTRAL RAYS (XS 2.8; ability SHOTLOCK_SPECTRAL_RAYS 263, Glintstone Dragon Smarag). Ever After's
//! shotlock (`TresShootFlowData` row 11 "ラプ反射" = `kh3_system.command.sf_laser_0` "Spectral Rays": up to 28
//! locks, sheet sora-magic-shotlocks `aim2.table.k11.*`; clip `M_1483` of the main anim set, rows
//! `SF_REFLECTIVELASER` 0.8 SMALL magic and `SF_REFLECTIVELASER_RANGE` 0.5 TOSS magic) as a SECOND SHOTLOCK:
//! - the Customize screen's Shotlock choice (`CommandState::shotlock`: 0 = the Keyblade's own, King of Hearts with
//!   its Ragnarok upgrade at MAX; 11 = Spectral Rays), stored per character (Sora's save record; Donald and Goofy
//!   keep their kits' own); it takes effect while the ability is equipped;
//! - RB held as before: the aim mode locks with row 11's count and interval; no Ragnarok upgrade at MAX;
//! - the attack (states/shoot_flow.rs kind 11): `M_1483` plays; over `spectral.beam_from` .. `spectral.beam_to` one
//!   beam per lock reaches its target (`SF_REFLECTIVELASER`) and refracts once to the nearest other locked target at
//!   `spectral.refract_scale` (ours). KH3's ShootFlow code of kind 11 was not read: the beam schedule is ours.
//! - the Elden Ring shotlock nerf applies unchanged (kh-er ermap.rs: rows `SF_*` are shotlock hits).

use super::{has, X};
use crate::enums::ETresAbilityKind as A;
use crate::host::{TargetFlags, TargetId, TargetInfo};
use crate::pawn::{Ctx, Pawn};

/// `ETresShootFlowKind` 11 (sheet row `spectral.kind`).
pub const KIND: u8 = 11;
/// The beams' and the refractions' hit groups (combat/hit.rs shots).
pub const GROUP: &str = "XS_RAY";
pub const GROUP_REFRACT: &str = "XS_RAY_REFRACT";
pub const ROW: &str = "SF_REFLECTIVELASER";
/// The beam's own effects (in the pack: build/sora_pack/vfx/effects).
pub const BEAM_EFFECT: &str = "vw_so072_stf_przm_one0";
pub const HIT_EFFECT: &str = "vw_so072_stf_przm_one_hit0";

/// The shotlock Sora's aim mode uses: Spectral Rays when it is chosen on the Customize screen AND equipped.
pub(crate) fn chosen(s: &Pawn) -> bool {
    s.command.shotlock == KIND && has(s, A::ShotlockSpectralRays)
}

/// The kinds the Customize screen offers: the Keyblade's own (0) and, when the ability is owned, Spectral Rays.
pub fn choices(owned: bool) -> Vec<u8> {
    let mut out = vec![0];
    if owned {
        out.push(KIND);
    }
    out
}

fn alive(t: &TargetInfo) -> bool {
    t.flags.has(TargetFlags::ALIVE)
}

/// The beams that are due at the clip time now; returns the new count fired.
pub(crate) fn beams(cx: &mut Ctx, fired: i32) -> i32 {
    let total = cx.pawn.magic.aim.total.max(0);
    if total == 0 {
        return fired;
    }
    let t = cx.pawn.anim.time;
    if t < X.spectral_beam_from {
        return fired;
    }
    let span = (X.spectral_beam_to - X.spectral_beam_from).max(0.01);
    let due = ((((t - X.spectral_beam_from) / span) * total as f32).floor() as i32 + 1).min(total);
    let mut fired = fired;
    while fired < due {
        fire(cx, fired);
        fired += 1;
    }
    fired
}

/// Beam `k`: the k-th lock in lock order (a target locked n times takes n beams, as the volley's spawner).
fn fire(cx: &mut Ctx, k: i32) {
    let locked: Vec<(TargetId, i32)> = cx.pawn.magic.aim.locked.iter().map(|e| (e.target, e.lock_count)).collect();
    let mut order: Vec<TargetId> = Vec::new();
    for &(t, n) in &locked {
        for _ in 0..n.max(1) {
            order.push(t);
        }
    }
    if order.is_empty() {
        return;
    }
    let id = order[k as usize % order.len()];
    let Some(t) = cx.target(id).filter(alive) else { return };
    hit_at(cx, &t, GROUP);
    // The refraction: the nearest OTHER locked target that lives.
    let next = locked.iter().filter(|(o, _)| *o != id).filter_map(|(o, _)| cx.target(*o).filter(alive)).min_by(|a, b| (a.center - t.center).length_squared().total_cmp(&(b.center - t.center).length_squared()));
    if let Some(n) = next {
        hit_at(cx, &n, GROUP_REFRACT);
    }
}

fn hit_at(cx: &mut Ctx, t: &TargetInfo, group: &'static str) {
    use crate::host::{Attach, EffectId, EffectSpawn};
    cx.spawn_effect(EffectSpawn { id: EffectId(crate::notify::notify_asset_id(BEAM_EFFECT)), name: BEAM_EFFECT, at: Attach::World(t.center), rotation: glam::Vec3::ZERO, scale: 1.0, persistent: false });
    crate::combat::hit::spawn_xs_shot(cx, group, ROW, t.center, (t.radius * X.spectral_hit_radius_share).max(X.spectral_hit_radius_min), X.spectral_hit_life, Some(HIT_EFFECT));
}

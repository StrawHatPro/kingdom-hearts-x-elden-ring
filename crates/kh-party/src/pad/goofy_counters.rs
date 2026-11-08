//! (GOOFY) THE BATTED SHOT (documentation/GOOFY_OVERHAUL.md 8, PLAN.md R18): rank 3 of Goofy Tornado sends a bullet it
//! deflected back at its shooter as a KH projectile (a live bullet's owner can't be changed). Rows `batted.*`.
//!
//! (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique, Goofy's big damage should solely come from his
//! guard break attacks from 6.3") THE COUNTERS THAT LIVED HERE ARE RETIRED: Counter Bash, Pin Down, Goofy Stomp, Pratfall Toss,
//! Return Fire, Ward Burst, Shield Shove and Crescendo, their window, their chain, their MP and the free Art after one. A parry
//! now gives 0 damage and its Stagger (pad/goofy.rs `answered`); the big damage is Goofy Breaker (pad/goofy_breaker.rs). Only
//! the batted shot, which Goofy Tornado shares, stays.

use super::goofy::stm;
use super::goofy_shots as shots;
use crate::goofy::{GoofyEvent, StaggerWhy, K};
use crate::host::PartyHost;
use crate::pawn::{forward, NpcPawn};
use crate::projectile::{Batted, Projectile, ProjectileAim, ProjectileKind};
use crate::view::PartyEventKind;
use glam::Vec3;
use kh_core::host::{TargetId, TargetInfo};

/// R18: the KH projectile that carries `shot` from his shield to `t`. Never a live bullet of the game's: that one was cancelled
/// where it landed.
pub fn bat_back(p: &mut NpcPawn, host: &mut PartyHost, t: &TargetInfo, shot: Batted) {
    let Some(kind) = ProjectileKind::of_key(K.batted_shot) else { return };
    let from = p.capsule_centre() + forward(p.yaw) * p.params.capsule_radius;
    let mut body = Projectile::spawn_at(kind, p.id, p.chara, from, p.yaw, ProjectileAim { target: t.id, point: Some(t.center), ground: Some(t.position) });
    if !kind.direct_pos() {
        body.velocity = (t.center - from).normalize_or_zero() * K.batted_speed;
        body.speed = K.batted_speed;
        body.max_speed = K.batted_speed;
        body.accel = 0.0;
        body.homing_delay = 0.0;
        body.homing_on = true;
        body.life = K.batted_life;
    }
    body.batted = Some(shot);
    crate::actions::spell_actor_effects(p, &mut body, from, host);
    p.events.push(PartyEventKind::ProjectileSpawned { kind: kind.name(), at: from });
    p.requests.push(crate::pawn::Request::Projectile(body));
}

/// R18: a batted shot reached its target: the hit, the impact, its Stagger. Never a hit-stop: he is done with it.
pub fn batted_landed(p: &mut NpcPawn, shot: &Batted, target: TargetId, part: u32, at: Vec3, dir: Vec3, host: &mut PartyHost) {
    if shots::deliver(p, &shot.row, target, part, at, dir, host).is_none() {
        return;
    }
    shots::effect(p, host, K.batted_effect, at, 1.0);
    shots::sound(p, host, K.batted_se, at);
    stm(p).core.events.push(GoofyEvent::Stagger { target, amount: shot.stagger, why: StaggerWhy::Art });
}

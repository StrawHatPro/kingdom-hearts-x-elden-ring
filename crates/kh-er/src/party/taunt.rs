//! (ER party I, X3G's request) OVER HERE! IN ELDEN RING: Goofy's taunt (`Party::goofy_taunt`: a centre, a radius, the
//! seconds left) pulls the enemies' HATE onto him. Elden Ring's enemies take as their target what hurt them, so every
//! hostile within the radius is given a minimal hit from Goofy's Elden Ring body:
//! - Goofy LEADING: the leader body is the player's own body, so it is a player bullet (`Bullets::fire`, the player's
//!   handle as the owner): the hate goes to the body he stands on.
//! - AI Goofy (when his kit taunts): a bullet OWNED BY HIS ALLY BODY (S3's pool, `Bullets::fire_owned`, U14's route: the
//!   hate moves to the body; go/no-go item 10 measures that route). No body = nothing (logged once).
//!   The hit is `party.taunt_phys` (1) attack, no poise, no stamina, `dmgLevel` 0 (no reaction asked): the game's minimum.
//!   Each enemy in the radius is hit once when the taunt starts and again every `party.taunt_every` s (2) while it lasts, one
//!   bullet a frame (the bullet slots are shared with every other hit). `party.taunt = 0`: off.
//!   (GF-THREAT) THE BACKUP: while `targetPriority` is on his body (prio.rs, the strong route: the game adds it to his score in every
//!   enemy's own search) these hits run `prio::BACKUP_SLOWER` times slower. Read from the exe: any hit, whatever its amount, doubles
//!   a target's score for a while. Taunt is a strong refreshed bias: nothing can force a target.
//!   THE AI BRANCH (GF-THREAT): it asked the leader's bullet slots and took a parked body too; it asks the body-owned slots and a
//!   live body now ([`route`]).

use std::sync::Mutex;

use glam::Vec3;
use kh_core::TargetId;
use kh_host::party::PartyChara;

/// (GF-THREAT) WHOSE BULLET A TAUNT'S HIT IS: the player's body for the leader; a live ally body, owned, for an AI Goofy (`None`:
/// he has no live body now: nothing can pull the hate to him). The bool: fired through the body-owned slots.
pub fn route<H>(who: PartyChara, leader: PartyChara, player: H, body: Option<H>) -> Option<(H, bool)> {
    if who == leader {
        Some((player, false))
    } else {
        body.map(|h| (h, true))
    }
}

/// (GF-THREAT) How often an enemy in reach is hit: `every`, slower while the strong route is on his body.
pub fn cadence(every: f32, priority_on: bool) -> f32 {
    if priority_on {
        every * super::prio::BACKUP_SLOWER
    } else {
        every
    }
}

/// THE PURE RULE: which enemy is taunted this frame. `enemies` = (id, KH3 position) of the hostiles; `fired` = (id, when)
/// of the hits of this taunt so far; an enemy within `radius` of `centre` not hit yet, or last hit `every` s ago, nearest
/// first.
pub fn pick(enemies: &[(TargetId, Vec3)], centre: Vec3, radius: f32, fired: &[(TargetId, f32)], now: f32, every: f32) -> Option<TargetId> {
    enemies
        .iter()
        .filter(|(_, p)| (*p - centre).truncate().length() <= radius)
        .filter(|(id, _)| fired.iter().find(|(f, _)| f == id).is_none_or(|(_, at)| now - at >= every))
        .min_by(|a, b| (a.1 - centre).truncate().length().total_cmp(&(b.1 - centre).truncate().length()))
        .map(|(id, _)| *id)
}

#[derive(Default)]
struct State {
    clock: f32,
    /// The taunt under way: who, and the hits of it so far.
    who: Option<PartyChara>,
    fired: Vec<(TargetId, f32)>,
    told_no_body: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// ONCE A FRAME (sora.rs pre_physics, after the ally bodies' pool): the taunts of the latest party step into hits.
#[cfg(windows)]
pub fn frame(core: Option<&super::glue::PartyCore>, player: eldenring::cs::FieldInsHandle, targets: &mut crate::targets::ErTargets, control: &crate::control::Control, dt: f32) {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let st = guard.get_or_insert_with(State::default);
    st.clock += dt.max(0.0);
    let taunt = core.filter(|_| control.flag("party.taunt") != Some(false)).and_then(|c| {
        c.rt.members.iter().find_map(|(chara, id)| c.rt.party.goofy_taunt(*id).filter(|t| t.left > 0.0).map(|t| (*chara, t, c.rt.leader_chara())))
    });
    let Some((who, t, leader)) = taunt else {
        if st.who.take().is_some() {
            crate::erlog!("taunt: Over Here! ended ({} enemy hit(s))", st.fired.len());
        }
        st.fired.clear();
        return;
    };
    if st.who != Some(who) {
        st.who = Some(who);
        st.fired.clear();
        crate::erlog!("taunt: {} calls OVER HERE! ({:.0} m, {:.1} s): the enemies within it are given a hit from {}", who.name(), t.radius / 100.0, t.left, if who == leader { "THE LEADER BODY (the player's body)" } else { "his ally body" });
    }
    let Some((owner, owned)) = route(who, leader, player, crate::allybody::live_handle(who)) else {
        if !st.told_no_body {
            st.told_no_body = true;
            crate::erlog!("taunt: {}'s Over Here! has no live Elden Ring body to pull the hate to (S3's pool has none for him now)", who.name());
        }
        return;
    };
    if !targets.bullets.available() || !(if owned { targets.bullets.free_owned_slot() } else { targets.bullets.free_slot() }) {
        return;
    }
    let every = cadence(control.f32("party.taunt_every").filter(|v| *v > 0.0).unwrap_or(2.0), super::prio::live_on(who, 1.0));
    let enemies: Vec<(TargetId, Vec3)> = targets.hostiles_by_distance().into_iter().map(|(_, e)| (e.id, e.pos)).collect();
    let Some(id) = pick(&enemies, t.centre, t.radius, &st.fired, st.clock, every) else { return };
    if !crate::targets::side_hit_allowed(id) {
        return; // (PARTY-AI) Protect the slam: no 1-HP side bullet at a target a Breaker winds up on
    }
    let Some(e) = targets.get(id).cloned() else { return };
    // the hit: from above and in front of the enemy toward its middle, as the party's other hits are aimed
    let towards = crate::coords::dir_kh_to_er((e.pos - t.centre) * Vec3::new(1.0, 1.0, 0.0)).normalize_or(Vec3::Z);
    let contact = e.er_pos + Vec3::Y * (e.half_height / crate::coords::CM_PER_M);
    let from = contact - towards * targets.shape.back;
    let phys = control.i64("party.taunt_phys").map_or(1, |v| v.clamp(0, 50)) as u16;
    let attack = crate::bullet::Attack { phys, poise: 0.0, stamina: 0, level: 0, carrier: Default::default(), sp_attribute: None, obj: None };
    let shape = targets.shape;
    let fired = if owned { targets.bullets.fire_owned(owner, Some(e.handle), from, towards, attack, shape) } else { targets.bullets.fire(owner, Some(e.handle), from, towards, attack, shape) };
    if fired.is_ok() {
        targets.note_side_hit(id, attack.phys as i32); // (2026-11-03) its take is its own, never a KH bullet's
    }
    st.fired.retain(|(f, _)| *f != id);
    st.fired.push((id, st.clock));
    match fired {
        Ok(row) => crate::erlog!("taunt: {}'s Over Here! hits target {} c{:04} (bullet row {row}, attack {phys}, owner {}): its hate should go to him", who.name(), id.0, e.npc_id, if owned { "his ally body" } else { "the leader body" }),
        Err(why) => crate::erlog!("taunt: the Over Here! hit on target {} could not be fired ({why})", id.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erpi_over_here_hits_each_enemy_in_reach_nearest_first_then_again_after_its_interval() {
        let e = |n: u32, x: f32| (TargetId(n), Vec3::new(x, 0.0, 0.0));
        let enemies = [e(1, 500.0), e(2, 200.0), e(3, 900.0)];
        let mut fired: Vec<(TargetId, f32)> = Vec::new();
        // 8 m: the one at 9 m is out of reach; nearest first
        assert_eq!(pick(&enemies, Vec3::ZERO, 800.0, &fired, 0.0, 2.0), Some(TargetId(2)));
        fired.push((TargetId(2), 0.0));
        assert_eq!(pick(&enemies, Vec3::ZERO, 800.0, &fired, 0.1, 2.0), Some(TargetId(1)));
        fired.push((TargetId(1), 0.1));
        // both hit: nothing until the interval
        assert_eq!(pick(&enemies, Vec3::ZERO, 800.0, &fired, 1.0, 2.0), None);
        assert_eq!(pick(&enemies, Vec3::ZERO, 800.0, &fired, 2.05, 2.0), Some(TargetId(2)));
        // no enemy in reach
        assert_eq!(pick(&enemies, Vec3::new(5000.0, 0.0, 0.0), 800.0, &[], 0.0, 2.0), None);
    }

    /// (GF-THREAT) The AI branch: an AI Goofy's hit is his live body's, through the body-owned slots; no body, no hit; the leader's
    /// is the player's.
    #[test]
    fn gf_taunt_works_when_he_is_ai_too() {
        assert_eq!(route(PartyChara::Goofy, PartyChara::Goofy, 1, Some(2)), Some((1, false)), "he leads: the player's body");
        assert_eq!(route(PartyChara::Goofy, PartyChara::Sora, 1, Some(2)), Some((2, true)), "AI: his own live body, owned");
        assert_eq!(route(PartyChara::Goofy, PartyChara::Sora, 1, None), None, "AI with no live body");
    }

    #[test]
    fn gf_the_hate_hits_slow_down_while_the_strong_route_is_on_him() {
        assert_eq!(cadence(2.0, false), 2.0);
        assert_eq!(cadence(2.0, true), 2.0 * crate::party::prio::BACKUP_SLOWER);
    }

    #[test]
    fn gf_taunt_radius_is_on_the_ground() {
        let flying = [(TargetId(1), Vec3::new(500.0, 0.0, 1500.0))];
        assert_eq!(pick(&flying, Vec3::ZERO, 1000.0, &[], 0.0, 2.0), Some(TargetId(1)));
    }
}

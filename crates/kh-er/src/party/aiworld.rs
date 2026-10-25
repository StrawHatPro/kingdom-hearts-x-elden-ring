//! (ER party S1) ELDEN RING'S CHARACTERS AS THE PARTY SEES THEM (contract ER.3 `PartyTargets`): the friends' hit targets
//! (their hits are `ErTargets::apply_hit`: player bullets, as Sora's; kills, juggles and poise follow unchanged) and the
//! AI's enemy list (`ai_actors`: every character of `ErTargets.list`, `ActorRef::Enemy(target id)`, centre = the lock-on
//! point; non-hostile characters as bystanders). The trees' "player attacked lately" is our own latch: a hostile within
//! 15 m of the leader playing an attack animation, kept for 1 s (ErTargets keeps its own attack memory private).

use glam::Vec3;
use kh_ai::{ActorRef, ActorView};
use kh_core::host::{Ally, AttackShape, FinishKind, Friend, FriendLinkRequest, HitResult, HurtShape, PushBody, TargetBody, TargetDefense, TargetInfo};
use kh_core::{HitRecord, TargetId, Targets};

use crate::targets::ErTargets;

/// The leader's "attacked lately" latch (steps): an enemy attack seen near him this many steps ago or less.
pub const ATTACKED_STEPS: u32 = 60;

/// `PartyTargets` over Elden Ring's characters.
pub struct ErPartyTargets<'a> {
    pub targets: &'a mut ErTargets,
    pub attacked: bool,
    /// (party AIX) ErTargets' engaged count measured round the LEADER (the party's battle flag while a friend leads,
    /// kh-partyhost `Leadership::battle_step`); ErTargets measures round `targets.sora` = Sora's feet, which is AI Sora
    /// (or, with Sora out of the party, where he was left) while a friend leads. The same count while Sora leads.
    pub engaged: u32,
}

impl<'a> ErPartyTargets<'a> {
    /// `latch`: the host's step counter of the "attacked lately" rule (counts up; reset by an attack seen near `leader`).
    pub fn new(targets: &'a mut ErTargets, leader: Vec3, latch: &mut u32) -> ErPartyTargets<'a> {
        let near = targets.list.iter().any(|e| e.hostile && e.alive && e.active && e.in_attack && e.pos.distance(leader) < 1500.0);
        *latch = if near { 0 } else { latch.saturating_add(1) };
        // (party AIX) the same rule as ErTargets::engaged_count (range + hit / attack memory), round the leader
        let at = std::mem::replace(&mut targets.sora, leader);
        let engaged = targets.engaged_count();
        targets.sora = at;
        ErPartyTargets { targets, attacked: *latch <= ATTACKED_STEPS, engaged }
    }
}

impl Targets for ErPartyTargets<'_> {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        self.targets.enumerate(out)
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.targets.info(id)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        self.targets.hurt_shapes(id, out)
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.targets.apply_hit(hit)
    }
    fn defense(&self, id: TargetId) -> Option<TargetDefense> {
        self.targets.defense(id)
    }
    fn allies(&self, out: &mut Vec<Ally>) {
        self.targets.allies(out)
    }
    fn heal_ally(&mut self, id: TargetId, amount: i32, effect: u8) {
        self.targets.heal_ally(id, amount, effect)
    }
    fn lock_on_range(&self, id: TargetId) -> f32 {
        self.targets.lock_on_range(id)
    }
    fn engaged_count(&self) -> u32 {
        self.engaged // (party AIX)
    }
    fn body(&self, id: TargetId, part: u32) -> TargetBody {
        self.targets.body(id, part)
    }
    fn take_armor(&mut self, id: TargetId, power: i32, weak: bool) -> i32 {
        self.targets.take_armor(id, power, weak)
    }
    fn take_reaction_power(&mut self, id: TargetId, power: i32) -> i32 {
        self.targets.take_reaction_power(id, power)
    }
    fn rush_suppression(&self, id: TargetId) -> bool {
        self.targets.rush_suppression(id)
    }
    fn terrain_dz(&self, id: TargetId) -> f32 {
        self.targets.terrain_dz(id)
    }
    fn push_body(&self, id: TargetId) -> Option<PushBody> {
        self.targets.push_body(id)
    }
    fn reaction_state(&self, id: TargetId) -> u8 {
        self.targets.reaction_state(id)
    }
    fn script_hold(&mut self, id: TargetId, hold: Option<kh_core::host::ScriptHold>) {
        self.targets.script_hold(id, hold) // (ZETTA) Zettaflare's held target
    }
    fn held_animated(&self, id: TargetId) -> bool {
        self.targets.held_animated(id) // (ZETTA)
    }
    fn script_finale(&mut self, id: TargetId, finale: Option<kh_core::host::ScriptFinale>) {
        self.targets.script_finale(id, finale) // (ZETTA)
    }
    fn drawn_extent(&self, id: TargetId) -> Option<(f32, f32)> {
        self.targets.drawn_extent(id) // (ZETTA 2026-10-23) what the host draws of it (the framing and the beam's cover)
    }
    fn attack_shapes(&self, out: &mut Vec<AttackShape>) {
        self.targets.attack_shapes(out)
    }
    fn touched(&mut self, hit: &HitRecord) {
        self.targets.touched(hit)
    }
    fn finish_kind(&self, id: TargetId) -> FinishKind {
        self.targets.finish_kind(id)
    }
    fn friends(&self, out: &mut Vec<Friend>) {
        self.targets.friends(out)
    }
    fn friend_link(&mut self, req: FriendLinkRequest) {
        self.targets.friend_link(req)
    }
    // (DON-PROG) forwarded too (the party's step reads them through this wrapper): DON-MAGIC's sizes, Insight's elements
    fn size_class(&self, id: TargetId) -> u8 {
        self.targets.size_class(id)
    }
    fn element_affinity(&self, id: TargetId) -> Option<[i32; 8]> {
        self.targets.element_affinity(id)
    }
}

impl kh_partyhost::PartyTargets for ErPartyTargets<'_> {
    fn ai_actors(&self, out: &mut Vec<ActorView>) {
        for e in &self.targets.list {
            if !e.active {
                continue;
            }
            let center = e.pos + Vec3::Z * self.targets.aim_height(e);
            out.push(ActorView {
                actor: ActorRef::Enemy(e.id.0),
                uid: -1,
                position: e.pos,
                center,
                yaw: e.yaw,
                velocity: e.velocity,
                radius: e.radius,
                half_height: e.half_height,
                alive: e.alive,
                // (BM) passive wildlife that is not fighting the party is no enemy of the trees (and no party aggro):
                // lockable still, so the leader can pick it; a hit either way makes it an enemy (ErTargets::passive_now)
                hostile: e.hostile && !e.friendly && !self.targets.passive_now(e),
                lockable: e.hostile && !e.lock_disabled,
                airborne: e.airborne || !e.grounded,
                hp: e.hp as f32,
                hp_max: e.hp_max as f32,
                ..ActorView::default()
            });
        }
    }
    fn attacked_by_enemies(&self) -> bool {
        self.attacked
    }
    // (FX5) the hit / attack memory of the engaged rule: an enemy fighting the party stays a threat wherever it stands
    fn fighting_party(&self, id: u32) -> bool {
        self.targets.fought_lately(TargetId(id))
    }
}

/// (party AIX) The party's engagement is measured round the LEADER: with Donald leading 30 m from where Sora was left
/// (out of the party, not stepped) and an enemy 4 m from Donald, ErTargets' own count (round `targets.sora`) is 0 but
/// the party's is 1 (the battle flag of kh-partyhost `Leadership::battle_step`); with Sora leading both are the same.
#[cfg(test)]
mod aix_tests {
    use super::*;
    use crate::coords::Frame;
    use crate::ermap::{Params, Weight};
    use crate::targets::Enemy;

    #[test]
    fn aix_the_party_engages_round_the_leader() {
        let mut t = ErTargets::new(Frame::at(Vec3::ZERO), Params::default());
        t.list = vec![Enemy::stub(1, Vec3::new(3400.0, 0.0, 0.0), 40.0, 90.0, 4000, Weight::Small)];
        t.sora = Vec3::ZERO; // where Sora was left
        let donald = Vec3::new(3000.0, 0.0, 0.0);
        assert_eq!(t.engaged_count(), 0, "ErTargets measures round Sora");
        let mut latch = 0;
        let p = ErPartyTargets::new(&mut t, donald, &mut latch);
        assert_eq!(kh_core::Targets::engaged_count(&p), 1, "the party's count is round the leader");
        assert_eq!(p.targets.sora, Vec3::ZERO, "Sora's own point is left as it was");
        // Sora leading: the leader is Sora, the count is ErTargets' own
        let sora = t.sora;
        let p = ErPartyTargets::new(&mut t, sora, &mut latch);
        assert_eq!(kh_core::Targets::engaged_count(&p), 0);
    }
}

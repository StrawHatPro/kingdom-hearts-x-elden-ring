//! THE BODY SEAMS a game host (Elden Ring) builds on (contract ER.2 / ER.5; research ER_PARTY_RESEARCH.md 2.1):
//!
//! - [`LeaderView`]: the controlled pawn as the hidden player body must follow it (Sora's `Output` when he leads, the
//!   leading friend's `MemberView` otherwise), built every step by [`PartyRuntime::leader_view_of`];
//! - [`LeaderSink`]: what the body code sends back (a hit taken, the body leading the pawn in a cutscene / action,
//!   a death), implemented for the runtime by [`RuntimeLeaderSink`];
//! - [`AllyWant`]: the AI allies' bodies wanted this step (who, where, how big, hittable or not), the list a host's
//!   body pool syncs to ([`PartyRuntime::ally_wants`]).
//!
//! The test level has no game bodies: it uses none of these (they are pure functions of the runtime's state).

use kh_core::{Output, Vec3};
use kh_host::party::PartyChara;
use kh_party::{MemberView, NpcStateId};

use crate::aiworld::SORA_ACTOR;
use crate::runtime::{party_chara, PartyRuntime};

/// Sora's capsule as kh-core moves him (radius, half height, centre above the feet; cm). kh-party `party-params`
/// rows give the friends' (Donald 30 / 49 / 51, Goofy 40 / 89 / 91).
pub const SORA_CAPSULE: (f32, f32, f32) = (34.0, 89.0, 91.0);

/// The controlled pawn (contract ER.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeaderView {
    pub chara: PartyChara,
    /// Actor origin (feet), KH3 space (cm).
    pub position: Vec3,
    pub yaw: f32,
    pub velocity: Vec3,
    pub on_floor: bool,
    /// (radius, half height, centre above the feet), cm.
    pub capsule: (f32, f32, f32),
    /// Hits are refused now (Sora: `Status::damage_invalid`, the host adds its own dodge-frame rule; a friend: Die,
    /// Warp or a team-attack FR state).
    pub invulnerable: bool,
    /// A team attack owns the pawn (Sora's Fr* state / the friend's `Fr(_)`).
    pub in_fr: bool,
    /// Knocked out (a friend in Die) / dead (Sora at 0 HP).
    pub ko: bool,
    pub airborne: bool,
    /// Sora's `ETresStateID` (kh_core::StateId as i32) when he leads, 0 for a friend.
    pub state: i32,
    /// (party B5) the body pitch the drawing applies (a friend's slides), deg.
    pub pitch: f32,
    /// The member id when a friend leads (None = Sora).
    pub member: Option<kh_party::MemberId>,
}

/// A hit the leader's body took, already through the host's damage-in rule (share of the maximum, `last_hit_by`,
/// the status class).
#[derive(Clone, Copy, Debug)]
pub struct LeaderHit {
    /// For Sora: `kh_core::states::HitOnPlayer` (his own reaction and guard rules); for a friend the record below.
    pub sora: Option<kh_core::states::HitOnPlayer>,
    /// For a friend: the final record (contract B.3 `receive_hit`).
    pub record: kh_core::HitRecord,
}

/// What the leader's body sends back (contract ER.2). The host calls it from its body code.
pub trait LeaderSink {
    /// The body was hit (Sora: `Sora::take_hit`; a friend: `Party::receive_hit`).
    fn take_hit(&mut self, hit: &LeaderHit);
    /// The body leads (a cutscene, a ladder, a grab, a talk): the pawn is put where the body is.
    fn follow_body(&mut self, at: Vec3, yaw: f32);
    /// The body died (drowned, a scripted death): the leader goes down (game over follows the host's rule).
    fn die(&mut self);
}

/// [`LeaderSink`] over the runtime and Sora.
pub struct RuntimeLeaderSink<'a> {
    pub runtime: &'a mut PartyRuntime,
    pub sora: &'a mut kh_core::Sora,
}

impl LeaderSink for RuntimeLeaderSink<'_> {
    fn take_hit(&mut self, hit: &LeaderHit) {
        match self.runtime.leader {
            None => {
                if let Some(h) = hit.sora {
                    self.sora.take_hit(h);
                }
            }
            Some(id) => {
                self.runtime.party.receive_hit(id, &hit.record);
            }
        }
    }
    fn follow_body(&mut self, at: Vec3, yaw: f32) {
        match self.runtime.leader {
            None => self.sora.teleport(kh_core::Pose { position: at, yaw }),
            Some(id) => {
                if let Some(m) = self.runtime.party.member_mut(id) {
                    m.cmc.location = at;
                    m.cmc.stop_movement();
                    m.yaw = yaw;
                }
            }
        }
    }
    fn die(&mut self) {
        match self.runtime.leader {
            // kh-er's drown death (IMPL_ER57): a blow of 1e6 from above his head
            None => {
                let from = self.sora.output().position + Vec3::Z * 100.0;
                self.sora.take_hit(kh_core::states::HitOnPlayer { direction: Vec3::NEG_Z, ..kh_core::states::HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 1.0e6, kh_core::TargetId::NONE, from) });
            }
            Some(id) => {
                let hp = self.runtime.party.view(id).map_or(0, |v| v.hp);
                let record = kh_core::HitRecord { damage: hp as f32 + 1.0, attack_name: "leader.die", ..kh_core::HitRecord::empty(kh_core::TargetId::NONE) };
                self.runtime.party.receive_hit(id, &record);
            }
        }
    }
}

/// One AI ally's body wanted this step (contract ER.5): the host's pool spawns, moves, hides and parks bodies so that
/// its list matches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AllyWant {
    pub chara: PartyChara,
    /// Actor origin (feet), KH3 space (cm).
    pub feet: Vec3,
    pub yaw: f32,
    /// (radius, half height, centre above the feet), cm.
    pub capsule: (f32, f32, f32),
    /// The body should take hits (false: hidden from enemies, e.g. while KO or parked).
    pub hittable: bool,
    /// Hits are refused now (Die / Warp / FR states): the body stays but is invincible.
    pub invulnerable: bool,
    pub ko: bool,
}

fn friend_capsule(v: &MemberView) -> (f32, f32, f32) {
    let (r, h) = if v.capsule.0 > 0.0 { v.capsule } else { (34.0, 70.0) };
    // the friends' capsule centre is its half height + 2 cm above the feet (party-params: Donald 49 / 51, Goofy 89 / 91)
    (r, h, h + 2.0)
}

fn friend_refuses_hits(v: &MemberView) -> bool {
    // (GOOFY-HR) a lethal blow playing (Damage at 0 HP: kh-party hurt.rs) refuses hits as Die does
    matches!(v.state, NpcStateId::Die | NpcStateId::Warp | NpcStateId::Fr(_)) || (v.state == NpcStateId::Damage && v.hp < 1)
}

impl PartyRuntime {
    /// The controlled pawn now (Sora's `Output` / `Status` when he leads).
    pub fn leader_view_of(&self, sora: &Output, status: &kh_core::Status) -> LeaderView {
        if let Some(v) = self.leader.and_then(|l| self.current.iter().find(|v| v.id == l)) {
            return LeaderView {
                chara: party_chara(v.chara),
                position: v.position,
                yaw: v.yaw,
                velocity: v.velocity,
                on_floor: v.on_ground,
                capsule: friend_capsule(v),
                invulnerable: friend_refuses_hits(v) || self.party.don_zetta_invincible(v.id), // (ZETTA round 5) Donald through Zettaflare's sequence
                in_fr: matches!(v.state, NpcStateId::Fr(_)),
                ko: v.ko,
                airborne: !v.on_ground,
                state: 0,
                pitch: v.pitch,
                member: Some(v.id),
            };
        }
        LeaderView {
            chara: PartyChara::Sora,
            position: sora.position,
            yaw: sora.yaw,
            velocity: sora.velocity,
            on_floor: sora.on_floor,
            capsule: SORA_CAPSULE,
            invulnerable: status.damage_invalid,
            in_fr: format!("{:?}", sora.state).starts_with("Fr"),
            ko: status.hp < 1,
            airborne: !sora.on_floor,
            state: sora.state as i32,
            pitch: 0.0,
            member: None,
        }
    }

    /// The AI allies' bodies of this step: every friend but the leader, and AI Sora while a friend leads and Sora is
    /// in the party.
    pub fn ally_wants(&self, sora: &Output, status: &kh_core::Status) -> Vec<AllyWant> {
        let mut out = Vec::new();
        if self.leader.is_some() && self.lead.sora_active {
            let ko = status.hp < 1;
            out.push(AllyWant { chara: PartyChara::Sora, feet: sora.position, yaw: sora.yaw, capsule: SORA_CAPSULE, hittable: !ko, invulnerable: status.damage_invalid || format!("{:?}", sora.state).starts_with("Fr"), ko });
        }
        for v in &self.current {
            if Some(v.id) == self.leader {
                continue;
            }
            out.push(AllyWant { chara: party_chara(v.chara), feet: v.position, yaw: v.yaw, capsule: friend_capsule(v), hittable: !v.ko, invulnerable: friend_refuses_hits(v) || self.party.don_zetta_invincible(v.id) /* (ZETTA round 5) */, ko: v.ko });
        }
        out
    }

    /// The actor id an AI-world ally view gives AI Sora (for hosts that list him).
    pub const SORA_ACTOR: u8 = SORA_ACTOR;
}

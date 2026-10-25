//! (DON-PROG) THE PARTY AURAS (documentation/DONALD_OVERHAUL.md 4.2, the column "Party aura (when in a party)"): what
//! Donald's attuned element(s) give the party round him. PARTY ONLY: none while he is alone (the same "at least one other
//! party member" as the aggro rules, sheet `aggro.others_min`). With Dual Attunement both elements' auras hold.
//!
//! | Element | Aura (sheet `aura.*`) |
//! |---|---|
//! | Fire | the party's attack +10 % (`aura.fire_attack`) |
//! | Blizzard | enemies within the aura of Donald -15 % speed (`aura.blizzard_slow`) |
//! | Thunder | the party's movement +10 % (`aura.thunder_move`) |
//! | Water | the party's MP regeneration +25 %: the MP Charge runs that much faster (`aura.water_mp`) |
//! | Aero | the party's evasion (dodge cooldowns / 1.15, invincible windows x1.15) and recovery speed (hit reactions and getting up x1.2) (`aura.aero_*`) |
//! | Light | the party's Regen: 0.5 % of max HP a second (`aura.light_regen`) |
//!
//! The aura reaches the members within `aura.radius` (10 m) of him (Aura Range+ x`aura.range_plus`), Donald included.
//! [`PartyAura`] is the party's snapshot, made every step (`Party::don_prog`); the shared code applies Water and Light
//! to the friends there ([`PartyAura::apply_friends`]); the party host applies them to Sora and the Fire attack to every
//! hit of the party (kh-partyhost donald.rs); the movement, evasion, recovery and enemy-slow factors are read by the
//! code that owns those motions ([`PartyAura::at`], [`PartyAura::enemy_speed`]).

use glam::Vec3;

use super::abilities::DonAbility;
use super::ability_info::prog;
use super::element::{ElemSet, Element};
use super::DonaldState;
use crate::ids::MemberId;

/// One Donald's aura now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aura {
    /// The attuned element(s) giving it (empty: no aura).
    pub elements: ElemSet,
    /// Reach (cm) round Donald.
    pub radius: f32,
    /// Fire: the party's attack x this (1.10).
    pub attack_mult: f32,
    /// Blizzard: enemies within `radius` of Donald move x this (0.85).
    pub enemy_speed_mult: f32,
    /// Thunder: the party's movement x this (1.10).
    pub move_mult: f32,
    /// Water: the party's MP regeneration (MP Charge) x this (1.25).
    pub mp_regen_mult: f32,
    /// Aero: the party's evasion share (dodge cooldowns / (1 + this), invincible windows x (1 + this)).
    pub evasion: f32,
    /// Aero: the party's recovery speed (hit reactions / getting up x this).
    pub recovery_mult: f32,
    /// Light: Regen, % of max HP per second.
    pub regen_pct_per_s: f32,
}

impl Aura {
    pub const NONE: Aura = Aura { elements: ElemSet::NONE, radius: 0.0, attack_mult: 1.0, enemy_speed_mult: 1.0, move_mult: 1.0, mp_regen_mult: 1.0, evasion: 0.0, recovery_mult: 1.0, regen_pct_per_s: 0.0 };

    pub fn is_none(&self) -> bool {
        self.elements.is_empty()
    }
}

impl Default for Aura {
    fn default() -> Aura {
        Aura::NONE
    }
}

/// THE AURA his attunement gives now (whether a party is there is the caller's rule: [`PartyAura::make`]).
pub fn aura_of(d: &DonaldState) -> Aura {
    let elements = d.core.elements();
    if elements.is_empty() {
        return Aura::NONE;
    }
    let p = prog();
    let range = if d.abilities.has(DonAbility::AuraRangePlus) { p.value("aura.range_plus").max(1.0) } else { 1.0 };
    let mut a = Aura { elements, radius: p.value("aura.radius") * range, ..Aura::NONE };
    for e in elements.iter() {
        match e {
            Element::Fire => a.attack_mult = 1.0 + p.value("aura.fire_attack"),
            Element::Blizzard => a.enemy_speed_mult = (1.0 - p.value("aura.blizzard_slow")).clamp(0.0, 1.0),
            Element::Thunder => a.move_mult = 1.0 + p.value("aura.thunder_move"),
            Element::Water => a.mp_regen_mult = 1.0 + p.value("aura.water_mp"),
            Element::Aero => {
                a.evasion = p.value("aura.aero_evasion").max(0.0);
                a.recovery_mult = 1.0 + p.value("aura.aero_recovery");
            }
            Element::Light => a.regen_pct_per_s = p.value("aura.light_regen").max(0.0),
        }
    }
    a
}

/// What the auras give one member at a place (several Donalds: the strongest of each kind; none: the neutral values).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuraEffect {
    pub attack_mult: f32,
    pub move_mult: f32,
    pub mp_regen_mult: f32,
    pub evasion: f32,
    pub recovery_mult: f32,
    pub regen_pct_per_s: f32,
}

impl AuraEffect {
    pub const NEUTRAL: AuraEffect = AuraEffect { attack_mult: 1.0, move_mult: 1.0, mp_regen_mult: 1.0, evasion: 0.0, recovery_mult: 1.0, regen_pct_per_s: 0.0 };
}

impl Default for AuraEffect {
    fn default() -> AuraEffect {
        AuraEffect::NEUTRAL
    }
}

/// One Donald's aura and where he stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuraSource {
    pub member: MemberId,
    pub pos: Vec3,
    pub aura: Aura,
}

/// THE PARTY'S AURAS this step, and the Regen's fractions carried between steps.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PartyAura {
    pub sources: Vec<AuraSource>,
    /// Regen's HP not given yet (a fraction of a point), per member (Sora: [`super::cast::SORA_MEMBER`]).
    pub regen_acc: Vec<(MemberId, f32)>,
}

impl PartyAura {
    /// The snapshot: every Donald's aura, when the party has at least `aggro.others_min` other members besides him
    /// (`party_size`: every member present, Sora included when he is in the party).
    pub fn make(&mut self, donalds: &[(MemberId, Vec3, &DonaldState)], party_size: usize) {
        self.sources.clear();
        let others_min = prog().value("aggro.others_min").max(1.0) as usize;
        if party_size < others_min + 1 {
            return;
        }
        for (id, pos, d) in donalds {
            let aura = aura_of(d);
            if !aura.is_none() {
                self.sources.push(AuraSource { member: *id, pos: *pos, aura });
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// What the auras give a member standing at `pos`.
    pub fn at(&self, pos: Vec3) -> AuraEffect {
        let mut out = AuraEffect::NEUTRAL;
        for s in self.sources.iter().filter(|s| s.pos.distance(pos) <= s.aura.radius) {
            let a = s.aura;
            out.attack_mult = out.attack_mult.max(a.attack_mult);
            out.move_mult = out.move_mult.max(a.move_mult);
            out.mp_regen_mult = out.mp_regen_mult.max(a.mp_regen_mult);
            out.evasion = out.evasion.max(a.evasion);
            out.recovery_mult = out.recovery_mult.max(a.recovery_mult);
            out.regen_pct_per_s = out.regen_pct_per_s.max(a.regen_pct_per_s);
        }
        out
    }

    /// The speed factor of an enemy at `pos` (Blizzard's aura: x0.85 within reach of an attuned Donald; else 1).
    pub fn enemy_speed(&self, pos: Vec3) -> f32 {
        self.sources.iter().filter(|s| s.pos.distance(pos) <= s.aura.radius).map(|s| s.aura.enemy_speed_mult).fold(1.0, f32::min)
    }

    /// REGEN: the whole HP points due to `who` this step (`max_hp`, `pct` % a second, `dt`), the fraction carried.
    pub fn regen_points(&mut self, who: MemberId, max_hp: i32, pct: f32, dt: f32) -> i32 {
        let i = match self.regen_acc.iter().position(|r| r.0 == who) {
            Some(i) => i,
            None => {
                self.regen_acc.push((who, 0.0));
                self.regen_acc.len() - 1
            }
        };
        if pct <= 0.0 || max_hp <= 0 {
            self.regen_acc[i].1 = 0.0;
            return 0;
        }
        self.regen_acc[i].1 += max_hp as f32 * pct / 100.0 * dt.max(0.0);
        let whole = self.regen_acc[i].1.floor();
        self.regen_acc[i].1 -= whole;
        whole as i32
    }

    /// WATER AND LIGHT ON THE FRIENDS (the shared rule; the host does Sora's): the MP Charge runs faster by the Water
    /// aura's share, Regen heals a living member. `members`: (id, position, status) of every friend.
    pub fn apply_friends<'a>(&mut self, members: impl Iterator<Item = (MemberId, Vec3, &'a mut crate::status::FriendStatus)>, dt: f32) {
        if self.sources.is_empty() {
            self.regen_acc.clear();
            return;
        }
        for (id, pos, s) in members {
            let fx = self.at(pos);
            if s.mp_charge_left > 0.0 && fx.mp_regen_mult > 1.0 {
                s.mp_charge_left -= dt * (fx.mp_regen_mult - 1.0) * s.don_mp_haste();
                if s.mp_charge_left <= 0.0 {
                    s.mp_charge_left = 0.0;
                    s.mp = s.max_mp();
                }
            }
            if s.hp > 0 && s.hp < s.max_hp() {
                let n = self.regen_points(id, s.max_hp(), fx.regen_pct_per_s, dt);
                if n > 0 {
                    s.hp = (s.hp + n).min(s.max_hp());
                }
            }
        }
    }
}

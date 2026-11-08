//! (COMBAT-FIX, THE USER 2026-11-06: "Rennala can currently be hit in her shield bubble during phase 1, this is not correct she
//! should only be targetable when her shield has broken") ELDEN RING'S OWN "CANNOT BE DAMAGED" STATE OF A CHARACTER, read the
//! way the game's damage code reads it, for every character (no per-enemy table):
//!
//! - the character's invincible flag (`ChrIns` +0x1C5 bit 4, `ChrInsFlags1c5::is_invincible`; the flag the mod itself sets on the
//!   Tarnished and the party's bodies to make them take nothing, sora.rs / allybody_live.rs);
//! - its no-hit flag (`ChrIns::debug_flags` bit 3, `ChrDebugFlags::disabled_hit`: "ignore all incoming damage"; same use);
//! - ITS ACTIVE SPEFFECTS' DAMAGE RATES FOR A PLAYER'S HIT (the party's hits are the player's bullets): Elden Ring multiplies a
//!   hit's damage of each type by every unconditional SpEffect's `xxxDamageCutRate` of that type (slash / blow / thrust /
//!   neutral / magic / fire / thunder / dark; 1 = no change, 0.1 for 180 s is the 90 % negation row 3507) and by its
//!   `defPlayerDmgCorrectRate` of that family (physics for the four physical types, magic, fire, thunder, dark); when the
//!   product is 0 for EVERY type the body takes nothing from the player. A conditional effect (`conditionHp` /
//!   `conditionHpRate` set) is not counted (none of the 16 rows that close every type is conditional, 2.7.1.0 / 2.7.2.0).
//! - (COMBAT-FIX follow-up, OFFLINE PROOF on the game's data) RENNALA'S BARRIER: her phase-1 body (event entity 14000801,
//!   model c2030) wears SpEffect 14361 -> 14368 -> 14369, one per ring (her map's events m14_00_00_00 14003825 / 14003805 as
//!   the glowing students fall; every rate 0, state info 143, vfx 52351 / 52355 / 52356; category 100 priority 16, so each
//!   replaces the last). At the shatter event 14003805 puts 14578 on her (the same category and priority: every rate still
//!   0, the shatter vfx 52358, state info 0, the status rates back to 1) and asks her AI to replan: her fall animation
//!   a000_003010 (c2030.tae) puts 14579 on her at 5.667 s (category 100 priority 16, every rate 1, endurance 0), which
//!   REPLACES 14578: from then on nothing closes any type and the Tarnished's hits land, until her barrier comes back
//!   (a000_003011's 14387 -> `replaceSpEffectId` 14361, or event 14003825). While 14578 is on (the shatter and her fall)
//!   Elden Ring's player takes nothing off her either, so the rule holds her shut for exactly that time;
//! - its animation's invincibility frames (TAE ChrActionFlag 94 PERFECT_INVINCIBILITY / 67 INVINCIBLE_EXCLUDING_THROW_ATTACKS_
//!   DEFENDER, `ChrActionModifiersFlags`): a dodge, a roll, a transition animation.
//!
//! Not invulnerability: the no-DEATH states (`zdefer::no_dead_of`: it takes damage down to 1 HP), super armour, a raised guard.
//!
//! THE RULE: a body ER holds invulnerable ([`State::Shut`]: one of the first three) is neither hittable nor lockable for the KH
//! party (no hit of any route lands: melee, magic, shotlocks, Zettaflare, a Breaker; no lock-on, soft target or AI pick); a body
//! in its animation's invincibility frames ([`State::Frames`]) is not hittable (attacks pass through it as ER's do) but stays
//! locked; once ER lets it be damaged again it is both, as in ER. No HP of ours is ever written past it (targets.rs).

#[cfg(windows)]
use eldenring::cs::ChrIns;

/// Elden Ring's word on whether a character can be damaged now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// It can be damaged.
    #[default]
    Open,
    /// Its animation's invincibility frames: hits pass through, it stays locked.
    Frames,
    /// The game holds it invulnerable: no hit, no lock.
    Shut(Why),
}

/// Why the game holds it invulnerable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// `ChrInsFlags1c5::is_invincible`.
    InvincibleFlag,
    /// `ChrDebugFlags::disabled_hit`.
    NoHitFlag,
    /// The SpEffects on it cut every damage type to 0 (the id of the first one that cuts any type to 0).
    SpEffect(i32),
    /// (MIMIC-BOSS, THE USER 2026-11-07: "also mimic tear boss should not be targetable or attackable until it has transformed
    /// into Sora") A Mimic Tear boss (or its summoned Mimic) whose copy of the leader has not formed yet (mimic_boss.rs
    /// `forming`, kh-partyhost `boss_untouchable` / `summon_untouchable`).
    Forming,
}

impl Why {
    pub fn text(self) -> String {
        match self {
            Why::InvincibleFlag => "its invincible flag (ChrIns +0x1C5 bit 4)".to_string(),
            Why::NoHitFlag => "its no-hit flag (ChrIns debug flags bit 3)".to_string(),
            Why::SpEffect(id) => format!("its SpEffects close every damage type to a player's hit (cut rate x defPlayerDmgCorrectRate = 0; SpEffect {id} ...)"),
            Why::Forming => "the Mimic Tear has not transformed into the leader yet (MIMIC-BOSS: its copy is still forming)".to_string(),
        }
    }
}

impl State {
    /// No damage of ours may reach it now.
    pub fn refuses_damage(self) -> bool {
        self != State::Open
    }

    /// Not to be locked on to / picked now.
    pub fn refuses_lock(self) -> bool {
        matches!(self, State::Shut(_))
    }

    /// The core's flags of a hostile with this state: HITTABLE off unless open, LOCKABLE off while shut.
    pub fn mask(self, flags: u32) -> u32 {
        use kh_core::host::TargetFlags;
        let mut f = flags;
        if self.refuses_damage() {
            f &= !TargetFlags::HITTABLE;
        }
        if self.refuses_lock() {
            f &= !TargetFlags::LOCKABLE;
        }
        f
    }
}

/// The damage types of the game's cut rates: slash, blow, thrust, neutral (standard), magic, fire, thunder, dark (holy).
pub const CUT_TYPES: usize = 8;

/// What is read off one character (pure, for the rule and its tests).
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub invincible: bool,
    pub no_hit: bool,
    pub frames: bool,
    /// Each active SpEffect on it.
    pub effects: Vec<Effect>,
}

/// (COMBAT-FIX follow-up) The families of `defPlayerDmgCorrectRate`: physics, magic, fire, thunder, dark.
pub const CORRECT_FAMILIES: usize = 5;

/// Which family each cut-rate type takes its player correction from (slash / blow / thrust / neutral: physics).
const FAMILY_OF: [usize; CUT_TYPES] = [0, 0, 0, 0, 1, 2, 3, 4];

/// One active SpEffect as its SpEffectParam row has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effect {
    pub id: i32,
    /// slash, blow, thrust, neutral, magic, fire, thunder, dark DamageCutRate.
    pub cuts: [f32; CUT_TYPES],
    /// defPlayerDmgCorrectRate physics, magic, fire, thunder, dark (a player's hit on this body).
    pub player: [f32; CORRECT_FAMILIES],
    /// `conditionHp` / `conditionHpRate` set (>= 0): it acts only under an HP condition; not counted.
    pub conditional: bool,
}

impl Effect {
    /// An unconditional effect with these cut rates and no player correction (the tests' rows).
    #[cfg(test)]
    pub const fn cuts(id: i32, cuts: [f32; CUT_TYPES]) -> Effect {
        Effect { id, cuts, player: [1.0; CORRECT_FAMILIES], conditional: false }
    }

    /// Its multiplier on a player's hit of cut-rate type `t` (a broken row's NaN counts as "no change").
    fn rate(&self, t: usize) -> f32 {
        let ok = |v: f32| if v.is_finite() { v.max(0.0) } else { 1.0 };
        ok(self.cuts[t]) * ok(self.player[FAMILY_OF[t]])
    }
}

/// THE RULE on what was read.
pub fn state_of(f: &Facts) -> State {
    if f.invincible {
        return State::Shut(Why::InvincibleFlag);
    }
    if f.no_hit {
        return State::Shut(Why::NoHitFlag);
    }
    if let Some(id) = immune_by_cuts(&f.effects) {
        return State::Shut(Why::SpEffect(id));
    }
    if f.frames {
        return State::Frames;
    }
    State::Open
}

/// The game's product, per damage type, of what the unconditional active SpEffects let through of a PLAYER's hit (cut rate x
/// the player correction of its family): 0 for every type -> the id of the first SpEffect closing some type. None: some type
/// still deals damage (or no SpEffect at all).
pub fn immune_by_cuts(effects: &[Effect]) -> Option<i32> {
    let counted = || effects.iter().filter(|e| !e.conditional);
    let mut product = [1.0f32; CUT_TYPES];
    for e in counted() {
        for (t, p) in product.iter_mut().enumerate() {
            *p *= e.rate(t);
        }
    }
    if product.iter().any(|p| *p > 1e-6) {
        return None;
    }
    counted().find(|e| (0..CUT_TYPES).any(|t| e.rate(t) <= 1e-6)).map(|e| e.id)
}

/// The SpEffects a body may carry that are read (the list is the game's linked list; a longer one is cut there).
#[cfg(windows)]
const MAX_EFFECTS: usize = 128;

/// THE READ on a live character (`chr` checked complete by the caller, `game::chr_complete`).
#[cfg(windows)]
pub fn read(chr: &ChrIns) -> State {
    use crate::game::plausible;
    let mut f = Facts { invincible: chr.chr_flags1c5.is_invincible(), no_hit: chr.debug_flags.disabled_hit(), ..Facts::default() };
    if plausible(crate::word!(chr.modules.action_flag)) {
        let a = &chr.modules.action_flag.action_modifiers_flags;
        f.frames = a.perfect_invincibility() || a.invincible_excluding_throw_attacks_defender();
    }
    if plausible(crate::word!(chr.special_effect)) {
        for e in chr.special_effect.entries().take(MAX_EFFECTS) {
            let Some(row) = e.param_data else { continue };
            if !plausible(row.as_ptr() as usize) {
                continue;
            }
            // SAFETY: the game's own pointer from a live SpEffect entry into its regulation's SpEffectParam rows (checked plausible).
            let r = unsafe { row.as_ref() };
            f.effects.push(Effect {
                id: e.param_id,
                cuts: [r.slash_damage_cut_rate(), r.blow_damage_cut_rate(), r.thrust_damage_cut_rate(), r.neutral_damage_cut_rate(), r.magic_damage_cut_rate(), r.fire_damage_cut_rate(), r.thunder_damage_cut_rate(), r.dark_damage_cut_rate()],
                player: [r.def_player_dmg_correct_rate_physics(), r.def_player_dmg_correct_rate_magic(), r.def_player_dmg_correct_rate_fire(), r.def_player_dmg_correct_rate_thunder(), r.def_player_dmg_correct_rate_dark()],
                conditional: r.condition_hp() >= 0.0 || r.condition_hp_rate() >= 0.0,
            });
        }
    }
    state_of(&f)
}

/// (COMBAT-FIX follow-up) Rennala's rows as SpEffectParam has them in 2.7.1.0 and 2.7.2.0 (the cut rates; every
/// defPlayerDmgCorrectRate is 1, no HP condition): the barrier rings, the shatter, the TAE's lift, the markers.
#[cfg(test)]
pub(crate) mod rennala {
    use super::{Effect, CUT_TYPES};
    const NEUTRAL: [f32; CUT_TYPES] = [1.0; CUT_TYPES];
    const BARRIER: [f32; CUT_TYPES] = [0.0; CUT_TYPES];
    /// The rings 14361 / 14368 / 14369 (state info 143) and the shatter 14578 (state info 0): every type closed.
    pub const RING_1: Effect = Effect::cuts(14361, BARRIER);
    pub const RING_2: Effect = Effect::cuts(14368, BARRIER);
    pub const RING_3: Effect = Effect::cuts(14369, BARRIER);
    pub const SHATTER: Effect = Effect::cuts(14578, BARRIER);
    /// The markers and the fall animation a000_003010's effects: nothing closed.
    pub const UP: Effect = Effect::cuts(14356, NEUTRAL);
    pub const BROKEN: Effect = Effect::cuts(14357, NEUTRAL);
    pub const RECOVERING: Effect = Effect::cuts(14358, NEUTRAL);
    pub const FALL: Effect = Effect::cuts(14359, NEUTRAL);

    /// Her live SpEffects in the phases of her barrier (category 100 / priority 16: a ring, the shatter and the lift
    /// 14579 replace one another; 14579 lasts 0 s, so after it nothing of category 100 is left on her).
    pub fn barrier(ring: Effect) -> Vec<Effect> {
        vec![UP, ring]
    }
    /// The shatter and the first 5.667 s of her fall (event 14003805: 14357 + 14578; a000_003010: 14358 / 14359 from 3.767 s).
    pub fn falling() -> Vec<Effect> {
        vec![BROKEN, SHATTER, RECOVERING, FALL]
    }
    /// From 5.667 s on (14579 has replaced 14578 and ended): the window to hit her.
    pub fn open() -> Vec<Effect> {
        vec![BROKEN, RECOVERING]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::host::TargetFlags;
    // (`rennala` is in scope through `super::*`)

    const NEUTRAL: [f32; CUT_TYPES] = [1.0; CUT_TYPES];
    const BARRIER: [f32; CUT_TYPES] = [0.0; CUT_TYPES];

    #[test]
    fn rennalas_barrier_rows_shut_her_and_the_break_opens_her() {
        for ring in [rennala::RING_1, rennala::RING_2, rennala::RING_3] {
            let up = Facts { effects: rennala::barrier(ring), ..Facts::default() };
            assert_eq!(state_of(&up), State::Shut(Why::SpEffect(ring.id)));
        }
        // the shatter and her fall: Elden Ring's player takes nothing off her either (14578 closes every type)
        assert_eq!(state_of(&Facts { effects: rennala::falling(), ..Facts::default() }), State::Shut(Why::SpEffect(14578)));
        // after 14579 replaced 14578 at 5.667 s of a000_003010: open, as for the Tarnished
        assert_eq!(state_of(&Facts { effects: rennala::open(), ..Facts::default() }), State::Open);
    }

    #[test]
    fn a_partial_cut_is_no_invulnerability() {
        // physical immune, magic not: damage still goes through
        let mut c = [0.0; CUT_TYPES];
        c[4] = 0.25;
        assert_eq!(immune_by_cuts(&[Effect::cuts(1, c)]), None);
        // the 90 % negation row (3507: 0.1 for every type): damage still goes through
        assert_eq!(immune_by_cuts(&[Effect::cuts(3507, [0.1; CUT_TYPES])]), None);
        // two effects that together close every type: immune by the game's product
        let mut a = [1.0; CUT_TYPES];
        a[..4].copy_from_slice(&[0.0; 4]);
        let mut b = [1.0; CUT_TYPES];
        b[4..].copy_from_slice(&[0.0; 4]);
        assert_eq!(immune_by_cuts(&[Effect::cuts(7, a), Effect::cuts(8, b)]), Some(7));
        assert_eq!(immune_by_cuts(&[]), None);
        assert_eq!(immune_by_cuts(&[Effect::cuts(9, [f32::NAN; CUT_TYPES])]), None, "an unreadable row changes nothing");
    }

    #[test]
    fn the_player_correction_and_the_hp_condition() {
        // closed for a player's hit by defPlayerDmgCorrectRate alone (the DLC's row 20011130: cut rates 1, corrections 0)
        let by_player = Effect { id: 20011130, cuts: NEUTRAL, player: [0.0; CORRECT_FAMILIES], conditional: false };
        assert_eq!(immune_by_cuts(&[by_player]), Some(20011130));
        // a player correction that leaves magic open: not immune
        let mut player = [0.0; CORRECT_FAMILIES];
        player[1] = 1.0;
        assert_eq!(immune_by_cuts(&[Effect { player, ..by_player }]), None);
        // an effect acting only under an HP condition is not counted
        assert_eq!(immune_by_cuts(&[Effect { conditional: true, ..Effect::cuts(5, BARRIER) }]), None);
    }

    #[test]
    fn the_flags_and_the_frames() {
        assert_eq!(state_of(&Facts { invincible: true, ..Facts::default() }), State::Shut(Why::InvincibleFlag));
        assert_eq!(state_of(&Facts { no_hit: true, ..Facts::default() }), State::Shut(Why::NoHitFlag));
        assert_eq!(state_of(&Facts { frames: true, ..Facts::default() }), State::Frames);
        assert_eq!(state_of(&Facts::default()), State::Open);
    }

    #[test]
    fn the_masks() {
        let all = TargetFlags::ENEMY | TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE;
        assert_eq!(State::Open.mask(all), all);
        assert_eq!(State::Frames.mask(all), all & !TargetFlags::HITTABLE, "frames: passed through, still locked");
        assert_eq!(State::Shut(Why::NoHitFlag).mask(all), TargetFlags::ENEMY | TargetFlags::ALIVE, "shut: an enemy in the fight, no target");
    }
}

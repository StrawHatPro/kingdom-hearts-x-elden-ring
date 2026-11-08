//! (REVENGE, 2026-11-07) KH3's revenge for the Mimic Tear bosses' copies (the boss's own copy and the Mimic Tear the boss
//! summons; never the player's Mimic). Pure choices and bookkeeping, no game access; mimic_boss.rs drives it
//! (build/sora_tmp/revenge/STATE.md section 6).
//!
//! THE USER (2026-11-06): "Mimic Sora should counter with Sonic Blade and zantetsuken, unless he's in a form change where
//! the keyblade has been turned into a completely different weapon than he does an appropriate move from the form's
//! moveset"; Donald's copy goes into his next spell, Goofy's into his charge / bash; and (2026-11-07) "make the mimic tear
//! boss stagger whenever Sora would stagger (but stay true to the revenge system we implemented as well)": a copy takes every
//! hit by KH3's PLAYER rules (its own core's or pawn's: reaction power against its state, its super-armour states and
//! abilities as copied, its guard and parry windows), not by the boss rule of bossrevenge.rs rule 4; only its revenge armour
//! refuses a reaction.
//!
//! 1. THE METER: the same limit as an Elden Ring boss, from the Mimic body's own data (bossrevenge.rs `limit`; its
//!    NpcParam 526100965 / 526100052 has stance 100, its HP is the copied player's x 3), and KH3's own meter: a kh-react
//!    `EnemyReaction` whose body never reacts (its revenge value only: every hit adds its revengepoint, held 3 s, 5 a
//!    second off, full after 20 s; nothing earned during the cool-down = `react.revenge.armour`).
//! 2. ON FULL: recover (an Aerial Recovery first when airborne: the Sora copy has KH3's, lead.rs `air_recovery`), then the
//!    counter below with the armour for `react.revenge.armour` seconds.
//! 3. THE COUNTER of a Sora copy by its form ([`sora_counter`]): Sonic Blade then Zantetsuken while the Keyblade stays a
//!    blade; in a form whose weapon is something else, that form's own lunge or finisher.

use kh_react::{Body, BodyClass, EnemyReaction};

use crate::bossrevenge::BossRules;

/// One move of KH3's data: the port's attack kind, its clip and its first attack row (p_ex001_AttackData; `shot:` = the
/// projectile that carries the hit), as sheets/port/sora-form-*.json has them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub kind: u8,
    pub clip: &'static str,
    pub row: &'static str,
}

/// SONIC BLADE: KH3's `LM_SONICRAVE` (command 0xAC, Second Form's finish level 2; move name "Sonic Blade" in
/// sora-keyblade-forms.json). Its clips M_1424 (start) -> M_1425 (dash, rows LM_SONICRAVE 1.2 SMALL) -> M_1426 (finish,
/// LM_SONICRAVE_END 1.5 TOSS). Outside Second Form the port plays the dash and the finish as the XS kinds 0xE0 / 0xE1
/// (kh-core abilities/xs/sonic.rs). Clips in build/sora_pack/anims (1424 - 1426.glb).
pub const SONIC_BLADE_DASH: Move = Move { kind: 0xE0, clip: "M_1425", row: "LM_SONICRAVE" };
pub const SONIC_BLADE_FINISH: Move = Move { kind: 0xE1, clip: "M_1426", row: "LM_SONICRAVE_END" };
/// ZANTETSUKEN: KH3's `TEXT_ACTION_FM0100`, Light / Dark Form's ground finisher 0xCD; clip M_M411, row SCLF_ZANTETSUKEN
/// (1.5 SMALL, revengepoint 0.25). Outside those forms the port's XS kind 0xE4 (kh-core abilities/xs/finishers.rs).
pub const ZANTETSUKEN: Move = Move { kind: 0xE4, clip: "M_M411", row: "SCLF_ZANTETSUKEN" };
/// The blade counter: Sonic Blade (its dash, its finish), then Zantetsuken.
pub const BLADE: [Move; 3] = [SONIC_BLADE_DASH, SONIC_BLADE_FINISH, ZANTETSUKEN];

/// The formchange commands (`Form::cmd`, sora-keyblade-forms.json `cmd`).
pub mod form {
    pub const DOUBLE_ARROWGUNS: i32 = 0x80;
    pub const HYPER_HAMMER: i32 = 0x81;
    pub const TWIN_YOYOS: i32 = 0x82;
    pub const MAGIC_LAUNCHER: i32 = 0x83;
    pub const DRILL_PUNCH: i32 = 0x84;
    pub const BLIZZARD_BLADES: i32 = 0x85;
    pub const NANO_ARMS: i32 = 0x86;
    pub const ULTIMATE: i32 = 0x87;
    pub const MIRAGE_STAFF: i32 = 0x88;
    pub const COUNTER_SHIELD: i32 = 0x89;
    pub const AGILE_CLAWS: i32 = 0x8A;
    pub const RAGE: i32 = 0x8B;
    pub const HIGHWIND: i32 = 0x8C;
    pub const STORM_FLAG: i32 = 0x8D;
    pub const HUNNY_BLASTERS: i32 = 0x8E;
    pub const HUNNY_LAUNCHER: i32 = 0x8F;
    pub const BLIZZARD_CLAWS: i32 = 0x90;
    pub const FRYING_PAN: i32 = 0x91;
    pub const BOOM_HAMMER: i32 = 0x92;
    pub const CLOCK_DRILL: i32 = 0x93;
    pub const SECOND_L1: i32 = 0x94;
    pub const SECOND_L2: i32 = 0x95;
    pub const SECOND_L5: i32 = 0x98;
    pub const LIGHT: i32 = 0xDD;
    pub const DARK: i32 = 0xDE;
    pub const DOUBLE: i32 = 0xDF;
    pub const DOUBLE_2: i32 = 0xE0;
}

/// What a Sora copy counters with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoraCounter {
    /// The Keyblade is still a blade: Sonic Blade (its dash, then its finish), then Zantetsuken.
    Blade([Move; 3]),
    /// The form's weapon is something else: that form's own move.
    Form(Move),
}

/// THE COUNTER of a Sora copy in form `cmd` (0 = no form).
pub fn sora_counter(cmd: i32) -> SoraCounter {
    use form::*;
    let own = |kind, clip, row| SoraCounter::Form(Move { kind, clip, row });
    match cmd {
        // a shield (Hero's Origin) / a frying pan (Grand Chef): Shield Charge
        COUNTER_SHIELD | FRYING_PAN => own(0x52, "M_B413", "SCSH_RAID"),
        // guns: the Arrowguns' finisher
        DOUBLE_ARROWGUNS | HUNNY_BLASTERS => own(0x47, "M_2414", "shot:DW_SPIRAL"),
        // a launcher: its near finisher
        MAGIC_LAUNCHER | HUNNY_LAUNCHER => own(0x4C, "M_5413", "SCBZ_LAUNCH"),
        // a hammer: Giant Stamp
        HYPER_HAMMER | BOOM_HAMMER => own(0x5D, "M_3416", "SCHM_GIANTSTAMP"),
        // a drill: Giga Upper
        DRILL_PUNCH | CLOCK_DRILL => own(0x6A, "M_6419", "SCDR_GIGAUPPER"),
        // a staff: its breaking blow
        MIRAGE_STAFF => own(0x83, "M_A415", "SCWA_BREAK"),
        // claws: the long stab dash
        AGILE_CLAWS | BLIZZARD_CLAWS => own(0x89, "M_C412", "SCCL_STAB"),
        // yo-yos: Walk the Dog
        TWIN_YOYOS => own(0x9A, "M_4418", "SCYO_WALKTHEDOG"),
        // twin ice blades (not a Keyblade): their rush
        BLIZZARD_BLADES => own(0xAB, "M_7412", "SCSK_RUSH"),
        // the shape-shifting Nano Gear: Nano Crash
        NANO_ARMS => own(0xB5, "M_8411", "SCGM_CRASH"),
        // a spear: Spear Rush; a flag: its launch
        HIGHWIND => own(0x9F, "M_E414", "SCHL_THRUST"),
        STORM_FLAG => own(0xA7, "M_F417", "SCFL_LAUNCH"),
        // Second Form, Light / Dark / Double Form, Ultimate Form, Rage Form (it keeps the held Keyblade): the Keyblade stays a blade
        SECOND_L1 | SECOND_L2 | SECOND_L5 | LIGHT | DARK | DOUBLE | DOUBLE_2 | ULTIMATE | RAGE => SoraCounter::Blade(BLADE),
        // no form (0), or a command this table does not know: the Keyblade
        _ => SoraCounter::Blade(BLADE),
    }
}

/// How far Goofy Bash reaches (cm, centre to centre): sheets/port/party-kit-goofy.json `art.bash.forward` 80 + its hit sphere
/// `art.bash.radius` 150.
pub const GOOFY_BASH_REACH: f32 = 230.0;

/// What a Goofy copy goes into: Goofy Bash (art 0xCC) on a target within `reach`, else Goofy Charge (art 0xCA).
pub fn goofy_counter(distance_cm: f32, reach_cm: f32) -> u8 {
    if distance_cm <= reach_cm { 0xCC } else { 0xCA }
}

/// The counter a Sora copy in form `cmd` asks of its core (`Sora::request_counter`): (the kinds, whether they are the form's
/// own).
pub fn sora_plan(cmd: i32) -> (Vec<u8>, bool) {
    match sora_counter(cmd) {
        SoraCounter::Blade(moves) => (moves.iter().map(|m| m.kind).collect(), false),
        SoraCounter::Form(m) => (vec![m.kind], true),
    }
}

/// What may happen to a copy hit now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyGate {
    /// Its own KH3 player rules decide its reaction (whenever Sora would flinch, stagger or be knocked back, it does).
    React,
    /// In its revenge armour: no reaction, no points.
    Revenge,
}

/// ONE COPY'S REVENGE: its meter and its armour.
#[derive(Clone, Debug)]
pub struct CopyRevenge {
    meter: EnemyReaction,
    rules: BossRules,
    armour: f32,
    armour_left: f32,
    /// Revenges so far, and one whose counter is still to be asked (the copy busy in a reaction).
    pub revenges: u32,
    pub due: bool,
}

impl Default for CopyRevenge {
    fn default() -> Self {
        CopyRevenge { meter: meter(&BossRules::DEFAULT, 0.0, 0.0, 0.0), rules: BossRules::DEFAULT, armour: 0.0, armour_left: 0.0, revenges: 0, due: false }
    }
}

impl CopyRevenge {
    /// The rules of this frame and the boss body's numbers: its limit and the armour's length. The limit is the FIXED
    /// `react.revenge.mimic_limit` (the user 2026-11-07: "make it 6.5"); with that row at 0, bossrevenge.rs rule 2's formula.
    pub fn sync(&mut self, rules: &BossRules, armour: f32, stance: f32, hp_max: f32) {
        self.rules = *rules;
        self.armour = armour;
        self.meter.body.revenge_limit = match (rules.on, rules.mimic_limit > 0.0) {
            (false, _) => 0.0,
            (true, true) => rules.mimic_limit,
            (true, false) => rules.limit(stance, hp_max),
        };
        self.meter.body.revenge_cooldown = armour;
    }

    /// The limit now.
    pub fn limit(&self) -> f32 {
        self.meter.body.revenge_limit
    }

    /// The revenge value now (the tests' view).
    #[cfg(test)]
    pub fn value(&self) -> f32 {
        self.meter.revenge_value()
    }

    /// The copy is unflinchable (its revenge armour runs).
    pub fn armoured(&self) -> bool {
        self.armour_left > 0.0
    }

    /// One step of its armour's clock.
    pub fn watch(&mut self, dt: f32) {
        self.armour_left = (self.armour_left - dt).max(0.0);
    }

    /// A PARTY HIT ON THE COPY (`hp_lost`: it took the body's HP): its revenge points, and whether its KH3 reaction may play.
    pub fn hit(&mut self, hit: &kh_core::HitRecord, hp_lost: bool) -> CopyGate {
        if !self.rules.on {
            return CopyGate::React;
        }
        if self.armoured() {
            return CopyGate::Revenge;
        }
        let request = kh_react::ReactionRequest::from_hit(hit, hp_lost, false);
        let _ = self.meter.hit(&request, kh_react::Pose { position: glam::Vec3::ZERO, yaw: 0.0, grounded: true });
        CopyGate::React
    }

    /// The meter's step (KH3's hold and decay). True: it crossed the limit now: the armour starts, the meter is reset and
    /// earns nothing during it, and the counter is due ([`CopyRevenge::due`]).
    pub fn step(&mut self, dt: f32) -> bool {
        let _ = self.meter.step_dt(&kh_react::FlatWorld { floor_z: 0.0, walls: Vec::new() }, dt);
        if !self.meter.take_revenge() {
            return false;
        }
        self.meter.clear_revenge();
        self.armour_left = self.armour;
        self.revenges += 1;
        self.due = true;
        true
    }
}

/// THE METER of a copy: kh-react's own revenge value on a body that never reacts, at the boss's limit.
pub fn meter(rules: &BossRules, stance: f32, hp_max: f32, armour: f32) -> EnemyReaction {
    let mut body = Body::of(BodyClass::Medium);
    body.never_reacts = true;
    body.revenge_limit = rules.limit(stance, hp_max);
    body.revenge_cooldown = armour;
    EnemyReaction::new(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_react::{HitOutcome, ReactionRequest};

    fn hit(m: &mut EnemyReaction, point: f32) -> HitOutcome {
        let mut q = ReactionRequest::new(kh_react::ETresDamageKind::Small, glam::Vec3::X);
        q.revenge_point = point;
        m.hit(&q, kh_react::Pose { position: glam::Vec3::ZERO, yaw: 0.0, grounded: true })
    }

    fn step(m: &mut EnemyReaction, seconds: f32) {
        for _ in 0..(seconds * 60.0).round() as u32 {
            m.step_dt(&kh_react::FlatWorld { floor_z: 0.0, walls: Vec::new() }, 1.0 / 60.0);
        }
    }

    #[test]
    fn the_counter_by_form() {
        use form::*;
        let blade = SoraCounter::Blade([SONIC_BLADE_DASH, SONIC_BLADE_FINISH, ZANTETSUKEN]);
        for cmd in [0, SECOND_L1, SECOND_L2, SECOND_L5, LIGHT, DARK, DOUBLE, DOUBLE_2, ULTIMATE, RAGE] {
            assert_eq!(sora_counter(cmd), blade, "{cmd:#x}");
        }
        let other = [COUNTER_SHIELD, FRYING_PAN, DOUBLE_ARROWGUNS, HUNNY_BLASTERS, MAGIC_LAUNCHER, HUNNY_LAUNCHER, HYPER_HAMMER, BOOM_HAMMER, DRILL_PUNCH, CLOCK_DRILL, MIRAGE_STAFF, AGILE_CLAWS, BLIZZARD_CLAWS, TWIN_YOYOS, BLIZZARD_BLADES, NANO_ARMS, HIGHWIND, STORM_FLAG];
        for cmd in other {
            assert!(matches!(sora_counter(cmd), SoraCounter::Form(_)), "{cmd:#x}");
        }
        assert_eq!(sora_counter(HYPER_HAMMER), SoraCounter::Form(Move { kind: 0x5D, clip: "M_3416", row: "SCHM_GIANTSTAMP" }));
        assert_eq!(sora_counter(CLOCK_DRILL), SoraCounter::Form(Move { kind: 0x6A, clip: "M_6419", row: "SCDR_GIGAUPPER" }));
        assert_eq!(sora_counter(FRYING_PAN), sora_counter(COUNTER_SHIELD));
        // Sonic Blade comes first, its finish second, Zantetsuken last
        let SoraCounter::Blade(moves) = sora_counter(0) else { panic!() };
        assert_eq!(moves.map(|m| m.kind), [0xE0, 0xE1, 0xE4]);
    }

    #[test]
    fn goofy_bashes_near_and_charges_far() {
        assert_eq!(goofy_counter(150.0, 250.0), 0xCC);
        assert_eq!(goofy_counter(600.0, 250.0), 0xCA);
    }

    #[test]
    fn a_copys_meter_is_kh3s_at_the_bosss_limit() {
        let rules = BossRules::DEFAULT;
        // a level-80 player (1900 HP) copied: 5700 HP, stance 100
        let mut m = meter(&rules, 100.0, 5700.0, 4.0);
        assert!((m.body.revenge_limit - 7.44).abs() < 0.01, "{}", m.body.revenge_limit);
        for _ in 0..7 {
            assert_eq!(hit(&mut m, 1.0), HitOutcome::NoReaction, "the meter body never reacts");
            step(&mut m, 0.3);
        }
        assert!(!m.take_revenge());
        hit(&mut m, 1.0);
        assert!(m.take_revenge(), "the 8th basic hit fills 7.44");
        m.clear_revenge();
        hit(&mut m, 1.0);
        assert_eq!(m.revenge_value(), 0.0, "nothing earned during the armour");
        step(&mut m, 4.1);
        hit(&mut m, 1.0);
        step(&mut m, 2.9);
        assert_eq!(m.revenge_value(), 1.0, "held 3 s");
        step(&mut m, 0.3);
        assert!(m.revenge_value() < 1.0e-3, "then 5 a second off: {}", m.revenge_value());
    }

    fn party_hit(point: f32) -> kh_core::HitRecord {
        let mut h = kh_core::HitRecord::empty(kh_core::TargetId(7));
        h.damage = 10.0;
        h.info.revenge_point = point;
        h
    }

    /// Every hit goes to the copy's own player rules, mid-combo too (the points count); its revenge arms it (no reaction, no
    /// points) and the counter is due once.
    #[test]
    fn a_copy_reacts_by_its_own_rules_and_its_revenge_arms_it() {
        let rules = BossRules::DEFAULT;
        let mut r = CopyRevenge::default();
        // (the user 2026-11-07: "make it 6.5") a FIXED 6.5 whatever the copied leader's HP or the body's stance
        for (stance, hp) in [(100.0, 3000.0), (100.0, 5700.0), (0.0, 1.0), (200.0, 90000.0)] {
            r.sync(&rules, 4.0, stance, hp);
            assert_eq!(r.limit(), 6.5, "stance {stance}, HP {hp}");
        }
        // the row at 0: the bosses' formula (the Mimic body, a 1000-HP leader: 7.65)
        r.sync(&BossRules { mimic_limit: 0.0, ..rules }, 4.0, 100.0, 3000.0);
        assert!((r.limit() - 7.65).abs() < 0.01, "{}", r.limit());
        r.sync(&rules, 4.0, 100.0, 3000.0);
        let dt = 1.0 / 60.0;
        let step = |r: &mut CopyRevenge, seconds: f32| {
            let mut fired = false;
            for _ in 0..(seconds / dt).round() as u32 {
                r.watch(dt);
                fired |= r.step(dt);
            }
            fired
        };
        step(&mut r, 1.0);
        for _ in 0..3 {
            assert_eq!(r.hit(&party_hit(1.0), true), CopyGate::React, "its own player rules decide, idle or mid-combo");
            step(&mut r, 0.25);
        }
        step(&mut r, 0.25);
        assert_eq!(r.value(), 3.0, "every hit counted");
        let mut hits = 3;
        while !r.due && hits < 20 {
            assert_eq!(r.hit(&party_hit(1.0), true), CopyGate::React);
            hits += 1;
            step(&mut r, 0.3);
        }
        assert!(r.due && r.revenges == 1 && hits == 7, "the 7th point crossed 6.5 ({hits} hits)");
        assert!(r.armoured());
        assert_eq!(r.hit(&party_hit(1.0), true), CopyGate::Revenge, "its revenge: unflinchable");
        assert_eq!(r.value(), 0.0, "no points meanwhile");
        step(&mut r, 4.1);
        assert!(!r.armoured());
        assert_eq!(r.hit(&party_hit(1.0), true), CopyGate::React);
        // a hit that took no HP, or a bad status, adds nothing (KH3's rule)
        let before = r.value();
        let _ = r.hit(&party_hit(1.0), false);
        assert_eq!(r.value(), before);
        // switched off: always a reaction, no meter
        let mut off = CopyRevenge::default();
        off.sync(&BossRules::OFF, 4.0, 100.0, 3000.0);
        assert_eq!(off.hit(&party_hit(1.0), true), CopyGate::React);
        assert_eq!(off.limit(), 0.0);
    }

    #[test]
    fn the_sora_copys_plan_by_form() {
        assert_eq!(sora_plan(0), (vec![0xE0, 0xE1, 0xE4], false));
        assert_eq!(sora_plan(form::RAGE), (vec![0xE0, 0xE1, 0xE4], false), "Rage keeps the Keyblade (the coordinator, 2026-11-07)");
        assert_eq!(sora_plan(form::HYPER_HAMMER), (vec![0x5D], true));
        assert_eq!(sora_plan(form::STORM_FLAG), (vec![0xA7], true));
    }
}

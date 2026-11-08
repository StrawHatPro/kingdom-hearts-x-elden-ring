//! (THE USER 2026-11-02: "make non-boss enemies bounce off of Goofy's shield if they're not doing a long combo string when Goofy
//! successfully performs a perfect deflect") THE BOUNCE, pad and AI alike: a perfect deflect of a normal enemy's swing asks the
//! host's rebound (`StaggerAsk::Rebound`) on top of the Stagger (no counter window since THE USER 2026-11-02); a boss, an attacker in the middle of a
//! string and a plain deflect keep today's rules.

use super::parry::{self, Parry, Tech};
use super::{GoofyAbilities, GoofyAbility, GoofyEvent, RankBank, Resolve, Telegraph, K};
use crate::growth::RankLine;
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::pad::PadFrame;
use crate::pawn::NpcPawn;
use crate::{Party, PartyConfig, SoraView};
use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{AttackClass, AttackInfo, HitAttacker, HitRecord, HitResult, HurtShape, StaggerAsk, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::test_world::TestWorld;
use kh_core::Tuning;

const DT: f32 = 1.0 / 60.0;
const T: TargetId = TargetId(1);
const T2: TargetId = TargetId(2);
const ENEMY: Vec3 = Vec3::new(200.0, 0.0, 0.0);
const ENEMY2: Vec3 = Vec3::new(200.0, 40.0, 0.0);
const X: PadButton = PadButton::FaceLeft;
const NONE: Vec2 = Vec2::ZERO;
const FWD: Vec2 = Vec2::new(0.0, 1.0);

/// The enemies; `boss`: the host calls them bosses (Elden Ring: on the boss bar); every question the kit asked.
struct Foes {
    list: Vec<(TargetId, Vec3)>,
    boss: bool,
    asks: Vec<(TargetId, StaggerAsk)>,
    hits: Vec<HitRecord>,
}

impl Foes {
    fn of(&self, (id, p): (TargetId, Vec3)) -> TargetInfo {
        TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
    }

    fn rebounds(&self) -> Vec<TargetId> {
        self.asks.iter().filter(|(_, a)| matches!(a, StaggerAsk::Rebound { .. })).map(|(t, _)| *t).collect()
    }
}

impl Targets for Foes {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        for &t in &self.list {
            out.push(self.of(t));
        }
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.list.iter().find(|t| t.0 == id).map(|&t| self.of(t))
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.position + Vec3::Z * 30.0, b: i.position + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.hits.push(*hit);
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
    fn stagger_ask(&mut self, id: TargetId, ask: StaggerAsk) -> bool {
        self.asks.push((id, ask));
        true
    }
    fn size_class(&self, _id: TargetId) -> u8 {
        if self.boss { 4 } else { 2 }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
    // (AUDIT-B) no bones here: a rider attached to a hit follows the target's origin and yaw
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

/// One Goofy, pad or AI, and his enemies.
struct Rig {
    world: TestWorld,
    foes: Foes,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    id: MemberId,
    held: PadButtons,
    pad: bool,
    events: Vec<GoofyEvent>,
}

impl Rig {
    /// The pad leader's Goofy at level 99, every ability owned and none worn (the plain rules), enemies at `at`.
    fn pad(at: &[Vec3], boss: bool) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        let mut rig = Rig::with(party, id, at, boss, true);
        rig.sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        let mut a = GoofyAbilities::default();
        for k in GoofyAbility::ALL {
            a.give(k);
        }
        rig.mm().status.goofy.as_mut().unwrap().abilities = a;
        rig.idle(10);
        rig
    }

    /// AI Goofy at level 99 (the overhaul's brain), the leader behind him.
    fn ai(boss: bool, seed: u64) -> Rig {
        let mut party = Party::new(PartyConfig { level99: true, total_exp: 1_527_640, seed });
        let id = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
        let mut rig = Rig::with(party, id, &[ENEMY], boss, false);
        rig.sora = SoraView { position: Vec3::new(-150.0, 0.0, 0.0), hp: 200, hp_max: 200, ..SoraView::default() };
        rig.party.set_in_battle(true);
        rig.mm().status.goofy.as_mut().unwrap().core.hop_cd = 0.0;
        rig.idle(10);
        rig
    }

    fn with(party: Party, id: MemberId, at: &[Vec3], boss: bool, pad: bool) -> Rig {
        let foes = Foes { list: at.iter().enumerate().map(|(i, p)| (TargetId(i as u32 + 1), *p)).collect(), boss, asks: Vec::new(), hits: Vec::new() };
        Rig { world: TestWorld::floor(), foes, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), id, held: PadButtons::NONE, pad, events: Vec::new() }
    }

    fn m(&self) -> &NpcPawn {
        self.party.member(self.id).unwrap()
    }

    fn mm(&mut self) -> &mut NpcPawn {
        self.party.member_mut(self.id).unwrap()
    }

    fn log(&self) -> Vec<&'static str> {
        self.m().status.goofy.as_ref().unwrap().run.log.clone()
    }

    fn logged(&self, w: &str) -> bool {
        self.log().contains(&w)
    }

    fn step(&mut self, stick: Vec2, held: &[PadButton]) {
        if self.pad {
            let mut now = PadButtons::NONE;
            for b in held {
                now.set(*b, true);
            }
            let pad = PadState { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
            self.held = now;
            self.party.set_pad_frame(self.id, PadFrame { stick, camera_yaw: 0.0, pad });
        }
        let mut host = PartyHost { world: &self.world, targets: &mut self.foes, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
        self.events.extend(self.party.gof.events.iter().map(|e| e.1));
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(NONE, &[]);
        }
    }

    /// A blow of `class` from `who` standing at `from`.
    fn hit_from(&mut self, who: TargetId, from: Vec3, class: AttackClass) -> HitResult {
        let me = self.m().position();
        let mut h = HitRecord::empty(TargetId::NONE);
        h.damage = 20.0;
        h.damage_kind = Kind::Small;
        h.direction = (me - from).normalize_or_zero();
        h.attacker = HitAttacker { position: from, yaw: 180.0, radius: 50.0, valid: true };
        h.position = me + Vec3::Z * 90.0;
        h.attack_info = AttackInfo { class, perilous: matches!(class, AttackClass::Thrust | AttackClass::Sweep | AttackClass::Grab | AttackClass::Heavy), attacker: Some(who) };
        self.party.receive_hit(self.id, &h)
    }

    /// The pad's tap of X with `stick`, the blow `lead` steps later, the step that hands its events on. Returns its events.
    fn tapped(&mut self, who: TargetId, from: Vec3, class: AttackClass, stick: Vec2, lead: usize) -> Vec<GoofyEvent> {
        self.events.clear();
        self.step(stick, &[X]);
        for _ in 0..lead {
            self.step(stick, &[]);
        }
        self.hit_from(who, from, class);
        self.idle(1);
        std::mem::take(&mut self.events)
    }

    /// One telegraphed swing at AI Goofy (as ai_tests.rs `attack`: on the list 0.8 s ahead, its window open 0.12 s, the blow
    /// 0.06 s in), then the step after. Returns its events.
    fn swing(&mut self, class: AttackClass) -> Vec<GoofyEvent> {
        self.events.clear();
        let tele = |eta: f32, active: bool| Telegraph { attacker: T, class, perilous: false, eta, active };
        let mut left = 0.8;
        while left > 0.0 {
            self.party.set_telegraphs(&[tele(left, false)]);
            self.idle(1);
            left -= DT;
        }
        let mut t = 0.0;
        let mut fired = false;
        while t < 0.12 {
            self.party.set_telegraphs(&[tele(0.0, true)]);
            if !fired && t >= 0.06 {
                fired = true;
                self.hit_from(T, ENEMY, class);
            }
            self.idle(1);
            t += DT;
        }
        self.party.set_telegraphs(&[]);
        self.idle(1);
        std::mem::take(&mut self.events)
    }

    /// Back to rest between swings: the string ends (rhythm.gap), HP, GUARD and Resolve back.
    fn settle(&mut self) {
        self.idle(150);
        let m = self.mm();
        m.status.hp = m.status.max_hp();
        if let Some(g) = m.status.guard.as_mut() {
            g.value = g.max;
            g.broken = false;
        }
        m.status.goofy.as_mut().unwrap().core.resolve = Resolve::default();
        self.events.clear();
    }

    fn bank(&mut self, line: RankLine, n: u64) {
        let i = RankLine::ALL.iter().position(|l| *l == line).unwrap();
        let mut parts: Vec<u64> = self.m().status.growth.ranks.parts().iter().map(|v| *v as u64).collect();
        parts[i] = n;
        self.mm().status.growth.ranks = RankBank::from_parts(&parts);
    }

    fn wear(&mut self, a: GoofyAbility) {
        let s = self.mm().status.goofy.as_mut().unwrap();
        s.abilities.give(a);
        s.abilities.set_equipped(a, true);
    }

    /// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique") the parry opened no counter: nothing of one
    /// stands on his state or in his log.
    fn no_counter(&self) -> bool {
        let log = &self.m().status.goofy.as_ref().unwrap().run.log;
        self.m().state != crate::states::NpcStateId::Pad(43) && !log.iter().any(|w| matches!(*w, "Counter Bash" | "Pin Down" | "Crescendo"))
    }
}

fn bounced(events: &[GoofyEvent]) -> Vec<TargetId> {
    events.iter().filter_map(|e| if let GoofyEvent::Bounce { target } = e { Some(*target) } else { None }).collect()
}

fn perfect_parry(events: &[GoofyEvent]) -> bool {
    events.iter().any(|e| matches!(e, GoofyEvent::Parried { right: true, .. }))
}

fn staggered(events: &[GoofyEvent], who: TargetId) -> bool {
    events.iter().any(|e| matches!(e, GoofyEvent::Stagger { target, .. } if *target == who))
}

// ------------------------------------------------------------------------------------------------ the rule

#[test]
fn a_perfect_deflect_is_the_right_answer_in_its_window_and_bounces_a_melee_swing_off_the_shield() {
    const { assert!(K.deflect_bounce, "the user's request is on in the build he plays") };
    let p = |tech, right| Parry { tech, right, aggressive: false, forgiven: false, revenge: false };
    assert!(parry::perfect(&p(Tech::Deflect, true)));
    assert!(!parry::perfect(&p(Tech::Deflect, false)), "a plain deflect of an attack that has its own technique");
    assert!(!parry::perfect(&Parry { forgiven: true, ..p(Tech::Deflect, true) }), "Second Deflect's late tap");
    assert!(parry::perfect(&Parry { revenge: true, ..p(Tech::Deflect, true) }), "11.3: Revenge Bash's deflect counts as perfect");
    for (answer, class, bounces) in [
        (p(Tech::Deflect, true), AttackClass::Normal, true),
        (p(Tech::PinningGuard, true), AttackClass::Thrust, true),
        (p(Tech::HopCounter, true), AttackClass::Sweep, true), // In the Zone: the shield's deflect of a sweep is the right one
        (p(Tech::Deflect, false), AttackClass::Thrust, false),
        (p(Tech::Deflect, false), AttackClass::Sweep, false),
        (p(Tech::BullBrace, true), AttackClass::Heavy, false),
        (p(Tech::ShieldReflect, true), AttackClass::Projectile, false),
        (p(Tech::SpellWard, true), AttackClass::Magic, false),
        (p(Tech::SlipCounter, true), AttackClass::Grab, false),
    ] {
        assert_eq!(parry::bounces(&answer, class, false), bounces, "{answer:?} of {class:?}");
        assert!(!parry::bounces(&answer, class, true), "mid-string: never ({answer:?} of {class:?})");
    }
}

#[test]
fn the_string_is_the_attackers_own() {
    let mut c = super::GoofyCore::default();
    assert!(!c.mid_string(Some(T)), "no string runs");
    c.string_hit();
    c.string_from = Some(T);
    assert!(c.mid_string(Some(T)));
    assert!(!c.mid_string(Some(T2)), "another attacker's first blow is no string of his");
    c.string_from = None;
    assert!(c.mid_string(Some(T2)), "a string of a blow nobody could name: nobody is known to be fresh");
    c.string_from = Some(T);
    for _ in 0..((K.rhythm_gap / DT) as usize + 2) {
        c.tick(DT);
    }
    assert!(!c.mid_string(Some(T)), "the string ran out");
}

// ------------------------------------------------------------------------------------------------ the pad

#[test]
fn pad_a_perfect_deflect_of_a_normal_enemy_bounces_it_and_keeps_the_stagger() {
    let mut rig = Rig::pad(&[ENEMY], false);
    let ev = rig.tapped(T, ENEMY, AttackClass::Normal, NONE, 3);
    assert!(rig.logged("deflect"), "{:?}", rig.log());
    assert_eq!(bounced(&ev), vec![T], "{ev:?}");
    assert_eq!(rig.foes.rebounds(), vec![T]);
    assert!(rig.foes.asks.iter().any(|(_, a)| *a == StaggerAsk::Rebound { wait: K.deflect_bounce_wait }), "the sheet's wait goes to the host");
    assert!(staggered(&ev, T), "the Stagger as before: {ev:?}");
    assert!(rig.no_counter(), "no counter window (THE USER 2026-11-02)");
}

#[test]
fn pad_a_pinning_guard_bounces_the_thrust() {
    let mut rig = Rig::pad(&[ENEMY], false);
    let ev = rig.tapped(T, ENEMY, AttackClass::Thrust, FWD, 3);
    assert!(rig.logged("pinning_guard"), "{:?}", rig.log());
    assert_eq!(bounced(&ev), vec![T]);
    assert!(rig.no_counter());
}

#[test]
fn pad_a_boss_never_bounces() {
    let mut rig = Rig::pad(&[ENEMY], true);
    let ev = rig.tapped(T, ENEMY, AttackClass::Normal, NONE, 3);
    assert!(rig.logged("deflect") && perfect_parry(&ev), "{:?}", rig.log());
    assert!(bounced(&ev).is_empty() && rig.foes.rebounds().is_empty(), "{ev:?}");
    assert!(staggered(&ev, T), "the boss keeps today's rules: the Stagger");
    assert!(rig.no_counter());
}

#[test]
fn pad_an_attacker_in_the_middle_of_its_string_does_not_bounce() {
    let mut rig = Rig::pad(&[ENEMY], false);
    let mut per_blow = Vec::new();
    for _ in 0..3 {
        rig.idle(19);
        let ev = rig.tapped(T, ENEMY, AttackClass::Normal, NONE, 1);
        assert!(perfect_parry(&ev), "{:?}", rig.log());
        per_blow.push(bounced(&ev).len());
    }
    assert_eq!(per_blow, vec![1, 0, 0], "its first blow cannot be known as a string's; the rest are mid-string");
    assert_eq!(rig.m().status.goofy.as_ref().unwrap().core.string, 3);
    assert!(rig.no_counter(), "Rhythm Deflect fills Stagger, no Crescendo");
}

#[test]
fn pad_another_attackers_first_blow_inside_his_string_still_bounces() {
    let mut rig = Rig::pad(&[ENEMY, ENEMY2], false);
    let first = rig.tapped(T, ENEMY, AttackClass::Normal, NONE, 3);
    rig.idle(19);
    let second = rig.tapped(T2, ENEMY2, AttackClass::Normal, NONE, 3);
    assert!(perfect_parry(&second), "{:?}", rig.log());
    assert_eq!((bounced(&first), bounced(&second)), (vec![T], vec![T2]));
}

#[test]
fn pad_a_plain_or_forgiven_deflect_does_not_bounce() {
    // a thrust deflected without Pinning Guard's stick: a plain deflect
    let mut rig = Rig::pad(&[ENEMY], false);
    let ev = rig.tapped(T, ENEMY, AttackClass::Thrust, NONE, 3);
    assert!(rig.logged("plain_deflect"), "{:?}", rig.log());
    assert!(bounced(&ev).is_empty() && rig.foes.rebounds().is_empty(), "{ev:?}");
    assert!(staggered(&ev, T), "its half Stagger as before");
    // a late tap Second Deflect forgave
    let mut rig = Rig::pad(&[ENEMY], false);
    rig.wear(GoofyAbility::SecondDeflect);
    rig.bank(RankLine::Deflect, 0);
    rig.events.clear();
    rig.step(NONE, &[X]);
    for _ in 0..14 {
        rig.step(NONE, &[X]); // past 0.20 s, inside the grace
    }
    rig.hit_from(T, ENEMY, AttackClass::Normal);
    rig.idle(1);
    assert!(rig.m().status.goofy.as_ref().unwrap().core.second_deflect_used, "{:?}", rig.log());
    assert!(bounced(&rig.events).is_empty() && rig.foes.rebounds().is_empty());
}

#[test]
fn a_host_that_refuses_the_rebound_hears_no_bounce() {
    struct Refuses(Foes);
    impl Targets for Refuses {
        fn enumerate(&self, out: &mut Vec<TargetInfo>) {
            self.0.enumerate(out)
        }
        fn info(&self, id: TargetId) -> Option<TargetInfo> {
            self.0.info(id)
        }
        fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
            self.0.hurt_shapes(id, out)
        }
        fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
            self.0.apply_hit(hit)
        }
        fn stagger_ask(&mut self, id: TargetId, ask: StaggerAsk) -> bool {
            self.0.stagger_ask(id, ask);
            !matches!(ask, StaggerAsk::Rebound { .. })
        }
        // (AUDIT-B) forwarded: the hit's bone and its pose are the wrapped host's
        fn hit_bone(&self, id: kh_core::TargetId, at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
            self.0.hit_bone(id, at)
        }
        fn bone_pose(&self, id: kh_core::TargetId, bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
            self.0.bone_pose(id, bone)
        }
    }
    let mut rig = Rig::pad(&[ENEMY], false);
    rig.events.clear();
    rig.step(NONE, &[X]);
    rig.idle(3);
    rig.hit_from(T, ENEMY, AttackClass::Normal);
    let world = TestWorld::floor();
    let mut refuses = Refuses(Foes { list: vec![(T, ENEMY)], boss: false, asks: Vec::new(), hits: Vec::new() });
    let mut host = PartyHost { world: &world, targets: &mut refuses, audio: &mut rig.audio, effects: &mut rig.effects };
    rig.party.step(DT, &rig.sora, &mut host);
    let ev: Vec<GoofyEvent> = rig.party.gof.events.iter().map(|e| e.1).collect();
    assert!(perfect_parry(&ev), "{:?}", rig.log());
    assert_eq!(refuses.0.rebounds(), vec![T], "asked");
    assert!(bounced(&ev).is_empty(), "refused: nothing to tell: {ev:?}");
}

// ------------------------------------------------------------------------------------------------ AI Goofy

/// Swings at AI Goofy until `n` of them were perfect deflects; checks each of those with `check` (its events, the rebounds it
/// asked), and that every other swing asked none.
fn ai_swings(boss: bool, n: usize, check: impl Fn(&[GoofyEvent], &[TargetId])) {
    let mut rig = Rig::ai(boss, 21);
    let mut perfect = 0;
    for _ in 0..40 {
        // (THE USER 2026-11-02: parries fill Stagger fast and it drains slowly now) a fresh gauge each swing: one the swings
        // filled would hold the enemy BROKEN, and a BROKEN body is never bounced (`seams::on_bounce`)
        rig.party.gof.book = Default::default();
        let before = rig.foes.rebounds().len();
        let ev = rig.swing(AttackClass::Normal);
        let rebounds = rig.foes.rebounds()[before..].to_vec();
        if perfect_parry(&ev) {
            perfect += 1;
            check(&ev, &rebounds);
        } else {
            assert!(bounced(&ev).is_empty() && rebounds.is_empty(), "no perfect deflect, no bounce: {ev:?}");
        }
        if perfect >= n {
            return;
        }
        rig.settle();
    }
    panic!("AI Goofy made {perfect} perfect deflect(s) in 40 swings: {:?}", rig.log());
}

#[test]
fn ai_a_perfect_deflect_of_a_normal_enemy_bounces_it() {
    ai_swings(false, 2, |ev, rebounds| {
        assert_eq!(bounced(ev), vec![T], "{ev:?}");
        assert_eq!(rebounds, [T]);
        assert!(staggered(ev, T));
    });
}

#[test]
fn ai_a_boss_never_bounces() {
    ai_swings(true, 2, |ev, rebounds| {
        assert!(bounced(ev).is_empty() && rebounds.is_empty(), "{ev:?}");
        assert!(staggered(ev, T), "the Stagger as before");
    });
}

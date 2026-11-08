//! (GF-ARTS) The Shield Arts, one test per rule of documentation/GOOFY_OVERHAUL.md 6.2, 8, 9, 11.2 and PLAN.md R1, R14, R16.

use super::super::goofy::{GoofyRun, GUARD, GUARD_BREAK, PARRY};
use super::super::goofy_shots as shots;
use super::super::{PadFrame, JUMP, RUN};
use super::*;
use crate::goofy::{GoofyAbilities, RankBank, Telegraph};
use crate::growth::RankLine;
use crate::ids::Chara;
use crate::{Party, PartyConfig, SoraView};
use kh_core::enums::ETresDamageKind as DK;
use kh_core::host::{AttackInfo, HitAttacker, HurtShape, TargetDefense, TargetFlags, TargetInfo, Targets};
use kh_core::input::{PadButton, PadButtons};
use kh_core::test_world::TestWorld;
use kh_core::Tuning;

const DT: f32 = 1.0 / 60.0;
const LB: PadButton = PadButton::LeftShoulder;
const X: PadButton = PadButton::FaceLeft;
const A: PadButton = PadButton::FaceBottom;
const B: PadButton = PadButton::FaceRight;
const NONE: Vec2 = Vec2::ZERO;
const ENEMY: Vec3 = Vec3::new(400.0, 0.0, 0.0);
const T: TargetId = TargetId(1);

struct Dummies(Vec<(TargetId, Vec3)>, Vec<HitRecord>);

impl Dummies {
    fn info_of(&self, (id, p): (TargetId, Vec3)) -> TargetInfo {
        TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
    }
}

impl Targets for Dummies {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        for &t in &self.0 {
            out.push(self.info_of(t));
        }
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.0.iter().find(|t| t.0 == id).map(|&t| self.info_of(t))
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.position + Vec3::Z * 30.0, b: i.position + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.1.push(*hit);
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
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

struct Rig {
    world: TestWorld,
    dummies: Dummies,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    id: MemberId,
    held: PadButtons,
    events: Vec<GoofyEvent>,
}

impl Rig {
    fn new(dummies: &[Vec3]) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        let d = Dummies(dummies.iter().enumerate().map(|(i, p)| (TargetId(i as u32 + 1), *p)).collect(), Vec::new());
        let mut rig = Rig { world: TestWorld::floor(), dummies: d, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), id, held: PadButtons::NONE, events: Vec::new() };
        rig.sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        let mut a = GoofyAbilities::default();
        for k in GA::ALL {
            a.give(k);
        }
        rig.s().abilities = a;
        rig.mm().status.growth.ranks = RankBank::default(); // the test level's ranks are all 3: rank 1 unless a test says
        rig.idle(10);
        rig
    }

    fn m(&self) -> &NpcPawn {
        self.party.member(self.id).unwrap()
    }

    fn mm(&mut self) -> &mut NpcPawn {
        self.party.member_mut(self.id).unwrap()
    }

    fn s(&mut self) -> &mut crate::goofy::GoofyState {
        self.mm().status.goofy.as_mut().unwrap()
    }

    fn k(&self) -> &GoofyRun {
        &self.m().status.goofy.as_ref().unwrap().run
    }

    fn a(&self) -> &ArtsRun {
        &self.k().arts
    }

    fn equip(&mut self, first: Art, second: Art) {
        self.party.set_goofy_arts([first.id(), second.id()]);
    }

    /// The rank of an Art.
    fn rank(&mut self, art: Art, rank: u64) {
        let i = RankLine::ALL.iter().position(|l| *l == art.mv()).unwrap();
        let mut parts: Vec<u64> = self.m().status.growth.ranks.parts().iter().map(|v| *v as u64).collect();
        parts[i] = rank - 1;
        self.mm().status.growth.ranks = RankBank::from_parts(&parts);
    }

    fn wear(&mut self, k: GA) {
        self.s().abilities.set_equipped(k, true);
    }

    fn step(&mut self, stick: Vec2, held: &[PadButton]) {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = kh_core::input::PadState { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        self.party.set_pad_frame(self.id, PadFrame { stick, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
        self.events.extend(self.party.gof.events.iter().map(|e| e.1));
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(NONE, &[]);
        }
    }

    fn run(&mut self, n: usize, stick: Vec2, held: &[PadButton]) {
        for _ in 0..n {
            self.step(stick, held);
        }
    }

    /// LB, then the button with it, then both let go.
    fn lb(&mut self, b: PadButton) {
        self.step(NONE, &[LB]);
        self.step(NONE, &[LB, b]);
        self.step(NONE, &[]);
    }

    fn until(&mut self, n: usize, stick: Vec2, f: impl Fn(&Rig) -> bool) -> usize {
        for i in 0..n {
            if f(self) {
                return i;
            }
            self.step(stick, &[]);
        }
        panic!("condition not met in {n} steps: state {:?} clip {} log {:?}", self.state(), self.m().anim.clip.name, self.k().log);
    }

    fn state(&self) -> NpcStateId {
        self.m().state
    }

    fn logged(&self, w: &str) -> bool {
        self.k().log.contains(&w)
    }

    fn mp(&self) -> i32 {
        self.m().status.mp
    }

    fn hits(&self, row: &str) -> Vec<HitRecord> {
        self.dummies.1.iter().filter(|h| h.attack_name == row).copied().collect()
    }

    fn stagger_seen(&self) -> f32 {
        self.events.iter().map(|e| if let GoofyEvent::Stagger { amount, why: StaggerWhy::Art, .. } = e { *amount } else { 0.0 }).sum()
    }

    fn record(&self, class: AttackClass, damage: f32) -> HitRecord {
        let mut h = HitRecord::empty(TargetId::NONE);
        h.damage = damage;
        h.damage_kind = DK::Small;
        h.direction = Vec3::NEG_X;
        h.attacker = HitAttacker { position: ENEMY, yaw: 180.0, radius: 50.0, valid: true };
        h.position = Vec3::Z * 90.0;
        h.attack_info = AttackInfo { class, perilous: false, attacker: Some(T) };
        h
    }

    fn hit(&mut self, class: AttackClass, damage: f32) -> HitResult {
        let h = self.record(class, damage);
        self.party.receive_hit(self.id, &h)
    }

    /// A friend (Donald, AI) at `at`.
    fn friend(&mut self, at: Vec3) -> MemberId {
        let d = self.party.spawn(Chara::Donald, at, 0.0);
        self.idle(2);
        d
    }

    fn telegraph(&mut self) {
        self.party.set_telegraphs(&[Telegraph { attacker: T, class: AttackClass::Normal, perilous: false, eta: 0.3, active: false }]);
    }
}

// ------------------------------------------------------------------------------------------------ the sheet

#[test]
fn every_art_costs_a_real_share_of_his_bar_and_each_rank_is_better() {
    // THE USER 2026-11-02: "Goofy's abilities (except taunt) should cost more MP ... they are way too spammable": a full bar buys a
    // few Arts, not a stream (his own pool: 88 at the start, 136 with every boss reward); Taunt keeps R16's price
    let (start, all) = (crate::balance::BP.n_ex002_start_mp as i32, (crate::balance::BP.n_ex002_start_mp + crate::balance::BP.n_ex002_rewards_mp) as i32);
    for art in Art::ALL {
        let costs = [mp(art, 1), mp(art, 2), mp(art, 3)];
        if art == Art::Taunt {
            assert_eq!(costs, [3, 3, 2], "Taunt is left as it was");
            continue;
        }
        assert!((3..=6).contains(&(start / costs[0])), "a level-1 bar buys a few at rank 1: {art:?} {costs:?}");
        assert!(all / costs[2] <= 12, "a full bar at rank 3 is still no stream: {art:?} {costs:?}");
        assert!(costs[0] >= costs[1] && costs[1] >= costs[2] && costs[0] > costs[2], "rank 3 is cheaper: {art:?} {costs:?}");
        let (p, s) = ([power(art, 1), power(art, 2), power(art, 3)], [stagger(art, 1), stagger(art, 2), stagger(art, 3)]);
        assert!(p[0] <= p[1] && p[1] <= p[2], "{art:?} {p:?}");
        assert!(s[0] <= s[1] && s[1] <= s[2], "{art:?} {s:?}");
    }
    assert!(stagger(Art::Bash, 1) > 2.0 * stagger(Art::Charge, 3), "Bash is the big Stagger");
    // (THE USER 2026-11-02: "Goofy's big damage should solely come from his guard break attacks from 6.3")
    assert!(power(Art::Bash, 3) * 3.0 <= K.breaker_power, "the Breaker stays where his damage lives");
    assert_eq!(mp(Art::Charge, 0), mp(Art::Charge, 1), "a rank below 1 reads as 1");
}

#[test]
fn every_clip_and_kind_of_the_arts_is_real() {
    for c in [K.art_charge_clip, K.art_tornado_start_clip, K.art_tornado_loop_clip, K.art_tornado_end_clip, K.art_bash_clip, K.art_turbo_start_clip, K.art_turbo_loop_clip, K.art_turbo_end_clip, K.art_taunt_clip, K.art_interpose_clip, K.art_toss_clip, K.art_toss_air_clip] {
        assert_eq!(crate::tables::clip(Chara::Goofy, c).name, c, "a misspelled clip silently becomes a 0.5 s empty one");
    }
    for k in [K.art_charge_kind, K.art_tornado_kind, K.art_bash_kind, K.art_turbo_kind, K.art_toss_kind] {
        assert!(Kind::from_game_name(k).is_some(), "{k}");
    }
    assert!(K.art_bash_hit < shots::clip(K.art_bash_clip).length && K.art_toss_throw < shots::clip(K.art_toss_clip).length && K.art_toss_throw < shots::clip(K.art_toss_air_clip).length);
}

#[test]
fn ids_match_kh_core_and_taunt_stands_apart() {
    for art in Art::ALL {
        assert_eq!(Art::from_id(art.id()), Some(art));
        assert!(!ids::name(art.id()).is_empty());
    }
    assert_eq!(Art::ALL.iter().filter(|a| ids::is_art(a.id())).count(), ids::ARTS.len());
    assert!(!ids::is_art(Art::Taunt.id()));
}

// ------------------------------------------------------------------------------------------------ the buttons, the MP

#[test]
fn lb_with_a_b_and_x_fire_the_two_arts_and_taunt_from_the_pad_alone() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Bash, Art::Turbo);
    rig.lb(A);
    assert!(rig.logged("art_bash"), "{:?}", rig.k().log);
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
    rig.lb(B);
    assert!(rig.logged("art_turbo"), "{:?}", rig.k().log);
    rig.step(NONE, &[X]);
    rig.step(NONE, &[]);
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
    rig.lb(X);
    assert_eq!(rig.state(), NpcStateId::Pad(TAUNT));
    assert!(rig.k().taunt.is_some());
    assert!(!matches!(rig.state(), NpcStateId::Pad(GUARD)), "LB + X is no guard");
}

#[test]
fn the_panel_does_not_run_the_old_action_route() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Charge, Art::Tornado);
    rig.lb(B);
    assert!(rig.m().action.is_none(), "no KH3 action: {:?}", rig.m().action);
    assert_eq!(rig.state(), NpcStateId::Pad(ART));
}

#[test]
fn an_empty_slot_does_nothing() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.party.set_goofy_arts([Art::Charge.id(), 0]);
    let before = rig.mp();
    rig.lb(B);
    assert_eq!((rig.state(), rig.mp()), (NpcStateId::Idle, before));
}

#[test]
fn an_art_not_learned_yet_does_not_start() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::ShieldToss, Art::Charge);
    rig.mm().status.level = 6;
    assert_eq!(rig.m().status.gof_rank(GoofyMove::ShieldToss), 0);
    rig.lb(A);
    assert_eq!(rig.state(), NpcStateId::Idle);
    rig.lb(B);
    assert!(rig.logged("art_charge"), "Charge is his from the start");
}

#[test]
fn an_art_pays_its_row_at_its_rank() {
    for rank in 1..=3u64 {
        let mut rig = Rig::new(&[ENEMY]);
        rig.equip(Art::Charge, Art::Bash);
        rig.rank(Art::Charge, rank);
        let before = rig.mp();
        rig.lb(A);
        assert_eq!(before - rig.mp(), mp(Art::Charge, rank as u8), "rank {rank}");
    }
}

#[test]
fn too_little_mp_refuses_with_the_panel_sound_and_charging_refuses() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Bash, Art::Charge);
    // (THE USER 2026-11-02: "let Goofy use his last MP and go into MP reserve") 0 MP is short (1 MP now buys the Art: crate::mp_rule)
    rig.mm().status.mp = 0;
    rig.step(NONE, &[LB]);
    rig.step(NONE, &[LB, A]);
    assert_eq!((rig.state(), rig.mp()), (NpcStateId::Idle, 0), "short of MP");
    assert!(rig.party.pad_menu(rig.id).unwrap().events.contains(&MenuEvent::Refused), "the refused SE: he has not enough MP");
    rig.idle(2);
    rig.mm().status.mp = 50;
    rig.mm().status.mp_charge_left = 5.0;
    rig.lb(A);
    assert_eq!(rig.state(), NpcStateId::Idle, "no Art while the charge runs");
}

#[test]
fn landing_on_zero_mp_starts_the_charge() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Bash, Art::Charge);
    rig.mm().status.mp = mp(Art::Bash, 1);
    rig.lb(A);
    assert!(rig.logged("art_bash"));
    assert_eq!(rig.mp(), 0);
    assert!(rig.m().status.mp_charge_left > 0.0);
}

// ------------------------------------------------------------------------------------------------ the cooldowns
// THE USER 2026-11-02: "Goofy's abilities (except taunt) should cost more MP, and introduce a cooldown with a visible timer, they
// are way too spammable"

/// Steps until the cooldown of `art` has run out.
fn wait_out(rig: &mut Rig, art: Art) {
    let n = (cooldown(art) / DT).ceil() as usize + 2;
    rig.idle(n);
    assert_eq!(cooldown_left(&rig.m().status, art), 0.0);
}

fn times(rig: &Rig, w: &str) -> usize {
    rig.k().log.iter().filter(|l| **l == w).count()
}

#[test]
fn every_art_but_taunt_has_a_cooldown_row() {
    for art in Art::ALL {
        if art == Art::Taunt {
            assert_eq!(cooldown(art), 0.0, "the user left Taunt out");
        } else {
            assert!((3.0..=20.0).contains(&cooldown(art)), "{art:?}: {}", cooldown(art));
        }
    }
    assert!(cooldown(Art::Interpose) >= cooldown(Art::Charge), "the free parry for a friend waits longest");
}

#[test]
fn a_used_art_waits_its_cooldown_refused_with_the_sound_and_nothing_spent_then_runs_again() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Bash, Art::Charge);
    let full = rig.mp();
    rig.lb(A);
    assert!(rig.logged("art_bash"));
    assert_eq!(full - rig.mp(), mp(Art::Bash, 1), "the cost is charged");
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
    let left = cooldown_left(&rig.m().status, Art::Bash);
    assert!(left > 0.0 && left < cooldown(Art::Bash), "it started when he used it: {left}");
    // pressed again while it waits: refused with the refused SE, as short MP is, and no MP goes
    let before = rig.mp();
    rig.step(NONE, &[LB]);
    rig.step(NONE, &[LB, A]);
    assert!(rig.party.pad_menu(rig.id).unwrap().events.contains(&MenuEvent::Refused), "the refused SE");
    rig.step(NONE, &[]);
    assert_eq!((rig.state(), rig.mp(), times(&rig, "art_bash")), (NpcStateId::Idle, before, 1));
    assert!(rig.logged("art_cooling"));
    // the wait runs out: it is his again, at its price
    wait_out(&mut rig, Art::Bash);
    let before = rig.mp();
    rig.lb(A);
    assert_eq!(times(&rig, "art_bash"), 2, "{:?}", rig.k().log);
    assert_eq!(before - rig.mp(), mp(Art::Bash, 1));
}

#[test]
fn each_art_waits_on_its_own_and_taunt_never_waits() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Bash, Art::Charge);
    rig.lb(A);
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
    rig.lb(B);
    assert!(rig.logged("art_charge"), "Bash's wait is Bash's alone");
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
    for n in 1..=2 {
        let before = rig.mp();
        rig.lb(X);
        assert_eq!(times(&rig, "art_taunt"), n, "Taunt again at once");
        assert_eq!(before - rig.mp(), mp(Art::Taunt, 1), "Taunt's price is untouched");
        rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
    }
    assert_eq!(cooldown_left(&rig.m().status, Art::Taunt), 0.0);
    assert!(cooldown_left(&rig.m().status, Art::Bash) > 0.0 && cooldown_left(&rig.m().status, Art::Charge) > 0.0);
}

#[test]
fn the_panel_row_carries_the_wait_and_taunts_never_does() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Bash, Art::Charge);
    assert_eq!(panel_row(&rig.m().status, ids::BASH).cooldown, (0.0, 0.0));
    rig.lb(A);
    rig.idle(30);
    let row = panel_row(&rig.m().status, ids::BASH);
    assert!(row.cooldown.0 > 0.0 && row.cooldown.0 < cooldown(Art::Bash), "{:?}", row.cooldown);
    assert_eq!(row.cooldown.1, cooldown(Art::Bash), "its full length: what the HUD's gauge drains from");
    assert!(row.enabled, "the wait is not `enabled`: the HUD keeps the entry lit over its gauge");
    rig.lb(X);
    assert_eq!(panel_row(&rig.m().status, ids::TAUNT).cooldown, (0.0, 0.0));
    wait_out(&mut rig, Art::Bash);
    assert_eq!(panel_row(&rig.m().status, ids::BASH).cooldown, (0.0, 0.0));
}

#[test]
fn ai_goofys_brain_is_not_offered_an_art_that_waits() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Bash, Art::Charge);
    assert_eq!(ready_art_moves(&rig.m().status), [Some(GoofyMove::GoofyBash), Some(GoofyMove::GoofyCharge)]);
    rig.lb(A);
    assert_eq!(ready_art_moves(&rig.m().status), [None, Some(GoofyMove::GoofyCharge)]);
    assert_eq!(art_moves(&rig.m().status), [Some(GoofyMove::GoofyBash), Some(GoofyMove::GoofyCharge)], "still equipped");
}

/// Tap, a Normal hit: a perfect deflect, its recoil.
fn deflect(rig: &mut Rig) {
    rig.step(NONE, &[X]);
    rig.step(NONE, &[]);
    assert!(rig.hit(AttackClass::Normal, 10.0).guarded);
    assert!(rig.logged("deflect"), "{:?}", rig.k().log);
}

/// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique") no Art is free after a parry any more: out of
/// its recoil an Art costs its MP like any other, and the perfect parry's own MP (`parry.mp`) came first.
#[test]
fn after_a_parry_an_art_costs_its_mp_and_the_perfect_parry_paid_some_back() {
    let mut rig = Rig::new(&[Vec3::new(170.0, 0.0, 0.0)]);
    rig.equip(Art::Bash, Art::Charge);
    rig.mm().status.mp = 40;
    deflect(&mut rig);
    assert_eq!(rig.mp(), 40 + K.parry_mp as i32, "the perfect deflect's MP");
    rig.until(60, NONE, |r| r.k().hitstop <= 0.0);
    let before = rig.mp();
    rig.lb(A);
    assert!(rig.logged("art_bash") && !rig.logged("free_art"), "{:?}", rig.k().log);
    assert_eq!(before - rig.mp(), mp(Art::Bash, rig.m().status.gof_rank(GoofyMove::GoofyBash)), "its full price");
}
#[test]
fn a_shove_leads_into_an_art_at_its_link_but_not_in_its_wind_up() {
    let mut rig = Rig::new(&[Vec3::new(170.0, 0.0, 0.0)]);
    rig.equip(Art::Bash, Art::Charge);
    rig.step(NONE, &[A]);
    rig.step(NONE, &[]);
    rig.idle(2);
    assert_eq!(rig.state(), NpcStateId::Pad(COMBO));
    rig.lb(A);
    assert!(!rig.logged("art_bash"), "not in the wind-up");
    rig.until(120, NONE, |r| combo::confirm_open(r.m()));
    rig.lb(A);
    assert!(rig.logged("art_bash"), "{:?}", rig.k().log);
}

#[test]
fn only_shield_toss_goes_in_the_air() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Bash, Art::ShieldToss);
    rig.step(NONE, &[B]);
    rig.run(14, NONE, &[B]);
    assert!(!rig.m().on_ground());
    rig.step(NONE, &[LB]);
    rig.step(NONE, &[LB, A]);
    assert!(!rig.logged("art_bash"), "no Bash up here");
    rig.step(NONE, &[LB]);
    rig.step(NONE, &[LB, B]);
    assert!(!rig.m().on_ground());
    assert!(rig.logged("art_toss"), "{:?}", rig.k().log);
    assert_eq!(rig.m().anim.clip.name, K.art_toss_air_clip);
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle && !bare_handed(r.m()));
}

// ------------------------------------------------------------------------------------------------ Goofy Charge

#[test]
fn charge_rushes_shoves_once_and_ends_in_a_guard_stance() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Charge, Art::Bash);
    rig.lb(A);
    assert_eq!(rig.state(), NpcStateId::Pad(ART));
    assert_eq!(rig.m().anim.clip.name, K.art_charge_clip);
    rig.until(120, NONE, |r| r.state() == NpcStateId::Pad(GUARD));
    let moved = horizontal(rig.m().position()).length();
    let want = K.art_charge_speed * K.art_charge_time;
    assert!((moved - want).abs() < 120.0, "the rush: {moved} of {want}");
    assert_eq!(rig.hits(Art::Charge.row()).len(), 1, "one shove per target");
    assert_eq!(rig.dummies.1[0].info.power, K.art_charge_power);
    assert!((rig.stagger_seen() - K.art_charge_stagger).abs() < 0.01);
    // the stance holds with X let go, then he stands down
    let held = rig.until(120, NONE, |r| r.state() != NpcStateId::Pad(GUARD));
    assert!((held as f32 * DT - K.art_charge_stance).abs() < 0.1, "held {held} steps");
    assert!(matches!(rig.state(), NpcStateId::Idle), "{:?}", rig.state());
}

#[test]
fn x_held_stays_in_the_guard_after_charge() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Charge, Art::Bash);
    rig.lb(A);
    rig.run(120, NONE, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD));
}

#[test]
fn charge_s_stance_blocks_and_only_rank_3_opens_it_as_a_tap() {
    for (rank, parried) in [(1, false), (3, true)] {
        let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)]);
        rig.equip(Art::Charge, Art::Bash);
        rig.rank(Art::Charge, rank);
        rig.lb(A);
        rig.until(120, NONE, |r| r.state() == NpcStateId::Pad(GUARD));
        // the rush took him past where the rig's hits come from: this one comes from ahead
        let mut h = rig.record(AttackClass::Normal, 10.0);
        h.attacker.position = rig.m().position() + forward(rig.m().yaw) * 200.0;
        let r = rig.party.receive_hit(rig.id, &h);
        assert!(r.guarded, "rank {rank}: the stance is a guard");
        assert_eq!(rig.state() == NpcStateId::Pad(PARRY), parried, "rank {rank}: {:?}", rig.state());
    }
}

// ------------------------------------------------------------------------------------------------ Goofy Tornado

#[test]
fn tornado_hits_around_him_again_and_again_and_ends_on_its_own() {
    let mut rig = Rig::new(&[Vec3::new(0.0, 170.0, 0.0), Vec3::new(-170.0, 0.0, 0.0)]);
    rig.equip(Art::Tornado, Art::Bash);
    rig.lb(A);
    rig.until(120, NONE, |r| r.m().anim.clip.name == K.art_tornado_loop_clip);
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
    let hits = rig.hits(Art::Tornado.row());
    let expected = (K.art_tornado_time / K.art_tornado_tick).floor() as usize + 1;
    assert_eq!(hits.len(), 2 * expected, "both enemies, each tick");
    assert!(hits.iter().all(|h| h.info.power == K.art_tornado_power));
}

#[test]
fn tornado_deflects_bullets_only_while_it_spins() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Tornado, Art::Bash);
    rig.lb(A);
    let r = rig.hit(AttackClass::Projectile, 10.0);
    assert!(!r.guarded, "the wind-up is open");
}

#[test]
fn the_spin_takes_a_bullet_from_any_side_and_costs_no_guard() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Tornado, Art::Bash);
    rig.lb(A);
    rig.until(120, NONE, |r| r.m().anim.clip.name == K.art_tornado_loop_clip);
    rig.mm().yaw = 180.0; // his back to the shooter
    let (hp, guard) = (rig.m().status.hp, rig.m().status.guard.unwrap().value);
    for class in [AttackClass::Projectile, AttackClass::Magic] {
        let r = rig.hit(class, 20.0);
        assert!(r.guarded && r.damage_dealt == 0.0, "{class:?}: {r:?}");
    }
    assert_eq!(rig.m().status.hp, hp);
    assert!(rig.m().status.guard.unwrap().value >= guard);
    assert!(rig.logged("spin_deflect"));
    assert_eq!(rig.state(), NpcStateId::Pad(ART), "he spins on");
    rig.idle(2);
    assert!(rig.events.iter().any(|e| matches!(e, GoofyEvent::Stagger { amount, .. } if *amount == K.art_tornado_deflect_stagger)));
    assert!(rig.hits(Art::Tornado.row()).iter().all(|h| h.damage < 20.0), "below rank 3 nothing flies back");
}

#[test]
fn a_blade_is_not_a_bullet() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Tornado, Art::Bash);
    rig.lb(A);
    rig.until(120, NONE, |r| r.m().anim.clip.name == K.art_tornado_loop_clip);
    assert!(!rig.hit(AttackClass::Normal, 10.0).guarded);
}

#[test]
fn rank_3_tornado_bats_the_shot_back_at_the_shooter() {
    let mut rig = Rig::new(&[Vec3::new(1000.0, 0.0, 0.0)]);
    rig.equip(Art::Tornado, Art::Bash);
    rig.rank(Art::Tornado, 3);
    rig.lb(A);
    rig.until(120, NONE, |r| r.m().anim.clip.name == K.art_tornado_loop_clip);
    rig.hit(AttackClass::Projectile, 30.0);
    rig.until(60, NONE, |r| !r.hits(Art::Tornado.row()).is_empty());
    assert_eq!(rig.hits(Art::Tornado.row())[0].damage, 30.0 * K.art_tornado_r3_back, "its own damage, flat");
}

// ------------------------------------------------------------------------------------------------ Goofy Bash

#[test]
fn bash_slams_ahead_of_him_for_a_big_stagger_with_a_hit_stop() {
    let mut rig = Rig::new(&[Vec3::new(200.0, 0.0, 0.0), Vec3::new(-200.0, 0.0, 0.0)]);
    rig.equip(Art::Bash, Art::Charge);
    rig.mm().yaw = 0.0;
    rig.step(NONE, &[LB]);
    rig.step(Vec2::new(0.0, 1.0), &[LB, A]);
    rig.step(NONE, &[]);
    assert_eq!(rig.m().anim.clip.name, K.art_bash_clip);
    rig.until(240, NONE, |r| !r.hits(Art::Bash.row()).is_empty());
    assert_eq!(rig.hits(Art::Bash.row()).len(), 1, "ahead only");
    assert_eq!(rig.dummies.1[0].info.power, K.art_bash_power);
    rig.idle(1);
    assert!(rig.k().hitstop > 0.0, "the slam sells its hit");
    assert!(rig.stagger_seen() >= K.art_bash_stagger);
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
}

#[test]
fn rank_3_bash_rings_out_around_him() {
    let mut rig = Rig::new(&[Vec3::new(200.0, 0.0, 0.0), Vec3::new(-200.0, 0.0, 0.0)]);
    rig.equip(Art::Bash, Art::Charge);
    rig.rank(Art::Bash, 3);
    rig.lb(A);
    rig.until(240, NONE, |r| !r.hits(Art::Bash.row()).is_empty());
    rig.idle(3);
    assert_eq!(rig.hits(Art::Bash.row()).len(), 2);
    assert_eq!(rig.dummies.1[0].info.power, K.art_bash_power_r3);
}

#[test]
fn the_stick_takes_him_out_of_bash_after_its_hit() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Bash, Art::Charge);
    rig.lb(A);
    rig.until(240, Vec2::new(0.0, 1.0), |r| r.state() != NpcStateId::Pad(ART));
    assert!(rig.m().state_time < 1.0);
    assert_eq!(rig.state(), NpcStateId::Pad(RUN));
}

// ------------------------------------------------------------------------------------------------ Goofy Turbo

#[test]
fn turbo_rides_and_steers_and_stops_itself() {
    let mut rig = Rig::new(&[]);
    rig.equip(Art::Turbo, Art::Bash);
    rig.lb(A);
    rig.until(120, NONE, |r| r.m().anim.clip.name == K.art_turbo_loop_clip);
    let start = rig.m().position();
    rig.run(60, NONE, &[]);
    let v = horizontal(rig.m().position() - start).length();
    assert!((v - K.art_turbo_speed).abs() < 60.0, "1 s of the ride: {v}");
    rig.run(30, Vec2::new(1.0, 0.0), &[]);
    assert!(rig.m().yaw > 60.0, "the stick steers: {}", rig.m().yaw);
    let yaw = rig.m().yaw;
    rig.run(10, NONE, &[]);
    assert!(wrap180(rig.m().yaw - yaw).abs() < 0.5, "no weave without the stick");
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
}

#[test]
fn turbo_hits_what_it_rides_into() {
    let mut rig = Rig::new(&[Vec3::new(900.0, 0.0, 0.0)]);
    rig.equip(Art::Turbo, Art::Bash);
    rig.dummies.1.clear();
    rig.lb(A);
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
    assert!(!rig.hits(Art::Turbo.row()).is_empty());
}

#[test]
fn x_cancels_turbo_into_the_guard_wherever_the_stick_points() {
    let mut rig = Rig::new(&[]);
    rig.equip(Art::Turbo, Art::Bash);
    rig.lb(A);
    rig.until(120, NONE, |r| r.m().anim.clip.name == K.art_turbo_loop_clip);
    rig.run(10, Vec2::new(1.0, 0.0), &[]);
    rig.step(Vec2::new(1.0, 0.0), &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD), "the guard, not the hop: {:?}", rig.k().log);
    assert!(rig.a().running.is_none());
    assert!(rig.k().shots.iter().all(|a| a.tag != TAG), "the sled's sphere went with it");
    assert_eq!(rig.m().anim.speed, 1.0);
}

#[test]
fn x_cancels_turbo_in_its_wind_up_too() {
    let mut rig = Rig::new(&[]);
    rig.equip(Art::Turbo, Art::Bash);
    rig.lb(A);
    rig.idle(3);
    rig.step(NONE, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD));
}

#[test]
fn no_other_art_cancels_into_the_guard() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Bash, Art::Turbo);
    rig.lb(A);
    rig.idle(3);
    rig.step(NONE, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(ART), "the slam commits him");
}

#[test]
fn b_jumps_off_the_sled() {
    let mut rig = Rig::new(&[]);
    rig.equip(Art::Turbo, Art::Bash);
    rig.lb(A);
    rig.until(120, NONE, |r| r.m().anim.clip.name == K.art_turbo_loop_clip);
    rig.step(NONE, &[B]);
    assert_eq!(rig.state(), NpcStateId::Pad(JUMP));
    assert!(rig.a().running.is_none());
}

#[test]
fn only_rank_3_turbo_starts_untouchable() {
    for (rank, safe) in [(2, false), (3, true)] {
        let mut rig = Rig::new(&[]);
        rig.equip(Art::Turbo, Art::Bash);
        rig.rank(Art::Turbo, rank);
        rig.lb(A);
        rig.until(120, NONE, |r| r.m().anim.clip.name == K.art_turbo_loop_clip);
        let hp = rig.m().status.hp;
        rig.hit(AttackClass::Normal, 10.0);
        assert_eq!(rig.m().status.hp == hp, safe, "rank {rank}");
    }
}

#[test]
fn a_ride_off_a_ledge_ends_in_the_fall() {
    let mut rig = Rig::new(&[]);
    rig.equip(Art::Turbo, Art::Bash);
    rig.lb(A);
    rig.until(120, NONE, |r| r.m().anim.clip.name == K.art_turbo_loop_clip);
    rig.world = TestWorld::default();
    rig.until(120, NONE, |r| r.state() != NpcStateId::Pad(ART));
    assert!(rig.a().running.is_none());
    assert!(!rig.m().cmc.gravity_off);
}

// ------------------------------------------------------------------------------------------------ Taunt

#[test]
fn taunt_sets_what_the_hosts_read_for_its_time_at_radius_1000() {
    for (rank, time) in [(1, K.art_taunt_time), (2, K.art_taunt_time_r2), (3, K.art_taunt_time_r3)] {
        let mut rig = Rig::new(&[ENEMY]);
        rig.rank(Art::Taunt, rank);
        let before = rig.mp();
        rig.lb(X);
        assert_eq!(rig.state(), NpcStateId::Pad(TAUNT));
        assert_eq!(rig.m().anim.clip.name, K.art_taunt_clip);
        let t = rig.party.goofy_taunt(rig.id).expect("the taunt");
        assert_eq!(t.radius, 1000.0);
        assert!((t.left - time).abs() < 0.1, "rank {rank}: {t:?}");
        assert_eq!(before - rig.mp(), mp(Art::Taunt, rank as u8));
        rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
    }
    assert_eq!(K.art_taunt_time, 5.0, "the doc's 5 s");
}

#[test]
fn the_taunt_goes_where_he_goes_and_runs_out() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.lb(X);
    rig.until(240, NONE, |r| r.state() == NpcStateId::Idle);
    rig.run(60, Vec2::new(1.0, 0.0), &[]);
    let t = rig.party.goofy_taunt(rig.id).unwrap();
    assert!(horizontal(t.centre - rig.m().position()).length() < 20.0);
    rig.run(400, NONE, &[]);
    assert!(rig.party.goofy_taunt(rig.id).is_none());
}

#[test]
fn only_rank_3_taunt_refills_guard() {
    let after = |rank: u64| {
        let mut rig = Rig::new(&[ENEMY]);
        rig.rank(Art::Taunt, rank);
        rig.mm().status.guard.as_mut().unwrap().value = 10.0;
        rig.lb(X);
        rig.m().status.guard.unwrap().value
    };
    assert!((after(3) - after(2) - K.art_taunt_r3_guard).abs() < 0.5);
}

#[test]
fn taunt_is_no_equipped_art() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.party.set_goofy_arts([Art::Taunt.id(), Art::Charge.id()]);
    assert_eq!(equipped(&rig.m().status, 0), Some(Art::Taunt), "nothing stops the data, kh-core's equip_art does");
    assert!(ids::arts(&rig.m().status.gof_art_book()).iter().all(|a| a.kind != ids::TAUNT));
}

// ------------------------------------------------------------------------------------------------ Interpose

fn at_friend() -> (Rig, MemberId) {
    let mut rig = Rig::new(&[Vec3::new(900.0, 700.0, 0.0)]);
    let friend = rig.friend(Vec3::new(800.0, 300.0, 0.0));
    rig.equip(Art::Interpose, Art::Bash);
    rig.telegraph();
    (rig, friend)
}

fn hit_on(rig: &mut Rig, friend: MemberId, damage: f32) -> HitResult {
    let h = rig.record(AttackClass::Normal, damage);
    rig.party.receive_hit(friend, &h)
}

#[test]
fn interpose_dashes_in_front_of_the_threatened_friend() {
    let (mut rig, friend) = at_friend();
    rig.lb(A);
    assert_eq!(rig.state(), NpcStateId::Pad(ART));
    assert_eq!(rig.m().anim.clip.name, K.art_interpose_clip);
    assert!(invincible_now(&rig), "he goes through what stands in the way");
    let steps = rig.until(120, NONE, |r| r.a().cover.is_some());
    assert!(steps as f32 * DT <= K.art_interpose_dash_max + 0.1);
    let (me, f) = (rig.m().position(), rig.party.member(friend).unwrap().position());
    let gap = horizontal(me - f).length();
    assert!(gap <= K.art_interpose_gap + K.art_interpose_arrive + 30.0, "at the friend's side: {gap}");
    assert!(horizontal(me - Vec3::new(900.0, 700.0, 0.0)).length() < horizontal(f - Vec3::new(900.0, 700.0, 0.0)).length(), "between the friend and the danger");
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD), "shield up");
    assert!(!invincible_now(&rig));
}

fn invincible_now(rig: &Rig) -> bool {
    goofy::invincible(rig.m())
}

#[test]
fn interpose_absorbs_the_next_hit_and_only_that_one() {
    let (mut rig, friend) = at_friend();
    rig.lb(A);
    rig.until(120, NONE, |r| r.a().cover.is_some());
    let hp = rig.party.member(friend).unwrap().status.hp;
    let r = hit_on(&mut rig, friend, 40.0);
    assert!(r.guarded && !r.accepted && r.damage_dealt == 0.0, "absorbed: {r:?}");
    assert_eq!(rig.party.member(friend).unwrap().status.hp, hp, "R14: not a point lands on the friend");
    assert!(rig.logged("interpose"));
    assert_eq!(rig.state(), NpcStateId::Pad(PARRY), "Goofy's side: the parry");
    assert!(rig.m().status.goofy.as_ref().unwrap().core.events.iter().any(|e| matches!(e, crate::goofy::GoofyEvent::Parried { tech: crate::goofy::Tech::Deflect, right: true, .. })), "a deflect of his own (no counter after it: THE USER 2026-11-02)");
    assert!(rig.a().cover.is_none(), "rank 1: one hit");
    let after = hit_on(&mut rig, friend, 40.0);
    assert!(after.damage_dealt > 0.0 || rig.party.member(friend).unwrap().status.hp < hp, "the next one lands: {after:?}");
}

#[test]
fn rank_3_interpose_takes_two_hits() {
    let (mut rig, friend) = at_friend();
    rig.rank(Art::Interpose, 3);
    rig.lb(A);
    rig.until(120, NONE, |r| r.a().cover.is_some());
    assert_eq!(rig.a().cover.unwrap().hits, 2);
    assert!((rig.a().cover.unwrap().left - K.art_interpose_cover_r3).abs() < 0.2);
    assert!(hit_on(&mut rig, friend, 40.0).guarded);
    assert!(rig.a().cover.is_some());
    assert!(hit_on(&mut rig, friend, 40.0).guarded);
    assert!(rig.a().cover.is_none());
}

#[test]
fn the_cover_runs_out_and_needs_him_close() {
    let (mut rig, friend) = at_friend();
    rig.lb(A);
    rig.until(120, NONE, |r| r.a().cover.is_some());
    // he walks off: the cover no longer reaches
    let spot = rig.m().position();
    rig.mm().cmc.location = spot + Vec3::new(K.art_interpose_leash + 200.0, 0.0, 0.0) * -1.0;
    rig.party.member_mut(friend).unwrap().status.hp = 500;
    let r = hit_on(&mut rig, friend, 40.0);
    assert!(!r.guarded, "out of reach: {r:?}");
    assert!(rig.a().cover.is_some(), "the cover stays for when he is back");
    rig.run((K.art_interpose_cover / DT) as usize + 5, NONE, &[]);
    assert!(rig.a().cover.is_none());
}

#[test]
fn interpose_waits_for_a_threatened_friend_and_keeps_the_mp() {
    // no friend at all
    let mut rig = Rig::new(&[ENEMY]);
    rig.equip(Art::Interpose, Art::Bash);
    let before = rig.mp();
    rig.lb(A);
    assert_eq!((rig.state(), rig.mp()), (NpcStateId::Idle, before), "nobody to reach");
    // a friend with no danger near
    let mut rig = Rig::new(&[Vec3::new(1800.0, 0.0, 0.0)]);
    rig.friend(Vec3::new(600.0, 0.0, 0.0));
    rig.equip(Art::Interpose, Art::Bash);
    rig.lb(A);
    assert_eq!(rig.state(), NpcStateId::Idle, "nobody in danger");
    // a threatened friend out of reach
    let mut rig = Rig::new(&[Vec3::new(4000.0, 0.0, 0.0)]);
    rig.friend(Vec3::new(3900.0, 0.0, 0.0));
    rig.equip(Art::Interpose, Art::Bash);
    rig.telegraph();
    rig.lb(A);
    assert_eq!(rig.state(), NpcStateId::Idle, "too far");
}

#[test]
fn interpose_takes_the_nearest_of_two_threatened_friends() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(-1200.0, 0.0, 0.0)]);
    let near = rig.friend(Vec3::new(500.0, 0.0, 0.0));
    let far = rig.party.spawn(Chara::Donald, Vec3::new(-1100.0, 0.0, 0.0), 0.0);
    rig.idle(2);
    rig.equip(Art::Interpose, Art::Bash);
    rig.lb(A);
    assert_eq!(rig.a().running.and_then(|r| r.friend), Some(Friend::Member(near)));
    let _ = far;
}

#[test]
fn bodyguard_waives_the_mp_only_while_a_friend_is_low() {
    for (worn, share, free) in [(true, 0.2, true), (true, 0.5, false), (false, 0.2, false)] {
        let (mut rig, friend) = at_friend();
        if worn {
            rig.wear(GA::Bodyguard);
        }
        let m = rig.party.member_mut(friend).unwrap();
        m.status.hp = (m.status.max_hp() as f32 * share) as i32;
        rig.idle(2);
        let before = rig.mp();
        rig.lb(A);
        assert!(rig.logged("art_interpose"));
        assert_eq!(rig.mp() == before, free, "worn {worn}, share {share}");
    }
}

#[test]
fn bodyguard_reads_sora_too() {
    let (mut rig, _) = at_friend();
    rig.wear(GA::Bodyguard);
    // the view's own HP is the leader's while a friend leads: it says nothing of Sora
    (rig.sora.hp, rig.sora.hp_max) = (10, 100);
    let before = rig.mp();
    rig.lb(A);
    assert!(rig.mp() < before, "his own low HP is no friend's");
    let (mut rig, _) = at_friend();
    rig.wear(GA::Bodyguard);
    rig.sora.ai_sora = Some((MemberId(9), Vec3::new(-4000.0, 0.0, 0.0)));
    rig.sora.ai_sora_hp = 0.1;
    let before = rig.mp();
    rig.lb(A);
    assert_eq!(rig.mp(), before, "AI Sora's own share");
    assert!(rig.logged("art_interpose"));
}

#[test]
fn interpose_covers_ai_sora_through_cover_sora() {
    let mut rig = Rig::new(&[Vec3::new(900.0, 500.0, 0.0)]);
    let sora_at = Vec3::new(800.0, 200.0, 0.0);
    rig.sora.ai_sora = Some((MemberId(9), sora_at));
    rig.equip(Art::Interpose, Art::Bash);
    rig.telegraph();
    rig.lb(A);
    rig.until(120, NONE, |r| r.a().cover.is_some());
    assert_eq!(rig.a().cover.unwrap().friend, Friend::Sora);
    assert!(!rig.party.cover_sora(sora_at + Vec3::X * 3000.0, 40.0, ENEMY, Some(T)), "he stands too far from there");
    assert!(rig.party.cover_sora(sora_at, 40.0, ENEMY, Some(T)));
    assert!(rig.a().cover.is_none());
    assert!(!rig.party.cover_sora(sora_at, 40.0, ENEMY, Some(T)), "the next one is Sora's");
}

#[test]
fn a_covered_hit_with_goofy_down_lands() {
    let (mut rig, friend) = at_friend();
    rig.lb(A);
    rig.until(120, NONE, |r| r.a().cover.is_some());
    rig.mm().status.guard.as_mut().unwrap().break_now();
    rig.mm().enter(NpcStateId::Pad(GUARD_BREAK));
    assert!(!hit_on(&mut rig, friend, 40.0).guarded, "a broken guard parries nothing");
}

// ------------------------------------------------------------------------------------------------ Shield Toss

fn tossed(enemy: &[Vec3]) -> Rig {
    let mut rig = Rig::new(enemy);
    rig.equip(Art::ShieldToss, Art::Bash);
    rig.lb(A);
    assert_eq!(rig.state(), NpcStateId::Pad(ART));
    rig.until(120, NONE, |r| bare_handed(r.m()));
    rig
}

#[test]
fn the_shield_flies_out_hits_and_comes_back_while_he_moves_freely() {
    let mut rig = tossed(&[Vec3::new(800.0, 0.0, 0.0)]);
    assert!(rig.party.thrown_shield(rig.id).is_some() && rig.party.weapon_hidden(rig.id));
    let mut far = 0.0f32;
    for _ in 0..240 {
        if !bare_handed(rig.m()) {
            break;
        }
        far = far.max(horizontal(rig.a().toss.as_ref().unwrap().body.position - rig.m().position()).length());
        rig.step(NONE, &[]);
    }
    assert!(!bare_handed(rig.m()), "it came back");
    assert!(far > 500.0, "it went out: {far}");
    assert_eq!(rig.hits(Art::ShieldToss.row()).len(), 1, "one hit per target");
    assert_eq!(rig.dummies.1[0].info.power, K.art_toss_power);
    assert!(rig.logged("toss_back"));
    assert!(rig.party.thrown_shield(rig.id).is_none() && !rig.party.weapon_hidden(rig.id));
}

#[test]
fn he_leaves_the_throw_with_the_shield_still_flying() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.until(240, Vec2::new(0.0, 1.0), |r| r.state() != NpcStateId::Pad(ART));
    assert!(bare_handed(rig.m()), "the stick took him out, the shield flies on");
    assert_eq!(rig.state(), NpcStateId::Pad(RUN));
}

#[test]
fn a_throw_with_no_target_flies_ahead_and_returns() {
    let mut rig = Rig::new(&[]);
    rig.equip(Art::ShieldToss, Art::Bash);
    rig.lb(A);
    rig.until(120, NONE, |r| bare_handed(r.m()));
    let goal = rig.a().toss.as_ref().unwrap().goal;
    assert!(goal.x > K.art_toss_range * 0.9, "{goal:?}");
    rig.until(240, NONE, |r| !bare_handed(r.m()));
}

#[test]
fn bare_handed_no_parry_works_and_a_block_costs_double() {
    let mut full = Rig::new(&[Vec3::new(1400.0, 0.0, 0.0)]);
    full.run(30, NONE, &[X]);
    let (g0, max) = (full.m().status.guard.unwrap().value, full.m().status.max_hp() as f32);
    full.hit(AttackClass::Normal, max * 0.1);
    let plain = g0 - full.m().status.guard.unwrap().value;

    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.until(120, NONE, |r| r.state() == NpcStateId::Idle);
    assert!(bare_handed(rig.m()));
    // a perfect tap
    rig.step(NONE, &[X]);
    rig.step(NONE, &[]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD), "he can guard");
    let g1 = rig.m().status.guard.unwrap().value;
    let r = rig.hit(AttackClass::Normal, max * 0.1);
    assert!(r.guarded, "the bare-handed guard takes it");
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD_HIT), "a block, no parry");
   let bare = g1 - rig.m().status.guard.unwrap().value;
    assert!((bare - plain * K.art_toss_bare_drain).abs() < 1.0, "half GUARD: {bare} against {plain}");
    assert!(rig.m().status.goofy.as_ref().unwrap().core.resolve.thirds == 0);
}

#[test]
fn bare_handed_the_hold_costs_nothing_and_unblockables_land() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.until(120, NONE, |r| r.state() == NpcStateId::Idle);
    rig.run(3, NONE, &[X]);
    let g0 = rig.m().status.guard.unwrap().value;
    rig.run(60, NONE, &[X]);
    assert_eq!(rig.m().status.guard.unwrap().value, g0, "holding never drains (THE USER 2026-11-02)");
    let hp = rig.m().status.hp;
    rig.hit(AttackClass::Heavy, 20.0);
    assert!(rig.m().status.hp < hp, "the brace needs the shield");
}

#[test]
fn bare_handed_the_arts_of_the_shield_wait_but_taunt_does_not() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.until(120, NONE, |r| r.state() == NpcStateId::Idle);
    let before = rig.mp();
    rig.lb(B);
    rig.lb(A);
    assert_eq!((rig.state(), rig.mp()), (NpcStateId::Idle, before), "no Bash and no second throw");
    rig.lb(X);
    assert_eq!(rig.state(), NpcStateId::Pad(TAUNT));
}

#[test]
fn bare_handed_ally_guard_and_the_cover_do_not_parry() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    let h = rig.record(AttackClass::Normal, 10.0);
    assert!(!goofy::parry_for_friend(rig.mm(), &h));
}

#[test]
fn the_parry_comes_back_with_the_shield() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.until(240, NONE, |r| !bare_handed(r.m()));
    rig.step(NONE, &[X]);
    rig.step(NONE, &[]);
    rig.hit(AttackClass::Normal, 10.0);
    assert_eq!(rig.state(), NpcStateId::Pad(PARRY));
}

#[test]
fn a_target_that_vanishes_sends_the_shield_home() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.idle(4);
    rig.dummies.0.clear();
    rig.until(240, NONE, |r| !bare_handed(r.m()));
    assert_eq!(rig.state(), NpcStateId::Idle);
}

#[test]
fn it_always_ends_by_its_own_clock() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.idle(4);
    // he is warped far off, as a host's warp can
    rig.mm().cmc.location = Vec3::new(-40000.0, 0.0, 0.0);
    let n = rig.until(((K.art_toss_max_time + 0.5) / DT) as usize, NONE, |r| !bare_handed(r.m()));
    assert!(n as f32 * DT <= K.art_toss_max_time + 0.1, "{n}");
    assert!(rig.logged("toss_back"));
}

#[test]
fn a_hit_that_stops_him_mid_throw_leaves_nothing_behind() {
    let mut rig = Rig::new(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.equip(Art::ShieldToss, Art::Bash);
    rig.lb(A);
    rig.hit(AttackClass::Normal, 30.0);
    rig.until(300, NONE, |r| r.state() == NpcStateId::Idle);
    assert!(!bare_handed(rig.m()), "it never left his hand");
    assert!(rig.a().running.is_none());
    rig.run(60, NONE, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD));
}

#[test]
fn a_hit_while_it_flies_stops_nothing() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.hit(AttackClass::Normal, 30.0);
    rig.until(300, NONE, |r| !bare_handed(r.m()));
    rig.until(300, NONE, |r| r.state() == NpcStateId::Idle);
}

#[test]
fn it_is_back_in_his_hand_when_he_goes_down() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    let hp = rig.m().status.hp as f32;
    rig.hit(AttackClass::Normal, hp + 10.0);
    rig.until(60, NONE, |r| !bare_handed(r.m()));
}

#[test]
fn only_rank_3_bounces_and_only_once() {
    let enemies = [Vec3::new(700.0, 0.0, 0.0), Vec3::new(700.0, 500.0, 0.0), Vec3::new(700.0, 900.0, 0.0)];
    for (rank, hits) in [(2, 1), (3, 2)] {
        let mut rig = Rig::new(&enemies);
        rig.equip(Art::ShieldToss, Art::Bash);
        rig.rank(Art::ShieldToss, rank);
        rig.lb(A);
        rig.until(120, NONE, |r| bare_handed(r.m()));
        rig.until(300, NONE, |r| !bare_handed(r.m()));
        assert_eq!(rig.hits(Art::ShieldToss.row()).len(), hits, "rank {rank}");
    }
}

// ------------------------------------------------------------------------------------------------ AI Goofy, the Customize feed

/// One step of an AI Goofy whose drive holds these buttons (what GF-AI's brain fills).
fn ai_step(rig: &mut Rig, held: &[PadButton], last: &mut PadButtons, stick: f32) {
    let mut now = PadButtons::NONE;
    for b in held {
        now.set(*b, true);
    }
    let pad = kh_core::input::PadState { held: now, pressed: PadButtons(now.0 & !last.0), released: PadButtons(last.0 & !now.0) };
    *last = now;
    let k = rig.s();
    k.run.ai.pad = pad;
    k.run.ai.stick_yaw = 0.0;
    k.run.ai.stick_magnitude = stick;
    let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
    rig.party.step(DT, &rig.sora, &mut host);
}

#[test]
fn an_ai_goofy_plays_them_through_his_buttons() {
    let mut rig = Rig::new(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.equip(Art::ShieldToss, Art::Bash);
    rig.party.set_pad(rig.id, None);
    rig.party.gof.kh3_ai = true; // the drive is the test's, not the brain's
    let mut last = PadButtons::NONE;
    ai_step(&mut rig, &[], &mut last, 0.0);
    assert!(rig.m().pad.is_none());
    let before = rig.mp();
    ai_step(&mut rig, &[LB], &mut last, 0.0);
    ai_step(&mut rig, &[LB, A], &mut last, 0.0);
    assert!(rig.logged("art_toss"), "{:?}", rig.k().log);
    assert_eq!(before - rig.mp(), mp(Art::ShieldToss, 1));
    for _ in 0..600 {
        ai_step(&mut rig, &[], &mut last, 0.0);
    }
    assert!(rig.logged("toss_back"), "{:?}", rig.k().log);
    assert_eq!(rig.state(), NpcStateId::Idle);
}

#[test]
fn an_ai_goofy_interposes_for_the_leader() {
    let mut rig = Rig::new(&[Vec3::new(900.0, 500.0, 0.0)]);
    rig.equip(Art::Interpose, Art::Bash);
    rig.party.set_pad(rig.id, None);
    rig.party.gof.kh3_ai = true; // the drive is the test's, not the brain's
    rig.sora.position = Vec3::new(800.0, 200.0, 0.0);
    (rig.sora.hp, rig.sora.hp_max) = (100, 100);
    rig.telegraph();
    let mut last = PadButtons::NONE;
    ai_step(&mut rig, &[], &mut last, 0.0);
    ai_step(&mut rig, &[LB], &mut last, 0.0);
    ai_step(&mut rig, &[LB, A], &mut last, 0.0);
    assert!(rig.logged("art_interpose"), "{:?}", rig.k().log);
    for _ in 0..120 {
        ai_step(&mut rig, &[], &mut last, 0.0);
    }
    assert_eq!(rig.a().cover.map(|c| c.friend), Some(Friend::Sora));
    assert!(rig.party.cover_sora(Vec3::new(800.0, 200.0, 0.0), 30.0, Vec3::new(900.0, 500.0, 0.0), Some(T)));
}

#[test]
fn before_the_first_feed_he_holds_the_first_two_arts_he_knows() {
    let rig = Rig::new(&[]);
    assert_eq!((equipped(&rig.m().status, 0), equipped(&rig.m().status, 1)), (Some(Art::Charge), Some(Art::Tornado)));
    assert_eq!(art_moves(&rig.m().status), [Some(GoofyMove::GoofyCharge), Some(GoofyMove::GoofyTornado)]);
    assert_eq!(equipped(&rig.m().status, 2), None);
}

#[test]
fn the_panel_row_reads_rank_and_mp() {
    let mut rig = Rig::new(&[]);
    let s = &rig.m().status;
    let row = panel_row(s, ids::BASH);
    assert_eq!((row.name, row.enabled, row.mp_short), ("Goofy Bash", true, false));
    rig.mm().status.mp = 0; // (THE USER 2026-11-02) short only at 0 MP (1 MP buys it: crate::mp_rule)
    assert!(panel_row(&rig.m().status, ids::BASH).mp_short);
    rig.mm().status.mp_charge_left = 3.0;
    assert!(!panel_row(&rig.m().status, ids::BASH).enabled);
    rig.mm().status.level = 1;
    assert!(!panel_row(&rig.m().status, ids::BASH).enabled, "not learned");
    assert_eq!(panel_row(&rig.m().status, 0x41), CommandRow::default());
}

// ------------------------------------------------------------------------------------------------ (GF-POLISH) while the shield is out (8)

#[test]
fn with_the_shield_out_a_does_nothing_on_the_ground_or_in_the_air_and_works_once_it_is_back() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.until(240, NONE, |r| r.state() != NpcStateId::Pad(ART));
    assert!(bare_handed(rig.m()));
    rig.step(NONE, &[A]);
    rig.step(NONE, &[]);
    rig.run(6, NONE, &[]);
    assert_ne!(rig.state(), NpcStateId::Pad(COMBO), "no shove");
    // in the air: the jump is his, the air shove is not
    rig.step(NONE, &[B]);
    rig.run(10, NONE, &[]);
    assert!(!rig.m().on_ground());
    rig.step(NONE, &[A]);
    rig.step(NONE, &[]);
    assert_ne!(rig.state(), NpcStateId::Pad(combo::AIR_COMBO), "no air shove");
    rig.until(((K.art_toss_max_time + 1.0) / DT) as usize, NONE, |r| !bare_handed(r.m()) && r.m().on_ground());
    rig.run(12, NONE, &[]);
    rig.step(NONE, &[A]);
    rig.step(NONE, &[]);
    assert_eq!(rig.state(), NpcStateId::Pad(COMBO), "the shield back, the shove back");
}

#[test]
fn with_the_shield_out_the_breaker_is_not_offered_and_y_does_not_start_it() {
    use super::super::goofy_breaker::{breaker_offered, BrokenMark};
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.until(240, NONE, |r| r.state() != NpcStateId::Pad(ART));
    assert!(bare_handed(rig.m()));
    let mark = BrokenMark { target: T, position: rig.m().position() + Vec3::X * 100.0, radius: 50.0, left: 9.0 };
    rig.s().run.breaker.marks = vec![mark];
    assert!(!breaker_offered(rig.m()));
    rig.until(((K.art_toss_max_time + 1.0) / DT) as usize, NONE, |r| !bare_handed(r.m()));
    rig.s().run.breaker.marks = vec![mark];
    assert!(breaker_offered(rig.m()), "offered again once it is back");
}

#[test]
fn with_the_shield_out_he_still_guards_and_hops() {
    let mut rig = tossed(&[Vec3::new(1400.0, 0.0, 0.0)]);
    rig.until(240, NONE, |r| r.state() != NpcStateId::Pad(ART));
    rig.run(10, NONE, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD), "the one thing left");
    assert!(bare_handed(rig.m()));
}

// the cap of the brief: the shield cannot lock him out for long
const _: () = assert!(K.art_toss_max_time <= 3.0);

#[test]
fn the_shield_can_never_lock_him_out_for_long() {
    let mut rig = tossed(&[]);
    let n = rig.until(((K.art_toss_max_time + 0.1) / DT) as usize, NONE, |r| !bare_handed(r.m()));
    assert!(n as f32 * DT <= K.art_toss_max_time + 0.05);
}

// ------------------------------------------------------------------------------------------------ (COMBAT-FIX) KH3's shapes
// THE USER 2026-11-06: "Goofy's ground combo hit box also extends out way too far"; the coordinator 2026-11-07: KH3 1:1 for
// every one of his attacks. The dummy's body is a 60 cm capsule (z 30 - 150).

/// Runs `art` at `rank` with one enemy kept at `at` in his frame (x ahead, y right) every step: the hits it took and the far
/// edge of the Art's live shapes from his feet (centre + radius: the prism's circumradius, the capsule's radius).
fn art_reach(art: Art, rank: u64, at: Vec2) -> (usize, f32) {
    let mut rig = Rig::new(&[Vec3::new(at.x, at.y, 0.0)]);
    rig.equip(art, if art == Art::Bash { Art::Charge } else { Art::Bash });
    rig.rank(art, rank);
    rig.lb(A);
    let mut reach = 0.0f32;
    for _ in 0..400 {
        rig.step(NONE, &[]);
        let (me, yaw) = (rig.m().position(), rig.m().yaw);
        let f = forward(yaw);
        rig.dummies.0[0].1 = me + f * at.x + Vec3::new(-f.y, f.x, 0.0) * at.y;
        for a in rig.k().shots.iter().filter(|a| a.tag == TAG) {
            // his frame, measured on his pose of this step: the shape of a group, else the sphere's offset
            let off = match (a.group, a.follow) {
                (Some(grp), _) => {
                    let s = crate::actions::attack_shape(rig.m(), grp);
                    horizontal((s.a + s.b) * 0.5 - me).length()
                }
                (None, Some(f)) => Vec2::new(f.x, f.y).length(),
                (None, None) => horizontal(a.centre - me).length(),
            };
            reach = reach.max(off + a.radius());
        }
        if rig.state() == NpcStateId::Idle {
            break;
        }
    }
    (rig.hits(art.row()).len(), reach)
}

/// Tornado: KH3's Attack_Tornado prism round him (circumradius 130), not a 280 sphere: an enemy 1.8 m to his side is hit,
/// one 2 m away is not (the old sphere reached 3.4 m).
#[test]
fn tornado_reaches_only_kh3s_prism() {
    let (near, reach) = art_reach(Art::Tornado, 1, Vec2::new(0.0, 180.0));
    assert!(near >= 1, "1.8 m to his side is in the spin");
    assert!((reach - crate::actions::TORNADO_HULL.0).abs() < 1.0, "the prism's circumradius: {reach}");
    assert_eq!(art_reach(Art::Tornado, 1, Vec2::new(0.0, 200.0)).0, 0, "2 m to his side is out of it");
}

/// Bash: the Trinity Guard pawn's FR_SHIELDGUARDIAN_HEAVYBASH, a 150 sphere 80 ahead and 150 up (230 cm of reach), not a 240
/// sphere 140 ahead (380): an enemy 2.8 m ahead is hit, one 3 m ahead or 1.5 m behind is not.
#[test]
fn bash_reaches_only_kh3s_heavy_bash_sphere() {
    let (near, reach) = art_reach(Art::Bash, 1, Vec2::new(280.0, 0.0));
    assert_eq!(near, 1, "2.8 m ahead is in the slam");
    assert!((reach - 230.0).abs() < 1.0, "80 ahead + 150: {reach}");
    assert_eq!(art_reach(Art::Bash, 1, Vec2::new(300.0, 0.0)).0, 0, "3 m ahead is out of it");
    assert_eq!(art_reach(Art::Bash, 1, Vec2::new(-150.0, 0.0)).0, 0, "1.5 m behind is out of it");
}

/// Turbo: KH3's Attack_Body capsule (radius 80, upright on his centre, which sways 5 behind to 19 ahead of him in M_1325),
/// not a 190 sphere 60 ahead (310 cm with the dummy's body): an enemy riding 1.3 m ahead is hit, one 2 m ahead is not.
#[test]
fn turbo_reaches_only_kh3s_body_capsule() {
    let (near, reach) = art_reach(Art::Turbo, 1, Vec2::new(130.0, 0.0));
    assert!(near >= 1, "1.3 m ahead is in the ride");
    assert!(reach > 80.0 && reach <= 100.0, "the capsule's radius on his swaying centre: {reach}");
    assert_eq!(art_reach(Art::Turbo, 1, Vec2::new(200.0, 0.0)).0, 0, "2 m ahead is out of it");
}

/// Charge: SGuardWalk opens no attack in KH3, so the rush hits with the shield's own SPHERE 50 (group "None") on SGuardWalk's
/// shield track (46 - 52 cm ahead, the socket offset placed), not a 170 sphere 110 ahead (340 cm with the dummy's body): an enemy kept 1.2 m ahead is
/// shoved, one 1.9 m ahead is not.
#[test]
fn charge_reaches_only_the_shield() {
    let (near, reach) = art_reach(Art::Charge, 1, Vec2::new(120.0, 0.0));
    assert_eq!(near, 1, "1.2 m ahead meets the shield");
    assert!(reach > 90.0 && reach <= 103.0, "the shield's sphere: {reach}");
    assert_eq!(art_reach(Art::Charge, 1, Vec2::new(190.0, 0.0)).0, 0, "1.9 m ahead never meets it");
}

/// Shield Toss: the flying shield is KH3's Attack_Discus SPHERE 50, not 110: thrown at an enemy 8 m ahead it hits a bystander
/// 1 m beside its path (and, as any first hit, turns home from it), but passes one 1.3 m beside it and hits the enemy (the old
/// sphere reached 1.7 m).
#[test]
fn the_tossed_shield_is_kh3s_50_cm_sphere() {
    let hits = |side: f32| {
        let mut rig = Rig::new(&[Vec3::new(800.0, 0.0, 0.0), Vec3::new(400.0, side, 0.0)]);
        rig.equip(Art::ShieldToss, Art::Bash);
        rig.mm().pad.as_mut().unwrap().lock = T; // thrown at the far enemy, past the bystander
        rig.lb(A);
        rig.until(120, NONE, |r| bare_handed(r.m()));
        rig.until(240, NONE, |r| !bare_handed(r.m()));
        rig.dummies.1.iter().map(|h| h.target).collect::<Vec<_>>()
    };
    assert_eq!(K.art_toss_radius, 50.0);
    assert_eq!(hits(100.0), [TargetId(2)], "1 m beside the path is in reach");
    assert_eq!(hits(130.0), [T], "1.3 m beside the path is out of reach: the shield flies on to the enemy");
}

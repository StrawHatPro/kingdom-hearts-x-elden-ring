//! (GF-BREAK) The Stagger seam, BROKEN and Goofy Breaker through the party's step (documentation/GOOFY_OVERHAUL.md 5, 6.3; PLAN.md
//! R6, R7, R13). The book's own rules are tested in goofy/stagger.rs.

use super::super::PadFrame;
use super::*;
use crate::goofy::{GoofyAbilities, GoofyEvent, StaggerWhy};
use crate::{Chara, PartyConfig};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetInfo, Targets};
use kh_core::input::{PadButton, PadButtons};
use kh_core::test_world::TestWorld;
use kh_core::Tuning;

const DT: f32 = 1.0 / 60.0;
const T: TargetId = TargetId(1);
const Y: PadButton = PadButton::FaceTop;

/// One enemy with an HP and a size, and every question the book asked.
struct Foe {
    at: Vec3,
    hp: f32,
    hp_max: f32,
    size: u8,
    alive: bool,
    lockable: bool,
    hits: Vec<HitRecord>,
    asks: Vec<StaggerAsk>,
    /// What the host answers to `Break` / `MayDefeat`.
    may_break: bool,
    may_defeat: bool,
}

impl Foe {
    fn info_of(&self) -> TargetInfo {
        let mut flags = TargetFlags::HITTABLE | TargetFlags::ENEMY;
        if self.alive {
            flags |= TargetFlags::ALIVE;
        }
        if self.lockable {
            flags |= TargetFlags::LOCKABLE;
        }
        TargetInfo { id: T, position: self.at, center: self.at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(flags), hp: self.hp, hp_max: self.hp_max }
    }
}

impl Targets for Foe {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.push(self.info_of());
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        (id == T).then(|| self.info_of())
    }
    fn hurt_shapes(&self, _id: TargetId, out: &mut Vec<HurtShape>) {
        out.push(HurtShape { a: self.at + Vec3::Z * 30.0, b: self.at + Vec3::Z * 150.0, radius: 60.0, part: 0 });
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.hits.push(*hit);
        let dealt = hit.damage.min(self.hp);
        self.hp -= dealt;
        self.alive = self.hp > 0.0;
        HitResult { accepted: true, damage_dealt: dealt, hp_after: self.hp, killed: !self.alive, guarded: false }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: self.hp_max as i32, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
    fn size_class(&self, _id: TargetId) -> u8 {
        self.size
    }
    fn stagger_ask(&mut self, _id: TargetId, ask: StaggerAsk) -> bool {
        self.asks.push(ask);
        match ask {
            StaggerAsk::Break { .. } => self.may_break,
            StaggerAsk::MayDefeat => self.may_defeat,
            _ => true,
        }
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
    foe: Foe,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    id: MemberId,
    held: PadButtons,
    events: Vec<GoofyEvent>,
}

impl Rig {
    fn new(size: u8) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        let foe = Foe { at: Vec3::new(200.0, 0.0, 0.0), hp: 10_000.0, hp_max: 10_000.0, size, alive: true, lockable: true, hits: Vec::new(), asks: Vec::new(), may_break: true, may_defeat: true };
        let mut rig = Rig { world: TestWorld::floor(), foe, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), id, held: PadButtons::NONE, events: Vec::new() };
        rig.sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        rig.sora.hp_max = 1000;
        rig.wear(&[]);
        rig.idle(10);
        rig
    }

    /// Exactly these abilities worn.
    fn wear(&mut self, list: &[GA]) {
        let mut a = GoofyAbilities::default();
        for k in GA::ALL {
            a.give(k);
        }
        for k in list {
            a.set_equipped(*k, true);
        }
        self.m().status.goofy.as_mut().unwrap().abilities = a;
    }

    fn m(&mut self) -> &mut NpcPawn {
        self.party.member_mut(self.id).unwrap()
    }

    fn step(&mut self, held: &[PadButton]) -> crate::PartyOutput {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = kh_core::input::PadState { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        self.party.set_pad_frame(self.id, PadFrame { stick: Vec2::ZERO, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &self.world, targets: &mut self.foe, audio: &mut self.audio, effects: &mut self.effects };
        let out = self.party.step(DT, &self.sora, &mut host);
        self.events.extend(self.party.gof.events.iter().map(|e| e.1));
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    /// Stagger through the seam the core's rules use.
    fn stagger(&mut self, amount: f32) {
        self.m().status.goofy.as_mut().unwrap().core.events.push(GoofyEvent::Stagger { target: T, amount, why: StaggerWhy::Deflect });
        self.step(&[]);
    }

    fn broken(&self) -> bool {
        self.party.stagger_view(T).is_some_and(|v| v.broken.is_some())
    }

    fn press_y(&mut self) {
        self.step(&[Y]);
        self.step(&[]);
    }

    /// Runs until he is out of the Breaker.
    fn finish(&mut self) {
        for _ in 0..400 {
            if self.m().state != NpcStateId::Pad(BREAKER) {
                return;
            }
            self.step(&[]);
        }
        panic!("the Breaker never ended");
    }

    fn slam(&self) -> Option<(bool, bool, bool, f32)> {
        self.events.iter().find_map(|e| match e {
            GoofyEvent::BreakerLanded { defeat, launched, rally, dealt, .. } => Some((*defeat, *launched, *rally, *dealt)),
            _ => None,
        })
    }

    /// Full gauge, Y, the whole Breaker: the hit that landed.
    fn breaker(&mut self) -> HitRecord {
        self.stagger(1000.0);
        assert!(self.broken());
        self.press_y();
        assert_eq!(self.m().state, NpcStateId::Pad(BREAKER));
        self.finish();
        *self.foe.hits.last().expect("the slam's hit")
    }
}

#[test]
fn every_stagger_on_the_seam_lands_on_its_target_and_a_full_gauge_breaks() {
    let mut r = Rig::new(1);
    r.stagger(40.0);
    let v = r.party.stagger_view(T).unwrap();
    assert_eq!((v.value, v.max, v.broken), (40.0, K.stagger_gauge, None));
    assert_eq!(v.share(), 0.4);
    r.stagger(60.0);
    assert_eq!(r.foe.asks, vec![StaggerAsk::Break { window: 2.5 }], "one explicit event");
    assert!(r.broken());
    assert!(r.events.iter().any(|e| matches!(e, GoofyEvent::Broken { target } if *target == T)));
    // only what the lock-on accepts has a gauge: not a pot, not a hidden body
    let mut r = Rig::new(1);
    r.foe.lockable = false;
    r.stagger(1000.0);
    assert_eq!(r.party.stagger_view(T), None);
    assert!(r.foe.asks.is_empty());
}

#[test]
fn the_window_runs_out_the_host_is_told_and_y_goes_back_to_the_team_rows() {
    let mut r = Rig::new(1);
    r.stagger(1000.0);
    r.idle(140);
    assert!(r.broken() && r.party.goofy_breaker_offered(r.id));
    r.idle(25);
    assert!(!r.broken());
    assert!(!r.party.goofy_breaker_offered(r.id));
    assert_eq!(r.foe.asks.last(), Some(&StaggerAsk::Release { juggle: 0.0 }));
    r.press_y();
    assert_ne!(r.m().state, NpcStateId::Pad(BREAKER));
    // a boss: 1.5 s, then half
    let mut r = Rig::new(4);
    r.stagger(1000.0);
    assert_eq!(r.foe.asks, vec![StaggerAsk::Break { window: 1.5 }]);
    r.idle(95);
    assert!(!r.broken());
    assert_eq!(r.party.stagger_view(T).unwrap().value, K.stagger_gauge_boss * 0.5);
}

#[test]
fn a_host_that_refuses_keeps_it_from_breaking() {
    let mut r = Rig::new(1);
    r.foe.may_break = false;
    r.stagger(1000.0);
    assert!(!r.broken());
    assert_eq!(r.party.stagger_view(T).unwrap().value, K.stagger_gauge * K.stagger_refused_share);
    r.press_y();
    assert_ne!(r.m().state, NpcStateId::Pad(BREAKER));
}

#[test]
fn y_needs_reach_and_a_lockable_body() {
    let mut r = Rig::new(1);
    r.foe.at = Vec3::new(800.0, 0.0, 0.0);
    r.stagger(1000.0);
    assert!(r.broken() && !r.party.goofy_breaker_offered(r.id), "too far");
    let mut r = Rig::new(1);
    r.stagger(1000.0);
    r.idle(1);
    assert!(r.party.goofy_breaker_offered(r.id));
    r.foe.lockable = false;
    r.idle(1);
    assert!(!r.party.goofy_breaker_offered(r.id), "hidden from the lock-on");
}

#[test]
fn the_breaker_is_wind_up_spin_slam_at_five_times_and_he_cannot_be_hit() {
    let mut r = Rig::new(1);
    r.stagger(1000.0);
    r.step(&[Y]);
    assert!(r.m().pad.as_ref().is_some_and(|d| d.x2.y_taken), "R13: ahead of the team rows");
    r.step(&[]);
    assert_eq!(r.m().anim.clip.name, K.breaker_clip_windup);
    assert!(r.m().status.goofy.as_ref().map(|s| s.run.hold_invincible).unwrap());
    assert!(matches!(r.foe.asks.last(), Some(StaggerAsk::Hold { seconds }) if *seconds > 1.5), "{:?}", r.foe.asks);
    let mut seen = vec![r.m().anim.clip.name];
    while r.foe.hits.is_empty() {
        r.step(&[]);
        let n = r.m().anim.clip.name;
        if seen.last() != Some(&n) {
            seen.push(n);
        }
        assert!(seen.len() <= 3);
    }
    assert_eq!(seen, vec![K.breaker_clip_windup, K.breaker_clip_spin, K.breaker_clip_slam]);
    assert_eq!(r.m().anim.clip.attack_windows().count(), 0, "no window of its own");
    r.finish();
    let hit = r.foe.hits[0];
    assert_eq!(hit.info.power, K.breaker_power);
    assert_eq!(hit.attack_name, "GOOFY_BREAKER");
    assert_eq!(hit.damage_kind, Kind::Blow);
    assert_eq!(hit.flags & kh_core::host::HIT_FLAG_BREAKER, 0, "healthy: no defeat");
    assert_eq!(r.foe.hits.len(), 1);
    assert!(!r.broken(), "the slam ends BROKEN");
    assert_eq!(r.party.stagger_view(T).map_or(0.0, |v| v.value), 0.0);
    assert!(matches!(r.foe.asks.last(), Some(StaggerAsk::Release { juggle }) if *juggle == 0.0));
    assert!(!r.m().status.goofy.as_ref().map(|s| s.run.hold_invincible).unwrap());
    // the kneel is released before the hit lands
    assert!(r.foe.asks.iter().filter(|a| matches!(a, StaggerAsk::Release { .. })).count() == 1);
}

#[test]
fn the_breaker_outlives_a_short_window() {
    // a boss's 1.5 s is shorter than the move: pressed late, the hold carries it to the slam
    let mut r = Rig::new(4);
    r.stagger(1000.0);
    r.idle(80);
    assert!(r.broken());
    r.press_y();
    r.finish();
    assert!(r.slam().is_some());
    assert_eq!(r.foe.asks.iter().filter(|a| matches!(a, StaggerAsk::Release { .. })).count(), 1, "only the slam released it");
    assert_eq!(r.party.stagger_view(T).unwrap().value, K.stagger_gauge_boss * 0.5);
}

#[test]
fn it_defeats_below_forty_percent_on_both_sides_of_the_line() {
    let mut r = Rig::new(2);
    r.foe.hp = 3900.0;
    let hit = r.breaker();
    assert_ne!(hit.flags & kh_core::host::HIT_FLAG_BREAKER, 0);
    assert_eq!(hit.flags & kh_core::host::HIT_FLAG_FORCE, 0, "not Zettaflare's flag");
    assert!(hit.damage >= 3900.0);
    assert!(!r.foe.alive);
    assert_eq!(r.slam().map(|s| s.0), Some(true));
    // at 40 % and above: only the damage
    for hp in [4000.0, 4100.0] {
        let mut r = Rig::new(2);
        r.foe.hp = hp;
        let hit = r.breaker();
        assert_eq!(hit.flags & kh_core::host::HIT_FLAG_BREAKER, 0, "{hp}");
        assert_eq!(r.slam().map(|s| s.0), Some(false));
        assert!(hit.damage < hp, "{}", hit.damage);
    }
}

#[test]
fn a_boss_is_never_defeated_and_a_no_death_state_says_no() {
    let mut r = Rig::new(4);
    r.foe.hp = 100.0;
    r.foe.hp_max = 100_000.0;
    let hit = r.breaker();
    assert_eq!(hit.flags & kh_core::host::HIT_FLAG_BREAKER, 0);
    assert!(!r.foe.asks.contains(&StaggerAsk::MayDefeat), "the rule never asks for a boss");
    assert_eq!(r.slam().map(|s| s.0), Some(false));
    // the host's no (zdefer::no_dead_of, an event): the slam only
    let mut r = Rig::new(1);
    r.foe.hp = 1000.0;
    r.foe.may_defeat = false;
    let hit = r.breaker();
    assert!(r.foe.asks.contains(&StaggerAsk::MayDefeat));
    assert_eq!(hit.flags & kh_core::host::HIT_FLAG_BREAKER, 0);
    assert_eq!(r.slam().map(|s| s.0), Some(false));
}

#[test]
fn deep_breaker_and_in_the_zone_multiply() {
    let mut r = Rig::new(1);
    r.wear(&[GA::DeepBreaker]);
    assert!((r.breaker().info.power - K.breaker_power * K.breaker_deep_mult).abs() < 1e-3);
    let mut r = Rig::new(1);
    r.m().status.goofy.as_mut().unwrap().core.resolve.thirds = crate::goofy::RESOLVE_FULL;
    r.wear(&[GA::DeepBreaker]);
    r.stagger(1000.0);
    r.press_y();
    assert!(r.m().status.goofy.as_ref().unwrap().core.resolve.in_zone());
    r.finish();
    assert!((r.foe.hits[0].info.power - K.breaker_power * K.breaker_deep_mult * K.resolve_zone_breaker).abs() < 1e-3);
}

#[test]
fn launch_breaker_tosses_what_it_may_and_opens_the_juggle() {
    let mut r = Rig::new(1);
    r.wear(&[GA::LaunchBreaker]);
    let hit = r.breaker();
    assert_eq!(hit.damage_kind, Kind::Toss);
    assert_eq!(r.slam().map(|s| s.1), Some(true));
    assert!(r.foe.asks.contains(&StaggerAsk::Release { juggle: K.breaker_launch_window }), "the host's poise window: {:?}", r.foe.asks);
    // bosses and large bodies: the launch simply does not happen
    for size in [3, 4] {
        let mut r = Rig::new(size);
        r.wear(&[GA::LaunchBreaker]);
        let hit = r.breaker();
        assert_eq!(hit.damage_kind, Kind::Blow, "size {size}");
        assert_eq!(r.slam().map(|s| s.1), Some(false));
        assert!(r.foe.asks.contains(&StaggerAsk::Release { juggle: 0.0 }));
    }
    // a body it defeats is not thrown
    let mut r = Rig::new(1);
    r.wear(&[GA::LaunchBreaker]);
    r.foe.hp = 1000.0;
    assert_eq!(r.breaker().damage_kind, Kind::Blow);
}

#[test]
fn rally_breaker_heals_ten_percent_of_each_ones_own_max_hp() {
    let mut r = Rig::new(1);
    let don = r.party.spawn(Chara::Donald, Vec3::new(-100.0, 0.0, 0.0), 0.0);
    r.idle(2);
    r.wear(&[GA::RallyBreaker]);
    r.m().status.hp = 10;
    r.party.status_mut(don).unwrap().hp = 10;
    let (gmax, dmax) = (r.m().status.max_hp(), r.party.status(don).unwrap().max_hp());
    r.breaker();
    assert_eq!(r.m().status.hp, 10 + ((gmax as f32 * K.breaker_rally_heal).round() as i32));
    assert_eq!(r.party.status(don).unwrap().hp, 10 + ((dmax as f32 * K.breaker_rally_heal).round() as i32));
    // Goofy leads here: "the player's" heal would be his again, and no AI Sora stands by
    assert_eq!(r.step(&[]).sora_heals.iter().filter(|h| h.amount > 0).count(), 0);
    // Sora leading (an AI Goofy's Breaker): through the host's heal, 10 % of his own
    let mut party = Party::new(PartyConfig::test_level99());
    let g = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
    let sora = SoraView { hp_max: 1000, ..SoraView::default() };
    rally(&mut party, g, &sora);
    let m = party.member(g).unwrap();
    assert!(m.requests.iter().any(|q| matches!(q, Request::Heal { target: TargetRef::Sora, amount: 100, .. })));
}

#[test]
fn party_breaker_brings_the_friends_near_the_enemy() {
    let mut r = Rig::new(1);
    r.party.spawn(Chara::Donald, Vec3::new(0.0, 300.0, 0.0), 0.0);
    r.idle(2);
    r.wear(&[GA::PartyBreaker]);
    r.breaker();
    let names: Vec<&str> = r.foe.hits.iter().map(|h| h.attack_name).collect();
    assert_eq!(names, vec!["GOOFY_BREAKER_PARTY", "GOOFY_BREAKER"], "the friend first, the slam last");
    // too far: no
    let mut r = Rig::new(1);
    r.party.spawn(Chara::Donald, Vec3::new(0.0, 5000.0, 0.0), 0.0);
    r.idle(2);
    r.wear(&[GA::PartyBreaker]);
    r.breaker();
    assert_eq!(r.foe.hits.len(), 1);
    // without the ability: no
    let mut r = Rig::new(1);
    r.party.spawn(Chara::Donald, Vec3::new(0.0, 300.0, 0.0), 0.0);
    r.idle(2);
    r.breaker();
    assert_eq!(r.foe.hits.len(), 1);
}

#[test]
fn it_stops_when_the_enemy_dies_under_it() {
    let mut r = Rig::new(1);
    r.stagger(1000.0);
    r.press_y();
    r.foe.alive = false;
    r.idle(3);
    assert_ne!(r.m().state, NpcStateId::Pad(BREAKER));
    assert!(r.foe.hits.is_empty());
    assert!(!r.m().status.goofy.as_ref().map(|s| s.run.hold_invincible).unwrap());
}

#[test]
fn the_debug_word_fills_a_gauge() {
    let mut r = Rig::new(1);
    let said = r.party.goofy_word(&["stagger", "1", "999"]);
    assert!(said.contains("target 1"), "{said}");
    r.idle(1);
    assert!(r.broken());
    assert!(r.party.goofy_word(&["stagger"]).contains("BROKEN"));
}

/// R17: a misspelled clip silently plays 0.5 s of nothing (`tables.rs`).
#[test]
fn the_clips_rows_name_real_clips() {
    for c in [K.breaker_clip_windup, K.breaker_clip_spin, K.breaker_clip_slam, K.breaker_party_clip] {
        assert_eq!(crate::tables::clip(Chara::Goofy, c).name, c);
    }
    assert_eq!(crate::tables::clip(Chara::Donald, K.breaker_party_clip).name, K.breaker_party_clip);
    assert!(Kind::from_game_name(K.breaker_kind).is_some() && Kind::from_game_name(K.breaker_launch_kind).is_some() && Kind::from_game_name(K.breaker_party_kind).is_some());
    assert!(shots::clip(K.breaker_clip_slam).length > K.breaker_hit);
    assert!(to_slam() < 3.0, "{}", to_slam());
}

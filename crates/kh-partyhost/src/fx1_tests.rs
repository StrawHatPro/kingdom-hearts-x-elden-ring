//! (FX1) A friend leader's finisher never vanishes (build/sora_tmp/er/party/s0/REBASELINE.md, the finding of tour 2's
//! second run): the full loop of the test level's step (route -> Sora's own step -> the party step) with the real
//! pad kits of Donald and Goofy and Sora's real core.
//! - THE GATE: AI Sora busy (mid-attack) when the leader's combo wants its finisher -> no roll, "no" at once, the
//!   friend's own finisher plays;
//! - THE HAPPY PATH: AI Sora idle beside him -> "yes", the team move starts within the latch (no late "no");
//! - THE SAFETY NET: "yes", then Sora cannot start it (a command cool-down for the whole latch) -> the latch runs out,
//!   the pad gets the late "no", the friend's own finisher plays.

use kh_core::host::{HitRecord, HitResult, HurtShape, TargetFlags, TargetId, TargetInfo};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Sora, StateId, Status, Vec3};
use kh_host::party::PartyChara;
use kh_party::NpcStateId;

use crate::*;

const DT: f32 = 1.0 / 60.0;

/// One dummy enemy (a target for Goofy's Jump Strike: without one his chain jumps at a point and never rolls).
struct Dummy(Vec3);

impl Dummy {
    fn info_of(&self) -> TargetInfo {
        TargetInfo { id: TargetId(1), position: self.0, center: self.0 + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
    }
}

impl kh_core::Targets for Dummy {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.push(self.info_of());
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        (id == TargetId(1)).then(|| self.info_of())
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if id == TargetId(1) {
            out.push(HurtShape { a: self.0 + Vec3::Z * 30.0, b: self.0 + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
}

impl PartyTargets for Dummy {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

/// Sora at level 99 (sheet sora-loadout-99) with the five team abilities equipped (as the test level's party-test
/// fixture equips them).
fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim_start_matches('\u{feff}')).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
            _ => continue,
        };
        let v = match (row[0].as_str().unwrap(), v) {
            ("abilities.equipped", LoadoutValue::List(mut l)) => {
                l.extend(["UNISON_FIRE", "UNISON_BLIZZARD", "UNISON_THUNDER", "FUSION_SPIN", "FUSION_ROCKET"].map(String::from));
                LoadoutValue::List(l)
            }
            (_, v) => v,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

/// What the run saw.
#[derive(Default, Debug)]
struct Seen {
    /// The answers the host gave (right after the route that rolled).
    answers: Vec<bool>,
    /// Sora's state at each answer.
    sora_at_answer: Vec<StateId>,
    /// The leader - Sora distance at each answer (cm).
    distance_at_answer: Vec<f32>,
    /// The leader's own finisher started (Donald's UnisonFinisher kit state / Goofy's FINISH or bash).
    own_finisher: bool,
    /// Sora's unison count went up (the team move started).
    team_move: bool,
    /// A late "no" reached the pad.
    late_no: bool,
}

struct Rig {
    rt: PartyRuntime,
    sora: Sora,
    world: TestWorld,
    dummy: Dummy,
    camera: kh_camera::CameraView,
    held: PadButtons,
    leader: kh_party::MemberId,
}

impl Rig {
    fn new(lead: PartyChara) -> Rig {
        let world = TestWorld::floor();
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&kh_core::SORA_PARAMS));
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: origin, yaw: 0.0 }).with_status(level99());
        let rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        let camera = kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let mut rig = Rig { rt, sora, world, dummy: Dummy(Vec3::new(5000.0, 0.0, 0.0)), camera, held: PadButtons::NONE, leader: kh_party::MemberId(0) };
        rig.step(false, None, false);
        // the playtest's kits (the test level's host equips them: kh-partyhost lead.rs / custom.rs)
        for c in [PartyChara::Donald, PartyChara::Goofy] {
            let id = rig.rt.id(c).unwrap();
            let s = rig.rt.party.status_mut(id).unwrap();
            s.kit = kh_party::pad::donald_kit::KitAbilities::test_all();
            s.goofy_kit = kh_party::pad::goofy_kit::GoofyKit::test_all();
            // (GOOFY-GK) Goofy's base string (2 steps + the finisher): the six taps reach his finisher press
            for k in [kh_party::pad::goofy_kit::GoofyAbility::ComboPlus, kh_party::pad::goofy_kit::GoofyAbility::ComboPlus2] {
                s.goofy_kit.set_equipped(k, false);
            }
        }
        rig.rt.lead_word(&["leader", if lead == PartyChara::Donald { "donald" } else { "goofy" }]);
        rig.step(false, None, false);
        rig.leader = rig.rt.leader.expect("a friend leads");
        // the leader 150 cm beside Sora (the roll's best band), the dummy 220 cm ahead of the leader (+x, his facing)
        let at = Vec3::new(0.0, 150.0, rig.rt.party.member(rig.leader).unwrap().position().z);
        rig.rt.party.member_mut(rig.leader).unwrap().cmc.location = at;
        rig.dummy = Dummy(Vec3::new(220.0, 150.0, 0.0));
        for _ in 0..30 {
            rig.step(false, None, false);
        }
        assert_eq!(rig.sora.output().state, StateId::Idle, "AI Sora settled");
        rig
    }

    /// One step of the test level's order. `a`: the pad's A this step (the leader's). `busy`: AI Sora's input is
    /// replaced by Blocks of his own (a guard every 1/3 s: he is mid-action most of the time). `block`: while a "yes" waits, Sora's core has a
    /// command cool-down (his `TryAttack` refuses: the team move cannot start within the latch).
    fn step(&mut self, a: bool, busy: Option<usize>, block: bool) -> Option<bool> {
        let held = if a { PadButtons::NONE.with(PadButton::FaceBottom) } else { PadButtons::NONE };
        let input = Input::default().with_pad(PadState::from_held(held, self.held));
        self.held = held;
        let pending = self.rt.leader.and_then(|l| self.rt.party.pad_state(l)).is_some_and(|p| p.finisher_pending);
        let mut sora_in = self.rt.route(RouteIn { sora: &mut self.sora, input: &input, dt: DT, targets: &self.dummy, camera: &self.camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: 0.0 });
        // the route answered the ask (Goofy's pad takes the answer inside the same route, Donald's in the party step):
        // a "yes" leaves the host's wait armed until the next route
        let answer = pending.then(|| self.rt.lead.team_wait.is_some());
        if let Some(i) = busy {
            let b = if i % 20 < 2 { PadButtons::NONE.with(PadButton::GUARD) } else { PadButtons::NONE };
            sora_in = Input { pad: PadState::from_held(b, if i % 20 == 1 { b } else { PadButtons::NONE }), ..Input::default() };
        }
        if block && self.rt.lead.team_wait.is_some() {
            self.sora.combat_mut().command_cooldown = 2.0;
        }
        let out = self.sora.step(DT, &sora_in, &self.world);
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        self.rt.step(StepIn { dt: DT, sora: &mut self.sora, out: &out, lock_on: TargetId::NONE, camera: &self.camera, world: &self.world, targets: &mut self.dummy, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
        let _ = self.rt.after_step(&self.dummy, DT, 0.0);
        answer
    }

    fn own_finisher(&self) -> bool {
        let p = self.rt.party.member(self.leader).unwrap();
        match p.chara {
            // (DON-KIT) his string's own finisher: the slam (pad/donald_combo.rs; no Unison / Meteor Drop finisher, doc 13)
            kh_party::Chara::Donald => kh_party::pad::donald::kit_state(p) == Some(kh_party::pad::donald::STEP) && p.pad.as_ref().is_some_and(|d| d.donald.combo.step == kh_party::pad::donald_combo::Step::Slam),
            // (GOOFY-GK) + his own string's finishers (pad/goofy_combo.rs: fin_big_bash / fin_shield_quake / fin_shield_upper)
            _ => matches!(p.state, NpcStateId::Pad(kh_party::pad::goofy::FINISH)) || p.pad.as_ref().is_some_and(|d| d.goofy.log.iter().any(|w| *w == "fusion_spin" || *w == "fusion_rocket" || *w == "late_no_finisher" || w.starts_with("fin_"))),
        }
    }

    /// Six A taps 0.3 s apart (the tour's combo), then `wait` s (see `step` for `busy` / `block`).
    fn combo(&mut self, busy: bool, block: bool, wait: f32) -> Seen {
        let mut seen = Seen::default();
        let unisons = self.sora.combat().friend.unisons;
        let steps = 6 * 18 + (wait / DT) as usize;
        for i in 0..steps {
            let a = i < 6 * 18 && i % 18 == 0;
            let (state_before, sora_at, waiting) = (self.sora.output().state, self.sora.output().position, self.rt.lead.team_wait.is_some());
            if let Some(yes) = self.step(a, busy.then_some(i), block) {
                seen.answers.push(yes);
                seen.sora_at_answer.push(state_before);
                let leader_at = self.rt.party.member(self.leader).unwrap().position();
                seen.distance_at_answer.push((leader_at - sora_at).length());
            }
            seen.own_finisher |= self.own_finisher();
            seen.team_move |= self.sora.combat().friend.unisons > unisons;
            // the wait ended without a team move: the late "no" (the pad may have taken it in the same step)
            seen.late_no |= waiting && self.rt.lead.team_wait.is_none() && self.sora.combat().friend.unisons == unisons;
        }
        seen
    }
}

fn gate(lead: PartyChara) {
    let mut rig = Rig::new(lead);
    let seen = rig.combo(true, false, 2.5);
    assert!(!seen.answers.is_empty(), "{lead:?}: the combo asked for its finisher: {seen:?}");
    assert!(seen.sora_at_answer.iter().all(|s| *s != StateId::Idle && *s != StateId::Run), "{lead:?}: AI Sora was busy at every ask: {seen:?}");
    assert!(seen.answers.iter().all(|y| !*y), "{lead:?}: busy Sora -> no roll, \"no\" at once: {seen:?}");
    assert!(seen.distance_at_answer.iter().all(|d| *d < kh_core::combat::friend::COMBO_FINISH_RANGE[0]), "{lead:?}: in the roll's range (the no is the gate's): {seen:?}");
    assert_eq!(rig.sora.combat().friend.leader_unison, 0.0, "{lead:?}: no latch armed");
    assert!(!seen.team_move && !seen.late_no, "{lead:?}: {seen:?}");
    assert!(seen.own_finisher, "{lead:?}: the friend played his own finisher: {seen:?}");
}

fn happy(lead: PartyChara) {
    let mut rig = Rig::new(lead);
    let seen = rig.combo(false, false, 2.5);
    assert_eq!(seen.answers.first(), Some(&true), "{lead:?}: idle Sora 150 cm away, five team abilities, chance 100 -> yes: {seen:?}");
    assert!(seen.team_move, "{lead:?}: AI Sora started the team move: {seen:?}");
    assert!(!seen.late_no, "{lead:?}: no late no when the move started: {seen:?}");
}

fn net(lead: PartyChara) {
    let mut rig = Rig::new(lead);
    let seen = rig.combo(false, true, 2.5);
    assert_eq!(seen.answers.first(), Some(&true), "{lead:?}: {seen:?}");
    assert!(!seen.team_move, "{lead:?}: the cool-down kept the move from starting: {seen:?}");
    assert!(seen.late_no, "{lead:?}: the latch ran out -> late no: {seen:?}");
    assert!(seen.own_finisher, "{lead:?}: the late no played his own finisher: {seen:?}");
    let f = rig.sora.combat().friend;
    assert_eq!((f.unison, f.leader_unison), (0, 0.0), "{lead:?}: the request was cleared");
}

/// (DON-KIT) Donald leading: his missile string never rolls a team unison (documentation/DONALD_OVERHAUL.md 13: Unison /
/// Meteor Drop / Flare Force are no combo finishers; the team attacks stay on Y): busy or idle Sora, the string ends in his
/// own slam and no team move starts.
fn donald_no_roll(busy: bool) {
    let mut rig = Rig::new(PartyChara::Donald);
    let at = rig.rt.party.member(rig.leader).unwrap().position();
    rig.dummy = Dummy(Vec3::new(at.x + 900.0, at.y, 0.0));
    let seen = rig.combo(busy, false, 2.5);
    assert!(seen.answers.is_empty(), "no roll: {seen:?}");
    assert!(!seen.team_move && !seen.late_no, "{seen:?}");
    assert!(seen.own_finisher, "his slam: {seen:?}");
}

#[test]
fn fx1_donald_leads_busy_sora_means_no_and_his_own_finisher() {
    donald_no_roll(true); // (DON-KIT)
}

#[test]
fn fx1_goofy_leads_busy_sora_means_no_and_his_own_finisher() {
    gate(PartyChara::Goofy);
}

#[test]
fn fx1_donald_leads_idle_sora_starts_the_unison() {
    donald_no_roll(false); // (DON-KIT) no unison finisher any more: the team attacks stay on Y
}

#[test]
fn fx1_goofy_leads_idle_sora_starts_the_fusion() {
    happy(PartyChara::Goofy);
}

#[test]
fn fx1_donald_leads_a_lapsed_latch_gives_a_late_no_and_his_own_finisher() {
    donald_no_roll(false); // (DON-KIT) no roll, so no latch to lapse
}

#[test]
fn fx1_goofy_leads_a_lapsed_latch_gives_a_late_no_and_his_own_finisher() {
    net(PartyChara::Goofy);
}

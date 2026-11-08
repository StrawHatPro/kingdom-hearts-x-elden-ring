//! (HUD-REVIVE-COMBAT, THE USER 2026-11-07: "if an enemy is actively fighting the party leader or any party member, the whole
//! party should still be in combat. Combat only ends when no enemy is targetting the party.") ONE PARTY-WIDE COMBAT STATE,
//! proven on a real `ErTargets` (stub rows) and the party core driven in sora.rs's order (frame -> the party's marks ->
//! route -> Sora's step with `SoraTargets` -> step):
//! - the count (`ErTargets::engaged_count` = `engages_the_party`): an enemy in range of a far member only, or one whose own
//!   Elden Ring AI targets a member's body (`party_aims`), engages the whole party; a passive kind's aim does not (BM);
//! - every reader of it: the leader's flag (Sora's `actions().battle` leading, `Leadership::battle_step` for a friend), the
//!   members' battle state (`Party::set_in_battle` -> each view's `in_battle`), the HUD's slots (`hud_slots().in_battle`,
//!   kh-host hud.rs `battle_mode` reads Sora's flag or the leading friend's slot), `PartyRuntime::party_combat`;
//! - the end: only once no enemy targets anyone, after KH3's 1.0 s linger (`stance.battle_linger`, 0x140EA0B20).

use glam::Vec3;
use kh_core::test_world::TestWorld;
use kh_core::{Input, NullHost, Output, Pose, Sora, TargetId, Targets, SORA_PARAMS};

use crate::coords::Frame;
use crate::ermap::{Params, Weight};
use crate::party::aiworld::ErPartyTargets;
use crate::party::glue::{BodyObs, CoreStep, FrameIn, HostRules, PartyCore};
use crate::targets::{engages_party, Enemy, ErTargets};

const STEP: f32 = 1.0 / 60.0;
const FOE: u32 = 7;

fn project(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn rows(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default()
}

/// A fighter (threatLv 1) or a passive animal (threatLv 0) at `at`.
fn foe(at: Vec3, passive: bool) -> Enemy {
    let mut e = Enemy::stub(FOE, at, 40.0, 90.0, 4000, Weight::Small);
    e.threat_lv = Some(u32::from(!passive));
    e.passive = passive;
    e.npc_id = if passive { 6060 } else { 3000 };
    e
}

fn targets(list: Vec<Enemy>) -> ErTargets {
    let mut t = ErTargets::new(Frame::at(Vec3::ZERO), Params::default());
    t.list = list;
    t.step = 1000;
    t
}

#[test]
fn combat_the_rule_table() {
    // (within range of any party place, passive, fought, attacking, its AI targets the party)
    assert!(engages_party(false, false, false, false, true), "a fighter whose AI targets a member engages wherever it stands");
    assert!(!engages_party(false, true, false, false, true), "a passive animal's aim (it watches what it flees from) does not");
    assert!(engages_party(false, true, true, false, true), "a passive animal that fought the party does (BM)");
    assert!(!engages_party(false, false, false, false, false), "no sign: no battle");
    assert!(engages_party(true, false, false, false, false), "a threat in range of a party place engages");
}

#[test]
fn combat_a_far_members_enemy_engages_the_whole_party() {
    // the leader at the origin, Donald 30 m off, a fighter 5 m from Donald (25 m from the leader: beyond his 8 m range)
    let donald = Vec3::new(3000.0, 0.0, 0.0);
    let mut t = targets(vec![foe(Vec3::new(3500.0, 0.0, 0.0), false)]);
    assert_eq!(t.engaged_count(), 0, "measured round the leader alone it is nobody's");
    t.set_party_marks(vec![donald], Vec::new());
    assert_eq!(t.engaged_count(), 1, "it stands in range of a member: the party's count");
    // a friend leading: the leader's count (ErPartyTargets, round the leader) is the same number
    let mut latch = 0;
    let p = ErPartyTargets::new(&mut t, Vec3::new(0.0, -100.0, 0.0), &mut latch);
    assert_eq!(Targets::engaged_count(&p), 1);
    // an enemy 25 m from everyone whose own AI targets a member's body (the game's AI target, aims_now)
    let mut t = targets(vec![foe(Vec3::new(0.0, 2500.0, 0.0), false)]);
    t.set_party_marks(vec![Vec3::new(300.0, 0.0, 0.0)], vec![TargetId(FOE)]);
    assert_eq!(t.engaged_count(), 1, "its AI targets a member: engaged");
    assert!(t.engaged_report().contains("its AI targets the party"), "{}", t.engaged_report());
    t.aims_rule = false;
    assert_eq!(t.engaged_count(), 0, "combat.aims = 0: the aim sign is not used");
    // a passive animal aiming at a member: no battle (BM's rule stands)
    let mut t = targets(vec![foe(Vec3::new(0.0, 2500.0, 0.0), true)]);
    t.set_party_marks(Vec::new(), vec![TargetId(FOE)]);
    assert_eq!(t.engaged_count(), 0);
}

/// The party core in sora.rs's order over a real `ErTargets`.
struct Bench {
    core: PartyCore,
    sora: Sora,
    out: Output,
    world: TestWorld,
    er: ErTargets,
    null: NullHost,
    latch: u32,
    t: f64,
    /// The foe's AI targets a member (what aims_now would read).
    aimed: bool,
}

impl Bench {
    fn new() -> Option<Bench> {
        let pack = kh_ai::AiPack::load_dir(&project("build/sora_pack/party/ai")).ok()?;
        let sheets = project("sheets/port");
        let config = kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&rows(&sheets.join("party-test.json"))), weapons: kh_partyhost::PartyWeapon::from_rows(&rows(&sheets.join("party-weapons.json"))), ai_pack: Some(std::sync::Arc::new(pack)), ai_pack_source: None };
        let host_rules = std::fs::read_to_string(sheets.join("party-er-host.json")).ok().and_then(|t| HostRules::from_sheet(&t).ok()).map_or(HostRules::DEFAULT, |r| r.0);
        let core = PartyCore::new(config, host_rules);
        let sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
        let out = sora.output();
        // the foe 30 m off: beyond everyone's engage range (8 m) and the friends' threat range (12 m: they never go for it)
        let er = targets(vec![foe(Vec3::new(0.0, 3000.0, 0.0), false)]);
        Some(Bench { core, sora, out, world: TestWorld::floor(), er, null: NullHost::default(), latch: 0, t: 0.0, aimed: false })
    }

    fn step(&mut self) {
        let fin = FrameIn { on: true, fresh: false, relocated: false, hidden: None, body_action: false, trace: false, body: Some(BodyObs { feet: self.out.position, yaw: self.out.yaw }), pose: false, dt: STEP, now: self.t };
        self.core.frame(fin);
        let leader = self.core.rt.leader_view_of(&self.out, self.sora.status()).position;
        // sora.rs: targets.sora = the leader; `party_marks`: the other party places and the foes aiming at the party
        self.er.step += 1;
        self.er.sora = leader;
        let rt = &self.core.rt;
        let mut at: Vec<Vec3> = rt.current.iter().filter(|v| !v.ko).map(|v| v.position).collect();
        if rt.leader.is_some() && rt.lead.sora_active {
            at.push(self.out.position);
        }
        self.er.set_party_marks(at, if self.aimed { vec![TargetId(FOE)] } else { Vec::new() });
        let mut latch = self.latch;
        let input = {
            let t = ErPartyTargets::new(&mut self.er, leader, &mut latch);
            self.core.route(&mut self.sora, &Input::default(), STEP, &self.world, &t, self.t)
        };
        if !self.core.sora_skipped() {
            let mut st = kh_partyhost::SoraTargets { targets: &mut self.er, runtime: &mut self.core.rt };
            let mut host = kh_core::Host { world: &self.world, targets: &mut st, audio: &mut self.null.audio, effects: &mut self.null.effects, ui: &mut self.null.ui };
            self.out = self.sora.step_hosted(STEP, &input, &mut host);
        }
        let mut t = ErPartyTargets::new(&mut self.er, leader, &mut latch);
        self.core.step(&mut self.sora, CoreStep { out: &self.out, dt: STEP, lock_on: TargetId::NONE, world: &self.world, ik: &kh_anim::NoIkWorld, targets: &mut t, audio: &mut self.null.audio, effects: &mut self.null.effects, sora_look: None, now: self.t });
        self.latch = latch;
        self.t += STEP as f64;
    }

    fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }

    /// Every reader of the party's combat state: (party_combat, the leader's flag, every member's state, the HUD's slots,
    /// Sora's own flag (leading or AI Sora)).
    fn readers(&self) -> (bool, bool, bool, bool, bool) {
        let rt = &self.core.rt;
        let leader_flag = match rt.leader {
            None => self.sora.actions().battle,
            Some(id) => rt.current.iter().find(|v| v.id == id).is_some_and(|v| v.in_battle),
        };
        let members = !rt.current.is_empty() && rt.current.iter().all(|v| v.in_battle);
        let hud = {
            let slots = rt.hud_slots();
            !slots.is_empty() && slots.iter().all(|s| s.in_battle)
        };
        (rt.party_combat(), leader_flag, members, hud, self.sora.actions().battle)
    }

    fn all(&self, on: bool) -> bool {
        let (a, b, c, d, e) = self.readers();
        [a, b, c, d, e].iter().all(|x| *x == on)
    }
}

/// One leader's run: out of combat; the far foe's AI targets a member -> the whole party in combat while it does; it stops ->
/// still in combat through KH3's 1.0 s linger, then out everywhere.
fn run(leader: Option<&str>) -> Option<()> {
    let mut b = Bench::new()?;
    b.steps(30);
    if let Some(name) = leader {
        b.core.rt.lead_word(&["leader", name]);
    }
    b.steps(60);
    assert_eq!(b.core.rt.leader.is_some(), leader.is_some(), "{leader:?}: the leader is set");
    assert!(b.all(false), "{leader:?}: nobody targets the party: out of combat {:?}", b.readers());
    let foe_at = b.er.list[0].pos;
    let party_near = b.er.party_places().map(|p| (foe_at - p).length()).fold(f32::INFINITY, f32::min);
    assert!(party_near > 1500.0, "{leader:?}: the foe stands {party_near} cm from the nearest party place: beyond every range");
    b.aimed = true;
    b.steps(3);
    assert!(b.all(true), "{leader:?}: its AI targets a member: the whole party in combat {:?}", b.readers());
    b.steps(240);
    assert!(b.all(true), "{leader:?}: 4 s on, still targeting: still in combat {:?}", b.readers());
    b.aimed = false;
    b.steps(45);
    assert!(b.all(true), "{leader:?}: 0.75 s after the last enemy stopped: KH3's 1.0 s linger holds {:?}", b.readers());
    b.steps(30);
    assert!(b.all(false), "{leader:?}: 1.25 s after: out of combat everywhere {:?}", b.readers());
    Some(())
}

#[test]
fn combat_one_party_state_with_sora_leading() {
    if run(None).is_none() {
        eprintln!("(skipped: no AI pack under build/sora_pack/party/ai)");
    }
}

#[test]
fn combat_one_party_state_with_a_friend_leading() {
    for name in ["Donald", "Goofy"] {
        if run(Some(name)).is_none() {
            eprintln!("(skipped: no AI pack under build/sora_pack/party/ai)");
            return;
        }
    }
}

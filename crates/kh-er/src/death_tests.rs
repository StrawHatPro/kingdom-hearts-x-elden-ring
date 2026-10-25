//! (DTH) THE DEATH FLOW FOR EVERY LEADER, THROUGH ONE HARNESS. Each leader is a real one where the crate has it (kh-core's
//! Sora; kh-party's Donald and Goofy through S2's leader body and kh-partyhost's runtime, the path sora.rs takes), or a
//! mock of a character the game does not have yet (it gets the same flow without a line of code for him). The game's side
//! is a model of what a death meets in Elden Ring: the hidden body's HP, the no-death switch, a real death (its sound, its
//! voice, YOU DIED), and the tutorial's event as the game's data has it (m10_01 EVENT 10010030: the player at HP == 1 with
//! the boss alive -> 1 s -> cutscene 10010000 -> the warp to m18; a real death ends it).

use super::*;
use glam::Vec3;
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::{EventKind, Input, Pose, Sora, StateId, TargetId, SORA_PARAMS};
use kh_host::party::PartyChara;

const DT: f32 = 1.0 / 60.0;

/// A leader the harness can knock out and step (the host steps him every frame, live path and dead path alike).
trait Lead {
    type W: Copy + PartialEq + Debug;
    fn who(&self) -> Self::W;
    /// `death::leader_alive` with this frame's facts, as sora.rs asks it.
    fn alive(&self) -> bool;
    fn knock_out(&mut self);
    fn step(&mut self, dt: f32);
    /// In his KO state, and what of it shows (clip / state and its time): it must move on, never freeze.
    fn ko(&self) -> (bool, String, f32);
}

// ------------------------------------------------------------------------------------------ Sora (kh-core)

struct SoraLead {
    sora: Sora,
    world: kh_core::test_world::TestWorld,
    fell: bool,
    t_die: f32,
}

/// Sora with a real status (the level-99 loadout sheet, no ability that leaves him 1 HP): Sora::new's is a placeholder that
/// takes no damage.
fn real_sora(at: Vec3) -> Sora {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = kh_core::Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        use kh_core::status::LoadoutValue as V;
        let value = if key == "abilities.equipped" {
            V::List(Vec::new())
        } else {
            match &row[1] {
                serde_json::Value::Number(n) => V::Number(n.as_f64().unwrap()),
                serde_json::Value::String(t) => V::Text(t.clone()),
                serde_json::Value::Array(items) => V::List(items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect()),
                serde_json::Value::Object(map) => V::Map(map.iter().filter_map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect()),
                _ => continue,
            }
        };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: at, yaw: 0.0 });
    *sora.status_mut() = kh_core::Status::from_loadout(&loadout);
    sora
}

impl SoraLead {
    fn new() -> SoraLead {
        SoraLead { sora: real_sora(Vec3::new(0.0, 0.0, 0.15)), world: kh_core::test_world::TestWorld::floor(), fell: false, t_die: 0.0 }
    }
}

impl Lead for SoraLead {
    type W = PartyChara;
    fn who(&self) -> PartyChara {
        PartyChara::Sora
    }
    fn alive(&self) -> bool {
        leader_alive(None, self.sora.status().hp, self.fell)
    }
    fn knock_out(&mut self) {
        self.sora.status_mut().hp = 1;
        let at = self.sora.output().position;
        self.sora.take_hit(HitOnPlayer::new(ETresDamageKind::Small, 60.0, TargetId::NONE, at - Vec3::X * 100.0));
    }
    fn step(&mut self, dt: f32) {
        let out = self.sora.step(dt, &Input::default(), &self.world);
        self.fell = out.events.iter().any(|e| matches!(e.kind, EventKind::PlayerDied { .. }));
        if out.state == StateId::Die {
            self.t_die += dt;
        }
    }
    fn ko(&self) -> (bool, String, f32) {
        let out = self.sora.output();
        (out.state == StateId::Die, format!("{:?}", out.state), self.t_die)
    }
}

// ------------------------------------------------------------------------------------------ Donald / Goofy (kh-party)

#[cfg(feature = "party")]
struct FriendLead {
    rt: kh_partyhost::PartyRuntime,
    body: crate::party::leader::LeaderBody,
    sora: Sora,
    chara: PartyChara,
    world: kh_core::test_world::TestWorld,
}

#[cfg(feature = "party")]
impl FriendLead {
    fn new(chara: PartyChara) -> FriendLead {
        let mut rt = kh_partyhost::PartyRuntime::new(kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        rt.spawn_one(PartyChara::Donald, Vec3::new(100.0, 0.0, 0.0), 0.0);
        rt.spawn_one(PartyChara::Goofy, Vec3::new(0.0, 100.0, 0.0), 0.0);
        rt.leader = rt.id(chara);
        let mut f = FriendLead { rt, body: Default::default(), sora: real_sora(Vec3::new(0.0, -100.0, 0.15)), chara, world: kh_core::test_world::TestWorld::floor() };
        f.views();
        f
    }

    fn views(&mut self) {
        self.rt.current = self.rt.members.iter().filter_map(|(_, id)| self.rt.party.view(*id)).collect();
        let out = self.sora.output();
        self.body.update(&self.rt, &out, self.sora.status(), false);
    }

    fn view(&self) -> kh_party::view::MemberView {
        self.rt.view(self.chara).expect("in the party")
    }
}

#[cfg(feature = "party")]
impl Lead for FriendLead {
    type W = PartyChara;
    fn who(&self) -> PartyChara {
        // sora.rs `lead_chara`: the leader body's chara while a friend leads
        assert!(self.body.friend_lead().is_some(), "a friend leads");
        self.body.chara()
    }
    fn alive(&self) -> bool {
        leader_alive(self.body.friend_lead().map(|f| f.alive), self.sora.status().hp, false)
    }
    fn knock_out(&mut self) {
        // the body's blows on him, as sora.rs `damage_in` hands them (S2's `hit_now`), until he is down
        let id = self.rt.id(self.chara).unwrap();
        self.rt.party.status_mut(id).unwrap().hp = 3;
        for _ in 0..4 {
            let Some(f) = self.body.friend_lead() else { break };
            let record = crate::guards::friend_hit(&f, Some(ETresDamageKind::Small), 10.0, TargetId::NONE, f.position - Vec3::X * 100.0, 0.0, 40.0);
            crate::party::leader::LeaderBody::hit_now(&mut self.rt, record);
            self.views();
        }
    }
    fn step(&mut self, dt: f32) {
        let mut null = kh_core::NullHost::default();
        let mut host = kh_party::PartyHost { world: &self.world, targets: &mut null.targets, audio: &mut null.audio, effects: &mut null.effects };
        self.rt.party.step(dt, &kh_party::SoraView::default(), &mut host);
        self.views();
    }
    fn ko(&self) -> (bool, String, f32) {
        let v = self.view();
        (v.ko, v.anim.clip.to_string(), v.anim.time)
    }
}

// ------------------------------------------------------------------------------------------ a character to come (mock)

/// A playable character the game does not have yet: the flow knows nothing of him.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hero {
    Riku,
}

struct HeroLead {
    hp: i32,
    ko_time: f32,
}

impl Lead for HeroLead {
    type W = Hero;
    fn who(&self) -> Hero {
        Hero::Riku
    }
    fn alive(&self) -> bool {
        leader_alive(Some(self.hp > 0), 120, false)
    }
    fn knock_out(&mut self) {
        self.hp = 0;
    }
    fn step(&mut self, dt: f32) {
        if self.hp <= 0 {
            self.ko_time += dt;
        }
    }
    fn ko(&self) -> (bool, String, f32) {
        (self.hp <= 0, "Riku KO".to_string(), self.ko_time)
    }
}

// ------------------------------------------------------------------------------------------ the game (model)

#[derive(Default)]
struct Game {
    hp: i32,
    max: i32,
    no_dead: bool,
    /// The tutorial boss's arena (the event exists) and the boss alive.
    chapel: bool,
    boss_alive: bool,
    since_hp1: f32,
    event_over: bool,
    scene: Option<f32>,
    warped: bool,
    you_died: bool,
    death_sound: bool,
}

impl Game {
    fn frame(&mut self, dt: f32) {
        if self.hp <= 0 && !self.you_died {
            // a REAL death: its sound, its voice, its banner; the tutorial's event ends on it
            self.you_died = true;
            self.death_sound = true;
            self.event_over = true;
        }
        if self.chapel && !self.event_over && self.scene.is_none() && self.boss_alive && self.hp == 1 {
            self.since_hp1 += dt;
            if self.since_hp1 >= 1.0 {
                self.scene = Some(0.0); // SetSpEffect 4790, the boss music off, PlayCutsceneToPlayerAndWarp(10010000, m18)
            }
        }
        if let Some(s) = self.scene.as_mut() {
            *s += dt;
            if *s > 3.0 {
                self.warped = true;
            }
        }
    }
}

/// What one run shows.
#[derive(Debug)]
struct Trace<W> {
    down_at: Option<f32>,
    kind: Option<Kind>,
    gameover: Option<W>,
    killed: bool,
    spare_at: Option<f32>,
    first_fade_at: Option<f32>,
    fade_at_spare: f32,
    scene_at: Option<f32>,
    fade_after_scene: f32,
    no_dead_dropped_before_scene: bool,
    /// The KO's progress: (clip / state, time) at the KO, at the hand-over, at the end.
    ko_first: Option<(String, f32)>,
    ko_at_spare: Option<(String, f32)>,
    ko_last: Option<(String, f32)>,
    left_ko: bool,
    you_died: bool,
    death_sound: bool,
    warped: bool,
    hp_min: i32,
}

/// THE HOST, FRAME BY FRAME, in sora.rs' order: the steps (the leader stepped in the live and the dead path alike), the
/// death flow's observation, the HUD's game over (`gameover_key`: `gameover.kh`; the menu answered at `answer_at`), the
/// body's order (`after_steps`) or, once handed over, the dead path's (`death_dead_frame`), then the game's own frame.
fn run<L: Lead>(lead: &mut L, chapel: bool, scripted_place: bool, gameover_key: bool, answer_at: Option<f32>, secs: f32) -> Trace<L::W> {
    let mut flow: Flow<L::W> = Flow::default();
    let mut game = Game { hp: 5220, max: 5220, chapel, boss_alive: true, ..Game::default() };
    let mut tr = Trace { down_at: None, kind: None, gameover: None, killed: false, spare_at: None, first_fade_at: None, fade_at_spare: 0.0, scene_at: None, fade_after_scene: 0.0, no_dead_dropped_before_scene: false, ko_first: None, ko_at_spare: None, ko_last: None, left_ko: false, you_died: false, death_sound: false, warped: false, hp_min: i32::MAX };
    let mut holds = false;
    let mut hud_died = false;
    let mut sent = false;
    let mut t = 0.0f32;
    while t < secs {
        if (t - 1.0).abs() < DT / 2.0 {
            lead.knock_out();
        }
        lead.step(DT);
        let (in_ko, clip, time) = lead.ko();
        if tr.down_at.is_some() {
            if !in_ko {
                tr.left_ko = true;
            }
            tr.ko_first.get_or_insert((clip.clone(), time));
            tr.ko_last = Some((clip.clone(), time));
        }
        // the flow's observation (before the HUD)
        if let Some((_, kind)) = flow.observe(lead.who(), lead.alive(), scripted_place) {
            tr.down_at = Some(t);
            tr.kind = Some(kind);
            tr.ko_first = Some((clip.clone(), time));
        }
        // the HUD: KH3's game over for the leader the flow names
        if gameover_key && !hud_died {
            if let Some(w) = flow.gameover() {
                hud_died = true;
                holds = true;
                tr.gameover = Some(w);
            }
        }
        if answer_at.is_some_and(|a| t >= a) {
            holds = false; // the menu answered (Retry: the game's own death follows)
        }
        let order = if !sent {
            let o = flow.live_step(DT, holds);
            match o.body {
                Body::Kill => {
                    game.hp = 0;
                    sent = true;
                    tr.killed = true;
                }
                Body::Spare(hp) => {
                    game.hp = hp;
                    sent = true;
                    tr.spare_at = Some(t);
                    tr.fade_at_spare = o.fade;
                    tr.ko_at_spare = Some((clip.clone(), time));
                }
                Body::Hold | Body::Keep => game.hp = game.max, // topped up while the body is ours
            }
            // the fall guard's switch while he lives, the flow's while a scripted death holds it
            game.no_dead = (flow.down().is_none() && o.body != Body::Kill) || o.hold_no_dead;
            o
        } else {
            let o = flow.dead_step(DT, game.scene.is_some());
            if o.body == Body::Kill {
                game.hp = 0;
                tr.killed = true;
            }
            game.no_dead = o.hold_no_dead;
            o
        };
        if order.fade > 0.0 && tr.first_fade_at.is_none() {
            tr.first_fade_at = Some(t);
        }
        if tr.down_at.is_some() && tr.kind == Some(Kind::Scripted) && game.scene.is_none() && !game.no_dead && !tr.killed {
            tr.no_dead_dropped_before_scene = true;
        }
        if game.scene.is_some() {
            tr.scene_at.get_or_insert(t);
            tr.fade_after_scene = tr.fade_after_scene.max(order.fade);
        }
        game.frame(DT);
        tr.hp_min = tr.hp_min.min(game.hp);
        t += DT;
    }
    tr.you_died = game.you_died;
    tr.death_sound = game.death_sound;
    tr.warped = game.warped;
    tr
}

/// THE SCRIPTED DEATH, the same assertions for every leader.
fn check_scripted<W: Copy + PartialEq + Debug>(tr: &Trace<W>, name: &str) {
    let r = Rules::DEFAULT;
    let down = tr.down_at.unwrap_or_else(|| panic!("{name}: never down"));
    assert_eq!(tr.kind, Some(Kind::Scripted), "{name}");
    assert_eq!(tr.gameover, None, "{name}: no game-over menu at a scripted death");
    assert!(!tr.killed && tr.hp_min > 0, "{name}: the body's HP never 0 (min {})", tr.hp_min);
    assert!(!tr.you_died && !tr.death_sound, "{name}: no death of the game's: no death sound, no voice, no YOU DIED");
    let fade_from = tr.first_fade_at.expect("a fade");
    assert!(fade_from - down >= r.ko_show - 2.0 * DT, "{name}: his own KO shows {:.2} s before the fade", fade_from - down);
    let spare = tr.spare_at.expect("handed over");
    assert!((spare - down - (r.ko_show + r.fade)).abs() < 3.0 * DT, "{name}: handed over at +{:.2} s", spare - down);
    assert!(tr.fade_at_spare >= 0.999, "{name}: black before the hand-over ({})", tr.fade_at_spare);
    let scene = tr.scene_at.unwrap_or_else(|| panic!("{name}: the game's scene never came"));
    assert!((scene - spare - 1.0).abs() < 3.0 * DT, "{name}: the event's own second, then its scene (+{:.2} s)", scene - spare);
    assert_eq!(tr.fade_after_scene, 0.0, "{name}: the black leaves when the scene plays");
    assert!(!tr.no_dead_dropped_before_scene, "{name}: the no-death switch held from the KO to the scene");
    assert!(tr.warped, "{name}: the game took its player on (m18)");
    // his own KO, not a freeze: in the KO all along, and it moves on between the KO and the hand-over
    assert!(!tr.left_ko, "{name}: he stays down");
    let (a, b) = (tr.ko_first.clone().unwrap(), tr.ko_at_spare.clone().unwrap());
    assert!(a != b, "{name}: the KO moved on ({a:?} -> {b:?})");
    let last = tr.ko_last.clone().unwrap();
    assert!(last.1 > 0.0 || last.0 != a.0, "{name}: still animated after the hand-over ({last:?})");
}

/// A NORMAL DEATH with KH3's game over: the menu for HIM, the body held (never HP 0) while it is up; with the menu off the
/// game's own death at once (as before).
fn check_normal<W: Copy + PartialEq + Debug>(tr_menu: &Trace<W>, tr_plain: &Trace<W>, who: W, name: &str) {
    assert_eq!(tr_menu.kind, Some(Kind::Normal), "{name}");
    assert_eq!(tr_menu.gameover, Some(who), "{name}: KH3's game over names him");
    assert!(!tr_menu.killed && !tr_menu.you_died, "{name}: nothing of the game's death while the menu is up");
    assert_eq!(tr_menu.first_fade_at, None, "{name}: no scripted fade");
    assert!(!tr_menu.left_ko, "{name}: he stays down");
    assert_eq!(tr_plain.gameover, None, "{name}: no menu without gameover.kh");
    assert!(tr_plain.killed && tr_plain.you_died, "{name}: without the menu the game's own death (as before)");
    assert!((tr_plain.down_at.unwrap() - 1.0).abs() < 0.2, "{name}: down at the blow");
}

fn every_case<L: Lead>(make: impl Fn() -> L, name: &str) {
    let tr = run(&mut make(), true, true, true, None, 10.0);
    check_scripted(&tr, name);
    let who = make().who();
    let menu = run(&mut make(), false, false, true, None, 6.0);
    let plain = run(&mut make(), false, false, false, None, 6.0);
    check_normal(&menu, &plain, who, name);
    // the menu answered by Retry: the game's own death then
    let retry = run(&mut make(), false, false, true, Some(4.0), 6.0);
    assert!(retry.killed && retry.gameover == Some(who), "{name}: Retry hands the death to the game");
}

#[test]
fn sora_leading_dies_by_the_one_path() {
    every_case(SoraLead::new, "Sora");
}

#[cfg(feature = "party")]
#[test]
fn donald_leading_dies_by_the_one_path() {
    every_case(|| FriendLead::new(PartyChara::Donald), "Donald");
}

#[cfg(feature = "party")]
#[test]
fn goofy_leading_dies_by_the_one_path() {
    every_case(|| FriendLead::new(PartyChara::Goofy), "Goofy");
}

/// A CHARACTER TO COME: the same harness, the same assertions, no code of the flow for him.
#[test]
fn a_new_playable_character_dies_by_the_one_path() {
    every_case(|| HeroLead { hp: 100, ko_time: 0.0 }, "Riku (mock)");
}

/// AI SORA KNOCKED OUT WHILE A FRIEND LEADS is no death of the leader's: no game over, nothing handed to the game.
#[cfg(feature = "party")]
#[test]
fn ai_sora_down_while_a_friend_leads_is_no_game_over() {
    let mut f = FriendLead::new(PartyChara::Goofy);
    let mut flow: Flow<PartyChara> = Flow::default();
    f.sora.status_mut().hp = 1;
    let at = f.sora.output().position;
    f.sora.take_hit(HitOnPlayer::new(ETresDamageKind::Small, 60.0, TargetId::NONE, at - Vec3::X * 100.0));
    let mut sora_down = false;
    for _ in 0..300 {
        let out = f.sora.step(DT, &Input::default(), &f.world);
        sora_down |= out.state == StateId::Die;
        f.step(DT);
        assert_eq!(flow.observe(f.who(), f.alive(), false), None);
        assert_eq!(flow.live_step(DT, false).body, Body::Keep);
    }
    assert!(sora_down && f.sora.status().hp <= 0, "AI Sora is down");
    assert_eq!((flow.down(), flow.gameover()), (None, None));
}

/// THE PARTY AFTER A DEATH (a new Sora host): the knocked-out leader stands, full; the new flow sees him alive.
#[cfg(feature = "party")]
#[test]
fn the_party_stands_again_at_the_next_host() {
    for chara in [PartyChara::Donald, PartyChara::Goofy] {
        let mut f = FriendLead::new(chara);
        f.knock_out();
        for _ in 0..30 {
            f.step(DT);
        }
        assert!(f.view().ko && !f.alive(), "{chara:?} down");
        // the new host's first frame
        let stood = revive_all::<PartyChara, _>(&mut f.rt);
        assert_eq!(stood, vec![chara], "only the KO'd one was down");
        f.views();
        let v = f.view();
        assert!(!v.ko && v.hp == v.hp_max && v.mp == v.mp_max && v.state == kh_party::NpcStateId::Idle, "{chara:?} stands, full: {v:?}");
        assert!(f.alive());
        let mut flow: Flow<PartyChara> = Flow::default();
        assert_eq!(flow.observe(f.who(), f.alive(), false), None, "no game over at the next host");
        f.step(DT);
        assert!(!f.view().ko, "and stays up");
    }
}

/// The mock's party: a character to come stands again by the same rule.
#[test]
fn a_new_character_stands_again_too() {
    struct Mock(Vec<(Hero, i32, bool)>);
    impl Revive<Hero> for Mock {
        fn members(&self) -> Vec<Hero> {
            self.0.iter().map(|m| m.0).collect()
        }
        fn revive(&mut self, who: Hero) -> bool {
            let m = self.0.iter_mut().find(|m| m.0 == who).unwrap();
            let was = m.2;
            m.1 = 100;
            m.2 = false;
            was
        }
    }
    let mut party = Mock(vec![(Hero::Riku, 0, true)]);
    assert_eq!(revive_all(&mut party), vec![Hero::Riku]);
    assert_eq!(party.0[0], (Hero::Riku, 100, false));
}

/// A SCRIPTED DEATH THAT GIVES NOTHING (the event not there, the boss dead): after `wait` the game's own death, the black
/// lifted, the switch let go; nothing before that.
#[test]
fn a_scripted_death_without_its_scene_falls_back_to_the_games_death() {
    let r = Rules::DEFAULT;
    let mut lead = HeroLead { hp: 100, ko_time: 0.0 };
    let tr = run(&mut lead, false, true, true, None, 1.0 + r.ko_show + r.fade + r.wait + 2.0);
    assert_eq!(tr.kind, Some(Kind::Scripted));
    assert!(tr.spare_at.is_some() && tr.scene_at.is_none());
    assert!(tr.killed && tr.you_died, "the game's own death at last");
    let mut flow: Flow<Hero> = Flow::default();
    flow.observe(Hero::Riku, false, true);
    let mut t = 0.0;
    while !flow.sent() {
        flow.live_step(DT, false);
        t += DT;
    }
    let mut kill_at = None;
    let mut fades = Vec::new();
    for k in 0..((r.wait + GAVE_UP_BLACK + 1.0) / DT) as usize {
        let o = flow.dead_step(DT, false);
        if o.body == Body::Kill {
            kill_at = Some(k as f32 * DT);
            assert!(!o.hold_no_dead, "the switch is let go with the kill");
        }
        if kill_at.is_some() {
            fades.push(o.fade);
        }
    }
    assert!((kill_at.unwrap() - r.wait).abs() < 2.0 * DT, "after death.wait ({:?})", kill_at);
    let in_10_s = (10.0 / DT) as usize;
    assert!(fades[..in_10_s].iter().all(|f| *f >= 0.999), "the game's own death runs under the black (no YOU DIED seen)");
    assert_eq!(*fades.last().unwrap(), 0.0, "the black lifted at last");
    assert!(t > 0.0);
}

/// `death.scripted = 0`: every death is a normal one (the old rule at the tutorial boss).
#[test]
fn the_scripted_rule_can_be_switched_off() {
    let mut flow: Flow<Hero> = Flow::new(Rules { scripted: false, ..Rules::DEFAULT });
    assert_eq!(flow.observe(Hero::Riku, false, true), Some((Hero::Riku, Kind::Normal)));
    assert_eq!(flow.live_step(DT, false).body, Body::Kill);
}

/// The leader-alive rule: a friend's KO decides while he leads, Sora's HP or his death event while Sora leads.
#[test]
fn who_is_alive() {
    assert!(leader_alive(Some(true), 0, true), "a friend leads: AI Sora's KO / event count for nothing");
    assert!(!leader_alive(Some(false), 120, false));
    assert!(leader_alive(None, 1, false));
    assert!(!leader_alive(None, 0, false));
    assert!(!leader_alive(None, 5, true), "Sora's death event");
}

/// The flow latches: a leader revived by his own KO timer, a new leader, the place changing do not undo the death.
#[test]
fn the_death_is_latched_once_per_host() {
    let mut flow: Flow<PartyChara> = Flow::default();
    assert_eq!(flow.observe(PartyChara::Donald, false, false), Some((PartyChara::Donald, Kind::Normal)));
    assert_eq!(flow.observe(PartyChara::Donald, true, false), None);
    assert_eq!(flow.observe(PartyChara::Sora, false, true), None);
    assert_eq!(flow.down(), Some(PartyChara::Donald));
    assert_eq!(flow.gameover(), Some(PartyChara::Donald));
}


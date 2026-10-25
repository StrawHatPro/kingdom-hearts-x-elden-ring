//! (party X2) The friend leader's framework: Donald casts Sora's magic (tiers, costs, the actors, Cure, his own MP and
//! the friends' 30 s charge, grand magic on Y), Customize blocks drive his menus, Goofy's Abilities page, the sprint,
//! and decision 1 (no friend damage halving on the leader).

use glam::{Vec2, Vec3};
use kh_core::command::leader::{goofy, LeaderCustom, LeaderKind, MagicBook};
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::input::{PadButton, PadButtons, PadState as Buttons};
use kh_core::magic::spells::cmd;
use kh_core::status::{AbilitySet, GameLevel};
use kh_core::test_world::TestWorld;
use kh_core::Tuning;
use kh_party::leader_magic::LeaderFeed;
use kh_party::pad::PadFrame;
use kh_party::{Chara, MemberId, NpcStateId, Party, PartyConfig, PartyHost, SoraView};

const DT: f32 = 1.0 / 60.0;

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
}

/// Sora at level 99: every element at tier 3, his slot order of the level-99 loadout.
fn book(tiers: [u8; 6]) -> MagicBook {
    MagicBook { tiers, sora_slots: [cmd::FIRAGA, cmd::WATAGA, cmd::CURAGA, cmd::BLIZZAGA, cmd::THUNDAGA, cmd::AEROGA] }
}

fn feed(tiers: [u8; 6], team: bool) -> LeaderFeed {
    let b = book(tiers);
    let mut customs = [LeaderCustom::default(); 2];
    for k in LeaderKind::ALL {
        customs[k as usize].sync(k, &b);
    }
    LeaderFeed { book: b, sora_abilities: AbilitySet::EMPTY, game_level: GameLevel::Standard, battle_level: 1, mp_cost_scale: 1.0, customs, sora_team_offer: team }
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
    feed: Option<LeaderFeed>,
}

impl Rig {
    fn new(chara: Chara, dummy: Option<Vec3>, feed: Option<LeaderFeed>) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        let dummies = Dummies(dummy.map(|p| vec![(TargetId(1), p)]).unwrap_or_default(), Vec::new());
        let mut rig = Rig { world: TestWorld::floor(), dummies, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), id, held: PadButtons::NONE, feed };
        rig.sora.position = Vec3::new(-2000.0, 0.0, 0.0);
        rig.idle(10);
        rig
    }

    fn m(&self) -> &kh_party::pawn::NpcPawn {
        self.party.member(self.id).unwrap()
    }

    fn step(&mut self, stick: Vec2, held: &[PadButton]) {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = Buttons { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        if let Some(pad) = self.party.member_mut(self.id).and_then(|m| m.pad.as_mut()).filter(|p| p.finisher_pending) {
            pad.finisher_pending = false;
            pad.finisher_answer = Some(false);
        }
        // the host's feed comes before the pad's frame (kh-partyhost route)
        self.party.set_leader_magic(self.feed.clone());
        self.party.set_pad_frame(self.id, PadFrame { stick, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(Vec2::ZERO, &[]);
        }
    }

    /// LB held, then a face button of the shortcut panel.
    fn shortcut(&mut self, button: PadButton) {
        self.step(Vec2::ZERO, &[PadButton::LeftShoulder]);
        self.step(Vec2::ZERO, &[PadButton::LeftShoulder, button]);
        self.step(Vec2::ZERO, &[]);
    }
}

#[test]
#[ignore = "(DON-MAGIC) REMOVED by the Donald overhaul (DONALD_OVERHAUL.md 13, DON_PLAN R26): Donald no longer casts Sora's spells / Grand Magic (crate::donald::cast)"]
fn donald_leading_casts_soras_firaga_with_his_own_clip_and_mp() {
    let mut rig = Rig::new(Chara::Donald, Some(Vec3::new(700.0, 0.0, 0.0)), Some(feed([3; 6], false)));
    // his Magic page is Sora's order, his set A is the best Fire / Blizzard / Thunder / Cure
    let menu = rig.party.pad_menu(rig.id).unwrap();
    assert_eq!(menu.lists[1].iter().map(|r| r.name).collect::<Vec<_>>(), ["Firaga", "Waterga", "Curaga", "Blizzaga", "Thundaga", "Aeroga"]);
    assert_eq!(menu.shortcut.iter().map(|r| r.name).collect::<Vec<_>>(), ["Firaga", "Blizzaga", "Thundaga", "Curaga"]);
    let mp0 = rig.m().status.mp;
    rig.shortcut(PadButton::FaceRight);
    assert_eq!(rig.m().state, NpcStateId::Action);
    assert_eq!(rig.m().action.map(|r| r.sora_cmd), Some(cmd::FIRAGA));
    assert_eq!(rig.m().anim.clip.name, "MagicFire", "Donald's own bolt clip");
    let mut spawned = None;
    for _ in 0..90 {
        rig.idle(1);
        if let Some(c) = rig.party.caster(rig.id) {
            if let Some(v) = c.projectiles().next() {
                spawned = Some(v.class);
                break;
            }
        }
    }
    assert_eq!(spawned, Some("TresProj_MagFigaNml_C"), "Sora's Firaga actor");
    assert_eq!(mp0 - rig.m().status.mp, 20, "Sora's Firaga cost (MP 20) from Donald's own MP");
    for _ in 0..120 {
        rig.idle(1);
    }
    let hit = rig.dummies.1.iter().find(|h| h.attack_name.starts_with("MAG_FI"));
    assert!(hit.is_some_and(|h| h.damage > 0.0), "Sora's row hits with Donald's Magic: {:?}", rig.dummies.1.iter().map(|h| h.attack_name).collect::<Vec<_>>());
    // the caster's actors are on the host's taps
    let _ = rig.party.caster_projectiles().count();
}

#[test]
#[ignore = "(DON-MAGIC) REMOVED by the Donald overhaul (DONALD_OVERHAUL.md 13, DON_PLAN R26): Donald no longer casts Sora's spells / Grand Magic (crate::donald::cast)"]
fn every_tier_casts_its_own_actor_with_its_clip() {
    for (c, clip, class) in [(cmd::THUNDAGA, "MagicCast", "Thundaga"), (cmd::BLIZZARA, "MagicFire", "Blizzara"), (cmd::AEROGA, "MagicCast", "Aeroga"), (cmd::WATAGA, "MagicCast", "Water")] {
        let mut f = feed([3; 6], false);
        f.customs[0].shortcuts[0][0] = c;
        let mut rig = Rig::new(Chara::Donald, Some(Vec3::new(600.0, 0.0, 0.0)), Some(f));
        rig.shortcut(PadButton::FaceRight);
        assert_eq!((rig.m().action.map(|r| r.sora_cmd), rig.m().anim.clip.name), (Some(c), clip), "{class}");
        let mut seen = Vec::new();
        for _ in 0..150 {
            rig.idle(1);
            if let Some(cs) = rig.party.caster(rig.id) {
                seen.extend(cs.projectiles().map(|v| v.class));
            }
        }
        assert!(!seen.is_empty(), "{class}: an actor of Sora's");
        let hits: Vec<&str> = rig.dummies.1.iter().map(|h| h.attack_name).collect();
        assert!(!hits.is_empty(), "{class}: hits {seen:?}");
    }
}

#[test]
#[ignore = "(DON-MAGIC) REMOVED by the Donald overhaul (DONALD_OVERHAUL.md 13, DON_PLAN R26): Donald no longer casts Sora's spells / Grand Magic (crate::donald::cast)"]
fn curaga_heals_him_takes_all_mp_and_the_friends_charge_refills_it() {
    let mut rig = Rig::new(Chara::Donald, None, Some(feed([3; 6], false)));
    let id = rig.id;
    let max = rig.party.status(id).unwrap().max_hp();
    rig.party.status_mut(id).unwrap().hp = 10;
    rig.shortcut(PadButton::FaceBottom);
    assert_eq!(rig.m().action.map(|r| r.sora_cmd), Some(cmd::CURAGA));
    rig.idle(120);
    let s = rig.party.status(id).unwrap();
    assert!(s.hp > 10 && s.hp <= max, "healed by Sora's Curaga ({} / {max})", s.hp);
    assert_eq!(s.mp, 0, "Cure costs all MP");
    let charge = s.mp_charge_left;
    // (DON-KIT) DON_PLAN R7: Donald's charge is the friends' 30 s x clamp(Sora's Magic / his, 0.6, 1)
    let want = kh_party::pad::donald::mp_charge_time(s);
    assert!(want < kh_party::tables::cparam(Chara::Donald, "mp_charge_time") && (charge - want).abs() < 2.1, "his R7 charge {want}: {charge}");
    rig.idle((31.0 / DT) as usize);
    let s = rig.party.status(id).unwrap();
    assert_eq!((s.mp, s.mp_charge_left), (s.max_mp(), 0.0), "refilled");
    // while the charge ran he could not cast (Sora's rule)
}

#[test]
#[ignore = "(DON-MAGIC) REMOVED by the Donald overhaul (DONALD_OVERHAUL.md 13, DON_PLAN R26): Donald no longer casts Sora's spells / Grand Magic (crate::donald::cast)"]
fn y_casts_the_grand_magic_offer_and_a_team_attack_comes_first() {
    let mut rig = Rig::new(Chara::Donald, Some(Vec3::new(600.0, 0.0, 0.0)), Some(feed([3; 6], false)));
    // a first cast makes his caster
    rig.shortcut(PadButton::FaceRight);
    rig.idle(90);
    let id = rig.id;
    rig.party.caster_mut(id).unwrap().debug_offer(112);
    rig.idle(1);
    let menu = rig.party.pad_menu(id).unwrap();
    assert_eq!(menu.situations.first().map(|s| (s.kind, s.icon_type)), Some((112, 3)), "the offer on his situation slot");
    // with a team attack in Sora's queue the offer waits (Y is Sora's)
    rig.feed = Some(feed([3; 6], true));
    rig.idle(1);
    assert!(rig.party.pad_menu(id).unwrap().situations.is_empty());
    rig.step(Vec2::ZERO, &[PadButton::FaceTop]);
    assert!(!rig.party.pad_state(id).unwrap().x2.y_taken);
    rig.idle(1);
    rig.feed = Some(feed([3; 6], false));
    rig.idle(1);
    let mp0 = rig.m().status.mp;
    rig.step(Vec2::ZERO, &[PadButton::FaceTop]);
    // (DON-KIT) Donald's Grand Magic on Y is gone (documentation/DONALD_OVERHAUL.md 6.3 / 13: the Arcane Y list replaces
    // it): the old offer's Y starts nothing and costs nothing (the X3D GRAND state no longer exists)
    rig.idle(90);
    assert!(kh_party::pad::donald::kit_state(rig.m()).is_none());
    assert_eq!(rig.m().status.mp, mp0);
}

#[test]
#[ignore = "(DON-MAGIC) REMOVED by the Donald overhaul (DONALD_OVERHAUL.md 13, DON_PLAN R26): Donald no longer casts Sora's spells / Grand Magic (crate::donald::cast)"]
fn the_magic_chains_fourth_cast_is_the_finish_end() {
    let mut rig = Rig::new(Chara::Donald, Some(Vec3::new(600.0, 0.0, 0.0)), Some(feed([3; 6], false)));
    rig.party.status_mut(rig.id).unwrap().mp = 999;
    let mut finishes = Vec::new();
    let wait_open = |rig: &mut Rig| {
        // Fire family: the follow-up opens straight after the shot
        for _ in 0..40 {
            let open = rig.m().state != NpcStateId::Action || kh_party::leader_magic::follow_open(rig.m());
            if open {
                break;
            }
            rig.idle(1);
        }
    };
    for _ in 0..3 {
        wait_open(&mut rig);
        rig.shortcut(PadButton::FaceRight);
        finishes.push(rig.party.pad_state(rig.id).unwrap().x2.current.map(|c| (c.chain, c.finish)));
    }
    assert_eq!(finishes, vec![Some((1, false)), Some((2, false)), Some((3, false))]);
    wait_open(&mut rig);
    // the 4th is the Finish End (X2 casts it with `MagicCast` and the `_FIN` rows unless X3D's kit replaces it with
    // Flare Force: `finish_next` is what X3D reads)
    assert!(kh_party::leader_magic::finish_next(rig.m()));
    rig.shortcut(PadButton::FaceRight);
    let fin = rig.party.pad_state(rig.id).unwrap().x2.current.filter(|c| c.chain == 4).map(|c| c.finish);
    assert!(fin.is_none_or(|f| f), "an X2 Finish End is marked finish");
}

#[test]
fn the_leader_has_no_friend_damage_halving_ai_friends_keep_it() {
    let mut party = Party::new(PartyConfig::test_level99());
    let d = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
    let g = party.spawn(Chara::Goofy, Vec3::new(200.0, 0.0, 0.0), 0.0);
    party.set_pad(d, Some(Tuning::USER));
    assert_eq!(party.defender(d).unwrap().damage_max, 0, "the leader: Sora's rules");
    assert_eq!(party.defender(g).unwrap().damage_max, 24, "an AI friend: KH3's m_DamageMax 24");
    // the damage function: a base above 24 on the leader stays (the AI friend: floor((24 + S - D) / 2))
    use kh_core::status::{compute_damage, Attacker, DamageAttack, DamageContext, Side};
    // S - D = 45 on the leader (between the 24 of m_DamageMax and the base cap 80)
    let strength = party.defender(d).unwrap().defense + 45;
    let attacker = Attacker { has_status: true, strength, magic: 0, abilities: None, side: Side::Other, state_boost: false };
    let attack = DamageAttack::physical(1.0);
    let lead = compute_damage(&attacker, &party.defender(d).unwrap(), &attack, &DamageContext::new(GameLevel::Standard, 1));
    let ai = compute_damage(&attacker, &party.defender(g).unwrap(), &attack, &DamageContext::new(GameLevel::Standard, 1));
    assert_eq!(lead.base, 45.0, "the leader: no halving");
    // Donald as an AI friend (the same body): floor((24 + 45) / 2) = 34
    party.set_pad(d, None);
    // (GOOFY-BAL) an AI friend has KH3's Defense again (the leader's is stream BAL's table): S - D = 45 on that one
    let attacker = Attacker { strength: party.defender(d).unwrap().defense + 45, ..attacker };
    let ai_donald = compute_damage(&attacker, &party.defender(d).unwrap(), &attack, &DamageContext::new(GameLevel::Standard, 1));
    assert_eq!(ai_donald.base, 34.0, "the AI friend: KH3's halving above 24");
    assert!(ai.base <= 80.0);
    party.set_pad(d, None);
    assert_eq!(party.defender(d).unwrap().damage_max, 24, "back to the AI: the friend rule again");
}

#[test]
fn goofy_leading_has_his_abilities_page_by_tier() {
    let mut rig = Rig::new(Chara::Goofy, Some(Vec3::new(400.0, 0.0, 0.0)), Some(feed([1, 2, 0, 0, 0, 1], false)));
    let menu = rig.party.pad_menu(rig.id).unwrap();
    assert_eq!(menu.main[1].name, "Abilities");
    assert_eq!(menu.lists[1].iter().map(|r| r.name).collect::<Vec<_>>(), ["Goofy Tornado II", "Goofy Turbo II", "Goofy Discus II"]);
    assert_eq!(menu.shortcut.iter().map(|r| r.name).collect::<Vec<_>>(), ["Goofy Tornado II", "Goofy Turbo II", "Goofy Discus II", "Potion"]);
    rig.shortcut(PadButton::FaceRight);
    assert_eq!(rig.m().action.and_then(|r| r.def), Some(kh_party::ActionDef::GoofyTornado));
    assert_eq!(rig.party.pad_state(rig.id).unwrap().x2.tier, 2);
    // the set flips with LB + D-pad down (Sora's page step): set B is empty
    rig.idle(200);
    rig.step(Vec2::ZERO, &[PadButton::LeftShoulder]);
    rig.step(Vec2::ZERO, &[PadButton::LeftShoulder, PadButton::DPadDown]);
    let menu = rig.party.pad_menu(rig.id).unwrap();
    assert_eq!((menu.shortcut_page, menu.shortcut[0].name), (1, ""));
    assert!(goofy::is_goofy(goofy::id(1, 2)));
}

#[test]
#[ignore = "(DON-UI) REPLACED by the Donald overhaul (DONALD_OVERHAUL.md 7, 13): his Magic lists his elements, fed or not (kh-party pad::menu::don_menu_tests)"]
fn without_the_feed_donald_keeps_his_kh3_spells() {
    let rig = Rig::new(Chara::Donald, None, None);
    let menu = rig.party.pad_menu(rig.id).unwrap();
    assert_eq!(menu.lists[1].iter().map(|r| r.name).collect::<Vec<_>>(), ["Fire", "Blizzard", "Thunder", "Aero", "Cure"]);
}

#[test]
fn a_friend_leader_sprints_after_one_second() {
    for chara in [Chara::Donald, Chara::Goofy] {
        let mut rig = Rig::new(chara, None, Some(feed([3; 6], false)));
        for _ in 0..50 {
            rig.step(Vec2::new(0.0, 1.0), &[]);
        }
        // (GOOFY-GK) Goofy leading runs / sprints slower than Sora and Donald (the user's request; goofy_kit K.move_*)
        let goofy = chara == Chara::Goofy;
        // (DON-KIT) Donald: run 680, sprint 900 (documentation/DONALD_OVERHAUL.md 8)
        let (run, sprint) = if goofy { (kh_party::pad::goofy_kit::K.move_run, kh_party::pad::goofy_kit::K.move_sprint) } else { (kh_party::pad::donald::K.move_run, kh_party::pad::donald::K.move_sprint) };
        assert_eq!(rig.m().cmc.max_walk_speed, run, "{chara:?}: not yet");
        for _ in 0..60 {
            rig.step(Vec2::new(0.0, 1.0), &[]);
        }
        let v = rig.m().cmc.velocity;
        assert_eq!(rig.m().cmc.max_walk_speed, sprint, "{chara:?}: MySprintSpeed");
        assert!(rig.party.pad_state(rig.id).unwrap().x2.sprinting);
        assert!((v.x * v.x + v.y * v.y).sqrt() > sprint - 80.0, "{chara:?}: {v:?}");
        rig.idle(30);
        assert!(!rig.party.pad_state(rig.id).unwrap().x2.sprinting, "{chara:?}: stopped");
    }
}

#[test]
#[ignore = "(DON-MAGIC) REMOVED by the Donald overhaul (DONALD_OVERHAUL.md 13, DON_PLAN R26): Donald no longer casts Sora's spells / Grand Magic (crate::donald::cast)"]
fn a_cast_at_exactly_zero_mp_starts_the_charge() {
    // Sora's UseMagicPoint: landing exactly on 0 does not start the charge, the next cast does (an overdraw)
    let mut f = feed([3; 6], false);
    f.customs[0].shortcuts[0][0] = cmd::WATAGA;
    let mut rig = Rig::new(Chara::Donald, Some(Vec3::new(600.0, 0.0, 0.0)), Some(f));
    rig.party.status_mut(rig.id).unwrap().mp = 0;
    rig.shortcut(PadButton::FaceRight);
    assert_eq!(rig.m().action.map(|r| r.sora_cmd), Some(cmd::WATAGA), "cast at MP 0: {:?} {:?}", rig.m().state, rig.party.pad_state(rig.id).unwrap().menu.events);
    rig.idle(60);
    let s = rig.party.status(rig.id).unwrap();
    assert!(s.mp == 0 && s.mp_charge_left > kh_party::pad::donald::mp_charge_time(s) - 5.0, "the overdraw starts his charge (DON-KIT: R7): {} {}", s.mp, s.mp_charge_left);
}

/// (DON-MAGIC) THE X2 DONALD BRANCH IS REMOVED (documentation/DONALD_OVERHAUL.md 13, DON_PLAN R26): a Donald leader's
/// shortcut of a Sora spell no longer casts it (his own magic is kh_party::donald::cast) and he gets no Grand Magic offer;
/// Goofy's Abilities page is untouched (goofy_leading_has_his_abilities_page_by_tier).
#[test]
fn donald_no_longer_casts_soras_magic_nor_gets_grand_magic() {
    let mut rig = Rig::new(Chara::Donald, Some(Vec3::new(700.0, 0.0, 0.0)), Some(feed([3; 6], false)));
    rig.shortcut(PadButton::FaceRight);
    rig.idle(90);
    assert!(rig.m().action.is_none_or(|r| r.sora_cmd == 0), "no Sora spell action");
    assert!(rig.party.caster(rig.id).is_none(), "no X2 caster (his own casts are donald::cast's)");
    assert!(rig.party.pad_menu(rig.id).unwrap().situations.iter().all(|s| s.icon_type != 3), "no Grand Magic offer");
    assert!(!kh_party::leader_magic::request_cast(rig.party.member_mut(rig.id).unwrap(), cmd::FIRAGA, kh_party::TargetRef::Enemy(TargetId(1))));
}

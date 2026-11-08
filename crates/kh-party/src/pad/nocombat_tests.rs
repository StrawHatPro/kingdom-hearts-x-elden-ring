//! (NOCOMBAT) THE USER (2026-11-03): "grey out all action commands except for item in the roundtable hold"; "this applies to
//! all characters". Every friend (`Chara::ALL`: a new one is walked here too), leading on the pad or run by the AI, under the
//! one rule (kh_core::nocombat): the menu greys but Items, A / Y / RB / LB + a face button strike nothing (a greyed entry is
//! refused aloud), the AI's actions, Cure and the team-attack hand-over are refused at the command intake; Items stays.

use super::*;
use crate::command::{ActionDef, CommandStatus, MemberCommand, TargetRef};
use crate::host::PartyHost;
use crate::{Party, PartyConfig, SoraView};
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetInfo, Targets};
use kh_core::input::PadButtons;
use kh_core::nocombat;
use kh_core::test_world::TestWorld;

const DT: f32 = 1.0 / 60.0;

struct Dummies(Vec<(TargetId, Vec3)>, Vec<HitRecord>);

impl Targets for Dummies {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        for &(id, p) in &self.0 {
            out.push(TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 });
        }
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        let mut all = Vec::new();
        self.enumerate(&mut all);
        all.into_iter().find(|t| t.id == id)
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
}

impl Rig {
    fn new(chara: Chara, pad: bool) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        if pad {
            party.set_pad(id, Some(Tuning::USER));
        }
        let dummies = Dummies(vec![(TargetId(1), Vec3::new(130.0, 0.0, 0.0))], Vec::new());
        let mut rig = Rig { world: TestWorld::floor(), dummies, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), id, held: PadButtons::NONE };
        rig.sora.position = Vec3::new(-2000.0, 0.0, 0.0);
        rig.idle(10);
        rig.dummies.1.clear();
        rig
    }

    fn m(&self) -> &NpcPawn {
        self.party.member(self.id).unwrap()
    }

    /// One step; the frame's menu events (taken before the step runs the members).
    fn step(&mut self, held: &[PadButton]) -> Vec<MenuEvent> {
        let now = held.iter().fold(PadButtons::NONE, |a, b| a.with(*b));
        let pad = Buttons { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        if let Some(pad) = self.party.member_mut(self.id).and_then(|m| m.pad.as_mut()).filter(|p| p.finisher_pending) {
            pad.finisher_pending = false;
            pad.finisher_answer = Some(false);
        }
        self.party.set_pad_frame(self.id, PadFrame { stick: Vec2::ZERO, camera_yaw: 0.0, pad });
        let events = self.m().pad.as_ref().map(|p| p.menu.events.clone()).unwrap_or_default();
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
        events
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    /// Presses a button (one frame down, `rest` frames up): whether its frame refused it.
    fn press(&mut self, b: PadButton, rest: usize) -> bool {
        let refused = self.step(&[b]).contains(&MenuEvent::Refused);
        self.idle(rest);
        refused
    }

    fn menu(&self) -> kh_core::command::CommandSnapshot {
        self.party.pad_menu(self.id).expect("a pad-driven member has a menu")
    }
}

#[test]
fn nocombat_every_friend_leading_strikes_nothing_and_hears_the_refusal() {
    for chara in Chara::ALL {
        let _zone = nocombat::scope(true);
        let mut rig = Rig::new(chara, true);
        // the greyed menu model
        let snap = rig.menu();
        for r in snap.main.iter().filter(|r| r.kind != 0 || !r.name.is_empty()) {
            assert_eq!(r.enabled && r.kind == nocombat::ITEMS_MAIN, r.enabled, "{chara:?}: only Items may stay lit ({})", r.name);
        }
        assert!(!snap.main[0].enabled, "{chara:?}: Attack greyed");
        // A on the greyed Attack: refused aloud, nothing struck, mashed for a second
        assert!(rig.press(PadButton::ATTACK, 2), "{chara:?}: the refused SE on A");
        for _ in 0..30 {
            rig.press(PadButton::ATTACK, 1);
        }
        // Y (Goofy Breaker, Donald's Arcane list), RB held (Spell Volley), LB + every face button (Arts, quick casts)
        rig.press(PadButton::SITUATION, 20);
        for _ in 0..40 {
            rig.step(&[PadButton::RightShoulder]);
        }
        rig.idle(5);
        for face in [PadButton::FaceRight, PadButton::FaceTop, PadButton::FaceLeft, PadButton::FaceBottom] {
            rig.step(&[PadButton::LeftShoulder]);
            rig.step(&[PadButton::LeftShoulder, face]);
            rig.idle(20);
        }
        rig.idle(120);
        assert!(rig.dummies.1.is_empty(), "{chara:?}: no hit reached the dummy: {:?}", rig.dummies.1.len());
    }
}

#[test]
fn nocombat_outside_the_zone_the_same_presses_strike() {
    for chara in Chara::ALL {
        let _zone = nocombat::scope(false);
        let mut rig = Rig::new(chara, true);
        for _ in 0..60 {
            rig.press(PadButton::ATTACK, 3);
        }
        rig.idle(60);
        assert!(!rig.dummies.1.is_empty(), "{chara:?}: the comparison: his A strikes outside the zone");
    }
}

#[test]
fn nocombat_every_friend_s_items_stay_usable() {
    for chara in Chara::ALL {
        let _zone = nocombat::scope(true);
        let mut rig = Rig::new(chara, true);
        let has_items = !menu::items(&rig.m().status).is_empty();
        // the cursor to Items (each friend's own row order)
        for _ in 0..6 {
            let snap = rig.menu();
            if snap.main.get(snap.main_cursor).is_some_and(|r| r.kind == nocombat::ITEMS_MAIN) {
                break;
            }
            rig.press(PadButton::DPadDown, 1);
        }
        let snap = rig.menu();
        assert_eq!(snap.main[snap.main_cursor].kind, nocombat::ITEMS_MAIN, "{chara:?}: the cursor reached Items");
        assert_eq!(snap.main[snap.main_cursor].enabled, has_items, "{chara:?}: Items lit while he has items");
        if has_items {
            assert!(!rig.press(PadButton::ATTACK, 1), "{chara:?}: Items opens without a refusal");
            assert_eq!(rig.menu().state, kh_core::command::MenuState::SubSel, "{chara:?}: the Items list is open");
            assert!(rig.menu().lists[rig.menu().list].iter().any(|r| r.enabled), "{chara:?}: its rows are lit");
        }
    }
}

#[test]
fn nocombat_the_ai_s_actions_cure_and_team_attacks_are_refused_at_the_intake() {
    for chara in Chara::ALL {
        let def = ActionDef::ALL.into_iter().find(|d| d.chara() == chara).expect("an action of his");
        let target = TargetRef::Enemy(TargetId(1));
        for (on, want) in [(true, false), (false, true)] {
            let _zone = nocombat::scope(on);
            let mut rig = Rig::new(chara, false);
            let id = rig.id;
            let t = rig.party.command(id, MemberCommand::Action { def, target });
            let running = rig.party.member(id).unwrap().command_status(t) == CommandStatus::Running;
            assert_eq!(running, want, "{chara:?} {def:?} with the zone {on}");
            if on {
                let t = rig.party.command(id, MemberCommand::EnterFrState { state: 1 });
                assert_eq!(rig.party.member(id).unwrap().command_status(t), CommandStatus::Failed, "{chara:?}: no team attack");
                let t = rig.party.command(id, MemberCommand::Cure { target: TargetRef::Sora });
                assert_eq!(rig.party.member(id).unwrap().command_status(t), CommandStatus::Failed, "{chara:?}: no Cure either (Magic greys)");
                // moving stays
                let t = rig.party.command(id, MemberCommand::MoveTo { goal: crate::command::MoveGoal::Point(Vec3::new(300.0, 0.0, 0.0)), locomotion: Default::default(), walk: false, acceptable_radius: 50.0, follow_path: false });
                assert_eq!(rig.party.member(id).unwrap().command_status(t), CommandStatus::Running, "{chara:?}: he still follows");
            }
        }
    }
}

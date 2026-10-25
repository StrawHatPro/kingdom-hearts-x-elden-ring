//! (party P) The pad drive: Sora's controls and the user's movement options on Donald / Goofy.

use super::*;
use crate::host::PartyHost;
use crate::{Party, PartyConfig, SoraView};
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetInfo, Targets};
use kh_core::input::PadButtons;
use kh_core::test_world::TestWorld;

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

struct Rig {
    world: TestWorld,
    dummies: Dummies,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    id: MemberId,
    held: PadButtons,
    /// The host's answer to the leader-unison roll (stream D's D.6): None = it never answers.
    unison: Option<bool>,
}

impl Rig {
    fn new(chara: Chara, dummy: Option<Vec3>) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        let dummies = Dummies(dummy.map(|p| vec![(TargetId(1), p)]).unwrap_or_default(), Vec::new());
        let mut rig = Rig { world: TestWorld::floor(), dummies, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), id, held: PadButtons::NONE, unison: Some(false) };
        rig.sora.position = Vec3::new(-2000.0, 0.0, 0.0);
        rig.idle(10);
        rig
    }

    fn m(&self) -> &NpcPawn {
        self.party.member(self.id).unwrap()
    }

    /// One step with this stick and these buttons held (presses = the edges).
    fn step(&mut self, stick: Vec2, held: &[PadButton]) {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = Buttons { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        // the host's part of the finisher (party_play.rs: FriendState::roll_leader_unison)
        if let Some(pad) = self.party.member_mut(self.id).and_then(|m| m.pad.as_mut()).filter(|p| p.finisher_pending) {
            pad.finisher_pending = false;
            pad.finisher_answer = self.unison;
        }
        self.party.set_pad_frame(self.id, PadFrame { stick, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(Vec2::ZERO, &[]);
        }
    }

    fn speed(&self) -> f32 {
        horizontal(self.m().cmc.velocity).length()
    }
}

#[test]
fn the_stick_is_camera_relative_with_the_games_dead_zone() {
    let (yaw, mag) = stick_of(Vec2::new(0.0, 1.0), 30.0);
    assert!((yaw - 30.0).abs() < 1e-4 && (mag - 1.0).abs() < 1e-5);
    let (yaw, _) = stick_of(Vec2::new(1.0, 0.0), 0.0);
    assert!((yaw - 90.0).abs() < 1e-4, "stick right = the camera's right (+Y)");
    let (_, mag) = stick_of(Vec2::new(0.0, 0.2), 0.0);
    assert_eq!(mag, 0.0, "inside the 0.25 dead zone");
}

#[test]
fn start_lunge_then_run_with_the_speed_matched_loop() {
    for chara in [Chara::Donald, Chara::Goofy] {
        let mut rig = Rig::new(chara, None);
        rig.step(Vec2::new(0.0, 1.0), &[]);
        assert_eq!(rig.m().state, NpcStateId::Pad(RUN), "{chara:?}");
        rig.step(Vec2::new(0.0, 1.0), &[]);
        assert_eq!(rig.m().anim.clip.name, if chara == Chara::Donald { "RunStartN" } else { "RunStartN" }, "the start lunge (the members' RunStartN)");
        assert!(!rig.m().anim.clip.root_motion, "the start clip's root motion is the pad's (blended samples)");
        for _ in 0..60 {
            rig.step(Vec2::new(0.0, 1.0), &[]);
        }
        let s = rig.speed();
        // (GOOFY-GK) Goofy leading runs slower (goofy_kit K.move_run, the user's request)
        let want = if chara == Chara::Goofy { super::goofy_kit::K.move_run } else { 680.0 };
        assert!((s - want).abs() < 5.0, "{chara:?} run speed {s}");
        let loco = rig.m().pad.as_ref().unwrap().loco.expect("speed-matched pose");
        assert!(loco.run_weight > if chara == Chara::Goofy { 0.7 } else { 0.95 } /* (GOOFY-GK) his slower run sits in his own blend space */, "{loco:?}");
        assert!(rig.m().position().x > if chara == Chara::Goofy { 480.0 } else { 600.0 } /* (GOOFY-GK) */, "moved forward {:?}", rig.m().position());
        // let go: the stop clip, then Idle
        for _ in 0..60 {
            rig.step(Vec2::ZERO, &[]);
        }
        assert_eq!(rig.m().state, NpcStateId::Idle);
    }
}

#[test]
fn walk_to_run_ramps_the_speed_limit() {
    let mut rig = Rig::new(Chara::Donald, None);
    for _ in 0..40 {
        rig.step(Vec2::new(0.0, 0.55), &[]);
    }
    let walk = rig.speed();
    assert!((walk - 140.0).abs() < 3.0, "walk {walk}");
    assert_eq!(rig.m().pad.as_ref().unwrap().loco.unwrap().run_weight, 0.0);
    // full stick after the start window: no run-start replay (dec013), the limit ramps over 0.25 s (dec015)
    let mut speeds = Vec::new();
    for _ in 0..30 {
        rig.step(Vec2::new(0.0, 1.0), &[]);
        speeds.push(rig.speed());
        assert_ne!(rig.m().anim.clip.name, "RunStartN", "continuous: no start clip from a walk");
    }
    assert!(speeds[5] > 150.0 && speeds[5] < 600.0, "mid-ramp {}", speeds[5]);
    assert!((speeds[29] - 680.0).abs() < 5.0, "ramped to {}", speeds[29]);
}

#[test]
fn rolling_the_stick_round_keeps_running_a_flick_plays_the_reversal() {
    let mut rig = Rig::new(Chara::Goofy, None);
    for _ in 0..40 {
        rig.step(Vec2::new(0.0, 1.0), &[]);
    }
    // a slow circle: 90 degrees per second
    for i in 0..120 {
        let a = (i as f32 * 1.5).to_radians();
        rig.step(Vec2::new(a.sin(), a.cos()), &[]);
        assert_eq!(rig.m().pad.as_ref().unwrap().run.sub, 0, "step {i}: no turn-around while the stick is rolled round");
    }
    let facing = rig.m().yaw;
    // a flick to the opposite direction
    let back = (facing + 180.0).to_radians();
    let mut reversed = false;
    for _ in 0..10 {
        rig.step(Vec2::new(back.sin(), back.cos()), &[]);
        if rig.m().pad.as_ref().unwrap().run.sub == 2 {
            reversed = true;
            assert!(rig.m().anim.axis.abs() > 135.0, "the turn-around sample (BL / BR), axis {}", rig.m().anim.axis);
        }
    }
    assert!(reversed, "the reversal clip on the flick");
}

#[test]
fn jump_height_is_cut_on_release_with_the_minimum() {
    let height = |hold: usize| {
        let mut rig = Rig::new(Chara::Donald, None);
        // (party X3D) B held while falling is Aero Float (XD 1.6): this test is the jump's physics alone
        rig.party.member_mut(rig.id).unwrap().status.kit.take(crate::pad::donald_kit::KitAbility::AeroFloat);
        let mut top = 0.0f32;
        for i in 0..120 {
            let held: &[PadButton] = if i < hold { &[PadButton::JUMP] } else { &[] };
            rig.step(Vec2::ZERO, held);
            top = top.max(rig.m().position().z);
        }
        assert_eq!(rig.m().state, NpcStateId::Idle, "landed");
        top
    };
    let (tap, full) = (height(1), height(60));
    assert!(tap >= Tuning::USER.min_jump_height - 1.0, "a tap reaches the minimum height: {tap}");
    assert!(full > tap + 60.0, "holding B jumps higher: {full} vs {tap}");
    assert!(full < 320.0, "Sora's jump physics: {full}");
}

#[test]
fn the_a_combo_chains_beat_swing_and_the_jump_attack() {
    // (DON-KIT) Donald's A combo is the overhaul's magic missiles (documentation/DONALD_OVERHAUL.md 3.1, pad/donald_combo.rs):
    // wave left, wave right, the slam; the Beat / Swing / Jump Strike melee chain is gone (doc 13)
    let mut rig = Rig::new(Chara::Donald, Some(Vec3::new(1200.0, 0.0, 0.0)));
    for i in 0..240 {
        let held: &[PadButton] = if i % 8 == 0 { &[PadButton::ATTACK] } else { &[] };
        rig.step(Vec2::ZERO, held);
        assert!(!matches!(rig.m().action.and_then(|r| r.def), Some(ActionDef::DonaldBeat | ActionDef::DonaldSwing | ActionDef::DonaldJump)));
    }
    for _ in 0..90 {
        rig.step(Vec2::ZERO, &[]);
    }
    let rows: Vec<&str> = rig.dummies.1.iter().map(|h| h.attack_name).collect();
    assert!(rows.contains(&"DON_MISSILE") && rows.contains(&"DON_SLAM"), "{rows:?}");
}

// (DON-UI) the Donald overhaul replaced his KH3 five-spell list (DONALD_OVERHAUL.md 7, 13): his Magic lists the elements
// he has, each opening its spells; the cast itself is DON-KIT's route (`MenuAction::DonCast` -> `pad::donald::wants_cast`).
// The menu's own rules: pad::menu::don_menu_tests.
#[test]
fn donald_magic_menu_lists_his_elements_then_their_spells() {
    let mut rig = Rig::new(Chara::Donald, Some(Vec3::new(600.0, 0.0, 0.0)));
    let snap = rig.party.pad_menu(rig.id).unwrap();
    assert_eq!(snap.main.map(|r| r.name), ["Attack", "Magic", "Attunement", "Items"]);
    let plain = |t: &str| t.rsplit('>').next().unwrap_or(t).trim().to_string();
    assert_eq!(snap.lists[1].iter().map(|r| plain(r.name)).collect::<Vec<_>>(), ["Fire", "Water"], "the start's elements");
    rig.step(Vec2::ZERO, &[PadButton::DPadDown]);
    rig.idle(1);
    rig.step(Vec2::ZERO, &[PadButton::ATTACK]);
    assert_eq!(rig.party.pad_menu(rig.id).unwrap().state, kh_core::command::MenuState::SubSel);
    rig.idle(1);
    rig.step(Vec2::ZERO, &[PadButton::ATTACK]);
    let snap = rig.party.pad_menu(rig.id).unwrap();
    assert_eq!(snap.lists[1].iter().map(|r| r.name).collect::<Vec<_>>(), ["Fire", "Flare"], "Fire's list: line 1, line 2");
}

#[test]
fn goofy_lists_abilities_and_turbo_runs_from_the_menu() {
    let mut rig = Rig::new(Chara::Goofy, Some(Vec3::new(700.0, 0.0, 0.0)));
    let snap = rig.party.pad_menu(rig.id).unwrap();
    assert_eq!(snap.main[1].name, "Abilities", "the user's request: Abilities replaces Magic for Goofy");
    assert_eq!(snap.lists[1].iter().map(|r| r.name).collect::<Vec<_>>(), ["Goofy Tornado", "Goofy Turbo", "Goofy Discus"]);
    rig.step(Vec2::ZERO, &[PadButton::DPadRight]);
    rig.idle(1);
    // cursor is on the main row 0 (Attack): D-pad right does not open it; go down first
    rig.step(Vec2::ZERO, &[PadButton::DPadDown]);
    rig.idle(1);
    rig.step(Vec2::ZERO, &[PadButton::DPadRight]);
    rig.idle(1);
    rig.step(Vec2::ZERO, &[PadButton::DPadDown]);
    rig.idle(1);
    rig.step(Vec2::ZERO, &[PadButton::ATTACK]);
    rig.idle(2);
    assert_eq!(rig.m().action.and_then(|r| r.def), Some(ActionDef::GoofyTurbo));
}

#[test]
fn lock_on_toggles_on_rb() {
    // (party X3D) Donald holds RB for Spell Volley (XD 2.8), so his lock toggles on a short tap's RELEASE (Sora's rule,
    // aim2.input.tap_max_time); Goofy keeps the press
    let mut rig = Rig::new(Chara::Donald, Some(Vec3::new(900.0, 0.0, 0.0)));
    rig.step(Vec2::ZERO, &[PadButton::RightShoulder]);
    rig.idle(1);
    assert_eq!(rig.m().pad.as_ref().unwrap().lock, TargetId(1));
    rig.step(Vec2::ZERO, &[PadButton::RightShoulder]);
    rig.idle(1);
    assert_eq!(rig.m().pad.as_ref().unwrap().lock, TargetId::NONE);
}

#[test]
fn handing_back_to_the_ai_restores_the_member() {
    let mut rig = Rig::new(Chara::Goofy, None);
    for _ in 0..20 {
        rig.step(Vec2::new(0.0, 1.0), &[]);
    }
    let own = crate::pawn::npc_params(Chara::Goofy);
    assert_eq!(rig.m().params.gravity_jump_up_scale, SORA_PARAMS.gravity_jump_up_scale);
    rig.party.set_pad(rig.id, None);
    assert!(rig.m().pad.is_none());
    assert_eq!(rig.m().params, own);
    assert_eq!(rig.m().state, NpcStateId::Idle);
}

#[test]
fn a_unison_on_the_leaders_combo_replaces_the_finisher() {
    let mut rig = Rig::new(Chara::Goofy, Some(Vec3::new(150.0, 0.0, 0.0)));
    rig.unison = Some(true);
    for i in 0..240 {
        let held: &[PadButton] = if i % 8 == 0 { &[PadButton::ATTACK] } else { &[] };
        rig.step(Vec2::ZERO, held);
    }
    // (GOOFY-GK) a pad Goofy's combo is his kit's own (pad/goofy_combo.rs): Push, Sweep, then the unison instead of
    // his finisher
    let log = rig.m().pad.as_ref().unwrap().goofy.log.clone();
    let steps: Vec<&str> = log.iter().copied().filter(|w| w.starts_with("combo_") || w.starts_with("fin_") || *w == "team_finisher").collect();
    assert_eq!(&steps[..2], &["combo_push", "combo_sweep"], "{log:?}");
    assert!(steps.contains(&"team_finisher"), "the roll's yes ends the string in the team move: {log:?}");
    assert!(!steps.contains(&"fin_big_bash"), "the unison (Sora's core) replaces the finisher: {log:?}");
}

/// (party P2) Goofy's battle walk / run blend through his mid sample 1016a (WalkRunB: 1016 at 140, 1016a at 410, 1017 at
/// 680): walk only at 0, 1016a only at the mid, the run samples only at 1; between them two neighbours.
#[test]
fn goofy_battle_loco_passes_the_mid_sample() {
    let runs = [("1017", 0.6), ("1017R", 0.4)];
    let wm = (410.0 - 140.0) / (680.0 - 140.0);
    let at = |w: f32| loco_three(("1016", 1.2), ("1016a", 0.8666667), runs, 0.6, 0.5, w, wm);
    let low = at(0.0);
    assert_eq!((low.walk, low.run[0].0, low.run_weight), ("1016", "1016a", 0.0));
    let mid = at(wm);
    assert_eq!((mid.walk, mid.run[0].0, mid.run_weight), ("1016", "1016a", 1.0));
    assert!((mid.run_time - 0.5 * 0.8666667).abs() < 1e-5, "1016a on its own time of the shared phase");
    let quarter = at(wm * 0.5);
    assert!((quarter.run_weight - 0.5).abs() < 1e-5);
    let high = at(1.0);
    assert_eq!((high.walk, high.run, high.run_weight), ("1016a", runs, 1.0));
    let three_quarter = at((1.0 + wm) * 0.5);
    assert_eq!(three_quarter.walk, "1016a");
    assert!((three_quarter.run_weight - 0.5).abs() < 1e-5);
}

// ------------------------------------------------------------------------------------------------ (party P3)

/// (party P3) From standstill with a real stick (it passes the walk zone first): the start clip is drawn (no stale
/// walk pose in `loco`), it lasts its samples' length (0.233 s straight ahead, not the first sample's 0.30 s), it blends
/// in over the Run state's 0.067 s, and the loop follows.
#[test]
fn the_run_start_from_a_walk_is_drawn_and_ends_with_its_samples() {
    for chara in [Chara::Donald, Chara::Goofy] {
        let mut rig = Rig::new(chara, None);
        let mut start_steps = 0;
        let mut first_weight = None;
        for i in 0..40 {
            let y = ((i + 1) as f32 / 6.0).min(1.0);
            rig.step(Vec2::new(0.0, y), &[]);
            let m = rig.m();
            if m.anim.clip.name.starts_with("RunStart") {
                start_steps += 1;
                assert!(m.pad.as_ref().unwrap().loco.is_none(), "{chara:?}: no speed-matched pose over the start clip");
                first_weight.get_or_insert(m.anim.weight);
                assert!((m.anim.clip.length - 0.2333).abs() < 0.01, "{chara:?}: start length {}", m.anim.clip.length);
                assert!((m.anim.clip.blend_in - kh_core::AnimId::RunStartN.data().blend_in).abs() < 1e-4);
            }
        }
        assert!((13..=15).contains(&start_steps), "{chara:?}: the start clip ran {start_steps} steps (0.233 s)");
        assert!(first_weight.unwrap() < 0.5, "{chara:?}: blended in, not snapped");
        assert!(rig.m().pad.as_ref().unwrap().loco.is_some(), "the loop after the start");
    }
}

/// (party P3) The start length between two samples of different lengths (0007L 0.233 s, 0007BL 0.30 s) is weighted.
#[test]
fn the_start_length_is_the_blend_spaces() {
    let l = |axis: f32| start_length(Chara::Goofy, false, axis).unwrap();
    assert!((l(0.0) - 0.2333).abs() < 1e-3);
    assert!((l(-180.0) - 0.30).abs() < 1e-3);
    assert!((l(-135.0) - (0.2333 + 0.30) / 2.0).abs() < 2e-3);
}

/// (party P3) Mashing A carries the combo through Push, Sweep and the jump attack (a press waits for its chain point).
#[test]
fn mashing_a_carries_goofys_combo() {
    let mut rig = Rig::new(Chara::Goofy, Some(Vec3::new(150.0, 0.0, 0.0)));
    for i in 0..300 {
        let held: &[PadButton] = if i % 20 == 0 { &[PadButton::ATTACK] } else { &[] };
        rig.step(Vec2::ZERO, held);
    }
    // (GOOFY-GK) a pad Goofy's combo is his kit's own (pad/goofy_combo.rs): mashing carries it to its finisher
    let log = rig.m().pad.as_ref().unwrap().goofy.log.clone();
    let steps: Vec<&str> = log.iter().copied().filter(|w| w.starts_with("combo_") || w.starts_with("fin_") || w.starts_with("fusion_")).collect();
    assert!(steps.len() >= 3 && steps[..2] == ["combo_push", "combo_sweep"], "{log:?}");
}

/// (party P3) Two air strikes from a short hop: the strike hovers (gravity off) until its EnableChange, the second
/// press finds its chain point, then he falls and lands.
#[test]
fn two_air_strikes_hover_and_chain() {
    let mut rig = Rig::new(Chara::Goofy, None);
    let mut clips = Vec::new();
    let mut hovered = false;
    for i in 0..200 {
        let held: &[PadButton] = match i {
            0..=3 => &[PadButton::JUMP],
            12 | 30 => &[PadButton::ATTACK],
            _ => &[],
        };
        rig.step(Vec2::ZERO, held);
        let name = rig.m().anim.clip.name;
        if clips.last() != Some(&name) {
            clips.push(name);
        }
        // (GOOFY-GK) a pad Goofy's air strikes are his kit's AIR_COMBO steps (same clips, same hover)
        hovered |= matches!(rig.m().state, NpcStateId::Pad(AIR_ATTACK) | NpcStateId::Pad(super::goofy_combo::AIR_COMBO)) && rig.m().cmc.gravity_off && !rig.m().on_ground();
    }
    assert!(hovered, "the strike hovers");
    let i1 = clips.iter().position(|c| *c == "M_1312").expect("first strike");
    let i2 = clips.iter().position(|c| *c == "M_1311").expect("second strike");
    assert!(i2 > i1, "{clips:?}");
    assert_eq!(rig.m().state, NpcStateId::Idle, "landed: {clips:?}");
    assert!(!rig.m().cmc.gravity_off);
}

/// (party P3) The free Discus: no target, the stick held to the right: he turns to it, the shield flies out along
/// it, comes back and he catches it; the hand weapon is hidden while it is out.
#[test]
fn a_free_discus_flies_out_along_the_stick_and_comes_back() {
    let mut rig = Rig::new(Chara::Goofy, None);
    // Abilities -> Goofy Discus (third row)
    for b in [PadButton::DPadDown, PadButton::DPadRight, PadButton::DPadDown, PadButton::DPadDown] {
        rig.step(Vec2::ZERO, &[b]);
        rig.idle(1);
    }
    rig.step(Vec2::new(1.0, 0.0), &[PadButton::ATTACK]);
    let mut far = 0.0f32;
    let mut out_at = None;
    let mut caught = false;
    let mut hidden = false;
    for i in 0..400 {
        rig.step(Vec2::new(1.0, 0.0), &[]);
        if let Some(s) = rig.party.thrown_shield(rig.id) {
            out_at.get_or_insert(i);
            let d = s.position - rig.m().capsule_centre();
            far = far.max(d.length());
            if d.length() > 200.0 {
                assert!(d.y > d.x.abs() * 2.0, "along the stick (+Y = the camera's right): {d:?}");
            }
            hidden |= rig.party.weapon_hidden(rig.id);
        }
        if rig.m().anim.clip.name == "M_1329" {
            caught = true;
            assert!(rig.party.thrown_shield(rig.id).is_none(), "back in his hand at the catch");
        }
        if caught && rig.m().state == NpcStateId::Idle {
            break;
        }
    }
    assert!(out_at.is_some(), "thrown");
    assert!(far > 1000.0 && far < 2200.0, "out about half the attack time at its speed: {far}");
    assert!(caught, "caught");
    assert!(hidden, "the hand weapon is hidden while the shield is out");
    assert!(!rig.party.weapon_hidden(rig.id));
    assert_eq!(rig.m().state, NpcStateId::Idle);
}

/// (party P3) Turbo steered by the stick: held to the right, his heading turns to +Y within a second; let go, it holds.
#[test]
fn the_stick_steers_goofys_turbo() {
    // a target to his left: its TurnToTarget window must not pull him there against the stick
    let mut rig = Rig::new(Chara::Goofy, Some(Vec3::new(300.0, -600.0, 0.0)));
    for b in [PadButton::DPadDown, PadButton::DPadRight, PadButton::DPadDown] {
        rig.step(Vec2::ZERO, &[b]);
        rig.idle(1);
    }
    rig.step(Vec2::ZERO, &[PadButton::ATTACK]);
    let mut riding = 0;
    for _ in 0..400 {
        rig.step(Vec2::new(1.0, 0.0), &[]);
        if rig.m().anim.clip.name == "M_1325" {
            riding += 1;
            if riding == 70 {
                assert!((wrap180(rig.m().yaw - 90.0)).abs() < 2.0, "turned to the stick: yaw {}", rig.m().yaw);
                let v = rig.m().cmc.velocity;
                assert!(v.y > 800.0, "sliding along it at the game's speed: {v:?}");
                break;
            }
        }
    }
    assert_eq!(riding, 70, "rode the shield");
    let yaw = rig.m().yaw;
    for _ in 0..30 {
        rig.step(Vec2::ZERO, &[]);
    }
    assert!((wrap180(rig.m().yaw - yaw)).abs() < 0.5, "no weave without the stick: {} -> {}", yaw, rig.m().yaw);
}

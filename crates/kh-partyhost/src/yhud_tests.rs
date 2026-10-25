//! (YHUD) THE Y STACK IS THE CONTROLLED CHARACTER'S, IN BOTH HOST PATHS (the user, 2026-10-23, Donald leading in Elden
//! Ring: "some of Sora's Y moves were still appearing in my HUD as Donald, ONLY the Y moves for the character you're
//! playing as have their Y moves show up on the HUD").
//!
//! The shared runtime (the code both hosts run: `PartyRuntime::route`), the friends' Y-capable states on (Donald's full
//! Arcane Flow, Goofy's Bombardier ready) and Sora's queue holding every kind of Y command: his Grand Magic, a formchange,
//! the finish in slot B and the four team attacks (193 Trinity Guard: Donald + Goofy, 195 Bombardier: Goofy, 207 / 208
//! Donald Meteor / Flare Force: Donald). The HUD's stack (kh-host `y_stack`, the function `Hud::live_model` draws) is
//! read from the two hosts' snapshots: the test level's (kh-app gfx_hud: `hud_snapshot` over Sora's live snapshot) and
//! Elden Ring's (kh-er partyui: `hud_snapshot` over a default snapshot, its command laid on the live one).
//! - Sora leads: his whole queue and his event button; never a friend's own row.
//! - Donald leads: his Arcane rows + 193 / 207 / 208 only (6.6: "Donald's with Sora and with Goofy"); with no Arcane
//!   charge and no team attack the stack is EMPTY (it was Sora's queue).
//! - Goofy leads: 193 / 195 only while they stand (Y is the team attack's), else his Bombardier alone.
//! - Y: the leader's selected team row runs (chosen in Sora's queue); Sora's own rows are never run by the player's Y.

use kh_core::combat::situation::{class, Entry, Slot};
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetId, TargetInfo};
use kh_core::input::PadButton;
use kh_core::{PadButtons, PadState, UiSnapshot, Vec3};
use kh_host::party::PartyChara;

use crate::k1_tests::Harness;

struct NoEnemies;

impl kh_core::Targets for NoEnemies {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
}

impl crate::PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

const GRAND: u32 = 9001;
const FORM: u32 = 9002;
const TRINITY: u32 = 9003;
const BOMBARDIER: u32 = 9004;
const METEOR: u32 = 9005;
const FLARE: u32 = 9006;
const FINISH: u32 = 9007;
const SORA_OWN: [u32; 3] = [GRAND, FORM, FINISH];

fn entry(class: u8, kind: i32, serial: u32) -> Entry {
    Entry { ex: false, class, kind, total: -1.0, remaining: -1.0, start_delay: 0.5, appear: 0.0, serial, selectable: true }
}

/// Sora's queue with his own Y commands and the team attacks `team` (by serial).
fn plant(h: &mut Harness, team: &[u32]) {
    let q = &mut h.sora.combat_mut().situation;
    q.list.clear();
    q.list.push(entry(class::GRAND_MAGIC, 30, GRAND));
    q.list.push(entry(class::FORMCHANGE, 128, FORM));
    for (serial, kind) in [(TRINITY, 193), (BOMBARDIER, 195), (METEOR, 207), (FLARE, 208)] {
        if team.contains(&serial) {
            q.list.push(entry(class::TEAM, kind, serial));
        }
    }
    q.slot_a = None;
    q.slot_b = Some(entry(class::FINISH, 171, FINISH));
    q.selected = -1;
}

/// One route (the hosts' first call of a step) with the pad `now` (the buttons of the previous route: `before`).
fn route(h: &mut Harness, now: PadButtons, before: PadButtons) -> kh_core::Input {
    let (lp, ly) = h.leader_pos();
    let (s, c) = ly.to_radians().sin_cos();
    let camera = kh_camera::CameraView { position: lp - Vec3::new(c, s, 0.0) * 450.0 + Vec3::Z * 220.0, rotation: kh_camera::Rot { pitch: -15.0, yaw: ly, roll: 0.0 }, rotation_target: kh_camera::Rot { pitch: -15.0, yaw: ly, roll: 0.0 }, look_point: lp, distance: 450.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
    let input = kh_core::Input { camera_yaw: ly, pad: PadState::from_held(now, before), ..kh_core::Input::default() };
    h.rt.route(crate::RouteIn { sora: &mut h.sora, input: &input, dt: 1.0 / 60.0, targets: &NoEnemies, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: h.t })
}

/// The HUD's Y stack of both hosts: (serials, names, an event button stands), [test level, Elden Ring].
fn stacks(h: &Harness) -> [(Vec<u32>, Vec<String>, bool); 2] {
    let name = |k: i64, fallback: &str| if fallback.is_empty() { format!("kind {k}") } else { fallback.to_string() };
    let friend_leads = h.rt.leader_chara() != PartyChara::Sora;
    let base = h.sora.ui_snapshot();
    // the test level (kh-app gfx_hud/mod.rs: `party_play::hud_snapshot(&party, &seam.ui.snapshot)`, else Sora's own)
    let app = h.rt.hud_snapshot(&base).unwrap_or_else(|| base.clone());
    // Elden Ring (kh-er partyui.rs: `rt.hud_snapshot(&UiSnapshot::default())` in the feed, its command over the live base)
    let mut er = base.clone();
    if let Some(l) = h.rt.hud_snapshot(&UiSnapshot::default()) {
        er.command = l.command;
    }
    [&app, &er].map(|s| {
        let y = kh_host::hud::y_stack(&s.command, &s.situation, friend_leads, &name);
        (y.situations.iter().map(|r| r.serial).collect(), y.situations.iter().map(|r| r.name.clone()).collect(), y.event_button.is_some())
    })
}

fn is_arcane(serial: u32) -> bool {
    serial & 0xFF00_0000 == 0x5D00_0000
}

fn is_bombardier(serial: u32) -> bool {
    serial & 0xFFFF_FFF0 == 0x5A00_0010
}

fn lead(h: &mut Harness, c: PartyChara) {
    h.rt.lead_word(&["leader", c.name()]);
    for _ in 0..30 {
        h.step(None);
    }
    assert_eq!(h.rt.leader_chara(), c, "{c:?} leads");
}

/// Y pressed (one route), the next route released: (the routed pad had Y, Sora's queue's current entry after the press).
fn press(h: &mut Harness, b: PadButton) -> (bool, Option<(Slot, i32)>) {
    let out = route(h, PadButtons::NONE.with(b), PadButtons::NONE);
    let current = h.sora.combat().situation.current();
    let _ = route(h, PadButtons::NONE, PadButtons::NONE.with(b));
    (out.pad.pressed.has(PadButton::FaceTop), current)
}

const ALL_TEAM: [u32; 4] = [TRINITY, BOMBARDIER, METEOR, FLARE];

#[test]
fn yhud_sora_leads_his_queue_only_never_a_friends_own_y() {
    let Some(mut h) = Harness::new() else {
        println!("no party AI pack: skipped");
        return;
    };
    lead(&mut h, PartyChara::Donald);
    lead(&mut h, PartyChara::Sora);
    h.rt.word("party don-zetta-ready"); // AI Donald's full Arcane Flow (his Y-capable state)
    plant(&mut h, &ALL_TEAM);
    let _ = route(&mut h, PadButtons::NONE, PadButtons::NONE);
    for (host, (serials, _, event)) in ["test level", "Elden Ring"].iter().zip(stacks(&h)) {
        assert_eq!(serials, vec![GRAND, FORM, TRINITY, BOMBARDIER, METEOR, FLARE], "{host}: Sora's whole queue");
        assert!(event, "{host}: Sora's finish on the event button");
        assert!(!serials.iter().any(|s| is_arcane(*s) || is_bombardier(*s)), "{host}: no friend's own Y row");
    }
    // his own Y reaches his core as ever
    let out = route(&mut h, PadButtons::NONE.with(PadButton::FaceTop), PadButtons::NONE);
    assert!(out.pad.pressed.has(PadButton::FaceTop));
}

#[test]
fn yhud_donald_leads_his_arcane_and_his_team_attacks_only() {
    let Some(mut h) = Harness::new() else {
        println!("no party AI pack: skipped");
        return;
    };
    lead(&mut h, PartyChara::Donald);
    // THE USER'S CASE: no Arcane charge yet and only Sora's own rows: the stack is empty (it was Sora's queue)
    plant(&mut h, &[BOMBARDIER]);
    let _ = route(&mut h, PadButtons::NONE, PadButtons::NONE);
    for (host, (serials, names, event)) in ["test level", "Elden Ring"].iter().zip(stacks(&h)) {
        assert!(serials.is_empty(), "{host}: nothing of Sora's (nor Goofy's Bombardier with Sora): {names:?}");
        assert!(!event, "{host}: no Sora finish on the event button");
    }
    // Y never runs Sora's rows (his finish was the event button's: Y ran it)
    let before = h.sora.combat().situation.current();
    let (forwarded, current) = press(&mut h, PadButton::FaceTop);
    assert!(!forwarded, "the player's Y does not reach Sora's queue");
    assert_eq!(current, before, "Sora's selection untouched");
    // his Y-capable state on and every team attack offered: his Arcane rows and 193 / 207 / 208
    h.rt.word("party don-zetta-ready");
    let don = h.rt.leader.expect("Donald leads");
    h.rt.party.status_mut(don).unwrap().mp = 300; // (the harness's Donald has no MP bar of his own: his Y spells' price)
    plant(&mut h, &ALL_TEAM);
    let _ = route(&mut h, PadButtons::NONE, PadButtons::NONE);
    for (host, (serials, names, event)) in ["test level", "Elden Ring"].iter().zip(stacks(&h)) {
        let team: Vec<u32> = serials.iter().copied().filter(|s| !is_arcane(*s)).collect();
        assert_eq!(team, vec![TRINITY, METEOR, FLARE], "{host}: his team attacks (with Sora, with Goofy): {names:?}");
        assert!(serials.iter().any(|s| is_arcane(*s)), "{host}: his Arcane rows stay: {names:?}");
        assert!(!serials.iter().any(|s| SORA_OWN.contains(s) || *s == BOMBARDIER), "{host}: none of Sora's own, not Goofy's Bombardier");
        assert!(names.iter().all(|n| !n.is_empty()), "{host}: every row named: {names:?}");
        assert!(!event, "{host}: no event button of Sora's");
    }
    // Y on his selected team row (the first: Trinity Guard) runs THAT row in Sora's queue (not the finish in slot B)
    let (forwarded, current) = press(&mut h, PadButton::FaceTop);
    assert!(forwarded, "his team attack's Y reaches Sora");
    assert_eq!(current, Some((Slot::List, 193)), "the chosen row is his team attack");
}

#[test]
fn yhud_goofy_leads_his_bombardier_and_his_team_attacks_only() {
    let Some(mut h) = Harness::new() else {
        println!("no party AI pack: skipped");
        return;
    };
    lead(&mut h, PartyChara::Goofy);
    h.rt.word("party don-zetta-ready"); // AI Donald's full Arcane Flow
    // only Donald's team attacks with Sora: not Goofy's -> his Bombardier alone
    plant(&mut h, &[METEOR, FLARE]);
    let _ = route(&mut h, PadButtons::NONE, PadButtons::NONE);
    for (host, (serials, names, event)) in ["test level", "Elden Ring"].iter().zip(stacks(&h)) {
        assert_eq!(serials.len(), 1, "{host}: {names:?}");
        assert!(is_bombardier(serials[0]), "{host}: his Bombardier: {names:?}");
        assert!(!event, "{host}");
    }
    let (forwarded, _) = press(&mut h, PadButton::FaceTop);
    assert!(!forwarded, "Y is his Bombardier, never Sora's queue");
    // his team attacks offered: 193 and 195 (Y and the slot are theirs), LT picks the second, Y runs it
    plant(&mut h, &ALL_TEAM);
    let _ = route(&mut h, PadButtons::NONE, PadButtons::NONE);
    for (host, (serials, names, event)) in ["test level", "Elden Ring"].iter().zip(stacks(&h)) {
        assert_eq!(serials, vec![TRINITY, BOMBARDIER], "{host}: his team attacks: {names:?}");
        assert!(!event, "{host}");
    }
    let _ = press(&mut h, PadButton::LeftTrigger);
    let (forwarded, current) = press(&mut h, PadButton::FaceTop);
    assert!(forwarded);
    assert_eq!(current, Some((Slot::List, 195)), "LT then Y: Goofy Bombardier with Sora");
}

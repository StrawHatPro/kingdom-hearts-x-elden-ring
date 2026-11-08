//! (GRABS) THE CAUGHT GATE OF THE PARTY'S AI-DRIVEN CHARACTERS: an Elden Ring throw holds a friend's body (kh-er
//! party/allybody_live.rs reads the body's throw node), so the friend is CAUGHT. While caught and while he gets up he does
//! not act: his tree is stopped (`AiController::set_stop_ai`, the same stop a team attack uses), his running action, a
//! spell included, is ended, and his pawn plays KH3's own caught rows through KH3's own NPC-motion request
//! (`RequestNpcMotionPlay` 0x141270750, situation 9: `CatchStart`, `CatchLoop` looped until the finish request, `CatchEnd`;
//! `begin_situation` ends the running action). At the release the finish is asked (0x141270970: the end motion next); when
//! the pawn leaves the motion he is free again and his tree STARTS AFRESH (its targets, its follow and its fights read anew).
//! AI Sora (a friend leading) is gated here too: his tree is stopped and his pad empty; kh-er plays his rows on his core.
//!
//! THE GUARDS, so a stale "caught" never holds anybody: a hold longer than [`HELD_MAX_S`] is given up (the motion cleared,
//! the gate deaf to that character until his body is seen free once); a get-up longer than [`UP_MAX_S`] is cleared; a
//! character who is gone, knocked out, or another member now (a load, a respawn, a warp rebuilds the party) is let go. The
//! user, 2026-11-06: "make sure AI doesn't break after they are released from the grab".

use kh_host::party::PartyChara;
use kh_party::{MemberId, NpcStateId};

use crate::runtime::PartyRuntime;
use crate::say;

/// KH3's caught rows, the same names in Sora's, Donald's and Goofy's AnimSets.
pub const CATCH_ROWS: [&str; 3] = ["CatchStart", "CatchLoop", "CatchEnd"];
/// Longer than any throw holds a body (kh-er's own cap is `grab.max`, 15 s).
pub const HELD_MAX_S: f32 = 20.0;
/// Longer than any get-up (`CatchEnd` 0.83 s, with its blends).
pub const UP_MAX_S: f32 = 3.0;
/// AI Sora's get-up (his core plays `CatchEnd` in kh-er): KH3 Sora's `CatchEnd` length.
pub const SORA_UP_S: f32 = 0.833_333_3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Held,
    /// Released; the finish request is asked until the pawn takes it.
    Releasing,
    GettingUp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub chara: PartyChara,
    /// The member he was when caught (None: AI Sora, or not a member).
    pub id: Option<MemberId>,
    pub phase: Phase,
    pub held_for: f32,
    pub up_for: f32,
    /// His pawn took the NPC-motion request (false: only his tree is stopped).
    pub motion: bool,
}

/// What a character is now, as the gate needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Now {
    pub id: Option<MemberId>,
    pub ko: bool,
    /// His pawn still plays the NPC motion (`SituationAction`).
    pub in_motion: bool,
}

/// What the runtime does for a character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Caught: the NPC motion with the caught rows (his action ended).
    Begin,
    /// Released: the finish request (the end motion next).
    Finish,
    /// Given up: the motion cleared at once.
    Clear,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CaughtGate {
    pub entries: Vec<Entry>,
    /// Characters whose hold was given up (the timeout): deaf until their body is seen free.
    pub deaf: Vec<PartyChara>,
}

impl CaughtGate {
    /// The host's word for one character this frame: his body held by a throw or not.
    pub fn note(&mut self, chara: PartyChara, held: bool, id: Option<MemberId>) -> Option<Order> {
        if !held {
            self.deaf.retain(|c| *c != chara);
        } else if self.deaf.contains(&chara) {
            return None;
        }
        let at = self.entries.iter().position(|e| e.chara == chara);
        match (at, held) {
            (None, true) => {
                self.entries.push(Entry { chara, id, phase: Phase::Held, held_for: 0.0, up_for: 0.0, motion: false });
                Some(Order::Begin)
            }
            (Some(i), true) if self.entries[i].phase != Phase::Held => {
                // caught again while getting up
                self.entries[i] = Entry { chara, id, phase: Phase::Held, held_for: 0.0, up_for: 0.0, motion: false };
                Some(Order::Begin)
            }
            (Some(i), false) if self.entries[i].phase == Phase::Held => {
                self.entries[i].phase = Phase::Releasing;
                Some(Order::Finish)
            }
            _ => None,
        }
    }

    /// The pawn took (or refused) the NPC motion of `Begin`.
    pub fn began(&mut self, chara: PartyChara, motion: bool) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.chara == chara) {
            e.motion = motion;
        }
    }

    /// The pawn took the finish request: the end motion plays.
    pub fn finished(&mut self, chara: PartyChara) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.chara == chara && e.phase == Phase::Releasing) {
            e.phase = Phase::GettingUp;
        }
    }

    /// Once a step: the guards and the get-ups. `now(c)` is the character as he is. Returns the orders to carry out.
    pub fn tick(&mut self, dt: f32, now: impl Fn(PartyChara) -> Now) -> Vec<(PartyChara, Order)> {
        let mut orders = Vec::new();
        let mut deaf = Vec::new();
        self.entries.retain_mut(|e| {
            let n = now(e.chara);
            // gone, another member now (a load, a respawn, the party rebuilt) or knocked out: let go, nothing to clear
            if n.ko || (e.chara != PartyChara::Sora && (n.id.is_none() || n.id != e.id)) {
                return false;
            }
            match e.phase {
                Phase::Held => {
                    e.held_for += dt;
                    if e.held_for > HELD_MAX_S {
                        if e.motion {
                            orders.push((e.chara, Order::Clear));
                        }
                        deaf.push(e.chara);
                        return false;
                    }
                    true
                }
                Phase::Releasing => {
                    e.up_for += dt;
                    if e.chara == PartyChara::Sora || !e.motion {
                        e.phase = Phase::GettingUp;
                    } else {
                        orders.push((e.chara, Order::Finish));
                    }
                    if e.up_for > UP_MAX_S {
                        orders.push((e.chara, Order::Clear));
                        return false;
                    }
                    true
                }
                Phase::GettingUp => {
                    e.up_for += dt;
                    let up = if e.chara == PartyChara::Sora { e.up_for >= SORA_UP_S } else { !e.motion || !n.in_motion };
                    if up {
                        return false;
                    }
                    if e.up_for > UP_MAX_S {
                        orders.push((e.chara, Order::Clear));
                        return false;
                    }
                    true
                }
            }
        });
        self.deaf.extend(deaf);
        orders
    }

    /// His tree is stopped and he takes no action: caught or getting up.
    pub fn stops(&self, chara: PartyChara) -> bool {
        self.entries.iter().any(|e| e.chara == chara)
    }
}

impl PartyRuntime {
    /// (GRABS) THE HOST'S WORD, once a frame for every AI-driven character: his Elden Ring body is held by a throw now.
    /// kh-er calls it for Donald, Goofy and AI Sora (a friend leading); the leader is the body code's (never gated here).
    pub fn set_caught(&mut self, chara: PartyChara, held: bool) {
        let id = if chara == PartyChara::Sora { None } else { self.id(chara) };
        if let Some(order) = self.caught.note(chara, held, id) {
            self.caught_order(chara, order);
        }
    }

    /// The character's tree is stopped by the gate (caught or getting up).
    pub fn ai_caught(&self, chara: PartyChara) -> bool {
        self.caught.stops(chara)
    }

    fn caught_order(&mut self, chara: PartyChara, order: Order) {
        let Some(id) = self.id(chara).filter(|_| chara != PartyChara::Sora) else {
            if order == Order::Begin {
                say!("party: (GRABS) {} CAUGHT by a throw: his tree stopped, no action until he is up (his core plays KH3's caught rows)", chara.name());
            }
            return;
        };
        match order {
            Order::Begin => {
                // a spell under way is cancelled (its own run holds his pose every step: `cast::interrupt`, not a hit; the MP
                // is spent, Zettaflare's circle ends with it), then the action and the command end with the request below
                let cancelled = self.party.member_mut(id).is_some_and(|m| kh_party::donald::cast::interrupt(m, false));
                if cancelled {
                    say!("party: (GRABS) {}'s spell was cancelled by the grab", chara.name());
                }
                let motion = self.party.request_npc_motion(id, -1, [Some(CATCH_ROWS[0]), Some(CATCH_ROWS[1]), Some(CATCH_ROWS[2])]);
                self.caught.began(chara, motion);
                say!("party: (GRABS) {} CAUGHT by a throw: his tree stopped, his action ended, {}", chara.name(), if motion { "KH3's caught motion (CatchStart, CatchLoop)" } else { "no motion (his state refuses it): only the tree held" });
            }
            Order::Finish => {
                if self.party.finish_npc_motion(id) {
                    self.caught.finished(chara);
                    say!("party: (GRABS) {} released: CatchEnd, then his tree starts afresh", chara.name());
                }
            }
            Order::Clear => {
                self.party.set_situation(id, 0, 0.0);
                say!("!!! party: (GRABS) {}'s caught state given up by the guard (a hold or get-up past its limit): his motion cleared, his tree free", chara.name());
            }
        }
    }

    /// (GRABS) Once a step, before the trees tick: the get-ups and the guards.
    pub(crate) fn caught_step(&mut self, dt: f32) {
        if self.caught.entries.is_empty() {
            return;
        }
        let views: Vec<(PartyChara, Option<MemberId>, bool, bool)> = self
            .caught
            .entries
            .iter()
            .map(|e| {
                let id = if e.chara == PartyChara::Sora { None } else { self.id(e.chara) };
                let v = id.and_then(|id| self.party.view(id));
                (e.chara, id, v.is_some_and(|v| v.ko), v.is_some_and(|v| v.state == NpcStateId::SituationAction))
            })
            .collect();
        let now = |c: PartyChara| views.iter().find(|v| v.0 == c).map_or(Now { id: None, ko: false, in_motion: false }, |v| Now { id: v.1, ko: v.2, in_motion: v.3 });
        let before: Vec<PartyChara> = self.caught.entries.iter().map(|e| e.chara).collect();
        let orders = self.caught.tick(dt, now);
        for (c, o) in orders {
            self.caught_order(c, o);
        }
        for c in before {
            if !self.caught.stops(c) {
                say!("party: (GRABS) {} is up again: his tree starts afresh", c.name());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    const D: MemberId = MemberId(1);

    fn free(id: Option<MemberId>) -> impl Fn(PartyChara) -> Now {
        move |_| Now { id, ko: false, in_motion: false }
    }

    fn moving(id: Option<MemberId>) -> impl Fn(PartyChara) -> Now {
        move |_| Now { id, ko: false, in_motion: true }
    }

    #[test]
    fn a_friend_is_held_then_gets_up_then_is_free() {
        let mut g = CaughtGate::default();
        assert_eq!(g.note(PartyChara::Donald, true, Some(D)), Some(Order::Begin));
        g.began(PartyChara::Donald, true);
        assert_eq!(g.note(PartyChara::Donald, true, Some(D)), None, "one begin");
        assert!(g.tick(DT, moving(Some(D))).is_empty());
        assert!(g.stops(PartyChara::Donald) && !g.stops(PartyChara::Goofy));
        // the release: the finish, asked until taken
        assert_eq!(g.note(PartyChara::Donald, false, Some(D)), Some(Order::Finish));
        assert_eq!(g.tick(DT, moving(Some(D))), vec![(PartyChara::Donald, Order::Finish)]);
        g.finished(PartyChara::Donald);
        assert!(g.tick(DT, moving(Some(D))).is_empty() && g.stops(PartyChara::Donald), "held still in CatchEnd");
        assert!(g.tick(DT, free(Some(D))).is_empty());
        assert!(!g.stops(PartyChara::Donald), "free the moment the motion ends");
    }

    #[test]
    fn ai_sora_gets_up_on_his_cores_catchend_time() {
        let mut g = CaughtGate::default();
        g.note(PartyChara::Sora, true, None);
        g.tick(DT, free(None));
        g.note(PartyChara::Sora, false, None);
        let mut t = 0.0;
        while g.stops(PartyChara::Sora) && t < 2.0 {
            g.tick(DT, free(None));
            t += DT;
        }
        assert!((t - SORA_UP_S).abs() < 0.05, "up after CatchEnd ({t:.2} s)");
    }

    #[test]
    fn the_timeout_guard_never_lets_a_stale_hold_persist() {
        let mut g = CaughtGate::default();
        g.note(PartyChara::Goofy, true, Some(D));
        g.began(PartyChara::Goofy, true);
        let mut cleared = false;
        for _ in 0..((HELD_MAX_S + 1.0) / DT) as usize {
            g.note(PartyChara::Goofy, true, Some(D));
            cleared |= g.tick(DT, moving(Some(D))).contains(&(PartyChara::Goofy, Order::Clear));
        }
        assert!(cleared && !g.stops(PartyChara::Goofy), "given up after {HELD_MAX_S} s");
        assert_eq!(g.note(PartyChara::Goofy, true, Some(D)), None, "deaf while the stale word goes on");
        g.note(PartyChara::Goofy, false, Some(D));
        assert_eq!(g.note(PartyChara::Goofy, true, Some(D)), Some(Order::Begin), "a real new grab after his body was free");
        // a get-up that never ends
        g.note(PartyChara::Goofy, false, Some(D));
        g.finished(PartyChara::Goofy);
        let mut n = 0;
        while g.stops(PartyChara::Goofy) && n < 400 {
            g.tick(DT, moving(Some(D)));
            n += 1;
        }
        assert!(!g.stops(PartyChara::Goofy) && (n as f32 * DT) <= UP_MAX_S + 0.1);
    }

    #[test]
    fn ko_a_rebuilt_party_or_a_missing_member_let_go_at_once() {
        let mut g = CaughtGate::default();
        g.note(PartyChara::Donald, true, Some(D));
        g.tick(DT, |_| Now { id: Some(D), ko: true, in_motion: true });
        assert!(!g.stops(PartyChara::Donald), "knocked out");
        g.note(PartyChara::Donald, false, Some(D));
        g.note(PartyChara::Donald, true, Some(D));
        g.tick(DT, moving(Some(MemberId(7))));
        assert!(!g.stops(PartyChara::Donald), "another member now (the party rebuilt after a warp / load)");
        g.note(PartyChara::Donald, false, Some(D));
        g.note(PartyChara::Donald, true, Some(D));
        g.tick(DT, free(None));
        assert!(!g.stops(PartyChara::Donald), "gone");
    }

    #[test]
    fn caught_again_while_getting_up_begins_anew() {
        let mut g = CaughtGate::default();
        g.note(PartyChara::Goofy, true, Some(D));
        g.began(PartyChara::Goofy, true);
        g.note(PartyChara::Goofy, false, Some(D));
        g.finished(PartyChara::Goofy);
        assert_eq!(g.note(PartyChara::Goofy, true, Some(D)), Some(Order::Begin));
        assert_eq!(g.entries[0].phase, Phase::Held);
    }

    /// The Sora copies (Mimic Sora on the player's side, the boss's Sora on the Mimic Tear's): on their real AIs (Goofy's
    /// tree n_ex002, the real pack) each acts, is caught 2 s (nothing pressed, no step, every step of it), is released, and
    /// acts again within 4 s after his CatchEnd.
    #[test]
    fn the_sora_copies_do_nothing_while_caught_and_act_again_after_the_release() {
        use crate::lead::{BossAi, BossScene, MimicAi, MimicScene};
        use crate::mimicboss::{BossTargets, Victim};
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party/ai");
        let Some(ai_pack) = kh_ai::AiPack::load_dir(&pack).ok().map(std::sync::Arc::new) else {
            eprintln!("no AI pack at build/sora_pack/party/ai: skipped");
            return;
        };
        let rt = PartyRuntime::new(crate::RuntimeConfig { wanted: true, test: crate::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: Some(ai_pack), ai_pack_source: None });
        let cam = kh_camera::CameraView { position: kh_core::Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: kh_core::Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let world = kh_core::test_world::TestWorld::floor();
        let pose = |x: f32| kh_core::Pose { position: kh_core::Vec3::new(x, 0.0, kh_core::Sora::origin_height_on_floor(&kh_core::SORA_PARAMS)), yaw: 0.0 };
        let player = kh_core::Sora::new(kh_core::SORA_PARAMS, pose(-300.0));
        let enemy = |chara: PartyChara| Victim { chara, feet: kh_core::Vec3::new(500.0, 0.0, 0.0), yaw: 180.0, velocity: kh_core::Vec3::ZERO, radius: 50.0, half_height: 90.0, hp: 1000, hp_max: 1000, defense: 0, airborne: false, down: false, tell: Default::default(), stance: Default::default() };
        let acted = |i: &kh_core::Input| i.pad.pressed != kh_core::PadButtons::NONE || i.pad.held != kh_core::PadButtons::NONE || i.move_stick.length() > 0.1 || i.jump_pressed;
        enum Copy {
            Mimic(Box<MimicAi>),
            Boss(Box<BossAi>),
        }
        for (name, mut ai, victim) in [("Mimic Sora", Copy::Mimic(Box::new(MimicAi::new(&rt))), PartyChara::Goofy), ("the boss's Sora", Copy::Boss(Box::new(BossAi::new(&rt))), PartyChara::Sora)] {
            let mut core = crate::mimic::make_core(&crate::mimic::copy_kit(&player, 0), pose(0.0));
            let mut null = kh_core::NullHost::default();
            // (phase, seconds): free, caught, released
            let mut log: Vec<(u8, usize, bool)> = Vec::new();
            for (phase, seconds) in [(0u8, 6.0f32), (1, 2.0), (2, 6.0)] {
                for k in 0..(seconds / DT) as usize {
                    let held = phase == 1;
                    let mut t = BossTargets { victims: vec![enemy(victim)], hits: Vec::new(), attack_rate: 1.0, orders: Vec::new() };
                    let now = log.len() as f64 * DT as f64;
                    let input = match &mut ai {
                        Copy::Mimic(a) => {
                            a.set_caught(held);
                            a.input(&rt, &mut core, MimicScene { sora: &player, lock_on: kh_core::TargetId::NONE, targets: &t, camera: &cam, camera_yaw: 0.0, nav: None, dt: DT, now })
                        }
                        Copy::Boss(a) => {
                            a.set_caught(held);
                            a.input(&rt, &mut core, BossScene { targets: &t, camera: &cam, camera_yaw: 0.0, nav: None, dt: DT, now })
                        }
                    };
                    log.push((phase, k, acted(&input)));
                    let mut host = kh_core::Host { world: &world, targets: &mut t, audio: &mut null.audio, effects: &mut null.effects, ui: &mut null.ui };
                    core.step_hosted(DT, &input, &mut host);
                }
            }
            assert!(log.iter().any(|e| e.0 == 0 && e.2), "{name} acts before the grab");
            assert!(log.iter().filter(|e| e.0 == 1).all(|e| !e.2), "{name} presses nothing while caught");
            let first = log.iter().filter(|e| e.0 == 2).find(|e| e.2).map(|e| e.1 as f32 * DT);
            assert!(first.is_some_and(|t| (SORA_UP_S - 0.05..=SORA_UP_S + 4.0).contains(&t)), "{name} acts again after his CatchEnd: {first:?}");
            let still_caught = match &ai {
                Copy::Mimic(a) => a.caught(),
                Copy::Boss(a) => a.caught(),
            };
            assert!(!still_caught, "{name}: the gate let go");
        }
    }

    /// The runtime's side on real pawns: the request ends the running action and enters KH3's NPC motion with the caught
    /// rows; the tree is stopped while caught.
    #[test]
    fn the_runtime_puts_a_caught_friend_into_kh3s_caught_motion_and_stops_his_tree() {
        let mut rt = PartyRuntime::new(crate::RuntimeConfig { wanted: true, test: crate::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        for c in [PartyChara::Donald, PartyChara::Goofy] {
            rt.spawn_one(c, kh_core::Vec3::ZERO, 0.0);
            let id = rt.id(c).expect("spawned");
            rt.set_caught(c, true);
            assert!(rt.ai_caught(c));
            assert_eq!(rt.party.view(id).map(|v| v.state), Some(NpcStateId::SituationAction), "{c:?} in the NPC motion");
            rt.set_caught(c, false);
            rt.caught_step(DT);
            assert!(rt.ai_caught(c), "{c:?} getting up");
            rt.party.set_situation(id, 0, 0.0); // the motion's end (the party's own step plays it out)
            rt.caught_step(DT);
            assert!(!rt.ai_caught(c), "{c:?} free again");
        }
    }
}

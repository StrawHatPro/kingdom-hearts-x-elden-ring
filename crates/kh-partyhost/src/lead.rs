//! (party P) THE LEADER AND AI SORA (contract ER.2; was kh-app party_play.rs, stream P; state note
//! build/sora_tmp/party/IMPL_P_STATE.md).
//!
//! The user's request (2026-10-15): "can donald and goofy be playable?" -> "yes they should control the same as Sora.
//! For Goofy, "Abilities" replaces "Magic". Use the same edits we made to Sora's movement controls for them", and the
//! save-point "Party Formation" (max 3 active, min 1, the leader marked). KH3 has neither: everything here is THE
//! PORT'S DESIGN on top of the members' real kits (kh-party, its `pad` module: the controls, the user's movement
//! options, the friend command menu).
//!
//! - THE LEADER is the character the pad controls (`kh_host::formation::Formation::leader`). It changes ONLY in the
//!   save point's Party Formation (the user removed quick switching, 2026-10-15: no pad shortcut, no camp-menu switch);
//!   test runs may use the words `leader sora|donald|goofy` / `formation in|out|lead WHO`.
//! - A friend leading: the pad's frame goes to `Party::set_pad_frame` (kh-party pad), the friend's AI tree is not
//!   ticked; Sora is run by "AI SORA" (below); the field camera, the lock-on camera and the HUD follow the leader.
//! - AI SORA (OUR DESIGN): Goofy's own behaviour tree (`n_ex002_BT`: battle / field / follow / attack flow) run by
//!   stream C's controller on a body that turns its commands into Sora's pad input: MoveTo = the stick toward the
//!   goal (camera-relative, walk below 0.8), an attack / special action = A presses at its target, Warp = Sora is put
//!   next to the leader (the friends' own warp rule), the rest answered at once.
//! - The other friends follow the LEADER: their trees see the leader as "the player" (`LeaderAiWorld`), the party's
//!   `SoraView` is the leader's.
//!
//! What the test level kept in process globals (the formation as applied, the leader's state, AI settings for the
//! save) is here as plain fields of [`Leadership`]; a host mirrors them where its other screens read them.

use kh_core::input::{PadButton, PadButtons};
use kh_core::{Input, Output, TargetId, Vec2, Vec3};
use kh_host::formation::Formation;
use kh_host::party::{FriendAiSettings, PartyChara};
use kh_party::pad::PadFrame;
use kh_party::MemberView;

use crate::aiworld::{EnemyFeed, LeaderAiWorld, PartyTargets, SORA_ACTOR, STRAIGHT_NAV};
use crate::runtime::{chara_of, PartyRuntime};
use crate::say;

/// The playable party's state (was kh-app `PartyPlay` + party_play.rs's statics).
pub struct Leadership {
    pub formation: Formation,
    /// The leader the simulation was last set to (the formation's leader is applied at the next step).
    pub applied: PartyChara,
    /// Sora's AI while a friend leads.
    sora_ai: Option<SoraAi>,
    /// The leader's pad lock that the camera was last told.
    camera_lock: TargetId,
    /// Sora is in the active party (else he is hidden and not stepped).
    pub sora_active: bool,
    /// A formation change waiting for the next step.
    dirty: bool,
    /// DEBUG: where the word `leader warp X Y` puts a friend leader.
    warp: Option<Vec3>,
    logged: String,
    /// The words `leader ...` / `formation ...`, taken at the next step.
    words: Vec<Vec<String>>,
    /// AI SORA'S SETTINGS (the camp menu's Customize entry "Sora" while a friend leads; saved in the party record):
    /// Combat Style, Abilities, Recovery, the target bits. The game's Restore Defaults (0x140E94550) as the friends'.
    pub sora_settings: FriendAiSettings,
    /// Sora is in the active party as the last formation applied it (the AI world lists him as an ally only then).
    pub sora_in: bool,
    /// The formation as the last step applied it (None = never applied: the default).
    pub current: Option<Formation>,
    /// Every character's AI settings as the last route had them (Sora's and the friends'), for the save.
    pub all_ai: Vec<(PartyChara, FriendAiSettings)>,
    /// The leader as the last step left him: which character, and whether he is down (KO). A friend leader who
    /// falls ends the game as Sora's death does (OUR DESIGN, the user's "the death screen should show the party
    /// leader in their death pose").
    pub leader_state: (PartyChara, bool),
    /// A friend leader's capsule (centre, radius, half height) as the last step left him (the save point tests it).
    pub leader_capsule: Option<(Vec3, f32, f32)>,
    /// (party AIX) The party's battle flag while a friend leads and its linger timer (`battle_step`).
    pub party_battle: bool,
    battle_linger: f32,
    /// (FX1) A "yes" the pad was given (D.6) and not yet seen through: the leader, Sora's unison count at the yes, the
    /// seconds since (`team_wait_step`).
    pub(crate) team_wait: Option<TeamWait>,
}

/// (FX1) The team move a friend leader's pad was told "yes" to, waiting for AI Sora's core to start it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TeamWait {
    member: kh_party::MemberId,
    unisons: u32,
    age: f32,
}

/// (FX1) The longest wait for an accepted team move (s): the latch's 0.5 s (FriendState::roll_leader_unison) and a
/// margin of a few steps. Past it the pad gets the late "no" whatever the latch says.
const TEAM_WAIT_MAX: f32 = 0.6;
/// (FX1) How long a late "no" lets the friend's own finisher start (s): long enough for Donald's Jump Strike to reach
/// its cancel point or Goofy's to land (OUR DESIGN).
const LATE_NO_TIME: f32 = 1.0;

impl Default for Leadership {
    fn default() -> Leadership {
        Leadership {
            formation: Formation::default(),
            applied: PartyChara::Sora,
            sora_ai: None,
            camera_lock: TargetId::NONE,
            sora_active: true,
            dirty: true,
            warp: None,
            logged: String::new(),
            words: Vec::new(),
            sora_settings: FriendAiSettings { battle: 0, ability: 0, cure: 1, cure_targets: 7 },
            sora_in: true,
            current: None,
            all_ai: Vec::new(),
            leader_state: (PartyChara::Sora, false),
            leader_capsule: None,
            party_battle: false,
            battle_linger: 0.0,
            team_wait: None,
        }
    }
}

impl Leadership {
    pub fn friend_leads(&self) -> bool {
        self.applied != PartyChara::Sora
    }

    /// (party AIX, OUR DESIGN: KH3 has no friend leader) THE PARTY'S BATTLE FLAG WHILE A FRIEND LEADS, one step: on while
    /// the host counts an engaged enemy (it measures round the leader), then KH3's linger (`stance.battle_linger` 1.0 s,
    /// 0x140EA0B20 `mov [rdi+5C0h], 1.0`) or the forced flag: Sora's own rule (kh-core actions.rs `pawn_tick`) on the
    /// leader's fight.
    pub fn battle_step(&mut self, engaged: u32, forced: bool, dt: f32) -> bool {
        // (engaged: the host's count + `PartyRuntime::party_aggro`)
        if engaged > 0 {
            self.battle_linger = kh_core::actions::ACTION_PARAMS.stance_battle_linger;
            self.party_battle = true;
        } else {
            self.battle_linger = (self.battle_linger - dt).max(0.0);
            self.party_battle = self.battle_linger > 0.0 || forced;
        }
        self.party_battle
    }

    /// The formation changed (the menu, a word, a load): applied at the next step.
    pub fn set_formation(&mut self, f: Formation) {
        if f != self.formation {
            say!("party-play: formation {:?}, leader {}", f.active().iter().map(|c| c.name()).collect::<Vec<_>>(), f.leader().name());
            self.formation = f;
            self.dirty = true;
        }
    }

    /// The formation as the last step applied it.
    pub fn current_formation(&self) -> Formation {
        self.current.clone().unwrap_or_default()
    }

    /// (party AIX) AI Sora's tree as it runs now and its log lines since the last call (turns the tree's log on): the
    /// scenario harness (kh-partyhost tests/aix.rs) and trace tools. None while Sora leads.
    pub fn sora_ai_trace(&mut self) -> Option<(kh_ai::AiTrace, Vec<(f64, String)>)> {
        let ctl = self.sora_ai.as_mut()?.ctl.as_mut()?;
        ctl.set_log(true);
        Some((ctl.trace(), ctl.drain_log()))
    }

    fn words(&mut self) {
        for w in std::mem::take(&mut self.words) {
            let w: Vec<&str> = w.iter().map(|s| s.as_str()).collect();
            let who = w.get(1).and_then(|x| PartyChara::from_name(x)).or_else(|| w.get(2).and_then(|x| PartyChara::from_name(x)));
            let mut f = self.formation.clone();
            // DEBUG (test runs): `leader warp X Y` puts a friend leader X metres forward, Y metres right of the origin
            if let (Some("leader"), Some("warp"), Some(x), Some(y)) = (w.first().copied(), w.get(1).copied(), w.get(2).and_then(|v| v.parse::<f32>().ok()), w.get(3).and_then(|v| v.parse::<f32>().ok())) {
                self.warp = Some(Vec3::new(x * 100.0, y * 100.0, 0.0));
                continue;
            }
            let r = match (w.first().copied(), w.get(1).copied(), who) {
                (Some("leader"), _, Some(c)) => {
                    if !f.is_active(c) {
                        let _ = f.toggle(c);
                    }
                    f.set_leader(c).map(|_| ())
                }
                (Some("formation"), Some("in" | "out" | "toggle"), Some(c)) => f.toggle(c).map(|_| ()),
                (Some("formation"), Some("lead"), Some(c)) => f.set_leader(c),
                (Some("formation"), Some("reset"), _) => {
                    f = Formation::default();
                    Ok(())
                }
                _ => {
                    say!("party-play: unknown word {:?} (leader sora|donald|goofy; formation in|out|lead WHO; formation reset)", w);
                    continue;
                }
            };
            match r {
                Ok(()) => self.set_formation(f),
                Err(e) => say!("party-play: {:?}: refused: {}", w, e.text()),
            }
        }
    }
}

/// (party AIX, OUR DESIGN) The stand-in for an enemy's sight of the party while a friend leads (`PartyRuntime::
/// party_aggro`), cm from a party member to the enemy's edge: Elden Ring's target.engage_range default.
pub const PARTY_AGGRO_RANGE: f32 = 800.0;

/// What the host hands the leader routing before Sora's step.
pub struct RouteIn<'a> {
    pub sora: &'a mut kh_core::Sora,
    /// The pad's (or the script's) input of this step.
    pub input: &'a Input,
    pub dt: f32,
    /// The enemies (AI Sora's tree).
    pub targets: &'a dyn PartyTargets,
    /// The drawn camera of the last step.
    pub camera: &'a kh_camera::CameraView,
    /// A formation the HUD's Party Formation screen or a load asked for since the last step.
    pub asked_formation: Option<Formation>,
    /// AI settings a load asked for (Sora's entry = AI Sora's).
    pub asked_ai: Vec<(PartyChara, FriendAiSettings)>,
    /// The host's navigation (None = the straight-line stand-in).
    pub nav: Option<&'a dyn kh_ai::Nav>,
    pub now: f64,
}

/// A change of the lock-on camera's target (the leader's RB lock): where the camera looks from and at.
#[derive(Clone, Copy, Debug)]
pub struct LockChange {
    pub target: kh_camera::CameraTarget,
    /// (target id, its lock-on point) or None = the lock is let go.
    pub at: Option<(u32, Vec3)>,
}

impl PartyRuntime {
    pub fn friend_leads(&self) -> bool {
        self.lead.friend_leads()
    }

    /// A word for the leader routing (`leader WHO`, `leader warp X Y`, `formation in|out|lead WHO`, `formation
    /// reset`), taken at the next step.
    pub fn lead_word(&mut self, words: &[&str]) {
        self.lead.words.push(words.iter().map(|w| w.to_string()).collect());
    }

    pub fn set_formation(&mut self, f: Formation) {
        self.lead.set_formation(f);
    }

    pub fn sora_ai_settings(&self) -> FriendAiSettings {
        self.lead.sora_settings
    }

    pub fn set_sora_ai_settings(&mut self, s: FriendAiSettings) {
        self.lead.sora_settings = s;
        say!("party-play: AI Sora settings: {}", s.describe());
    }

    /// (party P) THE MEMBERS THE AI RUNS NOW (Customize's dynamic friends list, the user's rule): Sora while a friend
    /// leads and he is in the party, every active friend but the leader; party order, with their settings.
    pub fn ai_members(&self) -> Vec<(PartyChara, FriendAiSettings)> {
        let leader = self.leader.and_then(|l| self.members.iter().find(|m| m.1 == l)).map(|m| m.0);
        let mut out = Vec::new();
        if leader.is_some() && self.lead.sora_in {
            out.push((PartyChara::Sora, self.lead.sora_settings));
        }
        for c in PartyChara::FRIENDS {
            if self.id(c).is_some() && leader != Some(c) {
                out.push((c, self.ai_settings(c)));
            }
        }
        out
    }

    /// The character who leads now (the HUD's main slot).
    pub fn leader_chara(&self) -> PartyChara {
        self.leader.and_then(|l| self.members.iter().find(|m| m.1 == l)).map_or(PartyChara::Sora, |m| m.0)
    }

    /// BEFORE SORA'S STEP: the formation and the leader applied, the pad's frame to a leading friend; returns Sora's
    /// input (the pad's when he leads, AI Sora's otherwise).
    pub fn route(&mut self, r: RouteIn) -> Input {
        let RouteIn { sora, input, dt, targets, camera, asked_formation, asked_ai, nav, now } = r;
        // (DON-KIT) Donald's Aero aura on Sora where he stands (every host's route: the test level's seam, kh-er's glue)
        let (evasion, recovery) = kh_party::pad::donald::sora_aura(&self.party, sora.output().position);
        sora.set_aura(evasion, recovery);
        self.lead.words();
        if let Some(f) = asked_formation {
            self.lead.set_formation(f);
        }
        for (c, s) in asked_ai {
            if c == PartyChara::Sora {
                self.set_sora_ai_settings(s);
            } else {
                self.set_ai_settings(c, s);
            }
        }
        self.lead.all_ai = std::iter::once((PartyChara::Sora, self.lead.sora_settings)).chain(PartyChara::FRIENDS.into_iter().map(|c| (c, self.ai_settings(c)))).collect();
        if self.lead.dirty {
            self.lead.dirty = false;
            self.apply_formation(sora, now);
            // (party P2) a friend leader asked for before the friends are spawned (a word or a load at the start): asked
            // again at the next step until he is there
            if self.lead.formation.leader() != PartyChara::Sora && self.leader.is_none() && self.wanted {
                self.lead.dirty = true;
            }
        }
        // (ER party S6) the leader's Zero EXP (progression.rs: the user's rule)
        self.sync_zero_exp(sora);
        // (stream D, D.6) the team attacks with a friend leading: the leader for the link points, AI Sora rolls no unison
        self.team.set_leader(self.leader);
        sora.combat_mut().friend.sora_is_ai = self.leader.is_some();
        // (FX1) the safety net: a "yes" whose team move never started becomes a late "no"
        self.team_wait_step(sora, dt, now);
        let Some(leader) = self.leader else {
            // (party K1) the player's own Sora: his own speeds; (DON-KIT) x Donald's Thunder aura (4.2 "Party movement +10 %")
            sora.set_speed_scale(kh_party::pad::donald::sora_move_mult(&self.party, sora.output().position));
            return *input;
        };
        // (D.6) the leader's combo wants its finisher: the unison / fusion roll (Sora in the party only)
        let leader_at = self.party.view(leader).map(|v| (v.position, v.chara.uid()));
        let sora_at = sora.output().position;
        let abilities = sora.status().abilities.clone();
        let sora_active = self.lead.sora_active;
        // (FX1) AI Sora's shotlock sequence (RB held, A at 1 s) is his own action in progress
        let ai_shooting = self.lead.sora_ai.as_ref().is_some_and(|ai| ai.shot.is_some());
        if let (Some(m), Some((pos, uid))) = (self.party.member_mut(leader), leader_at) {
            if let Some(pad) = m.pad.as_mut().filter(|p| p.finisher_pending) {
                pad.finisher_pending = false;
                // (FX1) THE ROLL ONLY WHEN AI SORA CAN START THE MOVE (kh-core `Sora::can_start_leader_unison`: grounded
                // Idle / Run, not mid-action or style change, nothing queued, not KO'd; the roll keeps its own distance
                // rule). Otherwise "no" at once: the friend plays his own finisher (X3D Gate::Solo, X3G's Jump Strike).
                let ready = sora_active && !ai_shooting && sora.can_start_leader_unison(uid);
                let roll = ready && sora.combat_mut().friend.roll_leader_unison(&abilities, uid, pos, sora_at);
                pad.finisher_answer = Some(roll);
                if roll {
                    self.lead.team_wait = Some(TeamWait { member: leader, unisons: sora.combat().friend.unisons, age: 0.0 });
                    say!("party-play: t={now:.3}s the leader's combo ends in a UNISON / FUSION with Sora (stream D plays it)");
                } else if sora_active && !ready {
                    say!("party-play: t={now:.3}s the leader's combo plays his own finisher: AI Sora cannot start a team move now ({:?}, {:.0} cm away)", sora.output().state, (pos - sora_at).length());
                }
            }
        }
        if let (Some(at), Some(m)) = (self.lead.warp.take(), self.party.member_mut(leader)) {
            m.cmc.location = Vec3::new(at.x, at.y, m.cmc.location.z);
            m.cmc.stop_movement();
            say!("party-play: t={now:.3}s DEBUG leader warp to ({:.0}, {:.0})", at.x, at.y);
        }
        // (party X2) Sora's magic and the leaders' Customize blocks to the party before the pad's decisions (custom.rs)
        self.don_affinity_from(targets); // (DON-UI) Elemental Insight: the lock-on target's elements (custom.rs)
        self.feed_leader_magic(sora);
        // (party X3D) Sora's side of Donald's kit (Glide, Aerial Recovery, his magic tiers and Unisons, AI Sora in the
        // party, his situation queue) and, in the test level, the whole kit equipped for the playtest
        {
            use kh_core::enums::ETresAbilityKind as Ab;
            use kh_core::magic::spells::Element as E;
            let st = sora.status();
            let tier = |e: E| st.magic_tier(e).min(3) as u8;
            let learned = kh_party::pad::donald::SoraLearned {
                known: true,
                glide: st.abilities.count(Ab::Glide) > 0,
                air_recovery: st.abilities.count(Ab::AirRecovery) > 0,
                magic: [tier(E::Fire), tier(E::Blizzard), tier(E::Thunder), tier(E::Water), tier(E::Aero), tier(E::Cure)],
                unison: [st.abilities.owned(Ab::UnisonFire), st.abilities.owned(Ab::UnisonBlizzard), st.abilities.owned(Ab::UnisonThunder)],
                sora_in_party: sora_active,
                situation_busy: sora_active && sora.combat().situation.count() > 0,
            };
            kh_party::pad::donald::feed(&mut self.party, leader, learned, input.view);
        }
        // the leader's pad: the stick, the camera yaw and every button (B = jump)
        self.party.set_pad_frame(leader, PadFrame { stick: input.move_stick, camera_yaw: input.camera_yaw, pad: input.pad });
        // AI Sora (when he is in the party)
        if !self.lead.sora_active {
            return Input { move_stick: Vec2::ZERO, jump_pressed: false, jump_held: false, pad: kh_core::PadState::default(), ..*input };
        }
        // A TEAM ATTACK RUNNING (Sora's FR state, started by the leader's Y): the whole pad is the player's (Trinity Guard's
        // own menu A / B / Y, the stick of its link pawn) - the user's rule: only the controlled leader drives team attacks
        if format!("{:?}", sora.output().state).starts_with("Fr") {
            return *input;
        }
        // (YHUD) THE PLAYER'S Y REACHES SORA'S QUEUE ONLY FOR THE LEADER'S OWN TEAM ROW (kh_party::yhud): the team attack
        // with him that his Y stack has selected, chosen in Sora's queue (`Queue::choose_serial`) so the press runs that
        // row, never Sora's Grand Magic, formchange, finish (the event button) or a team attack without the leader
        let player_y = input.pad.pressed.has(PadButton::FaceTop) || input.pad.held.has(PadButton::FaceTop);
        let y_taken = self.party.pad_state(leader).is_some_and(|p| p.x2.y_taken); // (party X2) a Y the leader spent on his own move
        let y_team = match (player_y && !y_taken).then(|| self.leader_y_forward(sora)).flatten() {
            Some(serial) => sora.combat_mut().situation.choose_serial(serial),
            None => false,
        };
        let mut ai = match self.lead.sora_ai.take() {
            Some(ai) => ai,
            None => SoraAi::new(self),
        };
        // (FX5) AI Sora's tree sees the same threats as the friends' (aiworld.rs `threatens`)
        let enemies = {
            let lead_pos = self.current.iter().find(|v| Some(v.id) == self.leader).map_or(sora.output().position, |v| v.position);
            let lock = self.leader_lock().unwrap_or(TargetId::NONE);
            let mut feed = EnemyFeed::of_party(targets, &self.threat_points(lead_pos, &sora.output()), lock);
            feed.reserve(&self.party.don_zetta_off_limit_targets(targets)); // (ZETTA round 5c) Zettaflare's target and every enemy near its zones
            feed
        };
        let mut out = ai.input(self, sora, dt, &enemies, camera, input.camera_yaw, nav.unwrap_or(&STRAIGHT_NAV), now);
        // (DON-PROG fix of team/tests s7) THE PLAYER'S Y ON A TEAM COMMAND WINS OVER AI SORA'S OWN PRESSES: while the leader
        // presses or holds Y and a team attack is Sora's current situation command, AI Sora presses nothing of his own (an
        // attack, a spell, a shotlock queued in Sora's core makes the executor refuse the team command: `try_team_attack`
        // needs nothing queued), so the held Y (kh-er YHold, 0.5 s) is taken as soon as his queue drains. He still moves.
        // (YHUD) the team command is the leader's selected row, chosen above
        if y_team {
            out.pad = kh_core::PadState::default();
        }
        // (party K1) AI SORA'S CATCH-UP: the friends' FollowPlayer speed rule (kh-party locomotion, KH3 0x1412196C0) for
        // him, as a scale on his run / sprint speeds: a friend leader sprints at 980 as he does, so a gap never closed
        let scale = self.ai_sora_speed_scale(leader, &sora.output(), sora.actions().battle);
        sora.set_speed_scale(scale * kh_party::pad::donald::sora_move_mult(&self.party, sora.output().position)); // (DON-KIT) x the Thunder aura
        self.lead.sora_ai = Some(ai);
        // Y and LT (the situation commands: the team attacks are Sora's FR states in D's queue) reach Sora from the pad:
        // THE LEADER starts a team attack, never AI Sora (the user's rule, 2026-10-15); AI Sora's own Y (a formchange,
        // a finish) is kept when the player presses nothing
        // (YHUD) only the leader's own team row (y_team): the player's Y never runs Sora's own situation commands, and his
        // LT is the leader's menu's (it cycles the LEADER's Y rows: pad/menu.rs), never Sora's queue
        if input.pad.pressed.has(PadButton::FaceTop) {
            if y_team {
                say!("party-play: t={now:.3}s the leader's Y -> his team attack in Sora's queue {:?} (queue {})", sora.combat().situation.current(), sora.combat().situation.count());
            } else if !y_taken {
                say!("party-play: t={now:.3}s the leader's Y: no Y command of his on offer (Sora's own rows are not his; queue {})", sora.combat().situation.count());
            }
        }
        let b = PadButton::FaceTop;
        if y_team {
            out.pad.pressed.set(b, input.pad.pressed.has(b));
            out.pad.held.set(b, input.pad.held.has(b));
        }
        Input { view: input.view, camera_yaw: input.camera_yaw, ..out }
    }

    /// (FX1) THE SAFETY NET for a "yes" (D.6): Sora's core started the team move (his unison count went up: `play_unison`)
    /// -> done; his latch (`FriendState::leader_unison`, 0.5 s) still runs -> wait; else (the latch ran out and
    /// kh-core cleared the request without a word, or something took the latch without playing it, or he was not
    /// stepped for `TEAM_WAIT_MAX`) -> the request is cleared and the pad gets a LATE "NO" (`PadState::finisher_late_no`):
    /// the friend plays his own finisher as soon as he can (pad/donald.rs, pad/goofy.rs).
    fn team_wait_step(&mut self, sora: &mut kh_core::Sora, dt: f32, now: f64) {
        let Some(mut w) = self.lead.team_wait.take() else { return };
        w.age += dt;
        let f = &mut sora.combat_mut().friend;
        if f.unisons > w.unisons {
            say!("party-play: t={now:.3}s AI Sora started the team move {:.2} s after the yes", w.age);
            return;
        }
        if f.leader_unison > 0.0 && w.age < TEAM_WAIT_MAX {
            self.lead.team_wait = Some(w);
            return;
        }
        (f.leader_unison, f.unison) = (0.0, 0);
        let state = sora.output().state;
        if let Some(pad) = self.party.member_mut(w.member).and_then(|m| m.pad.as_mut()) {
            (pad.finisher_late_no, pad.finisher_answer, pad.finisher_pending) = (LATE_NO_TIME, None, false);
            say!("party-play: t={now:.3}s the team move never started (AI Sora {state:?}, {:.2} s): a late \"no\", the leader plays his own finisher", w.age);
        }
    }

    /// The formation into the simulation: friends spawned / removed, the pad on the leader, Sora active or parked.
    fn apply_formation(&mut self, sora: &mut kh_core::Sora, now: f64) {
        let f = self.lead.formation.clone();
        // the friends of the formation (the runtime's spawn takes `active_friends`)
        for (i, c) in PartyChara::FRIENDS.into_iter().enumerate() {
            self.active_friends[i] = f.is_active(c);
        }
        let leader_pos = self.leader.and_then(|l| self.party.view(l)).map(|v| (v.position, v.yaw)).unwrap_or((sora.output().position, sora.output().yaw));
        for c in PartyChara::FRIENDS {
            let want = f.is_active(c);
            let have = self.id(c).is_some();
            if want && !have && !self.members.is_empty() {
                self.spawn_one(c, leader_pos.0, leader_pos.1);
                say!("party-play: t={now:.3}s {} joins the party", c.name());
            } else if !want && have {
                self.remove_one(c);
                say!("party-play: t={now:.3}s {} leaves the party", c.name());
            }
        }
        // Sora in / out of the party: out = hidden, not stepped; in again = next to the leader
        let sora_in = f.is_active(PartyChara::Sora);
        if sora_in && !self.lead.sora_active {
            if let Some((p, yaw)) = self.leader.and_then(|l| self.party.view(l)).map(|v| (v.position, v.yaw)) {
                let a = (yaw + 110.0).to_radians();
                sora.teleport(kh_core::Pose { position: p + Vec3::new(a.cos(), a.sin(), 0.0) * 180.0 + Vec3::Z * 5.0, yaw });
            }
            say!("party-play: t={now:.3}s Sora joins the party");
        } else if !sora_in && self.lead.sora_active {
            say!("party-play: t={now:.3}s Sora leaves the party (hidden, not stepped)");
        }
        self.lead.sora_active = sora_in;
        self.lead.current = Some(f.clone());
        self.lead.sora_in = sora_in;
        // the leader
        let leader = f.leader();
        let tuning = sora.tuning();
        let new_leader = chara_of(leader).and_then(|ch| self.party.find(ch));
        if self.leader != new_leader {
            let sora_role_changes = self.leader.is_none() || new_leader.is_none(); // (party AIX)
            if let Some(old) = self.leader {
                self.party.set_pad(old, None);
                if let Some(ai) = self.ai_of(old) {
                    ai.reset();
                }
            }
            if let Some(new) = new_leader {
                // (party AIX) HIS TREE STOPS HERE (his AI is not ticked while he leads), so its running task is aborted
                // as a stopped behaviour tree aborts it (UE StopTree -> the active task's AbortTask; every kh-ai task's
                // abort ends in Stop: Action 0x1404A2370, MoveTo 0x1404A8700): without it the AI's last command ran on
                // under the pad (Goofy's Turbo dash carried the new leader 50 m away with the stick released). A state
                // that refuses commands (a damage reaction, a KO) keeps its course.
                self.party.command(new, kh_party::MemberCommand::Stop);
                if let Some(ai) = self.ai_of(new) {
                    ai.reset();
                }
                self.party.set_pad(new, Some(tuning));
            }
            self.leader = new_leader;
            // (party AIX) AI Sora begins afresh only when HIS role changes (he leads / he is AI again): a hand-over from
            // one friend leader to the other kept dropping his tree mid-action (it re-planned at once while he still cast)
            if sora_role_changes {
                self.lead.sora_ai = None;
            }
            say!("party-play: t={now:.3}s {} is the leader (the pad, the camera and the HUD follow)", leader.name());
        }
        self.lead.applied = if new_leader.is_some() { leader } else { PartyChara::Sora };
        self.lead.camera_lock = TargetId::NONE;
    }

    /// The leader's field-camera target (a friend leading), as Sora's `camera_target` builds his.
    pub fn camera_target(&self, dt: f32) -> Option<kh_camera::CameraTarget> {
        let id = self.leader?;
        let now = self.current.iter().find(|v| v.id == id)?;
        let before = self.previous.iter().find(|v| v.id == id).unwrap_or(now);
        let moving = matches!(now.state, kh_party::NpcStateId::Pad(kh_party::pad::RUN));
        Some(kh_camera::CameraTarget {
            position: now.position,
            yaw: now.yaw,
            velocity: if dt > 0.0 { (now.position - before.position) / dt } else { Vec3::ZERO },
            state: if moving {
                kh_camera::TargetState::Run
            } else if now.state == kh_party::NpcStateId::Idle {
                kh_camera::TargetState::Idle
            } else {
                kh_camera::TargetState::Other
            },
            collision_half_height: now.capsule.1.max(40.0),
        })
    }

    /// The leader's lock-on target (hard lock).
    pub fn leader_lock(&self) -> Option<TargetId> {
        self.leader.and_then(|l| self.party.pad_state(l)).map(|p| p.lock).filter(|t| *t != TargetId::NONE)
    }

    /// A friend leader's capsule (centre, radius, half height) as the last step left him.
    pub fn leader_capsule(&self) -> Option<(Vec3, f32, f32)> {
        self.lead.leader_capsule
    }

    /// AFTER THE PARTY'S STEP: the leader's state for the host's screens; a change of the leader's RB lock for the
    /// lock-on camera (the host applies it: `CameraRig::set_lockon(&change.target, change.at)`).
    pub fn after_step(&mut self, targets: &dyn PartyTargets, dt: f32, now: f64) -> Option<LockChange> {
        let Some(id) = self.leader else {
            self.lead.leader_state = (PartyChara::Sora, false);
            self.lead.leader_capsule = None;
            return None;
        };
        self.lead.leader_capsule = self.current.iter().find(|v| v.id == id).map(|v| (v.position + Vec3::Z * v.capsule.1.max(1.0), v.capsule.0, v.capsule.1));
        if let Some((c, v)) = self.members.iter().find(|m| m.1 == id).and_then(|m| self.current.iter().find(|v| v.id == id).map(|v| (m.0, v))) {
            self.lead.leader_state = (c, v.ko);
        }
        // the leader's state and clip, printed when they change (the run's log)
        if let Some(v) = self.current.iter().find(|v| v.id == id) {
            let line = format!("{:?} {}", v.state, v.anim.clip);
            if line != self.lead.logged {
                say!("party-play: t={now:.3}s {} {line} at ({:.0}, {:.0}, {:.0}) {:.0} cm/s", v.chara.name(), v.position.x, v.position.y, v.position.z, Vec2::new(v.velocity.x, v.velocity.y).length());
                self.lead.logged = line;
            }
        }
        let lock = self.leader_lock().unwrap_or(TargetId::NONE);
        if lock != self.lead.camera_lock {
            self.lead.camera_lock = lock;
            if let Some(target) = self.camera_target(dt) {
                let feed = EnemyFeed::of(targets);
                let at = feed.centre(lock).map(|p| (lock.0, p));
                return Some(LockChange { target, at });
            }
        }
        None
    }

    /// The HUD's snapshot while a friend leads: his command menu (Attack / Magic or Abilities / Items), his lock and soft
    /// target; the situation commands stay Sora's (the team attacks run through him).
    pub fn hud_snapshot(&self, base: &kh_core::UiSnapshot) -> Option<kh_core::UiSnapshot> {
        let id = self.leader?;
        let menu = self.party.pad_menu(id)?;
        let pad = self.party.pad_state(id)?;
        let mut s = base.clone();
        s.command = menu;
        s.lock_on = pad.lock;
        s.soft_target = if pad.lock == TargetId::NONE { pad.soft } else { TargetId::NONE };
        s.prompt = None;
        // (party X2-b) his own aim (Discus Barrage / Spell Volley) as Sora's shotlock scope and lock marks
        s.aim = crate::custom::leader_aim(&self.party, id).unwrap_or_default();
        s.sneeze = false;
        Some(s)
    }

    /// (party K1) AI Sora's speed scale behind the leading friend: the FollowPlayer rule's follow speed (the leader's
    /// MaxWalkSpeed, at least his actual speed, x the in-view rate beyond m_RangeSamePlayerSpeed, x distance / range
    /// within it) over his sprint speed, at least 1; 1 in battle. Port design (KH3 has no AI Sora; the friends' rule).
    pub fn ai_sora_speed_scale(&self, leader: kh_party::MemberId, out: &kh_core::Output, in_battle: bool) -> f32 {
        let k = kh_party::locomotion::K;
        let (Some(m), false) = (self.party.member(leader), in_battle) else { return 1.0 };
        let d = m.position() - out.position;
        let dist = (d.x * d.x + d.y * d.y).sqrt();
        let range = k.follow_same_speed_range;
        let rate = if dist <= range { dist / range } else { k.follow_in_view_rate };
        let v = m.cmc.velocity;
        let reference = m.cmc.max_walk_speed.max(m.params.speed_run).max((v.x * v.x + v.y * v.y).sqrt());
        (reference * rate).min(k.k1_speed_cap) / kh_core::SORA_PARAMS.speed_sprint.max(1.0)
    }

    /// The party's view of "the player" while a friend leads: the leader's (the friends follow, warp to and cure him).
    pub fn leader_view(&self, mut view: kh_party::SoraView) -> kh_party::SoraView {
        if let Some(l) = self.leader.and_then(|l| self.current.iter().find(|v| v.id == l)) {
            view.position = l.position;
            view.yaw = l.yaw;
            view.velocity = l.velocity;
            view.on_ground = l.on_ground;
            view.hp = l.hp;
            view.hp_max = l.hp_max;
            view.mp = l.mp;
            view.mp_max = l.mp_max;
            view.dead = l.ko;
            view.lock_on = self.party.pad_state(l.id).map_or(TargetId::NONE, |p| p.lock);
            // (party K1) the leading friend's MaxWalkSpeed (his pad run: n_ex00x_PawnBase MaxWalkSpeed, no sprint)
            view.max_speed = self.party.member(l.id).map_or(0.0, |m| m.cmc.max_walk_speed.max(m.params.speed_run));
            view.analog = self.party.member(l.id).map_or(1.0, |m| if m.cmc.walk_mode { kh_core::SORA_PARAMS.speed_walk_rate } else { 1.0 });
        }
        view
    }
}

// ---------------------------------------------------------------------------------------------- AI Sora

/// The command AI Sora's tree is running, and how far it is.
struct SoraCmd {
    ticket: u32,
    cmd: kh_ai::AiCommand,
    age: f32,
    attacked: bool,
    /// (party AIX) the command's age at its first A press
    pressed_at: f32,
}

/// AI SORA's body: what his tree sees of him and what it asked for.
struct SoraBody<'a> {
    out: &'a Output,
    status: &'a kh_core::Status,
    in_battle: bool,
    cmd: &'a mut Option<SoraCmd>,
    history: &'a mut Vec<(u32, kh_ai::AiCommandStatus)>,
    next: &'a mut u32,
}

pub(crate) fn sora_state_name(s: kh_core::StateId) -> &'static str {
    use kh_core::StateId as S;
    match s {
        S::Idle => "TSID_STATE_IDLE",
        S::Run => "TSID_STATE_AI_MOVE",
        S::Damage => "TSID_STATE_DAMAGE",
        S::Die => "TSID_STATE_DIE",
        // (party AIX) a jump / fall is a jump to the trees (the warp rule lets JUMP warp, FUN_141258790), not an action
        S::Jump => "TSID_STATE_JUMP",
        S::Attack => "TSID_STATE_AI_ACTION",
        _ => "TSID_STATE_AI_ACTION",
    }
}

/// (party AIX) Sora is free for the tree's next command: standing or running (every other state is one of his own
/// actions, a reaction or a jump, which the friends' Action task waits out: 0x1404A2370 waits for the end message).
fn sora_free(s: kh_core::StateId) -> bool {
    matches!(s, kh_core::StateId::Idle | kh_core::StateId::Run)
}

impl kh_ai::AiBody for SoraBody<'_> {
    fn view(&self) -> kh_ai::BodyView {
        kh_ai::BodyView {
            member: SORA_ACTOR,
            uid: 0,
            position: self.out.position,
            yaw: self.out.yaw,
            velocity: self.out.velocity,
            on_ground: self.out.on_floor,
            radius: 34.0,
            half_height: 89.0,
            state: sora_state_name(self.out.state),
            hp: self.status.hp,
            hp_max: self.status.max_hp(),
            mp: self.status.mp,
            mp_max: self.status.max_mp(),
            mp_charging: self.status.mp_charging,
            ko: self.status.hp < 1,
            in_battle: self.in_battle,
            can_battle: self.in_battle,
            in_damage_reaction: self.out.state == kh_core::StateId::Damage,
            items: [None; 4],
            ..kh_ai::BodyView::default()
        }
    }
    fn function(&self, name: &str) -> Option<f32> {
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        Some(match name {
            "CanBattle" => b(self.in_battle),
            "IsActionOff" => 1.0,
            "IsSwimming" | "IsRideOn" | "IsUnderWater" => 0.0,
            "GetMagicPoint" | "GetMP" => self.status.mp as f32,
            "IsEnableWarp" => 1.0,
            _ => return None,
        })
    }
    fn command(&mut self, cmd: kh_ai::AiCommand) -> u32 {
        let t = *self.next;
        *self.next += 1;
        if let Some(old) = self.cmd.take() {
            self.history.push((old.ticket, kh_ai::AiCommandStatus::Aborted));
        }
        match cmd {
            // instant ones ((party AIX) TurnTo is not: it turns him, `SoraAi::input`)
            kh_ai::AiCommand::Focus(_) | kh_ai::AiCommand::SetBattleFollowMode(_) | kh_ai::AiCommand::Stop => {
                self.history.push((t, kh_ai::AiCommandStatus::Succeeded));
            }
            // Sora has none of Donald's cures or the friends' items in this design
            kh_ai::AiCommand::UseItem { .. } | kh_ai::AiCommand::Cure { .. } => {
                self.history.push((t, kh_ai::AiCommandStatus::Failed));
            }
            cmd => *self.cmd = Some(SoraCmd { ticket: t, cmd, age: 0.0, attacked: false, pressed_at: 0.0 }),
        }
        if self.history.len() > 64 {
            self.history.drain(..32);
        }
        t
    }
    fn command_status(&self, ticket: u32) -> kh_ai::AiCommandStatus {
        if self.cmd.as_ref().is_some_and(|c| c.ticket == ticket) {
            return kh_ai::AiCommandStatus::Running;
        }
        self.history.iter().rev().find(|h| h.0 == ticket).map_or(kh_ai::AiCommandStatus::Unknown, |h| h.1)
    }
}

/// AI Sora: stream C's controller on Goofy's tree, and the input it makes.
struct SoraAi {
    ctl: Option<kh_ai::AiController>,
    shared: kh_ai::AiShared,
    cmd: Option<SoraCmd>,
    history: Vec<(u32, kh_ai::AiCommandStatus)>,
    next: u32,
    held: PadButtons,
    press_wait: f32,
    far_time: f32,
    /// The tools' timers, the shotlock in progress (seconds into it), the settings given to the tree, the RNG.
    cure_wait: f32,
    magic_wait: f32,
    shot_wait: f32,
    situation_wait: f32,
    shot: Option<f32>,
    applied: Option<FriendAiSettings>,
    random: u32,
    /// (party AIX) seconds since one of HIS OWN TOOLS (a spell, a Cure / Potion, the shotlock, a situation command) started
    /// an action of his: the tree waits it out (`input`).
    tool_busy: Option<f32>,
    /// (party K1) the hold after an arrival behind a running leader (s left), as the friends' (kh-party locomotion).
    k1_hold: f32,
}

impl SoraAi {
    fn new(rt: &PartyRuntime) -> SoraAi {
        let ctl = rt.ai_pack.clone().map(|pack| kh_ai::AiController::new(pack, "n_ex002", SORA_ACTOR, 0x50_7A_A1));
        if ctl.is_none() {
            say!("party-play: no AI pack: AI Sora only follows the leader");
        }
        SoraAi { ctl, shared: kh_ai::AiShared::new(), cmd: None, history: Vec::new(), next: 1, held: PadButtons::NONE, press_wait: 0.0, far_time: 0.0, cure_wait: 0.0, magic_wait: 3.0, shot_wait: 6.0, situation_wait: 0.0, shot: None, applied: None, random: 0x50_7A_A1, tool_busy: None, k1_hold: 0.0 }
    }

    #[allow(clippy::too_many_arguments)]
    fn input(&mut self, rt: &PartyRuntime, sora: &mut kh_core::Sora, dt: f32, enemies: &EnemyFeed, camera: &kh_camera::CameraView, camera_yaw: f32, nav: &dyn kh_ai::Nav, now: f64) -> Input {
        let out = sora.output();
        let status = sora.status().clone();
        let Some(leader) = rt.leader.and_then(|l| rt.current.iter().find(|v| v.id == l)) else { return Input::default() };
        // (ZETTA round 5c) no enemy left to him through Zettaflare: he waits at the nearest spot off its zones
        let waiting = if rt.zetta_wait_spots(leader.position, enemies, nav).is_empty() { None } else { kh_party::donald::zettaflare::wait_place(&rt.party, None, out.position) };
        let in_battle = sora.actions().battle;
        let mps = rt.member_mps();
        let world = LeaderAiWorld {
            leader,
            lock_on: rt.party.pad_state(leader.id).map_or(TargetId::NONE, |p| p.lock),
            in_battle: in_battle || leader.in_battle,
            members: &rt.current,
            sora: Some((&out, &status)),
            enemies,
            camera: rt.ai_camera(camera), // (party I2) the drawn view
            nav,
            game_level: status.game_level as u8,
            mps: &mps, // (party I2)
        };
        // (party AIX, OUR DESIGN) WHILE ONE OF HIS OWN TOOLS RUNS (a spell, the shotlock, a situation command: actions the
        // tree did not ask for) the tree waits, as a friend's tree waits on its Action task: ticked meanwhile, it saw him
        // "in an action" with no task of its own, its lock-on query filtered him out (IsCharacterState AI_ACTION) and the
        // battle tree failed and restarted every few steps for the whole cast
        if let Some(t) = self.tool_busy.as_mut() {
            *t += dt;
            if (*t > 0.1 && sora_free(out.state) && self.shot.is_none()) || *t > 6.0 || matches!(out.state, kh_core::StateId::Damage | kh_core::StateId::Die) {
                self.tool_busy = None;
            }
        }
        if let (Some(ctl), None) = (self.ctl.as_mut(), self.tool_busy) {
            let mut body = SoraBody { out: &out, status: &status, in_battle: in_battle || leader.in_battle, cmd: &mut self.cmd, history: &mut self.history, next: &mut self.next };
            ctl.tick(dt, &mut body, &world, &mut self.shared);
        }
        // Warp (the friends' rule, kh-ai warp decision): far from the leader for a while -> next to him
        let far = out.position.distance(leader.position);
        // (party AIX) only while the camera does not see him (the friends' warp rule, FUN_141258790's frustum test): the
        // put-behind is never seen
        let seen = kh_ai::controller::in_view(&world.camera, out.capsule_center, 90.0);
        self.far_time = if far > 2500.0 && !seen { self.far_time + dt } else { 0.0 };
        // the tree's own warp (kh-ai's warp decision with EQSQuery_NpcWarp's point), else far for a second: behind him
        let tree_warp = match self.cmd.as_ref().map(|c| &c.cmd) {
            Some(kh_ai::AiCommand::Warp { goal, yaw }) => Some((*goal, *yaw)),
            _ => None,
        };
        if tree_warp.is_some() || self.far_time > 1.0 {
            let (p, yaw) = tree_warp.unwrap_or_else(|| {
                let a = (leader.yaw + 180.0).to_radians();
                (leader.position + Vec3::new(a.cos(), a.sin(), 0.0) * 250.0, leader.yaw)
            });
            sora.teleport(kh_core::Pose { position: p + Vec3::Z * 5.0, yaw });
            say!("party-play: t={now:.3}s AI Sora warps next to {}", leader.chara.name());
            self.far_time = 0.0;
            self.finish(true);
            return Input { camera_yaw, ..Input::default() };
        }
        // the command into the pad: the stick toward the goal, A at the target
        let mut stick = Vec2::ZERO;
        let mut buttons = PadButtons::NONE;
        let goal_of = |g: &kh_ai::AiGoal| -> Option<Vec3> {
            match g {
                kh_ai::AiGoal::Point(p) => Some(*p),
                kh_ai::AiGoal::Actor(a) => kh_ai::AiWorld::actor(&world, *a).map(|v| v.position),
            }
        };
        let towards = |p: Vec3, magnitude: f32| -> Vec2 {
            let d = Vec3::new(p.x - out.position.x, p.y - out.position.y, 0.0);
            if d.length() < 1.0 {
                return Vec2::ZERO;
            }
            let yaw = d.y.atan2(d.x).to_degrees() - camera_yaw;
            let (s, c) = yaw.to_radians().sin_cos();
            Vec2::new(s, c) * magnitude
        };
        let mut done: Option<bool> = None;
        if let Some(c) = self.cmd.as_mut() {
            c.age += dt;
            match &c.cmd {
                kh_ai::AiCommand::MoveTo { goal, walk, acceptable_radius, .. } => match goal_of(goal) {
                    Some(p) => {
                        let d = Vec2::new(p.x - out.position.x, p.y - out.position.y).length();
                        if d <= acceptable_radius.max(50.0) {
                            done = Some(true);
                        } else {
                            stick = towards(p, if *walk { 0.6 } else { 1.0 });
                        }
                        if c.age > 8.0 {
                            done = Some(false);
                        }
                    }
                    None => done = Some(false),
                },
                kh_ai::AiCommand::Action { target, location, .. } => {
                    let at = kh_ai::AiWorld::actor(&world, *target).map(|v| (v.position, v.radius)).or(location.map(|p| (p, 0.0)));
                    match at {
                        Some((p, r)) => {
                            let d = Vec2::new(p.x - out.position.x, p.y - out.position.y).length();
                            // (party AIX) in reach of the target's EDGE (his capsule 34 + 140 of the first swing; the old
                            // 220 cm to the centre was the same for a dummy of radius 50 but never reached an Elden Ring
                            // boss's capsule: he ran into it for 4 s and the command failed)
                            if d > 140.0 + 34.0 + r && !c.attacked {
                                stick = towards(p, 1.0);
                            } else if (!c.attacked || c.age - c.pressed_at <= 1.2) && self.press_wait <= 0.0 && !self.held.has(PadButton::ATTACK) {
                                // a chain of up to ~4 presses in the 1.2 s from the first, then the combo plays out
                                buttons.set(PadButton::ATTACK, true);
                                self.press_wait = 0.2;
                                if !c.attacked {
                                    c.pressed_at = c.age;
                                }
                                c.attacked = true;
                            }
                            // (party AIX) the tree decides again once HIS combo / finisher / reaction is over (the friends'
                            // Action task waits for the end message, 0x1404A2370): before, any state but Attack (a
                            // finisher's air part, a style change, magic) ended the command and the tree re-planned
                            // while he was busy (its lock-on query filters out a querier in an action: fail, restart)
                            if c.attacked && c.age - c.pressed_at > 1.2 && sora_free(out.state) {
                                done = Some(true);
                            }
                        }
                        None => done = Some(false),
                    }
                    if (c.age > 4.0 && !c.attacked) || c.age > 8.0 {
                        done = Some(c.attacked);
                    }
                }
                // (party AIX, OUR DESIGN) TurnTo: a short walk-stick tilt toward the goal turns him (the pad's own way; he
                // has no turn-in-place command), done within 10 degrees or after 0.75 s. Before, TurnTo answered at once
                // without turning: the WaitPhase / SideMove loops (AngleCheck 30 -> TurnTo -> Wait 0.1) spun 20 times a
                // second with him standing turned away from the enemy.
                kh_ai::AiCommand::TurnTo { goal } => match goal_of(goal) {
                    Some(p) => {
                        let d = Vec3::new(p.x - out.position.x, p.y - out.position.y, 0.0);
                        let want = d.y.atan2(d.x).to_degrees();
                        let off = kh_core::sora::wrap_degrees(want - out.yaw);
                        if off.abs() <= 10.0 || d.length() < 1.0 || c.age > 0.75 {
                            done = Some(true);
                        } else if sora_free(out.state) {
                            stick = towards(p, 0.45);
                        }
                    }
                    None => done = Some(false),
                },
                kh_ai::AiCommand::Idle { time } => {
                    if c.age >= time.max(0.0) {
                        done = Some(true);
                    }
                }
                _ => done = Some(true),
            }
        }
        // (party K1) THE HOLD, as the friends' (kh-party locomotion::arrive): behind a leader running faster than
        // k1.hold_leader_speed, a MoveTo that arrives does not stop him (every arrival was a RunStop and a new start at the
        // formation distance): the stick stays on along the leader's way for k1.hold_time or until the next command
        let was_move = matches!(self.cmd.as_ref().map(|c| &c.cmd), Some(kh_ai::AiCommand::MoveTo { .. }));
        let lead_v = Vec3::new(leader.velocity.x, leader.velocity.y, 0.0);
        let k = kh_party::locomotion::K;
        if done == Some(true) && was_move && lead_v.length() > k.k1_hold_leader_speed && !in_battle {
            self.k1_hold = k.k1_sora_hold_time;
        }
        if let Some(ok) = done {
            self.finish(ok);
        }
        // (the tree's next task after an arrival is often a TurnTo / Idle toward the player: the hold runs through them; a
        // MoveTo or an action ends it)
        let quiet = matches!(self.cmd.as_ref().map(|c| &c.cmd), None | Some(kh_ai::AiCommand::TurnTo { .. }) | Some(kh_ai::AiCommand::Idle { .. }));
        if quiet && self.k1_hold > 0.0 && lead_v.length() > k.k1_hold_leader_speed && !in_battle {
            stick = towards(out.position + lead_v.normalize() * 500.0, 1.0);
            self.k1_hold -= dt;
        } else {
            self.k1_hold = 0.0;
        }
        // (ZETTA) AI SORA KEEPS OUT OF ZETTAFLARE'S ZONES (the user: "allies should stay AWAY from the Zettaflare target"): in
        // one, the stick takes him out the nearest way; his stick never leads into one (it goes round); an attack on an
        // enemy in one is dropped (he disengages)
        let zones = rt.party.don_zetta_zones();
        if !zones.is_empty() {
            let in_zone_attack = matches!(self.cmd.as_ref().map(|c| &c.cmd), Some(kh_ai::AiCommand::Action { target, .. }) if kh_ai::AiWorld::actor(&world, *target).is_some_and(|v| zones.iter().any(|z| z.inside(v.position))));
            if in_zone_attack {
                self.finish(false);
                buttons = PadButtons::NONE;
                stick = Vec2::ZERO;
            }
            if let Some(exit) = kh_party::donald::zettaflare::zone_exit(&zones, out.position) {
                stick = towards(exit, 1.0);
                buttons = PadButtons::NONE;
            } else if stick.length() > 0.05 {
                let yaw = (stick.x.atan2(stick.y).to_degrees() + camera_yaw).to_radians();
                let ahead = out.position + Vec3::new(yaw.cos(), yaw.sin(), 0.0) * 300.0;
                let safe = kh_party::donald::zettaflare::zone_goal(&zones, out.position, ahead);
                if (safe - ahead).length() > 1.0 {
                    stick = towards(safe, stick.length().min(1.0));
                }
            }
        }
        // (ZETTA round 5c) WAITING OUT ZETTAFLARE (no enemy left to him): to the waiting spot round the zones, then he stands
        // ready there (no attack, no spell) until the player has Donald back
        if let Some(spot) = waiting {
            let zones = rt.party.don_zetta_zones();
            buttons = PadButtons::NONE;
            stick = if (out.position - spot).truncate().length() > 120.0 { towards(kh_party::donald::zettaflare::zone_goal(&zones, out.position, spot), 1.0) } else { Vec2::ZERO };
        } else {
            self.tools(sora, &status, &world, rt.lead.sora_settings, dt, now, &mut stick, &mut buttons);
        }
        self.press_wait = (self.press_wait - dt).max(0.0);
        let pad = kh_core::PadState { held: buttons, pressed: PadButtons(buttons.0 & !self.held.0), released: PadButtons(self.held.0 & !buttons.0) };
        self.held = buttons;
        Input { move_stick: stick, jump_pressed: false, jump_held: false, camera_yaw, pad, view: None }
    }

    /// AI SORA'S OWN TOOLS on top of the tree (OUR DESIGN, the user's decision 2026-10-15: his full kit except team
    /// attacks, which only the player-controlled leader starts):
    /// - Recovery (the Customize setting of his entry): Cure from his magic slots, else a Potion, when someone the
    ///   targets bits name (bits 0 / 1 himself, bit 2 the others: the leader and the friends) within 900 cm is below
    ///   the style's threshold (Use As Needed 50 %, In Emergencies 25 %, Like Crazy 75 %: the thresholds stream C's
    ///   cure decision uses [D]); at most every 4 s;
    /// - attack magic from his magic slots at the nearest enemy within 1200 cm, by the Abilities setting (Go Wild every
    ///   4 - 6 s; Watch MP every 7 - 10 s and only above half MP; Conserve every 14 - 18 s and only above 75 % MP);
    /// - shotlock: RB held 1 s (the aim), A, by the same setting (Go Wild 15 s, Watch MP 25 s, Conserve never), with
    ///   at least half the focus gauge (his gauge rules gate it: the core opens no aim without focus);
    /// - the situation command on offer when it is a formchange (Second Form), a finish or a grand magic, in battle
    ///   with an enemy within 800 cm: Y. NEVER a team attack (class TEAM) or an attraction.
    /// Every command goes through his own core (`Sora::command_request`, the pad), so his gates and costs apply.
    #[allow(clippy::too_many_arguments)]
    fn tools(&mut self, sora: &mut kh_core::Sora, status: &kh_core::Status, world: &LeaderAiWorld, s: FriendAiSettings, dt: f32, now: f64, stick: &mut Vec2, buttons: &mut PadButtons) {
        use kh_core::command::CommandRequest;
        use kh_core::magic::spells;
        if let (Some(ctl), true) = (self.ctl.as_mut(), self.applied != Some(s)) {
            ctl.set_settings(crate::runtime::ai_settings_of(s));
            self.applied = Some(s);
        }
        for w in [&mut self.cure_wait, &mut self.magic_wait, &mut self.shot_wait, &mut self.situation_wait] {
            *w = (*w - dt).max(0.0);
        }
        // (FX1) AN ACCEPTED TEAM MOVE FIRST: while the latch of a friend leader's unison / fusion runs (the host said
        // "yes" to his pad), none of his own tools (a situation command such as Myriad, a spell, a heal, the shotlock)
        // may take the window; his core starts the team move as his next attack
        if sora.combat().friend.leader_unison > 0.0 {
            return;
        }
        let out = sora.output();
        let me = out.position;
        let in_battle = sora.actions().battle || world.leader.in_battle;
        let mut enemies = Vec::new();
        kh_ai::AiWorld::enemies(world, &mut enemies);
        let nearest = enemies.iter().filter(|e| e.alive && e.hostile).map(|e| (e.position.distance(me), e.position)).min_by(|a, b| a.0.total_cmp(&b.0));
        // the shotlock in progress: RB held, A after a second, released after the volley starts
        if let Some(t) = self.shot.as_mut() {
            *t += dt;
            *stick = Vec2::ZERO;
            buttons.set(PadButton::RightShoulder, *t < 1.6);
            buttons.set(PadButton::ATTACK, (1.0..1.1).contains(t));
            if *t > 1.7 {
                self.shot = None;
            }
            return;
        }
        let mut rng = || {
            self.random ^= self.random << 13;
            self.random ^= self.random >> 17;
            self.random ^= self.random << 5;
            (self.random >> 8) as f32 / (1u32 << 24) as f32
        };
        // Recovery
        // (party AIX) the friends' own HP need (kh-ai cure.rs `hp_flag`, FUN_14126ac70): Use As Needed (OFTEN) below 35 %
        // in battle / 50 % out of it, Use in Emergencies (PINCH) at 25 % or less (vt+0x2538), Use Like Crazy (ALWAYS) below
        // 50 % / 80 %; before, the guessed 50 / 25 / 75 % [D] of the first design
        let low = |hp: i32, max: i32| {
            let pct = if max > 0 { hp * 100 / max } else { 100 };
            max > 0 && hp > 0 && match s.cure.min(2) { 0 => pct < if in_battle { 35 } else { 50 }, 1 => hp <= max * 25 / 100, _ => pct < if in_battle { 50 } else { 80 } }
        };
        let mut needs = Vec::new();
        if s.cure_targets & 3 != 0 && low(status.hp, status.max_hp()) {
            needs.push("himself".to_string());
        }
        if s.cure_targets & 4 != 0 {
            for v in world.members.iter().filter(|v| !v.ko && v.position.distance(me) < 900.0 && low(v.hp, v.hp_max)) {
                needs.push(v.chara.name().to_string());
            }
        }
        // (party AIX) the friends' field deferral (kh-ai params::FIELD_CURE_DEFER_SPEED): no heal out of battle while the
        // leader runs
        let leader_runs = world.leader.velocity.truncate().length() > kh_ai::controller::params::FIELD_CURE_DEFER_SPEED;
        if !needs.is_empty() && self.cure_wait <= 0.0 && !(leader_runs && !in_battle) {
            let cure = sora.command().magic_slots.iter().copied().find(|k| spells::is_cure(*k) && !spells::is_grand(*k));
            let potion = kh_core::magic::items::command_of_item(kh_core::status::ItemKind::Potion).filter(|_| sora.magic().pouch.count(kh_core::status::ItemKind::Potion) > 0);
            // (party AIX) a Potion heals only the one who drinks it (kh-core items: Sora): for someone else it was a
            // wasted item and the friend stayed low; Cure reaches the others in its range
            let himself = needs.first().is_some_and(|n| n == "himself");
            let kind = if !status.mp_charging { cure.or(potion.filter(|_| himself)) } else { potion.filter(|_| himself) };
            if let Some(kind) = kind {
                sora.command_request(CommandRequest::Execute(kind));
                self.tool_busy = Some(0.0); // (party AIX)
                say!("party-play: t={now:.3}s AI Sora: {} for {} (Recovery setting {})", spells::command_name(kind), needs.join(", "), s.cure);
            }
            self.cure_wait = 4.0;
            return;
        }
        let Some((dist, _at)) = nearest.filter(|_| in_battle) else { return };
        // the situation command on offer: Second Form, a finish, a grand magic (never a team attack)
        if self.situation_wait <= 0.0 && dist < 800.0 {
            if let Some((_, kind)) = sora.combat().situation.current() {
                use kh_core::combat::situation::{class, class_of};
                if matches!(class_of(kind), class::FORMCHANGE | class::FINISH | class::GRAND_MAGIC) {
                    buttons.set(PadButton::FaceTop, true);
                    self.situation_wait = 1.0;
                    self.tool_busy = Some(0.0); // (party AIX)
                    say!("party-play: t={now:.3}s AI Sora: situation command {kind} (Y)");
                    return;
                }
            }
        }
        let mp_rate = if status.max_mp() > 0 { status.mp as f32 / status.max_mp() as f32 } else { 0.0 };
        // shotlock
        let shot_every = [Some(15.0), Some(25.0), None][s.ability.min(2) as usize];
        if let Some(every) = shot_every {
            if self.shot_wait <= 0.0 && (300.0..1500.0).contains(&dist) && status.focus * 2 >= status.focus_max && status.focus_max > 0 {
                self.shot = Some(0.0);
                self.tool_busy = Some(0.0); // (party AIX)
                self.shot_wait = every;
                say!("party-play: t={now:.3}s AI Sora: shotlock (focus {}/{})", status.focus, status.focus_max);
                return;
            }
        }
        // attack magic
        let (every, mp_min) = [((4.0, 6.0), 0.0), ((7.0, 10.0), 0.5), ((14.0, 18.0), 0.75)][s.ability.min(2) as usize];
        if self.magic_wait <= 0.0 && dist < 1200.0 && !status.mp_charging && mp_rate >= mp_min {
            let list: Vec<u8> = sora.command().magic_slots.iter().copied().filter(|k| *k != 0 && spells::is_magic(*k) && !spells::is_cure(*k) && !spells::is_grand(*k)).collect();
            if !list.is_empty() {
                let kind = list[((rng() * list.len() as f32) as usize).min(list.len() - 1)];
                sora.command_request(CommandRequest::Execute(kind));
                self.tool_busy = Some(0.0); // (party AIX)
                say!("party-play: t={now:.3}s AI Sora: casts {} at {:.0} cm", spells::command_name(kind), dist);
            }
            self.magic_wait = every.0 + (every.1 - every.0) * rng();
        }
    }

    fn finish(&mut self, ok: bool) {
        if let Some(c) = self.cmd.take() {
            self.history.push((c.ticket, if ok { kh_ai::AiCommandStatus::Succeeded } else { kh_ai::AiCommandStatus::Failed }));
        }
    }
}

/// (party P) the leader's members by view (a convenience for hosts): the leading member's view after the last step.
pub fn leader_member(rt: &PartyRuntime) -> Option<&MemberView> {
    rt.leader.and_then(|l| rt.current.iter().find(|v| v.id == l))
}

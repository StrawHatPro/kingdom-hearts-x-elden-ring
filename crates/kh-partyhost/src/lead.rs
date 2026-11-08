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

use crate::aiworld::{EnemyFeed, LeaderAiWorld, PartyTargets, SoraAiWorld, SORA_ACTOR, STRAIGHT_NAV};
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
    /// (MIMIC-BOSS) THE SOLO FIGHT of a Mimic Tear boss (crate::mimicboss): the party as it was at the fog gate; while it is held
    /// no formation change is taken ([`Leadership::set_formation`]) and a save keeps `before`.
    pub solo: Option<crate::mimicboss::SoloHold>,
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
            solo: None, // (MIMIC-BOSS)
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
        // (MIMIC-BOSS) the solo fight: nobody is switched in or out until it ends (mimicboss.rs `solo_end`)
        if let Some(h) = self.solo.as_ref().filter(|h| f != h.solo_formation()) {
            say!("party-play: formation {:?}, leader {}: REFUSED: the {} fight is solo until it ends", f.active().iter().map(|c| c.name()).collect::<Vec<_>>(), f.leader().name(), h.boss);
            return;
        }
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

    /// (HUD-REVIVE SORAKO) AI SORA IS KNOCKED OUT: a friend leads and Sora's core is at 0 HP (in the Die state he lies in his
    /// down pose on the ground: kh-core states/die.rs, a party member's KO). False while Sora leads (his death is the
    /// game over: kh-er death.rs) and once he has HP again (his get-up plays, then Idle).
    pub fn ai_sora_down(&self, sora: &kh_core::Sora) -> bool {
        self.leader.is_some() && (sora_ko(sora.status()) || (sora.status().hp < 1 && sora.output().state == kh_core::StateId::Die))
    }

    /// (HUD-REVIVE SORAKO) THE REVIVE OF AI SORA (the revive policy's mechanism; KH3's friends' `UTresNpcState_Die` rule: HP
    /// above 0 ends the down): his HP set to `hp` (1 ..= his maximum); his core's Die state then plays the get-up (the blow
    /// family's 0213 / 0216, invincible) at his next step and finishes into Idle; his AI (stopped while he is down) starts
    /// afresh. A kill plane's KO (`combat.kill_plane_held`: he lies in the barrier, his HP held at 0) is lifted and he is put
    /// behind the leader standing (`Sora::teleport`: a fresh core in Idle, the friends' warp spot), as the friends' kill-plane
    /// KO ends only by the host's stand-up. Any HP gain revives him the same way (a Recover / FullRecover request on his core,
    /// a Cure): nothing else is needed for a save point's Recover. False: he was not down (or no friend leads).
    pub fn revive_ai_sora(&mut self, sora: &mut kh_core::Sora, hp: i32) -> bool {
        if !self.ai_sora_down(sora) {
            return false;
        }
        let hp = hp.clamp(1, sora.status().max_hp().max(1));
        if sora.combat().kill_plane_held {
            sora.combat_mut().kill_plane_held = false;
            let at = self.leader.and_then(|l| self.party.view(l)).map(|v| {
                let a = (v.yaw + 180.0).to_radians();
                (v.position + Vec3::new(a.cos(), a.sin(), 0.0) * 250.0 + Vec3::Z * 5.0, v.yaw)
            });
            let (position, yaw) = at.unwrap_or((sora.output().position, sora.output().yaw));
            sora.teleport(kh_core::Pose { position, yaw });
            say!("party-play: AI Sora revived from a kill plane's KO: put behind the leader, standing, {hp} HP");
        } else {
            say!("party-play: AI Sora revived: {hp} HP, his get-up plays");
        }
        sora.status_mut().hp = hp;
        true
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
        // (CHESTFIX) the follow speed cap is AI Sora's alone: set again below by his AI each step (`SoraAi::input`)
        sora.set_follow_cap(None);
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
        sora.combat_mut().friend.ko_game_over_pose = self.leader.is_some(); // (HUD-REVIVE) his KO: the game-over pose, hovering
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
        let abilities = sora.status().abilities;
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
        // (GRABS) AI Sora caught by a throw or getting up (caught.rs): his tree stopped, nothing pressed, no step; it starts
        // afresh when he is up
        let caught = self.caught.stops(PartyChara::Sora);
        // (HUD-REVIVE SORAKO) KO'd (or getting up: the Die state) his tree is stopped too, as caught: it starts afresh when
        // he stands (`SoraAi::drive` presses nothing and never warps him meanwhile)
        let down = sora_down(sora);
        if let Some(ctl) = ai.ctl.as_mut() {
            ctl.set_stop_ai(caught || down);
        }
        let mut out = ai.input(self, sora, AiStep { dt, now }, AiScene { enemies: &enemies, camera, camera_yaw: input.camera_yaw, nav: nav.unwrap_or(&STRAIGHT_NAV) });
        if caught {
            (out.move_stick, out.jump_pressed, out.jump_held, out.pad) = (kh_core::Vec2::ZERO, false, false, kh_core::PadState::default());
        }
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
    pub(crate) fn apply_formation(&mut self, sora: &mut kh_core::Sora, now: f64) {
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
        // (PROMPT-UI) the one gate of the Y owner's prompts: HIS button notice and style-point gauge, never Sora's
        kh_party::yhud::y_prompts(&self.party, id).apply(&mut s);
        // (party X2-b) his own aim (Discus Barrage / Spell Volley) as Sora's shotlock scope and lock marks
        s.aim = crate::custom::leader_aim(&self.party, id).unwrap_or_default();
        s.sneeze = false;
        Some(s)
    }

    /// (party K1) AI Sora's speed scale behind the leading friend: the FollowPlayer rule's follow speed (the leader's
    /// MaxWalkSpeed, at least his actual speed, x the in-view rate beyond m_RangeSamePlayerSpeed, x distance / range
    /// within it) over his sprint speed, at least 1; 1 in battle. Port design (KH3 has no AI Sora; the friends' rule).
    pub fn ai_sora_speed_scale(&self, leader: kh_party::MemberId, out: &kh_core::Output, in_battle: bool) -> f32 {
        let (Some(m), false) = (self.party.member(leader), in_battle) else { return 1.0 };
        follow_speed_scale(m.position(), m.cmc.velocity, m.cmc.max_walk_speed.max(m.params.speed_run), out)
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

/// (party K1) THE FRIENDS' FOLLOW SPEED RULE for an AI Sora (kh-party locomotion, KH3 0x1412196C0) as a scale on his sprint:
/// the leader's reference speed (his run, at least his actual speed) x the in-view rate beyond m_RangeSamePlayerSpeed, x
/// distance / range within it, capped. (MIMIC SORA) shared by AI Sora and Mimic Sora (whose leader may be the player's Sora).
pub(crate) fn follow_speed_scale(leader: Vec3, leader_velocity: Vec3, reference: f32, out: &kh_core::Output) -> f32 {
    let k = kh_party::locomotion::K;
    let d = leader - out.position;
    let dist = (d.x * d.x + d.y * d.y).sqrt();
    let range = k.follow_same_speed_range;
    let rate = if dist <= range { dist / range } else { k.follow_in_view_rate };
    let v = leader_velocity;
    let reference = reference.max((v.x * v.x + v.y * v.y).sqrt());
    (reference * rate).min(k.k1_speed_cap) / kh_core::SORA_PARAMS.speed_sprint.max(1.0)
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
    /// His actor id among the allies: `SORA_ACTOR` for AI Sora, (MIMIC SORA) `MIMIC_ACTOR` for Mimic Sora.
    actor: u8,
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

/// (HUD-REVIVE SORAKO) An AI Sora core is DOWN for his AI: HP 0, or still in the Die state (his down pose, or the get-up after
/// a revive: kh-core states/die.rs, a party member's KO). A placeholder status (no loadout given: test hosts) reads 0 HP and is
/// no KO.
fn sora_down(sora: &kh_core::Sora) -> bool {
    sora.output().state == kh_core::StateId::Die || sora_ko(sora.status())
}

/// (HUD-REVIVE SORAKO) 0 HP on a real status (the placeholder's 0 is no KO).
fn sora_ko(st: &kh_core::Status) -> bool {
    st.hp < 1 && !st.placeholder
}

/// (CHESTFIX) AI Sora's way behind a running friend leader: where his stick points and his speed cap (cm/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FollowWay {
    pub aim: Vec3,
    pub cap: f32,
    /// The goal was a spot beside or ahead of the leader: the spot behind him instead.
    pub moved_back: bool,
}

/// (CHESTFIX, the user 2026-10-27: "he also keeps trying to run in front of Donald so fix that, he should just follow like
/// Donald and Goofy do when playing as Sora") AI SORA FOLLOWS AS THE FRIENDS DO (kh-party locomotion.rs FOL2, the friends'
/// Move1 behind a running Sora). THE CAUSE: the friends' speed is a follow SPEED (the leader's plus the gap to their spot
/// over FOL2_GAP_TIME, a goal run past eased off along his way, never back for it, and only spots behind him travel with
/// him: FOL2 r3), but AI Sora's was a full stick: his own run 680 and after 0.3 s his sprint 980 x the catch-up scale
/// (never below 1), the tree's EQS spot taken as it is (beside or ahead of the leader too), and his K1 hold (3 s, the
/// friends' 0.6 s) ran the stick on along the leader's way wherever he was: behind Donald's run of 600-680 he passed him.
/// NOW, while the leader runs (faster than k1.hold_leader_speed) and AI Sora is out of battle on a run MoveTo or the hold:
/// - his goal: the tree's spot when it is at least FOL2_LIVE_BEHIND behind the leader along his way, else the spot
///   follow.same_speed_range (300) behind him, the tree's sideways offset kept up to FOL2_LATERAL_CM;
/// - level with it or past it (less than FOL2_ALONG_AHEAD ahead of him along the way): along the leader's way, the sideways
///   gap taken gently, at the leader's speed plus that gap over FOL2_GAP_TIME (at least half the leader's): ahead of the
///   leader that is half his speed, so the leader passes him and he falls in behind;
/// - farther behind: toward the goal at KH3's follow speed (the leader's x 1.1 seen / 2.5 not, distance / range within
///   it) and at most the leader's speed plus what FOL2_CLOSE_DECEL takes off before the follow distance (the friends'
///   catch-up that eases off), at most k1.speed_cap. `seen`: the camera sees him (the friends' frustum test).
///   None: the rule does not apply (in battle, the leader slow or standing): the tree's own way and his own speeds.
pub(crate) fn sora_follow_way(sora: Vec3, goal: Option<Vec3>, leader: Vec3, leader_velocity: Vec3, in_battle: bool, seen: bool) -> Option<FollowWay> {
    use kh_party::locomotion::{FOL2_ALONG_AHEAD, FOL2_CLOSE_DECEL, FOL2_GAP_TIME, FOL2_LATERAL_CM, FOL2_LIVE_BEHIND, K};
    let flat = |v: Vec3| Vec3::new(v.x, v.y, 0.0);
    let lv = flat(leader_velocity);
    let ls = lv.length();
    if in_battle || ls <= K.k1_hold_leader_speed {
        return None;
    }
    let ld = lv / ls;
    let lateral_of = |v: Vec3| {
        let side = flat(v) - ld * flat(v).dot(ld);
        if side.length() > FOL2_LATERAL_CM { side.normalize() * FOL2_LATERAL_CM } else { side }
    };
    let behind_spot = leader - ld * K.follow_same_speed_range;
    let (goal, moved_back) = match goal {
        Some(g) if flat(g - leader).dot(ld) <= -FOL2_LIVE_BEHIND => (g, false),
        Some(g) => (behind_spot + lateral_of(g - leader), true),
        None => (behind_spot, false),
    };
    let d = flat(goal - sora);
    let err = d.dot(ld);
    if err < FOL2_ALONG_AHEAD {
        let side = d - ld * err;
        let aim = sora + ld * 300.0 + side.clamp_length_max(FOL2_LATERAL_CM) * 0.5;
        return Some(FollowWay { aim: Vec3::new(aim.x, aim.y, sora.z), cap: (ls + err / FOL2_GAP_TIME).max(0.5 * ls), moved_back });
    }
    let dl = flat(leader - sora).length();
    // KH3's follow rule (0x1412196C0, the friends' `follow_speed_with`, continuous): the leader's speed x the view rate (1.1
    // seen, 2.5 not), within it distance / range, at least his speed
    let rate = (if seen { K.follow_in_view_rate } else { K.follow_out_view_rate }).min(dl / K.follow_same_speed_range).max(1.0);
    let ease = ls + (2.0 * FOL2_CLOSE_DECEL * (dl - K.follow_same_speed_range).max(0.0)).sqrt();
    let cap = (ls * rate).min(ease).min(K.k1_speed_cap);
    Some(FollowWay { aim: Vec3::new(goal.x, goal.y, sora.z), cap, moved_back })
}

impl kh_ai::AiBody for SoraBody<'_> {
    fn view(&self) -> kh_ai::BodyView {
        kh_ai::BodyView {
            member: self.actor,
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
            items: [None; kh_ai::ITEM_SLOTS],
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

/// AI Sora's step: the fixed step (s) and the run's time (s).
#[derive(Clone, Copy)]
struct AiStep {
    dt: f32,
    now: f64,
}

/// What AI Sora's step sees: the enemies, the drawn camera (`camera_yaw`: what his stick is relative to), the navigation.
#[derive(Clone, Copy)]
struct AiScene<'a> {
    enemies: &'a EnemyFeed,
    camera: &'a kh_camera::CameraView,
    camera_yaw: f32,
    nav: &'a dyn kh_ai::Nav,
}

/// The pad AI Sora's tools write this step.
struct ToolPad<'a> {
    stick: &'a mut Vec2,
    buttons: &'a mut PadButtons,
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
    /// (MIMIC SORA) his actor id (`SORA_ACTOR`, `MIMIC_ACTOR`) and his name in the log ("AI Sora", "Mimic Sora").
    actor: u8,
    name: &'static str,
    /// (MIMIC-BOSS-KIT) THE LEADER SORA'S FULL KIT ([`full_kit_of`]: the copies, Mimic Sora and the Mimic Tear boss's Sora):
    /// every Y command of his own (reaction commands too) with L2 to reach one behind a row he may not use, the Keyblade
    /// switch, Aerial Recovery. AI Sora keeps the kit the user chose for him (2026-10-15).
    full_kit: bool,
    /// (MIMIC-BOSS-KIT) seconds until the next Keyblade switch is tried; the switch count before the press in flight and the
    /// seconds it has been tried.
    switch_wait: f32,
    switch_try: Option<(u32, f32)>,
    /// (MIMIC-BOSS-KIT) the jump of an Aerial Recovery was pressed in this damage reaction.
    recovered: bool,
    /// (MIMIC-BOSS hotfix 3, MB3-AGGRO) A boss copy's hostile tuning (mimicboss.rs `CopyAggro`; None or off: as before), its own
    /// combo chain (seconds since its first press) and the wait of an Aerial Recovery it decided to take.
    aggro: Option<crate::mimicboss::CopyAggro>,
    chain: Option<f32>,
    recover_wait: Option<f32>,
}

/// (MIMIC-BOSS-KIT, the coordinator's rule: "a COPY has the copied leader's FULL kit, under the same requirements the real
/// character has") The AI Soras who are COPIES of the leader Sora: Mimic Sora ([`MIMIC_ACTOR`]) and the Mimic Tear boss's
/// Sora ([`BOSS_ACTOR`]). The party's own AI Sora ([`SORA_ACTOR`]) keeps the kit of the user's decision of 2026-10-15
/// ("his full kit except team attacks": the formchange / finish / grand magic Y, magic, the shotlock, Cure / Potion).
pub(crate) fn full_kit_of(actor: u8) -> bool {
    actor == MIMIC_ACTOR || actor == BOSS_ACTOR
}

/// (MIMIC-BOSS-KIT) THE Y ROWS A COPY MAY USE: every situation command of the leader Sora's own kit (reaction commands such as
/// Aerial Recovery's counter, formchanges, finishes and their extras, grand magic, Rage / XS commands, a shotlock row), never
/// a team attack (it needs the party: classes TEAM and SINGLE_B's team finishes SFinish 211 / FrFinish 214 /
/// FrLinkofhearts 241), an attraction (excluded by the user forever: class ATTRACTION and its finish AfFinish 192) or a Link
/// (KH3's summons: class LINK; the copies never summon). An ex entry (a prompt's reaction command) is always his own.
pub(crate) fn copy_may_use(queue: &kh_core::combat::situation::Queue, kind: i32) -> bool {
    use kh_core::combat::situation::{class, class_of};
    if queue.find(kind).is_some_and(|(_, e)| e.ex) {
        return true;
    }
    !matches!(class_of(kind), class::ATTRACTION | class::TEAM | class::LINK) && !matches!(kind, 186..=192 | 193..=210 | 211 | 214 | 237..=242)
}

/// (MIMIC-BOSS-KIT) OUR DESIGN (KH3 has no AI Sora): a copy switches Keyblades every [`COPY_SWITCH_EVERY`] s (the low end,
/// plus up to [`COPY_SWITCH_SPREAD`] at random) in battle, KH3's own way (D-pad right on Attack, kh-core
/// `status::keyblade::request_switch`, its whole gate); a refused press is tried again every [`COPY_SWITCH_RETRY`] s for at
/// most [`COPY_SWITCH_GIVE_UP`] s.
pub const COPY_SWITCH_EVERY: f32 = 12.0;
pub const COPY_SWITCH_SPREAD: f32 = 8.0;
pub const COPY_SWITCH_RETRY: f32 = 0.25;
pub const COPY_SWITCH_GIVE_UP: f32 = 3.0;

/// (MIMIC SORA) Mimic Sora's actor id among the allies (AI Sora's is `SORA_ACTOR`): his own tree controller, never AI Sora's.
pub const MIMIC_ACTOR: u8 = 8;

/// (MIMIC SORA) WHOM AN AI SORA FOLLOWS: where the leader stands, how he moves, his fight (AI Sora: the friend leader; Mimic
/// Sora: the player's Sora while he leads, else the friend leader).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lead {
    pub position: Vec3,
    pub yaw: f32,
    pub velocity: Vec3,
    pub in_battle: bool,
    pub name: &'static str,
}

impl Lead {
    fn of_member(v: &MemberView) -> Lead {
        Lead { position: v.position, yaw: v.yaw, velocity: v.velocity, in_battle: v.in_battle, name: v.chara.name() }
    }
}

/// (MIMIC SORA) The leader, the trees' world, the allies and the AI settings of one AI Sora step.
#[derive(Clone, Copy)]
struct AiLeadWorld<'a> {
    lead: Lead,
    world: &'a dyn kh_ai::AiWorld,
    members: &'a [MemberView],
    settings: FriendAiSettings,
}

impl SoraAi {
    fn new(rt: &PartyRuntime) -> SoraAi {
        SoraAi::with_actor(rt, SORA_ACTOR, "AI Sora", 0x50_7A_A1)
    }

    fn with_actor(rt: &PartyRuntime, actor: u8, name: &'static str, seed: u32) -> SoraAi {
        let ctl = rt.ai_pack.clone().map(|pack| kh_ai::AiController::new(pack, "n_ex002", actor, seed as u64));
        if ctl.is_none() {
            say!("party-play: no AI pack: {name} only follows the leader");
        }
        // (MIMIC-BOSS-KIT) the copies' full kit; the first Keyblade switch after the first quarter of the interval
        let full_kit = full_kit_of(actor);
        SoraAi { ctl, shared: kh_ai::AiShared::new(), cmd: None, history: Vec::new(), next: 1, held: PadButtons::NONE, press_wait: 0.0, far_time: 0.0, cure_wait: 0.0, magic_wait: 3.0, shot_wait: 6.0, situation_wait: 0.0, shot: None, applied: None, random: seed, tool_busy: None, k1_hold: 0.0, actor, name, full_kit, switch_wait: COPY_SWITCH_EVERY * 0.25, switch_try: None, recovered: false, aggro: None, chain: None, recover_wait: None }
    }

    /// (MIMIC-BOSS-KIT) AERIAL RECOVERY, the copies' (his ability AirRecovery; KH3's jump press in the recoverable window of a
    /// blow, toss, launch or parabola, kh-core states/jump.rs `jump_request` -> AIR_SLIDE with its reaction command, Blow
    /// Counter): once a reaction, as soon as the window opens (`Sora::damage_reaction().recoverable`, 0.2 s after the hit).
    /// (MIMIC-BOSS hotfix 3, MB3-AGGRO) A boss copy takes it out of `sora_recover_chance` of the blows (one roll a blow), and
    /// `sora_recover_delay` s into the window (a window that closes first: none).
    fn air_recovery(&mut self, sora: &kh_core::Sora, dt: f32) -> bool {
        match sora.damage_reaction() {
            Some(d) if d.recoverable && !self.recovered && sora.status().abilities.count(kh_core::enums::ETresAbilityKind::AirRecovery) > 0 => {
                let Some(a) = self.aggro.filter(|a| a.on) else {
                    self.recovered = true;
                    return true;
                };
                let waited = match self.recover_wait {
                    Some(w) => w + dt,
                    None => {
                        if self.unit() >= a.sora_recover_chance {
                            self.recovered = true;
                            return false;
                        }
                        0.0
                    }
                };
                if waited >= a.sora_recover_delay {
                    self.recovered = true;
                    self.recover_wait = None;
                    return true;
                }
                self.recover_wait = Some(waited);
                false
            }
            Some(_) => false,
            None => {
                self.recovered = false;
                self.recover_wait = None;
                false
            }
        }
    }

    /// (MB3-AGGRO) Its tuning when on.
    fn hostile(&self) -> Option<crate::mimicboss::CopyAggro> {
        self.aggro.filter(|a| a.on)
    }

    /// (MB3-AGGRO) The low end of its Keyblade switch interval (a boss copy's own; the spread scaled with it).
    fn switch_every(&self) -> (f32, f32) {
        self.hostile().map_or((COPY_SWITCH_EVERY, COPY_SWITCH_SPREAD), |a| (a.sora_switch_every, COPY_SWITCH_SPREAD * a.sora_switch_every / COPY_SWITCH_EVERY))
    }

    /// (MIMIC-BOSS hotfix 3, MB3-AGGRO; THE USER 2026-11-06: "the boss mimic tear needs to be way way more aggressive") A BOSS
    /// COPY PRESSES: whenever its tree has no attack of its own running (an Idle, a TurnTo, a walk to a formation spot) and no
    /// tool of its runs, it runs at the nearest enemy and, from `sora_gap_close` (the core's own soft target and dashes) or in
    /// reach, chains Attack presses for `sora_combo_for` s, every `sora_press_every` s, into its next combo as soon as the core
    /// lets it (the combo's end lag, a flinch, a recovery are the core's). Off: nothing.
    fn press(&mut self, out: &kh_core::Output, nearest: Option<(Vec3, f32)>, dt: f32, camera_yaw: f32, pad: ToolPad<'_>) {
        let ToolPad { stick, buttons } = pad;
        let Some(a) = self.hostile().filter(|a| a.sora_press) else { return };
        let tree_attacks = matches!(self.cmd.as_ref().map(|c| &c.cmd), Some(kh_ai::AiCommand::Action { .. }));
        if tree_attacks || self.tool_busy.is_some() || self.shot.is_some() {
            self.chain = None;
            return;
        }
        let Some((at, radius)) = nearest else {
            self.chain = None;
            return;
        };
        if let Some(t) = self.chain.as_mut() {
            *t += dt;
            if *t > a.sora_combo_for && sora_free(out.state) {
                self.chain = None;
            }
        }
        let gap = Vec2::new(at.x - out.position.x, at.y - out.position.y).length() - 34.0 - radius;
        let press_now = self.press_wait <= 0.0 && !self.held.has(PadButton::ATTACK);
        match self.chain {
            Some(t) => {
                if t <= a.sora_combo_for && press_now {
                    buttons.set(PadButton::ATTACK, true);
                    self.press_wait = a.sora_press_every;
                }
            }
            None if !sora_free(out.state) => {}
            None if gap <= a.sora_gap_close.max(140.0) => {
                if press_now {
                    buttons.set(PadButton::ATTACK, true);
                    self.press_wait = a.sora_press_every;
                    self.chain = Some(0.0);
                }
            }
            None => {
                let d = Vec3::new(at.x - out.position.x, at.y - out.position.y, 0.0);
                let yaw = d.y.atan2(d.x).to_degrees() - camera_yaw;
                let (sn, cs) = yaw.to_radians().sin_cos();
                *stick = Vec2::new(sn, cs);
            }
        }
    }

    /// (MIMIC-BOSS-KIT) THE KEYBLADE SWITCH of a copy (see [`COPY_SWITCH_EVERY`]): D-pad right while his command menu is on
    /// Attack (no other press this step), with two Keyblades or more, in battle. True: pressed now.
    fn keyblade_switch(&mut self, sora: &kh_core::Sora, dt: f32, now: f64) -> bool {
        let switches = sora.status().keyblade_switches;
        if let Some((before, age)) = self.switch_try.as_mut() {
            *age += dt;
            if switches != *before {
                say!("party-play: t={now:.3}s {}: Keyblade switch to {:?} (D-pad right on Attack)", self.name, sora.status().current_keyblade().map(|k| k.key()));
                self.switch_try = None;
                let (every, spread) = self.switch_every(); // (MB3-AGGRO)
                self.switch_wait = every + spread * self.unit();
                return false;
            }
            if *age > COPY_SWITCH_GIVE_UP {
                self.switch_try = None;
                self.switch_wait = COPY_SWITCH_EVERY;
                return false;
            }
        }
        self.switch_wait = (self.switch_wait - dt).max(0.0);
        let c = sora.command();
        let on_attack = c.state == kh_core::command::MenuState::MainSel && c.main_cursor == 0;
        if self.switch_wait > 0.0 || sora.status().keyblades().filled() < 2 || !on_attack || self.held.has(PadButton::DPadRight) {
            return false;
        }
        let age = self.switch_try.map_or(0.0, |t| t.1);
        self.switch_try = Some((switches, age));
        self.switch_wait = COPY_SWITCH_RETRY;
        true
    }

    /// (MIMIC-BOSS-KIT) His RNG's next number in 0..1 (the tools' xorshift).
    fn unit(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        (self.random >> 8) as f32 / (1u32 << 24) as f32
    }

    fn input(&mut self, rt: &PartyRuntime, sora: &mut kh_core::Sora, step: AiStep, scene: AiScene<'_>) -> Input {
        let out = sora.output();
        let status = sora.status().clone();
        let Some(leader) = rt.leader.and_then(|l| rt.current.iter().find(|v| v.id == l)) else { return Input::default() };
        let in_battle = sora.actions().battle;
        let mps = rt.member_mps();
        let world = LeaderAiWorld {
            leader,
            lock_on: rt.party.pad_state(leader.id).map_or(TargetId::NONE, |p| p.lock),
            in_battle: in_battle || leader.in_battle,
            members: &rt.current,
            sora: Some((&out, &status)),
            enemies: scene.enemies,
            camera: rt.ai_camera(scene.camera), // (party I2) the drawn view
            nav: scene.nav,
            game_level: status.game_level as u8,
            mps: &mps, // (party I2)
        };
        let lead = Lead::of_member(leader);
        self.drive(rt, sora, step, scene, AiLeadWorld { lead, world: &world, members: &rt.current, settings: rt.lead.sora_settings })
    }

    /// THE ONE BODY OF AN AI SORA'S STEP, whoever he follows (AI Sora behind a friend leader; (MIMIC SORA) Mimic Sora behind
    /// the player's Sora or a friend leader): the tree's tick, the warp, the command into the pad, the follow and his tools.
    fn drive(&mut self, rt: &PartyRuntime, sora: &mut kh_core::Sora, step: AiStep, scene: AiScene<'_>, AiLeadWorld { lead, world, members, settings }: AiLeadWorld<'_>) -> Input {
        let AiStep { dt, now } = step;
        let AiScene { enemies, camera_yaw, nav, .. } = scene;
        let out = sora.output();
        let status = sora.status().clone();
        // (ZETTA round 5c) no enemy left to him through Zettaflare: he waits at the nearest spot off its zones
        let waiting = if rt.zetta_wait_spots(lead.position, enemies, nav).is_empty() { None } else { kh_party::donald::zettaflare::wait_place(&rt.party, None, out.position) };
        let in_battle = sora.actions().battle;
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
        // (HUD-REVIVE SORAKO) DOWN (HP 0, or the Die state's down pose / get-up): no tree, no tool, nothing pressed and NO WARP
        // (a warp is `Sora::teleport`: a fresh core in Idle, which stood a KO'd Sora up at 0 HP); his running command fails,
        // a shotlock sequence in flight is over
        if sora_down(sora) {
            self.finish(false);
            self.shot = None;
            self.tool_busy = None;
            self.far_time = 0.0;
            self.held = PadButtons::NONE;
            return Input { camera_yaw, ..Input::default() };
        }
        if let (Some(ctl), None) = (self.ctl.as_mut(), self.tool_busy) {
            let mut body = SoraBody { actor: self.actor, out: &out, status: &status, in_battle: in_battle || lead.in_battle, cmd: &mut self.cmd, history: &mut self.history, next: &mut self.next };
            ctl.tick(dt, &mut body, world, &mut self.shared);
        }
        // Warp (the friends' rule, kh-ai warp decision): far from the leader for a while -> next to him
        let far = out.position.distance(lead.position);
        // (party AIX) only while the camera does not see him (the friends' warp rule, FUN_141258790's frustum test): the
        // put-behind is never seen
        let seen = kh_ai::controller::in_view(&world.camera(), out.capsule_center, 90.0);
        self.far_time = if far > 2500.0 && !seen { self.far_time + dt } else { 0.0 };
        // the tree's own warp (kh-ai's warp decision with EQSQuery_NpcWarp's point), else far for a second: behind him
        let tree_warp = match self.cmd.as_ref().map(|c| &c.cmd) {
            Some(kh_ai::AiCommand::Warp { goal, yaw }) => Some((*goal, *yaw)),
            _ => None,
        };
        if tree_warp.is_some() || self.far_time > 1.0 {
            let (p, yaw) = tree_warp.unwrap_or_else(|| {
                let a = (lead.yaw + 180.0).to_radians();
                (lead.position + Vec3::new(a.cos(), a.sin(), 0.0) * 250.0, lead.yaw)
            });
            sora.teleport(kh_core::Pose { position: p + Vec3::Z * 5.0, yaw });
            say!("party-play: t={now:.3}s {} warps next to {}", self.name, lead.name);
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
                kh_ai::AiGoal::Actor(a) => world.actor(*a).map(|v| v.position),
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
        // (MB3-AGGRO) a boss copy: its chain, its press rate, its reach for the first press (gap-close), the longest Idle
        let (chain_for, press_every, first_reach, idle_max) = self.hostile().map_or((1.2, 0.2, 140.0, f32::INFINITY), |a| (a.sora_combo_for, a.sora_press_every, a.sora_gap_close.max(140.0), a.sora_idle_max));
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
                    let at = world.actor(*target).map(|v| (v.position, v.radius)).or(location.map(|p| (p, 0.0)));
                    match at {
                        Some((p, r)) => {
                            let d = Vec2::new(p.x - out.position.x, p.y - out.position.y).length();
                            // (party AIX) in reach of the target's EDGE (his capsule 34 + 140 of the first swing; the old
                            // 220 cm to the centre was the same for a dummy of radius 50 but never reached an Elden Ring
                            // boss's capsule: he ran into it for 4 s and the command failed)
                            if d > first_reach + 34.0 + r && !c.attacked {
                                stick = towards(p, 1.0);
                            } else if (!c.attacked || c.age - c.pressed_at <= chain_for) && self.press_wait <= 0.0 && !self.held.has(PadButton::ATTACK) {
                                // a chain of up to ~4 presses in the 1.2 s from the first, then the combo plays out
                                buttons.set(PadButton::ATTACK, true);
                                self.press_wait = press_every;
                                if !c.attacked {
                                    c.pressed_at = c.age;
                                }
                                c.attacked = true;
                            }
                            // (party AIX) the tree decides again once HIS combo / finisher / reaction is over (the friends'
                            // Action task waits for the end message, 0x1404A2370): before, any state but Attack (a
                            // finisher's air part, a style change, magic) ended the command and the tree re-planned
                            // while he was busy (its lock-on query filters out a querier in an action: fail, restart)
                            if c.attacked && c.age - c.pressed_at > chain_for && sora_free(out.state) {
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
                    if c.age >= time.max(0.0).min(idle_max) {
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
        let lead_v = Vec3::new(lead.velocity.x, lead.velocity.y, 0.0);
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
        // (CHESTFIX, the user 2026-10-27: "he also keeps trying to run in front of Donald ... he should just follow like
        // Donald and Goofy do") THE FRIENDS' FORMATION behind a running leader (`sora_follow_way`): on a run MoveTo or the
        // hold, his goal only a spot behind the leader, his stick along the leader's way once level with it, his speed the
        // friends' follow speed (kh-core `set_follow_cap`); otherwise his own speeds
        let follow_goal = match self.cmd.as_ref().map(|c| &c.cmd) {
            Some(kh_ai::AiCommand::MoveTo { goal, walk: false, .. }) => goal_of(goal).map(Some),
            None | Some(kh_ai::AiCommand::TurnTo { .. }) | Some(kh_ai::AiCommand::Idle { .. }) if self.k1_hold > 0.0 => Some(None),
            _ => None,
        };
        match follow_goal.and_then(|g| sora_follow_way(out.position, g, lead.position, lead.velocity, in_battle, seen)) {
            Some(way) => {
                stick = towards(way.aim, 1.0);
                sora.set_follow_cap(Some(way.cap));
            }
            None => sora.set_follow_cap(None),
        }
        // (ZETTA) AI SORA KEEPS OUT OF ZETTAFLARE'S ZONES (the user: "allies should stay AWAY from the Zettaflare target"): in
        // one, the stick takes him out the nearest way; his stick never leads into one (it goes round); an attack on an
        // enemy in one is dropped (he disengages)
        let zones = rt.party.don_zetta_zones();
        if !zones.is_empty() {
            let in_zone_attack = matches!(self.cmd.as_ref().map(|c| &c.cmd), Some(kh_ai::AiCommand::Action { target, .. }) if world.actor(*target).is_some_and(|v| zones.iter().any(|z| z.inside(v.position))));
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
            // (MB3-AGGRO) a boss copy presses the nearest enemy (its tools may still take the step after)
            if self.hostile().is_some() {
                let mut seen = Vec::new();
                world.enemies(&mut seen);
                let nearest = seen.iter().filter(|e| e.alive && e.hostile).min_by(|a, b| a.position.distance(out.position).total_cmp(&b.position.distance(out.position))).map(|e| (e.position, e.radius));
                self.press(&out, nearest, dt, camera_yaw, ToolPad { stick: &mut stick, buttons: &mut buttons });
            }
            self.tools(sora, &status, AiLeadWorld { lead, world, members, settings }, step, ToolPad { stick: &mut stick, buttons: &mut buttons });
        }
        self.press_wait = (self.press_wait - dt).max(0.0);
        let pad = kh_core::PadState { held: buttons, pressed: PadButtons(buttons.0 & !self.held.0), released: PadButtons(self.held.0 & !buttons.0) };
        self.held = buttons;
        // (MIMIC-BOSS-KIT) a copy's Aerial Recovery (his jump in a blow's recoverable window)
        let recover = self.full_kit && self.air_recovery(sora, dt);
        Input { move_stick: stick, jump_pressed: recover, jump_held: recover, camera_yaw, pad, view: None }
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
    ///   Every command goes through his own core (`Sora::command_request`, the pad), so his gates and costs apply.
    /// - (MIMIC-BOSS-KIT) THE COPIES ([`full_kit_of`]: Mimic Sora, the boss's Sora) besides: every Y row of Sora's own kit
    ///   ([`copy_may_use`]: reaction commands too), L2 past a row that is not theirs, the Keyblade switch
    ///   ([`COPY_SWITCH_EVERY`]) and, in `drive`, Aerial Recovery. Never Summon (their cores' Summon rules are kept shut).
    fn tools(&mut self, sora: &mut kh_core::Sora, status: &kh_core::Status, AiLeadWorld { lead, world, members, settings: s }: AiLeadWorld<'_>, step: AiStep, pad: ToolPad<'_>) {
        use kh_core::command::CommandRequest;
        use kh_core::magic::spells;
        let AiStep { dt, now } = step;
        let who = self.name;
        let ToolPad { stick, buttons } = pad;
        let hostile = self.hostile(); // (MB3-AGGRO)
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
        let in_battle = sora.actions().battle || lead.in_battle;
        let mut enemies = Vec::new();
        world.enemies(&mut enemies);
        let nearest = enemies.iter().filter(|e| e.alive && e.hostile).map(|e| (e.position.distance(me), e.position)).min_by(|a, b| a.0.total_cmp(&b.0));
        // (MIMIC-BOSS-KIT) a hit broke a copy's shotlock (its damage reaction, its Aerial Recovery): the sequence is over (RB held
        // on would block its reaction command's Y: kh-core takes Y only with LB and RB up)
        if self.full_kit && self.shot.is_some() && matches!(out.state, kh_core::StateId::Damage | kh_core::StateId::AirSlide | kh_core::StateId::Die) {
            self.shot = None;
        }
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
        // (MIMIC-BOSS-KIT) a copy's Keyblade switch in battle (KH3's D-pad right on Attack; no A press with it)
        if self.full_kit && in_battle && nearest.is_some() && self.keyblade_switch(sora, dt, now) {
            buttons.set(PadButton::DPadRight, true);
            buttons.set(PadButton::ATTACK, false);
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
            for v in members.iter().filter(|v| !v.ko && v.position.distance(me) < 900.0 && low(v.hp, v.hp_max)) {
                needs.push(v.chara.name().to_string());
            }
        }
        // (party AIX) the friends' field deferral (kh-ai params::FIELD_CURE_DEFER_SPEED): no heal out of battle while the
        // leader runs
        let leader_runs = lead.velocity.truncate().length() > kh_ai::controller::params::FIELD_CURE_DEFER_SPEED;
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
                say!("party-play: t={now:.3}s {who}: {} for {} (Recovery setting {})", spells::command_name(kind), needs.join(", "), s.cure);
            }
            self.cure_wait = 4.0;
            return;
        }
        let Some((dist, _at)) = nearest.filter(|_| in_battle) else { return };
        // the situation command on offer: Second Form, a finish, a grand magic (never a team attack)
        // (MIMIC-BOSS-KIT) a copy's reaction command (a prompt's ex entry: Blow Counter after Aerial Recovery, Rage Raid) is
        // taken wherever the fight threw it (the counter goes at its attacker), within its own short window
        let reaction = self.full_kit && sora.combat().situation.current().is_some_and(|(_, kind)| sora.combat().situation.find(kind).is_some_and(|(_, e)| e.ex));
        if (self.situation_wait <= 0.0 || reaction) && (dist < 800.0 || reaction) {
            let q = &sora.combat().situation;
            if let Some((_, kind)) = q.current() {
                use kh_core::combat::situation::{class, class_of};
                // (MIMIC-BOSS-KIT) a copy: every Y command of the leader Sora's own kit (`copy_may_use`)
                let usable = if self.full_kit { copy_may_use(q, kind) } else { matches!(class_of(kind), class::FORMCHANGE | class::FINISH | class::GRAND_MAGIC) };
                if usable {
                    // (MB3-AGGRO) a boss copy lets go of Y for a step after holding it: a held Y is no new press (a reaction command
                    // offered step after step was held, never pressed again)
                    if hostile.is_some() && self.held.has(PadButton::FaceTop) {
                        return;
                    }
                    buttons.set(PadButton::FaceTop, true);
                    self.situation_wait = hostile.map_or(1.0, |a| a.sora_situation_every); // (MB3-AGGRO)
                    self.tool_busy = Some(0.0); // (party AIX)
                    say!("party-play: t={now:.3}s {who}: situation command {kind} (Y)");
                    return;
                }
                // (MIMIC-BOSS-KIT) a copy's L2 (KH3's own cycle, kh-core situation.rs `cycle`): the selected row is not his to use
                // (a team row) and another one is: the next row, Y at a later step
                let ready = |e: &kh_core::combat::situation::Entry| e.selectable && e.appear <= 0.0 && copy_may_use(q, e.kind);
                if self.full_kit && (q.list.iter().any(ready) || q.slot_a.iter().chain(q.slot_b.iter()).any(ready)) && !self.held.has(PadButton::SITUATION_NEXT) {
                    buttons.set(PadButton::SITUATION_NEXT, true);
                    say!("party-play: t={now:.3}s {who}: L2 past situation command {kind} (not his to use)");
                    return;
                }
            }
        }
        let mp_rate = if status.max_mp() > 0 { status.mp as f32 / status.max_mp() as f32 } else { 0.0 };
        // shotlock
        let shot_every = hostile.map_or([Some(15.0), Some(25.0), None][s.ability.min(2) as usize], |a| Some(a.sora_shot_every)); // (MB3-AGGRO)
        if let Some(every) = shot_every {
            if self.shot_wait <= 0.0 && (300.0..1500.0).contains(&dist) && status.focus * 2 >= status.focus_max && status.focus_max > 0 {
                self.shot = Some(0.0);
                self.tool_busy = Some(0.0); // (party AIX)
                self.shot_wait = every;
                say!("party-play: t={now:.3}s {who}: shotlock (focus {}/{})", status.focus, status.focus_max);
                return;
            }
        }
        // attack magic
        let (every, mp_min) = hostile.map_or([((4.0, 6.0), 0.0), ((7.0, 10.0), 0.5), ((14.0, 18.0), 0.75)][s.ability.min(2) as usize], |a| (a.sora_magic_every, 0.0)); // (MB3-AGGRO)
        if self.magic_wait <= 0.0 && dist < 1200.0 && !status.mp_charging && mp_rate >= mp_min {
            let list: Vec<u8> = sora.command().magic_slots.iter().copied().filter(|k| *k != 0 && spells::is_magic(*k) && !spells::is_cure(*k) && !spells::is_grand(*k)).collect();
            if !list.is_empty() {
                let kind = list[((rng() * list.len() as f32) as usize).min(list.len() - 1)];
                sora.command_request(CommandRequest::Execute(kind));
                self.tool_busy = Some(0.0); // (party AIX)
                say!("party-play: t={now:.3}s {who}: casts {} at {:.0} cm", spells::command_name(kind), dist);
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

// ---------------------------------------------------------------------------------------------- (MIMIC SORA) Mimic Sora

/// (MIMIC SORA) What one step of Mimic Sora's AI sees (the host's, every fixed step he is out).
pub struct MimicScene<'a> {
    /// The party's one Sora core: the player's Sora (the leader while no friend leads; AI Sora while one does).
    pub sora: &'a kh_core::Sora,
    /// The player's lock-on target while Sora leads (the HUD's), else NONE.
    pub lock_on: TargetId,
    /// The enemies (Elden Ring's characters: what ER's own lock-on accepts, the party's rule).
    pub targets: &'a dyn PartyTargets,
    pub camera: &'a kh_camera::CameraView,
    pub camera_yaw: f32,
    pub nav: Option<&'a dyn kh_ai::Nav>,
    pub dt: f32,
    pub now: f64,
}

/// (MIMIC SORA, THE USER 2026-11-03: "the mimic Sora can just use Sora's party member AI ... (mimic sora is unable to use
/// Summon)") MIMIC SORA'S AI: AI SORA'S OWN (Goofy's tree `n_ex002` on stream C's controller, his tools, his warp, the friends'
/// follow behind a running leader) on a second Sora core, with its own actor id ([`MIMIC_ACTOR`]) and controller. His leader
/// is whoever leads: the player's Sora (the trees see him as "the player", `SoraAiWorld`, as the friends' do while Sora leads)
/// or the friend leader (`LeaderAiWorld`, AI Sora an ally then). He is out of the formation: no camp-menu entry, no team
/// attacks (only the controlled leader starts those), never Summon (his core's Summon rules are kept shut every step). His AI
/// settings are AI Sora's (Customize's "Sora" entry).
pub struct MimicAi {
    ai: SoraAi,
    caught: CopyCaught, // (GRABS)
}

/// (GRABS) A SORA COPY'S CAUGHT GATE (Mimic Sora, the boss's copy): the party's own gate (caught.rs) on his core: while an
/// Elden Ring throw holds his body and while his core plays KH3's `CatchEnd` his tree is stopped and nothing is pressed;
/// the guards (a stale hold, a get-up past its time) are the same. The host says `set_caught` once a step.
#[derive(Clone, Debug, Default)]
pub struct CopyCaught(crate::caught::CaughtGate);

impl CopyCaught {
    pub fn note(&mut self, held: bool) {
        self.0.note(PartyChara::Sora, held, None);
    }

    /// One step of the gate; true = he does not act this step (his tree is stopped meanwhile, restarted when he is up).
    fn gate(&mut self, ai: &mut SoraAi, dt: f32) -> bool {
        self.0.tick(dt, |_| crate::caught::Now { id: None, ko: false, in_motion: false });
        let caught = self.0.stops(PartyChara::Sora);
        if let Some(ctl) = ai.ctl.as_mut() {
            ctl.set_stop_ai(caught);
        }
        caught
    }

    pub fn stops(&self) -> bool {
        self.0.stops(PartyChara::Sora)
    }
}

/// (GRABS) A caught core's step input: nothing pressed, no step (the camera's yaw kept).
fn caught_input(input: Input) -> Input {
    Input { move_stick: Vec2::ZERO, jump_pressed: false, jump_held: false, pad: kh_core::PadState::default(), ..input }
}

impl MimicAi {
    pub fn new(rt: &PartyRuntime) -> MimicAi {
        MimicAi { ai: SoraAi::with_actor(rt, MIMIC_ACTOR, "Mimic Sora", 0x3D_1A_7E), caught: CopyCaught::default() }
    }

    /// (GRABS) Once a step, before `input`: his Elden Ring body is held by a throw now.
    pub fn set_caught(&mut self, held: bool) {
        self.caught.note(held);
    }

    /// (GRABS) Caught or getting up.
    pub fn caught(&self) -> bool {
        self.caught.stops()
    }

    /// His AI has the tree pack (false: he only follows and warps, as AI Sora without one).
    pub fn has_tree(&self) -> bool {
        self.ai.ctl.is_some()
    }

    /// ONE STEP: the input of the mimic's core (`mimic`) for this fixed step; his follow cap and speed scale set on it.
    pub fn input(&mut self, rt: &PartyRuntime, mimic: &mut kh_core::Sora, scene: MimicScene<'_>) -> Input {
        let MimicScene { sora, lock_on, targets, camera, camera_yaw, nav, dt, now } = scene;
        let caught = self.caught.gate(&mut self.ai, dt); // (GRABS)
        mimic.set_follow_cap(None);
        mimic.combat_mut().friend.sora_is_ai = true; // (no unison of his own: the team attacks are the leader's)
        if crate::mimic::keep_summon_shut(mimic) {
            say!("party-play: t={now:.3}s Mimic Sora's Summon rules were opened by someone: shut again (the user's rule: no Summon)");
        }
        let sora_out = sora.output();
        let sora_status = sora.status().clone();
        let out = mimic.output();
        let status = mimic.status().clone();
        let friend = rt.leader.and_then(|l| rt.current.iter().find(|v| v.id == l));
        let lock = friend.map_or(lock_on, |_| rt.leader_lock().unwrap_or(TargetId::NONE));
        let lead_pos = friend.map_or(sora_out.position, |v| v.position);
        let enemies = {
            let mut points = rt.threat_points(lead_pos, &sora_out);
            points.push(out.position);
            let mut feed = EnemyFeed::of_party(targets, &points, lock);
            feed.reserve(&rt.party.don_zetta_off_limit_targets(targets)); // (ZETTA round 5c) as AI Sora's
            feed
        };
        let nav = nav.unwrap_or(&STRAIGHT_NAV);
        let scene = AiScene { enemies: &enemies, camera, camera_yaw, nav };
        let step = AiStep { dt, now };
        let mps = rt.member_mps();
        let in_battle = mimic.actions().battle;
        let settings = rt.lead.sora_settings;
        let (input, lead) = match friend {
            Some(leader) => {
                let world = LeaderAiWorld { leader, lock_on: lock, in_battle: in_battle || leader.in_battle, members: &rt.current, sora: rt.lead.sora_active.then_some((&sora_out, &sora_status)), enemies: &enemies, camera: rt.ai_camera(camera), nav, game_level: status.game_level as u8, mps: &mps };
                let lead = Lead::of_member(leader);
                (self.ai.drive(rt, mimic, step, scene, AiLeadWorld { lead, world: &world, members: &rt.current, settings }), lead)
            }
            None => {
                let battle = sora.actions().battle;
                let world = SoraAiWorld { sora: &sora_out, status: &sora_status, in_battle: battle || in_battle, attacking: sora_out.state == kh_core::StateId::Attack, lock_on: lock, enemies: &enemies, members: &rt.current, camera: rt.ai_camera(camera), nav, game_level: status.game_level as u8, mps: &mps };
                let lead = Lead { position: sora_out.position, yaw: sora_out.yaw, velocity: sora_out.velocity, in_battle: battle, name: "Sora" };
                (self.ai.drive(rt, mimic, step, scene, AiLeadWorld { lead, world: &world, members: &rt.current, settings }), lead)
            }
        };
        // (party K1) the friends' catch-up speed behind the leader (1 in battle), x Donald's Thunder aura as AI Sora's
        let reference = friend.and_then(|v| rt.party.member(v.id)).map_or(kh_core::SORA_PARAMS.speed_run, |m| m.cmc.max_walk_speed.max(m.params.speed_run));
        let scale = if mimic.actions().battle { 1.0 } else { follow_speed_scale(lead.position, lead.velocity, reference, &out) };
        mimic.set_speed_scale(scale * kh_party::pad::donald::sora_move_mult(&rt.party, out.position));
        // never a team attack of his (the leader starts them): (MIMIC-BOSS-KIT) his own Y for every command of Sora's own kit
        // (tools, `copy_may_use`), the Keyblade switch, Aerial Recovery
        let input = Input { view: None, camera_yaw, ..input };
        if caught { caught_input(input) } else { input } // (GRABS)
    }
}

// ---------------------------------------------------------------------------------------------- (MIMIC-BOSS) the boss's Sora

/// (MIMIC-BOSS) The boss copy's actor id among the trees' actors (its own controller, never AI Sora's or Mimic Sora's).
pub const BOSS_ACTOR: u8 = 9;

/// (MIMIC-BOSS) What one step of a boss copy's AI sees: the party as its enemies (crate::mimicboss::BossTargets), the drawn
/// camera, the navigation.
pub struct BossScene<'a> {
    pub targets: &'a dyn PartyTargets,
    pub camera: &'a kh_camera::CameraView,
    pub camera_yaw: f32,
    pub nav: Option<&'a dyn kh_ai::Nav>,
    pub dt: f32,
    pub now: f64,
}

/// (MIMIC-BOSS, THE USER 2026-11-05: "it needs to mimic whoever is party leader and actually fight the party") A SORA COPY'S
/// HOSTILE AI: AI SORA'S OWN (Goofy's tree `n_ex002` on stream C's controller, his tools: combos, magic, shotlock, the
/// situation commands of his kit, guard and dodge as the tree drives them) on the boss's core with its own actor id
/// ([`BOSS_ACTOR`]), turned round: the party is its only enemy list, it is its own "player" (nobody to follow: the tree's
/// formation goals are its own feet, its warp never fires), always in battle. Its settings: AI Sora's own (Customize's
/// "Sora" entry) with the battle style Free and its cures on itself only. Never Summon (its core's rules are kept shut; the
/// boss's summon is the host's, mimicboss.rs `summon_due`), never a team attack.
pub struct BossAi {
    ai: SoraAi,
    caught: CopyCaught, // (GRABS)
}

impl BossAi {
    pub fn new(rt: &PartyRuntime) -> BossAi {
        BossAi { ai: SoraAi::with_actor(rt, BOSS_ACTOR, "the Mimic Tear", 0x4D_42_05), caught: CopyCaught::default() }
    }

    /// (GRABS) Once a step, before `input`: the copy's Elden Ring body is held by a throw now.
    pub fn set_caught(&mut self, held: bool) {
        self.caught.note(held);
    }

    /// (GRABS) Caught or getting up.
    pub fn caught(&self) -> bool {
        self.caught.stops()
    }

    /// Its AI has the tree pack (false: it stands; the host logs it).
    pub fn has_tree(&self) -> bool {
        self.ai.ctl.is_some()
    }

    /// ONE STEP: the input of the boss's core for this fixed step.
    pub fn input(&mut self, rt: &PartyRuntime, boss: &mut kh_core::Sora, scene: BossScene<'_>) -> Input {
        let BossScene { targets, camera, camera_yaw, nav, dt, now } = scene;
        let caught = self.caught.gate(&mut self.ai, dt); // (GRABS)
        boss.set_follow_cap(None);
        boss.set_speed_scale(1.0);
        boss.combat_mut().friend.sora_is_ai = true;
        if crate::mimic::keep_summon_shut(boss) {
            say!("party-play: t={now:.3}s the Mimic Tear's own Summon rules were opened by someone: shut again (its summon is the host's)");
        }
        boss.actions_mut().battle = true;
        boss.actions_mut().battle_forced = true;
        let out = boss.output();
        let status = boss.status().clone();
        let enemies = EnemyFeed::of(targets);
        let nav = nav.unwrap_or(&STRAIGHT_NAV);
        let world = SoraAiWorld { sora: &out, status: &status, in_battle: true, attacking: out.state == kh_core::StateId::Attack, lock_on: TargetId::NONE, enemies: &enemies, members: &[], camera: rt.ai_camera(camera), nav, game_level: status.game_level as u8, mps: &[] };
        let lead = Lead { position: out.position, yaw: out.yaw, velocity: out.velocity, in_battle: true, name: "the Mimic Tear" };
        let settings = FriendAiSettings { battle: 1, cure_targets: 3, ..rt.lead.sora_settings };
        // (MB3-AGGRO) a hostile boss copy's Abilities setting: Go Wild (the real setting that fights hardest)
        let settings = if self.ai.hostile().is_some() { FriendAiSettings { ability: 0, ..settings } } else { settings };
        let input = self.ai.drive(rt, boss, AiStep { dt, now }, AiScene { enemies: &enemies, camera, camera_yaw, nav }, AiLeadWorld { lead, world: &world, members: &[], settings });
        let input = Input { view: None, camera_yaw, ..input };
        if caught { caught_input(input) } else { input } // (GRABS)
    }
}

impl BossAi {
    /// (MIMIC-BOSS hotfix 3, MB3-AGGRO) Its hostile tuning (mimicboss.rs `CopyAggro`, the sheet's `aggro.*` rows).
    pub fn with_aggro(mut self, a: crate::mimicboss::CopyAggro) -> BossAi {
        self.set_aggro(a);
        self
    }

    /// (MB3-AGGRO) The same on a running copy.
    pub fn set_aggro(&mut self, a: crate::mimicboss::CopyAggro) {
        self.ai.aggro = Some(a);
        self.ai.switch_wait = self.ai.switch_wait.min(self.ai.switch_every().0);
    }
}

/// (party P) the leader's members by view (a convenience for hosts): the leading member's view after the last step.
pub fn leader_member(rt: &PartyRuntime) -> Option<&MemberView> {
    rt.leader.and_then(|l| rt.current.iter().find(|v| v.id == l))
}

#[cfg(test)]
mod chestfix_tests {
    use super::*;

    /// (CHESTFIX, the user 2026-10-27: "he also keeps trying to run in front of Donald ... he should just follow like Donald
    /// and Goofy do") Behind a leader running +X at 650 cm/s: a spot of the tree's ahead of or beside him becomes the spot
    /// 300 cm behind him; level with or past it AI Sora goes along the leader's way at most at the leader's speed plus the
    /// gap (ahead of the leader: half his speed, so the leader passes him); far behind he closes at a capped catch-up
    /// speed; in battle or behind a standing leader the rule does not apply.
    #[test]
    fn ai_sora_follows_behind_like_the_friends() {
        let leader = Vec3::new(1000.0, 0.0, 0.0);
        let v = Vec3::new(650.0, 0.0, 0.0);
        // the tree's spot 200 cm AHEAD of the leader: moved behind him, the sideways offset kept
        let w = sora_follow_way(Vec3::new(650.0, 100.0, 0.0), Some(Vec3::new(1200.0, 150.0, 0.0)), leader, v, false, true).unwrap();
        assert!(w.moved_back);
        // AI Sora 350 cm behind the leader, his goal 300 behind: level with it -> along the way, the leader's speed + the gap
        assert!(w.aim.x > 650.0 && w.cap <= 650.0 + 50.0 / kh_party::locomotion::FOL2_GAP_TIME + 1.0, "{w:?}");
        // 100 cm short of the spot: KH3's follow rule (x 400 / 300, at most x 1.1 in view)
        let near = sora_follow_way(Vec3::new(600.0, 150.0, 0.0), Some(Vec3::new(1200.0, 150.0, 0.0)), leader, v, false, true).unwrap();
        assert!((near.cap - 650.0 * 1.1).abs() < 1.0, "{near:?}");
        // AI Sora AHEAD of the leader: half the leader's speed, along his way (never turned back, never faster than him)
        let ahead = sora_follow_way(Vec3::new(1300.0, 0.0, 0.0), None, leader, v, false, true).unwrap();
        assert!((ahead.cap - 325.0).abs() < 1.0 && ahead.aim.x > 1300.0, "{ahead:?}");
        // far behind (30 m): toward the spot, faster than the leader but capped
        let far = sora_follow_way(Vec3::new(-2000.0, 0.0, 0.0), Some(Vec3::new(500.0, 0.0, 0.0)), leader, v, false, false).unwrap();
        assert!(!far.moved_back && far.cap > 650.0 * 2.0 && far.cap <= kh_party::locomotion::K.k1_speed_cap && (far.aim - Vec3::new(500.0, 0.0, 0.0)).length() < 1.0, "{far:?}");
        // in battle, or the leader standing / walking: not this rule
        assert!(sora_follow_way(Vec3::ZERO, None, leader, v, true, true).is_none());
        assert!(sora_follow_way(Vec3::ZERO, None, leader, Vec3::new(100.0, 0.0, 0.0), false, true).is_none());
    }
}

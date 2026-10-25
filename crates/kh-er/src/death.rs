//! (DTH) ONE DEATH FOR WHOEVER LEADS. The leader's death (Sora, Donald, Goofy or any playable character added later) takes
//! ONE path from the moment he goes down to the moment the game has its player again. Nothing here knows who he is: the
//! flow is generic over the leader's identity (`W`), and its only facts are "the leader of this frame", "is he alive" and
//! "is this a scripted death's place". State note: build/sora_tmp/er/IMPL_DTH_STATE.md.
//!
//! THE USER'S BUG (2026-10-16, Goofy leading at the Chapel of Anticipation's tutorial boss): Goofy froze, Elden Ring's own
//! "YOU DIED" came, and the game-over menu then came up at the Stranded Graveyard instead of the scene there. Causes:
//! 1. the leader's KO was handed to the game THE SAME FRAME (the hidden body's HP written to 0); from then on the frame took
//!    the dead path, which stepped only Sora's own core: a friend leader's KO never advanced (his last pose stayed drawn);
//! 2. HP 0 is a REAL death for the game (its death sound, voice and banner), also for Sora; the tutorial's event (m10_01
//!    EVENT 10010030) waits for the player AT 1 HP (`IfCharacterHPValue(10000, ==, 1)` with the boss alive), and then fades
//!    out and plays cutscene 10010000 to m18; a real death ENDS that event instead (`EndIf` its "player dead" group);
//! 3. the party host carries over to the next Sora host: the friend leader was still KO'd (HP 0, Die state) there, so the
//!    HUD's "fallen leader" opened the game-over menu at once at the Stranded Graveyard, before its scene could play;
//! 4. the HUD's game over was keyed on Sora's own state (`out.state == Die`): AI Sora knocked out while a friend leads was a
//!    game over too.
//!
//! THE RULES:
//! - DOWN: the frame's leader is not alive (Sora: his HP; a friend: his KO). Latched once per Sora host, with its kind:
//!   SCRIPTED at a scripted death's place (sheet rows `death.scripted.*`; the only one in the game's 586 event files is the
//!   tutorial boss), NORMAL elsewhere.
//! - NORMAL: KH3's game-over menu for him (the HUD's hold), else (`gameover.kh = 0`, the menu answered by Retry) the body's
//!   HP 0 = the game's own death, as before.
//! - SCRIPTED: the leader plays his own KO (`ko_show` s), the picture fades to black (`fade` s), then the body's HP is set to
//!   the event's value (1) with the game's no-death switch held: no death of the game's (no death sound, no voice, no YOU
//!   DIED), the game's own event fades and plays its scene. The black stays until the scene plays (or the body is put
//!   somewhere else). If nothing comes of it within `wait` s, the game's own death is given (HP 0) as before.
//! - AFTER IT (a respawn, a load, fast travel = a new Sora host): every member stands again, full (`revive_all`), as Sora
//!   does (save.rs: a new host starts with full gauges).

use std::fmt::Debug;

/// IS THE FRAME'S LEADER ALIVE. A friend leading (`friend_alive` = his own "not knocked out"): his KO decides, AI Sora's
/// does not. Sora leading (`None`): his HP and his own death event of the frame.
pub fn leader_alive(friend_alive: Option<bool>, sora_hp: i32, sora_fell: bool) -> bool {
    match friend_alive {
        Some(alive) => alive,
        None => sora_hp > 0 && !sora_fell,
    }
}

/// How a death is carried out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// KH3's game-over menu (or the game's own death when the menu is off).
    Normal,
    /// The game's event takes it from the leader's KO (the tutorial boss): no death of the game's.
    Scripted,
}

/// The numbers of the flow (keys `death.ko_show`, `death.fade`, `death.wait`, `death.scripted_hp`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rules {
    /// Seconds of the leader's own KO before the fade begins (scripted).
    pub ko_show: f32,
    /// Seconds of the fade to black.
    pub fade: f32,
    /// Seconds the black waits for the game's scene before the game's own death is given instead.
    pub wait: f32,
    /// The body's HP the scripted event waits for (m10_01 EVENT 10010030: `== 1`).
    pub hp: i32,
    /// `death.scripted = 0`: every death is a normal one (the old rule at the tutorial boss: HP 0 at once, no menu).
    pub scripted: bool,
}

impl Rules {
    pub const DEFAULT: Rules = Rules { ko_show: 1.6, fade: 0.8, wait: 12.0, hp: 1, scripted: true };
}

impl Default for Rules {
    fn default() -> Rules {
        Rules::DEFAULT
    }
}

/// A scripted death that fell back to the game's own death keeps the black this long at most (s).
pub const GAVE_UP_BLACK: f32 = 20.0;

/// Where the flow is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase<W> {
    Alive,
    /// The leader is down; the body is still ours (unhittable, topped up). `t`: seconds since.
    Down { who: W, kind: Kind, t: f32 },
    /// The death was handed to the game. `t`: seconds since; `cut`: the game's scene has begun (scripted).
    Sent { who: W, kind: Kind, t: f32, cut: bool },
    /// Scripted, but nothing came of it in `wait` s: the game's own death was given. `t`: seconds since.
    GaveUp { who: W, t: f32 },
}

/// What is done with the hidden body this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
    /// Nothing of the flow's: the host's own rule (alive: topped up; dead path: nothing written).
    Keep,
    /// The leader is down, nothing handed over yet: unhittable, topped up.
    Hold,
    /// HP 0: the game's own death.
    Kill,
    /// HP set to this value (the scripted event's), the no-death switch held: no death of the game's.
    Spare(i32),
}

/// The flow's orders for one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Order<W> {
    pub body: Body,
    /// The black plate's opacity (0 = none).
    pub fade: f32,
    /// The game's "player cannot die" switch must be held.
    pub hold_no_dead: bool,
    /// KH3's game-over menu is wanted for him (the HUD opens it when `gameover.kh` is on).
    pub gameover: Option<W>,
}

impl<W> Order<W> {
    fn keep() -> Order<W> {
        Order { body: Body::Keep, fade: 0.0, hold_no_dead: false, gameover: None }
    }
}

/// THE FLOW (one per Sora host).
#[derive(Clone, Debug)]
pub struct Flow<W> {
    pub phase: Phase<W>,
    pub rules: Rules,
}

impl<W: Copy + PartialEq + Debug> Default for Flow<W> {
    fn default() -> Flow<W> {
        Flow { phase: Phase::Alive, rules: Rules::DEFAULT }
    }
}

impl<W: Copy + PartialEq + Debug> Flow<W> {
    pub fn new(rules: Rules) -> Flow<W> {
        Flow { phase: Phase::Alive, rules }
    }

    /// ONCE A FRAME after the steps, before the HUD: the frame's leader, whether he is alive, whether this is a scripted
    /// death's place. Returns the leader and the kind on the frame he goes down (latched: a later revival of the KO'd
    /// character, another leader, the place changing do not undo it).
    pub fn observe(&mut self, leader: W, alive: bool, scripted_place: bool) -> Option<(W, Kind)> {
        if self.phase != Phase::Alive || alive {
            return None;
        }
        let kind = if scripted_place && self.rules.scripted { Kind::Scripted } else { Kind::Normal };
        self.phase = Phase::Down { who: leader, kind, t: 0.0 };
        Some((leader, kind))
    }

    /// The leader who went down (any phase after it), None while he lives.
    pub fn down(&self) -> Option<W> {
        match self.phase {
            Phase::Alive => None,
            Phase::Down { who, .. } | Phase::Sent { who, .. } | Phase::GaveUp { who, .. } => Some(who),
        }
    }

    pub fn kind(&self) -> Option<Kind> {
        match self.phase {
            Phase::Down { kind, .. } | Phase::Sent { kind, .. } => Some(kind),
            Phase::GaveUp { .. } => Some(Kind::Scripted),
            Phase::Alive => None,
        }
    }

    /// The death was handed to the game.
    pub fn sent(&self) -> bool {
        matches!(self.phase, Phase::Sent { .. } | Phase::GaveUp { .. })
    }

    /// KH3's game-over menu is wanted for this leader (a normal death; never a scripted one).
    pub fn gameover(&self) -> Option<W> {
        match self.phase {
            Phase::Down { who, kind: Kind::Normal, .. } | Phase::Sent { who, kind: Kind::Normal, .. } => Some(who),
            _ => None,
        }
    }

    /// The game's no-death switch is held by the flow (a scripted death until its scene).
    pub fn holds_no_dead(&self) -> bool {
        matches!(self.phase, Phase::Down { kind: Kind::Scripted, .. } | Phase::Sent { kind: Kind::Scripted, .. })
    }

    fn fade_of(&self, t: f32) -> f32 {
        if self.rules.fade <= 0.0 {
            return if t >= self.rules.ko_show { 1.0 } else { 0.0 };
        }
        ((t - self.rules.ko_show) / self.rules.fade).clamp(0.0, 1.0)
    }

    /// THE BODY IS STILL OURS (sora.rs `after_steps`, once a frame): `gameover_holds` = KH3's game-over menu holds the
    /// death (unanswered).
    pub fn live_step(&mut self, dt: f32, gameover_holds: bool) -> Order<W> {
        match self.phase {
            Phase::Alive => Order::keep(),
            Phase::Down { who, kind: Kind::Normal, .. } => {
                if gameover_holds {
                    Order { body: Body::Hold, fade: 0.0, hold_no_dead: false, gameover: Some(who) }
                } else {
                    self.phase = Phase::Sent { who, kind: Kind::Normal, t: 0.0, cut: false };
                    Order { body: Body::Kill, fade: 0.0, hold_no_dead: false, gameover: Some(who) }
                }
            }
            Phase::Down { who, kind: Kind::Scripted, t } => {
                let t = t + dt.max(0.0);
                let fade = self.fade_of(t);
                if t >= self.rules.ko_show + self.rules.fade.max(0.0) {
                    self.phase = Phase::Sent { who, kind: Kind::Scripted, t: 0.0, cut: false };
                    Order { body: Body::Spare(self.rules.hp), fade: 1.0, hold_no_dead: true, gameover: None }
                } else {
                    self.phase = Phase::Down { who, kind: Kind::Scripted, t };
                    Order { body: Body::Hold, fade, hold_no_dead: true, gameover: None }
                }
            }
            // handed over: the dead path's (`dead_step`); a frame of the live path meanwhile writes nothing of ours
            Phase::Sent { kind: Kind::Scripted, cut, .. } => Order { body: Body::Keep, fade: if cut { 0.0 } else { 1.0 }, hold_no_dead: true, gameover: None },
            Phase::Sent { .. } | Phase::GaveUp { .. } => Order::keep(),
        }
    }

    /// THE BODY IS THE GAME'S (frame.rs' dead path, once a frame): `cutscene` = the game's cutscene manager plays a scene.
    pub fn dead_step(&mut self, dt: f32, cutscene: bool) -> Order<W> {
        let dt = dt.max(0.0);
        match self.phase {
            Phase::Sent { who, kind: Kind::Scripted, t, cut } => {
                let t = t + dt;
                let cut = cut || cutscene;
                if !cut && t > self.rules.wait {
                    self.phase = Phase::GaveUp { who, t: 0.0 };
                    return Order { body: Body::Kill, fade: 1.0, hold_no_dead: false, gameover: None };
                }
                self.phase = Phase::Sent { who, kind: Kind::Scripted, t, cut };
                Order { body: Body::Keep, fade: if cut { 0.0 } else { 1.0 }, hold_no_dead: true, gameover: None }
            }
            // the game's own death runs under the black until the next host takes over (its respawn); lifted after
            // `GAVE_UP_BLACK` s at the latest (nothing may keep the screen black for good)
            Phase::GaveUp { who, t } => {
                let t = t + dt;
                self.phase = Phase::GaveUp { who, t };
                Order { body: Body::Keep, fade: (1.0 - (t - GAVE_UP_BLACK) / 0.5).clamp(0.0, 1.0), hold_no_dead: false, gameover: None }
            }
            Phase::Sent { who, kind: Kind::Normal, t, cut } => {
                self.phase = Phase::Sent { who, kind: Kind::Normal, t: t + dt, cut };
                Order::keep()
            }
            // the game's own death without one of ours (deep water, a kill volume the switch did not hold): as before
            Phase::Alive | Phase::Down { .. } => Order::keep(),
        }
    }
}

// ------------------------------------------------------------------------------------------ the party after a death

/// A party whose members can be stood up again (the party runtime; a mock in the tests).
pub trait Revive<W> {
    /// Everyone in the party now (the leader too).
    fn members(&self) -> Vec<W>;
    /// HP / MP full, a KO ended at once (no stand-up clip: the new host's first frame shows him standing). True when he
    /// was down.
    fn revive(&mut self, who: W) -> bool;
}

/// A NEW SORA HOST (a respawn, a load, fast travel): every member full and standing. Returns those who were down.
pub fn revive_all<W: Copy, P: Revive<W> + ?Sized>(party: &mut P) -> Vec<W> {
    party.members().into_iter().filter(|w| party.revive(*w)).collect()
}

#[cfg(feature = "party")]
impl Revive<kh_host::party::PartyChara> for kh_partyhost::PartyRuntime {
    fn members(&self) -> Vec<kh_host::party::PartyChara> {
        self.members.iter().map(|m| m.0).collect()
    }

    fn revive(&mut self, who: kh_host::party::PartyChara) -> bool {
        let Some(id) = self.id(who) else { return false };
        let Some(pawn) = self.party.member_mut(id) else { return false };
        let was = pawn.state == kh_party::NpcStateId::Die || pawn.status.hp < 1;
        let (hp, mp) = (pawn.status.max_hp(), pawn.status.max_mp());
        pawn.status.hp = hp;
        pawn.status.mp = mp;
        pawn.status.mp_charge_left = 0.0;
        if pawn.state == kh_party::NpcStateId::Die {
            pawn.to_idle();
        }
        // the views the leader routing / the screens read until the next step
        if let Some(view) = self.party.view(id) {
            for v in self.current.iter_mut().chain(self.previous.iter_mut()) {
                if v.id == view.id {
                    *v = view;
                }
            }
        }
        if self.lead.leader_state.0 == who {
            self.lead.leader_state.1 = false;
        }
        was
    }
}

// ------------------------------------------------------------------------------------------ the black plate

#[cfg(windows)]
static PLATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// THE FADE TO BLACK (the overlay's plate over everything, as the game-over and the story's covers use it; it lapses by
/// itself 300 ms after the last call, so nothing can leave the screen covered). Called every frame with the flow's value.
#[cfg(windows)]
pub fn plate(alpha: f32) {
    use std::sync::atomic::Ordering::Relaxed;
    if alpha > 0.001 {
        kh_er_render::hud::set_plate(Some((kh_er_render::hud::MAX_LAYERS, [0.0, 0.0, 0.0, alpha.min(1.0)], 300)));
        PLATE.store(true, Relaxed);
    } else if PLATE.swap(false, Relaxed) {
        kh_er_render::hud::set_plate(None);
    }
}

#[cfg(test)]
#[path = "death_tests.rs"]
mod tests;

//! (IMPL_ER23A) Small pure rules of the Elden Ring host, each with its unit tests (no game types here):
//!
//! - `PressLatch`: WHO HAS A BUTTON PRESS. A button that is down while somebody else has the pad (a menu of Elden Ring's,
//!   a menu of ours, the game's own action of the body) is not Sora's until it has been released: the B that closes the
//!   save menu is not his jump, the A that picks a line of a dialogue is not his attack.
//! - `DialogWatch`: a selection window of Elden Ring's that leaves its HUD state at "gameplay" (an NPC's talk list, a
//!   choice box): the whole pad is the game's while it is up.
//! - `FallGuard`: THE HIDDEN BODY NEVER DIES OF A FALL. What the game takes off its body while Sora is on a long drop or
//!   has just landed from one is the game's fall damage: none of Sora's business (KH3 has no fall damage).
//! - `Stuck`: off the floor, not moving, with a falling speed that says he should: wedged. The host resolves it.

use glam::Vec3;
use kh_core::PadButtons;

// ---------------------------------------------------------------------------------------------- press ownership

/// Buttons that went down (or were down) while Sora did not have the pad: not his until released.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PressLatch {
    blocked: u32,
}

impl PressLatch {
    /// One frame. `raw`: the buttons physically down; `foreign`: somebody else has the pad this frame; `held`: what
    /// the rules before this one would give Sora. Returns what he gets.
    pub fn filter(&mut self, raw: PadButtons, held: PadButtons, foreign: bool) -> PadButtons {
        if foreign {
            self.blocked |= raw.0;
        }
        self.blocked &= raw.0;
        PadButtons(held.0 & !self.blocked)
    }

    /// The buttons still kept from him (for the log).
    pub fn blocked(&self) -> PadButtons {
        PadButtons(self.blocked)
    }
}

// ---------------------------------------------------------------------------------------------- Elden Ring's selection windows

/// What is read of Elden Ring's popup-menu object once a frame (SDK `CSPopupMenu`: `current_top_menu_job` at 0xb0,
/// `current_talk_id` at 0x180; the meaning of both is the SDK's naming, NOT measured by us).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PopupState {
    pub top_job: usize,
    pub talk_id: i32,
}

/// The rule, made safe against a field that does not mean what its name says: a window is "up" only when the top job
/// was seen EMPTY in this session before (a field that is never empty never takes the pad from Sora), and a window that
/// stays for longer than `LIMIT` seconds is no window (said in the log by the caller; the rule is off for the session).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DialogWatch {
    seen_empty: bool,
    up_for: f32,
    pub given_up: bool,
}

impl DialogWatch {
    pub const LIMIT: f32 = 180.0;

    /// True while a selection window of the game's has the pad.
    pub fn frame(&mut self, state: PopupState, dt: f32) -> bool {
        if state.top_job == 0 {
            self.seen_empty = true;
            self.up_for = 0.0;
            return false;
        }
        if !self.seen_empty || self.given_up {
            return false;
        }
        self.up_for += dt;
        if self.up_for > Self::LIMIT {
            self.given_up = true;
            return false;
        }
        true
    }
}

// ---------------------------------------------------------------------------------------------- the fall guard

/// A drop this deep (below the floor he left) or deeper is "a fall" for the game's damage, cm. (Elden Ring takes no
/// fall damage under about 8 m; half of that, so that the body's own later landing is always inside the window.)
pub const FALL_MIN_DROP: f32 = 400.0;
/// How long after his landing the body's own landing (gravity comes back on when he stands) may still cost HP, s.
/// (MEASURED in the user's log: the body died 5 frames after his landing.)
pub const FALL_WINDOW: f32 = 0.75;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FallGuard {
    floor_z: Option<f32>,
    /// The deepest he has been below the floor he left, while in the air, cm.
    drop: f32,
    /// Seconds since he landed from a drop of `FALL_MIN_DROP` or more (`None`: no such landing is recent).
    landed: Option<f32>,
    /// HP of the body's the game took in fall windows, discarded (for the log).
    pub discarded: i64,
}

impl FallGuard {
    /// Once a frame, from Sora's own state (KH3 cm, Z up).
    pub fn frame(&mut self, on_floor: bool, z: f32, dt: f32) {
        if on_floor {
            if self.drop >= FALL_MIN_DROP {
                self.landed = Some(0.0);
            } else if let Some(t) = self.landed.as_mut() {
                *t += dt;
                if *t > FALL_WINDOW {
                    self.landed = None;
                }
            }
            self.floor_z = Some(z);
            self.drop = 0.0;
        } else {
            let from = *self.floor_z.get_or_insert(z);
            self.drop = self.drop.max(from - z);
        }
    }

    /// The frame was rebased / he was put somewhere else: heights before it mean nothing.
    pub fn reset(&mut self) {
        *self = FallGuard { discarded: self.discarded, ..FallGuard::default() };
    }

    /// HP the game takes off its body NOW is its fall damage (or its fall death): Sora takes none.
    pub fn in_window(&self, on_floor: bool) -> bool {
        if on_floor { self.landed.is_some() } else { self.drop >= FALL_MIN_DROP }
    }

    pub fn drop(&self) -> f32 {
        self.drop
    }
}

// ---------------------------------------------------------------------------------------------- a body that hovers above its capsule

/// THE CAPSULE OF A HOVERING ENEMY. Elden Ring keeps the position node (and the capsule) of some flyers on the ground under
/// them and lifts the MODEL by animation (the user: "enemies like bugs ... target below them while flying (though other
/// flying enemies don't suffer from this)": those have their node at the body). `span`: where the model's bones are above
/// the position node, cm: (the lower tenth, the upper tenth), from the bone matrices; `None` = not readable. Returns how
/// far the capsule must go up so that its centre is the middle of the body: 0 for everything that stands on its node (a
/// tall ground enemy's legs reach down to it) and for a flyer whose node is at its body.
pub fn hover_lift(half_height: f32, span: Option<(f32, f32)>, hover_min: f32) -> f32 {
    let Some((low, high)) = span else { return 0.0 };
    let tall = high - low;
    if !(low.is_finite() && high.is_finite()) || tall < 5.0 || tall > 3000.0 || low > 3000.0 {
        return 0.0;
    }
    if low <= hover_min.max(0.5 * tall) {
        return 0.0;
    }
    ((low + high) * 0.5 - half_height).max(0.0)
}

/// How far above the (lifted) feet the point is that Sora's attacks aim at: the capsule's centre, capped at `max_height`
/// for what stands on the ground (IMPL_ER13: a troll's centre is metres up and KH3 would answer with air combos), NEVER
/// capped for a body that hovers (the cap would drag the point down out of it).
pub fn aim_height(half_height: f32, max_height: f32, lift: f32) -> f32 {
    if lift > 0.0 { half_height } else { half_height.min(max_height) }
}

// ---------------------------------------------------------------------------------------------- stuck

/// Steps of "off the floor, falling speed, no movement" after which he counts as wedged (0.4 s).
pub const STUCK_STEPS: u32 = 24;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stuck {
    last: Option<Vec3>,
    steps: u32,
    /// How often it fired in this session.
    pub count: u32,
}

impl Stuck {
    /// One simulation step. True on the step he has been wedged for `STUCK_STEPS`: off the floor, the movement code's
    /// own speed downward above 1.5 m/s (gravity has been pulling for a while: a hover of an attack or an air slide has
    /// none), and less than half a centimetre moved in the step.
    pub fn step(&mut self, on_floor: bool, position: Vec3, velocity: Vec3) -> bool {
        let moved = self.last.replace(position).map_or(f32::MAX, |p| (position - p).length());
        if on_floor || velocity.z > -150.0 || moved > 0.5 {
            self.steps = 0;
            return false;
        }
        self.steps += 1;
        if self.steps == STUCK_STEPS {
            self.steps = 0;
            self.count += 1;
            return true;
        }
        false
    }
}

// ---------------------------------------------------------------------------------------------- (IMPL_ER24) the nets' rules

/// Frames on the floor in a row after which a place is one "where he last stood" (a landing on an edge he slides off
/// in the next frames is not).
pub const ANCHOR_FRAMES: u32 = 12;
/// The fall net acts only on a real fall: this far below where he left the floor, cm.
pub const NET_MIN_DROP: f32 = 300.0;
/// ... and calls the second fall from the same place his death only this far down, cm.
pub const NET_DEATH_DROP: f32 = 1000.0;

/// Where he last STOOD (for `ANCHOR_FRAMES` in a row) and the height at which he last had a floor at all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Anchor {
    run: u32,
    pub at: Option<Vec3>,
    /// His height in the last frame he was on a floor.
    pub left_z: Option<f32>,
}

impl Anchor {
    pub fn frame(&mut self, on_floor: bool, position: Vec3) {
        if on_floor {
            self.run += 1;
            self.left_z = Some(position.z);
            if self.run >= ANCHOR_FRAMES {
                self.at = Some(position);
            }
        } else {
            self.run = 0;
        }
    }

    /// The game seated him here (a warp, the end of a cutscene, a put-back).
    pub fn seat(&mut self, position: Vec3) {
        self.at = Some(position);
        self.left_z = Some(position.z);
    }

    /// How far he is below the height at which he last had a floor.
    pub fn dropped(&self, z: f32) -> f32 {
        self.left_z.map_or(0.0, |left| left - z)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetVerdict {
    Nothing,
    PutBack,
    Death,
}

/// THE FALL NET'S RULE. `void_for`: seconds without any ground under him; `dropped`: how far below the height of his
/// last floor he is; `again`: he was put back to this very anchor a moment ago. A body that does not fall (wedged on a
/// plank's edge over a hole, hovering in an attack, sliding through the air) is never put back and never killed: MEASURED
/// in the user's log (879.8 s: wedged 6 cm from where he stood, the line under him going through a hole: put back, and
/// 1.6 s later "the fall is his death").
pub fn net_verdict(void_for: f32, dropped: f32, again: bool) -> NetVerdict {
    if void_for < 1.5 || dropped < NET_MIN_DROP {
        NetVerdict::Nothing
    } else if !again {
        NetVerdict::PutBack
    } else if dropped >= NET_DEATH_DROP {
        NetVerdict::Death
    } else {
        NetVerdict::Nothing
    }
}

/// WHERE THE STUCK RESOLVER MAY PUT HIM, by what was found (each already verified: walkable ground under the place, his
/// body free of overlaps there). `ground_dz` / `near_dz` / `anchor_dz`: how far above (+) or below (-) his feet the ground
/// under his centre / at a free spot beside him / where he last stood is. Under his centre and beside him only within
/// `STUCK_NEAR`; where he last stood within `STUCK_ANCHOR_NEAR`, and from the third time at one place whatever its height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unstick {
    Centre,
    Beside,
    Anchor,
    Leave,
}

pub const STUCK_NEAR: f32 = 30.0;
pub const STUCK_ANCHOR_NEAR: f32 = 100.0;

pub fn unstick_choice(again: u32, ground_dz: Option<f32>, near_dz: Option<f32>, anchor_dz: Option<f32>) -> Unstick {
    if again >= 4 {
        return Unstick::Leave;
    }
    if again < 2 {
        if ground_dz.is_some_and(|d| d.abs() <= STUCK_NEAR) {
            return Unstick::Centre;
        }
        if near_dz.is_some_and(|d| d.abs() <= STUCK_NEAR) {
            return Unstick::Beside;
        }
    }
    match anchor_dz {
        Some(d) if d.abs() <= STUCK_ANCHOR_NEAR || again >= 2 => Unstick::Anchor,
        _ => Unstick::Leave,
    }
}

/// HE PUSHES AGAINST SOMETHING ON THE FLOOR AND DOES NOT MOVE (the stairs of the user's screenshot): on the floor, his
/// movement code's speed above 1 m/s, less than 3 mm moved a step, for half a second: true once; again only after he has
/// been 1.5 m away from that place. For the log only.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Blocked {
    last: Option<Vec3>,
    steps: u32,
    told_at: Option<Vec3>,
    pub count: u32,
}

impl Blocked {
    pub fn step(&mut self, on_floor: bool, position: Vec3, velocity: Vec3) -> bool {
        let moved = self.last.replace(position).map_or(f32::MAX, |p| (position - p).length());
        if self.told_at.is_some_and(|at| (at - position).length() > 150.0) {
            self.told_at = None;
        }
        if !on_floor || velocity.truncate().length() < 100.0 || moved > 0.3 {
            self.steps = 0;
            return false;
        }
        self.steps += 1;
        if self.steps == 30 && self.told_at.is_none() {
            self.told_at = Some(position);
            self.count += 1;
            return true;
        }
        false
    }
}

// ---------------------------------------------------------------------------------------------- (IMPL_ER57) deep water

/// (IMPL_ER57) Two contacts of casts of different layers are the same surface within this much, m.
pub const SAME_SURFACE: f32 = 0.1;

/// THE WATER SURFACES IN A COLUMN, from the heights (m, Elden Ring Y) of the flat contacts of three casts straight down
/// through one place: `water_a` (the camera's layer 0x58), `water_b` (a second layer that sees water, 0x5a) and `solid`
/// (Sora's layer 0x56 and the characters' floor layer 0x5c). MEASURED (IMPL_ER3_STATE.md, the layer scan at a shore): the
/// sea surface is hit by 0x57 0x58 0x5a 0x5b 0x66 0x68 and NOT by 0x53 0x55 0x56 0x5c 0x5e 0x5f 0x67, which hit the very
/// same ground bodies as 0x58 everywhere else. A water surface = a contact of both water layers that no solid layer has.
pub fn water_surfaces(water_a: &[f32], water_b: &[f32], solid: &[f32]) -> Vec<f32> {
    let near = |list: &[f32], h: f32| list.iter().any(|x| (x - h).abs() <= SAME_SURFACE);
    let mut out: Vec<f32> = water_a.iter().copied().filter(|h| h.is_finite() && near(water_b, *h) && !near(solid, *h)).collect();
    out.sort_by(|a, b| a.total_cmp(b));
    out.dedup_by(|a, b| (*a - *b).abs() <= SAME_SURFACE);
    out
}

/// HOW DEEP UNDER WATER HIS FEET ARE (m): the nearest water surface above his feet, when nothing solid lies between it and
/// him (a cave under a lake, a room under a pond: the lake's bed is solid between). `None`: no water over him.
pub fn depth_under(feet: f32, surfaces: &[f32], solid: &[f32]) -> Option<f32> {
    let top = surfaces.iter().copied().filter(|h| *h > feet).fold(f32::INFINITY, f32::min);
    if !top.is_finite() {
        return None;
    }
    let roofed = solid.iter().any(|h| *h > feet + 0.3 && *h < top - SAME_SURFACE);
    (!roofed).then_some(top - feet)
}

/// THE GAME'S DEEP-WATER DEATH, for Sora. MEASURED (IMPL_ER3_STATE.md, before the no-death switch was held): the game
/// killed its passive player about 0.6 s after he went under the sea surface. Fires on the frame he has been at least
/// `depth` cm under water for `need` seconds, and again every `need` seconds while he stays under (a Second Chance that
/// left him at 1 HP does not keep him alive down there).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drown {
    pub under_for: f32,
    /// The deepest he was in this dive, cm (for the log).
    pub deepest: f32,
    pub count: u32,
}

impl Drown {
    pub const NEED: f32 = 0.6;

    pub fn frame(&mut self, depth_cm: Option<f32>, min_depth_cm: f32, need: f32, dt: f32) -> bool {
        match depth_cm {
            Some(d) if d >= min_depth_cm => {
                self.deepest = self.deepest.max(d);
                self.under_for += dt;
                if self.under_for >= need {
                    self.under_for = 0.0;
                    self.count += 1;
                    return true;
                }
                false
            }
            _ => {
                self.under_for = 0.0;
                self.deepest = 0.0;
                false
            }
        }
    }

    /// He is (going) under water now: the fall net must not put him back from the sea.
    pub fn under(&self) -> bool {
        self.under_for > 0.0
    }
}

// ---------------------------------------------------------------------------------------------- (IMPL_ER57) a grab of an enemy's

/// The states of the game's throw node (SDK `ThrowNodeState`) in which the PLAYER IS THE ONE GRABBED: 4 InThrowTarget,
/// 6 DeathTarget (the names are the SDK's, not measured by us; `grab.states` overrides).
pub const GRAB_STATES: [u32; 2] = [4, 6];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrabEdge {
    None,
    /// The body is grabbed from this frame on.
    On,
    /// The grab is over after this many seconds (the throw node left the grabbed states).
    Off,
    /// The node has said "grabbed" for longer than any throw lasts: taken as stale, the body is Sora's again.
    GaveUp,
}

/// THE GRAB RULE: the body is grabbed while its throw node is in one of the grabbed states, for at most `max` seconds in
/// one piece (a value that never goes away is no grab: given up until the state leaves the grabbed ones).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Grab {
    pub on: bool,
    pub seconds: f32,
    given_up: bool,
    pub count: u32,
}

impl Grab {
    pub fn frame(&mut self, grabbed: bool, max: f32, dt: f32) -> GrabEdge {
        if !grabbed {
            self.given_up = false;
            if std::mem::take(&mut self.on) {
                return GrabEdge::Off;
            }
            self.seconds = 0.0;
            return GrabEdge::None;
        }
        if self.given_up {
            return GrabEdge::None;
        }
        if !self.on {
            self.on = true;
            self.seconds = 0.0;
            self.count += 1;
            return GrabEdge::On;
        }
        self.seconds += dt;
        if self.seconds > max {
            self.on = false;
            self.given_up = true;
            return GrabEdge::GaveUp;
        }
        GrabEdge::None
    }
}

// ---------------------------------------------------------------------------------------------- (IMPL_ER57) save points in combat

/// THE SAVE POINTS HIDE IN COMBAT: hidden from the frame Sora's battle flag is up (KH3's own battle mode, which the host
/// feeds with its engaged-enemy count: a hostile, living, active, DRAWN enemy within the sheet's `engage_range`, or one
/// that hit him / he hit in the last 5 s, + KH3's 1 s linger) until it has been down for `hold` seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CombatHide {
    pub hidden: bool,
    quiet: f32,
}

impl CombatHide {
    pub fn frame(&mut self, battle: bool, hold: f32, dt: f32) -> bool {
        if battle {
            self.hidden = true;
            self.quiet = 0.0;
        } else if self.hidden {
            self.quiet += dt;
            if self.quiet >= hold {
                self.hidden = false;
                self.quiet = 0.0;
            }
        }
        self.hidden
    }
}

/// (IMPL_ER57) WHETHER A CHARACTER IS DRAWN, by the game's own flags: its render switch (`ChrInsFlags1c5::enable_render`,
/// what an event's "disable" and our own hiding of the player clear) and, when `drawgroups` (the player's own draw-group
/// bit is set, so the bit means what its name says), its MapStudio draw group (`ChrInsFlags1c4::is_render_group_enabled`:
/// a character in a room the player's draw groups do not show is not drawn).
pub fn drawn(render: bool, group_bit: bool, drawgroups: bool) -> bool {
    render && (group_bit || !drawgroups)
}

// ---------------------------------------------------------------------------------------------- (IMPL_ER67) pickups

/// (IMPL_ER67) The game's pickup animations of its player (c0000): 60070 .. 60079. MEASURED: 60071 (1.83 s, items from
/// remains and gathering, IMPL_ER17 and the user's log of 2026-10-14 111.7 s); the rest of the ten is the family's range
/// (only ids that are rows of `own.anims` were ever held, so the range changes nothing else). Never the chest (60110).
pub const PICKUP_ANIMS: (i32, i32) = (60070, 60079);

/// (IMPL_ER67) The body's animation is one of the game's pickups that Sora runs through. `keyed`: `pickup.anims`
/// (`lo-hi,id,...`; "none" = no id). The chest's animations never count (KH3's chest is Sora's own, chest.rs).
pub fn pickup_anim(anim: i32, keyed: Option<&str>) -> bool {
    if (crate::chest::DEFAULT_ANIMS.0..=crate::chest::DEFAULT_ANIMS.1).contains(&anim) {
        return false;
    }
    match keyed.map(str::trim) {
        None | Some("") => (PICKUP_ANIMS.0..=PICKUP_ANIMS.1).contains(&anim),
        Some(text) => crate::ermap::own_ranges(text).iter().any(|(a, b)| (*a..=*b).contains(&anim)),
    }
}

// ---------------------------------------------------------------------------------------------- (ER party S2) a friend leads

/// (ER party S2) THE BODY'S LEADER WHEN A FRIEND LEADS (Donald or Goofy: the hidden player body follows him, ER_PARTY_PLAN
/// S2), as the body rules read him (sora.rs `friend_lead`, made from kh-partyhost's `LeaderView` by party/leader.rs). With
/// Sora leading, and always without the party feature, there is none: every body rule reads Sora's own `Output` as it did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FriendLead {
    /// Actor origin (feet), KH3 space (cm), and yaw (deg).
    pub position: Vec3,
    pub yaw: f32,
    pub velocity: Vec3,
    pub on_floor: bool,
    /// (radius, half height, centre above the feet), cm.
    pub capsule: (f32, f32, f32),
    /// Not knocked out.
    pub alive: bool,
    pub hp: i32,
    pub max_hp: i32,
    /// (GOOFY-BAL) THE HP AN ELDEN RING HIT'S SHARE IS TAKEN OF (sora.rs `damage_in`): Sora's max HP x the leader's
    /// defence factor while the balance rule is on (kh-party balance.rs `incoming_reference`), else his own `max_hp`.
    pub share_max: f32,
    /// Refuses hits now (Die / Warp / a team attack's FR state).
    pub invulnerable: bool,
}

/// (ER party S2) What the body code sends a friend leader (party/leader.rs carries it out through kh-partyhost's
/// `LeaderSink` at the party's next step).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LeadOrder {
    /// A hit, already through the damage-in rule (share of HIS maximum, the status class).
    Hit(kh_core::HitRecord),
    /// The body leads (a cutscene, a grab, an action, the net's put-back): he is put here, KH3 space / yaw.
    Follow(Vec3, f32),
    /// The body leads no more: the hold of `Follow` ends.
    #[cfg_attr(not(feature = "party"), allow(dead_code))]
    Release,
    /// (a lift) He is carried by this much (KH3 cm), once.
    Nudge(Vec3),
    /// (LADDER2) Y at a ladder's foot: the friend leader LEAPS UP it onto the landing (KH3 floor point), facing this yaw.
    LadderJump { to: Vec3, facing: f32 },
    /// Drowned or a death of the net: his KO (the game over is the host's rule).
    Die,
}

/// (ER party S2) Sora's capsule top above his feet (`SORA_CAPSULE` 89 + 91, cm): the measure of the water rule's depth.
pub const SORA_TOP_CM: f32 = 180.0;

/// (ER party S2) THE DEEP-WATER RULE AT THE LEADER'S SIZE: `water.depth` (150 cm: Sora's head) is the depth for a body of
/// Sora's height; a friend drowns at the same share of HIS height (Donald, top 100 cm: 83 cm of water; Goofy as Sora).
pub fn water_depth_for(base_cm: f32, capsule: (f32, f32, f32)) -> f32 {
    let top = capsule.1 + capsule.2;
    if top <= 1.0 {
        return base_cm;
    }
    base_cm * (top / SORA_TOP_CM).clamp(0.3, 1.5)
}

/// (ER party S2) THE BODY'S HIT ON A FRIEND LEADER as kh-party's `receive_hit` takes it (contract B.3: the damage final;
/// the host caps it by his defender's `damage_max` when it delivers). `kind` None = a status tick or damage over time:
/// HP without a reaction (`ReactionResult::NoReaction`). The same facts Sora's `HitOnPlayer` gets (sora.rs damage_in).
#[allow(clippy::too_many_arguments)]
pub fn friend_hit(lead: &FriendLead, kind: Option<kh_core::enums::ETresDamageKind>, amount: f32, causer: kh_core::TargetId, from: Vec3, yaw: f32, radius: f32) -> kh_core::HitRecord {
    use kh_core::host::{HitAttacker, HitInfo, ReactionResult};
    let direction = ((lead.position - from) * Vec3::new(1.0, 1.0, 0.0)).normalize_or_zero();
    let direction = if direction == Vec3::ZERO { Vec3::X } else { direction };
    let info = HitInfo { victim_result: if kind.is_some() { ReactionResult::Damage } else { ReactionResult::NoReaction }, ..HitInfo::DEFAULT };
    kh_core::HitRecord {
        position: lead.position + Vec3::Z * lead.capsule.2,
        direction,
        damage: amount.max(0.0),
        damage_kind: kind.unwrap_or(kh_core::enums::ETresDamageKind::Small),
        attack_name: if kind.is_some() { "er.body" } else { "er.body.tick" },
        attacker: if causer == kh_core::TargetId::NONE { HitAttacker::NONE } else { HitAttacker { position: from, yaw, radius, valid: true } },
        info,
        ..kh_core::HitRecord::empty(kh_core::TargetId::NONE)
    }
}

/// (ER party S2) A blow the friend leader's defender caps (KH3's friend rule `damage_max`; 0 = no cap).
#[cfg_attr(not(feature = "party"), allow(dead_code))]
pub fn capped(damage: f32, damage_max: i32) -> f32 {
    if damage_max > 0 {
        damage.min(damage_max as f32)
    } else {
        damage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::PadButton;

    /// (ER party S2) Donald drowns in shallower water than Sora; Goofy at Sora's depth; a hit on a friend leader: the
    /// direction from the attacker, his capsule centre, no reaction for a tick, the attacker given; the cap.
    #[test]
    fn a_friend_leaders_water_and_hits() {
        assert!((water_depth_for(150.0, (34.0, 89.0, 91.0)) - 150.0).abs() < 1e-3);
        assert!((water_depth_for(150.0, (30.0, 49.0, 51.0)) - 83.33).abs() < 0.1);
        assert!((water_depth_for(150.0, (40.0, 89.0, 91.0)) - 150.0).abs() < 1e-3);
        let lead = FriendLead { position: Vec3::new(100.0, 0.0, 0.0), yaw: 0.0, velocity: Vec3::ZERO, on_floor: true, capsule: (30.0, 49.0, 51.0), alive: true, hp: 80, max_hp: 100, share_max: 100.0 /* (GOOFY-BAL) */, invulnerable: false };
        let hit = friend_hit(&lead, Some(kh_core::enums::ETresDamageKind::Blow), 12.0, kh_core::TargetId(7), Vec3::ZERO, 0.0, 40.0);
        assert!((hit.direction - Vec3::X).length() < 1e-4);
        assert_eq!(hit.position, Vec3::new(100.0, 0.0, 51.0));
        assert_eq!(hit.damage, 12.0);
        assert!(hit.attacker.valid && hit.takes_hp());
        assert_eq!(hit.info.victim_result, kh_core::host::ReactionResult::Damage);
        let tick = friend_hit(&lead, None, 3.0, kh_core::TargetId::NONE, Vec3::new(100.0, 0.0, 0.0), 0.0, 40.0);
        assert_eq!(tick.info.victim_result, kh_core::host::ReactionResult::NoReaction);
        assert!(tick.takes_hp() && !tick.attacker.valid && tick.direction == Vec3::X);
        assert_eq!(capped(500.0, 120), 120.0);
        assert_eq!(capped(50.0, 0), 50.0);
    }

    /// (IMPL_ER67) The user's log, 111.6 s: Y to the game, the item given, the body's 60071: Sora runs on through it; the
    /// chest, the grace, a door, the fog are not pickups.
    #[test]
    fn a_pickup_is_run_through_a_chest_door_or_grace_is_not() {
        assert!(pickup_anim(60071, None));
        assert!(pickup_anim(60070, None));
        for anim in [60110, 60030, 60060, 26011, 68000, 68010, 0, 63000] {
            assert!(!pickup_anim(anim, None), "{anim}");
        }
        // the key replaces the range; a chest id given by the key is still no pickup
        assert!(pickup_anim(61234, Some("61234")));
        assert!(!pickup_anim(60071, Some("61234")));
        assert!(!pickup_anim(60110, Some("60100-60120")));
        assert!(!pickup_anim(60071, Some("none")));
    }

    /// (IMPL_ER57) The user's log, 108 - 111 s: off a cliff into the sea, 110 m down through it, put back by the net.
    #[test]
    fn under_the_sea_surface_he_drowns_in_0_6_s_and_a_wade_or_a_cave_never() {
        // the sea: the water layers meet the surface at 0.0 m; the solid layers only the bed 30 m down
        let surfaces = water_surfaces(&[0.0, -30.0], &[0.04, -30.0], &[-30.0]);
        assert_eq!(surfaces, vec![0.0]);
        // falling through it: 0.5 m above (nothing), then under
        assert_eq!(depth_under(0.5, &surfaces, &[-30.0]), None);
        let d = depth_under(-2.0, &surfaces, &[-30.0]).unwrap();
        assert!((d - 2.0).abs() < 1e-5);
        let mut drown = Drown::default();
        let dt = 0.1;
        let mut fired_at = None;
        for k in 0..20 {
            let feet = -2.0 - 0.4 * k as f32;
            if drown.frame(depth_under(feet, &surfaces, &[-30.0]).map(|m| m * 100.0), 150.0, Drown::NEED, dt) {
                fired_at.get_or_insert(k);
            }
        }
        // 0.6 s after he went under (6 checks of 0.1 s), and again every 0.6 s
        assert_eq!(fired_at, Some(5));
        assert_eq!(drown.count, 3);
        // land: both kinds of layers meet the same ground (4 cm apart): no water at all
        assert!(water_surfaces(&[10.0], &[10.04], &[10.0, 10.04]).is_empty());
        // a wade: 40 cm of water over his feet is never a death
        let mut drown = Drown::default();
        let lake = water_surfaces(&[1.4], &[1.42], &[1.0]);
        assert!((0..100).all(|_| !drown.frame(depth_under(1.0, &lake, &[1.0]).map(|m| m * 100.0), 150.0, Drown::NEED, dt)));
        // a cave under a lake: the lake's bed lies between the surface and him
        assert_eq!(depth_under(-20.0, &[0.0], &[-3.0, -20.0]), None);
        // one layer alone (a camera-only mesh) is no water
        assert!(water_surfaces(&[5.0], &[], &[]).is_empty());
    }

    #[test]
    fn a_grab_follows_the_throw_node_and_a_stale_one_is_given_up() {
        let mut grab = Grab::default();
        assert_eq!(grab.frame(false, 10.0, 0.1), GrabEdge::None);
        assert_eq!(grab.frame(true, 10.0, 0.1), GrabEdge::On);
        for _ in 0..30 {
            assert_eq!(grab.frame(true, 10.0, 0.1), GrabEdge::None);
        }
        assert!(grab.on);
        assert_eq!(grab.frame(false, 10.0, 0.1), GrabEdge::Off);
        assert!(!grab.on);
        // a state that never goes away: given up after the limit, not taken again until it leaves
        assert_eq!(grab.frame(true, 1.0, 0.1), GrabEdge::On);
        let edges: Vec<GrabEdge> = (0..30).map(|_| grab.frame(true, 1.0, 0.1)).collect();
        assert_eq!(edges.iter().filter(|e| **e == GrabEdge::GaveUp).count(), 1);
        assert!(!grab.on);
        assert_eq!(grab.frame(false, 1.0, 0.1), GrabEdge::None);
        assert_eq!(grab.frame(true, 1.0, 0.1), GrabEdge::On);
        assert_eq!(grab.count, 3);
    }

    #[test]
    fn the_save_points_hide_in_a_fight_and_come_back_after_the_hold() {
        let mut hide = CombatHide::default();
        assert!(!hide.frame(false, 3.0, 0.5));
        assert!(hide.frame(true, 3.0, 0.5));
        // the fight is over: still hidden for 3 s
        for _ in 0..5 {
            assert!(hide.frame(false, 3.0, 0.5));
        }
        assert!(!hide.frame(false, 3.0, 0.5));
        // a new blow within the hold starts it again
        assert!(hide.frame(true, 3.0, 0.5));
        assert!(hide.frame(false, 3.0, 0.5));
        assert!(hide.frame(true, 3.0, 0.5));
    }

    #[test]
    fn only_a_drawn_character_is_a_target() {
        assert!(drawn(true, true, true));
        assert!(!drawn(false, true, true));
        assert!(!drawn(true, false, true));
        // the draw-group bit is not trusted (the player's own is not set): the render switch alone
        assert!(drawn(true, false, false));
        assert!(!drawn(false, false, false));
    }

    fn set(buttons: &[PadButton]) -> PadButtons {
        buttons.iter().fold(PadButtons::NONE, |s, b| s.with(*b))
    }

    /// The user: "he jumps when you press B to exit save point screen".
    #[test]
    fn the_press_that_closes_a_menu_is_not_soras() {
        let mut latch = PressLatch::default();
        let b = set(&[PadButton::JUMP]);
        // the menu is up, B goes down: the menu takes it (the rules before give Sora nothing while it is up)
        assert_eq!(latch.filter(b, PadButtons::NONE, true), PadButtons::NONE);
        // the menu has closed, B is still down: before this rule Sora got B here, with a fresh down-edge = a jump
        assert_eq!(latch.filter(b, b, false), PadButtons::NONE);
        assert_eq!(latch.filter(b, b, false), PadButtons::NONE);
        // released, pressed again: his
        assert_eq!(latch.filter(PadButtons::NONE, PadButtons::NONE, false), PadButtons::NONE);
        assert_eq!(latch.filter(b, b, false), b);
    }

    #[test]
    fn other_buttons_and_presses_made_in_play_are_untouched() {
        let mut latch = PressLatch::default();
        let (a, b) = (set(&[PadButton::FaceBottom]), set(&[PadButton::JUMP]));
        // A held from play into a menu and out again stays blocked until released; B pressed after the menu is his
        assert_eq!(latch.filter(a, a, false), a);
        assert_eq!(latch.filter(a, PadButtons::NONE, true), PadButtons::NONE);
        assert_eq!(latch.filter(PadButtons(a.0 | b.0), PadButtons(a.0 | b.0), false), b);
        assert_eq!(latch.blocked(), a);
        assert_eq!(latch.filter(b, b, false), b);
        assert_eq!(latch.blocked(), PadButtons::NONE);
    }

    /// The user: "Sora attacks when pressing A when making a selection in elden ring dialogue".
    #[test]
    fn a_selection_window_of_the_games_has_the_pad_and_its_last_press_too() {
        let (mut watch, mut latch) = (DialogWatch::default(), PressLatch::default());
        let a = set(&[PadButton::FaceBottom]);
        let play = PopupState { top_job: 0, talk_id: 0 };
        let window = PopupState { top_job: 0x7ff6_1234_5678, talk_id: 1040 };
        assert!(!watch.frame(play, 1.0 / 60.0));
        // the window is up: Elden Ring has the pad, A is not delivered to Sora
        let up = watch.frame(window, 1.0 / 60.0);
        assert!(up);
        assert_eq!(latch.filter(a, if up { PadButtons::NONE } else { a }, up), PadButtons::NONE);
        // A picked the line and the window closed under the held button: still not Sora's
        let up = watch.frame(play, 1.0 / 60.0);
        assert!(!up);
        assert_eq!(latch.filter(a, a, up), PadButtons::NONE);
        assert_eq!(latch.filter(PadButtons::NONE, PadButtons::NONE, up), PadButtons::NONE);
        assert_eq!(latch.filter(a, a, up), a);
    }

    #[test]
    fn a_field_that_is_never_empty_or_never_goes_away_takes_nothing_from_sora() {
        let mut watch = DialogWatch::default();
        let busy = PopupState { top_job: 1, talk_id: 0 };
        // never seen empty: it is not what its name says
        assert!((0..600).all(|_| !watch.frame(busy, 1.0 / 60.0)));
        // seen empty, then up for ever: given up after the limit
        let mut watch = DialogWatch::default();
        watch.frame(PopupState::default(), 1.0);
        assert!(watch.frame(busy, 1.0));
        let mut up = 0;
        for _ in 0..400 {
            up += u32::from(watch.frame(busy, 1.0));
        }
        assert!(watch.given_up && (up as f32 - DialogWatch::LIMIT).abs() < 2.0, "{up}");
        assert!(!watch.frame(busy, 1.0));
    }

    /// The user's log, 1745 - 1747 s: a fall of 70 m, his landing, the body's death 5 frames later.
    #[test]
    fn what_the_game_takes_during_a_long_drop_and_right_after_the_landing_is_fall_damage() {
        let mut guard = FallGuard::default();
        let dt = 1.0 / 60.0;
        guard.frame(true, 2256.0, dt);
        assert!(!guard.in_window(true));
        // a hop of 1.2 m and a jump down a 3 m ledge: no window, a hit then is a hit
        for z in [2300.0, 2376.0, 2300.0] {
            guard.frame(false, z, dt);
            assert!(!guard.in_window(false));
        }
        guard.frame(true, 2256.0, dt);
        guard.frame(false, 1960.0, dt);
        assert!(!guard.in_window(false));
        guard.frame(true, 1956.0, dt);
        assert!(!guard.in_window(true));
        // the long fall
        let mut z = 1956.0;
        while z > -4748.0 {
            z -= 50.0;
            guard.frame(false, z, dt);
            assert_eq!(guard.in_window(false), 1956.0 - z >= FALL_MIN_DROP, "at {z}");
        }
        // landed: the body lands a few frames later; 5 frames after his landing the game kills / hurts it
        for _ in 0..5 {
            guard.frame(true, z, dt);
        }
        assert!(guard.in_window(true), "the body's landing");
        // one second later a hit is a hit again
        for _ in 0..60 {
            guard.frame(true, z, dt);
        }
        assert!(!guard.in_window(true));
    }

    /// The user: bugs are targeted below them while flying; other flyers are fine; tall ground enemies keep the cap.
    #[test]
    fn the_point_of_a_tall_ground_enemy_a_hovering_bug_and_a_bird() {
        // a troll: half height 300, bones from its toes up: stands on its node -> no lift, the point capped at 100
        let lift = hover_lift(300.0, Some((70.0, 520.0)), 60.0);
        assert_eq!((lift, aim_height(300.0, 100.0, lift)), (0.0, 100.0));
        // a soldier: nothing changes
        let lift = hover_lift(95.0, Some((45.0, 160.0)), 60.0);
        assert_eq!((lift, aim_height(95.0, 100.0, lift)), (0.0, 95.0));
        // the bug: capsule of half height 60 on the water, its body 150 .. 230 above the node -> the capsule goes up 130,
        // the point is its centre = 190 above the node (was 60: on the water under it)
        let lift = hover_lift(60.0, Some((150.0, 230.0)), 60.0);
        assert_eq!(lift, 130.0);
        assert_eq!(lift + aim_height(60.0, 100.0, lift), 190.0);
        // a tall thing that hovers is not capped either
        let lift = hover_lift(150.0, Some((200.0, 500.0)), 60.0);
        assert_eq!((lift, aim_height(150.0, 100.0, lift)), (200.0, 150.0));
        // a bat: its node is at its body (bones around it): unchanged
        let lift = hover_lift(40.0, Some((-20.0, 50.0)), 60.0);
        assert_eq!((lift, aim_height(40.0, 100.0, lift)), (0.0, 40.0));
        // bones not readable / nonsense: unchanged
        assert_eq!(hover_lift(60.0, None, 60.0), 0.0);
        assert_eq!(hover_lift(60.0, Some((f32::NAN, 10.0)), 60.0), 0.0);
        assert_eq!(hover_lift(60.0, Some((5000.0, 5100.0)), 60.0), 0.0);
    }

    /// The user's log, 1570 - 1596 s: Falling, the speed running up to the terminal one, the place fixed.
    #[test]
    fn falling_speed_without_movement_is_wedged() {
        let mut stuck = Stuck::default();
        let at = Vec3::new(5195.0, -3804.0, 2077.9);
        let mut fired = 0;
        for k in 0..60 {
            fired += u32::from(stuck.step(false, at + Vec3::X * 0.01 * (k % 3) as f32, Vec3::new(0.0, 0.0, -25.0 * k as f32)));
        }
        // (the first 6 steps are under the speed limit)
        assert_eq!((fired, stuck.count), (2, 2));
        // a real fall, a hover (no falling speed), standing: never
        let mut stuck = Stuck::default();
        assert!((0..200).all(|k| !stuck.step(false, at - Vec3::Z * 10.0 * k as f32, Vec3::new(0.0, 0.0, -600.0))));
        assert!((0..200).all(|_| !stuck.step(false, at, Vec3::ZERO)));
        assert!((0..200).all(|_| !stuck.step(true, at, Vec3::new(0.0, 0.0, -600.0))));
    }
    /// (IMPL_ER24) The user's log, 879.8 - 881.4 s: wedged where he stood, the line under him through a hole: "put back",
    /// then "the fall is his death". And 739 - 741 s: a real pit.
    #[test]
    fn the_fall_net_acts_on_a_real_fall_only() {
        let mut anchor = Anchor::default();
        let stood = Vec3::new(297.0, -12929.0, 2890.0);
        for _ in 0..ANCHOR_FRAMES {
            anchor.frame(true, stood);
        }
        assert_eq!(anchor.at, Some(stood));
        // wedged 3 cm lower, no ground seen for 1.5 s and more: nothing, not even the second time
        anchor.frame(false, stood - Vec3::Z * 3.0);
        assert_eq!(net_verdict(1.5, anchor.dropped(stood.z - 3.0), false), NetVerdict::Nothing);
        assert_eq!(net_verdict(9.0, anchor.dropped(stood.z - 3.0), true), NetVerdict::Nothing);
        // an air slide out over a void, higher than where he stood: nothing until he falls
        assert_eq!(net_verdict(2.0, anchor.dropped(stood.z + 237.0), false), NetVerdict::Nothing);
        // a real fall: put back; again from the same place: nothing until 10 m down, then his death
        assert_eq!(net_verdict(1.5, anchor.dropped(stood.z - 972.0), false), NetVerdict::PutBack);
        assert_eq!(net_verdict(1.5, anchor.dropped(stood.z - 400.0), true), NetVerdict::Nothing);
        assert_eq!(net_verdict(1.5, anchor.dropped(stood.z - 1400.0), true), NetVerdict::Death);
        assert_eq!(net_verdict(1.0, anchor.dropped(stood.z - 1400.0), true), NetVerdict::Nothing);
        // a landing of a few frames on an edge is not a place to go back to; the height he left the floor at is kept
        let mut anchor = Anchor::default();
        for _ in 0..ANCHOR_FRAMES {
            anchor.frame(true, stood);
        }
        anchor.frame(false, stood + Vec3::X * 300.0 - Vec3::Z * 100.0);
        for _ in 0..5 {
            anchor.frame(true, stood + Vec3::X * 500.0 - Vec3::Z * 200.0);
        }
        anchor.frame(false, stood + Vec3::X * 520.0 - Vec3::Z * 210.0);
        assert_eq!(anchor.at, Some(stood));
        assert_eq!(anchor.dropped(stood.z - 500.0), 300.0);
    }

    #[test]
    fn the_stuck_resolver_moves_him_little_and_far_only_after_two_tries() {
        // ground under his centre 5 cm up: there
        assert_eq!(unstick_choice(0, Some(4.6), None, Some(-3.0)), Unstick::Centre);
        // ground only 45 cm down through a hole: not there; a free spot beside him at his height: there
        assert_eq!(unstick_choice(0, Some(-45.0), Some(2.0), Some(-489.0)), Unstick::Beside);
        // nothing near, where he last stood is 4.9 m below (the user's log, 507 s): left for two tries, then put back
        assert_eq!(unstick_choice(0, None, None, Some(-489.0)), Unstick::Leave);
        assert_eq!(unstick_choice(1, None, None, Some(-489.0)), Unstick::Leave);
        assert_eq!(unstick_choice(2, None, None, Some(-489.0)), Unstick::Anchor);
        // where he last stood is at his height: at once
        assert_eq!(unstick_choice(0, None, None, Some(-12.0)), Unstick::Anchor);
        // no verified place at all, or freed too often here: left
        assert_eq!(unstick_choice(2, Some(3.0), Some(1.0), None), Unstick::Leave);
        assert_eq!(unstick_choice(4, Some(3.0), Some(1.0), Some(0.0)), Unstick::Leave);
    }

    /// The user's trace, steps 448 - 548: Run on the floor, speed 980 cm/s, the place fixed to the hundredth.
    #[test]
    fn pushing_against_something_without_moving_is_told_once() {
        let mut blocked = Blocked::default();
        let at = Vec3::new(2906.35, -1259.01, 1294.37);
        let fired: u32 = (0..200).map(|_| u32::from(blocked.step(true, at, Vec3::new(682.6, -703.2, 0.0)))).sum();
        assert_eq!(fired, 1);
        // standing, walking on, in the air: never
        let mut blocked = Blocked::default();
        assert!((0..200).all(|_| !blocked.step(true, at, Vec3::ZERO)));
        assert!((0..200).all(|k| !blocked.step(true, at + Vec3::X * 2.0 * k as f32, Vec3::X * 120.0)));
        assert!((0..200).all(|_| !blocked.step(false, at, Vec3::X * 600.0)));
    }

}

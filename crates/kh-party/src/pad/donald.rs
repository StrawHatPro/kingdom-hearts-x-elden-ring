//! (DON-KIT) DONALD'S OWN KIT, THE OVERHAUL (documentation/DONALD_OVERHAUL.md sections 2, 3, 4.2, 4.4, 4.6, 5.1 - 5.3,
//! 6.6, 8, 12, 13; the plan build/sora_tmp/party/DON_PLAN.md; state note build/sora_tmp/party/IMPL_DON_KIT_STATE.md).
//! The user (2026-10-22): "Okay, implement the entire doc. He needs to play just as the doc specifies". This file
//! REPLACES the X3D kit (the Beat / Swing melee combo, the Unison / Meteor Drop / Flare Force finishers, Spell Volley and
//! the RB hold, the Superslide, Air Blink, the Flutter, Counter Blast: doc 13). Donald has NO shotlock (the user,
//! 2026-10-22): RB is lock-on only.
//!
//! THE PAD (Xbox, KH3's layout):
//! | input | ground | air |
//! |---|---|---|
//! | A | magic missiles: wave left, wave right (+ Combo Plus steps), the slam (2 x 0.75, or the finisher / the element's slam); an enemy within 2 m: Wand Bonk (not a combo step) | two downward missiles + a 0.5 s float (Air Combo Plus +1), then Starfall |
//! | A held | the Charged Missile (0.8 s, 0.5 with Charged Missile+): x2.5, pierces, effect doubled, slow walk, no MP | - |
//! | X + stick | the dodge of his attunement (Hop / Flame Step / Ice Glide / Blink / Bubble Hop / Ward Step), 0.9 s cooldown + buffer | Air Step (Aero attuned, 2 per airtime) |
//! | X neutral | Magic Block (frontal, one hit; perfect in 0.15 s: reflects projectiles, +1 pip, +1 Arcane Flow; a miss leaves him open) | the same |
//! | Y | the Y list's selected Arcane spell (DON-UI `FriendMenu::y_selected`, cast by DON-MAGIC `cast::begin`); the team attacks stay on Y (Sora's queue) | same |
//! | LB + face / menu | spells (`wants_cast`) and attunements (`begin_attune`) | same |
//! | B | jump (lower than Sora's); a combo step's cancel after its missile left | held falling: Aero Float / Aero Glide (Aero attuned) |
//!
//! Every number is a row of `sheets/port/party-kit-donald.json` ([`K`], written by build/sora_tmp/party/DON-KIT/
//! make_sheet.js). The shared rules (attunement, Resonance, reactions, statuses, the cast price) are the foundation's
//! (`crate::donald`, DON1), read through `FriendStatus::donald`.

use super::donald_combo as combo;
use super::donald_moves as moves;
use super::donald_shots::{self as shots, Shots};
use super::{enter_jump, enter_run, PadState, JUMP, RUN};
use crate::command::TargetRef;
use crate::donald::cast::{self, CastTarget};
use crate::donald::{data as don_data, CastPlan, DonAbility, DonCmd, DonaldState, Element, SpellRef};
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::pawn::{NpcPawn, StepCtx};
use crate::states::NpcStateId;
use crate::team::proj::TeamProj;
use glam::Vec3;
use kh_core::host::{HitRecord, HitResult, TargetId};
use kh_core::input::PadButton;
use kh_core::params::SORA_PARAMS;

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_kit_donald.rs"));

/// The kit's sheet (sheets/port/party-kit-donald.json).
pub const K: PartyKitDonaldParams = PARTY_KIT_DONALD_PARAMS;

/// The kit's pad states (`NpcStateId::Pad(n)`, n >= [`FIRST`]; Goofy's 40 .. 55 are checked first by `pad::tick`).
pub const FIRST: u8 = 10;
/// A ground combo step (the waves, the Combo Plus steps, the slam).
pub const STEP: u8 = 10;
/// An air missile (and Starfall).
pub const AIR: u8 = 11;
/// Holding A: the Charged Missile.
pub const CHARGE: u8 = 12;
/// Wand Bonk.
pub const BONK: u8 = 13;
/// The ground dodge of his attunement.
pub const DODGE: u8 = 14;
/// Air Step (Aero attuned).
pub const AIRSTEP: u8 = 15;
/// Magic Block.
pub const BLOCK: u8 = 16;
/// Aero Float / Aero Glide.
pub const FLOAT: u8 = 17;
/// The attune cast (4.1; also AI Donald's, through [`begin_attune`]).
pub const ATTUNE: u8 = 18;
pub const LAST: u8 = 18;

/// The name of a kit state (traces, tests).
pub fn state_name(n: u8) -> &'static str {
    match n {
        STEP => "MissileStep",
        AIR => "AirMissile",
        CHARGE => "ChargedMissile",
        BONK => "WandBonk",
        DODGE => "Dodge",
        AIRSTEP => "AirStep",
        BLOCK => "MagicBlock",
        FLOAT => "AeroFloat",
        ATTUNE => "Attune",
        _ => "?",
    }
}

/// A line of the kit's log (traces and tests).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KitEvent {
    Started(&'static str),
    /// A missile left (its kind, element, share).
    Missile { kind: shots::MissileKind, element: Option<Element>, mult: f32 },
    Dodged(&'static str),
    Blocked { perfect: bool },
    /// A press that could not run (the refused SE was asked for).
    Refused(&'static str),
    Cancelled(&'static str),
    Cast(SpellRef),
    Attune(Element),
    /// A hit was absorbed (Bubble Hop, the Ward barrier).
    Absorbed(&'static str),
    /// A hit of the kit landed a reaction (4.5, `Party::don_hit`).
    Reaction(crate::donald::Reaction),
}

/// WHAT THE HOST TELLS THE KIT OF SORA (kh-partyhost lead.rs `route`, kept for the hosts' feed; the overhaul's Donald
/// reads his own learned magic, `FriendStatus::donald`). Only `sora_in_party` / `situation_busy` are read: Y goes to
/// Sora's team-attack queue while it holds a command and the Y list has nothing selected.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SoraLearned {
    pub known: bool,
    pub glide: bool,
    pub air_recovery: bool,
    pub magic: [u8; 6],
    pub unison: [u8; 3],
    pub sora_in_party: bool,
    pub situation_busy: bool,
}

/// The running kit state's own data.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct KitRun {
    /// Seconds in the state.
    pub t: f32,
    pub sub: u8,
    pub dir: Vec3,
    pub target: TargetId,
    pub done: bool,
    pub element: Option<Element>,
    /// An effect / sound handle the state owns (0 = none).
    pub fx: u32,
    pub sound: u32,
    /// The state's effect is to be spawned at its first tick (a move started without the host: the attune cast).
    pub fx_pending: bool,
    pub start: Vec3,
    pub trail: f32,
    pub blocked: bool,
}

/// Donald's kit on his pad (`PadState::donald`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonaldKit {
    pub learned: SoraLearned,
    pub view: Option<kh_core::magic::View>,
    /// The kit's clock (s).
    pub clock: f32,
    pub run: KitRun,
    pub combo: combo::ComboRun,
    pub shots: Shots,
    /// The next dodge may start at this clock time (the 0.9 s cooldown); a press kept for its end.
    pub dodge_ready_at: f32,
    pub dodge_buffered: bool,
    /// The buffered X press (seconds left).
    pub buffered_x: f32,
    /// Air dodges used this airtime (Air Step: 2); float time used this airtime.
    pub air_dodges: u8,
    pub float_used: f32,
    /// Bubble Hop's bubble: seconds left (it absorbs one hit) and its effect.
    pub bubble: f32,
    pub bubble_fx: u32,
    /// The looping sound / persistent effect of the running move (the Ice Glide, the float): stopped when it is left.
    pub owned_fx: u32,
    pub owned_snd: u32,
    /// (KITFX) the kit clock of the float's last start whoosh (Aero Float / Glide's sound is debounced: `float.se_debounce`).
    pub float_whoosh_at: Option<f32>,
    /// More persistent effects of the running move (the Aero Glide's speed lines).
    pub owned_more: Vec<u32>,
    /// The kit's clock time from which a Wand Bonk may start again (its whole clip, the user 2026-10-22: "Donald
    /// shouldn't be able to spam wand bonk even without a target, he can only use it again after the animation").
    pub bonk_ready_at: f32,
    /// A spell cast held him still last step (its end gives the idle back).
    pub cast_hold: bool,
    /// A missed Magic Block leaves him open this long more (no dodge, no attack).
    pub open: f32,
    /// The clock time his last spell's cast began (Spell Weaver).
    pub last_spell_at: Option<f32>,
    /// A cast waiting for this step's pre-move (`wants_cast` / Y): the plan and its target.
    pub pending_cast: Option<(CastPlan, CastTarget)>,
    /// Missiles launched (Dual Attunement's alternation, `DonCore::missile_element`).
    pub missile_n: u32,
    pub rng: u32,
    pub log: Vec<(f32, KitEvent)>,
    prev_state: Option<NpcStateId>,
}

impl DonaldKit {
    pub fn note(&mut self, e: KitEvent) {
        self.log.push((self.clock, e));
        if self.log.len() > 96 {
            self.log.remove(0);
        }
    }

    /// Seconds before the next dodge may start.
    pub fn dodge_cooldown_left(&self) -> f32 {
        (self.dodge_ready_at - self.clock).max(0.0)
    }
}

// ================================================================================================ access

pub(crate) fn kit(p: &mut NpcPawn) -> Option<&mut DonaldKit> {
    p.pad.as_mut().map(|d| &mut d.donald)
}

pub(crate) fn kit_ref(p: &NpcPawn) -> Option<&DonaldKit> {
    p.pad.as_ref().map(|d| &d.donald)
}

pub(crate) fn note(p: &mut NpcPawn, e: KitEvent) {
    if let Some(k) = kit(p) {
        k.note(e);
    }
}

/// His overhaul state (attunement, learned magic, abilities).
pub fn don(p: &NpcPawn) -> Option<&DonaldState> {
    p.status.donald.as_ref()
}

pub(crate) fn don_mut(p: &mut NpcPawn) -> Option<&mut DonaldState> {
    p.status.donald.as_mut()
}

/// He owns and wears `a` (`DonAbilities::has`).
pub fn has(p: &NpcPawn, a: DonAbility) -> bool {
    don(p).is_some_and(|d| d.abilities.has(a))
}

/// The kit state running, if any.
pub fn kit_state(p: &NpcPawn) -> Option<u8> {
    match p.state {
        NpcStateId::Pad(n) if (FIRST..=LAST).contains(&n) => Some(n),
        _ => None,
    }
}


/// The element his dodge and slam take: the primary (newest) attunement (4.2).
pub fn primary(p: &NpcPawn) -> Option<Element> {
    don(p).and_then(|d| d.core.attune.primary())
}

/// Attuned to `e` with Resonance 3.
pub fn full_power(p: &NpcPawn, e: Element) -> bool {
    don(p).is_some_and(|d| d.core.full_power(e, &don_data()))
}

/// The element of the next missile (4.6: Dual alternates) and the count moved on.
pub fn next_missile_element(p: &mut NpcPawn) -> Option<Element> {
    let n = kit_ref(p).map_or(0, |k| k.missile_n);
    let e = don(p).and_then(|d| d.core.missile_element(n));
    if let Some(k) = kit(p) {
        k.missile_n = k.missile_n.wrapping_add(1);
    }
    e
}

/// Spell Weaver's window is open (9: missiles right after a spell).
pub fn weaver(p: &NpcPawn) -> bool {
    has(p, DonAbility::SpellWeaver) && kit_ref(p).is_some_and(|k| k.last_spell_at.is_some_and(|t| k.clock - t <= K.weaver_window))
}

/// The target a move turns to: the lock, else the soft target.
pub fn target(p: &NpcPawn) -> TargetRef {
    p.pad.as_ref().map_or(TargetRef::None, |d| d.target())
}

pub(crate) fn target_info(p: &NpcPawn, host: &PartyHost) -> Option<kh_core::host::TargetInfo> {
    match target(p) {
        TargetRef::Enemy(t) => host.targets.info(t),
        _ => None,
    }
}

pub(crate) fn stick(p: &NpcPawn) -> (f32, f32, bool) {
    p.pad.as_ref().map_or((p.yaw, 0.0, false), |d| (d.stick_yaw, d.stick_magnitude, d.stick_magnitude >= SORA_PARAMS.input_stick_active_min))
}

pub(crate) fn held(p: &NpcPawn, b: PadButton) -> bool {
    p.pad.as_ref().is_some_and(|d| d.frame.pad.held.has(b))
}

pub(crate) fn refused(p: &mut NpcPawn, why: &'static str) {
    if let Some(d) = p.pad.as_mut() {
        d.menu.events.push(kh_core::command::MenuEvent::Refused);
        d.donald.note(KitEvent::Refused(why));
    }
}

// ================================================================================================ invincibility / hits

/// Invincible now (the hit does not exist, as Sora's GHOST_INVINCIBLE): the dodges' windows.
pub fn invincible(p: &NpcPawn) -> bool {
    let Some(k) = kit_ref(p) else { return false };
    match kit_state(p) {
        Some(DODGE) => moves::dodge_invincible(p, k),
        Some(AIRSTEP) => k.run.t <= K.airstep_time * moves::evasion(p),
        _ => false,
    }
}

/// `NpcPawn::receive_hit`'s first word, for EVERY member: the Ward barrier (any member inside one); and for Donald (pad
/// or AI): the dodges' invincibility, the bubble, Magic Block, the attune cast's interruption (R23, Cast Guard), the
/// heavy-hit pip (4.1) and the spell cast's interruption (DON-MAGIC `cast::interrupt`). Some(result) = answered here.
pub fn on_hit(p: &mut NpcPawn, hit: &HitRecord) -> Option<HitResult> {
    let lands = !matches!(p.state, NpcStateId::Die | NpcStateId::Fr(_)) && !p.hurt.dying;
    let absorbed = |p: &NpcPawn| Some(HitResult { accepted: false, damage_dealt: 0.0, hp_after: p.status.hp as f32, killed: false, guarded: true });
    // the Ward Step barrier (4.2 Light): one hit absorbed per member per barrier
    if lands && p.don_ward_offer != 0 && hit.takes_hp() {
        p.don_ward_used = p.don_ward_offer; // (DON-KIT)
        p.don_ward_offer = 0;
        note(p, KitEvent::Absorbed("ward"));
        return absorbed(p);
    }
    if p.chara != Chara::Donald {
        return None;
    }
    if p.pad.is_some() {
        if invincible(p) {
            note(p, KitEvent::Dodged("invincible"));
            return Some(HitResult { accepted: false, damage_dealt: 0.0, hp_after: p.status.hp as f32, killed: false, guarded: false });
        }
        if lands && kit_ref(p).is_some_and(|k| k.bubble > 0.0) && hit.takes_hp() {
            moves::pop_bubble(p);
            note(p, KitEvent::Absorbed("bubble"));
            return absorbed(p);
        }
        if kit_state(p) == Some(BLOCK) {
            if let Some(r) = moves::block_hit(p, hit) {
                return Some(r);
            }
        }
    }
    // the hit will land: the casts it interrupts, the pip it costs
    if lands && hit.takes_hp() && hit.damage >= 1.0 {
        let d = don_data();
        let heavy = crate::hurt::heavy(hit.damage_kind);
        let guard = has(p, DonAbility::CastGuard) && (p.rng.next_u32() % 100) < K.attune_cast_guard as u32;
        if let Some(st) = don_mut(p) {
            if heavy {
                st.core.on_heavy_hit(&d);
            } else if !guard {
                st.core.interrupt_attune();
            }
        }
        cast::interrupt(p, true);
    }
    None
}

// ================================================================================================ the decide hook

/// From `pad::decide` (set_pad_frame, no host), a pad Donald: the menu's Donald actions, Y, and the A buffer. The A
/// press itself runs in pre-move (the Wand Bonk needs the targets).
pub fn decide(p: &mut NpcPawn, action: Option<super::menu::MenuAction>, target: TargetRef) {
    use super::menu::MenuAction;
    match action {
        Some(MenuAction::DonCast { cmd, target: member }) => {
            let t = match member {
                Some(m) => Some(CastTarget::Member(m)),
                None => None,
            };
            let _ = wants_cast(p, cmd, t.map_or(target_of(target), |t| t));
        }
        Some(MenuAction::DonAttune(e)) => {
            if !begin_attune(p, e) {
                refused(p, "attune");
            }
        }
        // (DON-UI) Unattune (the Attunement submenu's last row / a shortcut)
        Some(MenuAction::DonUnattune) => {
            if !begin_unattune(p) {
                refused(p, "unattune");
            }
        }
        _ => {}
    }
    // Y: the Y list's selected Arcane spell (DON-UI); else Y stays the team attacks' (Sora's queue)
    let (y, lb) = p.pad.as_ref().map_or((false, false), |d| (d.frame.pad.pressed.has(PadButton::FaceTop), d.frame.pad.held.has(PadButton::LeftShoulder)));
    if y && !lb {
        let pick = p.pad.as_ref().and_then(|d| d.menu.y_selected());
        if let Some(spell) = pick {
            if let Some(d) = p.pad.as_mut() {
                d.x2.y_taken = true;
            }
            if !wants_spell(p, spell, target_of(target)) {
                refused(p, "y");
            }
        }
    }
    // the X press (dodge / block), kept a moment
    let x = p.pad.as_ref().is_some_and(|d| d.frame.pad.pressed.has(PadButton::GUARD) && !d.frame.pad.held.has(PadButton::LeftShoulder));
    if let Some(k) = kit(p) {
        if x {
            k.buffered_x = K.feel_buffer;
        }
    }
}

fn target_of(t: TargetRef) -> CastTarget {
    match t {
        TargetRef::Enemy(id) => CastTarget::Enemy(id),
        TargetRef::Member(m) => CastTarget::Member(m),
        TargetRef::Point(at) => CastTarget::Point(at),
        _ => CastTarget::Enemy(TargetId::NONE),
    }
}

/// A cast may start now: free, in the air, at a combo step's cancel point (after its missile left), out of a dodge's /
/// block's end.
pub fn cast_window(p: &NpcPawn) -> bool {
    let free = matches!(p.state, NpcStateId::Idle | NpcStateId::Pad(RUN)) || matches!(p.state, NpcStateId::Pad(JUMP));
    if kit_ref(p).is_some_and(|k| k.open > 0.0) {
        return false;
    }
    free || combo::cancel_open(p) || moves::cancel_open(p)
}

/// THE MENU / SHORTCUT ROUTE INTO A CAST (DON-UI's `MenuAction::DonCast`): the command's spell at the level / tier he
/// casts now (R14), its price (`DonCore::check_cast`), queued for this step's pre-move (`cast::begin`, DON-MAGIC). An
/// attunement command goes to [`begin_attune`]. False = refused (the refused SE is asked for).
pub fn wants_cast(p: &mut NpcPawn, cmd: DonCmd, target: CastTarget) -> bool {
    match cmd {
        DonCmd::Attune(e) => {
            let ok = begin_attune(p, e);
            if !ok {
                refused(p, "attune");
            }
            ok
        }
        DonCmd::Spell(s) => {
            let ok = wants_spell(p, s, target);
            if !ok {
                refused(p, "cast");
            }
            ok
        }
    }
}

/// The spell a command casts now (R14): a line-2 id at the level he knows, the -za tier at -ga below full power.
pub fn resolve_spell(st: &DonaldState, s: SpellRef) -> SpellRef {
    let d = don_data();
    match s {
        SpellRef::Line2(e, _) => st.learned.line2_spell(e).unwrap_or(s),
        SpellRef::Line1(..) => st.core.shortcut_tier(s, &st.learned, &d),
        other => other,
    }
}

fn wants_spell(p: &mut NpcPawn, s: SpellRef, target: CastTarget) -> bool {
    if !cast_window(p) {
        return false;
    }
    let d = don_data();
    let (mp, max_mp) = (p.status.mp, p.status.max_mp());
    if p.status.mp_charge_left > 0.0 && !matches!(s, SpellRef::Arcane(..)) {
        return false;
    }
    let Some(st) = don(p) else { return false };
    let spell = resolve_spell(st, s);
    // (DON-UI stub, until DON-SPELLB's / ZETTA's cast gate lands) the ultimates, Ultima and Zettaflare are cast on the
    // ground only (the user, 2026-10-22): refused in the air, as the menu greys them
    if spell.ground_only() && !p.on_ground() {
        return false;
    }
    let dual = st.abilities.has(DonAbility::DualAttunement);
    let Ok(plan) = st.core.check_cast(spell, &st.learned, mp, max_mp, dual, &d) else { return false };
    if let Some(k) = kit(p) {
        k.pending_cast = Some((plan, target));
    }
    true
}

/// THE ATTUNE CAST AS A MOVE (4.1): `DonCore::begin_attune` (0.6 s, Swift 0.3, Instant 0), the cast clip (MagicCast /
/// MagicAirCast) with the element's cast effect; interrupted by a hit (R23, Cast Guard: `on_hit`). For the pad Donald
/// and for AI Donald (DON-PROG's `donald::ai`). False = refused (locked element, already attuned, a cast running, busy).
pub fn begin_attune(p: &mut NpcPawn, e: Element) -> bool {
    let busy = matches!(p.state, NpcStateId::Die | NpcStateId::Damage | NpcStateId::Fr(_)) || kit_ref(p).is_some_and(|k| k.open > 0.0);
    let window = p.pad.is_none() || cast_window(p) || kit_state(p) == Some(FLOAT);
    if busy || !window {
        return false;
    }
    let d = don_data();
    let Some(st) = don_mut(p) else { return false };
    let mods = st.abilities.mods();
    let learned = st.learned;
    let Ok(seconds) = st.core.begin_attune(e, &learned, &mods, &d) else { return false };
    note(p, KitEvent::Attune(e));
    moves::start_attune(p, e, seconds);
    true
}

/// (DON-UI) UNATTUNE (the user, 2026-10-22: "a way ... to unattune and fire his default missiles again"): the same move
/// as attuning (the clip and the attune cast's rules: its length, Swift / Instant Attune, interrupted by a hit, rooted),
/// in his current element's colour; at its end every attunement goes (`DonCore::begin_unattune`; an element at
/// Resonance 3 is Released). False = refused (not attuned, busy).
pub fn begin_unattune(p: &mut NpcPawn) -> bool {
    let busy = matches!(p.state, NpcStateId::Die | NpcStateId::Damage | NpcStateId::Fr(_)) || kit_ref(p).is_some_and(|k| k.open > 0.0);
    let window = p.pad.is_none() || cast_window(p) || kit_state(p) == Some(FLOAT);
    if busy || !window {
        return false;
    }
    let d = don_data();
    let Some(st) = don_mut(p) else { return false };
    let Some(e) = st.core.attune.primary() else { return false };
    let mods = st.abilities.mods();
    let Ok(seconds) = st.core.begin_unattune(&mods, &d) else { return false };
    note(p, KitEvent::Attune(e));
    moves::start_attune(p, e, seconds);
    true
}

// ================================================================================================ pre-move

/// A spell cast (a menu / shortcut / Y spell, a hybrid, an ultimate, Zettaflare: DON-MAGIC's `don_cast.run`, to its end)
/// or the attune cast is running: Donald may not move (pad and AI alike; `NpcPawn::step` holds his place and skips the
/// locomotion states' ticks, the pad's pre-move takes no stick).
pub fn cast_holds_still(p: &NpcPawn) -> bool {
    p.chara == crate::ids::Chara::Donald && (p.don_cast.run.is_some() || p.status.donald.as_ref().is_some_and(|st| st.core.attune.cast.is_some()))
}

/// The pad's side of the hold: out of the run (its loop pose would cover the cast clip), no movement asked.
fn hold_for_cast(p: &mut NpcPawn) {
    if p.state == NpcStateId::Pad(RUN) {
        p.enter(NpcStateId::Idle);
    }
    if let Some(d) = p.pad.as_mut() {
        d.loco = None;
    }
    p.cmc.stop_movement();
    if let Some(k) = kit(p) {
        k.cast_hold = true;
    }
}

fn jump_pressed(p: &NpcPawn) -> bool {
    p.pad.as_ref().is_some_and(|d| d.frame.pad.pressed.has(PadButton::JUMP) && !d.frame.pad.held.has(PadButton::LeftShoulder) && d.menu.state != kh_core::command::MenuState::SubSel)
}

/// From `pad::pre_move` (after targeting), a pad Donald: the kit's clocks, the queued cast, the kit states' input, the
/// entries (A, X, B held falling). True = the kit took the frame.
pub fn pre_move(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) -> bool {
    let dt = cx.dt;
    left(p, host);
    let grounded = p.on_ground();
    if let Some(k) = kit(p) {
        k.clock += dt;
        k.open = (k.open - dt).max(0.0);
        k.buffered_x = (k.buffered_x - dt).max(0.0);
        if k.bubble > 0.0 {
            k.bubble -= dt;
        }
        if grounded {
            k.air_dodges = 0;
            k.float_used = 0.0;
            k.combo.air_n = 0;
        }
        if k.rng == 0 {
            k.rng = 0x0D0A_1D01;
        }
    }
    if kit_ref(p).is_some_and(|k| k.bubble <= 0.0 && k.bubble_fx != 0) {
        moves::end_bubble(p, host);
    }
    // ---- the queued cast (menu / shortcut / Y): DON-MAGIC's cast starts it
    if let Some((plan, target)) = kit(p).and_then(|k| k.pending_cast.take()) {
        if kit_state(p).is_some() {
            end_kit(p, "cast");
        }
        if cast::begin(p, &plan, target, host) {
            hold_for_cast(p);
            let clock = kit_ref(p).map_or(0.0, |k| k.clock);
            if let Some(k) = kit(p) {
                k.last_spell_at = Some(clock);
                k.note(KitEvent::Cast(plan.spell));
            }
            return true;
        }
        refused(p, "cast");
    }
    // ---- A SPELL BEING CAST HOLDS HIM STILL (the user 2026-10-22: "Ensure Donald does not ever walk while casting a spell,
    //      his animation takes priority"): from the cast's start to its end (the shot + its recovery; the attune cast
    //      to its end) no run, walk or drift and no other clip; after the shot a dodge, A or a jump may take him out
    //      (the cast's cancel point). The missile string's strafe / hop and the walk while A is held are attacks: not here.
    if cast_holds_still(p) {
        let after_shot = p.don_cast.run.is_some_and(|r| r.shot);
        let out = after_shot && (jump_pressed(p) || kit_ref(p).is_some_and(|k| k.buffered_x > 0.0) || p.pad.as_ref().is_some_and(|d| d.buffered > 0.0));
        if out {
            p.don_cast.run = None;
        } else {
            hold_for_cast(p);
            return true;
        }
    }
    if kit_ref(p).is_some_and(|k| k.cast_hold) {
        if let Some(k) = kit(p) {
            k.cast_hold = false;
        }
        if p.state == NpcStateId::Idle {
            p.to_idle();
        }
    }
    // ---- a kit state owns the frame
    if kit_state(p).is_some() {
        return kit_input(p, cx, host);
    }
    if kit_ref(p).is_some_and(|k| k.open > 0.0) {
        return false;
    }
    let free = matches!(p.state, NpcStateId::Idle | NpcStateId::Pad(RUN)) || (p.state == NpcStateId::Pad(JUMP) && p.pad.as_ref().is_some_and(|d| d.jump.sub >= 10));
    let airborne = !p.on_ground() && p.state == NpcStateId::Pad(JUMP);
    // ---- X: the dodge (stick) / Magic Block (neutral)
    if kit_ref(p).is_some_and(|k| k.buffered_x > 0.0) && (free || airborne) {
        if moves::press_x(p, host) {
            return true;
        }
    }
    // ---- a dodge kept for the end of its cooldown
    if kit_ref(p).is_some_and(|k| k.dodge_buffered) && (free || airborne) && moves::dodge_ready(p) {
        if let Some(k) = kit(p) {
            k.dodge_buffered = false;
        }
        if moves::start_dodge(p, host) {
            return true;
        }
    }
    // ---- A
    let buffered = p.pad.as_ref().is_some_and(|d| d.buffered > 0.0);
    if buffered && (free || airborne) && combo::press(p, host) {
        if let Some(d) = p.pad.as_mut() {
            d.buffered = 0.0;
        }
        return true;
    }
    // ---- Aero Float / Aero Glide: B held while falling
    let held_b = held(p, PadButton::JUMP) && !held(p, PadButton::LeftShoulder);
    let falling = p.state == NpcStateId::Pad(JUMP) && p.pad.as_ref().is_some_and(|d| d.jump.sub == 4) && p.cmc.velocity.z <= 0.0 && !p.on_ground();
    if held_b && falling && has(p, DonAbility::AeroFloat) && moves::float_left(p) > 0.0 {
        moves::begin_float(p, host);
        return true;
    }
    false
}

/// Input while a kit state runs: its cancels.
fn kit_input(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) -> bool {
    let Some(n) = kit_state(p) else { return false };
    let jump = p.pad.as_ref().is_some_and(|d| d.frame.pad.pressed.has(PadButton::JUMP) && !d.frame.pad.held.has(PadButton::LeftShoulder) && d.menu.state != kh_core::command::MenuState::SubSel);
    let x = kit_ref(p).is_some_and(|k| k.buffered_x > 0.0);
    match n {
        STEP | AIR | BONK => {
            let open = combo::cancel_open(p);
            if open && x {
                if moves::press_x(p, host) {
                    return true;
                }
            } else if open && jump && p.on_ground() {
                end_kit(p, "jump");
                enter_jump(p, false);
                return true;
            }
            // A during a step: queued for its link point (combo.rs)
            if p.pad.as_ref().is_some_and(|d| d.buffered > 0.0) && combo::press(p, host) {
                if let Some(d) = p.pad.as_mut() {
                    d.buffered = 0.0;
                }
            }
        }
        DODGE | BLOCK => {
            if moves::cancel_open(p) {
                if p.pad.as_ref().is_some_and(|d| d.buffered > 0.0) && combo::press(p, host) {
                    if let Some(d) = p.pad.as_mut() {
                        d.buffered = 0.0;
                    }
                } else if x && moves::press_x(p, host) {
                } else if jump && p.on_ground() {
                    end_kit(p, "jump");
                    enter_jump(p, false);
                }
            }
        }
        FLOAT => moves::float_input(p, host),
        _ => {}
    }
    let _ = cx;
    true
}

// ================================================================================================ MainProc

/// `pad::tick` for the kit's states (also an AI Donald's attune cast).
pub fn tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let Some(n) = kit_state(p) else { return };
    match n {
        STEP | AIR | CHARGE | BONK => combo::tick(p, cx, host),
        DODGE | AIRSTEP | BLOCK | FLOAT | ATTUNE => moves::tick(p, cx, host),
        _ => back_to_pad(p),
    }
}

/// Ends the running kit state's overrides (anything that takes him out of it).
pub fn end_kit(p: &mut NpcPawn, why: &'static str) {
    combo::end(p);
    moves::end(p);
    note(p, KitEvent::Cancelled(why));
}

/// Back to the pad's Idle / Run / fall (or Idle for an AI Donald).
pub fn back_to_pad(p: &mut NpcPawn) {
    p.cmc.gravity_off = false;
    p.cmc.direct_xy = None;
    p.cmc.direct_z = None;
    p.anim.speed = 1.0;
    if p.pad.is_none() {
        if !p.on_ground() {
            p.cmc.set_falling();
        }
        p.to_idle();
        return;
    }
    if !p.on_ground() {
        p.cmc.set_falling();
        enter_jump(p, true);
        return;
    }
    let (yaw, mag, active) = stick(p);
    if active {
        enter_run(p, yaw, mag, false);
        if let Some(d) = p.pad.as_mut() {
            d.run.sub = 0;
        }
    } else {
        p.to_idle();
    }
}

/// Enters a kit state (the run's options off, the run data fresh).
pub fn enter_kit(p: &mut NpcPawn, n: u8) {
    p.end_action();
    p.action = None;
    p.enter(NpcStateId::Pad(n));
    p.cmc.speed_limit_override = None;
    p.cmc.walk_mode = false;
    p.anim.speed = 1.0;
    if let Some(d) = p.pad.as_mut() {
        d.loco = None;
        d.donald.run = KitRun::default();
        d.donald.note(KitEvent::Started(state_name(n)));
    }
}

// ================================================================================================ cleanup

/// The kit state was left by something else (a hit reaction, KO, a team attack, a cast): its movement overrides and
/// its owned effects go. First in every pre-move.
pub fn left(p: &mut NpcPawn, host: &mut PartyHost) {
    let prev = kit_ref(p).and_then(|k| k.prev_state);
    let now = kit_state(p).map(NpcStateId::Pad);
    if let (Some(NpcStateId::Pad(n)), true) = (prev, prev != now) {
        if now.is_none() {
            if p.cmc.gravity_off {
                p.cmc.gravity_off = false;
                p.cmc.direct_z = None;
                if !p.on_ground() {
                    p.cmc.set_falling();
                }
            }
            p.cmc.direct_xy = None;
            if p.state != NpcStateId::BadStatFreeze {
                p.anim.speed = 1.0;
            }
        }
        moves::left_state(p, host, n);
    }
    if let Some(k) = kit(p) {
        k.prev_state = now;
    }
}

// ================================================================================================ the party's part

/// THE PARTY'S PART, members only (`Party::step` after the members): every Donald's MP Charge length (R7), his Max MP
/// steps (doc 2: `learned.mp_bonus()` is in `FriendStatus::max_mp`).
pub fn party_step(party: &mut crate::party::Party) {
    let ids: Vec<MemberId> = party.members().collect();
    for id in ids {
        let Some(s) = party.status_mut(id) else { continue };
        if s.chara != Chara::Donald || s.mp_charge_left <= 0.0 {
            continue;
        }
        // R7: the friends' charge (30 s) x clamp(Sora's Magic / his, 0.6, 1); a charge started anywhere (a cast's
        // `pay`, MP spent to 0) runs from KH3's 30 s: it is cut to his length, never lengthened
        let want = mp_charge_time(s);
        if s.mp_charge_left > want {
            s.mp_charge_left = want;
        }
    }
}

/// THE AURAS ON THE MEMBERS (`Party::step`, after DON-PROG's `party_pass` made them, before the members step): what
/// Donald's auras give each member where he stands (`NpcPawn::don_aura`): Thunder's movement (pawn.rs, the movement tick),
/// Aero's evasion (the dodges' windows and cooldowns: Donald's, Goofy's Hop) and recovery (crate::hurt).
pub fn aura_to_members(party: &mut crate::party::Party) {
    let ids: Vec<MemberId> = party.members().collect();
    for id in ids {
        let at = party.member(id).map(|m| m.position());
        let fx = at.map_or(crate::donald::aura::AuraEffect::NEUTRAL, |at| party.don_prog.aura.at(at));
        if let Some(m) = party.member_mut(id) {
            m.don_aura = fx;
        }
    }
}

/// Sora's share of the Thunder aura (the host scales his run / sprint speeds by it, kh-partyhost lead.rs).
pub fn sora_move_mult(party: &crate::party::Party, sora_at: Vec3) -> f32 {
    party.don_prog.aura.at(sora_at).move_mult
}

/// Sora's share of the Aero aura where he stands: (evasion, recovery) for `kh_core::Sora::set_aura` (0, 0 out of reach /
/// no aura: KH3's own Sora).
pub fn sora_aura(party: &crate::party::Party, sora_at: Vec3) -> (f32, f32) {
    let fx = party.don_prog.aura.at(sora_at);
    (fx.evasion.max(0.0), (fx.recovery_mult - 1.0).max(0.0))
}

/// THE PARTY'S PART WITH THE HOST (`Party::step`, after `party_step`): the missiles and areas (`donald_shots`), whose
/// hits go through `Party::don_hit`.
pub fn party_host_step(party: &mut crate::party::Party, dt: f32, sora: &crate::view::SoraView, host: &mut PartyHost) {
    shots::party_step(party, dt, sora, host);
}

/// R7: HIS MP CHARGE LENGTH: the friends' `mpcharge.base` x clamp(Sora's Magic at his level / his Magic, min, 1).
pub fn mp_charge_time(s: &crate::status::FriendStatus) -> f32 {
    let lv = s.level.clamp(1, 99);
    // Sora's Magic at the level (sora-levels + the Kingdom Key's +3, as stream BAL's tables measure him)
    let sora = kh_core::status::tables::level_row(lv).magic.max(1) as f32 + 3.0;
    let own = s.magic().max(1) as f32;
    K.mpcharge_base * (sora / own).clamp(K.mpcharge_ratio_min, 1.0)
}

/// His extra max MP from the boss steps at a line's top (doc 2, `DonLearned::mp_bonus`): 0 for anyone else.
pub fn mp_bonus(s: &crate::status::FriendStatus) -> i32 {
    s.donald.as_ref().map_or(0, |d| d.learned.mp_bonus())
}

/// THE HOST'S FEED (kh-partyhost `route`, before the pad frame): Sora's side (kept for the hosts; see [`SoraLearned`]).
pub fn feed(party: &mut crate::party::Party, id: MemberId, learned: SoraLearned, view: Option<kh_core::magic::View>) {
    let Some(m) = party.member_mut(id) else { return };
    if m.chara != Chara::Donald {
        return;
    }
    if let Some(d) = m.pad.as_mut() {
        d.donald.learned = learned;
        d.donald.view = view;
    }
}

/// The kit's log lines since the last call (the host's trace: "party: t=.. donald: KIT ...").
pub fn take_log(party: &mut crate::party::Party) -> Vec<(MemberId, f32, KitEvent)> {
    let mut out = Vec::new();
    let ids: Vec<MemberId> = party.members().collect();
    for id in ids {
        if let Some(d) = party.member_mut(id).and_then(|m| m.pad.as_mut()) {
            out.extend(d.donald.log.drain(..).map(|(t, e)| (id, t, e)));
        }
    }
    out
}

/// Every pad Donald's missile bodies (for the hosts' projectile taps).
pub fn party_taps(party: &crate::party::Party) -> Vec<(u32, Vec3, Vec3)> {
    party.members().filter_map(|id| party.member(id)).flat_map(taps).collect()
}

/// The plain missiles in flight as stream D's `TeamProj` (Flare Force's missile mesh): the hosts draw them with the
/// team props' meshes (kh-app party_draw `team_props`, kh-er party/team.rs `props`).
pub fn party_props(party: &crate::party::Party) -> Vec<TeamProj> {
    party.members().filter_map(|id| party.member(id)).filter_map(|m| m.pad.as_ref()).flat_map(|d| d.donald.shots.props()).collect()
}

/// The kit's body effects to follow (kh-partyhost runtime: `StepOut::projectiles`).
pub fn taps(p: &NpcPawn) -> Vec<(u32, Vec3, Vec3)> {
    kit_ref(p).map_or(Vec::new(), |k| k.shots.taps())
}

/// The point A aims at with no target: ahead (the stick, else his facing).
pub fn jump_point(p: &NpcPawn) -> TargetRef {
    let (yaw, _, active) = stick(p);
    let yaw = if active { yaw } else { p.yaw };
    TargetRef::Point(p.position() + crate::pawn::forward(yaw) * K.combo_no_target_ahead)
}

impl PadState {
    /// Donald's kit (empty for Goofy).
    pub fn kit(&self) -> &DonaldKit {
        &self.donald
    }
}

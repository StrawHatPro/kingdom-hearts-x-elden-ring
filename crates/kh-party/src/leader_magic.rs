//! (party X2) THE FRIEND LEADER'S FRAMEWORK in kh-party (build/sora_tmp/party/XD_PLAYABLE_KITS.md 1.1, 1.5, 3;
//! approved by the user 2026-10-15). KH3 has no playable Donald / Goofy: the rules below are the port's design built
//! from KH3's pieces, and nothing here acts unless the member is PAD-DRIVEN (he leads) AND the host fed the party
//! [`LeaderFeed`] (`Party::set_leader_magic`). AI Donald keeps his own five KH3 spells (actions.rs, unchanged).
//!
//! DONALD LEADING CASTS SORA'S MAGIC:
//! - the list, the tiers and the unlocks are SORA'S (`MagicBook`: what Sora has learned, his Grand Magic abilities);
//!   Donald's own Magic page / shortcuts (kh-core command/leader.rs) choose among them;
//! - the clip is Donald's own (`n_ex001` anim set): the bolts (Fire, Blizzard) `MagicFire` / `MagicAirFire` (Shot
//!   0.31 / 0.29 s, near Sora's 0.30), the placed spells (Thunder, Water, Aero, Cure), the Finish End and grand magic
//!   `MagicCast` / `MagicAirCast` (Shot 0.50) [KH3 clips, mapping ours: XD 1.1]; his cast effect per element
//!   (`n_ex001_Magic_ead`: vn_ex001_fir / blz / thd / air / cur_cal00 at `vfx_call`); Water has none, so Sora's Water
//!   cast effect `vm_wat_cal0` (the Keyblade's `ead_wep_cal0` row) at the same socket; the voice number as his own
//!   casts (`magic_voice_number`, slot 1325);
//! - the actors, damage, Cure and the formchange gauge are SORA'S (kh-core magic/caster.rs, one caster per leader);
//! - the cost is Sora's (`caster::effective_cost`: the command row's MP, MP Thrift, Magic Combo Thrift, the host's
//!   scale) paid at the Shot from DONALD'S OWN MP by Sora's `UseMagicPoint` rule (0x1405BCDC0: all MP / overdraw ->
//!   0 and the charge, Extra Cast keeps it off, landing exactly on 0 does not start it), with the FRIENDS' charge time
//!   (`mp_charge_time`, 30 s: decision 2);
//! - the chain: a cast out of a melee step or a cast continues the combo count (Sora's shared counter); the 4th is
//!   the Finish End (`_FIN` rows, `MagicCast`); a Fire-family cast may follow straight after the Shot, the others from
//!   the clip's EnableChange [KH3 rule, magic spec A 2.2 / XD 1.1]; an air cast hovers until the Shot (Sora's FireMagic
//!   gravity off until OnShot);
//! - GRAND MAGIC: the caster's full gauge offers the -za spell(s) (`RollGrandMagic`); the offer shows on Donald's
//!   situation slot ahead of Sora's queue, Y casts it (no MP, `MagicCast` at 0.85x speed, the cast effect at 1.5x,
//!   invincible until the Shot as Sora's grand casts are: XD 1.5).
//!
//! GOOFY LEADING: his "Abilities" page / shortcuts (his block) pick Goofy Tornado / Turbo / Discus by tier; the action
//! is his KH3 one, the tier of the run is [`PadMagic::tier`] (X3G implements the tiers' effects).
//!
//! What the host does (kh-partyhost custom.rs): feed [`LeaderFeed`] every step before `set_pad_frame`, publish
//! `Party::caster_projectiles` with the other spell actors, and not forward a Y the leader used ([`PadMagic::y_taken`]).

use crate::actions::ActionRun;
use crate::command::{ActionDef, MemberCommand, TargetRef};
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::pawn::{forward, horizontal, yaw_of, NpcPawn, StepCtx};
use crate::states::NpcStateId;
use crate::view::{PartyEventKind, PartyOutput, SoraHeal, SoraView};
use glam::Vec3;
use kh_core::command::leader::{goofy, LeaderCustom, LeaderKind, MagicBook};
use kh_core::command::CommandRow;
use kh_core::enums::ETresAbilityKind;
use kh_core::host::targets::Ally;
use kh_core::host::{Attach, EffectId, EffectSpawn, PartyMember, TargetId};
use kh_core::magic::caster::{self, CasterStats, GrandOffer, Shot, SpellCaster};
use kh_core::magic::spells::{self, Element};
use kh_core::status::{AbilitySet, GameLevel};

/// What the host feeds the party every step (Sora's side of the shared progression and both leaders' blocks).
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderFeed {
    pub book: MagicBook,
    /// Sora's abilities (only the grand-magic ones are read: `caster::GRAND_ABILITIES`).
    pub sora_abilities: AbilitySet,
    pub game_level: GameLevel,
    pub battle_level: u8,
    /// `Status::mp_cost_scale` (1 = KH3; the Elden Ring host's sheet row).
    pub mp_cost_scale: f32,
    /// Donald's and Goofy's Customize blocks (index = `LeaderKind as usize`).
    pub customs: [LeaderCustom; 2],
    /// Sora's situation queue holds a TEAM attack (class 7) the leader may start: Y and the situation slot are its
    /// first (the user's rule: the leader starts the team attacks), the leader's grand magic waits behind it.
    pub sora_team_offer: bool,
}

/// The party's side: the feed and one caster per leader who cast (kept until his actors are gone).
#[derive(Clone, Debug, Default)]
pub struct PartyMagic {
    pub feed: Option<LeaderFeed>,
    pub casters: Vec<(MemberId, SpellCaster)>,
}

/// The menu's copy of the feed (kh-party pad/menu.rs reads it): `kind` None = no feed (the KH3 lists of stream P).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MenuFeed {
    pub kind: Option<LeaderKind>,
    pub book: MagicBook,
    pub custom: LeaderCustom,
    /// The shortcut set on the panel (0 A, 1 B, 2 C): the pad's own, flipped by LB + D-pad up / down.
    pub page: u8,
    pub offers: Vec<GrandOffer>,
    pub mp_cost_scale: f32,
    /// `LeaderFeed::sora_team_offer`: the leader's offers are not shown and Y is Sora's.
    pub sora_busy: bool,
    /// (YHUD) The team attacks of Sora's queue THAT INCLUDE THIS LEADER (`crate::yhud::leader_team_rows`, set by the
    /// host with `Party::set_leader_team`): his Y rows while they stand. Sora's other rows are never his.
    pub team: Vec<kh_core::command::SituationRow>,
    /// (YHUD) The selected one of `team` (LT cycles, KH3's situation rule).
    pub y_cursor: u8,
}

/// One Sora spell for the next / running action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cast {
    pub cmd: u8,
    pub finish: bool,
    /// The grand-magic offer's serial (0 = a normal spell).
    pub grand: u32,
    /// The combo count of this cast (1 ..= 4).
    pub chain: u8,
}

/// The pad's per-member X2 state (`PadState::x2`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PadMagic {
    /// Parked by the menu for `actions::begin` (taken there), and the cast running now.
    pub queued: Option<Cast>,
    pub current: Option<Cast>,
    /// The Shots of this step for the party's caster (taken by the party's step).
    pub shots: Vec<Shot>,
    /// A grand-magic offer to take off the caster's queue.
    pub take_offer: Option<u32>,
    /// GOOFY: the tier (1 ..= 3) of the ability the menu ran last (X3G reads it at the action's Begin).
    pub tier: u8,
    /// The magic chain: casts in the running combo, and the element of the last cast with its age (seconds).
    pub chain: u8,
    pub last: Option<(Element, f32)>,
    /// The air cast's hover is on (gravity off until the Shot).
    pub hover: bool,
    /// (decision 9, pad/sprint.rs) the run's sprint timer, and the sprint is on (MySprintSpeed).
    pub sprint_timer: f32,
    pub sprinting: bool,
    /// The leader's Y this step was a grand magic: the host does not forward it to Sora's core.
    pub y_taken: bool,
}

pub const fn kind_of(chara: Chara) -> LeaderKind {
    match chara {
        Chara::Donald => LeaderKind::Donald,
        Chara::Goofy => LeaderKind::Goofy,
    }
}

/// The Donald action whose state, start turn (`StartYawMax`) and end rule a Sora spell of `element` runs under.
pub const fn element_def(element: Element) -> ActionDef {
    match element {
        Element::Fire => ActionDef::DonaldFire,
        Element::Blizzard => ActionDef::DonaldBlizzard,
        Element::Thunder | Element::Water => ActionDef::DonaldThunder,
        Element::Aero => ActionDef::DonaldAero,
        Element::Cure => ActionDef::DonaldCure,
    }
}

/// A bolt (`MagicFire` clips) or a placed spell (`MagicCast`).
pub const fn is_bolt(element: Element) -> bool {
    matches!(element, Element::Fire | Element::Blizzard)
}

/// The Goofy action of an ability index (0 Tornado, 1 Turbo, 2 Discus).
pub const fn goofy_def(index: u8) -> ActionDef {
    match index {
        0 => ActionDef::GoofyTornado,
        1 => ActionDef::GoofyTurbo,
        _ => ActionDef::GoofyDiscus,
    }
}

/// The member's menu feed (None = no feed or AI).
pub fn feed_of(p: &NpcPawn) -> Option<&MenuFeed> {
    p.pad.as_ref().map(|d| &d.menu.leader).filter(|f| f.kind.is_some())
}

/// FOR X3: the element of the leader's last cast and how long ago (seconds); None before any.
pub fn last_element(p: &NpcPawn) -> Option<(Element, f32)> {
    p.pad.as_ref().and_then(|d| d.x2.last)
}

/// FOR X3: the magic chain's count (casts in the running combo).
pub fn chain(p: &NpcPawn) -> u8 {
    p.pad.as_ref().map_or(0, |d| d.x2.chain)
}

/// FOR X3: the running action is a Sora spell cast by the leader.
pub fn is_sora_cast(p: &NpcPawn) -> bool {
    p.state == NpcStateId::Action && p.action.is_some_and(|r| r.sora_cmd != 0)
}

/// FOR X3: the next chained cast would be the Finish End (the 4th of the combo).
pub fn finish_next(p: &NpcPawn) -> bool {
    next_chain(p) >= FINISH_AT
}

/// FOR X3 / the hit path: invincible until the Shot (a grand magic, the Finish End: Sora's `damage_invalid` rule).
pub fn invincible(p: &NpcPawn) -> bool {
    is_sora_cast(p) && !p.action.is_some_and(|r| r.shot) && p.pad.as_ref().and_then(|d| d.x2.current).is_some_and(|c| c.grand != 0 || c.finish)
}

/// The 4th step of the combo is the Finish End (Sora's base MaxCombo 3 + the finish: XD 1.2 "3 casts, then a MAGIC
/// FINISHER").
pub const FINISH_AT: u8 = 4;

fn next_chain(p: &NpcPawn) -> u8 {
    let Some(pad) = p.pad.as_ref() else { return 1 };
    if p.state == NpcStateId::Action {
        if p.action.is_some_and(|r| r.sora_cmd != 0) {
            return pad.x2.chain + 1;
        }
        if p.action.and_then(|r| r.def).is_some_and(|d| matches!(d, ActionDef::DonaldBeat | ActionDef::DonaldSwing | ActionDef::DonaldJump)) {
            return pad.combo + 1;
        }
    }
    1
}

/// THE FOLLOW-UP WINDOW of a running Sora cast: after its Shot for the Fire family (FireMagic's follow-up straight
/// after the shot), else from the clip's EnableChange.
pub fn follow_open(p: &NpcPawn) -> bool {
    let Some(run) = p.action.filter(|r| r.sora_cmd != 0) else { return false };
    if p.state != NpcStateId::Action || !run.shot {
        return false;
    }
    let fire = spells::unit(run.sora_cmd).is_some_and(|u| u.element == Element::Fire);
    fire || run.jump.enable_change
}

/// The cost check of a Sora spell for this member now (Sora's `CanUseCommand` / `can_cast`: never while the charge
/// runs; with MP Safety a cost above the MP is refused unless Extra Cast and MP > 0; Cure (all MP) always).
pub fn can_pay(p: &NpcPawn, cmd: u8, scale: f32) -> bool {
    let s = &p.status;
    if s.mp_charge_left > 0.0 {
        return false;
    }
    let Some(cost) = caster::effective_cost(cmd, chain(p) as i32, &s.abilities, scale) else { return false };
    if cost > 0 && s.abilities.has(ETresAbilityKind::MpSafety) && s.mp < cost && !(s.abilities.has(ETresAbilityKind::MpLeave) && s.mp > 0) {
        return false;
    }
    true
}

/// The row of a Sora spell in Donald's menus (`row` of the command menu: enabled unless charging, "MP short" when the
/// effective cost is above his MP).
pub fn spell_row(p_status: &crate::FriendStatus, feed: &MenuFeed, cmd: u8) -> CommandRow {
    let cost = caster::effective_cost(cmd, 0, &p_status.abilities, feed.mp_cost_scale).unwrap_or(0);
    let learned = feed.book.learned(cmd);
    CommandRow { kind: cmd, name: spells::command_name(cmd), enabled: learned && p_status.mp_charge_left <= 0.0, mp_short: learned && cost > 0 && p_status.mp < cost, quantity: 0, needs_target: matches!(cmd, 49..=51) }
}

/// The row of one of Goofy's tiered abilities (his own MP row, `FriendStatus::mp_cost`).
pub fn goofy_row(p_status: &crate::FriendStatus, id: u8) -> CommandRow {
    let Some((index, _)) = goofy::split(id) else { return CommandRow::default() };
    let cost = p_status.mp_cost(goofy::ABILITY_ROWS[index as usize]);
    CommandRow { kind: 28, name: goofy::name(id), enabled: cost.is_some() && p_status.mp_charge_left <= 0.0, mp_short: cost.is_some_and(|c| c > 0 && p_status.mp < c), quantity: 0, needs_target: false }
}

/// A grand-magic offer as the HUD's situation row (class 1 = icon type 3 "magic", kh-host hud.rs).
pub fn offer_row(o: &GrandOffer) -> kh_core::command::SituationRow {
    kh_core::command::SituationRow { serial: 0x5800_0000 | o.serial, kind: o.cmd, name: spells::command_name(o.cmd), icon_type: 3, total: o.total, remaining: o.remaining, enabled: o.delay <= 0.0, refreshed: 0 }
}

/// THE MENU'S SORA SPELL REQUEST (decide, after its state test): parks the cast and issues Donald's action for it.
/// False = refused (not learned, charging, the command refused).
pub fn request_cast(p: &mut NpcPawn, cmd: u8, target: TargetRef) -> bool {
    // (DON-MAGIC) REMOVED FOR DONALD (documentation/DONALD_OVERHAUL.md 13, DON_PLAN R26): he no longer casts Sora's spells;
    // his own magic is crate::donald::cast (the menu / shortcut route of DON-KIT and DON-UI)
    if p.chara == Chara::Donald {
        return false;
    }
    let Some(feed) = feed_of(p).cloned() else { return false };
    let Some(unit) = spells::unit(cmd) else { return false };
    let grand = 0;
    if !unit.grand && (!feed.book.learned(cmd) || !can_pay(p, cmd, feed.mp_cost_scale)) {
        return false;
    }
    let chain = next_chain(p);
    let finish = !unit.grand && chain >= FINISH_AT;
    issue_cast(p, Cast { cmd, finish, grand, chain: chain.min(FINISH_AT) }, target, unit.element)
}

/// Y ON A GRAND-MAGIC OFFER (decide): the -za spell (no MP), its offer leaves the caster's queue at the step.
pub fn request_grand(p: &mut NpcPawn, serial: u32, cmd: u8, target: TargetRef) -> bool {
    // (DON-MAGIC) REMOVED FOR DONALD: his Grand Magic on Y is replaced by the Arcane Y (6.3; crate::donald::cast)
    if p.chara == Chara::Donald {
        return false;
    }
    let Some(unit) = spells::unit(cmd).filter(|u| u.grand) else { return false };
    let ok = issue_cast(p, Cast { cmd, finish: false, grand: serial, chain: 1 }, target, unit.element);
    if let Some(pad) = p.pad.as_mut() {
        pad.x2.y_taken = true;
        if ok {
            pad.x2.take_offer = Some(serial);
        }
    }
    ok
}

fn issue_cast(p: &mut NpcPawn, cast: Cast, target: TargetRef, element: Element) -> bool {
    // a Cure is on himself (Sora's DoCure centres on the caster); a placed / thrown spell on the lock or soft target
    let target = if element == Element::Cure { TargetRef::Member(p.id) } else { target };
    if let Some(pad) = p.pad.as_mut() {
        pad.x2.queued = Some(cast);
    }
    // the command is taken by this step's intake (`actions::begin` takes the parked cast); a refused one drops it
    let t = p.command(MemberCommand::Action { def: element_def(element), target });
    let ok = p.command_status(t) == crate::command::CommandStatus::Running;
    if let Some(pad) = p.pad.as_mut() {
        if !ok {
            pad.x2.queued = None;
        }
        pad.issued.push((t, MemberCommand::Action { def: element_def(element), target }));
        if pad.issued.len() > 8 {
            pad.issued.remove(0);
        }
    }
    ok
}

/// A Sora spell is parked for the action being issued (the command's KH3 ability gate does not apply to it).
pub fn parked(p: &NpcPawn) -> bool {
    p.pad.as_ref().is_some_and(|d| d.x2.queued.is_some())
}

/// actions::begin: the parked cast of this action, if any.
pub fn take_cast(p: &mut NpcPawn, def: ActionDef) -> Option<Cast> {
    let pad = p.pad.as_mut()?;
    let cast = pad.x2.queued.take()?;
    let element = spells::unit(cast.cmd)?.element;
    (element_def(element) == def).then_some(cast)
}

/// THE BEGIN OF A SORA SPELL CAST BY THE LEADER (in place of the KH3 action's Begin): the start turn of the action
/// definition (`m_StartYawMax`) toward the target or the held stick, the clip, the hover, the cast effect, the voice.
pub fn begin(p: &mut NpcPawn, mut run: ActionRun, cast: Cast, cx: &StepCtx, host: &mut PartyHost) -> bool {
    let Some(unit) = spells::unit(cast.cmd) else { return false };
    let Some(def) = run.def else { return false };
    let asset = def.asset();
    let max = crate::tables::aparam(p.chara, asset, "StartYawMax");
    let target_pos = p.target_point(run.target, cx, host).filter(|t| horizontal(*t - p.position()).length() > 1.0);
    if let Some(t) = target_pos {
        let diff = crate::pawn::wrap180(yaw_of(horizontal(t - p.position())) - p.yaw);
        p.yaw = crate::pawn::wrap180(p.yaw + diff.clamp(-max, max));
    } else if let Some(want) = p.pad.as_ref().filter(|d| d.stick_magnitude >= kh_core::params::SORA_PARAMS.input_stick_active_min).map(|d| d.stick_yaw) {
        let diff = crate::pawn::wrap180(want - p.yaw);
        p.yaw = crate::pawn::wrap180(p.yaw + diff.clamp(-max, max));
    }
    p.cmc.stop_movement();
    let air = !p.on_ground();
    let placed = !is_bolt(unit.element) || cast.finish || cast.grand != 0;
    let clip = match (placed, air) {
        (false, false) => "MagicFire",
        (false, true) => "MagicAirFire",
        (true, false) => "MagicCast",
        (true, true) => "MagicAirCast",
    };
    p.play(clip, 0.1);
    if cast.grand != 0 {
        // XD 1.5: the grand cast at 0.85x speed
        p.anim.speed = GRAND_CLIP_SPEED;
    }
    let hover = air;
    if hover {
        // Sora's air cast: gravity off until OnShot (FireMagic Begin / OnShot step 11)
        p.cmc.velocity.z = 0.0;
        p.cmc.gravity_off = true;
        p.cmc.set_flying();
    }
    p.events.push(PartyEventKind::SpellCast { def });
    // the voice number of his own casts (E.4: the clip's MagicCast group notify plays it)
    let m = PartyMember(p.id.0);
    let n = if unit.element == Element::Cure { 0 } else { kh_core::host::party::magic_voice_number(m, p.rng.next_u32()) };
    host.audio.party_voice_group_no(m, n);
    // the cast effect: his own per element (n_ex001_Magic_ead "詠唱" rows), Water's from the Keyblade's table
    let (name, group) = cast_effect(unit.element);
    let scale = if cast.grand != 0 { GRAND_EFFECT_SCALE } else { 1.0 };
    p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(group), name, at: Attach::party(m, Some("vfx_call")), rotation: Vec3::ZERO, scale, persistent: false });
    run.sora_cmd = cast.cmd;
    p.action = Some(run);
    if let Some(pad) = p.pad.as_mut() {
        pad.x2.current = Some(cast);
        pad.x2.chain = cast.chain;
        pad.x2.hover = hover;
        if cast.finish {
            // the Finish End ends the combo (Sora's finisher count)
            pad.combo = 0;
        }
    }
    true
}

/// XD 1.5: the grand cast's clip speed and cast-effect scale [ours].
pub const GRAND_CLIP_SPEED: f32 = 0.85;
pub const GRAND_EFFECT_SCALE: f32 = 1.5;

/// The cast effect of an element: (name, effect group). Donald's `n_ex001_Magic_ead` groups 1300 / 1310 / 1320 / 1330 /
/// 1340 (magicset.<spell>.effect_group) [A]; Water: the Keyblade's `ead_wep_cal0` row group 1360 `vm_wat_cal0` [A,
/// kh-core fire_magic.rs cast_effect_name].
pub fn cast_effect(element: Element) -> (&'static str, u32) {
    let key = match element {
        Element::Fire => "fire",
        Element::Blizzard => "blizzard",
        Element::Thunder => "thunder",
        Element::Aero => "aero",
        Element::Cure => "cure",
        Element::Water => return ("vm_wat_cal0", 1360),
    };
    let name = match element {
        Element::Fire => "vn_ex001_fir_cal00",
        Element::Blizzard => "vn_ex001_blz_cal00",
        Element::Thunder => "vn_ex001_thd_cal00",
        Element::Aero => "vn_ex001_air_cal00",
        _ => "vn_ex001_cur_cal00",
    };
    (name, crate::tables::param(&format!("magicset.{key}.effect_group")) as u32)
}

/// THE SHOT OF A SORA SPELL (the clip's Shot notify, or its end): the MP by Sora's rule from Donald's own MP, the cast
/// handed to the party's caster, the hover off.
pub fn on_shot(p: &mut NpcPawn, mut run: ActionRun, cx: &StepCtx, host: &mut PartyHost) {
    run.shot = true;
    let cmd = run.sora_cmd;
    let cast = p.pad.as_ref().and_then(|d| d.x2.current).filter(|c| c.cmd == cmd).unwrap_or(Cast { cmd, finish: false, grand: 0, chain: 1 });
    let scale = feed_of(p).map_or(1.0, |f| f.mp_cost_scale);
    let unit = spells::unit(cmd);
    // "MP was full when the first shot left" (OnShot step 2)
    let full_mp = cast.grand == 0 && p.status.mp >= p.status.max_mp() && p.status.mp_charge_left <= 0.0;
    if cast.grand == 0 {
        if let Some(cost) = caster::effective_cost(cmd, cast.chain.saturating_sub(1) as i32, &p.status.abilities, scale) {
            use_mp(&mut p.status, cost);
        }
    }
    // the muzzle: the staff's attack socket ATK_01 of the clip (A's track, actor frame), else the body's front
    let f = forward(p.yaw);
    let right = Vec3::new(-f.y, f.x, 0.0);
    let keys = p.anim.clip.socket;
    let muzzle = if keys.is_empty() {
        p.capsule_centre() + f * p.params.capsule_radius
    } else {
        let v = crate::actions::socket_track_at(keys, p.anim.time);
        p.position() + f * v.x + right * v.y + Vec3::Z * v.z
    };
    let target = match run.target {
        TargetRef::Enemy(t) => t,
        _ => TargetId::NONE,
    };
    let shot = Shot { cmd, origin: p.position(), yaw: p.yaw, muzzle: Some(muzzle), target, finish: cast.finish, grand: cast.grand != 0, full_mp, combo: [cast.chain as i32, 0, 0, 0] };
    let _ = (cx, &host);
    p.action = Some(run);
    if let Some(pad) = p.pad.as_mut() {
        pad.x2.shots.push(shot);
        if let Some(u) = unit {
            pad.x2.last = Some((u.element, 0.0));
        }
        if cast.finish {
            pad.x2.chain = 0;
        }
    }
    end_hover(p);
    if let Some(u) = unit {
        p.events.push(PartyEventKind::ProjectileSpawned { kind: spells::command_name(u.cmd), at: muzzle });
    }
}

fn end_hover(p: &mut NpcPawn) {
    let on = p.pad.as_ref().is_some_and(|d| d.x2.hover);
    if on {
        p.cmc.gravity_off = false;
        if !p.on_ground() {
            p.cmc.set_falling();
        }
        if let Some(pad) = p.pad.as_mut() {
            pad.x2.hover = false;
        }
    }
}

/// `UseMagicPoint(cost)` 0x1405BCDC0 (kh-core `Status::use_mp`) on a friend's MP with the friends' charge time:
/// -1 = all MP (0 and the charge); an overdraw -> 0 and the charge, unless Extra Cast (MP_LEAVE) and MP > 0 (0, no
/// charge); else MP - cost (landing exactly on 0 does not start the charge: the next cast does).
pub fn use_mp(s: &mut crate::FriendStatus, cost: i32) {
    if s.mp_charge_left > 0.0 || cost == 0 {
        return;
    }
    let charge = crate::tables::cparam(s.chara, "mp_charge_time");
    if cost < 0 {
        s.mp = 0;
        s.mp_charge_left = charge;
    } else if s.mp - cost < 0 {
        let leave = s.abilities.has(ETresAbilityKind::MpLeave) && s.mp > 0;
        s.mp = 0;
        if !leave {
            s.mp_charge_left = charge;
        }
    } else {
        s.mp -= cost;
    }
}

// ------------------------------------------------------------------------------------------------ the party's side

/// The Cure's allies: ids the caster hands back in its heals.
const ALLY_SORA: TargetId = TargetId(0xA11E_00FF);
const fn ally_member(id: MemberId) -> TargetId {
    TargetId(0xA11E_0000 + id.0 as u32)
}

impl crate::party::Party {
    /// (party X2) THE HOST'S FEED, every step before `set_pad_frame` (None: no feed, every friend keeps his KH3 kit).
    pub fn set_leader_magic(&mut self, feed: Option<LeaderFeed>) {
        let offers: Vec<(MemberId, Vec<GrandOffer>)> = self.x2.casters.iter().map(|(id, c)| (*id, c.offers().to_vec())).collect();
        let ids: Vec<MemberId> = self.members().collect();
        for id in ids {
            let Some(m) = self.member_mut(id) else { continue };
            let chara = m.chara;
            let Some(pad) = m.pad.as_mut() else { continue };
            let menu = &mut pad.menu.leader;
            match &feed {
                Some(f) => {
                    let kind = kind_of(chara);
                    menu.kind = Some(kind);
                    menu.book = f.book;
                    menu.custom = f.customs[kind as usize];
                    menu.mp_cost_scale = f.mp_cost_scale;
                    menu.sora_busy = f.sora_team_offer;
                    menu.page = menu.page.min(2);
                    // (DON-MAGIC) no Grand Magic offer for Donald any more (his Y is the Arcane Y): every leader's list is empty
                    let _ = &offers;
                    menu.offers = Vec::new();
                }
                None => *menu = MenuFeed::default(),
            }
        }
        self.x2.feed = feed;
    }

    /// (party X2) The leaders' spell actors (Sora's, cast by Donald) for the effect side: (body-effect handle, position,
    /// forward), as `ProjectileTap`.
    pub fn caster_projectiles(&self) -> impl Iterator<Item = (u32, Vec3, Vec3)> + '_ {
        self.x2.casters.iter().chain(self.don_fx.casters.iter()) // (DON-MAGIC part A) Donald's line-1 casters (Sora's actors) too
            .flat_map(|(_, c)| c.projectiles().filter_map(|v| v.effect.map(|e| (e.0, v.position, v.forward))))
    }

    /// (party X2) A member's caster (tests, the debug text, the HUD gauge).
    pub fn caster(&self, id: MemberId) -> Option<&SpellCaster> {
        self.x2.casters.iter().find(|c| c.0 == id).map(|c| &c.1)
    }

    pub fn caster_mut(&mut self, id: MemberId) -> Option<&mut SpellCaster> {
        self.x2.casters.iter_mut().find(|c| c.0 == id).map(|c| &mut c.1)
    }

    /// (party X2) The Blizzard ice rails the leaders' bolts laid (a host merges them with Sora's).
    pub fn caster_rails(&self) -> Vec<kh_core::magic::RailNode> {
        self.x2.casters.iter().chain(self.don_fx.casters.iter()).flat_map(|(_, c)| c.rails().iter().copied()).collect() // (DON-MAGIC part A) Donald's line-1 casters' ice rails too
    }
}

/// (party X2) THE PARTY STEP'S PART (after the members, before the KH3 projectiles): the Shots of this step to the
/// casters, the taken offers, every caster's actors, the Cure heals.
pub fn step(party: &mut crate::party::Party, dt: f32, sora: &SoraView, host: &mut PartyHost, out: &mut PartyOutput) {
    // the sprint lives in the run state's loop only (pad/sprint.rs): the stop, the start / turn-around clips and any other
    // state end it (Sora's Run resets the timer there too)
    for id in party.members().collect::<Vec<_>>() {
        if let Some(m) = party.member_mut(id) {
            let running = m.state == NpcStateId::Pad(crate::pad::RUN);
            if let Some(pad) = m.pad.as_mut() {
                if !running || pad.run.sub != 0 {
                    crate::pad::sprint::reset(&mut pad.x2);
                    if running {
                        m.cmc.max_walk_speed = m.params.speed_run;
                    }
                }
            }
        }
    }
    let Some(feed) = party.x2.feed.clone() else {
        // no feed: the casters left finish their actors
        step_casters(party, dt, sora, host, out);
        return;
    };
    let ids: Vec<MemberId> = party.members().collect();
    for id in ids {
        let Some(m) = party.member_mut(id) else { continue };
        if m.chara != Chara::Donald {
            continue;
        }
        // (DON-MAGIC) Donald's X2 casts of Sora's spells are removed (request_cast refuses him): nothing reaches a caster
        // here any more; his magic is crate::donald::cast. The loop stays for a cast already in flight at a load.
        // timers of the pad side; the hover never outlives the cast (a hit or a cancel before the Shot)
        if let Some(pad) = m.pad.as_mut() {
            if let Some((_, age)) = pad.x2.last.as_mut() {
                *age += dt;
            }
        }
        let casting = is_sora_cast(m) && !m.action.is_some_and(|r| r.shot);
        if !casting {
            end_hover(m);
            if let Some(pad) = m.pad.as_mut() {
                if pad.x2.current.is_some() && m.state != NpcStateId::Action {
                    pad.x2.current = None;
                }
                // the chain lives while he chains: a cast or a melee step running
                if !matches!(m.state, NpcStateId::Action) {
                    pad.x2.chain = 0;
                }
            }
        }
        let Some(pad) = m.pad.as_mut() else { continue };
        let shots = std::mem::take(&mut pad.x2.shots);
        let take = pad.x2.take_offer.take();
        if shots.is_empty() && take.is_none() && !party.x2.casters.iter().any(|c| c.0 == id) {
            continue;
        }
        let (strength, magic, own, charging, position, yaw) = {
            let m = party.member(id).unwrap();
            (m.status.strength(), m.status.magic(), m.status.abilities.clone(), m.status.mp_charge_left > 0.0, m.position(), m.yaw)
        };
        let k = match party.x2.casters.iter().position(|c| c.0 == id) {
            Some(k) => k,
            None => {
                party.x2.casters.push((id, SpellCaster::new(PartyMember(id.0))));
                party.x2.casters.len() - 1
            }
        };
        let c = &mut party.x2.casters[k].1;
        c.set_stats(CasterStats { strength, magic, game_level: feed.game_level, battle_level: feed.battle_level, mp_cost_scale: feed.mp_cost_scale, mp_charging: charging }, &own, &feed.sora_abilities);
        c.place(position, yaw);
        if let Some(serial) = take {
            c.take_offer(serial);
        }
        for shot in shots {
            c.shoot(host.world, host.targets, host.audio, host.effects, &shot);
        }
    }
    step_casters(party, dt, sora, host, out);
}

fn step_casters(party: &mut crate::party::Party, dt: f32, sora: &SoraView, host: &mut PartyHost, out: &mut PartyOutput) {
    if party.x2.casters.is_empty() {
        return;
    }
    let members: Vec<(MemberId, Vec3, bool, f32)> = party.members().collect::<Vec<_>>().into_iter().filter_map(|i| party.member(i).map(|m| (m.id, m.position(), m.status.ko(), m.yaw))).collect();
    let mut casters = std::mem::take(&mut party.x2.casters);
    for (id, c) in casters.iter_mut() {
        if let Some(&(_, pos, _, yaw)) = members.iter().find(|m| m.0 == *id) {
            c.place(pos, yaw);
        }
        let mut allies: Vec<Ally> = members.iter().filter(|m| m.0 != *id).map(|m| Ally { id: ally_member(m.0), position: m.1, dead: m.2 }).collect();
        // Sora: AI Sora while a friend leads (`ai_sora`), else the player
        match sora.ai_sora {
            Some((_, at)) => allies.push(Ally { id: ALLY_SORA, position: at, dead: false }),
            None if party.member(*id).is_some_and(|m| m.pad.is_none()) => allies.push(Ally { id: ALLY_SORA, position: sora.position, dead: sora.dead }),
            None => {}
        }
        let step = c.step(host.world, host.targets, host.audio, host.effects, &allies, dt);
        for heal in step.heals {
            match heal.ally {
                None => {
                    party.heal(*id, heal.amount, heal.effect);
                }
                Some(ALLY_SORA) => match sora.ai_sora {
                    Some(_) => out.ai_sora_heals.push(SoraHeal { amount: heal.amount, effect: heal.effect, mp: 0, from: *id }),
                    None => out.sora_heals.push(SoraHeal { amount: heal.amount, effect: heal.effect, mp: 0, from: *id }),
                },
                Some(t) if t.0 & 0xFFFF_FF00 == 0xA11E_0000 => {
                    party.heal(MemberId((t.0 & 0xFF) as u8), heal.amount, heal.effect);
                }
                Some(_) => {}
            }
        }
    }
    // a caster whose member no longer leads (or is gone) is dropped once its actors are over
    casters.retain(|(id, c)| party.member(*id).is_some_and(|m| m.pad.is_some()) || c.alive() > 0);
    party.x2.casters = casters;
}

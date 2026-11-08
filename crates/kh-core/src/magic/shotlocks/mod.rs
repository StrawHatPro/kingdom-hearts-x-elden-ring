//! (SHOT-A) THE KEYBLADES' SHOTLOCKS: the `ETresShootFlowKind`s other than the Kingdom Key's King of Hearts / Ragnarok
//! (kinds 1 / 2) and Focus Syphon (36), all run by `TresPlayerState_ShootFlow` (41, states/shoot_flow.rs). KH3's state
//! class has ONE update function per family of kinds (Begin 0x1413A5CD0 stores it as the delegate +0x170); each family is
//! a module here (sheet `sora-shotlocks` `kind.k<N>.family`):
//!
//! | module | update | kinds (KH3 name) |
//! |---|---|---|
//! | volley (states/shoot_flow.rs) | 0x1413B8C90 / 0x1413B88F0 | 1, 2, 12 Snakebite (MI_SHOT), SHOT-B/C: 20, 30, 32, 34 / 28, 31, 33, 35 |
//! | `drain` | 0x1413B73D0 | 3 Drain Shock (HE_DRAIN), 10 Shimmering Drops (RA_DRAIN) |
//! | `pellet` | 0x1413B9910 | 4 Atomic Deluge, (24) |
//! | `meteor` | 0x1413BA090 | 5 Meteor Shower (SF_SHOOTINGSTAR), (18, 23, 40) |
//! | `barrage` | 0x1413B6320 | 6 Diving Barrage (DW_BARRAGE), (21) |
//! | `cannon` | 0x1413B6960 | 7 Cluster Cannonade (BZ_CANNON), (22) |
//! | `avatar` | 0x1413B5E30 | 8 Ghost Horde, (25) |
//! | `rush` | 0x1413BC550 | 9 Drill Dive, (26) |
//! | `laser` | 0x1413B9600 | 11 Spectral Rays |
//! | `gate` | 0x1413B7B40 | 13 Warp Trick (MI_GATE), (19) |
//! | `burst` (SHOT-B) | 0x1413BADB0 | 14 Diamond Dust (FZ_BURST), 16 Blade Storm (CA_BURST), (27, 41) |
//! | `vacuum` (SHOT-B) | 0x1413BBEE0 | 15 Frozen Crescents (FZ_VACUUMWAVE), 17 Flag Rampage (CA_VACUUMWAVE) |
//!
//! WHICH KIND: `SelectKind` 0x141411B80 (magic/shotlock.rs `select_kind`): the base kind of the held Keyblade's weapon
//! type (pawn vt+0x12C8) while aiming, its MAX kind once the locks reach the base kind's `Param0` (Sora only): rows
//! `select.base.w<T>` / `select.max.w<T>`. KH3 has no menu choice between a Keyblade's shotlocks: the abilities
//! `TEXT_SHOOTFLOW_*` its `m_ShootflowAbilitys` lists are display rows (m_UseAP 0) naming the base and the MAX kind.
//! A kind whose family is not ported yet ("pending", as P3's pending forms) falls back to the Kingdom Key's kinds.
//!
//! CONVENTIONS FOR A FAMILY MODULE (SHOT-B / SHOT-C follow them): `pub(crate) struct State` (Copy, Default, Debug,
//! PartialEq: the family's fields of the state object), one `Ext` variant here, and five functions (signatures as the
//! stubs): `begin` (after the common Begin and the per-kind set-up), `update` (the update delegate; `false` = the state
//! finished this tick: no PostTick), `on_notify` (after the shared handlers below), `end` (before the common End's
//! release), `muzzle` (the Shot handler's spawn point for a `LocationIndex`, `None` = weapon 0's ATK_01). `PORTED`
//! true once the family runs. Its rows: `tools/shotlocks/<family>.js` -> sora-shotlocks `<family>.*`. Its trace test:
//! `crates/kh-core/tests/shotlock_<family>.rs` with `tests/shotlocks_common/mod.rs`.

use glam::Vec3;

use super::projectile::{self, SpawnArgs, StageDef};
use super::spells::cmd;
use super::tables::Sheet;
use crate::anim::{AnimId, NotifyClass, NotifyPhase, Notifies, PlayArgs};
use crate::host::{TargetFlags, TargetId};
use crate::pawn::{Ctx, Pawn};
use crate::states::shoot_flow::ShootFlow;

pub mod avatar; // (SHOT-C avatar) pub: the hosts read `PawnMagic::shot_avatars`
pub(crate) mod barrage;
pub(crate) mod cannon;
pub(crate) mod drain;
pub(crate) mod gate;
pub(crate) mod burst; // (SHOT-B) Diamond Dust / Blade Storm
pub mod vacuum; // (SHOT-B) Frozen Crescents / Flag Rampage (pub: the hosts read its weapon parts)
pub(crate) mod laser;
pub(crate) mod meteor;
pub(crate) mod pellet;
pub mod rush; // (SHOT-A rush) pub: the hosts read its weapon parts
pub mod ultima; // (SHOT-C) Infinity Circle (pub: the hosts read its weapon parts)
pub mod inception; // (SHOT-C) Stellar Inception (pub: the hosts read its weapon parts)

const S: Sheet = Sheet::Shotlocks;

// ------------------------------------------------------------------------------------------------ the kind table

/// `kind.k<N>.<property>` as a name ("-" when the sheet has none).
fn kind_text(kind: u8, property: &str) -> &'static str {
    S.text_or(&format!("kind.k{kind}.{property}"), "-")
}

/// The family module of a kind ("volley", "drain" ...; "-" for a kind the sheet does not list).
pub fn family(kind: u8) -> &'static str {
    kind_text(kind, "family")
}

/// The ability that names the kind (`TEXT_SHOOTFLOW_KB02_00` ...; "-" when no Keyblade lists it).
pub fn ability(kind: u8) -> &'static str {
    kind_text(kind, "ability")
}

/// The kind's name / help text ids (`kh3_system.command.sf_drain_0`, `kh3_system.ability.detail_txt_0120`) and its English
/// name (WD's) for the HUD.
pub fn name_id(kind: u8) -> &'static str {
    kind_text(kind, "name_id")
}

pub fn help_id(kind: u8) -> &'static str {
    kind_text(kind, "help_id")
}

pub fn name_en(kind: u8) -> &'static str {
    kind_text(kind, "name_en")
}

/// The ETresItemDefWeapon value of a Keyblade (pawn vt+0x12C8 while it is held): `keyblade.<key>.weapon_type`.
pub fn weapon_type_of(id: crate::status::KeybladeId) -> u8 {
    S.num_or(&format!("keyblade.{}.weapon_type", id.key().to_ascii_lowercase()), 1.0) as u8
}

/// The weapon type Sora holds now (the Kingdom Key's 1 without a Keyblade row).
pub(crate) fn weapon_type(pawn: &Pawn) -> u8 {
    pawn.status.current_keyblade().map_or(1, weapon_type_of)
}

/// `SelectKind`'s base kind of a weapon type (`select.base.w<T>`).
pub fn base_kind(weapon_type: u8) -> u8 {
    S.num_or(&format!("select.base.w{weapon_type}"), 1.0) as u8
}

/// `SelectKind`'s MAX kind of a weapon type (`select.max.w<T>`; `alt`: 0x1413ECC80's choice, `select.max_alt.w<T>`).
pub fn max_kind(weapon_type: u8, alt: bool) -> u8 {
    let normal = S.num_or(&format!("select.max.w{weapon_type}"), 2.0) as u8;
    if alt {
        S.num_or(&format!("select.max_alt.w{weapon_type}"), normal as f32) as u8
    } else {
        normal
    }
}

/// The two (Shooting Star: three) kinds of a Keyblade in KH3's order: base, MAX, the alternative MAX.
pub fn keyblade_kinds(id: crate::status::KeybladeId) -> Vec<u8> {
    let w = weapon_type_of(id);
    let mut out = vec![base_kind(w)];
    for k in [max_kind(w, false), max_kind(w, true)] {
        if !out.contains(&k) {
            out.push(k);
        }
    }
    out
}

/// A kind runs as KH3's: the volley kinds of the Kingdom Key's code and every family whose module is ported. A pending
/// kind is never selected (`select_kind` keeps the Kingdom Key's kinds instead).
pub fn ready(kind: u8) -> bool {
    match family(kind) {
        "volley" | "syphon" => true,
        f => family_ported(f),
    }
}

fn family_ported(family: &str) -> bool {
    match family {
        "drain" => drain::PORTED,
        "pellet" => pellet::PORTED,
        "meteor" => meteor::PORTED,
        "barrage" => barrage::PORTED,
        "cannon" => cannon::PORTED,
        "avatar" => avatar::PORTED,
        "rush" => rush::PORTED,
        "ultima" => ultima::PORTED, // (SHOT-C)
        "inception" => inception::PORTED, // (SHOT-C)
        "laser" => laser::PORTED,
        "gate" => gate::PORTED,
        "burst" => burst::PORTED,   // (SHOT-B)
        "vacuum" => vacuum::PORTED, // (SHOT-B)
        _ => false,
    }
}

/// `0x1413ECC80(ctrl)`: the alternative MAX kind of weapon types 3 / 8 (Diving Barrage instead of Cluster Cannonade):
/// at most `select.alt.max_locked_targets` locked targets (at least one), the first one alive, and a sphere of radius
/// `select.alt.radius` swept from Sora's reference point to `select.alt.up` + his half height above that target's
/// centre meets no level geometry. (SHOT-FIX, traced) The sweep is pawn vt+0x2750 = 0x141431630 ("Warp"): a sphere of
/// radius 30 swept by channel 2 with FCollisionResponseParams SetAllChannels(Ignore) (0x14443E7E0(p, 0)) then Block for
/// object types 0 WorldStatic, 1 WorldDynamic and 18 TresGimmick only (0x14443F9F0(p, 0 / 1 / 0x12, 2)), Sora ignored by
/// the query params; any blocking hit but Sora's own fails it (param_7 = 1: not even the target is excused). Pawns
/// (object type Pawn) never block it, the same responses as the lock's line of sight 0x1413F0A80 (docs/sora-magic-spec.md
/// A4.3): other enemies do not block the sky, so the port's sweep against the level is the whole test. The target
/// actor's vt+0xAB0 test (0x1405AAFB0) has no counterpart: alive and lockable.
pub(crate) fn alt_max(cx: &Ctx) -> bool {
    let aim = &cx.pawn.magic.aim;
    if aim.locked_targets > S.int("select.alt.max_locked_targets") || aim.locked.is_empty() {
        return false;
    }
    let Some(t) = cx.target(aim.locked[0].target).filter(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE)) else { return false };
    let from = cx.pawn.cmc.location + Vec3::Z * S.num("select.alt.from_up");
    let half = cx.pawn.params().capsule_half_height;
    let to = t.center + Vec3::Z * (S.num("select.alt.up") + half);
    let r = S.num("select.alt.radius");
    cx.world.sweep_capsule(from, to, r, r).is_none()
}

// ------------------------------------------------------------------------------------------------ the state's part

/// The family part of the state object (one variant per family; `None`: the Kingdom Key's kinds).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) enum Ext {
    #[default]
    None,
    Drain(drain::State),
    Pellet(pellet::State),
    Meteor(meteor::State),
    Barrage(barrage::State),
    Cannon(cannon::State),
    Avatar(avatar::State),
    Rush(rush::State),
    Ultima(ultima::State), // (SHOT-C)
    Inception(inception::State), // (SHOT-C)
    Laser(laser::State),
    Gate(gate::State),
    Burst(burst::State),   // (SHOT-B)
    Vacuum(vacuum::State), // (SHOT-B)
}

/// The kind is run by a module here (not by the Kingdom Key's code of states/shoot_flow.rs).
pub(crate) fn owns(kind: u8) -> bool {
    !matches!(family(kind), "volley" | "syphon" | "-") && ready(kind)
}

/// Runs `f` on the family's state and the state object (the variant is copied out and back: the state is `Copy`).
macro_rules! with_family {
    ($sf:expr, $st:ident => $body:expr, $none:expr) => {{
        let mut ext = $sf.ext;
        let out = match &mut ext {
            Ext::Drain($st) => { use drain as fam; $body }
            Ext::Pellet($st) => { use pellet as fam; $body }
            Ext::Meteor($st) => { use meteor as fam; $body }
            Ext::Barrage($st) => { use barrage as fam; $body }
            Ext::Cannon($st) => { use cannon as fam; $body }
            Ext::Avatar($st) => { use avatar as fam; $body }
            Ext::Rush($st) => { use rush as fam; $body }
            Ext::Ultima($st) => { use ultima as fam; $body } // (SHOT-C)
            Ext::Inception($st) => { use inception as fam; $body } // (SHOT-C)
            Ext::Laser($st) => { use laser as fam; $body }
            Ext::Gate($st) => { use gate as fam; $body }
            Ext::Burst($st) => { use burst as fam; $body } // (SHOT-B)
            Ext::Vacuum($st) => { use vacuum as fam; $body } // (SHOT-B)
            Ext::None => $none,
        };
        if ext != Ext::None {
            $sf.ext = ext;
        }
        out
    }};
}

/// Begin's per-kind set-up (0x1413A5CD0's switch: +0xB8 / +0xC0 / +0xC4) for every kind with a row, then the family's
/// state and its `begin`.
pub(crate) fn begin(sf: &mut ShootFlow, cx: &mut Ctx) {
    let kind = sf.kind;
    if family(kind) == "-" {
        return;
    }
    sf.proj_name = kind_text(kind, "proj_name");
    sf.group = S.num_or(&format!("kind.k{kind}.effect_group"), 0.0) as i32;
    sf.voice = S.num_or(&format!("kind.k{kind}.voice"), 0.0) as i32;
    if !owns(kind) {
        return;
    }
    sf.ext = match family(kind) {
        "drain" => Ext::Drain(drain::State::default()),
        "pellet" => Ext::Pellet(pellet::State::default()),
        "meteor" => Ext::Meteor(meteor::State::default()),
        "barrage" => Ext::Barrage(barrage::State::default()),
        "cannon" => Ext::Cannon(cannon::State::default()),
        "avatar" => Ext::Avatar(avatar::State::default()),
        "rush" => Ext::Rush(rush::State::default()),
        "ultima" => Ext::Ultima(ultima::State::default()), // (SHOT-C)
        "inception" => Ext::Inception(inception::State::default()), // (SHOT-C)
        "laser" => Ext::Laser(laser::State::default()),
        "gate" => Ext::Gate(gate::State::default()),
        "burst" => Ext::Burst(burst::State::default()),   // (SHOT-B)
        "vacuum" => Ext::Vacuum(vacuum::State::default()), // (SHOT-B)
        _ => Ext::None,
    };
    with_family!(sf, st => fam::begin(sf, st, cx), ())
}

/// The update delegate of an owned kind: `Some(true)` = PostTick follows, `Some(false)` = the state finished this tick,
/// `None` = not a kind of this module (the Kingdom Key's code runs).
pub(crate) fn tick(sf: &mut ShootFlow, cx: &mut Ctx, dt: f32) -> Option<bool> {
    if sf.ext == Ext::None {
        return None;
    }
    Some(with_family!(sf, st => fam::update(sf, st, cx, dt), true))
}

/// The family's part of End.
pub(crate) fn end(sf: &mut ShootFlow, cx: &mut Ctx) {
    with_family!(sf, st => fam::end(sf, st, cx), ())
}

/// The anim interface's handlers of the state for an owned kind (KH3: one handler per notify class for every kind):
/// Shot (slot 69, 0x1413B4A70), TresAtk begin (slot 64, 0x1413B5430), OnTiming (slot 105, 0x1413B57D0); then the
/// family's `on_notify` for every event. True when the kind is owned (the Kingdom Key's handler is skipped).
pub(crate) fn on_notifies(sf: &mut ShootFlow, cx: &mut Ctx, notifies: &Notifies) -> bool {
    if sf.ext == Ext::None {
        return false;
    }
    for event in &notifies.events {
        match (event.class(), event.phase) {
            (NotifyClass::TresAnimNotifyShot, NotifyPhase::Instant) => {
                let index = match event.def().data {
                    crate::anim::NotifyData::Shot { location_index, .. } => location_index.unwrap_or(0),
                    _ => 0,
                };
                shot_handler(sf, cx, index);
            }
            (NotifyClass::TresAnimNotifyStateAttack, NotifyPhase::Begin) => {
                // slot 64: flagsCC |= 1; kinds 4 / 0x2A: +0xA0 = 0.3, kind 0xB: 0.5.
                sf.ring_active = true;
                match sf.kind {
                    4 | 0x2A => sf.ring_timer = 0.3,
                    0x0B => sf.ring_timer = 0.5,
                    _ => {}
                }
            }
            (NotifyClass::TresAnimNotifyTiming, NotifyPhase::Instant) => match sf.kind {
                // slot 105: kinds 3 / 10: flagsCC |= 1; 0x2A: |= 4.
                3 | 10 => sf.ring_active = true,
                0x2A => sf.timing2 = true,
                _ => {}
            },
            _ => {}
        }
        with_family!(sf, st => fam::on_notify(sf, st, cx, event), ());
    }
    true
}

/// The Shot handler (anim interface slot 69, 0x1413B4A70) for `LocationIndex` `index`.
pub(crate) fn shot_handler(sf: &mut ShootFlow, cx: &mut Ctx, index: i32) {
    match sf.kind {
        // "shot notify seen": the update spawns.
        3 | 5 | 10 | 0x12 | 0x17 | 0x1D | 0x24 | 0x28 | 0x2B => sf.ring_active = true,
        8 => {}
        0x0F | 0x11 => {
            sf.ring_active = true;
            sf.syphon_timer += index as f32; // +0x9C += LocationIndex (the count field of those kinds; SHOT-B/C)
        }
        0x27 => {
            if sf.sub == 5 {
                sf.sub = 6; // (+ vt+0x538: SHOT-B/C)
            }
        }
        _ => spawn_shot(sf, cx, index),
    }
}

/// The default branch of the Shot handler: one projectile of `sf.proj_name` at muzzle `index` (< 2, else 0) along his
/// facing, homing on `locked[spawnIndex mod lockedTargets]` (a dead entry: none; no locked target: the first target);
/// kinds 6 / 0x15 fire a pair (the other hand next, both outward from his centre); kinds 7 / 0x16 keep the last one.
fn spawn_shot(sf: &mut ShootFlow, cx: &mut Ctx, index: i32) {
    if sf.spawn_index >= sf.n {
        return;
    }
    let Some(def) = projectile_named(cx, sf.proj_name) else { return };
    let pair = matches!(sf.kind, 6 | 0x15);
    let mut index = if index < 2 { index.max(0) as usize } else { 0 };
    let yaw = cx.pawn.yaw.to_radians();
    let mut position = muzzle(sf, cx, index);
    let mut direction = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
    if pair {
        direction = (position - super::shotlock::lock_point(cx)).normalize_or_zero();
    }
    let mut last = 0;
    for round in 0..if pair { 2 } else { 1 } {
        if round == 1 {
            index ^= 1;
            position = muzzle(sf, cx, index);
            direction = (position - super::shotlock::lock_point(cx)).normalize_or_zero();
        }
        let target = target_for(sf, cx, sf.spawn_index);
        last = projectile::spawn(cx, &SpawnArgs { def, position, direction, cmd: cmd::SHOOTFLOW, shoot_flow: sf.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target, attached: None, fire: true });
        cx.pawn.magic.volley.push(last);
        sf.spawn_index += 1;
    }
    if matches!(sf.kind, 7 | 0x16) {
        sf.kept = last;
    }
}

// ------------------------------------------------------------------------------------------------ shared helpers

/// The homing target of spawn number `i`: `locked[i mod lockedTargets]` when it lives (else none); without locked
/// targets the state's first target (+0xD0). (The spawner 0x1413C1350 and the Shot handler 0x1413B4A70.)
pub(crate) fn target_for(sf: &ShootFlow, cx: &Ctx, i: i32) -> TargetId {
    let locked = &cx.pawn.magic.aim.locked;
    if locked.is_empty() {
        return sf.first_target;
    }
    let t = locked[i.max(0) as usize % locked.len()].target;
    if cx.target(t).is_some_and(|info| info.flags.has(TargetFlags::ALIVE)) {
        t
    } else {
        TargetId::NONE
    }
}

/// The spawn point of weapon `index`'s muzzle (pawn vt+0x1410: the weapon's ATK_01): the family's own (form weapons),
/// else weapon 0's from the clip's socket track, else his lock-on point.
pub(crate) fn muzzle(sf: &ShootFlow, cx: &Ctx, index: usize) -> Vec3 {
    let own = match &sf.ext {
        Ext::Drain(st) => drain::muzzle(sf, st, cx, index),
        Ext::Pellet(st) => pellet::muzzle(sf, st, cx, index),
        Ext::Meteor(st) => meteor::muzzle(sf, st, cx, index),
        Ext::Barrage(st) => barrage::muzzle(sf, st, cx, index),
        Ext::Cannon(st) => cannon::muzzle(sf, st, cx, index),
        Ext::Avatar(st) => avatar::muzzle(sf, st, cx, index),
        Ext::Rush(st) => rush::muzzle(sf, st, cx, index),
        Ext::Ultima(st) => ultima::muzzle(sf, st, cx, index), // (SHOT-C)
        Ext::Inception(st) => inception::muzzle(sf, st, cx, index), // (SHOT-C)
        Ext::Laser(st) => laser::muzzle(sf, st, cx, index),
        Ext::Gate(st) => gate::muzzle(sf, st, cx, index),
        Ext::Burst(st) => burst::muzzle(sf, st, cx, index),   // (SHOT-B)
        Ext::Vacuum(st) => vacuum::muzzle(sf, st, cx, index), // (SHOT-B)
        Ext::None => None,
    };
    own.or_else(|| crate::states::fire_magic::muzzle(cx.pawn)).unwrap_or_else(|| super::shotlock::lock_point(cx))
}

/// The held Keyblade's projectile set (`projset.<w>.<name>`, the weapon of `projectile_set`) resolves `name` to a class
/// of sora-shotlocks (pawn vt+0x1450 -> equipment -> the projectile manager). `None`: no such projectile.
pub(crate) fn projectile_named(cx: &Ctx, name: &str) -> Option<StageDef> {
    if name == "-" || name.is_empty() {
        return None;
    }
    let set = cx.pawn.status.current_keyblade().map_or("w_so010", |id| id.row().projectile_set.rsplit('/').next().unwrap_or("").trim_end_matches("_ProjSet"));
    let class = S.text_or(&format!("projset.{set}.{}", name.to_ascii_lowercase()), "");
    if class.is_empty() {
        return None;
    }
    StageDef::shot_class(class)
}

/// The kind runs the Ragnarok update 0x1413B88F0 (clips M_1481 / M_1482, the other spin): kinds 2, 28, 31, 33, 35.
pub(crate) fn ragnarok_code(kind: u8) -> bool {
    kind_text(kind, "update") == "0x1413B88F0"
}

/// The volley's projectile for a kind of the Kingdom Key's code that names another class (Snakebite's MI_SHOT); `None`
/// for the Kingdom Key's own kinds (their `StageDef::shotlock`).
pub(crate) fn volley_def(cx: &Ctx, sf: &ShootFlow) -> Option<StageDef> {
    if matches!(sf.kind, 1 | 2) || family(sf.kind) != "volley" {
        return None;
    }
    projectile_named(cx, sf.proj_name)
}

/// The effect group a Play of the Kingdom Key's code passes for a kind that is not the Kingdom Key's (+0xC0: Snakebite
/// 0x7A3); `None` for kinds 1 / 2 (their effect is spawned by Begin, as before).
pub(crate) fn play_group(sf: &ShootFlow) -> Option<i32> {
    (!matches!(sf.kind, 1 | 2) && sf.group != 0).then_some(sf.group)
}

/// `Play(name, slot 0, blend -1, mode 2, +0xC0, rate 1)` of the state (G 2): the kind's effect group goes with it.
pub(crate) fn play(sf: &ShootFlow, cx: &mut Ctx, clip: AnimId) {
    if sf.group != 0 {
        cx.pawn.next_effect_group = Some(sf.group);
    }
    cx.pawn.anim.play_with(clip, PlayArgs::mode(2, -1.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::KeybladeId;

    #[test]
    fn the_kind_table_resolves_every_keyblade_s_shotlocks() {
        // Every shotlock the Keyblades list is a kind with a family and a name, and SelectKind's base / MAX kinds of
        // the Keyblade are exactly those (the abilities of m_ShootflowAbilitys name them).
        for id in KeybladeId::all() {
            let kinds = keyblade_kinds(id);
            let listed: Vec<&str> = id.shotlocks().to_vec();
            let named: Vec<&str> = kinds.iter().map(|&k| ability(k)).collect();
            for a in &listed {
                assert!(named.contains(a), "{}: {a} is none of SelectKind's kinds {kinds:?} ({named:?})", id.key());
            }
            for &k in &kinds {
                assert_ne!(family(k), "-", "kind {k}");
            }
        }
        // SHOT-A's Keyblades.
        let kinds = |key: &str| keyblade_kinds(KeybladeId::from_key(key).unwrap());
        assert_eq!(kinds("WEP_KEYBLADE_SO_00"), vec![1, 2]);
        assert_eq!(kinds("WEP_KEYBLADE_SO_01"), vec![3, 4]);
        assert_eq!(kinds("WEP_KEYBLADE_SO_02"), vec![5, 7, 6]);
        assert_eq!(kinds("WEP_KEYBLADE_SO_03"), vec![8, 9]);
        assert_eq!(kinds("WEP_KEYBLADE_SO_04"), vec![10, 11]);
        assert_eq!(kinds("WEP_KEYBLADE_SO_05"), vec![12, 13]);
    }

    #[test]
    fn every_shootflow_projectile_class_loads() {
        for (id, v) in Sheet::Shotlocks.rows() {
            if id.starts_with("proj.") && id.ends_with(".class") {
                let super::super::tables::PV::S(class) = v else { panic!("{id}") };
                let def = StageDef::shot_class(class).unwrap_or_else(|| panic!("{class}"));
                assert_eq!(def.class_name, *class);
                if def.respawn_enabled {
                    assert!(def.successor().is_some(), "{class}: successor {}", def.respawn_class);
                }
            }
        }
        // The Meteor Shower star is a TresProj_Ragnarok: the helix.
        assert_eq!(StageDef::shot("ss_shoot").unwrap().class, projectile::ProjClass::Ragnarok);
    }
}

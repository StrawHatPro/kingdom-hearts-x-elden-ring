//! (HM-MAGIC) KH3'S TURN-TYPE MAGIC SET `Blueprints/Player/p_ex001/p_ex001_hm_MagicSet`: Sora's magic while a formchange
//! whose weapon form names it as `m_ReplaceMagicSet` is held (Favorite Deputy's Hyper Hammer, Happy Gear's Agile Claws,
//! Crystal Snow's Blizzard Claws and Blizzard Blades, Wheel of Fate's Storm Flag, Classic Tone's Boom Hammer). The data is
//! `sheets/port/sora-hm-magic.json` (tools/sora_hm_magic.js); the evidence is build/sora_tmp/weapons/HM-MAGIC/STATE.md.
//!
//! How KH3 does it (re/decomp):
//! - THE SWAP. StartStyle 0x14141A9D0 -> ChangeWeapon -> UTresEquipmentComponent 0x141791120 installs the form entry's
//!   `m_ReplaceMagicSet` (+0x40 of the equip data's form entry; the port's `sora-keyblade-forms` column `magic_set`);
//!   EndStyle's weapon change back installs the Keyblade's own entry (null). The port holds the form in
//!   `combat.form.cmd` from `start_style` to `end_style`, so [`magic_set`] of that command IS the installed set.
//! - THE LOOKUP. pawn vt+0x1440 = 0x141780ED0 (and the class, vt+0x1438 = 0x141780F20): the replacement set's unit when
//!   it has one for the command, else `p_ex001_MagicSet`'s. The hm set has 30 units (every Fire / Blizzard / Thunder /
//!   Water / Aero tier and its `S_*` grand id) and no Cure: Cure, Cura, Curaga and Curagan stay the base set's.
//! - THE UNITS. Every unit is turn type (`m_bIsTurnType`, +4 bit 1), neither cast type nor direct-pos; effect groups and
//!   voices as the base set's except Blizzaga 1322 (base 1323) and Blizzagan 1323 (base 1325); the class is the
//!   `TresProj_Mag<tier>CL` actor (the ring around the form weapon).
//! - THE CAST (`states/fire_magic.rs`): Tick 0x1413794B0 plays `MagicTurn` / `MagicAirTurn` (`MagicSpin` / `MagicAirSpin`
//!   for a finish-end or grand cast) through the form's anim set; vt+0x3C0 (0x14136F320) clears the moving flag in weapon
//!   forms 3, 5, 6, 7, 12, 14, 15, 17, 18, 20, 21 ([`clears_moving`]); OnShot 0x14137BB30 hands a turn-type unit to
//!   0x141380470 ([`turn_shot`]).
//! - THE ACTOR (`projectile.rs`, [`stage`]): `TresProj_Mag*CL`: a static actor (speed 0, no homing) at the form weapon's
//!   ATK_01, its ring particle on its root, one sphere growing 150 + 800 / s up to 250 / 300 / 400 / 500 by tier, hitting
//!   every enemy inside again every `m_CollAutoActivate` interval (0.4 / 0.2 / 0.133 / 0.1 s: TimerTick 0x14062CFC0),
//!   for its life (0.8 s; Figa 1.2, Figan 1.5, Thundagan 1.067); not destroyed by a hit; no respawn
//!   (`m_bReSpawnProjectileOnTimeOut` false); Aero's swirl counter 3 / 8 / 14 / none, then BLOW.

use glam::Vec3;

use super::projectile::{ProjClass, RespawnCheck, Rolling, RotInherit, ShapeDef, ShapeKind, StageDef};
use super::spells::{self, SpellUnit};
use super::tables::Sheet;
use crate::anim::AnimId;
use crate::pawn::Ctx;

// The rows (sheets/port/sora-hm-magic.json): `HmMagicRow`, `HM_MAGIC`, `HMM_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_hm_magic.rs"));

/// `m_MagicSet` of `p_ex001_PawnBase`: the set every other form and the plain Keyblade use.
pub const MAIN_SET: &str = "/Game/Blueprints/Player/p_ex001/p_ex001_MagicSet.p_ex001_MagicSet";
/// The turn-type set (the forms' `m_ReplaceMagicSet`).
pub const HM_SET: &str = "/Game/Blueprints/Player/p_ex001/p_ex001_hm_MagicSet.p_ex001_hm_MagicSet";

fn rows(what: &'static str) -> impl Iterator<Item = &'static HmMagicRow> {
    HM_MAGIC.iter().filter(move |r| r.what == what)
}

/// The form code (`HM`, `CL` ...) of a held form command whose set is the hm set.
fn form_code(form_cmd: i32) -> Option<&'static str> {
    rows("form").find(|r| r.code == form_cmd).map(|r| r.form)
}

/// The magic set installed while the form `form_cmd` is held (0 = no form): [`HM_SET`] for the six forms whose weapon
/// form names it, else [`MAIN_SET`]. (The `_bz_` / `_dw_` replacement sets of the gun / launcher / drill / yo-yo forms are
/// not ported: those forms cast with the base set's actors.)
pub fn magic_set(form_cmd: i32) -> &'static str {
    if form_code(form_cmd).is_some() {
        HM_SET
    } else {
        MAIN_SET
    }
}

/// A unit of the hm set (0x141780ED0's first branch).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnUnit {
    pub cmd: u8,
    /// The unit key (`s_firagan`).
    pub key: &'static str,
    /// `m_AssetData00` (`TresProj_MagFiganCL_C`).
    pub class: &'static str,
    pub cast_type: bool,
    pub turn_type: bool,
    pub direct_pos: bool,
    pub effect_group: i32,
    pub voice: i32,
    pub voice_finish: i32,
}

/// The hm set's unit of `cmd` while `form_cmd` is held; `None` when the form does not install the set or the set has no
/// unit for the command (the Cure family).
pub fn turn_unit(form_cmd: i32, cmd: u8) -> Option<TurnUnit> {
    form_code(form_cmd)?;
    let r = rows("set").find(|r| r.code == i32::from(cmd))?;
    let v = |k: usize| r.values.get(k).copied().unwrap_or(0.0);
    Some(TurnUnit { cmd, key: r.logical, class: r.clip, cast_type: v(0) != 0.0, turn_type: v(1) != 0.0, direct_pos: v(2) != 0.0, effect_group: v(3) as i32, voice: v(4) as i32, voice_finish: v(5) as i32 })
}

/// pawn vt+0x1440 (0x141780ED0): the unit the cast state reads while `form_cmd` is held: the hm set's (its flags, effect
/// group and voices on the base unit's command data: element, tier, grand, the `Mp` column of TresCmdData) when it has
/// one, else the base set's.
pub fn unit(form_cmd: i32, cmd: u8) -> Option<SpellUnit> {
    let base = spells::unit(cmd)?;
    Some(match turn_unit(form_cmd, cmd) {
        Some(t) => SpellUnit { cast_type: t.cast_type, direct_pos: t.direct_pos, effect_group: t.effect_group, voice: t.voice, voice_finish: t.voice_finish, ..base },
        None => base,
    })
}

/// FireMagic vt+0x3A8 (0x14136F400: `m_bIsTurnType` of the unit): the class the turn-type shot spawns, `None` for a
/// unit that is not turn type (every unit of the base set).
pub fn turn_class(form_cmd: i32, cmd: u8) -> Option<&'static str> {
    turn_unit(form_cmd, cmd).filter(|t| t.turn_type).map(|t| t.class)
}

/// FireMagic vt+0x3C0 (0x14136F320): the weapon form of the held form is one whose casts are never moving casts (3, 5, 6,
/// 7, 12, 14, 15, 17, 18, 20, 21: Hyper Hammer, Magic Launcher, Drill Punch, Blizzard Blades, Agile Claws, Highwind,
/// Storm Flag, Hunny Launcher, Blizzard Claws, Boom Hammer, Clock Drill). No form: the Keyblade's form 1.
pub fn clears_moving(form_cmd: i32) -> bool {
    let Some(form) = crate::combat::form::row_of(form_cmd) else { return false };
    rows("code").find(|r| r.id == "code.move_clear_forms").is_some_and(|r| r.values.contains(&(form.weapon_form_value as f32)))
}

/// The clip of a turn-type cast (Tick 0x1413794B0): `MagicTurn` / `MagicAirTurn`, or `MagicSpin` / `MagicAirSpin` for a
/// finish-end or grand cast (`spin`), as the held form's anim set has it (the base set has none of the four).
pub fn turn_clip(form_cmd: i32, spin: bool, air: bool) -> Option<AnimId> {
    let logical = match (spin, air) {
        (false, false) => "MagicTurn",
        (false, true) => "MagicAirTurn",
        (true, false) => "MagicSpin",
        (true, true) => "MagicAirSpin",
    };
    crate::combat::forms::replaced(form_cmd, logical)
}

/// The Shot notify's time and the EnableChange time of a form's turn-type clip (`anim.<form>.<logical>`), s.
pub fn clip_times(form_cmd: i32, logical: &str) -> Option<(f32, f32)> {
    let form = form_code(form_cmd)?;
    rows("anim").find(|r| r.form == form && r.logical == logical).map(|r| (r.values.first().copied().unwrap_or(0.0), r.values.get(1).copied().unwrap_or(-1.0)))
}

/// pawn vt+0x1410(0) (0x1405AA790): the form weapon's ATK_01 now, in the world: the `muzzle.<form>.<sequence>` track of
/// the playing clip at its time (actor space, linear between frames, turned by his yaw). `None`: no track for the clip
/// (the caller takes the actor location, as 0x1405AA790 without a weapon).
pub(crate) fn muzzle(cx: &Ctx) -> Option<Vec3> {
    let form = form_code(cx.pawn.combat.form.cmd)?;
    let sequence = cx.pawn.anim.name.name();
    let row = rows("muzzle").find(|r| r.form == form && r.clip == sequence)?;
    let frames = usize::try_from(row.code).ok().filter(|f| *f > 0)?;
    let v = row.values;
    if v.len() < frames * 3 {
        return None;
    }
    let last = frames - 1;
    let f = if row.length > 0.0 && last > 0 { (cx.pawn.anim.time / row.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let at = |k: usize| v[i * 3 + k] + (v[j * 3 + k] - v[i * 3 + k]) * t;
    let local = Vec3::new(at(0), at(1), at(2));
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    Some(cx.pawn.cmc.location + Vec3::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos, local.z))
}

/// 0x141380470's target vector lift: T = RootComponent location + (0, 0, this).
pub fn target_lift() -> f32 {
    rows("code").find(|r| r.id == "code.turn_target_lift").and_then(|r| r.values.first().copied()).unwrap_or(100.0)
}

/// The `actor.<tier>CL` row of a class.
pub fn actor(class: &str) -> Option<&'static HmMagicRow> {
    rows("actor").find(|r| r.clip == class)
}

/// The blueprint of a `TresProj_Mag*CL` class as the life cycle of `projectile.rs` reads it (the sheet's `actor` row:
/// class defaults resolved through the chain, see tools/sora_hm_magic.js).
pub fn stage(class: &str) -> Option<StageDef> {
    let r = actor(class)?;
    let v = |k: usize| r.values.get(k).copied().unwrap_or(0.0);
    let name = |list: &'static [&'static str], k: usize| list.get(k).copied().filter(|n| *n != "-").unwrap_or("None");
    let shape = ShapeDef { kind: ShapeKind::Sphere, row: name(r.rows, 0), size: Vec3::new(v(1), 0.0, 0.0), inc_size: Vec3::new(v(2), 0.0, 0.0), inc_max_size: Vec3::new(v(3), 0.0, 0.0), scale: Vec3::ONE, inc_scale: Vec3::ZERO, inc_max_scale: Vec3::ZERO, offset: Vec3::new(v(6), v(7), v(8)), curve: None };
    // The native class adds nothing that runs for these blueprints (no put effect, no rail: m_GenerateRailLife 0, no
    // homing target for the Water steering, no tail effect); Aero's swirl counter does (notify 0x14147D060).
    let class_kind = match r.flags.first().copied() {
        Some("TresProj_MagicFire") => ProjClass::Fire,
        Some("TresProj_MagicBlizzard") => ProjClass::Blizzard,
        Some("TresProj_MagicAero") => ProjClass::Aero,
        _ => ProjClass::Generic,
    };
    let respawn_check = match r.flags.get(2).copied() {
        Some("TPR_TYPE_HIT_ANY") => RespawnCheck::HitAny,
        Some("TPR_TYPE_HIT_PAWN") => RespawnCheck::HitPawn,
        Some("TPR_TYPE_HIT_MAP") => RespawnCheck::HitMap,
        Some("TPR_TYPE_NO_HIT") => RespawnCheck::NoHit,
        _ => RespawnCheck::HitAlways,
    };
    Some(StageDef {
        family: r.id,
        prefix: "",
        class_name: r.clip,
        class: class_kind,
        life: v(0),
        max_distance: 0.0,
        no_destroy_on_hit: true,
        explode_on_time_out: false,
        stuck_time: v(9),
        finish_row: name(r.rows, 1),
        situation_row: name(r.rows, 2),
        put_effect: false,
        put_duration: 0.0,
        put_first: 0.0,
        put_xy: false,
        respawn_enabled: false,
        respawn_check,
        respawn_class: name(r.flags, 1),
        respawn_rot: RotInherit::Normal,
        take_target: false,
        take_hit_list: false,
        take_finish: false,
        initial_speed: 0.0,
        max_speed: 0.0,
        accel: 0.0,
        move_start_delay: 0.0,
        normal_check: false,
        normal_angle: 90.0,
        homing: false,
        homing_delay: 0.0,
        homing_min_dist: 0.0,
        turn_max: 0.0,
        turn_accel: 0.0,
        max_turn_max: 0.0,
        homing_max_angle: 0.0,
        root_radius: 0.0,
        shapes: [Some(shape), None],
        set_falling: false,
        swirl_limit: v(5) as i32,
        rail_height: 0.0,
        rail_life: 0.0,
        residue: false,
        cure: None,
        hit_effect: name(r.effects, 2),
        // The pawn-hit SE goes by the class (ACTOR_PAWN_HIT: the audio side's table of the CL classes).
        hit_sound: "",
        map_hit_effect: name(r.effects, 3),
        sheet: Sheet::Proj,
        rolling: Rolling::kc(),
        attach_to_hit_actor: false,
        explode_life: 0.0,
        delay_fire: (false, 0.0, 0.0),
        hit_interval: v(4),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magic::spells::cmd;

    #[test]
    fn the_set_and_its_forms() {
        // Six formchanges install it (Favorite Deputy, Happy Gear, Crystal Snow x2, Wheel of Fate, Classic Tone).
        let forms: Vec<i32> = rows("form").map(|r| r.code).collect();
        assert_eq!(forms, [0x81, 0x8A, 0x90, 0x85, 0x8D, 0x92]);
        for f in &forms {
            assert_eq!(magic_set(*f), HM_SET);
            assert!(clears_moving(*f));
            for (spin, air) in [(false, false), (false, true), (true, false), (true, true)] {
                assert!(turn_clip(*f, spin, air).is_some(), "form {f:#x}: a turn clip");
            }
        }
        // Second Form, the Kingdom Key (0) and a _bz_ form keep the base set.
        for f in [0, 0x94, 0x83] {
            assert_eq!(magic_set(f), MAIN_SET);
            assert!(turn_unit(f, cmd::FIRA).is_none());
        }
        // 30 units, all turn type, no Cure.
        assert_eq!(rows("set").count(), 30);
        assert!(rows("set").all(|r| r.values[..3] == [0.0, 1.0, 0.0]));
        assert!(turn_unit(0x81, cmd::CURE).is_none() && unit(0x81, cmd::CURE) == spells::unit(cmd::CURE));
        // Blizzaga / Blizzagan's groups differ from the base set's; Thunder is no longer cast type nor direct-pos.
        assert_eq!((unit(0x81, 35).unwrap().effect_group, unit(0, 35).unwrap().effect_group), (1322, 1323));
        assert_eq!((unit(0x81, 115).unwrap().effect_group, unit(0, 115).unwrap().effect_group), (1323, 1325));
        let t = unit(0x81, cmd::THUNDER).unwrap();
        assert!(!t.cast_type && !t.direct_pos && unit(0, cmd::THUNDER).unwrap().cast_type);
        assert_eq!(turn_class(0x92, 112), Some("TresProj_MagFiganCL_C"));
    }

    #[test]
    fn the_cl_actors() {
        for (class, life, cap, interval, swirl, row) in [
            ("TresProj_MagFiganCL_C", 1.5, 500.0, 0.2, -1, "MAG_C_FIGAN"),
            ("TresProj_MagBlizzaganCL_C", 0.8, 500.0, 0.1, -1, "MAG_C_BLIZZAGAN"),
            ("TresProj_MagThundaganCL_C", 1.066_666, 500.0, 0.133_333, -1, "MAG_C_THUNDAGAN"),
            ("TresProj_MagWataganCL_C", 0.8, 500.0, 0.1, -1, "MAG_C_WATAGAN"),
            ("TresProj_MagAeroganCL_C", 0.8, 500.0, 0.1, -1, "MAG_C_AEROGAN"),
            ("TresProj_MagAeroCL_C", 0.8, 250.0, 0.4, 3, "MAG_C_AERO"),
        ] {
            let s = stage(class).unwrap();
            let shape = s.shapes[0].unwrap();
            assert!((s.life - life).abs() < 1.0e-5 && (s.hit_interval - interval).abs() < 1.0e-5, "{class}: life {} interval {}", s.life, s.hit_interval);
            assert_eq!((shape.inc_max_size.x, s.swirl_limit, shape.row), (cap, swirl, row), "{class}");
            assert_eq!((shape.size.x, shape.inc_size.x, s.initial_speed, s.no_destroy_on_hit, s.respawn_enabled), (150.0, 800.0, 0.0, true, false));
            assert!(spells::attack_row(shape.row).is_some(), "{row} is a row of p_ex001_AttackData");
        }
        let fira = stage("TresProj_MagFiraCL_C").unwrap();
        assert_eq!((fira.finish_row, fira.situation_row), ("MAG_C_FIRA_FIN", "MAG_C_FIRA_S"));
        assert_eq!(stage("TresProj_MagAeroCL_C").unwrap().class, ProjClass::Aero);
        assert_eq!(stage("TresProj_MagBlizzaraCL_C").unwrap().hit_effect, "vm_blz_hit0");
    }
}

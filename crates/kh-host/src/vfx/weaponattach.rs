//! (FINISH-AUDIT2) THE ATTACH OBJECT OF AN AttachEffect NOTIFY ROW. KH3's `SQEX_AttachEffectNotifyData` (the notify's
//! `MetaAttachDataList`) names first the OBJECT the effect hangs on (`AttachType`: `SQEX_ATTACH_OBJECT_OWNER` = Sora's mesh,
//! `ATTACH_TO_WEAPON` = the weapon actor standing on the group's `SocketName`, `ATTACH_TO_RIGHT_HAND_WEAPON` /
//! `ATTACH_TO_LEFT_HAND_WEAPON` = the right / left hand's weapon), then, per `MetaAttachData`, the socket ON THAT OBJECT
//! (`SocketName`; None = the object's root component; a name the object's mesh lacks answers the component as UE's
//! `GetSocketLocation` does).
//!
//! kh-core sends one socket name per row (`EffectAttach::socket`: the row's own `SocketName`, else the group's) and keeps the
//! Second Form golden trace byte-identical, so the object is decided HERE, on the host side every renderer shares (kh-er's
//! effect step and the harness both run `EffectHost`): the row is found again by its effect, its socket name and the clip
//! Sora plays, and a weapon row's anchor becomes the weapon's own place:
//! - a form's unit on that hand / socket (`Form::weapon_units`, the transform units while STYLECHANGE runs): `kb:<model>:
//!   <mount>:<socket>` (kbweapon.rs: the unit's mount socket x the weapon's posed bone / socket, its root when it has none);
//! - the Keyblade in his hand (no form, or a form whose right-hand unit IS a Keyblade: Second Form, Light / Dark / Double
//!   Form's right hand): `wep:<socket>` (vfx.rs `socket_world`: the Keyblade's own socket on R_wep_skt, its root without one).
//!
//! Before this, every weapon row resolved on SORA's socket of the same name: Sonic Rave's `vw_lmt_snr_tuk00`
//! (ATTACH_TO_WEAPON at R_wep_skt, socket `vfx_root` of the Keyblade) sat on his kh_sk, at his feet, while he dashed above it.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

/// The attach object of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Object {
    /// `SQEX_ATTACH_OBJECT_OWNER` (and every other kind the host has no object for): Sora's mesh.
    Owner,
    /// `ATTACH_TO_RIGHT_HAND_WEAPON`.
    Right,
    /// `ATTACH_TO_LEFT_HAND_WEAPON`.
    Left,
    /// `ATTACH_TO_WEAPON`: the weapon standing on this socket of his (the group's `SocketName`).
    At(&'static str),
}

/// One row: the clip it is a notify of, its object and the socket on that object ("" = the object's root).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Row {
    pub sequence: &'static str,
    pub object: Object,
    pub socket: &'static str,
}

fn object_of(kind: &str, group_socket: &'static str) -> Object {
    match kind {
        k if k.ends_with("ATTACH_TO_RIGHT_HAND_WEAPON") => Object::Right,
        k if k.ends_with("ATTACH_TO_LEFT_HAND_WEAPON") => Object::Left,
        k if k.ends_with("ATTACH_TO_WEAPON") => Object::At(group_socket),
        _ => Object::Owner,
    }
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// Every AttachEffect row of every clip of Sora's (kh_core's generated `SEQUENCES`), by (effect base name in lower case,
/// the socket name kh-core sends for it: tools/gen_port.js `effect()`, the row's own SocketName else the group's).
pub(crate) fn rows() -> &'static HashMap<(String, String), Vec<Row>> {
    static ROWS: OnceLock<HashMap<(String, String), Vec<Row>>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let mut out: HashMap<(String, String), Vec<Row>> = HashMap::new();
        for sequence in kh_core::anim::SEQUENCES.iter() { // (PACKGEN) a run-time table in the release build (kh-gendata)
            for notify in sequence.notifies {
                let Some((_, json)) = notify.props.iter().find(|(name, _)| *name == "MetaAttachDataList") else { continue };
                let Ok(Value::Array(list)) = serde_json::from_str::<Value>(json) else { continue };
                for group in &list {
                    let kind = group.get("AttachType").and_then(Value::as_str).unwrap_or("");
                    let group_socket = leak(group.get("SocketName").and_then(Value::as_str).unwrap_or("None"));
                    let object = object_of(kind, group_socket);
                    for meta in group.get("MetaAttachData").and_then(Value::as_array).into_iter().flatten() {
                        let own = meta.get("SocketName").and_then(Value::as_str).filter(|s| *s != "None");
                        let sent = own.unwrap_or(group_socket);
                        let socket = leak(own.unwrap_or(""));
                        for effect in meta.get("EffectDataList").and_then(Value::as_array).into_iter().flatten().filter_map(|e| e.get("AttachEffect").and_then(Value::as_str)) {
                            out.entry((kh_vfx::base_name(effect).to_lowercase(), sent.to_string())).or_default().push(Row { sequence: sequence.name, object, socket });
                        }
                    }
                }
            }
        }
        out
    })
}

/// The row behind a spawn of `effect` at `socket` (the name kh-core sent) while Sora plays `playing`: the row of a playing
/// clip first; else, when every row of that pair is a weapon row, the first one. None: an OWNER row (or no row at all).
pub(crate) fn row_of(effect: &str, socket: &str, playing: &[&str]) -> Option<Row> {
    let rows = rows().get(&(effect.to_lowercase(), socket.to_string()))?;
    let row = rows.iter().find(|r| playing.contains(&r.sequence)).or_else(|| rows.iter().all(|r| r.object != Object::Owner).then(|| rows.first()).flatten())?;
    (row.object != Object::Owner).then_some(*row)
}

/// A unit string's hand, mount socket and model (`RWeapon@RIGHT_HAND:R_wep5_skt=/Game/.../w_so092_Wep`).
fn unit_parts(unit: &'static str) -> Option<(&'static str, Option<&'static str>, &'static str)> {
    let (_, rest) = unit.split_once('@')?;
    let (hand, rest) = rest.split_once(':').unwrap_or((rest, ""));
    let socket = rest.split('=').next().filter(|s| !s.is_empty() && *s != "None");
    let model = kh_core::status::keyblade::model_of(kh_core::status::keyblade::blueprint_of(unit));
    Some((hand, socket, model))
}

/// (FORMWEP-FIX) Whether `model` is EQUIPPED NOW as a unit the equipment attaches with NO socket (`Deform@RIGHT_HAND:None`,
/// Nano Arms' `None@RIGHT_HAND:None`, Storm Flag's `RWeapon@RIGHT_HAND:None`: UE's AttachToComponent with NAME_None is Sora's
/// mesh component itself, 0x14177E4E0): the held form's transform set while kh-core equips it (`FormEquip::Transform`, the
/// StyleChange), or a shotlock's own equip set. Such a unit's clips are authored in his mesh space (kh-er-render keyblade.rs
/// `place_part`, tests/formwep_grip.rs there): while it plays one of them its effects' `kb:` anchors stand on his component
/// (vfx.rs), as the renderer draws it, whatever mount the row names. The same model as a form's own weapon unit (Nano Arms'
/// w_so172 on R_wep_skt) keeps that socket.
pub(crate) fn on_mesh_now(sora: &kh_core::Sora, model: &str) -> bool {
    let socketless = |units: &[&'static str]| units.iter().filter_map(|u| unit_parts(u)).any(|p| p.1.is_none() && p.2.eq_ignore_ascii_case(model));
    let form = &sora.combat().form;
    let transform = form.equip == kh_core::combat::form::FormEquip::Transform && form.row().is_some_and(|r| socketless(r.transform_units));
    transform || socketless(sora.magic().shot_weapon_units)
}

/// The base models of the Keyblades (`w_so010`, `w_so150` ...): a unit of one of them is the Keyblade in his hand.
fn keyblade_models() -> &'static Vec<&'static str> {
    static MODELS: OnceLock<Vec<&'static str>> = OnceLock::new();
    MODELS.get_or_init(|| kh_core::status::keyblade::KeybladeId::all().map(|k| kh_core::status::keyblade::model_of(kh_core::status::keyblade::blueprint_of(k.row().blueprint))).collect())
}

/// The anchor of a weapon row: `kb:<model>:<mount>:<socket>` on a form unit, `wep:<socket>` on the Keyblade in his hand.
/// `units`: the held form's units (the transform units while STYLECHANGE runs); empty: no form.
pub(crate) fn anchor(row: Row, units: &[&'static str]) -> String {
    let parts: Vec<_> = units.iter().filter_map(|u| unit_parts(u)).collect();
    let pick = match row.object {
        Object::Right => parts.iter().find(|p| p.0 == "RIGHT_HAND"),
        Object::Left => parts.iter().find(|p| p.0 == "LEFT_HAND"),
        Object::At(socket) => parts
            .iter()
            .find(|p| p.1.is_some_and(|m| m.eq_ignore_ascii_case(socket)))
            .or_else(|| parts.iter().find(|p| p.0 == if socket.starts_with(['L', 'l']) { "LEFT_HAND" } else { "RIGHT_HAND" })),
        Object::Owner => None,
    }
    .or_else(|| parts.first());
    match pick {
        Some((hand, mount, model)) if !(*hand == "RIGHT_HAND" && keyblade_models().contains(model)) => {
            let mount = mount.unwrap_or(if *hand == "LEFT_HAND" { "L_wep_skt" } else { "R_wep_skt" });
            format!("kb:{model}:{mount}:{}", row.socket)
        }
        _ => format!("wep:{}", row.socket),
    }
}

/// The units of the weapon objects now: the held form's (`Form::weapon_units` at its grow level), its transform units while
/// STYLECHANGE runs; empty without a form.
pub(crate) fn units_now(sora: &kh_core::Sora, state: kh_core::StateId) -> Vec<&'static str> {
    let form = &sora.combat().form;
    match form.row().map(|r| r.transform_units).filter(|t| !t.is_empty() && state == kh_core::StateId::Stylechange) {
        Some(t) => t.to_vec(),
        None => form.weapon_units(),
    }
}

/// (FINISH-AUDIT2, the measurement's "before" pass only) true: vfx.rs `start` keeps every row on his own socket, as before
/// this fix (the decision is still made and measured). Never set by a host.
pub static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The host's decision at a spawn (vfx.rs `start`): the anchor of a spawn of `file` (the effect file, a host copy `BASE#..`
/// as its base) at the socket kh-core sent (`at`; None = his mesh) when its row is a weapon row, made with the clip and the
/// form of the spawn's step as KH3 attaches it once; None: his own socket.
pub(super) fn decide(file: &str, at: Option<&str>, s: &super::Scene) -> Option<String> {
    let sent = at.unwrap_or("None");
    if sent.contains(':') {
        return None;
    }
    let playing: Vec<&str> = s.out.anim_ticked.body_samples().iter().map(|b| b.0).collect();
    let base = file.split('#').next().unwrap_or(file);
    let row = row_of(base, sent, &playing)?;
    Some(anchor(row, &units_now(s.sora, s.out.state)))
}

/// One measured effect system of Sora's notifies: where the host has it vs where KH3's row attaches it.
#[derive(Clone, Debug)]
pub struct Measured {
    pub effect: String,
    /// The socket kh-core sent ("None": his mesh).
    pub sent: String,
    /// The attach object KH3's row names (`Owner`: his mesh; `Weapon`: a weapon actor).
    pub object: String,
    /// The anchor KH3's row asks for (`wep:..`, `kb:..`, or his socket).
    pub kh3_anchor: String,
    /// The system's attach point (world, cm) as the host has it, and KH3's (the row's object socket, its LocationOffset /
    /// RotationOffset in that frame, the core's offset in his actor frame).
    pub actual: glam::Vec3,
    pub expected: glam::Vec3,
    /// `ToAttach`: it follows its socket (measured every step); false: left where it started (measured at its spawn).
    pub follow: bool,
    /// A weapon row whose socket the weapon lacks, resolved on the weapon's ROOT (its grip): a fault of the census
    /// (the user 2026-11-07: "the misplaced effects should be coming from the keyblade tip").
    pub root_fallback: bool,
}

impl Measured {
    pub fn off_cm(&self) -> f32 {
        self.actual.distance(self.expected)
    }
}

impl super::EffectHost {
    /// (FINISH-AUDIT2) THE PLACEMENT MEASUREMENT: every live system of Sora's notifies (not a trail, not a Sora copy's) with
    /// its attach point now and KH3's: the row's attach OBJECT read from the notify data (`row_of`), its socket on that object
    /// (`anchor`) decided at its spawn (`decide`: KH3 attaches once), the same socket resolution and attach-data placement as
    /// the drawing. A following system is measured every step; one left in the world (`ToAttach` false) only in its spawn
    /// step (`fresh`: the systems started this step).
    pub fn measure_placements(&self, s: &super::Scene, fresh: &[kh_vfx::EffectId]) -> Vec<Measured> {
        let mut out = Vec::new();
        for live in &self.live {
            let Some((file, sent, kh3_anchor)) = &live.sent else { continue };
            if live.trail || live.inst.is_some() {
                continue;
            }
            let follow = !matches!(live.anchor, super::Anchor::World(_));
            if !follow && !fresh.contains(&live.id) {
                continue;
            }
            let Some((x, _)) = self.world.latest_nodes(live.id) else { continue };
            let base = file.split('#').next().unwrap_or(file);
            let offset = match &live.anchor {
                super::Anchor::Sora(_, o) => *o,
                _ => glam::Vec3::ZERO,
            };
            let mut at = self.socket_world(s, (kh3_anchor != "None").then_some(kh3_anchor.as_str()));
            let (sin, cos) = s.out.yaw.to_radians().sin_cos();
            at.position += glam::Vec3::new(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos, offset.z);
            let p = &live.placement;
            let expected = at.then(&kh_vfx::Xform { position: p.offset, rotation: kh_vfx::rotator(p.rotation.x, p.rotation.y, p.rotation.z), scale: p.scale }).position;
            let object = if kh3_anchor.starts_with("wep:") || (kh3_anchor.starts_with("kb:") && !sent.starts_with("kb:")) { "Weapon" } else { "Owner" };
            let root_fallback = self.falls_to_root(kh3_anchor);
            out.push(Measured { effect: base.to_string(), sent: sent.clone(), object: object.to_string(), kh3_anchor: kh3_anchor.clone(), actual: x.position, expected, follow, root_fallback });
        }
        out
    }

    /// Whether a Keyblade row ends on the Keyblade's grip for want of its socket (`wep:<socket>` with neither that socket nor
    /// a tip on the Keyblade in his hand). A form weapon's `kb:` bone its mesh lacks is KH3's own component root only when
    /// KH3's mesh lacks it too: build/sora_tmp/weapons/FINISH-AUDIT2/tools/weapon_sockets_check.js proves the pack has every
    /// socket / bone KH3's weapon meshes have for every weapon row and `kb:` anchor (0 port fallbacks; 6 names KH3's meshes
    /// lack themselves).
    fn falls_to_root(&self, anchor: &str) -> bool {
        let Some(inner) = anchor.strip_prefix("wep:") else { return false };
        let has = |n: &str| self.kb.as_ref().is_some_and(|k| k.sockets.contains_key(n)) || self.weapon_sockets.contains_key(n);
        !inner.is_empty() && !has(inner) && !has("vfx_top")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sonic Rave's thrust (M_1425 `vw_lmt_snr_tuk00`: ATTACH_TO_WEAPON at R_wep_skt, the Keyblade's socket `vfx_root`) is a
    /// weapon row on the Keyblade in his hand; its shockwave (`vw_lmt_snr_sho00`, OWNER) stays his.
    #[test]
    fn sonic_raves_thrust_hangs_on_the_keyblade() {
        let row = row_of("vw_lmt_snr_tuk00", "vfx_root", &["1425"]).expect("a weapon row");
        assert_eq!((row.object, row.socket), (Object::At("R_wep_skt"), "vfx_root"));
        assert_eq!(anchor(row, &[]), "wep:vfx_root");
        assert_eq!(anchor(row, &["RWeapon@RIGHT_HAND:R_wep_skt=/Game/Blueprints/Player/p_ex001/w_so010_Wep"]), "wep:vfx_root", "Second Form's unit is the Keyblade");
        assert!(row_of("vw_lmt_snr_sho00", "vfx_root", &["1425"]).is_none(), "an OWNER row stays on his socket");
    }

    /// A form's unit: the right-hand unit on its own mount (Highwind's w_so092 on R_wep5_skt), the left hand's for a left row.
    #[test]
    fn a_form_units_row_hangs_on_that_unit() {
        let units = ["RWeapon@RIGHT_HAND:R_wep1_skt=/Game/Blueprints/Player/p_ex001/w_so022_Wep", "LWeapon@LEFT_HAND:L_wep1_skt=/Game/Blueprints/Player/p_ex001/w_so023_Wep"];
        assert_eq!(anchor(Row { sequence: "x", object: Object::Right, socket: "vfx_top" }, &units), "kb:w_so022:R_wep1_skt:vfx_top");
        assert_eq!(anchor(Row { sequence: "x", object: Object::Left, socket: "" }, &units), "kb:w_so023:L_wep1_skt:");
        assert_eq!(anchor(Row { sequence: "x", object: Object::At("L_wep1_skt"), socket: "shaft" }, &units), "kb:w_so023:L_wep1_skt:shaft");
        assert_eq!(anchor(Row { sequence: "x", object: Object::At("R_wep_skt"), socket: "" }, &units), "kb:w_so022:R_wep1_skt:", "the right hand's unit");
    }

    /// The table holds every weapon row of the clips' MetaAttachDataList (328 on 2026-11-07; the AttachDataList rows of the
    /// attach data assets are their own code's: vfx.rs `attach_data_rows`).
    #[test]
    fn every_weapon_row_is_in_the_table() {
        let n = rows().values().flatten().filter(|r| r.object != Object::Owner).count();
        assert!(n >= 300, "{n} weapon rows");
    }
}

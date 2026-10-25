//! (ER party S4) DONALD, GOOFY AND THE TEAM PROPS DRAWN BY OUR RENDERER (build/sora_tmp/er/party/ER_PARTY_PLAN.md stream S4;
//! state note build/sora_tmp/er/party/IMPL_S4_STATE.md). Everything here is behind the cargo feature `party` and the key
//! `r.party` ([`set_enabled`]: 0 = no party draws and no party pipelines). Without a submission nothing of the party is
//! made or drawn: Sora's frame is the frame of the build without the party.
//!
//! WHAT A FRIEND GETS (gfx/party.rs), per draw exactly the paths Sora's draws take:
//! - his OWN translated KH3 shaders (`PACK/party/<npc>/materials/er/*.dxil`, kh-shaderc `--party`: the `_pl` point lights,
//!   the `_bl` blood layer, the `_hy2` / `_gb2` hybrid pair), his own material library, textures and skeleton (no retarget);
//! - his weapon (the staff / shield model of `PACK/party/weapons/<model>`, its materials are in his library);
//! - the early depth with the stencil, the hybrid G-buffer write, the glow mask (ER80), the light-shaft mask (ER80b), the
//!   contact / thrown shadow, the real shadow in the game's cascades (`r.shadow_real`), his motion vectors;
//! - the camp menu's panel pictures ([`set_party_portraits`]) and the game-over void ([`set_void_char`]);
//! - the team attacks' props (`PACK/party/team`: Donald Meteor's rocks, Flare Force's missiles) as the TEAM "member".
//!
//! The host hands its drawing over with [`submit_party`] once a frame (after `submit`); a list lapses half a second after
//! the latest call, as Sora's frames do.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use glam::Vec3;
use kh_anim::Trs;

use crate::{Body, PortraitFrame};

/// The party's draw ids (kh-partyhost `poser::DONALD` / `GOOFY`), and the team props' library.
pub const DONALD: u8 = 1;
pub const GOOFY: u8 = 2;
pub const TEAM: u8 = 3;

/// (RSF) The folder of a draw id from the pack's roster (`PACK/party/party.json`: `members[].member` -> `dir`): a character
/// added to the roster is drawn with no code naming him (every path of gfx/party.rs is per member).
pub fn dir_in_roster(pack: &std::path::Path, chara: u8) -> Option<String> {
    let text = std::fs::read_to_string(pack.join("party").join("party.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('﻿')).ok()?;
    json["members"].as_array()?.iter().find(|m| m["member"].as_u64() == Some(chara as u64)).and_then(|m| m["dir"].as_str().or(m["npc"].as_str())).filter(|d| !d.is_empty() && !d.contains(['/', '\\', '.'])).map(str::to_string)
}

/// The folder of a draw id under `PACK/party`.
pub fn dir_of(chara: u8) -> Option<&'static str> {
    match chara {
        DONALD => Some("n_ex001"),
        GOOFY => Some("n_ex002"),
        TEAM => Some("team"),
        _ => None,
    }
}

/// THE TEAM PROPS' OWN TURN (`PACK/party/team/props/props.json`, tools/party_team_pack.js: the blueprint's mesh component
/// rotation, a UE rotator pitch / yaw / roll): model id (`o_ex210_0`, `o_ex208_0`) -> the rotation that goes on top of the
/// projectile's actor rotation (kh-app party_draw.rs `team_props`: actor x relative). Empty without the file.
pub fn team_prop_turns(pack: &std::path::Path) -> std::collections::HashMap<String, glam::Quat> {
    let text = std::fs::read_to_string(pack.join("party").join("team").join("props").join("props.json")).unwrap_or_default();
    let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('﻿')).unwrap_or_default();
    json["props"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, p)| {
            let model = std::path::Path::new(p["glb"].as_str()?).file_stem()?.to_string_lossy().to_string();
            let r = &p["relative_rotation"];
            let (pitch, yaw, roll) = (r[0].as_f64().unwrap_or(0.0) as f32, r[1].as_f64().unwrap_or(0.0) as f32, r[2].as_f64().unwrap_or(0.0) as f32);
            // (kh-partyhost poser::ue_rotator: yaw about Z, then pitch about Y nose up, then roll about X)
            Some((model, glam::Quat::from_rotation_z(yaw.to_radians()) * glam::Quat::from_rotation_y(-pitch.to_radians()) * glam::Quat::from_rotation_x(-roll.to_radians())))
        })
        .collect()
}

/// (KITFX) A skinned team prop's joint matrix in its bind pose (PACK space): the world matrix of the skin's first joint node
/// in the glb (its parents' matrices / TRS, the exporter's `UE_to_glTF` root included) x that joint's inverse bind matrix,
/// which is what glTF skinning draws (kh-app's Bevy scene). None when the file has no skin or cannot be read.
pub fn prop_bind_joint(glb: &std::path::Path, inverse_bind: glam::Mat4) -> Option<glam::Mat4> {
    use glam::{Mat4, Quat, Vec3};
    let bytes = std::fs::read(glb).ok()?;
    let len = u32::from_le_bytes(bytes.get(12..16)?.try_into().ok()?) as usize;
    let json: serde_json::Value = serde_json::from_slice(bytes.get(20..20 + len)?).ok()?;
    let nodes = json["nodes"].as_array()?;
    let joint = json["skins"][0]["joints"][0].as_u64()? as usize;
    let f = |v: &serde_json::Value, i: usize, d: f32| v.get(i).and_then(serde_json::Value::as_f64).map_or(d, |x| x as f32);
    let local = |n: &serde_json::Value| -> Mat4 {
        if let Some(m) = n["matrix"].as_array().filter(|m| m.len() == 16) {
            let c: Vec<f32> = m.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect();
            return Mat4::from_cols_slice(&c);
        }
        let (t, r, s) = (&n["translation"], &n["rotation"], &n["scale"]);
        Mat4::from_scale_rotation_translation(Vec3::new(f(s, 0, 1.0), f(s, 1, 1.0), f(s, 2, 1.0)), Quat::from_xyzw(f(r, 0, 0.0), f(r, 1, 0.0), f(r, 2, 0.0), f(r, 3, 1.0)).normalize(), Vec3::new(f(t, 0, 0.0), f(t, 1, 0.0), f(t, 2, 0.0)))
    };
    let parent = |i: usize| nodes.iter().position(|n| n["children"].as_array().is_some_and(|c| c.iter().any(|x| x.as_u64() == Some(i as u64))));
    let mut world = local(nodes.get(joint)?);
    let (mut at, mut guard) = (joint, 0);
    while let Some(p) = parent(at) {
        world = local(&nodes[p]) * world;
        at = p;
        guard += 1;
        if guard > 64 {
            return None;
        }
    }
    Some(world * inverse_bind)
}

/// A friend's weapon: the model id (`w_do010` .. / `w_gu010` ..), its component in GAME space (the hand's socket, or the
/// thrown shield's flight), its bones relative to its component (empty: the reference pose; the shield's spin clip on
/// "wep" while it flies) and its opacity (0 = not drawn).
#[derive(Clone, Debug)]
pub struct PartyWeapon {
    pub model: String,
    pub to_world: Trs,
    pub component: Vec<Trs>,
    pub opacity: f32,
}

/// One character of the party this frame (contract: `CharDraw`).
#[derive(Clone, Debug)]
pub struct CharDraw {
    /// [`DONALD`], [`GOOFY`] or [`TEAM`] (the props only).
    pub chara: u8,
    /// His pose (kh-partyhost `PosedChar`: the mesh component in game space, local and component bones of HIS skeleton).
    pub body: Option<Body>,
    pub weapon: Option<PartyWeapon>,
    /// TEAM: the props in flight (model `o_ex210_0` / `o_ex208_0`, its component in game space; drawn in their bind pose).
    pub props: Vec<(String, Trs)>,
    /// His opaque pixels go out of the game's glow source and its light shafts (ER80 / ER80b), as Sora's.
    pub glow_mask: bool,
    /// The blood on him (S5's per-member blood; zero = none).
    pub blood: crate::blood::BloodLook,
}

/// (U5) WHO CARRIES THE HERO LIGHT when it is not Sora: the leader's feet and his weapon tip (with its opacity). The light
/// pass then places the hero light (and its darkness gate's probe) at him instead of at Sora.
#[derive(Clone, Copy, Debug)]
pub struct HeroCarrier {
    pub feet: Vec3,
    pub tip: Option<(Vec3, f32)>,
}

/// A friend's camp-menu picture (his UI actor's capture, lights and pose: `PortraitFrame` with HIS body).
#[derive(Clone, Debug)]
pub struct PartyPortrait {
    pub chara: u8,
    pub frame: PortraitFrame,
}

/// The fallen friend leader in the game-over void (his KO pose, his skeleton); the void's camera and lights are Sora's.
#[derive(Clone, Debug)]
pub struct VoidChar {
    pub chara: u8,
    pub body: Body,
}

static ENABLED: AtomicBool = AtomicBool::new(true);
static DRAWS: Mutex<Option<(Arc<Vec<CharDraw>>, Instant)>> = Mutex::new(None);
static PORTRAITS: Mutex<Option<(Arc<Vec<PartyPortrait>>, Instant)>> = Mutex::new(None);
static VOID_CHAR: Mutex<Option<(Arc<VoidChar>, Instant)>> = Mutex::new(None);
static HERO: Mutex<Option<(HeroCarrier, Instant)>> = Mutex::new(None);
/// Every draw id ever asked for (its assets are made once and kept).
static WANTED: Mutex<Vec<u8>> = Mutex::new(Vec::new());

fn want(chara: u8) {
    if let Ok(mut w) = WANTED.lock() {
        // (RSF) any drawn KH character: his pack folder is found by gfx/party.rs (built-in, else PACK/party/party.json)
        if !w.contains(&chara) && chara != 0 {
            w.push(chara);
        }
    }
}

/// `r.party` (default 1): 0 = nothing of the party is drawn and no party pipeline is made.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// (RSF2) `r.party_shared_level` (default 1): every drawn KH character at the frame's ONE light level (Sora's, else the leader's),
/// each with his own light; 0 = each at the level of his own probe (RSF2 round 2).
static SHARED_LEVEL: AtomicBool = AtomicBool::new(true);

pub fn set_shared_level(on: bool) {
    SHARED_LEVEL.store(on, Ordering::Relaxed);
}

pub fn shared_level() -> bool {
    SHARED_LEVEL.load(Ordering::Relaxed)
}

/// (ER party I) `r.party_probes` (default 1): each friend lit by his own steady light probe at his feet (gfx/party_probe.rs);
/// 0 = Sora's light on everyone (as before).
static PROBES: AtomicBool = AtomicBool::new(true);

pub fn set_probes(on: bool) {
    PROBES.store(on, Ordering::Relaxed);
}

pub fn probes() -> bool {
    PROBES.load(Ordering::Relaxed)
}

/// THE PARTY OF THIS FRAME (any thread; once a frame after [`crate::submit`]). An empty list draws nobody.
pub fn submit_party(draws: Vec<CharDraw>) {
    for d in &draws {
        want(d.chara);
    }
    if let Ok(mut g) = DRAWS.lock() {
        *g = Some((Arc::new(draws), Instant::now()));
    }
}

/// (LDI) THE MEMBERS TO HAVE READY (any thread, any time; cheap): their assets and pipeline states are made by the worker
/// from now on, as for a member drawn, and kept (also across loads): the party asks for its active members while the
/// loading screen is up, so they are ready when it lifts instead of after their first submission.
pub fn preload(charas: &[u8]) {
    for c in charas {
        want(*c);
    }
}

/// (RSF2, the offline harness only: it has no game-data read-back) a member's light probe reading at his feet, as the game's
/// read-back would give it (his own light; with it the frame-block levels of gfx/party.rs `prepare` are his).
#[doc(hidden)]
pub fn test_probe(chara: u8, light: crate::light::GameLight) {
    crate::gfx::test_party_probe(chara, light);
}

/// Nobody of the party is drawn until the next submission.
pub fn clear_party() {
    if let Ok(mut g) = DRAWS.lock() {
        *g = None;
    }
}

/// The camp menu's friend pictures of this frame (empty = none; lapses 150 ms after the latest call, as Sora's).
pub fn set_party_portraits(portraits: Vec<PartyPortrait>) {
    for p in &portraits {
        want(p.chara);
    }
    let off = portraits.is_empty();
    if let Ok(mut g) = PORTRAITS.lock() {
        *g = if off { None } else { Some((Arc::new(portraits), Instant::now())) };
    }
    // (off = off at once, as Sora's: the overlay's copies go in this very call)
    if off {
        crate::hud::set_party_pictures(Vec::new());
    }
}

/// The fallen friend leader in the void (None: Sora's copy, as before). Lapses half a second after the latest call.
pub fn set_void_char(chara: Option<VoidChar>) {
    if let Some(c) = &chara {
        want(c.chara);
    }
    if let Ok(mut g) = VOID_CHAR.lock() {
        *g = chara.map(|c| (Arc::new(c), Instant::now()));
    }
}

/// (U5) The hero light's carrier when it is not Sora (None: Sora, as before).
pub fn set_hero_carrier(carrier: Option<HeroCarrier>) {
    if let Ok(mut g) = HERO.lock() {
        *g = carrier.map(|c| (c, Instant::now()));
    }
}

pub(crate) fn draws_now() -> Option<Arc<Vec<CharDraw>>> {
    if !enabled() {
        return None;
    }
    let g = DRAWS.lock().ok()?;
    g.as_ref().filter(|(d, at)| !d.is_empty() && at.elapsed() < Duration::from_millis(500)).map(|(d, _)| d.clone())
}

pub(crate) fn portraits_now() -> Option<Arc<Vec<PartyPortrait>>> {
    if !enabled() {
        return None;
    }
    let g = PORTRAITS.lock().ok()?;
    g.as_ref().filter(|(_, at)| at.elapsed() < Duration::from_millis(150)).map(|(d, _)| d.clone())
}

pub(crate) fn void_char_now() -> Option<Arc<VoidChar>> {
    if !enabled() {
        return None;
    }
    let g = VOID_CHAR.lock().ok()?;
    g.as_ref().filter(|(_, at)| at.elapsed() < Duration::from_millis(500)).map(|(d, _)| d.clone())
}

pub(crate) fn hero_carrier_now() -> Option<HeroCarrier> {
    if !enabled() {
        return None;
    }
    let g = HERO.lock().ok()?;
    g.as_ref().filter(|(_, at)| at.elapsed() < Duration::from_millis(500)).map(|(c, _)| *c)
}

/// (ERT4) THE FRIEND LEADER'S DRAWN FEET, every frame he leads (None: Sora leads / no friend leader), whether or not he
/// carries the hero light. The hero light's carrier is set only while that light is wanted (dark places), so by day the
/// shared light level fell back to "the first member with a light" = Donald while Goofy led (ERT4's in-game run: Goofy
/// drawn at Donald's one-tap probe level, x735 -> x1604 with both standing still).
static LEAD_FEET: Mutex<Option<(Vec3, Instant)>> = Mutex::new(None);

/// (ERT4) Call once a frame (partydraw); it lapses as `set_hero_carrier`.
pub fn set_lead_feet(feet: Option<Vec3>) {
    if let Ok(mut g) = LEAD_FEET.lock() {
        *g = feet.map(|f| (f, Instant::now()));
    }
}

pub(crate) fn lead_feet_now() -> Option<Vec3> {
    if !enabled() {
        return None;
    }
    let g = LEAD_FEET.lock().ok()?;
    g.as_ref().filter(|(_, at)| at.elapsed() < Duration::from_millis(500)).map(|(f, _)| *f)
}

/// (ERT4) Where the frame's shared level is taken when Sora is not drawn: the friend leader's own feet, else the hero
/// light's carrier (the same man when it is lit), else None (the first member with a light of his own).
pub(crate) fn level_anchor(lead_feet: Option<Vec3>, carrier_feet: Option<Vec3>) -> Option<Vec3> {
    lead_feet.or(carrier_feet)
}

pub(crate) fn level_anchor_now() -> Option<Vec3> {
    level_anchor(lead_feet_now(), hero_carrier_now().map(|c| c.feet))
}

pub(crate) fn wanted() -> Vec<u8> {
    if !enabled() {
        return Vec::new();
    }
    WANTED.lock().map(|w| w.clone()).unwrap_or_default()
}

/// (for the log) set by gfx/party.rs once a second.
pub(crate) static STATUS: Mutex<String> = Mutex::new(String::new());

/// One line for the host's once-a-second log.
pub fn status() -> String {
    let s = STATUS.lock().map(|s| s.clone()).unwrap_or_default();
    format!("(ER party S4) party drawing {}: {}", if enabled() { "on" } else { "OFF (r.party = 0)" }, if s.is_empty() { "nothing asked yet".to_string() } else { s })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_party_lists_lapse_and_r_party_turns_everything_off() {
        let body = Body { component_to_world: Trs::IDENTITY, local: Vec::new(), component: Vec::new() };
        submit_party(vec![CharDraw { chara: DONALD, body: Some(body.clone()), weapon: None, props: Vec::new(), glow_mask: true, blood: Default::default() }]);
        assert!(draws_now().is_some_and(|d| d.len() == 1));
        assert!(wanted().contains(&DONALD));
        set_enabled(false);
        assert!(draws_now().is_none() && wanted().is_empty(), "r.party = 0: nothing");
        set_enabled(true);
        submit_party(Vec::new());
        assert!(draws_now().is_none(), "an empty list draws nobody");
        set_void_char(Some(VoidChar { chara: GOOFY, body }));
        assert!(void_char_now().is_some_and(|v| v.chara == GOOFY));
        set_void_char(None);
        assert!(void_char_now().is_none());
        assert_eq!(dir_of(TEAM), Some("team"));
        assert_eq!(dir_of(9), None);
        // (ERT4) the friend leader's feet reach the level anchor, carrier or not, and r.party = 0 drops them
        let feet = Vec3::new(300.0, 0.0, 0.0);
        set_hero_carrier(None);
        set_lead_feet(Some(feet));
        assert_eq!(level_anchor_now(), Some(feet), "by day (no carrier): the leader's feet");
        set_enabled(false);
        assert_eq!(level_anchor_now(), None);
        set_enabled(true);
        set_lead_feet(None);
        assert_eq!(level_anchor_now(), None, "Sora leads: no anchor");
    }
}

/// (KITFX) Flare Force's missile (o_ex208_0, skinned, its vertices in UE axes under the glb's `UE_to_glTF` root) in its bind
/// pose: its long axis (UE Y, the nose) must come out level in PACK space (glTF, Y up), not upright as the bare 0.01 scale
/// drew it in Elden Ring. Read from the test level's pack; skipped, saying so, without it.
#[cfg(test)]
mod kitfx_tests {
    #[test]
    fn the_flare_force_missile_lies_level_in_its_bind_pose() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build");
        let Some(glb) = [root.join("play/donpack/party/team/props/o_ex208_0.glb"), root.join("sora_pack/party/team/props/o_ex208_0.glb")].into_iter().find(|p| p.is_file()) else {
            eprintln!("no o_ex208_0.glb: skipped");
            return;
        };
        // its inverse bind matrix: the kh_sk joint's quarter turn undone (as the file holds it)
        let ibm = glam::Mat4::from_quat(glam::Quat::from_xyzw(-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2)).inverse();
        let m = super::prop_bind_joint(&glb, ibm).expect("a skinned prop");
        let nose = m.transform_vector3(glam::Vec3::new(0.0, 56.7, 0.0));
        assert!(nose.y.abs() < 1e-3, "level, not upright: {nose:?}");
        assert!((nose.length() - 0.567).abs() < 1e-3, "cm -> m: {nose:?}");
        let up = m.transform_vector3(glam::Vec3::Z * 10.0);
        assert!((up.y - 0.1).abs() < 1e-3, "UE up is pack up: {up:?}");
    }
}

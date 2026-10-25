//! (ER party S4) DONALD, GOOFY AND THE TEAM PROPS HANDED TO THE RENDERER (kh-er-render `party`), once a drawn frame, from
//! the party core (S1's poser), the leader body (S2's overlay), the staged friends of the unlock (S8), the
//! team's props (S7) and the friends' blood (S5). Plan: build/sora_tmp/er/party/ER_PARTY_PLAN.md stream S4; state note
//! build/sora_tmp/er/party/IMPL_S4_STATE.md. Cargo feature `party`, keys `party.er` (0 = nothing of the party) and
//! `r.party` (0 = the renderer draws nobody of the party and makes no party pipeline).
//!
//! - THE MEMBERS: every friend of `rt.current` posed at the frame's place between the two latest steps (`r.interp`), his
//!   weapon on its socket; Goofy's shield while it flies at the projectile (its spin clip `anims/1328.glb` on "wep"), no
//!   weapon while `weapon_hidden`; his rush pitch (`Team::mesh_pitch`) on his actor.
//! - THE LEADER (a friend): the body's pose laid over his own (`lead.overlay`: grabs, chests, actions). (LADDER2) The
//!   ladder jump is each one's own KH3 jump (kh-party's ladder state plays it): nothing to lay over.
//! - CUTSCENES (U6): only a friend leader, in the scene's figure, while `!lead.leader_hidden()`; the AI allies are hidden.
//! - THE HERO LIGHT (U5): at the friend leader's weapon tip while `lead.light` is him and the light is wanted.
//! - THE CAMP MENU's friend pictures (`camp`) and the game-over void's fallen friend (void.rs calls `void_char`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use glam::{Quat, Vec3};
use kh_anim::Trs;
use kh_er_render::party::{CharDraw, PartyPortrait, PartyWeapon, DONALD, GOOFY, TEAM};
use kh_host::party::PartyChara;
use kh_partyhost::poser::{draw_id, npc_of, PosedChar};

use crate::control::Control;
use crate::erlog;

/// The thrown shield's spin clips and the weapons' skeletons, by model (read once).
#[derive(Default)]
struct Cache {
    spin: HashMap<String, Option<kh_anim::Clip>>,
    skeletons: HashMap<String, Option<kh_anim::Skeleton>>,
    turns: Option<HashMap<String, Quat>>,
    told: bool,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
/// The main pack's folder (sora.rs at the host's start).
static PACK: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn set_pack(pack: &Path) {
    *PACK.lock().unwrap_or_else(|e| e.into_inner()) = Some(pack.to_path_buf());
}

fn track_at(clip: &kh_anim::Clip, bone: &str, time: f32) -> Option<Trs> {
    let track = clip.tracks.iter().find(|t| t.bone == bone)?;
    let mut out = Trs::IDENTITY;
    if let Some((a, b, f)) = track.translation.around(time) {
        out.translation = a.lerp(b, f);
    }
    if let Some((a, b, f)) = track.rotation.around(time) {
        out.rotation = a.slerp(b, f).normalize();
    }
    Some(out)
}

/// A friend's weapon from his posed character (on its socket), or None while it is not drawn.
fn weapon_of(posed: &PosedChar) -> Option<PartyWeapon> {
    let (model, root, visible) = posed.weapon.as_ref()?;
    visible.then(|| PartyWeapon { model: model.clone(), to_world: root.then(&posed.component_to_world), component: Vec::new(), opacity: 1.0 })
}

/// THE THROWN SHIELD (kh-party P3 `Party::thrown_shield`): at the projectile (between the two steps by `alpha`), turned
/// along its flight, its own spin clip on "wep" (2 s, looped), as kh-app party_draw.rs `thrown_shield`.
fn thrown(cache: &mut Cache, pack: &Path, model: &str, s: &kh_party::projectile::Projectile, alpha: f32, dt: f32, yaw: f32) -> PartyWeapon {
    let at = s.position - s.velocity * dt * (1.0 - alpha);
    let flight = if s.velocity.truncate().length() > 1.0 { s.velocity.y.atan2(s.velocity.x).to_degrees() } else { yaw };
    let to_world = Trs { translation: at, rotation: kh_partyhost::poser::ue_rotator(0.0, flight, 0.0), scale: Vec3::ONE };
    let dir = pack.join("party").join("weapons").join(model);
    let clip = cache.spin.entry(model.to_string()).or_insert_with(|| kh_anim::Clip::load(&dir.join("anims").join("1328.glb"), "M_1328").map_err(|e| erlog!("party: thrown shield {model}: no spin clip ({e})")).ok());
    let skeleton = cache.skeletons.entry(model.to_string()).or_insert_with(|| {
        let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
        kh_anim::Skeleton::from_manifest(&kh_anim::json::Json::parse(text.trim_start_matches('\u{feff}')).ok()?).ok()
    });
    let component = match (clip.as_ref(), skeleton.as_ref()) {
        (Some(c), Some(sk)) => match (track_at(c, "wep", s.age % 2.0), sk.index_of("wep")) {
            (Some(wep), Some(k)) => {
                let mut local = sk.reference_pose();
                local[k] = wep;
                kh_anim::pose::component_space(sk, &local)
            }
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    PartyWeapon { model: model.to_string(), to_world, component, opacity: 1.0 }
}

/// Goofy's rush pitch on his actor (kh-app `team_mesh_pitch`): his mesh turned about his feet in his actor frame.
fn pitch_actor(c2w: &mut Trs, pitch: Option<f32>) {
    let Some(p) = pitch.filter(|p| p.is_finite() && *p != 0.0) else { return };
    let mesh_yaw = Quat::from_rotation_z((-90.0f32).to_radians());
    c2w.rotation = (c2w.rotation * mesh_yaw.inverse() * Quat::from_rotation_y(-p.to_radians()) * mesh_yaw).normalize();
}

fn body_of(p: PosedChar) -> kh_er_render::Body {
    kh_er_render::Body { component_to_world: p.component_to_world, local: p.local, component: p.component }
}

fn blood_of(c: PartyChara) -> kh_er_render::blood::BloodLook {
    crate::blood::party::party_look(c).unwrap_or_default()
}

/// The weapon tip of a posed friend (game space): a socket of his weapon whose name says "top" / "tip", else 50 cm along
/// the weapon's own +X from its root (the hero light's place when he carries it, U5).
fn tip_of(core: &crate::party::glue::PartyCore, id: u8, w: &PartyWeapon) -> Vec3 {
    let socket = core.poser.members.get(&id).and_then(|m| m.weapon.as_ref()).and_then(|w| w.sockets.iter().find(|(n, _)| { let n = n.to_ascii_lowercase(); n.contains("top") || n.contains("tip") }).map(|(n, _)| n.clone()));
    if let Some(world) = socket.and_then(|s| core.poser.members.get(&id)?.weapon_socket_world(&s)) {
        return world.translation;
    }
    w.to_world.translation + w.to_world.rotation * Vec3::X * 50.0
}

/// (ZETTA round 5c) Donald's wand-tip socket (the test level feeds the same one: kh-app zetta_crumble.rs).
pub const ZETTA_TIP_SOCKET: &str = "vfx_call";

/// (ZETTA round 5c) Donald's wand tip (game space, cm) to Zettaflare's launch (kh-party member 0 = Donald); None stops it.
pub fn feed_zetta_tip(at: Option<Vec3>) {
    kh_party::donald::zettaflare::feed_tip(0, at);
}

/// ONCE A DRAWN FRAME (sora.rs, right after Sora's own `submit`): the party's list for the renderer.
#[allow(clippy::too_many_arguments)]
pub fn frame(slot: &mut crate::party::PartySlot, lead: &mut crate::party::leader::LeaderBody, control: &Control, alpha: f32, dt: f32, cutscene: bool) {
    kh_er_render::party::set_enabled(control.flag("r.party") != Some(false));
    kh_er_render::party::set_probes(control.flag("r.party_probes") != Some(false)); // (ER party I) each friend's own light probe
    kh_er_render::party::set_shared_level(control.flag("r.party_shared_level") != Some(false)); // (RSF2) one light level for every drawn KH character
    let Some(pack) = PACK.lock().ok().and_then(|p| p.clone()) else { return };
    let pack = pack.as_path();
    // (ZETTA round 5c) Zettaflare's launch starts at Donald's wand tip as drawn: his weapon's socket vfx_call, fed every frame
    // (UNTESTED IN GAME)
    feed_zetta_tip(slot.core().and_then(|core| core.poser.members.get(&draw_id(PartyChara::Donald))?.weapon_socket_world(ZETTA_TIP_SOCKET)).map(|t| t.translation));
    // (ZETTA 2026-10-23) his staff as drawn (Zettaflare's down leaves it where it fell) (UNTESTED IN GAME)
    kh_party::donald::zettaflare::feed_staff(0, slot.core().and_then(|core| core.poser.members.get(&draw_id(PartyChara::Donald))?.weapon_world()).map(|t| (t.translation, t.rotation)));
    let Some(core) = slot.core() else {
        kh_er_render::party::clear_party();
        kh_er_render::party::set_hero_carrier(None);
        kh_er_render::party::set_lead_feet(None); // (ERT4)
        kh_er_render::camfade::set_lead(None); // (NF) Sora leads (no party)
        crate::tarnfx::set_friend_leads(false); // (RSF round 18)
        return;
    };
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(Cache::default);
    let leader = lead.friend_lead().map(|_| lead.chara());
    let mut draws: Vec<CharDraw> = Vec::new();
    let mut carrier = None;
    let mut lead_feet = None; // (ERT4) the friend leader's drawn feet, lit or not (the shared light level's anchor)
    let glow_mask = control.flag("r.party_glow_mask") != Some(false);
    if cutscene || lead.cut_on {
        // (U6) a real-time cutscene: the friend leader alone, in the scene's figure (its own place), unless the cut hides him
        if let (Some(chara), false) = (leader, lead.leader_hidden()) {
            if let Some(mut posed) = core.poser.pose(draw_id(chara), alpha) {
                if lead.overlay(chara, &mut posed) {
                    lead_feet = Some(posed.component_to_world.translation); // (ERT4)
                    posed.weapon = None;
                    draws.push(CharDraw { chara: draw_id(chara), body: Some(body_of(posed)), weapon: None, props: Vec::new(), glow_mask, blood: blood_of(chara) });
                }
            }
        }
    } else if core.visible() {
        for v in &core.rt.current {
            let chara = kh_partyhost::runtime::party_chara(v.chara);
            if chara == PartyChara::Sora {
                continue;
            }
            let id = draw_id(chara);
            let Some(mut posed) = core.poser.pose(id, alpha) else { continue };
            // (LDI) never drawn at a place that is no place (a pose not made yet after a load)
            if !posed.component_to_world.translation.is_finite() {
                continue;
            }
            pitch_actor(&mut posed.component_to_world, core.rt.team.mesh_pitch(v.id).or_else(|| core.rt.party.pad_mesh_pitch(v.id))); // (ER party I, X3G) or the pad's own pitch (Goofy's solo Bombardier Dive), as kh-app
            if leader == Some(chara) {
                lead.overlay(chara, &mut posed);
                lead.walk_pose(chara, core.poser.members.get(&id), &mut posed); // (ANIMS) walked through an action: his own walk
                lead_feet = Some(posed.component_to_world.translation); // (ERT4)
            }
            let weapon = match core.rt.party.thrown_shield(v.id) {
                Some(s) => posed.weapon.as_ref().map(|(model, ..)| thrown(cache, pack, model, s, alpha, dt, v.yaw)),
                // (ZETTA 2026-10-23, the user: "Donald's wand should stay where it falls") lying on the ground: drawn there
                None if core.rt.party.weapon_ground(v.id).is_some() => {
                    let (at, rot) = core.rt.party.weapon_ground(v.id).unwrap_or_default();
                    posed.weapon.as_ref().map(|(model, ..)| PartyWeapon { model: model.clone(), to_world: Trs { translation: at, rotation: rot, scale: Vec3::ONE }, component: Vec::new(), opacity: 1.0 })
                }
                None if core.rt.party.weapon_hidden(v.id) => None,
                None => weapon_of(&posed),
            };
            if leader == Some(chara) && lead.light == Some(chara) && kh_er_render::pointlight::keyblade_wanted() {
                // (INT) the hero light at his DRAWN feet (between the steps, as his body and weapon tip)
                carrier = Some(kh_er_render::party::HeroCarrier { feet: posed.component_to_world.translation, tip: weapon.as_ref().map(|w| (tip_of(core, id, w), w.opacity)) });
            }
            draws.push(CharDraw { chara: id, body: Some(body_of(posed)), weapon, props: Vec::new(), glow_mask, blood: blood_of(chara) });
        }
        // (S7) the team attacks' props, with the models' own turn of props.json on top (kh-app's rule)
        let turns = cache.turns.get_or_insert_with(|| kh_er_render::party::team_prop_turns(pack));
        let props: Vec<(String, Trs)> = crate::party::team::props(&core.rt).iter().map(|p| {
            let mut t = crate::interp::prop_at(p.model, p.index, p.trs(), alpha); // (INT) between the steps (interp.rs `Props`)
            t.rotation = (t.rotation * turns.get(p.model).copied().unwrap_or(Quat::IDENTITY)).normalize();
            (p.model.to_string(), t)
        }).collect();
        if !props.is_empty() {
            draws.push(CharDraw { chara: TEAM, body: None, weapon: None, props, glow_mask, blood: Default::default() });
        }
    }
    // (S8) the staged friends of the unlock (Goofy at Leonine, Donald at the Finger Reader), every frame they are staged
    if !cutscene {
        for s in crate::unlock::stage_draws() {
            if s.chara == PartyChara::Sora {
                continue;
            }
            let posed = s.at(alpha); // (INT) between the steps, as the party's friends
            let weapon = weapon_of(&posed);
            draws.push(CharDraw { chara: draw_id(s.chara), body: Some(body_of(posed)), weapon, props: Vec::new(), glow_mask, blood: blood_of(s.chara) });
        }
    }
    if !cache.told && !draws.is_empty() {
        cache.told = true;
        erlog!("party: S4 DRAWS THE PARTY: {} character(s) this frame ({}); r.party = 0 switches the drawing off", draws.len(), draws.iter().map(|d| match d.chara { DONALD => "Donald", GOOFY => "Goofy", _ => "team props" }).collect::<Vec<_>>().join(", "));
    }
    kh_er_render::party::set_hero_carrier(carrier);
    kh_er_render::party::set_lead_feet(lead_feet); // (ERT4) the shared light level is the leader's by day too
    kh_er_render::camfade::set_lead(leader.map(draw_id)); // (NF) the camera fade's leader (the camera is inside him: he fades; never by the look line)
    crate::tarnfx::set_friend_leads(leader.is_some()); // (RSF round 18) a friend leads: the Tarnished's Lantern model hidden (tarnfx.rs)
    kh_er_render::party::submit_party(draws);
}

// ------------------------------------------------------------------------------------------------ the camp menu

/// A friend's camp copy (kh-partyhost `MenuPoser`: the camp clips with the face rows: the eyes open) and its UI actor.
struct Copy {
    chara: PartyChara,
    poser: kh_partyhost::poser::MenuPoser,
    data: kh_host::menu_portrait::PortraitData,
    sync: kh_host::menu_portrait::PictureSync,
    loops: u32,
    in_break: bool,
    random: u32,
    clock: f32,
}

// SAFETY: plain data (the animator), made on a thread of its own, then only used under the mutex.
unsafe impl Send for Copy {}

static COPIES: Mutex<Vec<Copy>> = Mutex::new(Vec::new());
static LOADING: Mutex<Vec<PartyChara>> = Mutex::new(Vec::new());
static SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn load_copy(pack: PathBuf, chara: PartyChara) {
    let _ = std::thread::Builder::new().name("kh-party-camp".into()).spawn(move || {
        let started = std::time::Instant::now();
        let npc = npc_of(draw_id(chara)).unwrap_or("n_ex001");
        let dir = pack.join("party").join(npc);
        let made = std::panic::catch_unwind(|| -> Result<Copy, String> {
            let text = std::fs::read_to_string(dir.join("manifest.json")).map_err(|e| e.to_string())?;
            let manifest: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
            let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir)?, kh_anim::Settings { leg_ik: false, ..kh_anim::Settings::default() })?;
            let mut poser = kh_partyhost::poser::MenuPoser::new(npc, animator, &manifest)?;
            poser.play_row("Idle_CM");
            let sora = kh_host::menu_portrait::PortraitData::load(&pack)?;
            let data = kh_host::menu_portrait::friend_data(&sora, &pack, npc);
            Ok(Copy { chara, poser, data, sync: Default::default(), loops: 0, in_break: false, random: 0x51ED_0000 ^ draw_id(chara) as u32, clock: 0.0 })
        });
        match made {
            Ok(Ok(copy)) => {
                erlog!("portrait: {}'s camp copy is read in {:.2} s (his UI actor's capture {:?}, his own {} spot light(s), the camp face rows)", chara.name(), started.elapsed().as_secs_f32(), copy.data.capture, copy.data.rig.spots.len());
                COPIES.lock().unwrap_or_else(|e| e.into_inner()).push(copy);
            }
            Ok(Err(e)) => erlog!("portrait: {}'s camp copy could not be read ({e}): his panel stays as the menu draws it", chara.name()),
            Err(_) => erlog!("portrait: PANIC while reading {}'s camp copy", chara.name()),
        }
    });
}

/// The idle break's count (kh-app menu_model.rs `draw`: 2 .. 5 loops).
fn next_count(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    2 + *state % 4
}

/// ONCE A HUD FRAME, after Sora's portrait (hud.rs): the friends' pictures on their camp panels (`panels`: the camp's top
/// panels by character, `member`: the party slot a sub screen shows; `camp`: the camp layer's counters, as Sora's).
#[allow(clippy::too_many_arguments)]
pub fn camp(control: &Control, movie: Option<&kh_ui::Movie>, root: &kh_ui::Matrix, output: (u32, u32), dt: f32, pack: Option<&Path>, panels: &[PartyChara], member: usize, camp: (u64, u64, bool)) {
    let off = control.flag("menu.portrait") == Some(false) || control.flag("menu.party_portraits") == Some(false);
    let Some(movie) = movie.filter(|_| !off) else {
        if SHOWN.swap(false, std::sync::atomic::Ordering::Relaxed) {
            kh_er_render::party::set_party_portraits(Vec::new());
            erlog!("menu: the friends' pictures OFF (the camp menu is gone)");
        }
        return;
    };
    let friends: Vec<(usize, PartyChara)> = if panels.is_empty() {
        Vec::new()
    } else {
        panels.iter().enumerate().filter(|(_, c)| **c != PartyChara::Sora).map(|(k, c)| (k, *c)).collect()
    };
    let mut copies = COPIES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(pack) = pack {
        let mut loading = LOADING.lock().unwrap_or_else(|e| e.into_inner());
        for (_, c) in &friends {
            if !copies.iter().any(|x| x.chara == *c) && !loading.contains(c) {
                loading.push(*c);
                load_copy(pack.to_path_buf(), *c);
            }
        }
    }
    let exposure = control.f32("menu.portrait.exposure").filter(|v| (0.05..=20.0).contains(v)).unwrap_or(kh_host::menu_portrait::MENU_EXPOSURE);
    let env = control.f32("menu.portrait.env").filter(|v| (0.0..=20.0).contains(v)).unwrap_or(1.0);
    let mut list = Vec::new();
    for copy in copies.iter_mut() {
        let Some((slot, _)) = friends.iter().find(|(_, c)| *c == copy.chara) else { continue };
        // his top panel, or the sub screen's picture while it shows him (the camp member of his slot)
        let target = copy.data.target;
        let panel = kh_host::menu_portrait::panel_at(movie, &kh_host::menu_portrait::top_panel(*slot), target, 1.0, 0.0).or_else(|| if member == *slot { kh_host::menu_portrait::PANELS[1..].iter().find_map(|p| kh_host::menu_portrait::panel_at(movie, p, target, 1.0, 0.0)) } else { None });
        let now = panel.and_then(|p| kh_host::menu_portrait::placement_of(&p, root, output));
        let Some(placed) = (if control.flag("menu.portrait.sync") == Some(false) { now } else { copy.sync.step(now, camp.0, camp.1, camp.2) }) else { continue };
        // the camp idle with its break (kh-app: 2 .. 5 idle loops), the face rows replayed with each body row
        let dt = dt.clamp(0.0, 0.1);
        if copy.poser.step(dt) {
            if copy.in_break {
                copy.in_break = false;
                copy.poser.play_row("Idle_CM");
            } else {
                copy.loops += 1;
                if copy.loops >= next_count(&mut copy.random) && copy.poser.play_row("IdleBreak_CM").is_some() {
                    copy.loops = 0;
                    copy.in_break = true;
                }
            }
        }
        copy.clock += dt;
        let pose = copy.poser.animator.pose().clone();
        let out = kh_host::menu_portrait::friend_out(&copy.data, placed, pose.local, pose.component, copy.clock);
        let frame = crate::portrait::frame_of(&copy.data, out, exposure, env);
        list.push(PartyPortrait { chara: draw_id(copy.chara), frame });
    }
    if !list.is_empty() && !SHOWN.swap(true, std::sync::atomic::Ordering::Relaxed) {
        erlog!("menu: the friends' pictures ON ({} panel(s))", list.len());
    }
    kh_er_render::party::set_party_portraits(list);
}

/// (void.rs, the marked line) THE FALLEN FRIEND LEADER IN THE VOID: `Some` = his KO pose drawn by the renderer (Sora's copy
/// is not handed over then); `None` = Sora's copy as before.
pub fn void_char(friend: Option<(PartyChara, Trs, kh_anim::Pose)>) -> bool {
    match friend {
        Some((chara, c2w, pose)) if chara != PartyChara::Sora => {
            kh_er_render::party::set_void_char(Some(kh_er_render::party::VoidChar { chara: draw_id(chara), body: kh_er_render::Body { component_to_world: c2w, local: pose.local, component: pose.component } }));
            true
        }
        _ => {
            kh_er_render::party::set_void_char(None);
            false
        }
    }
}

/// The payload goes away.
pub fn shutdown() {
    kh_er_render::party::clear_party();
    kh_er_render::party::set_party_portraits(Vec::new());
    kh_er_render::party::set_void_char(None);
    COPIES.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

#[cfg(test)]
mod tests {
    #[test]
    fn zetta_the_launch_is_fed_donalds_wand_tip() {
        // (ZETTA round 5c) the ER launch starts at his wand tip: fed as kh-party's Donald (member 0), the test level's socket
        assert_eq!(super::ZETTA_TIP_SOCKET, "vfx_call");
        let at = glam::Vec3::new(10.0, 20.0, 130.0);
        super::feed_zetta_tip(Some(at));
        assert_eq!(kh_party::donald::zettaflare::tip_of(kh_party::MemberId(0)), Some(at));
        super::feed_zetta_tip(None);
        assert_eq!(kh_party::donald::zettaflare::tip_of(kh_party::MemberId(0)), None);
    }

    use super::*;
    #[test]
    fn the_rush_pitch_turns_the_actor_about_his_feet() {
        let mut c2w = kh_partyhost::poser::component_to_world(Vec3::new(10.0, 20.0, 0.0), 30.0);
        let before = c2w;
        pitch_actor(&mut c2w, None);
        assert_eq!(c2w, before);
        pitch_actor(&mut c2w, Some(90.0));
        assert_eq!(c2w.translation, before.translation, "about his feet");
        // the actor's forward (yaw 30) turned up by 90: the mesh's +Y (its forward after the -90 mesh yaw) points up
        let fwd = c2w.rotation * Vec3::Y;
        assert!(fwd.z > 0.99, "{fwd:?}");
    }
}

//! (ER party S4) THE PARTY DRAWN IN THE EFFECTS HARNESS (examples/effects.rs `#[path]` module `party_draw`; S5's `--party` is its simulation): Donald and Goofy beside Sora,
//! posed by kh-partyhost's own poser (the anim-set rows of the test level, their weapons mounted), drawn by the renderer's
//! party path; their camp-menu pictures (their UI actors' captures and lights, the camp copies with the face clips), the game-over void with a fallen friend, the
//! team props.
//!
//! ```text
//! --friends [ROW_DONALD,ROW_GOOFY]   Donald and Goofy beside Sora (default IdleB,IdleB; a row with a speed walks a circle)
//! --friends-off                      the same, with r.party = 0 (must be Sora's picture of the build without the party)
//! --friends-at DX,DY,GX,GY           their places relative to Sora's start (cm; default -40,-150,-40,160)
//! --friends-glow 0                   their glow / shaft mask off (CharDraw::glow_mask)
//! --friends-camp                     the friends' camp pictures (overlay layer 6 shown empty; panels at the right)
//! --friends-void goofy|donald        the void of the game over with that friend's KO pose (BadStatStunLoop)
//! --friends-props                    the team props (Donald Meteor's rock, Flare Force's missile) in the air before Sora
//! --friends-blood X                  blood X (0..1) on both friends
//! --friends-hero                     the hero light carried by Donald's staff (U5: a friend leads, Sora is out of the party)
//! ```

use std::path::{Path, PathBuf};

use glam::{Quat, Vec3};
use kh_anim::Trs;
use kh_er_render::party::{CharDraw, PartyPortrait, PartyWeapon, VoidChar, DONALD, GOOFY, TEAM};
use kh_er_render::{Body, Camera, FrameInput, PortraitFrame};
use kh_partyhost::poser::{MenuPoser, PosedMember};

pub struct PartyHarness {
    pack: PathBuf,
    members: Vec<(u8, PosedMember, Vec3, f32)>,
    camp: Vec<(u8, MenuPoser, kh_host::menu_portrait::PortraitData, u32, bool)>,
    void: Option<(u8, PosedMember)>,
    props: bool,
    glow: bool,
    blood: f32,
    camp_layer_shown: bool,
    hero: bool,
}

fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{}: {e}", path.display()))
}

fn member(pack: &Path, id: u8, leg_ik: bool) -> Result<PosedMember, String> {
    let npc = kh_partyhost::poser::npc_of(id).ok_or("no member")?;
    let dir = pack.join("party").join(npc);
    let index = read_json(&pack.join("party").join("party.json"))?;
    let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(id as u64)).cloned().unwrap_or_default();
    let manifest = read_json(&dir.join("manifest.json"))?;
    let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir)?, kh_anim::Settings { leg_ik, ..kh_anim::Settings::default() })?;
    let mut m = PosedMember::new(id, animator, &manifest, &entry)?;
    // his default weapon, as S1 mounts the equipped one
    if let Some(def) = entry["default_weapon"].as_str().and_then(|w| m.weapon_def(w).ok()) {
        let wm = read_json(&pack.join("party").join("weapons").join(&def.model).join("manifest.json"))?;
        m.mount_weapon(&def, &wm)?;
    }
    Ok(m)
}

impl PartyHarness {
    pub fn from_args(args: &[String], pack: &Path, sora_at: Vec3, sora_yaw: f32) -> Option<PartyHarness> {
        let value = |name: &str| args.iter().position(|a| a == name).and_then(|k| args.get(k + 1)).cloned();
        let flag = |name: &str| args.iter().any(|a| a == name);
        let any = ["--friends", "--friends-off", "--friends-camp", "--friends-void", "--friends-props"].iter().any(|f| flag(f));
        if !any {
            return None;
        }
        kh_er_render::party::set_enabled(!flag("--friends-off"));
        let mut h = PartyHarness { pack: pack.to_path_buf(), members: Vec::new(), camp: Vec::new(), void: None, props: flag("--friends-props"), glow: value("--friends-glow").as_deref() != Some("0"), blood: value("--friends-blood").and_then(|v| v.parse().ok()).unwrap_or(0.0), camp_layer_shown: false, hero: flag("--friends-hero") };
        if flag("--friends") || flag("--friends-off") {
            let rows: Vec<String> = value("--friends").filter(|v| !v.starts_with("--")).map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_else(|| vec!["IdleB".into(), "IdleB".into()]);
            let at: Vec<f32> = value("--friends-at").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).filter(|v: &Vec<f32>| v.len() == 4).unwrap_or(vec![-40.0, -150.0, -40.0, 160.0]);
            let turn = Quat::from_rotation_z(sora_yaw.to_radians());
            // (CV1) `--friends-yaw D,G`: their yaws relative to Sora's (degrees; default 0,0: facing his way)
            let yaws: Vec<f32> = value("--friends-yaw").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).filter(|v: &Vec<f32>| v.len() == 2).unwrap_or(vec![0.0, 0.0]);
            for (k, id) in [DONALD, GOOFY].into_iter().enumerate() {
                match member(pack, id, false) {
                    Ok(mut m) => {
                        let row = rows.get(k).cloned().unwrap_or_else(|| "IdleB".into());
                        let place = sora_at + turn * Vec3::new(at[2 * k], at[2 * k + 1], 0.0);
                        m.position = (place, place);
                        let own_yaw = sora_yaw + yaws[k]; // (CV1)
                        m.yaw = (own_yaw, own_yaw);
                        match m.debug_pose(&row, 0.0) {
                            Ok(d) => m.debug = Some(d),
                            Err(e) => println!("party harness: {e}"),
                        }
                        println!("party harness: {} plays {row} at {place:?}", m.name);
                        h.members.push((id, m, place, own_yaw));
                    }
                    Err(e) => println!("party harness: member {id}: {e}"),
                }
            }
        }
        // (RSF) `--friends-extra ID,DX,DY[,YAW]`: A MOCKED EXTRA CHARACTER with draw id ID (no renderer code names it: its pack folder
        // comes from PACK/party/party.json), posed with Goofy's skeleton and rows (the test pack's folder for ID is a copy of
        // Goofy's), at DX, DY from Sora's start, YAW relative to Sora's
        if let Some(v) = value("--friends-extra") {
            let n: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if n.len() >= 3 {
                match member(pack, GOOFY, false) {
                    Ok(mut m) => {
                        let turn = Quat::from_rotation_z(sora_yaw.to_radians());
                        let place = sora_at + turn * Vec3::new(n[1], n[2], 0.0);
                        let own_yaw = sora_yaw + n.get(3).copied().unwrap_or(0.0);
                        (m.position, m.yaw) = ((place, place), (own_yaw, own_yaw));
                        let row = value("--friends").filter(|v| !v.starts_with("--")).and_then(|v| v.split(',').nth(1).map(str::to_string)).unwrap_or_else(|| "IdleB".into());
                        if let Ok(d) = m.debug_pose(&row, 0.0) {
                            m.debug = Some(d);
                        }
                        println!("party harness: (RSF) a mocked extra character, draw id {}, plays {row} at {place:?}", n[0] as u8);
                        h.members.push((n[0] as u8, m, place, own_yaw));
                    }
                    Err(e) => println!("party harness: the mocked extra character: {e}"),
                }
            }
        }
        // (RSF2) `--friends-probe FILE`: every member's light probe reading at his feet = the `light:` line of FILE (the game's
        // read-back does it in game; the harness has none): with `--game-light` of a frame without Sora (his probe empty), the
        // in-game state of a friend leading with Sora out of the party
        // (RSF2) `--shared-level 0`: each member at his own probe's level (r.party_shared_level 0)
        if value("--shared-level").as_deref() == Some("0") {
            kh_er_render::party::set_shared_level(false);
        }
        // (RSF2) FILE,FILE2,..: one per member in order (Donald, Goofy, the extra); one file: all of them
        if let Some(files) = value("--friends-probe") {
            let files: Vec<&str> = files.split(',').collect();
            for (k, (id, ..)) in h.members.iter().enumerate() {
                let file = files.get(k).or(files.last()).copied().unwrap_or_default();
                let text = std::fs::read_to_string(file).unwrap_or_default();
                match text.lines().find_map(kh_er_render::cgfield::parse_log_line) {
                    Some(l) => {
                        kh_er_render::party::test_probe(*id, l);
                        println!("party harness: (RSF2) member {id}'s probe reading from {file}");
                    }
                    None => println!("party harness: --friends-probe {file}: no light: line"),
                }
            }
        }
        if flag("--friends-camp") {
            let data = kh_host::menu_portrait::PortraitData::load(pack).ok();
            for id in [DONALD, GOOFY] {
                let npc = kh_partyhost::poser::npc_of(id).unwrap();
                let dir = pack.join("party").join(npc);
                let made = (|| -> Result<MenuPoser, String> {
                    let manifest = read_json(&dir.join("manifest.json"))?;
                    let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir)?, kh_anim::Settings { leg_ik: false, ..kh_anim::Settings::default() })?;
                    MenuPoser::new(npc, animator, &manifest)
                })();
                match (made, data.as_ref()) {
                    (Ok(mut copy), Some(d)) => {
                        copy.play_row("Idle_CM");
                        let friend = kh_host::menu_portrait::friend_data(d, pack, npc);
                        println!("party harness: {npc}'s camp copy, capture {:?}, spots {:?}", friend.capture, friend.rig.spots.iter().map(|s| s.position).collect::<Vec<_>>());
                        h.camp.push((id, copy, friend, 0, false));
                    }
                    (Err(e), _) => println!("party harness: {npc}'s camp copy: {e}"),
                    (_, None) => println!("party harness: no PACK/menu/menu.json"),
                }
            }
        }
        if let Some(who) = value("--friends-void") {
            let id = if who == "donald" { DONALD } else { GOOFY };
            match member(pack, id, false) {
                Ok(mut m) => {
                    m.position = (sora_at, sora_at);
                    m.yaw = (sora_yaw, sora_yaw);
                    if let Ok(d) = m.debug_pose("BadStatStunLoop", 0.0) {
                        m.debug = Some(d);
                    }
                    m.weapon = None;
                    h.void = Some((id, m));
                }
                Err(e) => println!("party harness: void member: {e}"),
            }
        }
        Some(h)
    }

    /// One harness step: the friends posed and the party handed over (after Sora's `submit`). `camera`: the harness's
    /// camera (the void uses it), `sora_feet`: where Sora stands now.
    pub fn frame(&mut self, step: usize, dt: f32, camera: Camera, sora_feet: Vec3) {
        let mut draws = Vec::new();
        for (id, m, place, yaw) in self.members.iter_mut() {
            m.step_debug(1, dt, &kh_anim::NoIkWorld);
            m.position = (*place, *place);
            m.yaw = (*yaw, *yaw);
            let posed = m.posed(1.0);
            let weapon = posed.weapon.as_ref().filter(|w| w.2).map(|(model, root, _)| PartyWeapon { model: model.clone(), to_world: root.then(&posed.component_to_world), component: Vec::new(), opacity: 1.0 });
            if *id == DONALD && self.hero {
                let tip = weapon.as_ref().map(|w| (w.to_world.translation + w.to_world.rotation * Vec3::X * 50.0, 1.0));
                kh_er_render::party::set_hero_carrier(Some(kh_er_render::party::HeroCarrier { feet: posed.component_to_world.translation, tip }));
            }
            draws.push(CharDraw { chara: *id, body: Some(Body { component_to_world: posed.component_to_world, local: posed.local, component: posed.component }), weapon, props: Vec::new(), glow_mask: self.glow, blood: kh_er_render::blood::BloodLook { body: self.blood, blade: 0.0, fresh: 1.0, seed: 3.0 } });
        }
        if self.props {
            let turns = kh_er_render::party::team_prop_turns(&self.pack);
            let spin = |model: &str| Quat::from_rotation_z(step as f32 * 0.05) * turns.get(model).copied().unwrap_or(Quat::IDENTITY);
            let at = |dx: f32, dy: f32, dz: f32| sora_feet + Vec3::new(dx, dy, dz);
            draws.push(CharDraw {
                chara: TEAM,
                body: None,
                weapon: None,
                props: vec![("o_ex210_0".into(), Trs { translation: at(260.0, -60.0, 150.0), rotation: spin("o_ex210_0"), scale: Vec3::ONE }), ("o_ex208_0".into(), Trs { translation: at(220.0, 90.0, 120.0), rotation: spin("o_ex208_0"), scale: Vec3::ONE })],
                glow_mask: self.glow,
                blood: Default::default(),
            });
        }
        kh_er_render::party::submit_party(draws);
        // ---- the camp pictures: two panels at the right of the 1280 x 720 picture, the camp layer shown (empty)
        if !self.camp.is_empty() {
            if !self.camp_layer_shown {
                kh_er_render::hud::set_options(kh_er_render::hud::HudOptions { on: true, gamma: false, linear_scale: 1.0, opacity: 1.0 });
                kh_er_render::hud::show_layer(6, (0, 0, 1280, 720), &vec![0u8; 1280 * 720 * 4], None);
                self.camp_layer_shown = true;
            }
            let mut list = Vec::new();
            for (k, (id, copy, data, loops, in_break)) in self.camp.iter_mut().enumerate() {
                if copy.step(dt) {
                    // the idle's break after three loops (kh-app: 2 .. 5 at random), then the idle again
                    if *in_break {
                        *in_break = false;
                        copy.play_row("Idle_CM");
                    } else {
                        *loops += 1;
                        if *loops >= 3 && copy.play_row("IdleBreak_CM").is_some() {
                            *loops = 0;
                            *in_break = true;
                        }
                    }
                }
                let pose = copy.animator.pose().clone();
                let (w, h) = (230.0f32, 230.0 * data.target.1 as f32 / data.target.0 as f32);
                let x = 780.0 + k as f32 * (w + 10.0);
                let placement = kh_host::menu_portrait::Placement { visible: (x as i32, 40, w as u32, h as u32), picture: (x, 40.0, w, h), alpha: 1.0 };
                let out = kh_host::menu_portrait::friend_out(data, placement, pose.local, pose.component, step as f32 * dt);
                let host = |v: Vec3| Vec3::new(v.y, v.z, v.x);
                let frame = PortraitFrame {
                    input: FrameInput { camera: Camera { right: host(out.right), up: host(out.up), forward: host(out.forward), position: host(out.eye) * 0.01, fov: out.fov_y, aspect: out.aspect, near: 0.3, far: 50.0 }, origin: Vec3::ZERO, clock: out.clock, body: Some(Body { component_to_world: out.component_to_world, local: out.local, component: out.component }), weapon: None, forms: Vec::new() },
                    rig: data.rig.clone(),
                    env_level: 1.0,
                    exposure: kh_host::menu_portrait::MENU_EXPOSURE,
                    khs: kh_look::KHS_MENU_PRESET.to_string(),
                    visible: out.visible,
                    picture: out.picture,
                    alpha: out.alpha,
                    before_layer: 7,
                    with_layer: 6,
                    clear: kh_host::menu_portrait::CLEAR,
                };
                list.push(PartyPortrait { chara: *id, frame });
            }
            kh_er_render::party::set_party_portraits(list);
        }
        // ---- the game over's void with the fallen friend (Sora's copy not drawn)
        if let Some((id, m)) = self.void.as_mut() {
            m.step_debug(1, dt, &kh_anim::NoIkWorld);
            m.position = (sora_feet, sora_feet);
            let posed = m.posed(1.0);
            let lift = Trs { translation: Vec3::new(0.0, 0.0, 30.0), ..Trs::IDENTITY };
            kh_er_render::party::set_void_char(Some(VoidChar { chara: *id, body: Body { component_to_world: posed.component_to_world.then(&lift), local: posed.local, component: posed.component } }));
            kh_er_render::set_void(Some(kh_er_render::VoidFrame {
                input: FrameInput { camera, origin: Vec3::ZERO, clock: step as f32 * dt, body: None, weapon: None, forms: Vec::new() },
                // (KH3's game-over lights, kh_host::gameover's normal set, around his feet as kh-er void.rs places them)
                lights: [([0.0, 0.0, 120.0], [3.0, 4.5, 5.0], 350.0, 2.0), ([0.0, 35.0, 100.0], [10.0, 10.0, 10.0], 105.0, 2.0), ([100.0, 180.0, -30.0], [0.8, 1.6, 2.0], 700.0, 4.0)].iter().map(|(p, c, r, e)| kh_look::PointLight { position: sora_feet + Vec3::new(0.0, 0.0, 30.0) + Vec3::from(*p), color: Vec3::from(*c), radius: *r, exponent: *e }).collect(),
                exposure: 1.0,
                khs: kh_host::gameover::KHS_PRESET.to_string(),
            }));
        }
    }
}

/// (ER party I) cargo also finds this file as an example of its own (it is `examples/effects.rs`'s `#[path]` module):
/// `cargo test --workspace` builds every example, so it needs a `main` (it does nothing; run `effects --friends`).
#[allow(dead_code)]
fn main() {}

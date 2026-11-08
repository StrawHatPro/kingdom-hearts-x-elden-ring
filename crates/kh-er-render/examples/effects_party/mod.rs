//! (ER party S5) THE PARTY IN THE EFFECTS HARNESS (`examples/effects.rs --party SCRIPT`): Donald and Goofy beside Sora
//! (kh-party's `Party` with the test level's level-99 loadout, no AI: the script commands them), posed from their packs
//! (kh-partyhost `PosedMember`, the bones / sockets / `wep:` sockets the ER host feeds the effect side), their sounds and
//! effects on the SAME seam queues as Sora's, and the effect side fed as kh-er's fx.rs feeds it (`EffectHost::set_party`:
//! the members' poses and the spell actors). They are not drawn themselves (S4's drawing): their effects are.
//!
//! SCRIPT: `T WHO ACTION; ...` (T in seconds from the script's start; WHO = donald | goofy; ACTION = fire | blizzard |
//! thunder | aero | cure | beat | swing | jump | tornado | turbo | discus | push | sweep). Cure aims at the other friend.
//! Every second, and whenever an action ends, `party loops:` prints the running loops (looping sounds by handle, the
//! party's persistent effects and endless one-shots): after each move it must say 0.
//! (party VF) `T WHO fx NAME [loop]`: spawns the effect NAME on WHO's vfx_root (a party handle; `loop` = persistent, ended
//! at the next `T WHO fx-end`), to look at one party effect on its own (the Fusion effects of Goofy's kit / team attacks).

use std::collections::HashMap;
use std::path::Path;

use glam::Vec3;
use kh_core::host::party::is_party_handle;
use kh_host::seam::{AudioCommand, AudioQueue, EffectQueue};
use kh_host::vfx::{EffectHost, PartyMemberFx};
use kh_party::{ActionDef, Chara, MemberCommand, MemberId, Party, PartyConfig, PartyHost, SoraView, TargetRef};

/// (HEROLIGHT-KB) `PartyRig::hero_carrier`: member id, weapon model, feet, the light's point on the weapon and its opacity.
pub type HeroFriend = (u8, String, Vec3, Option<(Vec3, f32)>);

/// One member's row of `PartyRig::audio`.
type MemberAudio = (u8, u64, bool, (String, f32), String);

/// The step's seam queues.
pub struct Queues<'a> {
    pub audio: &'a mut AudioQueue,
    pub effects: &'a mut EffectQueue,
}

/// Sora's skeleton and pose of the step, and his component-to-world.
pub struct PosedBody<'a> {
    pub skeleton: &'a kh_anim::Skeleton,
    pub pose: &'a kh_anim::Pose,
    pub to_world: &'a kh_anim::Trs,
}

pub struct PartyRig {
    pub party: Party,
    posers: HashMap<u8, kh_partyhost::PosedMember>,
    script: Vec<(f32, Chara, ActionDef)>,
    next: usize,
    taps: Vec<(u8, PartyMemberFx)>,
    /// The members' audio state of the step: (member, tap step, grounded, anim row + time, weapon).
    audio: Vec<MemberAudio>,
    loop_sounds: HashMap<u32, &'static str>,
    busy: HashMap<u8, bool>,
    pub said: Vec<String>,
    pub casts: Vec<(usize, String)>,
    /// (party VF) the script's effect spawns: (T, member, name or "" = end the member's loops, persistent)
    fx: Vec<(f32, u8, &'static str, bool)>,
    fx_next: usize,
    fx_live: Vec<(u8, u32)>,
    /// (ZETTA) the script's debug words: (T, word, argument): `T donald cast ID` (a party-don-spells row id: Donald casts it at
    /// the nearest foe, kh_party::donald::cast debug_casts) and `T donald zetta-ready` (Zettaflare's condition met now)
    words: Vec<(f32, &'static str, String)>,
    /// (PARTY-WEAPONS) the pack (the weapons' manifests for `equip`)
    pack: std::path::PathBuf,
    /// (PARTY-WEAPONS) each member's speed-matched loop clocks (kh-partyhost `pose_loco`)
    loco_time: HashMap<u8, [f32; 3]>,
}

fn action(chara: Chara, word: &str) -> Option<ActionDef> {
    Some(match (chara, word) {
        (Chara::Donald, "fire") => ActionDef::DonaldFire,
        (Chara::Donald, "blizzard") => ActionDef::DonaldBlizzard,
        (Chara::Donald, "thunder") => ActionDef::DonaldThunder,
        (Chara::Donald, "aero") => ActionDef::DonaldAero,
        (Chara::Donald, "cure") => ActionDef::DonaldCure,
        (Chara::Donald, "beat") => ActionDef::DonaldBeat,
        (Chara::Donald, "swing") => ActionDef::DonaldSwing,
        (Chara::Donald, "jump") => ActionDef::DonaldJump,
        (Chara::Goofy, "tornado") => ActionDef::GoofyTornado,
        (Chara::Goofy, "turbo") => ActionDef::GoofyTurbo,
        (Chara::Goofy, "discus") => ActionDef::GoofyDiscus,
        (Chara::Goofy, "push") => ActionDef::GoofyPush,
        (Chara::Goofy, "sweep") => ActionDef::GoofySweep,
        (Chara::Goofy, "jump") => ActionDef::GoofyJump,
        _ => return None,
    })
}

impl PartyRig {
    /// Donald at Sora's left, Goofy at his right (KH3 cm), facing the dummy (+X); their posers from `PACK/party`.
    pub fn new(pack: &Path, script: &str) -> Result<PartyRig, String> {
        let mut list = Vec::new();
        let mut fx = Vec::new(); // (party VF)
        let mut words: Vec<(f32, &'static str, String)> = Vec::new(); // (ZETTA)
        for part in script.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            let w: Vec<&str> = part.split_whitespace().collect();
            let (Some(t), Some(who), Some(what)) = (w.first().and_then(|t| t.parse::<f32>().ok()), w.get(1), w.get(2)) else { return Err(format!("--party: \"{part}\" is not \"T WHO ACTION\"")) };
            let chara = match *who {
                "donald" => Chara::Donald,
                "goofy" => Chara::Goofy,
                _ => return Err(format!("--party: no one called {who}")),
            };
            // (party VF) an effect by name
            if *what == "fx" || *what == "fx-end" {
                let name: &'static str = if *what == "fx" { Box::leak(w.get(3).ok_or_else(|| format!("--party: \"{part}\": fx NAME"))?.to_string().into_boxed_str()) } else { "" };
                fx.push((t, if chara == Chara::Donald { 0 } else { 1 }, name, w.get(4) == Some(&"loop")));
                continue;
            }
            // (FREEZE-FX) `T WHO freeze SECONDS`: WHO takes a FREEZE row's hit (the Mimic copy's Blizzard on the party): his own
            // KH3 freeze state, KH3's frozen look on him by the shared route (kh-party freeze_fx)
            if *what == "freeze" {
                words.push((t, if chara == Chara::Donald { "freeze-donald" } else { "freeze-goofy" }, w.get(3).map(|s| s.to_string()).unwrap_or_else(|| "2".to_string())));
                continue;
            }
            // (ZETTA) the debug words
            if *what == "cast" || *what == "zetta-ready" {
                words.push((t, if *what == "cast" { "cast" } else { "zetta-ready" }, w.get(3).map(|s| s.to_string()).unwrap_or_default()));
                continue;
            }
            // (PARTY-WEAPONS) `T WHO battle` / `T WHO peace`: the party's battle flag on / off (the host's engaged count in kh-er);
            // `T WHO equip KEY`: WHO's weapon (an item key WEP_DONALD_03 / a model w_gu050), mounted on his poser too
            // (PARTY-WEAPONS) `T WHO hold` / `T WHO release`: WHO's weapon held out (the hero light's hold); `T WHO walk DX,DY`: WHO
            // walks (KH3's Move1 run) to his place + (DX, DY) cm
            if matches!(*what, "battle" | "peace" | "equip" | "hold" | "release" | "walk") {
                let don = chara == Chara::Donald;
                let word = match *what {
                    "battle" => "battle",
                    "peace" => "peace",
                    "hold" => if don { "hold-donald" } else { "hold-goofy" },
                    "release" => if don { "release-donald" } else { "release-goofy" },
                    "walk" => if don { "walk-donald" } else { "walk-goofy" },
                    _ => if don { "equip-donald" } else { "equip-goofy" },
                };
                words.push((t, word, w.get(3).map(|s| s.to_string()).unwrap_or_default()));
                continue;
            }
            let def = action(chara, what).ok_or_else(|| format!("--party: {who} has no action {what}"))?;
            list.push((t, chara, def));
        }
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        fx.sort_by(|a: &(f32, u8, &'static str, bool), b| a.0.total_cmp(&b.0)); // (party VF)
        let mut party = Party::new(PartyConfig::test_level99());
        party.spawn(Chara::Donald, Vec3::new(60.0, -220.0, 0.0), 0.0);
        party.spawn(Chara::Goofy, Vec3::new(60.0, 220.0, 0.0), 0.0);
        party.gof.kh3_ai = true; // (GF-AI) the script plays his KH3 actions
        let index: serde_json::Value = std::fs::read_to_string(pack.join("party").join("party.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null);
        let mut posers = HashMap::new();
        for (member, id) in [(0u8, kh_partyhost::poser::DONALD), (1u8, kh_partyhost::poser::GOOFY)] {
            let npc = kh_partyhost::poser::npc_of(id).ok_or("no npc")?;
            let dir = pack.join("party").join(npc);
            let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).map_err(|e| format!("{npc}: {e}"))?).map_err(|e| e.to_string())?;
            let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(id as u64)).cloned().unwrap_or(serde_json::Value::Null);
            let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir)?, kh_anim::Settings::default())?;
            let mut posed = kh_partyhost::PosedMember::new(id, animator, &manifest, &entry)?;
            // the equipped weapon (its sockets: the staff's vfx_call, the shield's)
            if let Some(weapon) = party.status(MemberId(member)).map(|s| s.weapon).filter(|w| !w.is_empty()) {
                if let Ok(def) = posed.weapon_def(weapon) {
                    let wm: Option<serde_json::Value> = std::fs::read_to_string(pack.join("party").join("weapons").join(&def.model).join("manifest.json")).ok().and_then(|t| serde_json::from_str(&t).ok());
                    if let Some(wm) = wm {
                        let _ = posed.mount_weapon(&def, &wm);
                    }
                }
            }
            posers.insert(member, posed);
        }
        Ok(PartyRig { party, posers, script: list, next: 0, taps: Vec::new(), audio: Vec::new(), loop_sounds: HashMap::new(), busy: HashMap::new(), said: Vec::new(), casts: Vec::new(), fx, fx_next: 0, fx_live: Vec::new(), words /* (ZETTA) */, pack: pack.to_path_buf() /* (PARTY-WEAPONS) */, loco_time: HashMap::new() })
    }

    /// ONE STEP after Sora's (as kh-er: the party's step on the same seams, then the poses).
    pub fn step(&mut self, step: usize, dt: f32, sora: &kh_core::Output, world: &dyn kh_core::World, targets: &mut dyn kh_core::Targets, queues: Queues) {
        let Queues { audio, effects } = queues;
        let now = step as f32 * dt;
        while let Some((t, chara, def)) = self.script.get(self.next).copied() {
            if t > now {
                break;
            }
            self.next += 1;
            let Some(id) = self.party.find(chara) else { continue };
            let target = match def {
                ActionDef::DonaldCure => self.party.find(Chara::Goofy).map_or(TargetRef::None, TargetRef::Member),
                _ => TargetRef::Enemy(kh_core::TargetId(1)),
            };
            if def == ActionDef::DonaldCure {
                // a Cure wants someone hurt: Goofy at half his HP
                if let Some(g) = self.party.find(Chara::Goofy).and_then(|g| self.party.status_mut(g)) {
                    g.hp = g.max_hp() / 2;
                }
            }
            self.party.command(id, MemberCommand::Action { def, target });
            self.casts.push((step, format!("{:?} {:?}", chara, def)));
            println!("party: step {step}: {chara:?} {def:?} at {target:?}");
        }
        // (party VF) the script's effects, on the member's vfx_root (handles from the top of his range: never kh-party's)
        while let Some(&(t, m, name, persistent)) = self.fx.get(self.fx_next) {
            if t > now {
                break;
            }
            self.fx_next += 1;
            if name.is_empty() {
                for (_, h) in self.fx_live.iter().filter(|l| l.0 == m) {
                    effects.0.push(kh_host::seam::EffectCommand::Stop(kh_core::EffectHandle(*h), false));
                }
                self.fx_live.retain(|l| l.0 != m);
                continue;
            }
            let h = kh_core::host::party::PARTY_HANDLE_BASE + m as u32 * kh_core::host::party::PARTY_HANDLE_SPAN + kh_core::host::party::PARTY_HANDLE_SPAN - 4096 + self.fx_next as u32;
            let at = kh_core::Attach::Party { member: kh_core::host::party::PartyMember(m), bone: Some("vfx_root"), offset: Vec3::ZERO };
            // (DON-MAGIC tint) NAME@R,G,B[,I]: the effect recoloured (kh_core::Effects::tint), the [R] proofs
            let (name, tint) = match name.split_once('@') {
                Some((n, t)) => {
                    let v: Vec<f32> = t.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    (n, (v.len() >= 3).then(|| kh_core::host::Tint { color: [v[0], v[1], v[2]], intensity: v.get(3).copied().unwrap_or(1.0) }))
                }
                None => (name, None),
            };
            effects.0.push(kh_host::seam::EffectCommand::Spawn(kh_core::EffectHandle(h), kh_core::EffectSpawn { id: kh_core::EffectId(u32::MAX), name, at, rotation: Vec3::ZERO, scale: 1.0, persistent }));
            if let Some(t) = tint {
                effects.0.push(kh_host::seam::EffectCommand::Tint(kh_core::EffectHandle(h), t));
            }
            if persistent {
                self.fx_live.push((m, h));
            }
            println!("party: step {step}: fx {name} on member {m}{}", if persistent { " (persistent)" } else { "" });
        }
        // (ZETTA) the debug words due
        while let Some(k) = self.words.iter().position(|w| w.0 <= now) {
            let (_, word, arg) = self.words.remove(k);
            match word {
                "cast" => match kh_party::donald::SpellRef::from_id(&arg) {
                    Some(s) => {
                        self.party.don_fx.debug_casts.push(s);
                        println!("party: step {step}: Donald casts {arg} (debug cast)");
                    }
                    None => println!("party: step {step}: cast {arg}: no such spell row"),
                },
                // (FREEZE-FX) a FREEZE row's hit on him
                "freeze-donald" | "freeze-goofy" => {
                    let chara = if word == "freeze-donald" { Chara::Donald } else { Chara::Goofy };
                    if let Some(id) = self.party.find(chara) {
                        let at = self.party.view(id).map_or(Vec3::ZERO, |v| v.position);
                        let mut h = kh_core::HitRecord::empty(kh_core::TargetId(1));
                        (h.damage, h.damage_kind, h.position, h.direction, h.attack_name) = (1.0, kh_core::enums::ETresDamageKind::Small, at + Vec3::Z * 90.0, Vec3::new(-1.0, 0.0, 0.0), "MAG_BLIZZARD");
                        (h.info.bad_status, h.info.bad_time) = ("FREEZE", arg.parse().unwrap_or(2.0));
                        let r = self.party.receive_hit(id, &h);
                        println!("party: step {step}: {chara:?} takes a FREEZE hit ({} s): {r:?}", h.info.bad_time);
                    }
                }
                // (PARTY-WEAPONS) the battle flag, a weapon
                "battle" | "peace" => {
                    self.party.set_in_battle(word == "battle");
                    println!("party: step {step}: the battle flag {}", if word == "battle" { "ON" } else { "off" });
                }
                "hold-donald" | "hold-goofy" | "release-donald" | "release-goofy" => {
                    let chara = if word.ends_with("donald") { Chara::Donald } else { Chara::Goofy };
                    if let Some(id) = self.party.find(chara) {
                        self.party.set_weapon_hold(id, word.starts_with("hold"));
                        println!("party: step {step}: {chara:?}'s weapon {}", if word.starts_with("hold") { "held out" } else { "released" });
                    }
                }
                "walk-donald" | "walk-goofy" => {
                    let chara = if word.ends_with("donald") { Chara::Donald } else { Chara::Goofy };
                    let d: Vec<f32> = arg.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    if let (Some(id), [dx, dy]) = (self.party.find(chara), d.as_slice()) {
                        let goal = self.party.view(id).map_or(Vec3::ZERO, |v| v.position) + Vec3::new(*dx, *dy, 0.0);
                        self.party.gof.kh3_ai = true;
                        self.party.command(id, MemberCommand::MoveTo { goal: kh_party::MoveGoal::Point(goal), locomotion: kh_party::LocomotionDef::Move1, walk: false, acceptable_radius: 100.0, follow_path: false });
                        println!("party: step {step}: {chara:?} walks to {goal:?}");
                    }
                }
                "equip-donald" | "equip-goofy" => {
                    let (chara, member) = if word == "equip-donald" { (Chara::Donald, 0u8) } else { (Chara::Goofy, 1u8) };
                    match (kh_party::weapon_draw::weapon_row(&arg), self.party.find(chara)) {
                        (Some(row), Some(id)) => {
                            if let Some(s) = self.party.status_mut(id) {
                                s.weapon = row.key;
                            }
                            self.mount(member, row.key);
                            println!("party: step {step}: {chara:?} holds {} ({})", row.key, row.model);
                        }
                        _ => println!("party: step {step}: equip {arg}: no such weapon of {chara:?}"),
                    }
                }
                _ => println!("party: step {step}: zetta-ready: {} Donald(s) meet Zettaflare's condition", kh_party::donald::zettaflare::debug_make_ready(&mut self.party)),
            }
        }
        let view = SoraView { position: sora.position, yaw: sora.yaw, on_ground: sora.on_floor, hp: 100, hp_max: 100, in_battle: true, ..SoraView::default() };
        let first = audio.0.len();
        {
            let mut host = PartyHost { world, targets, audio, effects };
            self.party.step(dt, &view, &mut host);
        }
        for e in self.party.don_zetta.take_log() {
            println!("party: step {step}: zetta {e:?}"); // (ZETTA) the Zettaflare sequence's log
        }
        // the party's looping sounds (as kh-er fx_party.rs counts them)
        for c in &audio.0[first..] {
            match c {
                AudioCommand::Play(h, play) if play.looping && is_party_handle(h.0) => {
                    self.loop_sounds.insert(h.0, play.name);
                }
                AudioCommand::Stop(h, _) => {
                    self.loop_sounds.remove(&h.0);
                }
                _ => {}
            }
        }
        // the poses: the members' bones / sockets for the effect side
        self.taps.clear();
        self.audio.clear();
        let ids: Vec<MemberId> = self.party.members().collect();
        for id in ids {
            let Some(v) = self.party.view(id) else { continue };
            let Some(m) = self.posers.get_mut(&id.0) else { continue };
            // (PARTY-WEAPONS) in his hand while kh-party's rule has it out (weapon_draw)
            let weapon = !self.party.weapon_hidden(id) && self.party.weapon_view(id).is_some_and(|w| w.opacity > 0.0);
            // (PARTY-WEAPONS) a member in his speed-matched loop (the AI's Move, the pad's run) posed from it, as kh-partyhost does
            let loco = self.party.pad_state(id).and_then(|p| p.loco).or_else(|| self.party.ai_loco(id));
            let tap = match loco {
                Some(loco) => Some(kh_partyhost::poser::pose_loco(self.loco_time.entry(id.0).or_default(), m, &v, &loco, dt, weapon, &NoIk)),
                None => m.set_pose_view(kh_partyhost::poser::PoseState { member: id.0, position: v.position, yaw: v.yaw, weapon_visible: weapon, grounded: v.on_ground }, dt, &v.anim, &NoIk),
            };
            if let Some(tap) = tap {
                self.audio.push((id.0, tap.step, tap.grounded, tap.anim.clone(), tap.weapon.clone()));
                self.taps.push((id.0, PartyMemberFx { position: tap.position, yaw: tap.yaw, bones: tap.bones, anim: tap.anim.0 }));
            }
            let busy = v.state == kh_party::NpcStateId::Action;
            if self.busy.insert(id.0, busy) == Some(true) && !busy {
                self.said.push(format!("party: step {step}: {:?}'s action is over", v.chara));
            }
        }
    }

    /// (PARTY-WEAPONS) Mounts weapon `key` on member `member's` poser (the pack's weapon manifest).
    fn mount(&mut self, member: u8, key: &str) {
        let Some(m) = self.posers.get_mut(&member) else { return };
        let Ok(def) = m.weapon_def(key) else { return };
        let wm: Option<serde_json::Value> = std::fs::read_to_string(self.pack.join("party").join("weapons").join(&def.model).join("manifest.json")).ok().and_then(|t| serde_json::from_str(&t).ok());
        if let Some(wm) = wm {
            let _ = m.mount_weapon(&def, &wm);
        }
    }

    /// (PARTY-WEAPONS) `--party-draw`: the simulated members DRAWN by the renderer's party path (as kh-er partydraw.rs): their
    /// latest poses, the weapon in the hand at kh-party's `OpacityFade` (weapon_draw), the thrown shield not drawn.
    #[cfg(feature = "party")]
    pub fn draws(&self, glow_mask: bool) -> Vec<kh_er_render::party::CharDraw> {
        let mut out = Vec::new();
        for id in self.party.members() {
            let Some(m) = self.posers.get(&id.0) else { continue };
            let posed = m.posed(1.0);
            let opacity = self.party.weapon_view(id).map_or(0.0, |w| w.opacity);
            let weapon = posed.weapon.as_ref().filter(|w| w.2 && opacity > 0.0 && !self.party.weapon_hidden(id)).map(|(model, root, _)| kh_er_render::party::PartyWeapon { model: model.clone(), to_world: root.then(&posed.component_to_world), component: Vec::new(), opacity });
            out.push(kh_er_render::party::CharDraw { chara: m.id, body: Some(kh_er_render::Body { component_to_world: posed.component_to_world, local: posed.local, component: posed.component }), weapon, props: Vec::new(), glow_mask, blood: Default::default() });
        }
        out
    }

    /// (HEROLIGHT-KB) `--hero-friend donald|goofy`: that friend carries the hero light (as kh-er when he leads): his member id,
    /// his weapon's model, his feet and the light's place on his weapon's own line (kh-er partydraw.rs `hero_point`) with the
    /// weapon's opacity; his weapon is held out by the shared gate (kh_core::weapon_gate::hold).
    pub fn hero_carrier(&mut self, chara: Chara) -> Option<HeroFriend> {
        let id = self.party.members().find(|id| self.party.view(*id).is_some_and(|v| v.chara == chara))?;
        self.party.set_weapon_hold(id, kh_core::weapon_gate::hold(true, kh_er_render::pointlight::keyblade_wanted()));
        let m = self.posers.get(&id.0)?;
        let posed = m.posed(1.0);
        let (model, _, visible) = posed.weapon.clone()?;
        let opacity = self.party.weapon_view(id).map_or(0.0, |w| w.opacity);
        let f = kh_er_render::herolight::blade_fraction(&kh_er_render::pointlight::settings());
        let line = kh_er_render::herolight::weapon(&model).and_then(|w| w.line_sockets.clone());
        let point = line.and_then(|(a, b)| m.weapon_socket_world(&a).zip(m.weapon_socket_world(&b))).map(|(a, b)| a.translation.lerp(b.translation, f));
        Some((id.0, model, posed.component_to_world.translation, point.filter(|_| visible && opacity > 0.0).map(|p| (p, opacity))))
    }

    /// (PARTY-WEAPONS) What the weapons are now (the harness's line per shot).
    #[cfg(feature = "party")]
    pub fn weapons_line(&self) -> String {
        self.party.members().filter_map(|id| Some(format!("{:?} {} out={} opacity={:.2}", self.party.view(id)?.chara, self.party.status(id)?.weapon, self.party.weapon_view(id)?.out as u8, self.party.weapon_view(id)?.opacity))).collect::<Vec<_>>().join(", ")
    }

    /// The effect side of the step (kh-er fx.rs `effect_step`): the members' poses and the spell actors / team props.
    pub fn feed(&self, effects: &mut EffectHost) {
        let projectiles = self.party.projectiles.iter().filter(|p| p.alive).map(|p| (p.effect, p.position, p.velocity.normalize_or_zero())).collect();
        effects.set_party(self.taps.clone(), projectiles);
    }

    /// The looping sounds still running: those the seam stopped are gone; `live`: the seam handle still plays (a harness
    /// without an audio engine: every started loop counts until its Stop).
    pub fn loops(&self, effects: &EffectHost) -> String {
        let mut by: HashMap<u8, (usize, usize)> = HashMap::new();
        for h in self.loop_sounds.keys() {
            by.entry(kh_core::host::party::handle_member(*h).map_or(255, |m| m.0)).or_default().0 += 1;
        }
        for (o, n) in effects.party_loops() {
            by.entry(o).or_default().1 += n;
        }
        let mut v: Vec<(u8, (usize, usize))> = by.into_iter().filter(|(_, (s, e))| s + e > 0).collect();
        v.sort();
        if v.is_empty() {
            return "0".to_string();
        }
        v.iter().map(|(o, (s, e))| format!("{} {s} sound(s) / {e} effect(s){}", match o { 0 => "Donald", 1 => "Goofy", _ => "team" }, if *s > 0 { format!(" {:?}", self.loop_sounds.values().collect::<Vec<_>>()) } else { String::new() })).collect::<Vec<_>>().join(", ")
    }
}

/// The friends' leg IK on the harness's flat floor: none (their feet as animated).
struct NoIk;

impl kh_anim::IkWorld for NoIk {
    fn sweep_sphere(&self, _start: Vec3, _end: Vec3, _radius: f32) -> Option<kh_anim::SweepHit> {
        None
    }
}

/// (ER party S5) THE AUDIO SIDE OF THE HARNESS, as kh-er's fx.rs runs it: the main audio pack with the party audio root
/// merged, `kh_host::audio::HostAudio` (output: none; `--party-audio-log FILE` writes the engine's sound log, the test
/// app's `--audio-log` format), the step's seam commands of Sora AND the party, `Attach::Party` on the members' bones,
/// the members' rows / weapons and their motion sounds (footsteps on the flat floor's surface 0) before the engine step.
pub struct PartyAudio {
    pub host: kh_host::audio::HostAudio,
    pending: Vec<AudioCommand>,
    last_steps: HashMap<u8, u64>,
}

impl PartyAudio {
    pub fn new(pack: &Path, log: Option<&str>) -> Result<PartyAudio, String> {
        let mut p = kh_audio::Pack::load(&pack.join("audio"))?;
        let merged = p.merge_party_root(&pack.join("party").join("audio"))?;
        println!("effects: (party) audio pack {} banks, {} triggers; party audio root merged: {merged}", p.banks.len(), p.triggers.len());
        let mut host = kh_host::audio::HostAudio::new(std::sync::Arc::new(p), kh_host::audio::Output::null(), 1, kh_audio::Language::En);
        if let Some(path) = log {
            host.log_to(path).map_err(|e| format!("{path}: {e}"))?;
        }
        Ok(PartyAudio { host, pending: Vec::new(), last_steps: HashMap::new() })
    }

    /// The step's seam sounds (Sora's and the party's), before the harness drops them.
    pub fn take(&mut self, queue: &mut AudioQueue) {
        self.pending.append(&mut queue.0);
    }

    /// One audio step after the effect side's (the same order as kh-er's `Fx::after_step`).
    pub fn step(&mut self, out: &kh_core::Output, body: PosedBody, effect_commands: &[kh_host::seam::EffectCommand], listener: kh_audio::Listener, sora_place: &dyn Fn(&kh_core::Attach) -> Vec3, party: &mut PartyRig) {
        let PosedBody { skeleton, pose, to_world } = body;
        if pose.component.is_empty() {
            self.pending.clear();
            return;
        }
        for (m, _, _, anim, weapon) in &party.audio {
            if self.host.engine.party(*m).is_some() {
                self.host.engine.set_party_anim(*m, &anim.0, anim.1);
                if !weapon.is_empty() {
                    self.host.engine.set_party_weapon(*m, weapon);
                }
            }
        }
        self.host.queue.0.append(&mut self.pending);
        let runs: Vec<(u8, u64)> = party
            .audio
            .iter()
            .filter_map(|(m, step, ..)| {
                let last = self.last_steps.get(m).copied().unwrap_or(0);
                (*step > last).then(|| {
                    self.last_steps.insert(*m, *step);
                    (*m, if last == 0 { 1 } else { (*step - last).min(8) })
                })
            })
            .collect();
        let taps = &party.taps;
        let audio_rows = &party.audio;
        let place = |at: &kh_core::Attach| match at {
            kh_core::Attach::Party { member, bone, offset } => taps.iter().find(|t| t.0 == member.0).map_or(out.position + *offset, |t| t.1.xform(*bone, *offset).0),
            _ => sora_place(at),
        };
        let step = kh_host::audio::Step { out, dt: 1.0 / 60.0, skeleton, pose, component_to_world: to_world, effects: effect_commands, surface: 0, listener, hp: None, place: &place };
        self.host.step_with(&step, &mut |engine| {
            for (m, n) in &runs {
                let (Some((_, fx)), Some(a)) = (taps.iter().find(|t| t.0 == *m), audio_rows.iter().find(|a| a.0 == *m)) else { continue };
                let positions: [Vec3; 14] = std::array::from_fn(|i| fx.bones.get(kh_audio::autose::BONES[i]).map_or(if i == 0 { fx.position } else { fx.position + Vec3::Z * 50.0 }, |b| b.0));
                let frame = kh_audio::BoneFrame::from_positions(&positions, a.2);
                for e in engine.party_auto_se(*m, kh_audio::AutoSeTick { dt: *n as f32 / 60.0, frame: &frame, surface: 0, anim: Some((a.3 .0.as_str(), a.3 .1)), listener: &listener, bone: &|b| fx.bones.get(b).map(|x| x.0) }) {
                    let _ = e;
                }
            }
        });
        // the looping sounds the engine no longer plays (ended, or never resolved) are not running
        let engine = &self.host.engine;
        party.loop_sounds.retain(|h, _| engine.seam_live(*h));
    }
}

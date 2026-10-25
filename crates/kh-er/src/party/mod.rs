//! (ER party S1) DONALD, GOOFY AND AI SORA IN ELDEN RING: the kh-er host of the engine-free party runtime
//! (`kh_partyhost::PartyRuntime`, stream S0). Plan: build/sora_tmp/er/party/ER_PARTY_PLAN.md stream S1; contract:
//! docs/party-contracts.md section ER (ER.10: this host's interfaces for S2 .. S8); state note:
//! build/sora_tmp/er/party/IMPL_S1_STATE.md. Everything here is behind the cargo feature `party` (default on) and the
//! key `party.er` (default on): with `party.er = 0` Sora's frame is exactly as before the party.
//!
//! - `glue.rs`: THE CORE (game-free): the contract's step order, the trail nav and waypoints, the ladder jump, the
//!   park / reseat / catch-up / stuck rules, the feed for the other streams; the offline harness (glue/tests.rs).
//! - `nav.rs`: the leader's breadcrumb trail, kh-ai's `Nav` over it and the game's casts, the waypoint rule.
//! - `aiworld.rs`: Elden Ring's characters as the party's targets and the AI's enemies (`PartyTargets` over ErTargets).
//! - `soratargets.rs`: Sora's step with the party (`kh_partyhost::SoraTargets`: allies, heal_ally, friends, friend_link).
//! - this file: THE SLOT a Sora host holds (the core lives on across Sora hosts: loads, deaths, fast travel, the title)
//!   and the game's reads (the body, the camera), the calls sora.rs makes.
//!
//! KEYS (`kh-er.ini`, (INI) once `kh-er.ctl`; none needs to be written: the defaults are the shipped behaviour):
//! - `party.er` (default 1): the party runs. 0 = Sora alone (the friends wait, frozen, not drawn; 1 brings them back
//!   beside the leader).
//! - `party.bodies`, `party.body_test`, `party.bullet_owner`: the ally bodies' keys (stream S3 reads them).

//! - `party.pose` (default 1): the members' poses are evaluated every step (their bones for the effect / sound side and
//!   the drawing).
//! - `party.trace` (default 0): the AI trace lines (`party-trace:`) and a line a second of the nav's numbers.
//! - `party.draw` (default 0, testing aid): the friends' capsules and facing in the game's debug drawer.
//! - `party.recover`, `party.word` (one-shots, development aids): read only by a build with the cargo feature `dev-aids`
//!   (control.rs); the released mod ignores them.
//! Sheet `sheets/port/party-er-host.json` (rows `nav.*`, `casts.*`, `ladder.*`, `stuck.*`, `catchup.*`, `warp.*`).

pub mod glue;
pub mod nav;
#[cfg(windows)]
pub mod aiworld;
#[cfg(windows)]
pub mod soratargets;
// (ER party S2) the leader body (stream S2's file; declared here, the folder's mod.rs is S1's).
// Stream S3's allybody.rs reaches bodytest.rs / redirect.rs by `#[path]`: they are NOT declared here.
#[cfg(windows)]
pub mod leader;
// (ER party S7) the team attacks and their cameras in Elden Ring (stream S7's file)
pub mod team;
// (ER party I, X3G's request) Over Here! pulls the enemies' hate onto Goofy's Elden Ring body
pub mod taunt;
// (DON-PROG) Donald's overhaul in Elden Ring: the sheets at run time, his boss steps, his aggro (the hate route)
pub mod donaggro;
// (AGGRO) whom Elden Ring's enemies attack: the whole party, by the abilities' rules and a threat model
pub mod aggro;

#[cfg(windows)]
pub use host::*;

#[cfg(windows)]
mod host {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use eldenring::cs::PlayerIns;
    use glam::Vec3;
    use kh_core::{Input, Output, Sora, TargetId};

    use super::aiworld::ErPartyTargets;
    use super::glue::{BodyObs, FrameIn, HostRules, PartyCore};
    use crate::control::{Control, OneShot};
    use crate::coords::{self, Frame};
    use crate::erlog;
    use crate::game;
    use crate::targets::ErTargets;
    use crate::world::ErWorld;

    /// The party core between two Sora hosts (a load, a death, fast travel, the title): the friends never despawn.
    static KEPT: Mutex<Option<Box<PartyCore>>> = Mutex::new(None);

    fn sink(text: &str) {
        crate::log::write(text);
    }

    /// WHAT A SORA HOST HOLDS of the party: the core (taken from `KEPT` at the host's start, put back when it is dropped).
    pub struct PartySlot {
        core: Option<Box<PartyCore>>,
        pack: PathBuf,
        sheets: PathBuf,
        /// This host has not run a party frame yet (its first frame reseats the friends).
        fresh: bool,
        /// The "attacked lately" latch (aiworld.rs) and the frame's camera, the Sora step's clock.
        latch: u32,
        recover: Option<OneShot>,
        /// (GOOFY-GG) `party.word`
        word: Option<OneShot>,
        failed: bool,
    }

    impl Drop for PartySlot {
        fn drop(&mut self) {
            if let Some(core) = self.core.take() {
                *KEPT.lock().unwrap_or_else(|e| e.into_inner()) = Some(core);
            }
        }
    }

    fn read_json(path: &Path) -> Result<serde_json::Value, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn rows(path: &Path) -> Vec<serde_json::Value> {
        read_json(path).ok().and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default()
    }

    /// The core made from the pack and the sheets (once per game run: it is kept across Sora hosts).
    fn create(pack: &Path, sheets: &Path) -> Box<PartyCore> {
        let started = std::time::Instant::now();
        kh_partyhost::log::set_logger(Some(Box::new(|line: &str| crate::log::write(line))));
        let test = kh_partyhost::PartyTest::from_rows(&rows(&sheets.join("party-test.json")));
        let weapons = kh_partyhost::PartyWeapon::from_rows(&rows(&sheets.join("party-weapons.json")));
        let ai_dir = pack.join("party").join("ai");
        let ai_pack = match kh_ai::AiPack::load_dir(&ai_dir) {
            Ok(p) => Some(std::sync::Arc::new(p)),
            Err(e) => {
                erlog!("party: NO AI PACK ({}: {e}): the friends stand where they are put", ai_dir.display());
                None
            }
        };
        let host_sheet = sheets.join("party-er-host.json");
        let rules = match std::fs::read_to_string(&host_sheet).map_err(|e| e.to_string()).and_then(|t| HostRules::from_sheet(&t)) {
            Ok((r, problems)) => {
                erlog!("party: host numbers {} ({} rows not understood{}): {r:?}", host_sheet.display(), problems.len(), if problems.is_empty() { String::new() } else { format!(": {}", problems.join("; ")) });
                r
            }
            Err(e) => {
                erlog!("party: host numbers {} not read ({e}): the built-in defaults are used", host_sheet.display());
                HostRules::DEFAULT
            }
        };
        let config = kh_partyhost::RuntimeConfig { wanted: true, test, weapons, ai_pack, ai_pack_source: None };
        let mut core = Box::new(PartyCore::new(config, rules));
        // (ER party S6) the real progression (KH3's init data, the shared EXP, what the friends earn): before the first step
        crate::partyui::configure(&mut core.rt);
        core.sink = Some(sink);
        // the posers (S4 draws from them; their bones feed S5): the friends' packs as the test level reads them
        let index = read_json(&pack.join("party").join("party.json")).unwrap_or(serde_json::Value::Null);
        for id in [kh_partyhost::poser::DONALD, kh_partyhost::poser::GOOFY] {
            let Some(npc) = kh_partyhost::poser::npc_of(id) else { continue };
            let dir = pack.join("party").join(npc);
            let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(id as u64)).cloned().unwrap_or(serde_json::Value::Null);
            let made = read_json(&dir.join("manifest.json")).and_then(|manifest| {
                let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir)?, kh_anim::Settings::default())?;
                kh_partyhost::PosedMember::new(id, animator, &manifest, &entry)
            });
            match made {
                Ok(m) => {
                    erlog!("party: {npc} posed from {} ({} bones, {} weapons)", dir.display(), m.animator.skeleton().len(), m.weapons.len());
                    core.poser.members.insert(id, m);
                }
                Err(e) => erlog!("party: {npc}: no pose ({e}): no bones for the effects / drawing"),
            }
        }
        erlog!("party: the party host is made in {:.2} s (it lives on across loads; party.er = 0 switches it off)", started.elapsed().as_secs_f32());
        core
    }

    impl PartySlot {
        /// At a Sora host's start: the core of the game run (made at the first start the party is on).
        pub fn start(pack: &Path, sheets: &Path, control: &Control) -> PartySlot {
            let core = KEPT.lock().unwrap_or_else(|e| e.into_inner()).take();
            if core.is_some() {
                erlog!("party: the party host carries over to this Sora host (the friends stay as they were; they are put beside the leader)");
            }
            PartySlot { core, pack: pack.to_path_buf(), sheets: sheets.to_path_buf(), fresh: true, latch: u32::MAX, recover: Some(OneShot::new("party.recover", control)), word: Some(OneShot::new("party.word", control)) /* (GOOFY-GG) */, failed: false }
        }

        /// The core while the party is on (S2 .. S8: the runtime, the feed, the poser).
        pub fn core(&self) -> Option<&PartyCore> {
            self.core.as_deref().filter(|c| c.on)
        }

        pub fn core_mut(&mut self) -> Option<&mut PartyCore> {
            self.core.as_deref_mut().filter(|c| c.on)
        }

        /// The runtime while the party is on (S6's menus, S7's team, Sora's targets).
        pub fn runtime_on(&mut self) -> Option<&mut kh_partyhost::PartyRuntime> {
            self.core_mut().map(|c| &mut c.rt)
        }

        /// ONCE A FRAME before the steps (sora.rs pre_physics, after the follow phase): the keys, the park / reseat
        /// rules, the body's place, the camera of the AI.
        #[allow(clippy::too_many_arguments)]
        pub fn frame(&mut self, player: &PlayerIns, control: &Control, frame: &Frame, relocated: bool, hidden: Option<&'static str>, body_action: bool, dt: f32, now: f64) {
            let on = control.flag("party.er") != Some(false);
            // (FX5) `party.threat_range` (cm, default 1200; 0 = off): the friends' trees fight only what threatens the party
            kh_partyhost::aiworld::set_threat_range(control.f32("party.threat_range"));
            crate::unlock::frame(control); // (ER party S8) the unlock keys and one-shots for the next party step
            kh_party::donald::zettaflare::set_cinematic(control.flag("zetta.cinematic")); // (ZETTA) `zetta.cinematic = 0`: no cinematic camera for Zettaflare
            // (FACE) `party.look_head_up.donald` / `.goofy` (deg, default 30; 0 = KH3's own limit): the head's look-up cap
            kh_partyhost::poser::set_look_head_up_max(kh_partyhost::poser::DONALD, control.f32("party.look_head_up.donald"));
            kh_partyhost::poser::set_look_head_up_max(kh_partyhost::poser::GOOFY, control.f32("party.look_head_up.goofy"));
            if !on {
                crate::partyui::reset(); // (ER party S6) the core is off: the screens are Sora's alone
            }
            if on && self.core.is_none() && !self.failed {
                let core = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| create(&self.pack, &self.sheets)));
                match core {
                    Ok(c) => self.core = Some(c),
                    Err(_) => {
                        self.failed = true;
                        erlog!("!!! party: the party host could not be made (a panic): Sora alone this session");
                    }
                }
            }
            let fired = self.recover.as_mut().and_then(|r| r.fired(control));
            let word = self.word.as_mut().and_then(|r| r.fired(control)); // (GOOFY-GG)
            let Some(core) = self.core.as_deref_mut() else { return };
            let physics = &player.chr_ins.modules.physics;
            let body = Some(BodyObs { feet: frame.pos_er_to_kh(game::v3(&physics.position)), yaw: coords::yaw_kh_of_er_quat(game::quat(&physics.orientation)) });
            if let Some(cam) = game::camera() {
                let forward = coords::dir_er_to_kh(cam.forward).normalize_or_zero();
                core.camera.position = frame.pos_er_to_kh(cam.position);
                core.camera.rotation = kh_camera::Rot { pitch: forward.z.clamp(-1.0, 1.0).asin().to_degrees(), yaw: forward.y.atan2(forward.x).to_degrees(), roll: 0.0 };
                core.camera.rotation_target = core.camera.rotation;
            }
            let fresh = std::mem::take(&mut self.fresh);
            // (DTH) A NEW SORA HOST (a respawn, a load, fast travel): every member stands again, full, as Sora does (save.rs: a
            // new host starts with full gauges). Before, a knocked-out friend leader carried over KO'd and the next host's
            // game over came at once (the user's tutorial run as Goofy: at the Stranded Graveyard, before its scene).
            if fresh && on {
                let stood = crate::death::revive_all::<kh_host::party::PartyChara, _>(&mut core.rt);
                crate::partyui::clear_leader_down();
                if !stood.is_empty() {
                    erlog!("party: a new Sora host: {} stood up again, HP / MP full (DTH: the party after a death)", stood.iter().map(|c| c.name()).collect::<Vec<_>>().join(", "));
                }
            }
            // (FOL2) the natural follow (no sprint back to the spot, walkable spots, hallway file, speed matching): ctl
            // `party.follow2` (ON by default; `party.follow2 = 0`: the follow as in FINAL15)
            let fol2 = control.flag("party.follow2") != Some(false);
            kh_party::locomotion::FOL2_ON.store(fol2, std::sync::atomic::Ordering::Relaxed);
            kh_ai::formation::ON.store(fol2, std::sync::atomic::Ordering::Relaxed);
            core.frame(FrameIn { on, fresh, relocated, hidden, body_action, body, pose: control.flag("party.pose") != Some(false), trace: control.on("party.trace"), dt, now });
            super::donaggro::frame(core, control, dt); // (DON-PROG) the Donald sheets + don.* keys at the start and every reload; his boss steps
            // (EQ) the camp menu's edits of a friend in force at once (the world is paused under the menu: no party step
            // runs until it closes), as Sora's are
            if core.on {
                let n = crate::partyui::menu_sync(&mut core.rt);
                if n > 0 {
                    core.say(format!("party: t={now:.3}s {n} camp-menu edit(s) applied at once (EQ)"));
                }
            }
            if fired.is_some() && core.on {
                core.rt.recover();
                core.say("party: party.recover: the friends' HP / MP are full".into());
            }
            // (GOOFY-GG) the testing aid `party.word`
            if let (Some(text), true) = (word, core.on) {
                let words: Vec<&str> = text.split_whitespace().collect();
                let said = match words.as_slice() {
                    ["gg", rest @ ..] => kh_partyhost::custom::guard_word(&mut core.rt, rest),
                    _ => {
                        core.rt.lead_word(&words);
                        "queued for the next route".to_string()
                    }
                };
                core.say(format!("party: party.word {text}: {said}"));
            }
            let pack = self.pack.clone();
            core.mount_weapons(&|model: &str| read_json(&pack.join("party").join("weapons").join(model).join("manifest.json")).ok());
            // (LDI) the active members' models and shader sets made ready (and kept) by the renderer from the first frame of the
            // game run, under the loading screen, not at their first drawing after it
            if core.on {
                // (the members, and the formation they are made from: under a held loading screen no route has made them yet)
                let formation = core.asked_formation.as_ref().unwrap_or(&core.rt.lead.formation).active().to_vec();
                let ids: Vec<u8> = core.rt.members.iter().map(|(c, _)| *c).chain(formation).filter(|c| *c != kh_host::party::PartyChara::Sora).map(kh_partyhost::poser::draw_id).collect();
                kh_er_render::party::preload(&ids);
            }
        }

        /// ER.0 STEP 1, BEFORE SORA'S STEP: Sora's input (the pad's when he leads, AI Sora's otherwise).
        pub fn route(&mut self, sora: &mut Sora, input: &Input, dt: f32, world: &ErWorld, targets: &mut ErTargets, now: f64) -> Input {
            let mut latch = self.latch;
            let Some(core) = self.core_mut() else { return *input };
            let leader = core.rt.leader_view_of(&sora.output(), sora.status()).position;
            let t = ErPartyTargets::new(targets, leader, &mut latch);
            let input = core.route(sora, input, dt, world, &t, now);
            self.latch = latch;
            input
        }

        /// ER.0 STEP 2: SORA'S STEP with the party (his allies, heals on friends, the team seam); not at all while he is
        /// out of the active party (his output stays: the leader body follows the leader, stream S2).
        #[allow(clippy::too_many_arguments)]
        pub fn step_sora(&mut self, sora: &mut Sora, dt: f32, input: &Input, world: &ErWorld, targets: &mut ErTargets, fx: &mut crate::fx::Fx, ui: &mut crate::hud::Keep, before: &Output) -> Output {
            match self.core_mut() {
                // (ER party S8) out of the party, Sora may be FOUND FIGHTING MARGIT: his stage step (else his output stays)
                Some(core) if core.sora_skipped() => crate::unlock::stage_sora_step(sora, dt, world, fx).unwrap_or_else(|| Output { events: Vec::new(), ..before.clone() }),
                Some(core) => super::soratargets::step(sora, dt, input, world, targets, &mut core.rt, fx, ui),
                None => {
                    sora.set_aura(0.0, 0.0); // (DON-KIT) no party: no Donald's aura
                    let mut host = kh_core::Host { world, targets, audio: &mut fx.audio, effects: &mut fx.effects, ui };
                    sora.step_hosted(dt, input, &mut host)
                }
            }
        }

        /// ER.0 STEPS 3 - 5, RIGHT AFTER SORA'S STEP: the party's step, the poses, the leader's lock, the trail and the
        /// fallbacks, the bodies' want list (`core().feed`).
        #[allow(clippy::too_many_arguments)]
        /// `look`: Sora's look-at point (his "t_camera" bone of the last drawn pose: the friends' head look-at);
        /// `leaping`: AI Sora is in his ladder jump (LADDER2).
        #[allow(clippy::too_many_arguments)]
        pub fn after_sora_step(&mut self, sora: &mut Sora, out: &Output, dt: f32, world: &ErWorld, targets: &mut ErTargets, fx: &mut crate::fx::Fx, lock_on: TargetId, look: Option<Vec3>, leaping: bool, now: f64) {
            let mut latch = self.latch;
            let (unlock_pack, unlock_sheets) = (self.pack.clone(), self.sheets.clone()); // (ER party S8)
            let Some(core) = self.core_mut() else { return };
            core.sora_leaping = leaping;
            let leader = core.rt.leader_view_of(out, sora.status()).position;
            let mut t = ErPartyTargets::new(targets, leader, &mut latch);
            // (S2's status rule) SORA'S CURE ON A FRIEND (`heal_ally`, effect kinds 10 / 11 / 12 = Cure / Cura / Curaga on the
            // others: kh-core magic) clears that friend's body's ailments, as the friends' own Cures do
            let sora_cures: Vec<(kh_host::party::PartyChara, u8)> = core.rt.pending_heals.iter().filter(|h| (10..=12).contains(&h.2)).filter_map(|h| core.rt.chara_of_id(h.0).map(|c| (c, h.2 - 10))).collect();
            let look = look.or(Some(out.capsule_center + Vec3::Z * 55.0));
            let casts = world.casts.get();
            let mut effects = CureWatch { inner: &mut fx.effects, seen: Vec::new() };
            core.step(sora, out, dt, lock_on, world, &PartyIk(world), &mut t, &mut fx.audio, &mut effects, look, now);
            core.world_casts += world.casts.get().wrapping_sub(casts);
            // (DON-MAGIC) the statuses' Elden Ring side: the snapshot the hit routes read, the slowed / Frozen characters' speed,
            // the Blind enemies' aim
            crate::donstatus::publish(&core.rt.party);
            crate::donstatus::apply_game(targets);
            // (ER party S6) the screens' asks into the runtime and the feed out (HUD, menus, save, save point, prizes); a
            // load's unlocks restored as saved (no "newly unlocked join": the saved formation stands), its unlock record
            // kept for S8, the save point's cure of every AI ally's body for S3
            let unlocked = core.unlocked().map(<[_]>::to_vec);
            let s6 = crate::partyui::sync(&mut core.rt, sora, unlocked.as_deref(), now);
            if let Some(list) = s6.restore_unlocked {
                core.restore_unlocked(&list, now);
            }
            if s6.restore_unlock_record.is_some() {
                core.restored_unlock_record = s6.restore_unlock_record;
            }
            core.feed.cure_bodies |= s6.cure_bodies;
            // (S2's status rules) a Cure's effect landing on a member clears his body's ailments; an item a member used on
            // a member clears them when its text says so
            let leader = core.rt.leader_chara();
            for (name, at) in effects.seen {
                let who = match at {
                    kh_core::Attach::Party { member, .. } => core.rt.chara_of_id(kh_party::MemberId(member.0)),
                    kh_core::Attach::Sora { .. } => Some(kh_host::party::PartyChara::Sora),
                    _ => None,
                };
                if let Some(who) = who {
                    crate::status::on_party_effect(name, who, leader);
                }
            }
            for (who, tier) in sora_cures {
                crate::status::on_party_effect(kh_core::host::audio::cure_heal_effect(tier as usize), who, leader);
            }
            for e in &core.feed.events {
                if let kh_party::PartyEventKind::ItemUsed { item, target } = e.kind {
                    let who = match target {
                        kh_party::TargetRef::Member(m) => core.rt.chara_of_id(m),
                        kh_party::TargetRef::Sora => Some(leader),
                        _ => core.rt.chara_of_id(e.member),
                    };
                    if let Some(who) = who {
                        crate::status::on_party_item(item, who, leader);
                    }
                }
            }
            // (ER party S8) THE UNLOCK RULE: who is in the party, the staged fights, Donald at the crone, the unlock moment
            crate::unlock::party_step(core, &unlock_pack, &unlock_sheets, sora, out, dt, world, targets, fx, now);
            crate::conv::hold_party(core); // (CV1) a conversation holds the party: AI stopped, standing, looking at the speaker
            self.latch = latch;
        }

        /// (testing aid, `party.draw = 1`) the friends' capsules and facing in the game's debug drawer (sora.rs post_physics).
        pub fn draw(&self, control: &Control, frame: &Frame) {
            if !control.on("party.draw") {
                return;
            }
            let Some(core) = self.core().filter(|c| c.visible()) else { return };
            let Some(mut draw) = game::Draw::get() else { return };
            for v in &core.rt.current {
                let (r, h) = v.capsule;
                draw.colour(if v.ko { game::ORANGE } else if v.on_ground { game::GREEN } else { game::CYAN });
                draw.capsule(frame.pos_kh_to_er(v.position + Vec3::Z * (h + 2.0)), r / coords::CM_PER_M, h / coords::CM_PER_M);
                let chest = frame.pos_kh_to_er(v.position + Vec3::Z * h * 1.2);
                draw.colour(game::WHITE);
                draw.line(chest, chest + coords::facing_er(v.yaw) * 0.6);
            }
            draw.colour(game::CYAN);
            let crumbs: Vec<Vec3> = core.trail.crumbs.iter().rev().take(60).map(|c| frame.pos_kh_to_er(c.at + Vec3::Z * 5.0)).collect();
            for w in crumbs.windows(2) {
                draw.line(w[0], w[1]);
            }
        }
    }

    /// The step's effects seen on their way to the host's effect queue (the Cure effects for S2's status rule).
    struct CureWatch<'a> {
        inner: &'a mut dyn kh_core::Effects,
        seen: Vec<(&'static str, kh_core::Attach)>,
    }

    impl kh_core::Effects for CureWatch<'_> {
        fn spawn(&mut self, handle: kh_core::EffectHandle, spawn: &kh_core::EffectSpawn) {
            if !spawn.name.is_empty() {
                self.seen.push((spawn.name, spawn.at));
            }
            self.inner.spawn(handle, spawn)
        }
        fn stop(&mut self, handle: kh_core::EffectHandle, immediate: bool) {
            self.inner.stop(handle, immediate)
        }
        fn tint(&mut self, handle: kh_core::EffectHandle, tint: kh_core::host::Tint) {
            self.inner.tint(handle, tint) // (DON-MAGIC tint) passed on
        }
        fn place(&mut self, handle: kh_core::EffectHandle, at: glam::Vec3) {
            self.inner.place(handle, at) // (DON-MAGIC orbit) passed on
        }
        fn transform(&mut self, handle: kh_core::EffectHandle, at: glam::Vec3, rotation: glam::Vec3, scale: glam::Vec3) {
            self.inner.transform(handle, at, rotation, scale) // (ZETTA) passed on
        }
    }

    /// The friends' leg IK against the game's world without Sora's own floor clamp (`ErWorld::ik_floor` is his).
    struct PartyIk<'a>(&'a ErWorld);

    impl kh_anim::IkWorld for PartyIk<'_> {
        fn sweep_sphere(&self, start: Vec3, end: Vec3, radius: f32) -> Option<kh_anim::SweepHit> {
            let hit = crate::sweep::sweep_first(self.0, start, end, radius, radius, &self.0.opts.get())?;
            Some(kh_anim::SweepHit { time: hit.time, impact_point: hit.position, impact_normal: hit.impact_normal, start_penetrating: hit.start_penetrating })
        }
    }
}

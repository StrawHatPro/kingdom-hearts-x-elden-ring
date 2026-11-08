//! (AUDIT-A) THE FORM AUDIT'S RECORDER: one Sora session through the REAL core, the REAL effect side (`EffectHost`, main +
//! keyblade roots) and the REAL audio side (`HostAudio`, the audio pack's routes, a null output), step by step, with the pack.
//! Every step records: the state, the clip whose notifies fired (`Output::anim_ticked`, its notify window), the form, every
//! effect / sound command of the seam, what the effect side made of each spawn (drawn, refused, its place relative to the
//! socket it names) and what the audio side made of each sound (the asset played, or why nothing was). `coverage` then
//! lays every sound / voice / weapon-SE / effect notify KH3's clips carry over the steps that fired them and says, per
//! notify, whether the seam emitted it and whether the host resolved it. Used by `tests/audit_a_*.rs` (`kh_host::audit_a`).
//!
//! Nothing is drawn or heard. Skipped (None) without the pack.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use glam::{Vec2, Vec3};
use kh_audio::{Language, Listener};
use kh_core::anim::{NotifyData, SEQUENCES};
use kh_core::{Attach, Input, PadButton, PadButtons, PadState, StateId};
use crate::audio::{HostAudio, Output as AudioOut, Step};
use crate::rig::{component_to_world, Dummy, Rig, STEP};
use crate::seam::{AudioCommand, EffectCommand, EffectQueue};
use crate::vfx::{EffectHost, Scene};

pub fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// One effect command of a step, with the effect side's answer.
#[derive(Clone, Debug)]
pub struct FxEvent {
    pub handle: u32,
    pub name: String,
    /// `Attach` as the seam gave it (Debug text).
    pub attach: String,
    pub persistent: bool,
    /// None = a Spawn; Some(immediate) = a Stop of `handle`.
    pub stop: Option<bool>,
    /// New live systems of this name after the step (0 = nothing drawn for it).
    pub drawn: usize,
    /// The effect side's log lines of this step that name it.
    pub notes: Vec<String>,
    /// Where the first new system of that name is, in Sora's ACTOR frame (x forward, y right, z up, cm), and the socket
    /// point it names in the same frame (None: no socket / not found).
    pub at_actor: Option<Vec3>,
    pub socket_actor: Option<Vec3>,
    /// (AUDIT-B W-FIN) The row timing (`EffectCommand::Timing`) sent with this spawn, if any.
    pub timing: Option<kh_core::host::EffectTiming>,
}

/// One sound command of a step, with the audio side's answer (from its log).
#[derive(Clone, Debug)]
pub struct SoundEvent {
    pub handle: u32,
    pub name: String,
    pub attach: String,
    pub looping: bool,
    /// Stop(handle, fade) instead of a play.
    pub stop: Option<f32>,
    /// (AUDIT-A SOUND) Where the audio side put this play (world, cm; `HostAudio::placed`), None for a stop / a play it
    /// skipped (a reflected hit).
    pub placed: Option<Vec3>,
}

#[derive(Clone, Debug)]
pub struct StepRow {
    pub step: u64,
    pub state: StateId,
    /// The sequence whose notifies fired this step (its heaviest sample) and its notify window `(from, to]`.
    pub clip: &'static str,
    pub time: f32,
    pub window: (f32, f32),
    /// The play loops (only then can a window wrap).
    pub looping: bool,
    pub play_id: u32,
    /// (AUDIT-A SHIELD) The play's effect group (`Sora::effect_group`, the current group of the attach component: a
    /// notify row spawns only for its own group or -1; kh-core notify.rs `effect_group_passes`).
    pub effect_group: i32,
    pub form_cmd: i32,
    pub finish_cmd: i32,
    pub wear_form: i32,
    pub position: Vec3,
    pub yaw: f32,
    pub effects: Vec<FxEvent>,
    pub sounds: Vec<SoundEvent>,
    /// The audio log lines of this step: (kind, cause, sound asset, note).
    pub audio: Vec<(String, String, String, String)>,
    /// The step's core events (Debug text), for the agents' own filters.
    pub events: Vec<String>,
}

/// (FINISH-AUDIT2) One census row: (step, clip, clip time, the measurement of one system).
pub type CensusRow = (u64, &'static str, f32, crate::vfx::weaponattach::Measured);

/// (FINISH-AUDIT2) The placement census's tolerance: a system farther than this from KH3's attach point is a fault (cm).
pub const PLACEMENT_TOLERANCE_CM: f32 = 2.0;

pub struct Session {
    pub rig: Rig,
    pub fx: EffectHost,
    pub audio: HostAudio,
    held: PadButtons,
    pub stick: Vec2,
    pub rows: Vec<StepRow>,
    log_path: PathBuf,
    fx_said: usize,
    /// The keyblade model whose swing / hit sets are selected (kh-er kbdraw::select_sounds).
    sound_model: String,
    /// (AUDIT-A CAMS) the formchange / finisher camera layer as kh-er cam.rs steps it (None: off; `cams_on`).
    pub cams: Option<CamLayer>,
    /// (AUDIT-A WORLDTIME) the host's world clock as kh-er worldtime.rs steps it: before each step from the core's table
    /// (WorldSlow) and the camera layer's `SetGlobalGameSpeed` of the step before (with `cams_on`).
    pub world: crate::worldtime::WorldClock,
    /// (AUDIT-A WORLDTIME) per step: (step, state, clip, clip time, the rates that step ran at).
    pub world_rows: Vec<(u64, StateId, &'static str, f32, crate::worldtime::WorldRates)>,
    /// (AUDIT-A WORLDTIME) `world_on`: Sora's step, his effects and the camera layer run `STEP x` the global game speed,
    /// as in kh-er (off: every step is STEP, as before).
    pub world_scaled: bool,
    /// (FINISH-AUDIT2) The placement census of this session: every step, every live system of his notifies measured against
    /// KH3's attach object and socket (`EffectHost::measure_placements`): (step, clip, clip time, the measurement).
    pub census: Vec<CensusRow>,
    census_name: String,
}

/// (AUDIT-A CAMS) kh-er cam.rs's form camera layer over a field camera: kh-camera `form_bp::FormBpCameras` with the step's
/// core events, `command` = pawn+0xE2C and the held keyblade's weapon id (kh-er cam.rs / the harness set them the same way),
/// the field camera's reset / takeover. Every line of the layer's log, with the step, state and clip.
pub struct CamLayer {
    pub form_bp: kh_camera::form_bp::FormBpCameras,
    pub field: Option<kh_camera::CameraRig>,
    /// (step, state, clip @time, line).
    pub log: Vec<(u64, StateId, String, String)>,
    /// (step, the layer's view or None = the field camera's).
    pub views: Vec<(u64, Option<kh_camera::camera_anim::AnimView>)>,
}

/// The keyblade data of a keyblade id (`w_so150`): its models' effect prefixes (kh-er kbdraw `allowed_effects`).
fn allowed_for(models: &[&str]) -> Vec<String> {
    models.iter().map(|m| format!("v{m}")).collect()
}

impl Session {
    /// Sora with the level-99 loadout, `weapon` (an item key, e.g. `WEP_KEYBLADE_SO_17`) in his hand, the dummy at
    /// `dummy` (None: none). `models`: the keyblade's model ids (`w_so150`, `w_so151` ...) whose effects pass the
    /// other-keyblade rule; `sound_model`: the model whose swing / hit sets play (kh-er: the hand keyblade's base model).
    pub fn new(weapon: &str, models: &[&str], sound_model: &str, dummy: Option<Vec3>, log_name: &str) -> Option<Session> {
        let pack = project().join("build").join("sora_pack");
        if !pack.join("vfx").is_dir() || !pack.join("audio").is_dir() {
            eprintln!("SKIPPED: no pack under {}", pack.display());
            return None;
        }
        let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
        let mut rig = Rig::new(&pack, &loadout, dummy).expect("Sora's rig");
        rig.sora.status_mut().weapon = weapon.to_string();
        let mut fx = EffectHost::load(&pack).expect("the main effect pack");
        if let Err(e) = fx.load_kb_root(&pack) {
            eprintln!("(AUDIT-A) no keyblade effect root: {e}");
        }
        // (AUDIT-A SHIELD) + the other models' effects of the keyblade's forms' shared clips (kh-er kbdraw.rs, the same rule)
        let mut allowed = allowed_for(models);
        allowed.extend(kh_core::combat::forms::shared_clip_effects(weapon));
        fx.set_keyblade(None, None, &allowed);
        let mut apack = kh_audio::Pack::load(&pack.join("audio")).expect("the audio pack");
        // (AUDIT-A SOUND) the roots kh-er fx.rs merges: the XS clips' sounds and the keyblade forms' (PACK/keyblades/audio)
        for root in [pack.join("xsi").join("audio"), pack.join("keyblades").join("audio")] {
            if let Err(e) = apack.merge_party_root(&root) {
                eprintln!("(AUDIT-A) the audio root {} is not merged: {e}", root.display());
            }
        }
        let apack = Arc::new(apack);
        let mut audio = HostAudio::new(apack, AudioOut::null(), 1, Language::En);
        let dir = project().join("build").join("sora_tmp").join("weapons").join("AUDIT-A").join("logs");
        std::fs::create_dir_all(&dir).ok();
        let log_path = dir.join(format!("{log_name}.audio.csv"));
        audio.log_to(&log_path.to_string_lossy()).expect("the audio log");
        if !audio.engine.routes.select_weapon(sound_model) {
            eprintln!("(AUDIT-A) the audio pack has no swing / hit sets of {sound_model}");
        }
        Some(Session { rig, fx, audio, held: PadButtons::NONE, stick: Vec2::ZERO, rows: Vec::new(), log_path, fx_said: 0, sound_model: sound_model.to_string(), cams: None /* (AUDIT-A CAMS) */, world: crate::worldtime::WorldClock::default(), world_rows: Vec::new(), world_scaled: false /* (AUDIT-A WORLDTIME) */, census: Vec::new(), census_name: log_name.to_string() /* (FINISH-AUDIT2) */ })
    }

    /// (AUDIT-A CAMS) Turns the form camera layer on (kh-er cam.rs's `form_bp` over a field camera).
    pub fn cams_on(&mut self) {
        self.cams = Some(CamLayer { form_bp: kh_camera::form_bp::FormBpCameras::new(), field: None, log: Vec::new(), views: Vec::new() });
    }

    /// (AUDIT-A WORLDTIME) Sora's step, his effects and the camera layer on KH3's global game speed (kh-er's `wdt`).
    pub fn world_on(&mut self) {
        self.world_scaled = true;
    }

    /// (AUDIT-A CAMS) kh-er cam.rs's order: the field camera, then the layer with the step's events. (AUDIT-A WORLDTIME)
    /// `dt`: the world's step (kh-er cam.rs: `dt x world_rate`).
    fn step_cams(&mut self, dt: f32) {
        let Some(c) = self.cams.as_mut() else { return };
        let out = &self.rig.out;
        let target = kh_camera::CameraTarget { position: out.position, yaw: out.yaw, velocity: out.displacement_velocity, state: kh_camera::TargetState::Other, collision_half_height: kh_core::SORA_PARAMS.capsule_half_height };
        let field = c.field.get_or_insert_with(|| kh_camera::CameraRig::new(kh_camera::CAMERA_PARAMS, &target));
        let under = field.step(dt, &kh_camera::CameraInput::default(), &target, None, &self.rig.world); // (AUDIT-A WORLDTIME) dt
        let frame = kh_camera::form_bp::FormBpFrame {
            sora: (out.position, out.yaw),
            field: kh_camera::camera_anim::AnimView { location: under.position, rotation: under.rotation, fov: kh_camera::projection(&kh_camera::CAMERA_PARAMS).0 },
            state: out.state,
            ultima: kh_core::combat::form::equipped_keyblade(self.rig.sora.status()) == "WEP_KEYBLADE_SO_14",
            perf: kh_camera::form_bp::PerfCamera::default(),
        };
        c.form_bp.command = self.rig.sora.combat().form.command_in_action;
        c.form_bp.guns.weapon = kh_core::combat::form::weapon_type(self.rig.sora.status());
        let o = c.form_bp.step(dt, &out.events, &frame, Some(&self.rig.world)); // (AUDIT-A WORLDTIME) dt
        if let Some(t) = o.takeover {
            field.field.take_over(t.location, t.rotation);
        }
        if o.reset_field_camera {
            field.field.reset(&target, true);
        }
        let clip = out.anim_ticked.body_samples()[0];
        let at = format!("{}@{:.3}", clip.0, clip.2);
        for e in &out.events {
            if let kh_core::host::EventKind::AnimBpEvent { seq, param, begin } = e.kind {
                c.log.push((out.step, out.state, at.clone(), format!("core BpEvent {seq} param {param} {}", if begin { "begin" } else { "end" })));
            }
        }
        if o.reset_field_camera {
            c.log.push((out.step, out.state, at.clone(), "field camera reset".to_string()));
        }
        if o.takeover.is_some() {
            c.log.push((out.step, out.state, at.clone(), "field camera takes over".to_string()));
        }
        for l in c.form_bp.log.drain(..) {
            c.log.push((out.step, out.state, at.clone(), l));
        }
        c.views.push((out.step, o.view));
    }

    /// One fixed step with these buttons held (a button held in the step before and still held is not a new press).
    pub fn step(&mut self, held: &[PadButton]) -> StateId {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input { move_stick: self.stick, ..Input::default() }.with_pad(PadState::from_held(now, self.held));
        self.held = now;
        self.step_input(&input)
    }

    /// One fixed step with a whole `Input` (the stick, the camera, the view ...).
    pub fn step_input(&mut self, input: &Input) -> StateId {
        let mut effects = EffectQueue::default();
        // (AUDIT-A WORLDTIME) kh-er's order: the world clock first (the table as the step before left it), then his step
        let sora = &self.rig.sora;
        let rates = self.world.step(STEP, 1.0, |t| sora.time_dilation(t));
        let dt = if self.world_scaled { STEP * rates.global } else { STEP };
        self.rig.step_with_dt(dt, input, &mut self.audio.queue, &mut effects);
        self.step_cams(dt); // (AUDIT-A CAMS) (AUDIT-A WORLDTIME) dt
        // (AUDIT-A WORLDTIME) the blueprint's SetGlobalGameSpeed of this step, for the next (kh-er sora.rs after cam.step)
        let request = self.cams.as_mut().and_then(|c| c.form_bp.shield.game_speed.take());
        self.world.request(request);
        let ticked = self.rig.out.anim_ticked;
        let sample = ticked.body_samples()[0];
        self.world_rows.push((self.rig.out.step, self.rig.out.state, sample.0, sample.2, rates)); // (AUDIT-A WORLDTIME)
        // the seam's sound commands, before the audio side takes them
        let mut sounds: Vec<SoundEvent> = self
            .audio
            .queue
            .0
            .iter()
            .map(|c| match c {
                AudioCommand::Play(h, p) => SoundEvent { handle: h.0, name: p.name.to_string(), attach: format!("{:?}", p.at), looping: p.looping, stop: None, placed: None },
                AudioCommand::Stop(h, f) => SoundEvent { handle: h.0, name: String::new(), attach: String::new(), looping: false, stop: Some(*f), placed: None },
                AudioCommand::VoiceGroupNo(n) => SoundEvent { handle: 0, name: format!("voice_group_no {n}"), attach: String::new(), looping: false, stop: None, placed: None },
            })
            .collect();
        // (AUDIT-A SHIELD) the swing / hit sets follow the weapon the hits come from, every step, as kh-er does it (sora/kbdraw.rs
        // `sound_model` / `select_sounds`: kh-core `Status::attack_model`, in a form its hand unit: Counter Shield's w_so032 ..)
        let attack = self.rig.sora.status().attack_model.to_string();
        let model = if attack.is_empty() { self.sound_model.clone() } else { attack };
        if self.audio.engine.routes.weapon.as_deref() != Some(model.as_str()) {
            self.audio.engine.routes.select_weapon_kh3(&model); // (kbdraw select_sounds' current rule, AUDIT-A SOUND)
        }
        // (AUDIT-C TF) the left hand's sets (kbdraw select_sounds: Status::left_attack_model)
        let left = self.rig.sora.status().left_attack_model;
        self.audio.engine.routes.select_left_weapon(left);
        let out = self.rig.out.clone();
        let to_world = component_to_world(&out);
        let listener = Listener { position: out.position + Vec3::new(-400.0, 0.0, 150.0), forward: Vec3::X, right: Vec3::Y };
        let status = self.rig.sora.status();
        let hp = (status.hp, status.max_hp());
        let rig = &self.rig;
        // (AUDIT-A SOUND) the weapons' / pawns' own points (`kb:` / `pawn:`) of the sounds on them, from the effect side, as
        // kh-er fx.rs hands them over (its effect step runs first, on this step's pose)
        let wanted = self.audio.weapon_anchors(&[], &effects.0);
        if !wanted.is_empty() {
            let mut tap = crate::audio::PoseTap::default();
            tap.record(out.step, &out, rig.animator.skeleton(), &rig.pose, &to_world);
            let target = |_: kh_core::TargetId| rig.dummy.map(|d: Dummy| d.info.center);
            let scene = Scene { out: &out, sora: &rig.sora, tap: &tap, target: &target, ground: Some(0.0), eye: out.position + Vec3::new(-500.0, 0.0, 200.0) };
            self.audio.anchors = wanted.iter().map(|name| (*name, self.fx.socket_point(&scene, name))).collect();
        }
        self.audio.step(&Step { out: &out, dt: STEP, skeleton: rig.animator.skeleton(), pose: &rig.pose, component_to_world: &to_world, effects: &effects.0, surface: 0, listener, hp: Some(hp), place: &|at| rig.place(at) });
        for (handle, at) in &self.audio.placed {
            if let Some(x) = sounds.iter_mut().find(|x| x.handle == *handle && x.stop.is_none()) {
                x.placed = Some(*at);
            }
        }
        let before: HashMap<String, usize> = self.live_counts();
        // (AUDIT-A ARROWGUN) the systems alive before the step: a new one is drawn even when an older one of the same name
        // ended in the same step (the net count alone reads 0 then)
        let before_ids: Vec<_> = self.fx.world.live().iter().map(|l| l.id).collect();
        let target = |_: kh_core::TargetId| self.rig.dummy.map(|d: Dummy| d.info.center);
        let scene = Scene { out: &out, sora: &self.rig.sora, tap: &self.audio.tap, target: &target, ground: Some(0.0), eye: out.position + Vec3::new(-500.0, 0.0, 200.0) };
        self.fx.step(dt, &effects.0, &scene); // (AUDIT-A WORLDTIME) dt (kh-er sora.rs `effect_dt = wdt`)
        // (FINISH-AUDIT2) the placement census of this step
        {
            let fresh: Vec<_> = self.fx.world.live().iter().map(|l| l.id).filter(|id| !before_ids.contains(id)).collect();
            let sample = out.anim_ticked.body_samples()[0];
            for m in self.fx.measure_placements(&scene, &fresh) {
                self.census.push((out.step, sample.0, out.anim_ticked.time, m));
            }
        }
        let notes: Vec<String> = self.fx.log[self.fx_said.min(self.fx.log.len())..].to_vec();
        self.fx_said = self.fx.log.len();
        let after = self.live_counts();
        let live = self.fx.world.live();
        let (sin, cos) = out.yaw.to_radians().sin_cos();
        let to_actor = |p: Vec3| {
            let d = p - out.position;
            Vec3::new(d.x * cos + d.y * sin, -d.x * sin + d.y * cos, d.z)
        };
        let mut fxs = Vec::new();
        for c in &effects.0 {
            match c {
                EffectCommand::Spawn(h, s) => {
                    let file = s.name.rsplit(['/', '.']).next().unwrap_or(s.name).to_lowercase();
                    let net = after.get(&file).copied().unwrap_or(0).saturating_sub(before.get(&file).copied().unwrap_or(0));
                    let fresh = live.iter().filter(|l| l.name.to_lowercase() == file && !before_ids.contains(&l.id)).count(); // (AUDIT-A ARROWGUN)
                    let drawn = net.max(fresh).max(self.fx.drawn_of(*h)); // (AUDIT-C SW) + an attach data asset's rows (one system per row)
                    let at_actor = live.iter().rev().find(|l| l.name.to_lowercase() == file).and_then(|l| self.fx.world.latest_nodes(l.id)).map(|(x, _)| to_actor(x.position));
                    let socket_actor = match s.at {
                        Attach::Sora { bone: Some(bone), .. } => Some(to_actor(self.fx.socket_point(&scene, bone))),
                        _ => None,
                    };
                    let own: Vec<String> = notes.iter().filter(|n| n.to_lowercase().contains(&file)).cloned().collect();
                    fxs.push(FxEvent { handle: h.0, name: file, attach: format!("{:?}", s.at), persistent: s.persistent, stop: None, drawn, notes: own, at_actor, socket_actor, timing: None });
                }
                EffectCommand::Stop(h, immediate) => fxs.push(FxEvent { handle: h.0, name: String::new(), attach: String::new(), persistent: false, stop: Some(*immediate), drawn: 0, notes: Vec::new(), at_actor: None, socket_actor: None, timing: None }),
                // (AUDIT-B W-FIN) the row timing goes with its spawn
                EffectCommand::Timing(h, t) => {
                    if let Some(f) = fxs.iter_mut().rev().find(|f| f.handle == h.0 && f.stop.is_none()) {
                        f.timing = Some(*t);
                    }
                }
                _ => {}
            }
        }
        let ticked = out.anim_ticked;
        let clip = ticked.body_samples()[0].0;
        let form = &self.rig.sora.combat().form;
        self.rows.push(StepRow {
            step: out.step,
            state: out.state,
            clip,
            time: ticked.time,
            window: ticked.notify_window,
            looping: ticked.looping,
            play_id: ticked.play_id,
            effect_group: self.rig.sora.effect_group(), // (AUDIT-A SHIELD)
            form_cmd: form.cmd,
            finish_cmd: form.finish_cmd,
            wear_form: i32::from(form.wear_form),
            position: out.position,
            yaw: out.yaw,
            effects: fxs,
            sounds,
            audio: Vec::new(),
            events: out.events.iter().map(|e| format!("{e:?}")).collect(),
        });
        out.state
    }

    fn live_counts(&self) -> HashMap<String, usize> {
        let mut m = HashMap::new();
        for l in self.fx.world.live() {
            *m.entry(l.name.to_lowercase()).or_insert(0) += 1;
        }
        m
    }

    pub fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    /// Steps until `done` holds (at most `limit` steps). True: it held.
    pub fn until(&mut self, limit: usize, mut done: impl FnMut(&Session) -> bool) -> bool {
        for _ in 0..limit {
            if done(self) {
                return true;
            }
            self.step(&[]);
        }
        done(self)
    }

    /// Reads the audio log back into the rows (call once, at the end of the session).
    pub fn finish(&mut self) {
        self.write_census();
        // (FINISH-AUDIT2) every recorder session guards the placement: no effect of his notifies off KH3's attach point
        if !crate::vfx::weaponattach::OFF.load(std::sync::atomic::Ordering::Relaxed) {
            let faults: Vec<String> = self.placement_faults().iter().map(|c| format!("step {} {} @{:.3} {} at {} (KH3 {}): {:.0} cm off", c.0, c.1, c.2, c.3.effect, c.3.sent, c.3.kh3_anchor, c.3.off_cm())).collect();
            assert!(faults.is_empty(), "{}: {} effect placement fault(s) (census in build/sora_tmp/weapons/FINISH-AUDIT2/census):\n{}", self.census_name, faults.len(), faults.join("\n"));
        }
        self.audio.flush();
        let text = std::fs::read_to_string(&self.log_path).unwrap_or_default();
        let by_step: HashMap<u64, usize> = self.rows.iter().enumerate().map(|(i, r)| (r.step, i)).collect();
        for line in text.lines().skip(1) {
            let f: Vec<&str> = split_csv(line);
            if f.len() < 12 {
                continue;
            }
            // the audio side counts its own steps from 1, in the same order as ours
            let Ok(step) = f[0].parse::<usize>() else { continue };
            if let Some(row) = step.checked_sub(1).and_then(|k| self.rows.get(k).map(|r| r.step)).and_then(|s| by_step.get(&s).copied()) {
                self.rows[row].audio.push((f[2].to_string(), f[3].to_string(), f[4].to_string(), f[11].to_string()));
            }
        }
    }

    /// (FINISH-AUDIT2) The census's faults: systems more than `PLACEMENT_TOLERANCE_CM` from KH3's attach point, and weapon
    /// rows on a weapon's root for want of its socket.
    pub fn placement_faults(&self) -> Vec<&CensusRow> {
        self.census.iter().filter(|c| c.3.off_cm() > PLACEMENT_TOLERANCE_CM || c.3.root_fallback).collect()
    }

    /// (FINISH-AUDIT2) Writes the census (build/sora_tmp/weapons/FINISH-AUDIT2/census/<session>.tsv: every measured system
    /// once per (clip, effect, socket) with its worst step, the faults first) and the session's counts.
    pub fn write_census(&self) {
        let dir = project().join("build/sora_tmp/weapons/FINISH-AUDIT2/census");
        std::fs::create_dir_all(&dir).ok();
        let mut worst: HashMap<(&str, String, String), &CensusRow> = HashMap::new();
        for c in &self.census {
            let key = (c.1, c.3.effect.clone(), c.3.sent.clone());
            if worst.get(&key).is_none_or(|w| w.3.off_cm() < c.3.off_cm()) {
                worst.insert(key, c);
            }
        }
        let mut rows: Vec<_> = worst.into_values().collect();
        rows.sort_by(|a, b| b.3.off_cm().total_cmp(&a.3.off_cm()).then(a.0.cmp(&b.0)));
        let mut t = String::from("verdict\tstep\tclip\ttime\teffect\tsent socket\tKH3 object\tKH3 anchor\tfollow\tcm off\tactual\texpected\n");
        for (step, clip, time, m) in rows {
            let verdict = if m.root_fallback { "FAULT-ROOT" } else if m.off_cm() > PLACEMENT_TOLERANCE_CM { "FAULT" } else { "ok" };
            t.push_str(&format!("{verdict}\t{step}\t{clip}\t{time:.3}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.0},{:.0},{:.0}\t{:.0},{:.0},{:.0}\n", m.effect, m.sent, m.object, m.kh3_anchor, m.follow, m.off_cm(), m.actual.x, m.actual.y, m.actual.z, m.expected.x, m.expected.y, m.expected.z));
        }
        let pass = if crate::vfx::weaponattach::OFF.load(std::sync::atomic::Ordering::Relaxed) { "before" } else { "after" };
        std::fs::write(dir.join(format!("{}.{pass}.tsv", self.census_name)), t).ok();
    }

    pub fn sound_model(&self) -> &str {
        &self.sound_model
    }

    /// (AUDIT-C LD) + THE XS ABILITIES' EFFECT ROOT (PACK/xsi/vfx), as kh-er fx.rs reads it unless `fx.xsi = 0` (fx.rs 489:
    /// `control.flag("fx.xsi") != Some(false)`): Light Form's Zantetsuken / Black Hole effects (vw_so200_ztk_* / ehz_*) are
    /// only in that root. Name resolution stays kh-er's (EffectHost::ensure: main, party, XSI, then the keyblades' root).
    pub fn load_xsi_root(&mut self) {
        let pack = project().join("build").join("sora_pack");
        if let Err(e) = self.fx.load_xsi_root(&pack) {
            eprintln!("(AUDIT-C) no XS effect root: {e}");
        }
    }
}

fn split_csv(line: &str) -> Vec<&str> {
    // the log's fields: no field but `note` / `cause` holds a comma in practice; quotes are honoured
    let mut out = Vec::new();
    let (mut start, mut quoted) = (0, false);
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                out.push(line[start..i].trim_matches('"'));
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(line[start..].trim_matches('"'));
    out
}

/// What a notify asks for, as the audit compares it.
#[derive(Clone, Debug, PartialEq)]
pub enum Want {
    Effect { effect: &'static str, socket: &'static str, group: i32, state: bool },
    Sound { sound: &'static str, state: bool },
    WeaponSe { alias: &'static str, hit: bool },
    Voice { name: &'static str, random: usize },
    AutoSe { name: &'static str },
}

impl Want {
    /// The name the seam carries for it (notify.rs).
    pub fn seam_name(&self) -> &'static str {
        match self {
            Want::Effect { effect, .. } => effect,
            Want::Sound { sound, .. } => sound,
            Want::WeaponSe { alias, .. } => alias,
            Want::Voice { name, .. } => name,
            Want::AutoSe { name } => name,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Want::Effect { .. } => "effect",
            Want::Sound { .. } => "sound",
            Want::WeaponSe { hit: false, .. } => "swing-se",
            Want::WeaponSe { hit: true, .. } => "hit-se",
            Want::Voice { .. } => "voice",
            Want::AutoSe { .. } => "auto-se",
        }
    }
}

/// Every sound / effect notify of a sequence: (trigger time, what it asks for).
pub fn wants_of(sequence: &str) -> Vec<(f32, Want)> {
    let Some(seq) = SEQUENCES.iter().find(|s| s.name == sequence) else { return Vec::new() };
    let mut out = Vec::new();
    for n in seq.notifies {
        match n.data {
            NotifyData::Sound { sound, .. } => out.push((n.time, Want::Sound { sound, state: n.state })),
            NotifyData::WeaponSe { hit, alias, .. } => out.push((n.time, Want::WeaponSe { alias, hit })),
            NotifyData::Voice { group, asset, random, .. } => out.push((n.time, Want::Voice { name: asset.or(group).or(random.first().map(|r| r.0)).unwrap_or(""), random: random.len() })),
            NotifyData::AutoSe { motion, source, .. } => out.push((n.time, Want::AutoSe { name: motion.or(source).unwrap_or("") })),
            NotifyData::Effect { entries } => {
                for e in entries {
                    match e.effect {
                        Some(effect) => out.push((n.time, Want::Effect { effect, socket: e.socket, group: e.group_id, state: n.state })),
                        None if n.state && e.common && e.attach_type.ends_with("ATTACH_TO_WEAPON") => out.push((n.time, Want::Effect { effect: "key_trail0", socket: e.socket, group: e.group_id, state: true })),
                        None => {}
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// One notify of a played clip and what became of it.
#[derive(Clone, Debug)]
pub struct Covered {
    pub step: u64,
    pub clip: &'static str,
    pub time: f32,
    pub want: Want,
    /// The seam emitted it this step (by name, case-insensitive).
    pub emitted: bool,
    /// Effects: systems drawn / the host's notes; sounds: the audio log's lines for it.
    pub resolved: String,
    pub ok: bool,
}

fn in_window(t: f32, (from, to): (f32, f32), length: f32, looping: bool) -> bool {
    if to >= from || !looping {
        t > from && t <= to || (from < 0.0 && t == 0.0)
    } else {
        t > from && t <= length || t <= to
    }
}

/// Every sound / effect notify of every step's clip that the step's notify window fired, matched against the step's seam
/// commands and the hosts' answers. `ok`: emitted AND resolved (an effect drawn; a sound played or silent by a rule of
/// KH3's own data, e.g. an empty swing slot).
pub fn coverage(rows: &[StepRow]) -> Vec<Covered> {
    let mut out = Vec::new();
    for r in rows {
        if r.window == (0.0, 0.0) {
            continue;
        }
        let length = SEQUENCES.iter().find(|s| s.name == r.clip).map_or(0.0, |s| s.length);
        for (t, want) in wants_of(r.clip) {
            if !in_window(t, r.window, length, r.looping) {
                continue;
            }
            // (AUDIT-A SHIELD) KH3's effect-group rule (attach component `Set` 0x141770D30, spawn lambda 0x141762070; kh-core
            // notify.rs): a row of another group is not this play's (Frying Pan's 0x867 rows in a Counter Shield play of the
            // shared clip, and the other way round). The trail row (no asset, -2) passes in group 0 only.
            if let Want::Effect { effect, group, .. } = &want {
                if !kh_core::notify::effect_group_passes(*group, *effect != "key_trail0", r.effect_group) {
                    continue;
                }
            }
            let name = want.seam_name().to_lowercase();
            let short = name.rsplit(['/', '.']).next().unwrap_or(&name).to_string();
            let (emitted, resolved, ok) = match &want {
                Want::Effect { .. } => match r.effects.iter().find(|e| e.stop.is_none() && e.name == short) {
                    Some(e) => (true, format!("drawn {}{}", e.drawn, if e.notes.is_empty() { String::new() } else { format!(" ({})", e.notes.join("; ")) }), e.drawn > 0 || short == "key_trail0"),
                    None => (false, "not emitted".to_string(), false),
                },
                _ => {
                    // (AUDIT-C TF) an LH_WEAPON alias is carried as `LH_WEAPON:SWING3` (kh-core notify::left_hand_alias)
                    let emitted = r.sounds.iter().any(|s| s.stop.is_none() && { let n = s.name.to_lowercase(); n == name || n.strip_prefix("lh_weapon:") == Some(name.as_str()) });
                    let lines: Vec<String> = r.audio.iter().filter(|a| a.1.to_lowercase().ends_with(&short)).map(|a| format!("{} {} {}", a.0, a.2, a.3).trim().to_string()).collect();
                    let played = r.audio.iter().any(|a| a.1.to_lowercase().ends_with(&short) && (a.0 == "play" || a.0 == "voice"));
                    let silent_by_data = r.audio.iter().any(|a| a.1.to_lowercase().ends_with(&short) && a.0 == "silent" && silent_by_kh3_rule(&a.3)); // (AUDIT-A SOUND) the rules of `silent_by_kh3_rule`
                    (emitted, if lines.is_empty() { "no audio line".to_string() } else { lines.join(" | ") }, emitted && (played || silent_by_data))
                }
            };
            out.push(Covered { step: r.step, clip: r.clip, time: t, want, emitted, resolved, ok });
        }
    }
    out
}

/// (AUDIT-A SOUND) A `silent` line of the audio log whose reason is KH3's OWN rule, not a gap of the port: an empty slot of
/// the weapon's swing / hit set, a one-entry random list played once, SEAD's start macro (the same sound started within its
/// window: macro 1's 0.067 s ...) or full category pool (kh-audio engine `play_asset`), the asset's UE concurrency, a
/// looping asset refused by an instant notify (0x145815C90).
pub fn silent_by_kh3_rule(note: &str) -> bool {
    ["no entry in the weapon", "one-entry list plays once", "SEAD start macro", "SEAD category pool", "concurrency:", "a looping asset from an instant notify is refused"].iter().any(|r| note.contains(r))
}

/// (AUDIT-A SOUND) The seam's sound plays of one step and one name, and what the audio side made of them.
#[derive(Clone, Debug)]
pub struct SoundJudged {
    pub step: u64,
    pub clip: &'static str,
    pub name: String,
    /// The plays the seam asked for (this step, this name).
    pub requests: usize,
    /// Audio-log lines for them: `play`, `silent` by KH3's rule (their notes), anything else (unknown, other silences).
    pub played: usize,
    pub kh3_silent: Vec<String>,
    pub other: Vec<String>,
    /// Where the audio side put them (`SoundEvent::placed`).
    pub placed: Vec<Vec3>,
    /// Every request has its own line: played, or silent by KH3's rule.
    pub ok: bool,
}

/// (AUDIT-A SOUND) EVERY SOUND PLAY OF THE SEAM, JUDGED: per step and name, each request must leave a line of its own in
/// the audio log (`play`, or `silent` with KH3's rule, `silent_by_kh3_rule`). The lines of a name are found by their cause
/// (`seam <name>`); a voice-group number and stops are not plays.
pub fn judge_sounds(rows: &[StepRow]) -> Vec<SoundJudged> {
    let mut out = Vec::new();
    for r in rows {
        let mut names: Vec<&str> = Vec::new();
        for x in r.sounds.iter().filter(|x| x.stop.is_none() && !x.name.starts_with("voice_group_no ")) {
            if !names.contains(&x.name.as_str()) {
                names.push(&x.name);
            }
        }
        for name in names {
            let mine: Vec<&SoundEvent> = r.sounds.iter().filter(|x| x.stop.is_none() && x.name == name).collect();
            let short = name.rsplit('/').next().unwrap_or(name).to_lowercase();
            // a spell / shot actor's hit names its class (cause `actor <class> hit`)
            let lines: Vec<&(String, String, String, String)> = r.audio.iter().filter(|a| { let c = a.1.to_lowercase(); c == format!("seam {short}") || (c.starts_with("actor ") && c.contains(&short)) }).collect();
            let played = lines.iter().filter(|a| a.0 == "play").count();
            let kh3_silent: Vec<String> = lines.iter().filter(|a| a.0 == "silent" && silent_by_kh3_rule(&a.3)).map(|a| a.3.clone()).collect();
            let other: Vec<String> = lines.iter().filter(|a| !matches!(a.0.as_str(), "play" | "voice" | "random-voice") && !(a.0 == "silent" && silent_by_kh3_rule(&a.3))).map(|a| format!("{} {}", a.0, a.3)).collect();
            let requests = mine.len();
            out.push(SoundJudged { step: r.step, clip: r.clip, name: name.to_string(), requests, played, ok: other.is_empty() && played + kh3_silent.len() == requests, kh3_silent, other, placed: mine.iter().filter_map(|x| x.placed).collect() });
        }
    }
    out
}

/// The clips the session played, in order, with their first / last step.
pub fn clips(rows: &[StepRow]) -> Vec<(&'static str, u64, u64, StateId)> {
    let mut out: Vec<(&'static str, u64, u64, StateId)> = Vec::new();
    let mut play = u32::MAX;
    for r in rows {
        if r.play_id != play || out.last().is_none_or(|l| l.0 != r.clip) {
            play = r.play_id;
            out.push((r.clip, r.step, r.step, r.state));
        } else if let Some(l) = out.last_mut() {
            l.2 = r.step;
        }
    }
    out
}

/// A text report: the clips, then every covered notify (one line each), then every seam effect / sound the clips' notifies
/// do not explain (code-driven: hit effects, charge glow, projectiles ...).
pub fn report(rows: &[StepRow]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# clips");
    for (clip, a, b, state) in clips(rows) {
        let _ = writeln!(s, "{a:5}..{b:5} {state:?} {clip}");
    }
    let cov = coverage(rows);
    let _ = writeln!(s, "# notifies ({} fired, {} ok)", cov.len(), cov.iter().filter(|c| c.ok).count());
    for c in &cov {
        let extra = match &c.want {
            Want::Effect { socket, group, state, .. } => format!(" socket {socket} group {group}{}", if *state { " state" } else { "" }),
            Want::Voice { random, .. } if *random > 1 => format!(" random x{random}"),
            _ => String::new(),
        };
        let _ = writeln!(s, "{} step {:5} {} @{:.3} {} {}{} -> {} {}", if c.ok { "PASS" } else { "FAIL" }, c.step, c.clip, c.time, c.want.kind(), c.want.seam_name(), extra, if c.emitted { "emitted" } else { "NOT EMITTED" }, c.resolved);
    }
    let _ = writeln!(s, "# code-driven seam commands (no clip notify of the step names them)");
    for r in rows {
        let wants: Vec<String> = wants_of(r.clip).into_iter().map(|(_, w)| w.seam_name().to_lowercase()).collect();
        for e in r.effects.iter().filter(|e| e.stop.is_none() && !wants.iter().any(|w| w.ends_with(&e.name))) {
            let _ = writeln!(s, "step {:5} {:?} {} effect {} at {} persistent {} drawn {} actor {:?} socket {:?} {}", r.step, r.state, r.clip, e.name, e.attach, e.persistent, e.drawn, e.at_actor, e.socket_actor, e.notes.join("; "));
        }
        for x in r.sounds.iter().filter(|x| x.stop.is_none() && !wants.contains(&x.name.to_lowercase())) {
            let short = x.name.rsplit('/').next().unwrap_or(&x.name).to_lowercase();
            let lines: Vec<String> = r.audio.iter().filter(|a| a.1.to_lowercase().ends_with(&short)).map(|a| format!("{} {} {}", a.0, a.2, a.3)).collect();
            let _ = writeln!(s, "step {:5} {:?} {} sound {} at {} -> {}", r.step, r.state, r.clip, x.name, x.attach, lines.join(" | "));
        }
    }
    // (AUDIT-A SOUND) the sounds on a weapon's / pawn's own point (`kb:` / `pawn:`): where the audio side put them, in his
    // actor frame (x forward, y right, z up, cm) and in the world
    let _ = writeln!(s, "# weapon / pawn-anchored sound plays: where the audio side put them");
    for r in rows {
        let (sin, cos) = r.yaw.to_radians().sin_cos();
        for x in r.sounds.iter().filter(|x| x.stop.is_none() && (x.attach.contains("\"kb:") || x.attach.contains("\"pawn:"))) {
            let actor = x.placed.map(|p| {
                let d = p - r.position;
                Vec3::new(d.x * cos + d.y * sin, -d.x * sin + d.y * cos, d.z)
            });
            let _ = writeln!(s, "step {:5} {} sound {} at {} -> world {:?}, actor {:?}", r.step, r.clip, x.name, x.attach, x.placed, actor);
        }
    }
    // (AUDIT-A SOUND) every play of the seam has a line of its own: the groups with a silence by KH3's rule or a gap
    let judged = judge_sounds(rows);
    let _ = writeln!(s, "# seam sound plays judged per step and name ({} groups, {} ok; listed: KH3-rule silences and gaps)", judged.len(), judged.iter().filter(|j| j.ok).count());
    for j in judged.iter().filter(|j| !j.ok || !j.kh3_silent.is_empty()) {
        let _ = writeln!(s, "{} step {:5} {} {}: {} request(s), {} played, {} silent by KH3's rule [{}]{}", if j.ok { "PASS" } else { "FAIL" }, j.step, j.clip, j.name, j.requests, j.played, j.kh3_silent.len(), j.kh3_silent.join("; "), if j.other.is_empty() { String::new() } else { format!(", other: {}", j.other.join("; ")) });
    }
    s
}

/// Writes `report(rows)` to AUDIT-A/logs/<name>.report.txt and returns it.
pub fn write_report(rows: &[StepRow], name: &str) -> String {
    let text = report(rows);
    let dir = project().join("build").join("sora_tmp").join("weapons").join("AUDIT-A").join("logs");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join(format!("{name}.report.txt")), &text).ok();
    text
}

/// A file in AUDIT-A/logs.
pub fn path_in_logs(name: &str) -> PathBuf {
    project().join("build").join("sora_tmp").join("weapons").join("AUDIT-A").join("logs").join(name)
}

//! (AUDIT-A HARNESS) A FORM SESSION DRAWN AS kh-er DRAWS IT, for the effects harness (examples/effects.rs; flags and status:
//! build/sora_tmp/weapons/AUDIT-A/HARNESS.md):
//! - `--game-forms`: the form weapons from the core's Form and every finisher / accompany pawn, exactly as kh-er sora.rs
//!   `submit` hands them to the renderer (the shotlock's weapon form, the form's parts by grow level, the transform units
//!   during StyleChange, the hand keyblade hidden, Pegasus + Chariot / the Frying Pan, Clock Drill's clock, Mirage Staff's
//!   tower, Storm Flag's Kraken, Nano's gate and hide), his body tilted by the actor's pitch (F-LOCO) as kh-er draws it.
//!   Implies `--game-wear`.
//! - `--game-wear`: the wear form from the core every frame (kh-er boc::frame -> `kh_er_render::set_wear_form`), so the
//!   renderer's own slide (1 s in, 0.2 s out, by the frames' clock) runs; `--wear N` pins one instead.
//! - `--game-camera` / `--form-camera`: the view from the port's camera stack as kh-er cam.rs (camera.rs here).
//! - `--form-grow STEP:N[,STEP:N]`: the form's grow level set at that step (LV2 / LV3 parts for pictures).

mod camera;

use glam::Vec3;
use kh_anim::Trs;
use kh_host::rig::Rig;

pub use camera::GameCamera;

/// What the harness draws him with this frame: `hand_kb` = the keyblade in his hand (`--keyblade`'s, the Kingdom Key when
/// none); `to_world` the drawn (tilted) transform; `feet` his feet untilted (kh-er's `feet`); `socket` the hand keyblade's
/// socket on R_buki; `opacity` the hand keyblade's.
#[derive(Clone, Copy)]
pub struct Drawn<'a> {
    pub hand_kb: &'a str,
    pub to_world: &'a Trs,
    pub feet: Vec3,
    pub socket: &'a Trs,
    pub opacity: f32,
}

/// The session's switches and the camera (one per harness run).
pub struct FormSession {
    forms: bool,
    wear: bool,
    grow_at: Vec<(usize, u8)>,
    pub camera: Option<GameCamera>,
    told: Option<(i32, u8, String)>,
    /// (AUDIT-A PLACEMENT) the parts off their sockets the last step (for the log)
    placed: Option<Vec<String>>,
    /// (FORMWEP-FIX) the hand Keyblade between a form's start and its transformation (kh-host formstart.rs, as kh-er sora.rs)
    form_gate: kh_host::formstart::FormStartGate,
}

impl FormSession {
    /// From the harness's arguments (`value` / `flag` as effects.rs reads them); `pinned_wear` = `--wear N` given.
    pub fn from_args(value: &dyn Fn(&str) -> Option<String>, flag: &dyn Fn(&str) -> bool, pack: &std::path::Path) -> FormSession {
        let forms = flag("--game-forms");
        let wear = (forms || flag("--game-wear")) && value("--wear").is_none();
        let grow_at: Vec<(usize, u8)> = value("--form-grow")
            .map(|v| v.split(',').filter_map(|s| s.split_once(':')).filter_map(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?))).collect())
            .unwrap_or_default();
        let camera = (flag("--game-camera") || flag("--form-camera")).then(|| GameCamera::new(pack));
        if forms || wear || camera.is_some() {
            println!(
                "effects: (AUDIT-A) form session as kh-er draws it: form weapons and pawns {}, wear form from the core {}, the port's camera stack {}{}",
                if forms { "ON" } else { "off" },
                if wear { "ON" } else { "off" },
                if camera.is_some() { "ON" } else { "off" },
                if grow_at.is_empty() { String::new() } else { format!(", grow levels {grow_at:?}") }
            );
        }
        FormSession { forms, wear, grow_at, camera, told: None, placed: None /* (AUDIT-A PLACEMENT) */, form_gate: Default::default() /* (FORMWEP-FIX) */ }
    }

    /// The input yaw the core steers by (kh-er: `cam.input_yaw()`, else 0 as the harness had).
    pub fn input_yaw(&self) -> f32 {
        self.camera.as_ref().and_then(GameCamera::input_yaw).unwrap_or(0.0)
    }

    /// (AUDIT-A WORLDTIME) KH3's global game speed as of the camera's last step (kh-er sora.rs `wdt = STEP x` it: the
    /// hit-stops slow Sora's step and his effects); 1 without `--game-camera` or with `--no-world-time`.
    pub fn world_rate(&self) -> f32 {
        if std::env::args().any(|a| a == "--no-world-time") {
            return 1.0;
        }
        self.camera.as_ref().map_or(1.0, GameCamera::world_rate)
    }

    /// The core's view for the shotlock reticle (kh-er: `cam.core_view()`); None = the harness's own.
    pub fn core_view(&self) -> Option<kh_core::magic::View> {
        self.camera.as_ref().and_then(GameCamera::core_view)
    }

    /// Before the core's step: `--form-grow` at this step.
    pub fn before_step(&mut self, rig: &mut Rig, step: usize) {
        for (at, grow) in &self.grow_at {
            if *at == step {
                rig.sora.combat_mut().form.grow = *grow;
                println!("effects: (AUDIT-A) step {step}: form grow level set to {grow}");
            }
        }
    }

    /// After the core's step (kh-er's order: Sora, the effect side, then the camera in the same fixed step); the camera
    /// takes the effect side's camera shakes (the harness's effect step runs later in its frame: the shakes of the step
    /// before, one step late).
    pub fn after_step(&mut self, rig: &Rig, effects: &mut kh_host::vfx::EffectHost) {
        if self.wear {
            kh_er_render::set_wear_form(wear_form(rig));
        }
        if let Some(c) = self.camera.as_mut() {
            c.step(rig, effects.take_shakes());
        }
    }

    /// kh-er sora.rs `submit`: his body drawn tilted by the actor's pitch about the capsule centre (F-LOCO).
    pub fn tilt(&self, out: &kh_core::Output, mut to_world: Trs) -> Trs {
        if self.forms {
            let feet = to_world.translation;
            kh_host::rig::pitch_about(&mut to_world, out.yaw, out.pitch, feet + (out.capsule_center - out.position));
        }
        to_world
    }

    /// kh-er sora.rs `submit` (INT-F block): the form weapons and finisher pawns into `forms`; returns (hand keyblade hidden,
    /// body hidden).
    pub fn draw(&mut self, rig: &Rig, step: usize, d: &Drawn<'_>, forms: &mut Vec<kh_er_render::FormWeapon>) -> (bool, bool) {
        let Drawn { hand_kb, to_world, feet, socket, opacity } = *d;
        if !self.forms {
            return (!forms.is_empty(), false);
        }
        let skeleton = rig.animator.skeleton();
        let index_of = |b: &str| skeleton.index_of(b);
        let out = &rig.out;
        // (AUDIT-B W-DATA) a blend-space row of the held form's set: the weapons' row of its logical name, as kh-er sora.rs
        let weapon_anim = kh_core::combat::forms::weapon_logical(rig.sora.combat().form.cmd, out.anim.name).or_else(|| out.anim.name.sequences().first().copied());
        let form_pose = kh_er_render::keyblade::FormPose { index_of: &index_of, component: &rig.pose.component, to_world, anim: weapon_anim, time: out.anim.time, hand_socket: socket };
        let mut hand_hidden = !forms.is_empty();
        let combat = rig.sora.combat();
        let form = &combat.form;
        // (FORMWEP-FIX) one harness frame = one step: a form just started keeps the hand Keyblade until its transformation
        if self.form_gate.step(form, out.state) {
            println!("forms: step {step}: (FORMWEP-FIX) form 0x{:X} started, state {:?}: the hand Keyblade, not the form's units", form.cmd, out.state);
        }
        let magic = rig.sora.magic();
        let first = forms.len();
        if magic.shot_weapon_form != 0 {
            let id = format!("{:02X}", magic.shot_weapon_form);
            if magic.shot_weapon_units.is_empty() {
                forms.extend(kh_er_render::keyblade::draw_form_weapon_parts(hand_kb, &id, "", false, &form_pose));
            } else {
                forms.extend(unit_parts(hand_kb, &id, magic.shot_weapon_units, &form_pose));
            }
            hand_hidden = true;
        } else if let Some(row) = form.row().filter(|r| r.weapon_form_value > 1 && !hand_hidden && form.equip != kh_core::combat::form::FormEquip::Keyblade /* (AUDIT-B W-CORE) */ && !self.form_gate.on() /* (FORMWEP-FIX) */) {
            // (AUDIT-B W-CORE) the units KH3's equipment holds (kh-core `FormEquip`, as kh-er sora.rs `submit`): the Keyblade until
            // StyleChange's equip, the transform units ALONE from it, the form's own units from StyleChange's End
            let grow = form.grow.max(1).to_string();
            let style_change = out.state == kh_core::StateId::Stylechange;
            let transform = form.equip == kh_core::combat::form::FormEquip::Transform;
            if !kh_er_render::keyblade::equipped_parts(hand_kb, row.weapon_form, &grow, transform).is_empty() {
                if opacity > 0.0 || style_change {
                    forms.extend(kh_er_render::keyblade::draw_equipped_parts(hand_kb, row.weapon_form, &grow, transform, &form_pose));
                }
                hand_hidden = true;
            }
        }
        // (AUDIT-A STAFF) a form weapon's own TresAnimNotifyState_ChangeAttachParent (the staff's A416, the braid: on Sora's
        // kh_root 0 .. 3.1 s), as the INT line for kh-er sora.rs `submit` does it
        // (AUDIT-A PLACEMENT) now through ONE generic reader of every form weapon's window (kh-core
        // combat::forms::placement::part_parent: the launchers' FINISH_BZ 5417 / H417, the staff's A416, Nano Arms' w_so172
        // clips; keyed by his sequence as rig_of picks the weapon's clip), and the parts KH3 threw (TresAnimNotify_ThrowWeapon:
        // placement::ThrownWeapon, the raid's shield / pan, the flag) where the core says they fly, as kh-er sora.rs `submit`
        let (seq, time) = (out.anim.name.sequences().first().copied().unwrap_or(""), out.anim.time);
        let mut placed = Vec::new();
        for model in forms[first..].iter().filter_map(|f| f.model.strip_prefix(kh_er_render::keyblade::PREFIX).map(str::to_string)).collect::<Vec<_>>() {
            // the parent is a SOCKET of his (kh_root: on kh_sk, roll -90), else a bone: its component-space transform
            // (AUDIT-B W-HANDOVER) or the weapon clip's own window whoever equips it (a shotlock's equip set: placement::clip_parent)
            if let Some((name, b)) = kh_core::combat::forms::placement::part_parent(form.cmd, &model, seq, time).or_else(|| kh_core::combat::forms::placement::clip_parent(&model, seq, time)).and_then(|name| {
                let (bone, loc, rot) = kh_core::combat::forms::placement::socket(name).unwrap_or((name, Vec3::ZERO, Vec3::ZERO));
                let socket = Trs { translation: loc, rotation: kh_anim::blend::Rotator { pitch: rot.x, yaw: rot.y, roll: rot.z }.quat(), scale: Vec3::ONE };
                index_of(bone).and_then(|i| rig.pose.component.get(i)).map(|b| (name, socket.then(b)))
            }) {
                kh_er_render::keyblade::part_on_bone(&mut forms[first..], &model, &b, to_world);
                placed.push(format!("{model} on {name} ({:.0}, {:.0}, {:.0})", b.translation.x, b.translation.y, b.translation.z));
            }
        }
        for t in &combat.thrown {
            let (logical, _) = t.clip();
            let at = Trs { translation: t.position, rotation: t.rotation, scale: Vec3::ONE };
            if kh_er_render::keyblade::part_thrown(&mut forms[first..], t.model, at, logical, t.clip_time()) > 0 {
                placed.push(format!("{} THROWN at ({:.0}, {:.0}, {:.0}) {logical}@{:.2}", t.model, t.position.x, t.position.y, t.position.z, t.clip_time()));
            }
        }
        if !placed.is_empty() && (self.placed.as_ref() != Some(&placed) || step.is_multiple_of(10)) {
            println!("forms: step {step}: (AUDIT-A PLACEMENT) {seq}@{time:.3}: {}", placed.join("; "));
        }
        if placed.is_empty() && self.placed.is_some() {
            println!("forms: step {step}: (AUDIT-A PLACEMENT) {seq}@{time:.3}: every part back on its own socket");
        }
        self.placed = (!placed.is_empty()).then_some(placed);
        let parts = forms.len() - first;
        let mut pawns: Vec<String> = Vec::new();
        if let Some(row) = form.row() {
            let finish = rig.sora.finish();
            let (yaw_d, anim_time) = (out.yaw, out.anim.time);
            let wf = row.weapon_form;
            let mut pawn = |forms: &mut Vec<kh_er_render::FormWeapon>, model: &str, ground: Vec3, yaw: f32, clip: Option<&str>, time: f32| {
                let p = kh_er_render::keyblade::form_pawn(hand_kb, wf, model, ground, yaw, clip, time);
                pawns.push(format!("{}{} {}@{time:.2}", if model.is_empty() { "(first pawn)" } else { model }, if p.is_some() { "" } else { " NOT DRAWN (not in the pack / not loaded)" }, clip.unwrap_or("(reference pose)")));
                forms.extend(p);
            };
            // (AUDIT-A SHIELD) as kh-er sora.rs: placed by KH3's links, M_B425 as its heaviest sample
            for p in kh_core::combat::forms::shield::finisher_pawns_at(finish, form.cmd, &out.anim) {
                let (at, yaw) = kh_core::combat::forms::shield::place_on(feet, yaw_d, &p);
                pawn(forms, p.model, at, yaw, p.clip, anim_time);
            }
            for p in kh_core::combat::forms::hammer::finisher_pawns(finish, form.cmd) {
                pawn(forms, p.model, p.at, p.yaw, Some(p.clip), anim_time);
            }
            if let Some(t) = kh_core::combat::forms::staff::tower(finish, combat) {
                pawn(forms, t.model, t.position, t.yaw, Some(t.clip), t.time);
            }
            for p in kh_core::combat::forms::ship::finisher_pawns(finish) {
                pawn(forms, p.model, feet, yaw_d, p.clip, finish.map_or(0.0, |f| f.root_len));
            }
            if combat.nano.hide_sora {
                forms.clear();
                hand_hidden = true;
            }
            if let Some(g) = combat.nano.gate {
                pawn(forms, g.model, g.position, g.yaw, Some(g.clip), g.time);
            }
        }
        // (AUDIT-B F-NANO) the flying Rocket Fists w_so173 (reference pose, yaw and pitch of the flight), as kh-er sora.rs
        for r in combat.nano.rocket_views().filter(|r| r.stage == 1) {
            let (yaw, pitch) = (r.forward.y.atan2(r.forward.x), r.forward.z.clamp(-1.0, 1.0).asin());
            if let Some(mut m) = kh_er_render::keyblade::model_at("w_so173", r.position, yaw.to_degrees()) {
                m.to_world.rotation = glam::Quat::from_rotation_z(yaw) * glam::Quat::from_rotation_y(-pitch) * glam::Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
                forms.push(m);
            }
        }
        let body_hidden = combat.nano.hide_sora || magic.shot_body_hidden;
        // a line when what is drawn changes, and every 30 steps while a form runs
        let key = (form.cmd, form.grow, format!("{} {parts} {:?} {hand_hidden} {body_hidden}", magic.shot_weapon_form, pawns.iter().map(|p| p.split('@').next().unwrap_or("")).collect::<Vec<_>>()));
        if self.told.as_ref() != Some(&key) || (form.cmd != 0 && step.is_multiple_of(30)) {
            println!(
                "forms: step {step}: form cmd {:#X} weapon form {} grow {} state {:?}: {} part(s) {:?}; pawns {:?}; hand keyblade {}; body {}",
                form.cmd,
                form.row().map_or("-", |r| r.weapon_form),
                form.grow,
                out.state,
                parts,
                forms.iter().skip(first).take(parts).map(|f| f.model.as_str()).collect::<Vec<_>>(),
                pawns,
                if hand_hidden { "hidden" } else { "drawn" },
                if body_hidden { "hidden" } else { "drawn" }
            );
            self.told = Some(key);
        }
        (hand_hidden, body_hidden)
    }
}

/// kh-er boc::frame's wear form (sora.rs `submit`: a shotlock's weapon form's wear form while it holds one).
pub fn wear_form(rig: &Rig) -> u8 {
    let shot_form = rig.sora.magic().shot_weapon_form;
    let form = rig.sora.combat().form.wear_form;
    if shot_form != 0 {
        kh_core::magic::shotlocks::vacuum::wear_form(shot_form).unwrap_or(form)
    } else {
        form
    }
}

/// kh-er sora/kbdraw.rs `unit_parts`: a shotlock's own equip set of the hand keyblade's form `form` (the form's parts of
/// every grow level and its transform units whose model a unit names), each once.
fn unit_parts(keyblade: &str, form: &str, units: &[&str], pose: &kh_er_render::keyblade::FormPose<'_>) -> Vec<kh_er_render::FormWeapon> {
    let Some(t) = kh_er_render::keyblade::table() else { return Vec::new() };
    let models: Vec<String> = units.iter().filter_map(|u| u.rsplit('/').next()).map(|b| b.to_ascii_lowercase().trim_end_matches("_wep").to_string()).collect();
    let loaded = kh_er_render::keyblade::loaded();
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for grow in ["1", "2", "3"] {
        for p in kh_er_render::keyblade::form_parts(keyblade, form, grow, true) {
            if models.contains(&p.model) && !seen.contains(&p.model) && loaded.contains(&p.model) {
                seen.push(p.model.clone());
                out.extend(kh_er_render::keyblade::place_part(t, &p, pose));
            }
        }
    }
    out
}

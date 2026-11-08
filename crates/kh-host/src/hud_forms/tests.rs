//! (F-HUD) The form HUD with a real Sora (kh-host's `Rig`: the core, the level-99 loadout, a training dummy) and the real
//! kh_ui movies, drawn with the kh_ui software renderer. Pictures: build/sora_tmp/weapons/F-HUD/shots/hud_*.png.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3};
use kh_camera::forms::{FormCameras, FormShot};
use kh_camera::{CameraTarget, FieldCamera, TargetState, CAMERA_PARAMS};
use kh_core::host::{NullAudio, NullEffects};
use kh_core::{Input, PadButton, PadButtons, PadState, StateId};
use kh_ui::command::{CommandDisplay, CommandModel, CommandStrings, MenuRow, MenuState};
use kh_ui::def::write_png;
use kh_ui::forms_hud::CommandDim;
use kh_ui::hud_command::HudCommand;
use kh_ui::hud_target::{HudTarget, TargetDriver, View, LOCKON_TYPE_AUTO};
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer};

use super::FormsHud;
use crate::rig::Rig;

const DT: f32 = 1.0 / 60.0;
const W: f32 = 1920.0;
const H: f32 = 1080.0;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The pieces of the HUD the forms touch, as `Hud` holds them.
struct Bench {
    rig: Rig,
    forms: FormsHud,
    target: HudTarget,
    driver: TargetDriver,
    command: HudCommand,
    display: CommandDisplay,
    strings: CommandStrings,
    fonts: Fonts,
    cameras: FormCameras,
    field: FieldCamera,
    names: HashMap<i64, String>,
    held: PadButtons,
    /// The camera drawn last (eye, rotation, horizontal FOV).
    eye: (Vec3, kh_camera::Rot, f32),
}

fn names(pack: &Pack) -> HashMap<i64, String> {
    let mut out = HashMap::new();
    if let Ok(data) = pack.data("commands") {
        for row in data["rows"].as_array().into_iter().flatten() {
            if let (Some(id), Some(name)) = (row["id"].as_i64(), row["name_en"].as_str()) {
                out.insert(id, name.to_string());
            }
        }
    }
    out
}

impl Bench {
    fn new(weapon: &str) -> Option<Bench> {
        let Ok(mut pack) = Pack::open_default() else {
            eprintln!("SKIPPED: no build/sora_pack/ui");
            return None;
        };
        let fonts = Fonts::load(&mut pack).ok()?;
        let mut rig = match Rig::new(&root().join("build/sora_pack"), &root().join("sheets/port/sora-loadout-99.json"), Some(Vec3::new(700.0, 0.0, 0.0))) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("SKIPPED: {e}");
                return None;
            }
        };
        rig.sora.status_mut().weapon = weapon.to_string();
        let mut target = HudTarget::new(pack.movie("HudTarget").ok()?, fonts.clone());
        target.native_root("loop");
        let command = HudCommand::new(pack.movie("HudCommand").ok()?, fonts.clone());
        let forms = FormsHud::load(&mut pack, &fonts);
        let field = FieldCamera::new(CAMERA_PARAMS, &camera_target(&rig));
        let names = names(&pack);
        let strings = CommandStrings::load(&mut pack);
        let mut b = Bench { rig, forms, target, driver: TargetDriver::default(), command, display: CommandDisplay::new(), strings, fonts, cameras: FormCameras::default(), field, names, held: PadButtons::NONE, eye: (Vec3::ZERO, kh_camera::Rot::default(), 100.0) };
        b.frames(&[], 5);
        Some(b)
    }

    /// One fixed step of the core with `held` (the camera of the last frame as its view), the cameras, then one HUD frame.
    fn frame(&mut self, held: &[PadButton]) -> super::FormsFrame {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let (eye, rot, fov) = self.eye;
        self.rig.view = Some(kh_core::magic::View::from_camera(eye, rot.yaw, rot.pitch, fov, W as i32, H as i32));
        let input = Input { view: self.rig.view, camera_yaw: rot.yaw, ..Input::default() }.with_pad(PadState::from_held(set, self.held));
        self.held = set;
        self.rig.step_with(&input, &mut NullAudio, &mut NullEffects);
        // The cameras: the field camera, then the form cameras on top.
        let t = camera_target(&self.rig);
        let under = self.field.step(DT, &kh_camera::CameraInput::default(), &t, &self.rig.world);
        let point = |id: kh_core::host::TargetId| self.rig.dummy.as_ref().filter(|d| d.info.id == id).map(|d| d.info.center);
        let shot = FormShot::of(&self.rig.sora.combat().arrowgun.aim, self.rig.sora.finish(), self.rig.sora.combat().form.cmd, &point);
        let shown = self.cameras.step(DT, shot, &t, &under, glam::Vec2::ZERO, &self.rig.world);
        self.eye = match shown {
            Some(v) => (v.view.position, v.view.rotation, v.fov),
            None => (under.position, under.rotation, 100.0),
        };
        // The HUD frame: the forms first (as Hud::frame would, before the cursors' reconcile).
        let out = self.forms.frame(&self.rig.sora, &mut self.target, false);
        let mut sounds = Vec::new();
        let shown_keys: Vec<u64> = self.driver.cursors.iter().filter(|c| c.ty == LOCKON_TYPE_AUTO).map(|c| c.key).collect();
        for k in &shown_keys {
            if !out.markers.contains(k) {
                self.driver.remove_cursor(&mut self.target, *k, LOCKON_TYPE_AUTO, &mut sounds);
            }
        }
        for k in &out.markers {
            if !shown_keys.contains(k) {
                self.driver.add_cursor(&mut self.target, *k, LOCKON_TYPE_AUTO, false, &mut sounds);
            }
        }
        let view = self.hud_view();
        let dummy = self.rig.dummy.as_ref().map(|d| (d.info.id.0 as u64, d.info.center));
        let position_of = |key: u64| dummy.filter(|d| d.0 == key).map(|d| d.1);
        self.driver.update(&mut self.target, DT, Some(&view), &position_of, &mut sounds);
        let mut model = self.model();
        if let Some(CommandDim::Dark) = out.dim {
            self.command.set_action_select(kh_ui::hud_command::ACTION_COMMAND_DISABLE, 0, 0);
        }
        if let Some(CommandDim::Back) = out.dim {
            self.command.set_action_select(kh_ui::hud_command::ACTION_COMMAND_ENABLE, 0, 0);
        }
        self.display.update(&mut self.command, &model, &self.strings, DT, &mut sounds);
        model.events.clear();
        self.target.tick();
        self.command.tick();
        self.forms.tick();
        out
    }

    fn frames(&mut self, held: &[PadButton], n: usize) {
        for _ in 0..n {
            self.frame(held);
        }
    }

    /// The command menu's model as `live_model` builds it for these pieces (the four slots, the form gauge, then F-HUD's).
    fn model(&self) -> CommandModel {
        let mut model = CommandModel { state: MenuState::MainSel, battle_mode: 2, link_none: true, ..CommandModel::default() };
        for (slot, text) in ["Attack", "Magic", "Items", ""].into_iter().enumerate() {
            model.main[slot] = MenuRow { text: text.to_string(), enabled: !text.is_empty(), ..MenuRow::default() };
        }
        let form = self.rig.ui.snapshot.situation.form.as_ref();
        if let Some(f) = form {
            model.form_gauge = Some((f.left, f.total));
        }
        self.forms.fill_model(&self.rig.sora, form, &self.names, &mut model);
        model
    }

    /// The kh_ui projection of the camera drawn last (KH axes: X forward, Y right, Z up: left-handed).
    fn hud_view(&self) -> View {
        let (eye, rot, fov) = self.eye;
        let fov_y = 2.0 * ((fov.to_radians() * 0.5).tan() / (W / H)).atan();
        let clip = Mat4::perspective_lh(fov_y, W / H, 10.0, 1.0e6) * Mat4::look_to_lh(eye, rot.forward(), Vec3::Z);
        View { clip_from_world: clip, rect: (0.0, 0.0, W, H) }
    }

    fn transform(&mut self, cmd: i32) {
        self.rig.sora.debug_add_style(100.0);
        self.frames(&[], 15);
        for _ in 0..40 {
            self.frame(&[PadButton::SITUATION]);
            self.frame(&[]);
            if self.rig.out.state == StateId::Stylechange {
                break;
            }
        }
        for _ in 0..600 {
            self.frame(&[]);
            if self.rig.out.state != StateId::Stylechange {
                break;
            }
        }
        assert_eq!(self.rig.sora.combat().form.cmd, cmd, "the form is on");
    }

    /// Puts the dummy `ahead` cm in front of Sora (along his facing).
    fn place_dummy(&mut self, ahead: f32) {
        let (p, yaw) = (self.rig.out.position, self.rig.out.yaw.to_radians());
        if let Some(d) = self.rig.dummy.as_mut() {
            d.info.position = Vec3::new(p.x + ahead * yaw.cos(), p.y + ahead * yaw.sin(), 0.0);
            d.info.center = d.info.position + Vec3::Z * 90.0;
        }
    }

    fn shot(&self, name: &str) {
        let dir = root().join("build/sora_tmp/weapons/F-HUD/shots");
        std::fs::create_dir_all(&dir).ok();
        let mut canvas = Canvas::new(W as usize, H as usize);
        let mut renderer = Renderer::new(self.fonts.clone());
        let mut movies: Vec<&Movie> = vec![&self.target.movie, &self.command.movie];
        if let Some(m) = self.forms.operation_movie() {
            movies.push(m);
        }
        for m in movies {
            renderer.draw(&mut canvas, m, &Matrix::IDENTITY);
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / H), 0.38, 0.46], &mut rgba);
        write_png(&dir.join(format!("{name}.png")), W as u32, H as u32, &rgba).unwrap();
    }
}

fn camera_target(rig: &Rig) -> CameraTarget {
    CameraTarget { position: rig.out.position, yaw: rig.out.yaw, velocity: Vec3::ZERO, state: TargetState::Other, collision_half_height: 89.0 }
}

#[test]
fn double_arrowguns_multilock_markers_then_the_shooter_scope_guide_and_camera() {
    let Some(mut b) = Bench::new("WEP_KEYBLADE_SO_02") else { return };
    b.transform(kh_core::combat::forms::arrowgun::CHANGE_DW);
    b.place_dummy(700.0);
    b.frames(&[], 30);
    // The multi-lock: a type-0 marker on the dummy; its camera is on top of the field camera.
    let f = b.frame(&[]);
    assert!(b.rig.sora.combat().arrowgun.aim.multilock);
    assert_eq!(f.markers, vec![1], "the dummy is listed");
    assert!(b.driver.has_cursor(1, LOCKON_TYPE_AUTO));
    assert!(b.cameras.active(), "the multi-lock camera");
    assert!(b.command.act_logo_clip().is_some(), "Double Arrowguns' logo on the plate");
    b.shot("hud_multilock_dw");
    // RT held: ShooterMode, its camera (a cut), the white scope, the guide, the gauge on the aim's target.
    let mut dark = false;
    let mut gauge = None;
    for _ in 0..60 {
        let f = b.frame(&[PadButton::RightTrigger]);
        dark |= f.dim == Some(CommandDim::Dark);
        gauge = gauge.or(f.gauge_target);
    }
    assert_eq!(b.rig.out.state, StateId::Shootermode);
    assert!(b.rig.sora.combat().arrowgun.aim.view.camera);
    assert!(dark, "the situation commands darken");
    // The scope is above the dummy (the eye is at head height, the dummy's centre at 90 cm): the right stick (the
    // state's manual aim, `Sora::set_shooter_look`, KH3's inverted Y: up = aim down) brings it onto it.
    for _ in 0..90 {
        if gauge.is_some() {
            break;
        }
        b.rig.sora.set_shooter_look(glam::Vec2::new(0.0, 0.7));
        gauge = gauge.or(b.frame(&[PadButton::RightTrigger]).gauge_target);
    }
    b.rig.sora.set_shooter_look(glam::Vec2::ZERO);
    assert_eq!(gauge, Some(1), "the enemy gauge points at the aim's target");
    assert!(b.target.calls.iter().any(|c| c == "setShooterModeScope(0, 0)"), "{:?}", b.target.calls);
    assert_eq!(b.target.movie.current_label("shootScpW").map(|l| l != "off"), Some(true));
    assert!(b.forms.operation.as_ref().is_some_and(|g| g.is_open()), "the operation guide");
    assert!(b.frame(&[PadButton::RightTrigger]).markers.is_empty(), "no markers in the shooter aim");
    assert!((b.eye.2 - kh_camera::forms::SHOOTER_FOV).abs() < 1e-3, "the shooter camera's FOV 60");
    // The screen centre (the scope) is on the aim: the camera looks through the state's look-at point.
    let v = b.rig.sora.combat().arrowgun.aim.view;
    let ray = (v.look_at - b.eye.0).normalize();
    assert!(ray.dot(b.eye.1.forward()) > 0.999, "the scope's centre is the aim");
    b.frames(&[PadButton::RightTrigger], 20);
    b.shot("hud_shooter_dw");
    // Release: the scope out, the guide closes, the multi-lock again.
    let mut back = false;
    for _ in 0..60 {
        back |= b.frame(&[]).dim == Some(CommandDim::Back);
    }
    assert!(back);
    assert!(b.target.calls.iter().any(|c| c == "setShooterModeScope(1, 0)"));
    assert!(!b.forms.operation.as_ref().is_some_and(|g| g.is_open()));
    assert!(b.rig.sora.combat().arrowgun.aim.multilock);
}

#[test]
fn magic_launcher_s_scope_charges_to_full() {
    let Some(mut b) = Bench::new("WEP_KEYBLADE_SO_02") else { return };
    b.transform(kh_core::combat::forms::arrowgun::CHANGE_DW);
    b.frames(&[], 20);
    b.transform(kh_core::combat::forms::arrowgun::CHANGE_BZ);
    b.place_dummy(700.0);
    b.frames(&[], 30);
    let mut charging_shot = false;
    for k in 0..240 {
        b.frame(&[PadButton::RightTrigger]);
        let v = b.rig.sora.combat().arrowgun.aim.view;
        if v.camera && v.charge > 40.0 && !charging_shot {
            b.shot("hud_shooter_bz_charging");
            charging_shot = true;
        }
        if v.charged {
            b.frames(&[PadButton::RightTrigger], 4);
            b.shot("hud_shooter_bz_full");
            break;
        }
        assert!(k < 239, "no full charge");
    }
    assert!(b.target.calls.iter().any(|c| c == "setShooterModeScope(0, 1)"), "the launchers' scope: {:?}", b.target.calls);
    assert!(b.target.calls.iter().any(|c| c == "setShooterModeScope(3, 1)"), "the full charge: {:?}", b.target.calls);
    assert_eq!(b.target.movie.current_label("shootScpB"), Some("change_full"));
}

#[test]
fn counter_shield_s_counter_label_and_the_ride_s_gauge_and_cameras() {
    let Some(mut b) = Bench::new("WEP_KEYBLADE_SO_01") else { return };
    b.transform(kh_core::combat::forms::shield::CHANGE_SH);
    b.frames(&[], 20);
    // A counter window (as kh-core's form_counter opens it on a block): the Attack slot shows its command, flashing.
    let grown = b.rig.sora.combat().form.grow > 1;
    {
        let a = b.rig.sora.actions_mut();
        a.counter_window = 2.0;
        a.counter_command = if grown { 0x11 } else { 0x10 };
    }
    let model = b.model();
    assert_eq!(model.main[0].text, "Wrathful Fist");
    assert!(model.main[0].flash);
    b.frames(&[], 3);
    b.shot("hud_shield_counter");
    b.rig.sora.actions_mut().counter_window = 0.0;
    assert_eq!(b.model().main[0].text, "Attack");
    // The Ultimate Finish: the fixed camera, then the ride behind Pegasus; the logo gauge runs on the ride's timer.
    b.rig.sora.debug_add_style(100.0);
    b.frames(&[], 15);
    let mut saw = (false, false, false);
    for _ in 0..900 {
        b.frame(&[PadButton::SITUATION]);
        b.frame(&[]);
        let shot = kh_camera::forms::FormShot::of(&b.rig.sora.combat().arrowgun.aim, b.rig.sora.finish(), b.rig.sora.combat().form.cmd, &|_| None);
        saw.0 |= shot == Some(FormShot::ShieldFix);
        if shot == Some(FormShot::ShieldRide) {
            saw.1 = true;
            if let Some((left, _)) = super::ride_gauge(&b.rig.sora, 30.0) {
                if left < 6.0 && !saw.2 {
                    saw.2 = true;
                    let model = b.model();
                    assert_eq!(model.form_gauge.map(|g| g.0), Some(left));
                    b.shot("hud_shield_ride");
                }
            }
        }
        if saw.2 {
            break;
        }
    }
    assert!(saw.0, "the fixed camera (sub 0 .. 1)");
    assert!(saw.1, "the camera behind Pegasus (sub 2 .. 4)");
    assert!(saw.2, "the ride's gauge");
}

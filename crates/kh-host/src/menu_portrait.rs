//! (IMPL_ER21) SORA'S MODEL IN THE CAMP MENU'S CHARACTER PANEL: the host-independent half.
//!
//! The game draws each party member for the camp menu with a "UI actor" blueprint
//! (`/Game/UI/Blueprints/CampTop/p_ex001_UIActor`): a skeletal mesh that plays the camp anim set (`Idle_CM`, with
//! `IdleBreak_CM` once every 2 .. 5 loops), a scene capture (512 x 800, FOV 30 degrees over the larger side, 250 cm in
//! front of him at 130 cm) and two spot lights; the picture goes into the clip `charaWindow01.charaAnm.chara` of the
//! CampMenu movie (and into the character picture of the Abilities and Stats screens). `PACK/menu/menu.json`
//! (tools/sora_material_pack.js) carries those numbers and the two clips.
//!
//! WHAT IS HERE, for every host:
//! - WHERE THE PICTURE GOES ([`panel`]): the movie's display list walked down to the panel: the stage rectangle the
//!   capture's picture covers, the rectangle its mask leaves visible (cut at the top of the name / LV / HP / MP
//!   block, which lies over the picture), the clip's opacity. MOVED here from kh-app/src/menu_model.rs (which now
//!   calls it): the test app and the Elden Ring mod place him by the same code.
//! - THE UI ACTOR'S DATA ([`PortraitData`]): capture, clips, the two spots and the ambient cube as a `kh_look::Rig`.
//! - THE IDLE CYCLE ([`IdleCycle`]) of `UTresUIActorAnimInstance`.
//! - A POSED COPY ([`MenuPortrait`]) for a host that draws him itself (the Elden Ring renderer): its own
//!   `kh_anim::Animator`, stepped only while the panel shows, and what a renderer needs for one picture
//!   ([`PortraitOut`]). The test app keeps its own copy (Bevy entities), driven by the same data and rules.

use std::path::Path;

use glam::{Quat, Vec3};
use kh_anim::{Animator, BodySlot, Frame, NoIkWorld, Settings, Trs};
use kh_look::{Env, Rig, Spot};
use kh_ui::player::{Kind, Sprite};
use kh_ui::{Cx, Matrix, Movie, Rect};
use serde_json::Value;

/// The movie's clips the game puts the capture's picture in (the top screen's panel, the character picture of the
/// Abilities and of the Stats screen).
pub const PANELS: [&str; 3] = ["topMenu.statusAnm.status.charaWindow01", "ability.abilityAnm.selectCharaImage1", "status.statusAnm.selectCharaImage2"];
/// `UTresUIActorAnimInstance::m_IdleRandomMin / Max` (constructor 0x141531AC0).
pub const IDLE_RANDOM: (u32, u32) = (2, 5);
/// Steps the hair and cloth are given to settle before the first picture (LOCAL: the game's
/// `m_BonamikPreRollNumDefault` of TresCampPawnBase was not read).
pub const PRE_ROLL_STEPS: u32 = 45;
/// The factor in front of the tone curve for the menu's picture (kh-app char_material.rs `MENU_EXPOSURE`).
pub const MENU_EXPOSURE: f32 = 1.0 / 0.6;
/// The capture's clear colour: the panel's dark blue, transparent (the model's edge pixels are resolved against it).
pub const CLEAR: [f32; 4] = [0.03, 0.06, 0.16, 0.0];

/// Where the panel is: the stage rectangle the capture's picture covers, the rectangle its mask leaves visible, and
/// the clip's opacity.
#[derive(Clone, Copy, Debug)]
pub struct Panel {
    pub picture: Rect,
    pub mask: Rect,
    pub alpha: f32,
}

fn shape_bounds(sprite: &Sprite, m: &Matrix, out: &mut Rect) {
    for node in sprite.children.iter().chain(&sprite.extra) {
        let world = m.mul(&node.matrix);
        match &node.kind {
            Kind::Shape(shape) => *out = out.union(&shape.bounds.rect().transform(&world)),
            Kind::Sprite(child) => shape_bounds(child, &world, out),
            _ => {}
        }
    }
}

/// Walks the movie's display list down to the panel. `None`: the panel is not on screen. `target` = the capture's
/// size; `zoom` / `shift`: an enlargement and a raise of the picture (1, 0 = as the movie places its placeholder).
pub fn panel(movie: &Movie, target: (u32, u32), zoom: f32, shift: f32) -> Option<Panel> {
    PANELS.iter().find_map(|path| panel_at(movie, path, target, zoom, shift))
}

pub fn panel_at(movie: &Movie, path: &str, target: (u32, u32), zoom: f32, shift: f32) -> Option<Panel> {
    let (mut sprite, mut matrix, mut cx) = (&movie.root, Matrix::IDENTITY, Cx::IDENTITY);
    for name in path.split('.') {
        let node = sprite.child(name)?;
        if !node.is_visible() {
            return None;
        }
        matrix = matrix.mul(&node.matrix);
        cx = cx.concat(&node.cx);
        sprite = node.sprite()?;
    }
    // The mask of the panel's content: the clip `shapeMask` (its shapes at the frame of the party size).
    let mut mask = Rect::EMPTY;
    if let Some(node) = sprite.child("shapeMask") {
        if let Some(s) = node.sprite() {
            shape_bounds(s, &matrix.mul(&node.matrix), &mut mask);
        }
    } else if let Some((node, shape)) = sprite.children.iter().find_map(|n| match &n.kind {
        // (the select panes: an unnamed clipping shape is the picture's mask)
        Kind::Shape(shape) if n.clip != 0 => Some((n, shape)),
        _ => None,
    }) {
        mask = shape.bounds.rect().transform(&matrix.mul(&node.matrix));
    }
    // The picture's clip: `charaAnm` (the appear animation) > `chara` (the placeholder; hidden
    // until the game has the picture).
    let anm = sprite.child("charaAnm")?;
    if !anm.is_visible() {
        return None;
    }
    // What the movie draws over the picture: the unnamed clip above it in the panel's display
    // list is the background of the name / LV / HP / MP block (stage y 733 .. 873 for one
    // member). The picture ends at its top edge (the game's shows again in the 15 pixel strip
    // under the block: not drawn here).
    for node in sprite.children.iter().filter(|n| n.depth > anm.depth && n.name.is_none() && n.is_visible()) {
        if let Some(s) = node.sprite() {
            let mut block = Rect::EMPTY;
            shape_bounds(s, &matrix.mul(&node.matrix), &mut block);
            if !block.is_empty() && !mask.is_empty() && block.y0 > mask.y0 {
                mask.y1 = mask.y1.min(block.y0);
            }
        }
    }
    let (m, c) = (matrix.mul(&anm.matrix), cx.concat(&anm.cx));
    let chara = anm.sprite()?.child("chara")?;
    let (m, c) = (m.mul(&chara.matrix), c.concat(&chara.cx));
    let placeholder = placeholder_rect(chara.sprite(), &m);
    // The placeholder is a transparent 278 x 800 rectangle (for one member) that the game
    // replaces by the capture's 512 x 800 picture. HOW it places the picture there was not read
    // (the native side of the camp menu): LOCAL: the picture is centred on the clip, enlarged by
    // `zoom` and raised by `shift` stage pixels; the panel's mask cuts what sticks out.
    let picture = picture_rect(placeholder, &m, target, zoom, shift);
    let alpha = (c.mul[3] + c.add[3] / 255.0).clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return None;
    }
    Some(Panel { picture, mask, alpha })
}

/// (IMPL_ER34) THE `chara` CLIP'S PLACEHOLDER RECTANGLE on the stage (`m` = the clip's matrix down from the root).
/// The clip is a `tres.controls.Icon` (class `Chara`): its constructor measures the authoring placeholder
/// (`m_defaultSize`, 278 x 800, at the clip's origin) and then REMOVES it (`removeChildren`), so since the Icon control
/// was ported (the item pictures) the clip has no shape left to measure: the size the constructor kept is the
/// rectangle. (Before this the empty clip sent the picture down the "no placeholder" path: its LEFT EDGE on the clip's
/// origin instead of its CENTRE on the clip's centre = (512 - 278) / 2 = 117 stage pixels too far right: the user's
/// "Sora is off centre in the pause menu".) A clip that still has its shapes is measured as before.
pub fn placeholder_rect(chara: Option<&Sprite>, m: &Matrix) -> Rect {
    let Some(sprite) = chara else { return Rect::EMPTY };
    if let Some(icon) = sprite.icon.filter(|i| i.size.0 > 0.0 && i.size.1 > 0.0) {
        let (x, y, w, h) = icon.clip.unwrap_or((0.0, 0.0, icon.size.0, icon.size.1));
        return Rect::new(x, y, x + w, y + h).transform(m);
    }
    let mut placeholder = Rect::EMPTY;
    shape_bounds(sprite, m, &mut placeholder);
    placeholder
}

/// (IMPL_ER34) WHERE THE CAPTURE'S PICTURE LIES ON THE STAGE, a pure function of the placeholder's rectangle: the
/// capture (`target`, 512 x 800) at the placeholder's HEIGHT, its CENTRE on the placeholder's centre, enlarged by `zoom`
/// and raised by `shift` (LOCAL rule: how the game's native side lays the picture into the clip was not read; this is
/// what agrees with the capture of the real menu: he stands in the middle of his panel). Everything is in STAGE
/// units: the output's size and aspect only enter through the stage -> output matrix the hosts apply afterwards (a
/// centred 16:9 rectangle at any resolution), so his place relative to the menu's panels is the same at every
/// resolution and aspect. Without any placeholder (an unknown movie) the picture lies at the clip's origin and scale.
pub fn picture_rect(placeholder: Rect, m: &Matrix, target: (u32, u32), zoom: f32, shift: f32) -> Rect {
    if placeholder.is_empty() {
        let (x0, y0) = m.apply(0.0, 0.0);
        let (x1, y1) = m.apply(target.0 as f32, target.1 as f32);
        return Rect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
    }
    let scale = (placeholder.y1 - placeholder.y0) / target.1.max(1) as f32;
    let (half, height) = (target.0 as f32 * scale * zoom * 0.5, target.1 as f32 * scale * zoom);
    let centre = (placeholder.x0 + placeholder.x1) * 0.5;
    let top = placeholder.y0 - shift * scale;
    Rect::new(centre - half, top, centre + half, top + height)
}

/// `PACK/menu/menu.json`: the UI actor, in ITS OWN space (cm, X forward, Y right, Z up; the actor at the origin).
#[derive(Clone, Debug)]
pub struct PortraitData {
    pub target: (u32, u32),
    pub fov_deg: f32,
    pub capture: Vec3,
    pub capture_yaw_deg: f32,
    /// The mesh component's yaw in the actor (-90: the model faces +X, toward the capture).
    pub mesh_yaw_deg: f32,
    /// `Idle_CM`: sequence, file, length.
    pub clip: (String, String, f32),
    /// `IdleBreak_CM`.
    pub idle_break: Option<(String, String, f32)>,
    /// The two spot lights and the ambient cube map (its face means stand in for it).
    pub rig: Rig,
}

fn vec3_of(v: &Value) -> Vec3 {
    let f = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
    Vec3::new(f("x"), f("y"), f("z"))
}

impl PortraitData {
    pub fn load(pack: &Path) -> Result<PortraitData, String> {
        let text = std::fs::read_to_string(pack.join("menu").join("menu.json")).map_err(|e| format!("no PACK/menu/menu.json ({e}): node tools/sora_material_pack.js"))?;
        let json: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let number = |v: &Value, k: &str| v.get(k).and_then(Value::as_f64).map(|x| x as f32);
        let clip = json.get("clips").and_then(|c| c.get(0)).ok_or("menu.json names no clip")?;
        let spots = json
            .get("lights")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(2)
            .map(|l| {
                let rotation = l.get("rotation_deg").cloned().unwrap_or_default();
                let (pitch, yaw) = (number(&rotation, "pitch").unwrap_or(0.0).to_radians(), number(&rotation, "yaw").unwrap_or(0.0).to_radians());
                Spot {
                    position: vec3_of(l.get("location_cm").unwrap_or(&Value::Null)),
                    // A rotator's forward vector.
                    direction: Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin()),
                    intensity: number(l, "intensity").unwrap_or(5000.0),
                    inner_cone_deg: number(l, "inner_cone_deg").unwrap_or(0.0),
                    outer_cone_deg: number(l, "outer_cone_deg").unwrap_or(44.0),
                    radius: number(l, "attenuation_radius_cm").unwrap_or(1000.0),
                    min_roughness: number(l, "min_roughness").unwrap_or(0.08),
                }
            })
            .collect();
        // The ambient cube map (AmbientCubemapIntensity x the mean of each face): a constant band and a vertical
        // gradient from the +Z / -Z faces for the diffuse part, a cube of the six face means for the reflections.
        let intensity = json.get("post_process").and_then(|p| number(p, "AmbientCubemapIntensity")).unwrap_or(0.0);
        let faces: Vec<Vec3> = json.get("ambient_cube_face_means").and_then(Value::as_array).into_iter().flatten().map(|f| Vec3::new(f[0].as_f64().unwrap_or(0.0) as f32, f[1].as_f64().unwrap_or(0.0) as f32, f[2].as_f64().unwrap_or(0.0) as f32)).collect();
        let (mean, top, bottom) = if faces.len() == 6 { (faces.iter().sum::<Vec3>() / 6.0, faces[4], faces[5]) } else { (Vec3::splat(0.35), Vec3::splat(0.4), Vec3::splat(0.3)) };
        let luminance = |c: Vec3| c.dot(Vec3::new(0.3, 0.59, 0.11));
        let rig = Rig {
            spots,
            key: Vec3::ONE,
            key_of_level: false,
            ambient: mean * intensity,
            ambient_of_level: false,
            sky_gradient: ((luminance(top) - luminance(bottom)) * 0.5 / luminance(mean).max(1.0e-4)).clamp(-1.0, 1.0),
            shadows: false,
            env: if faces.len() == 6 { Env::Faces([faces[0] * intensity, faces[1] * intensity, faces[2] * intensity, faces[3] * intensity, faces[4] * intensity, faces[5] * intensity]) } else { Env::Sky { zenith: top * intensity, horizon: mean * intensity, ground: bottom * intensity } },
            points: Vec::new(),
        };
        let target = json.get("target").cloned().unwrap_or_default();
        let capture = json.get("capture").cloned().unwrap_or_default();
        let named = |c: &Value| Some((c.get("sequence")?.as_str()?.to_string(), c.get("file")?.as_str()?.to_string(), number(c, "duration_s")?));
        Ok(PortraitData {
            target: (number(&target, "width").unwrap_or(512.0) as u32, number(&target, "height").unwrap_or(800.0) as u32),
            fov_deg: number(&capture, "fov_deg").unwrap_or(30.0),
            capture: vec3_of(capture.get("location_cm").unwrap_or(&Value::Null)),
            capture_yaw_deg: capture.get("rotation_deg").and_then(|r| number(r, "yaw")).unwrap_or(180.0),
            mesh_yaw_deg: json.get("mesh").and_then(|m| m.get("rotation_deg")).and_then(|r| number(r, "yaw")).unwrap_or(-90.0),
            clip: named(clip).ok_or("menu.json: the first clip has no sequence / file / duration")?,
            idle_break: json.get("clips").and_then(Value::as_array).into_iter().flatten().find(|c| c.get("name").and_then(Value::as_str) == Some("IdleBreak_CM")).and_then(named),
            rig,
        })
    }

    /// The capture's VERTICAL field of view (radians) and aspect: stock UE 4.17 `BuildProjectionMatrix` of a scene
    /// capture gives FOVAngle to the LARGER side of the target (here the 800 pixel height).
    pub fn projection(&self) -> (f32, f32) {
        let (w, h) = (self.target.0.max(1) as f32, self.target.1.max(1) as f32);
        let half = (self.fov_deg.to_radians() * 0.5).tan();
        (2.0 * (if w > h { half * h / w } else { half }).atan(), w / h)
    }
}

/// THE CAMP IDLE of the UI actor's anim instance (`UTresUIActorAnimInstance`, native update 0x141540B10): `Idle_CM`
/// loops; each time it ends a counter goes up, and when it reaches a number drawn from `IDLE_RANDOM` (drawn again
/// after every break, 0x14153D2F0) `IdleBreak_CM` plays once.
#[derive(Clone, Copy, Debug, Default)]
pub struct IdleCycle {
    /// Position in the playing clip, seconds.
    pub time: f32,
    pub in_break: bool,
    idle_ends: u32,
    idle_limit: u32,
    random: u32,
}

impl IdleCycle {
    /// `FMath::RandRange(m_IdleRandomMin, m_IdleRandomMax)`, on a generator of its own.
    fn draw(state: &mut u32) -> u32 {
        *state ^= *state << 13;
        *state ^= *state >> 17;
        *state ^= *state << 5;
        let span = IDLE_RANDOM.1 - IDLE_RANDOM.0 + 1;
        IDLE_RANDOM.0 + (((*state >> 8) as f32 * 5.960_464_5e-8 * span as f32) as u32).min(span - 1)
    }

    /// One step of `dt`: the clip position before it and whether the break plays AFTER it. `idle` / `idle_break` =
    /// the clips' lengths (`None`: the actor has no break).
    pub fn advance(&mut self, dt: f32, idle: f32, idle_break: Option<f32>) -> (f32, bool) {
        if self.idle_limit == 0 {
            self.random = 0x2F6E_2B1D;
            self.idle_limit = Self::draw(&mut self.random);
        }
        let length = match (idle_break, self.in_break) {
            (Some(length), true) => length,
            _ => idle,
        };
        let from = self.time;
        self.time += dt;
        if self.time >= length {
            self.time -= length;
            if self.in_break {
                self.in_break = false;
                self.idle_limit = Self::draw(&mut self.random);
            } else if idle_break.is_some() {
                self.idle_ends += 1;
                if self.idle_ends >= self.idle_limit {
                    self.idle_ends = 0;
                    self.in_break = true;
                }
            }
        }
        (from, self.in_break)
    }
}

/// What a renderer needs for one picture of the panel. Positions and directions are in the UI actor's space (KH3
/// axes, cm; the actor at the origin), rectangles in pixels of the output.
#[derive(Clone, Debug)]
pub struct PortraitOut {
    /// The part of the picture that shows: x, y, width, height.
    pub visible: (i32, i32, u32, u32),
    /// Where the WHOLE capture lies (it may stick out of `visible` on every side): x, y, width, height.
    pub picture: (f32, f32, f32, f32),
    /// The panel clip's opacity (the menu's open / close animation).
    pub alpha: f32,
    /// The capture: place, the unit axes forward / right / up, the vertical field of view (radians), width / height.
    pub eye: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub fov_y: f32,
    pub aspect: f32,
    /// The mesh component and its pose (every bone relative to its parent / to the component).
    pub component_to_world: Trs,
    pub local: Vec<Trs>,
    pub component: Vec<Trs>,
    /// The seconds the copy has been posed for (the materials' clock).
    pub clock: f32,
}

/// The posed copy of a host that draws the picture itself.
pub struct MenuPortrait {
    pub data: PortraitData,
    animator: Animator,
    cycle: IdleCycle,
    rolled: bool,
    clock: f32,
    /// Pictures made, seconds spent posing.
    pub frames: u64,
    pub seconds: f64,
}

impl MenuPortrait {
    /// Reads the pose evaluator with the two camp clips (about a second: a host does it off its frame thread).
    pub fn load(pack: &Path) -> Result<MenuPortrait, String> {
        let data = PortraitData::load(pack)?;
        let mut read = kh_anim::Pack::load(pack)?;
        for (sequence, file, length) in std::iter::once(&data.clip).chain(data.idle_break.iter()) {
            read.info.insert(sequence.clone(), kh_anim::pack::ClipInfo { length: *length, root_motion_enabled: false, interpolation_time: 0.0, notifies: Vec::new() });
            read.load_clip(pack, sequence, file)?;
        }
        let animator = Animator::new(read, Settings { leg_ik: false, kine_driver: true, face: true, bonamik: true, first_update_holds_position: kh_core::anim::FIRST_UPDATE_HOLDS_POSITION })?;
        Ok(MenuPortrait { data, animator, cycle: IdleCycle::default(), rolled: false, clock: 0.0, frames: 0, seconds: 0.0 })
    }

    /// ONE FRAME while a camp menu is up: `movie` = the CampMenu movie, `root` = stage -> output pixels, `output` =
    /// the output's size. `None`: the panel does not show (nothing was posed: it costs a walk of five clips).
    pub fn frame(&mut self, movie: &Movie, root: &Matrix, output: (u32, u32), dt: f32) -> Option<PortraitOut> {
        let placement = placement(self.data.target, movie, root, output)?;
        Some(self.frame_at(placement, dt))
    }

    /// (IMPL_ER34) One frame of the posed copy for a picture that lies at `placement` (see [`placement`] and
    /// [`PictureSync`]: a host whose menu picture comes a frame or two late places him by the state that picture shows).
    pub fn frame_at(&mut self, placement: Placement, dt: f32) -> PortraitOut {
        let started = std::time::Instant::now();
        let dt = dt.clamp(0.0, 0.1);
        let component_to_world = Trs { translation: Vec3::ZERO, rotation: Quat::from_rotation_z(self.data.mesh_yaw_deg.to_radians()), scale: Vec3::ONE };
        let steps = if self.rolled { 1 } else { PRE_ROLL_STEPS };
        let step_dt = if self.rolled { dt } else { 1.0 / 60.0 };
        self.rolled = true;
        for _ in 0..steps {
            let (from, in_break) = self.cycle.advance(step_dt, self.data.clip.2, self.data.idle_break.as_ref().map(|b| b.2));
            let clip = match (&self.data.idle_break, in_break) {
                (Some((sequence, ..)), true) => sequence.as_str(),
                _ => self.data.clip.0.as_str(),
            };
            let sequences = [clip];
            let time = self.cycle.time;
            let body = BodySlot { samples: [(clip, 1.0), (clip, 0.0)], blend_with: None, notify_sequences: &sequences, time, weight: 1.0, active: true, notify_window: (from.min(time), time) };
            self.animator.step(&Frame { dt: step_dt, body, component_to_world }, &NoIkWorld);
            self.clock += step_dt;
        }
        let pose = self.animator.pose();
        let yaw = self.data.capture_yaw_deg.to_radians();
        let (fov_y, aspect) = self.data.projection();
        self.frames += 1;
        self.seconds += started.elapsed().as_secs_f64();
        PortraitOut {
            visible: placement.visible,
            picture: placement.picture,
            alpha: placement.alpha,
            eye: self.data.capture,
            // A rotator's axes (pitch 0, roll 0): forward, right (the engine's Y axis), up.
            forward: Vec3::new(yaw.cos(), yaw.sin(), 0.0),
            right: Vec3::new(-yaw.sin(), yaw.cos(), 0.0),
            up: Vec3::Z,
            fov_y,
            aspect,
            component_to_world,
            local: pose.local.clone(),
            component: pose.component.clone(),
            clock: self.clock,
        }
    }
}

/// (IMPL_ER34) WHERE THE PICTURE LIES IN THE OUTPUT for one state of the movie: the visible part (whole pixels: x, y,
/// width, height), the whole capture (x, y, width, height) and the panel's opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub visible: (i32, i32, u32, u32),
    pub picture: (f32, f32, f32, f32),
    pub alpha: f32,
}

/// (IMPL_ER34) THE PLACEMENT FOR THIS STATE OF THE MOVIE: a pure function of the movie's display list, the stage ->
/// output matrix and the output's size. `None`: the panel does not show.
pub fn placement(target: (u32, u32), movie: &Movie, root: &Matrix, output: (u32, u32)) -> Option<Placement> {
    let panel = panel(movie, target, 1.0, 0.0)?;
    placement_of(&panel, root, output)
}

/// The output rectangles of a panel (stage units) under `root`.
pub fn placement_of(panel: &Panel, root: &Matrix, output: (u32, u32)) -> Option<Placement> {
    let picture = panel.picture.transform(root);
    let window = if panel.mask.is_empty() { picture } else { picture.intersect(&panel.mask.transform(root)) };
    let window = window.intersect(&Rect::new(0.0, 0.0, output.0 as f32, output.1 as f32));
    // (whole pixels: the picture is drawn texel for pixel)
    let (x0, y0, x1, y1) = (window.x0.round() as i32, window.y0.round() as i32, window.x1.round() as i32, window.y1.round() as i32);
    if x1 - x0 < 2 || y1 - y0 < 2 {
        return None;
    }
    Some(Placement { visible: (x0, y0, (x1 - x0) as u32, (y1 - y0) as u32), picture: (picture.x0, picture.y0, picture.x1 - picture.x0, picture.y1 - picture.y0), alpha: panel.alpha })
}

/// (IMPL_ER34) THE PORTRAIT GOES WITH THE MENU PICTURE THAT IS ON SCREEN. A host that rasterises the camp menu on a
/// worker thread (Elden Ring: about 12 ms a full-screen picture at 2560 x 1440) shows the menu's state of one or more
/// frames ago, while the portrait was placed by the movie's state of THIS frame: in every transition (open, close,
/// the top screen growing back after a sub screen) he ran ahead of his own panel. This keeps the placement that
/// belongs to the picture last handed over: `sent` / `taken` = the camp layer's counters of states sent to the
/// worker and of results taken back (`kh_host::hud_layers::Layer`), `busy` = one is being made.
#[derive(Clone, Copy, Debug, Default)]
pub struct PictureSync {
    shown: Option<Placement>,
    /// The placement of the state the worker is drawing.
    pending: Option<Option<Placement>>,
    sent: u64,
    taken: u64,
}

impl PictureSync {
    /// Once a frame, AFTER the layers were updated; `now` = the placement for the movie's state of this frame.
    /// Returns where the portrait is to be drawn (`None`: not at all).
    pub fn step(&mut self, now: Option<Placement>, sent: u64, taken: u64, busy: bool) -> Option<Placement> {
        if taken != self.taken {
            // (the worker's picture came back: what it shows is on screen from this frame on)
            self.taken = taken;
            if let Some(placement) = self.pending.take() {
                self.shown = placement;
            }
        }
        if sent != self.sent {
            self.sent = sent;
            self.pending = Some(now);
        }
        if !busy {
            // (nothing is being made: the picture on screen IS this frame's state, or was drawn in this very call)
            self.pending = None;
            self.shown = now;
        }
        self.shown
    }

    /// The menu went away: nothing is shown, nothing is waited for.
    pub fn clear(&mut self) {
        self.shown = None;
        self.pending = None;
    }
}

// ------------------------------------------------------------------------------------------- (ER party S4) the friends

/// (ER party S4) THE FRIENDS' UI ACTORS (kh-app menu_model.rs `FriendPictures`, the Elden Ring renderer's camp pictures):
/// `UI/Blueprints/CampTop/n_ex001_UIActor` captures at (300, 0, 74) cm, Goofy's `n_ex002_UIActor` at (305, 0, 123) cm, both
/// FOV 30 and yaw 180 as Sora's [A]; target, clips and the rest of the rig as Sora's (`PACK/menu/menu.json`).
pub const FRIEND_CAPTURE: [(&str, Vec3); 2] = [("n_ex001", Vec3::new(300.0, 0.0, 74.0)), ("n_ex002", Vec3::new(305.0, 0.0, 123.0))];

/// (ER party S4) The top-screen panel of each party slot (`charaWindow01` Sora, `02` / `03` the friends).
pub fn top_panel(slot: usize) -> String {
    format!("topMenu.statusAnm.status.charaWindow0{}", slot + 1)
}

/// (ER party S4) A friend's UI actor: Sora's data with the friend's capture and HIS two spot lights
/// (`PACK/party/<npc>/data/ui_actor.json`, tools/party_menu_lights.js: the lights the eyes stream applied in kh-app); without
/// the file, Sora's lights (as kh-app).
pub fn friend_data(sora: &PortraitData, pack: &Path, npc: &str) -> PortraitData {
    let mut data = sora.clone();
    if let Some((_, capture)) = FRIEND_CAPTURE.iter().find(|(n, _)| *n == npc) {
        data.capture = *capture;
    }
    let path = pack.join("party").join(npc).join("data").join("ui_actor.json");
    let json: Option<Value> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('﻿')).ok());
    let number = |v: &Value, k: &str| v.get(k).and_then(Value::as_f64).map(|x| x as f32);
    let spots: Vec<Spot> = json
        .as_ref()
        .and_then(|j| j.get("lights"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(2)
        .map(|l| {
            let rotation = l.get("rotation_deg").cloned().unwrap_or_default();
            let (pitch, yaw) = (number(&rotation, "pitch").unwrap_or(0.0).to_radians(), number(&rotation, "yaw").unwrap_or(0.0).to_radians());
            Spot {
                position: vec3_of(l.get("location_cm").unwrap_or(&Value::Null)),
                direction: Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin()),
                intensity: number(l, "intensity").unwrap_or(5000.0),
                inner_cone_deg: number(l, "inner_cone_deg").unwrap_or(0.0),
                outer_cone_deg: number(l, "outer_cone_deg").unwrap_or(44.0),
                radius: number(l, "attenuation_radius_cm").unwrap_or(1000.0),
                min_roughness: number(l, "min_roughness").unwrap_or(0.08),
            }
        })
        .collect();
    if spots.len() == 2 {
        data.rig.spots = spots;
    }
    data
}

/// (ER party S4) What a renderer needs for a friend's picture: his data (`friend_data`), where the panel lies, his posed
/// camp copy (kh-partyhost `MenuPoser`: component at the origin, turned by the mesh yaw) and its clock.
pub fn friend_out(data: &PortraitData, placement: Placement, local: Vec<Trs>, component: Vec<Trs>, clock: f32) -> PortraitOut {
    let yaw = data.capture_yaw_deg.to_radians();
    let (fov_y, aspect) = data.projection();
    PortraitOut {
        visible: placement.visible,
        picture: placement.picture,
        alpha: placement.alpha,
        eye: data.capture,
        forward: Vec3::new(yaw.cos(), yaw.sin(), 0.0),
        right: Vec3::new(-yaw.sin(), yaw.cos(), 0.0),
        up: Vec3::Z,
        fov_y,
        aspect,
        component_to_world: Trs { translation: Vec3::ZERO, rotation: Quat::from_rotation_z(data.mesh_yaw_deg.to_radians()), scale: Vec3::ONE },
        local,
        component,
        clock,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_idle_gives_way_to_the_break_after_two_to_five_loops() {
        let mut cycle = IdleCycle::default();
        let (mut loops, mut breaks, mut was) = (Vec::new(), 0, false);
        let mut ends = 0;
        for _ in 0..(600.0 * 60.0) as u32 {
            let before = cycle.time;
            let (_, in_break) = cycle.advance(1.0 / 60.0, 6.0, Some(9.23));
            if cycle.time < before && !was {
                ends += 1;
            }
            if in_break && !was {
                breaks += 1;
                loops.push(ends);
                ends = 0;
            }
            was = in_break;
        }
        assert!(breaks >= 8, "{breaks}");
        assert!(loops.iter().all(|n| (IDLE_RANDOM.0..=IDLE_RANDOM.1).contains(n)), "{loops:?}");
        // without a break clip the idle loops for ever
        let mut cycle = IdleCycle::default();
        assert!((0..10_000).all(|_| !cycle.advance(1.0 / 60.0, 6.0, None).1));
    }

    /// (IMPL_ER23B, the user: "when closing the menu Sora stays on screen for too long") THE PANEL'S LIFETIME, with the
    /// real movie: no panel before the menu opens; while it opens the panel's opacity rises to 1; while it closes the
    /// opacity FOLLOWS the clip down frame by frame; once the close animation ended the view is no longer open (the
    /// hosts hand `None` to the renderer from that frame on: kh-er/src/portrait.rs, kh-er-render `picture_opacity`).
    #[test]
    fn the_panel_appears_and_disappears_with_the_camp_menu() {
        use kh_ui::camp::{CampMenu, StatsIn};
        use kh_ui::camp_menu::CampMenuView;
        let Ok(mut pack) = kh_ui::Pack::open_default() else {
            println!("SKIPPED: no build/sora_pack/ui");
            return;
        };
        let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
        let mut view = CampMenuView::load(&mut pack, fonts).unwrap();
        // the host's rule: the movie is only looked at while the view is open
        let alpha = |view: &CampMenuView| view.is_open().then(|| panel(&view.movie, (512, 800), 1.0, 0.0).map(|p| p.alpha)).flatten();
        assert_eq!(alpha(&view), None, "closed: no panel");
        let stats = StatsIn { name: "Sora".into(), level: 99, exp: 1_527_640, next_level_exp: 0, level_exp: 1_527_640, hp: 100, hp_max: 265, mp: 169, mp_max: 169, focus: 100, focus_max: 100, focus_unlocked: true, ap_max: 180, strength: 66, magic: 67, defense: 59, resists: [0; 6], munny: 1234, play_seconds: 3725 };
        let mut menu = CampMenu::open(false, false, stats, &mut Vec::new());
        view.open(&menu);
        // OPENING: the opacity rises (never falls) and reaches 1
        let mut opening = Vec::new();
        for _ in 0..60 {
            view.sync(&mut menu);
            view.tick();
            opening.extend(alpha(&view));
        }
        assert!(opening.len() > 20 && opening.windows(2).all(|w| w[1] >= w[0] - 1.0e-4), "{opening:?}");
        assert!(opening[0] < 0.5 && (opening.last().unwrap() - 1.0).abs() < 1.0e-3, "{opening:?}");
        // CLOSING: the opacity follows the clip down, every frame lower than the one before
        view.close();
        let mut closing = Vec::new();
        let mut ended_after = None;
        for frame in 0..120 {
            let ended = view.tick();
            match alpha(&view) {
                Some(a) => closing.push(a),
                None => {
                    ended_after.get_or_insert(frame);
                }
            }
            if ended {
                break;
            }
        }
        assert!(closing.len() >= 5 && closing.windows(2).all(|w| w[1] < w[0]), "{closing:?}");
        assert!(closing[0] <= 1.0 && *closing.last().unwrap() < 0.5, "{closing:?}");
        // CLOSED: the view is not open any more: no panel, no picture (within a third of a second of the close)
        assert!(!view.is_open() && alpha(&view).is_none());
        assert!(ended_after.is_some_and(|f| f <= 20), "the close animation took {ended_after:?} frames");
    }

    /// (IMPL_ER34, the user: "Sora is off centre in the pause menu") WHERE HE STANDS, with the real movie: in each of
    /// the three panels the capture's centre line is the panel's (its mask's) centre line, the picture is as high as
    /// the clip's 800 pixel placeholder, and that holds in output pixels at 16:9 (the user's 2560 x 1440, 1920 x 1080,
    /// 1280 x 720), 16:10 and 21:9: outside 16:9 the stage is the centred 16:9 rectangle and he moves with it.
    #[test]
    fn he_stands_in_the_middle_of_his_panel_at_every_resolution() {
        use kh_ui::camp::{CampMenu, StatsIn};
        use kh_ui::camp_menu::CampMenuView;
        let Ok(mut pack) = kh_ui::Pack::open_default() else {
            println!("SKIPPED: no build/sora_pack/ui");
            return;
        };
        let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
        let mut view = CampMenuView::load(&mut pack, fonts).unwrap();
        let stats = StatsIn { name: "Sora".into(), level: 5, hp: 92, hp_max: 120, mp: 100, mp_max: 100, ..StatsIn::default() };
        let mut menu = CampMenu::open(false, false, stats, &mut Vec::new());
        view.open(&menu);
        for _ in 0..60 {
            view.sync(&mut menu);
            view.tick();
        }
        // the clip is an Icon whose constructor removed the authoring placeholder: its kept size is the rectangle
        let chara = view.movie.sprite("topMenu.statusAnm.status.charaWindow01.charaAnm.chara").expect("the chara clip");
        assert!(chara.children.is_empty(), "the placeholder's shapes are gone (the Icon constructor)");
        assert_eq!(chara.icon.map(|i| i.size), Some((278.0, 800.0)));
        let top = panel(&view.movie, (512, 800), 1.0, 0.0).expect("the top screen's panel");
        let centre = |r: &Rect| (r.x0 + r.x1) * 0.5;
        assert!((centre(&top.picture) - centre(&top.mask)).abs() < 0.51, "picture {:?} mask {:?}", top.picture, top.mask);
        assert!((centre(&top.mask) - 1301.0).abs() < 0.01, "the one-member panel's centre line on the stage: {:?}", top.mask);
        assert!((top.picture.x1 - top.picture.x0 - 512.0).abs() < 0.05 && (top.picture.y1 - top.picture.y0 - 800.0).abs() < 0.05, "{:?}", top.picture);
        assert!((top.picture.y0 - 88.0).abs() < 0.01, "{:?}", top.picture);
        // (what the user's build did: the left edge on the clip's origin = 117 stage pixels to the right)
        let wrong = picture_rect(Rect::EMPTY, &Matrix::new(1.0, 0.0, 0.0, 1.0, 1162.0, 88.0), (512, 800), 1.0, 0.0);
        assert!((centre(&wrong) - centre(&top.picture) - 117.0).abs() < 0.01);
        for (w, h) in [(2560u32, 1440u32), (1920, 1080), (1280, 720), (1920, 1200), (2560, 1600), (2560, 1080), (3440, 1440)] {
            let (root, stage) = crate::hud_layers::stage_root(w as f32, h as f32);
            // the stage: the centred 16:9 rectangle
            assert!((stage.2 / stage.3 - 16.0 / 9.0).abs() < 0.01 && (stage.0 * 2.0 + stage.2 - w as f32).abs() < 1.0 && (stage.1 * 2.0 + stage.3 - h as f32).abs() < 1.0, "{w}x{h}: {stage:?}");
            let at = placement_of(&top, &root, (w, h)).unwrap_or_else(|| panic!("{w}x{h}: no placement"));
            let scale = stage.2 / 1920.0;
            let mask_centre = stage.0 + 1301.0 * scale;
            let (capture_centre, shown_centre) = (at.picture.0 + at.picture.2 * 0.5, at.visible.0 as f32 + at.visible.2 as f32 * 0.5);
            assert!((capture_centre - mask_centre).abs() < 1.0, "{w}x{h}: the capture's centre {capture_centre} vs the panel's {mask_centre}");
            assert!((shown_centre - mask_centre).abs() < 1.5, "{w}x{h}: the visible part's centre {shown_centre} vs the panel's {mask_centre}");
            assert!((at.picture.3 - 800.0 * scale).abs() < 0.1 && (at.picture.1 - (stage.1 + 88.0 * scale)).abs() < 0.1, "{w}x{h}: {at:?}");
            assert_eq!(at.alpha, 1.0);
        }
        // the user's resolution, in numbers: the capture 683 px wide around x = 1734.7, the visible part 363 px
        let (root, _) = crate::hud_layers::stage_root(2560.0, 1440.0);
        let at = placement_of(&top, &root, (2560, 1440)).unwrap();
        assert_eq!(at.visible, (1553, 117, 363, 860), "{at:?}");
        assert!((at.picture.0 - 1393.3).abs() < 0.1 && (at.picture.2 - 682.7).abs() < 0.1, "{at:?}");
        // the character picture of the Abilities and of the Stats screen: the same rule
        for (clip, path) in [("ability", PANELS[1]), ("status", PANELS[2])] {
            view.movie.goto_and_stop(clip, "loop");
            view.movie.goto_and_stop(&format!("{path}.charaAnm"), "loop");
            for _ in 0..12 {
                view.movie.tick();
            }
            let p = panel_at(&view.movie, path, (512, 800), 1.0, 0.0).unwrap_or_else(|| panic!("{path}: no panel"));
            assert!((centre(&p.picture) - centre(&p.mask)).abs() < 0.51, "{path}: picture {:?} mask {:?}", p.picture, p.mask);
        }
    }

    /// (IMPL_ER34) The placement follows the menu picture ON SCREEN: with a raster worker that needs two frames a
    /// picture, the portrait is drawn where the picture last handed over shows the panel, never ahead of it.
    #[test]
    fn the_portrait_is_placed_by_the_menu_picture_on_screen() {
        let at = |x: i32| Some(Placement { visible: (x, 0, 10, 10), picture: (x as f32, 0.0, 10.0, 10.0), alpha: 1.0 });
        let x_of = |p: Option<Placement>| p.map(|p| p.visible.0);
        let mut sync = PictureSync::default();
        // no worker (drawn in the same call): always this frame's
        assert_eq!(x_of(sync.step(at(1), 0, 0, false)), Some(1));
        assert_eq!(x_of(sync.step(None, 0, 0, false)), None);
        // a worker: frame A sends state 10 (busy); frame B: still busy, the movie moved on to 11; frame C: the picture
        // of state 10 is back and state 12 goes out; frame D: busy; frame E: 12 is back, nothing new (the menu rests)
        assert_eq!(x_of(sync.step(at(10), 1, 0, true)), None, "the panel's first picture is not on screen yet");
        assert_eq!(x_of(sync.step(at(11), 1, 0, true)), None);
        assert_eq!(x_of(sync.step(at(12), 2, 1, true)), Some(10), "the picture of state 10 is on screen");
        assert_eq!(x_of(sync.step(at(13), 2, 1, true)), Some(10));
        assert_eq!(x_of(sync.step(at(13), 2, 2, false)), Some(13), "at rest: this frame's state is the picture");
        // the panel goes (a sub screen opens): he stays exactly as long as the picture that still shows the panel
        assert_eq!(x_of(sync.step(None, 3, 2, true)), Some(13));
        assert_eq!(x_of(sync.step(None, 3, 3, false)), None);
        // the menu closes
        sync.step(at(20), 4, 3, true);
        sync.clear();
        assert_eq!(x_of(sync.step(None, 4, 4, false)), None);
    }

    #[test]
    fn the_capture_is_the_ui_actors() {
        let pack = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack"));
        let Ok(data) = PortraitData::load(pack) else {
            println!("SKIPPED: no PACK/menu");
            return;
        };
        assert_eq!((data.target, data.fov_deg, data.capture, data.capture_yaw_deg, data.mesh_yaw_deg), ((512, 800), 30.0, Vec3::new(250.0, 2.0, 130.0), 180.0, -90.0));
        assert_eq!((data.clip.0.as_str(), data.clip.2), ("0001CM", 6.0));
        // FOV 30 over the 800 pixel height
        let (fov_y, aspect) = data.projection();
        assert!((fov_y.to_degrees() - 30.0).abs() < 1.0e-3 && (aspect - 0.64).abs() < 1.0e-6);
        // the key spot stands in front of him to his right and points back at his head
        let key = data.rig.spots.iter().find(|s| s.intensity == 5000.0).expect("the key light");
        let to_head = (Vec3::new(0.0, 0.0, 150.0) - key.position).normalize();
        assert!(key.position.x > 100.0 && key.position.y > 50.0 && to_head.dot(key.direction) > (20.0f32).to_radians().cos());
    }
}

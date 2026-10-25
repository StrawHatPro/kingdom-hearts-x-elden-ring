//! (game over) THE GAME-OVER SEQUENCE of Kingdom Hearts III, engine-free: what happens from Sora's death to
//! the menu's answer, as times and plain data a host draws (docs/sora-gameover-spec.md; hand-over
//! build/sora_tmp/er/IMPL_GAMEOVER_UI.md).
//!
//! THE GAME (the game mode's game-over state `0x140EAD210`, decompiled C; times in real seconds):
//! - at the death (the Die state is pushed and `StartGameOver` `0x140EBA4D0` is called in the same call): the
//!   loop SE (static SE 21; mission failed: 34), the world's time dilation goes to 0.01 within 0.5 s, the HUD's
//!   fade plate goes to full over 1.0 s (normal death: colour `0x14871D850` = WHITE; mission failed: black);
//! - when the plate is full: the world is paused and HIDDEN (every level actor), the pawn goes to its
//!   GAMEOVER state (clip `0270` in place, the light or the heart effect, the camera anim `game_over_01`
//!   blended in over 2.0 s), then 1.0 s passes;
//! - then the menu opens and the plate goes away over 0.5 s;
//! - at the choice the plate comes back over 0.5 s (black) and the choice is carried out.
//!
//! USER DECISION 2026-10-12 ("fade to black when Sora dies into this"): [`GameOverFade::black`] is the
//! default; [`GameOverFade::game`] is the game's own plate (white for a normal death).
//!
//! A host WITHOUT the test app's renderer draws the 3D part itself from [`GameOverView`]: the pose source
//! ([`GameOverScene`]: clip, time, face row), the camera and light constants ([`CAMERA`], [`LIGHTS`]), the
//! screen rectangle (the whole 16:9 stage: [`crate::hud_layers::stage_root`]) and the plate's opacity.

pub use kh_ui::gameover::{GameOverButton, GameOverResult, GameOverVariant};

/// `ETresGameOverKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GameOverKind {
    #[default]
    Normal,
    MissionFailed,
}

/// The fade plate of the sequence (the HUD's fade: it covers the scene AND the cockpit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameOverFade {
    /// Colour of the plate at the death (sRGB 0..1).
    pub colour: [f32; 3],
    /// Seconds from the death to the full plate (`0x140EAD210` step 0: 1.0; the Gummi Ship's kind: 2.0).
    pub out_seconds: f32,
    /// Seconds the plate stays full while the void scene is set up (step 4's timer: 1.0), after the
    /// [`SETUP_FRAMES`] the steps between take.
    pub hold_seconds: f32,
    /// Seconds the plate takes to leave when the menu opens (step 5: 0.5).
    pub in_seconds: f32,
    /// Seconds of the plate's return after the choice (step 6: 0.5), and its colour (black).
    pub choice_seconds: f32,
    pub choice_colour: [f32; 3],
    /// Seconds the plate takes to leave after the host carried the choice out (the game's fade in after a
    /// map change, `0x140EA0F90`: 0.5).
    pub after_seconds: f32,
}

impl GameOverFade {
    /// The game's plate for a kind: white for a normal death, black for a failed mission.
    pub fn game(kind: GameOverKind) -> GameOverFade {
        GameOverFade { colour: if kind == GameOverKind::Normal { [1.0; 3] } else { [0.0; 3] }, ..GameOverFade::black() }
    }

    /// The game's times with a BLACK plate (the user's decision; the game's failed-mission plate).
    pub fn black() -> GameOverFade {
        GameOverFade { colour: [0.0; 3], out_seconds: 1.0, hold_seconds: 1.0, in_seconds: 0.5, choice_seconds: 0.5, choice_colour: [0.0; 3], after_seconds: 0.5 }
    }
}

impl Default for GameOverFade {
    fn default() -> GameOverFade {
        GameOverFade::black()
    }
}

/// What a host asks for with [`crate::hud::Hud::open_gameover`], at the moment Sora dies.
#[derive(Clone, Debug, PartialEq)]
pub struct GameOverSetup {
    /// Which buttons the menu shows.
    pub variant: GameOverVariant,
    /// The hint text of the panel (`<br>` = a line break; `None`: the panel is there without a text, as in
    /// the game when no hint row applies).
    pub hint: Option<String>,
    pub kind: GameOverKind,
    /// The heart effect instead of the light effect (the game: the killing blow's attacker flag, see the spec).
    pub heart: bool,
    pub fade: GameOverFade,
    /// Buttons the host cannot carry out (shown disabled; decide on one is refused).
    pub disabled: Vec<GameOverButton>,
    /// The loop SE of the sequence (`bPlayDeadSE`).
    pub dead_se: bool,
}

impl Default for GameOverSetup {
    fn default() -> GameOverSetup {
        GameOverSetup { variant: GameOverVariant::Field, hint: None, kind: GameOverKind::Normal, heart: false, fade: GameOverFade::default(), disabled: Vec::new(), dead_se: true }
    }
}

/// Stage frames (1/60 s) the game mode's steps 1 - 3 take between the full plate and the pawn's GAMEOVER state.
pub const SETUP_FRAMES: u32 = 3;
/// `0x140EAD210` step 0: the time dilation the world goes to at the death, and the seconds it takes.
pub const DEATH_TIME_DILATION: (f32, f32) = (0.01, 0.5);
/// Static SE of the loop (`ETresStaticSE` 21 = `se00000_046`; a failed mission: 34 = `se00000_068`).
pub const SE_GAMEOVER_LOOP: u32 = 21;
pub const SE_MISSION_FAILED_LOOP: u32 = 34;

/// The camera of the scene: a `CameraAnim` of one key, played in the pawn's space (the pawn's rotation is
/// set to zero by the state, so its axes are the world's: X forward, Y right, Z up, cm; the origin is the
/// pawn's ROOT = the capsule's centre, [`ROOT_ABOVE_FEET`] above his feet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameOverCamera {
    pub position: [f32; 3],
    pub pitch_deg: f32,
    pub yaw_deg: f32,
    pub roll_deg: f32,
    /// The HORIZONTAL field of view over the 16:9 stage.
    pub fov_deg: f32,
    /// Seconds of the blend in from the camera of the death (`0x1405D9D40`: 2.0, blend function 3 =
    /// stock `VTBlend_EaseOut`).
    pub blend_seconds: f32,
}

/// `/Game/Camera/sora/hanyo/game_over_01` (read from the asset).
pub const CAMERA: GameOverCamera = GameOverCamera { position: [393.3685, 6.905567, 81.76496], pitch_deg: -7.632026, yaw_deg: -178.9123, roll_deg: 0.0, fov_deg: 60.0, blend_seconds: 2.0 };
/// `/Game/Camera/sora/hanyo/game_over_mission_fail` (read from the asset).
pub const CAMERA_MISSION_FAILED: GameOverCamera = GameOverCamera { position: [518.5602, 120.9843, 22.01692], pitch_deg: -0.3634949, yaw_deg: -167.0936, roll_deg: 0.0, fov_deg: 50.0, blend_seconds: 2.0 };

/// The origin of the camera anim and of the effect above his FEET (the mesh component's origin), cm. FITTED TO
/// THE REFERENCE CAPTURE: with the capsule centre (91 above the feet, where the decompiled C says the effect is
/// attached: "the pawn's root component") he lies 97 cm too low in the picture and the lights are far above him;
/// with the feet he is where the capture has him. His mesh component sits at the feet with yaw -90 against the pawn.
pub const ROOT_ABOVE_FEET: f32 = 0.0;
pub const MESH_YAW_DEG: f32 = -90.0;

/// A particle light of the scene's effect, in the pawn's root space (cm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameOverLight {
    pub name: &'static str,
    pub position: [f32; 3],
    /// `ColorScaleOverLife` x `BrightnessOverLife` (linear).
    pub colour: [f32; 3],
    /// `RadiusScale` x the mean of the particle's X and Y size.
    pub radius: f32,
    /// `LightExponent` (the engine's exponent falloff, not inverse squared).
    pub exponent: f32,
}

/// `vs_so_dth_ui00` (`m_PlayerGameOverLightEffect`): three light particles and nothing drawn. THE LIGHTING
/// OF THE VOID: the level's lights are hidden with the level. Read from the effect's emitters (emitter
/// origin + node position; size 0.35 x SizeScale 1000 = 350).
pub const LIGHTS: [GameOverLight; 3] = [
    GameOverLight { name: "light", position: [0.0, 0.0, 120.0], colour: [3.0, 4.5, 5.0], radius: 350.0, exponent: 2.0 },
    GameOverLight { name: "light_face", position: [0.0, 35.0, 100.0], colour: [10.0, 10.0, 10.0], radius: 105.0, exponent: 2.0 },
    GameOverLight { name: "OnePart_lp", position: [100.0, 180.0, -30.0], colour: [0.8, 1.6, 2.0], radius: 700.0, exponent: 4.0 },
];
/// `vs_so_dth_ui00_2` (`m_PlayerGameOverHeartEffect`): the heart's red light and a face light (besides its
/// drawn emitters: the heart meshes `heart00` / `heart0` at (0, 0, 120) and the rising sparks `Range_lp`).
pub const LIGHTS_HEART: [GameOverLight; 2] = [
    GameOverLight { name: "heart00", position: [0.0, 0.0, 120.0], colour: [72.0, 9.0, 15.0], radius: 285.0, exponent: 2.0 },
    GameOverLight { name: "light_face", position: [0.0, 35.0, 100.0], colour: [2.0, 2.0, 2.0], radius: 105.0, exponent: 2.0 },
];
/// `vs_so_misn_ui00` (`m_PlayerMissionFailedLightEffect`).
pub const LIGHTS_MISSION_FAILED: [GameOverLight; 2] = [
    GameOverLight { name: "light", position: [80.0, 40.0, 120.0], colour: [20.7, 21.85, 23.0], radius: 350.0, exponent: 2.0 },
    GameOverLight { name: "light_face", position: [0.0, 30.0, 90.0], colour: [5.0, 5.0, 5.0], radius: 105.0, exponent: 2.0 },
];

/// The character materials' post-process collections in the void: the plain `KHS_Blueprint` the game mode
/// spawns for it (`TresGameModeLoad.m_GameOverKHS`): the preset `blueprint` of PACK/materials.
pub const KHS_PRESET: &str = "blueprint";

/// THE POSE SOURCE of the void scene, per frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameOverScene {
    /// The body sequence (`M_0270` = sequence `0270`, 5 s; a failed mission: `0271`, 3 s), looped in place at
    /// rate 1, and the seconds into it. Its own notifies ride with it: the face row, the Bonamik winds /
    /// speed rate / weights (kh-anim plays them from the pack when the sequence is named as the notify source).
    pub clip: &'static str,
    pub clip_seconds: f32,
    pub clip_time: f32,
    /// The face row the sequence asks for at its start, looped (`TresAnimNotify_FaceAnim`): type `NO_1`
    /// (row `NO_1_EL` = face/0001EM: eyes closed); the failed mission's is the row `BA_15_EL`.
    pub face: &'static str,
    /// Seconds since the scene began (the pawn's GAMEOVER state).
    pub seconds: f32,
    /// 0..1: the camera's blend from the camera of the death to [`GameOverScene::camera`], eased.
    pub camera_blend: f32,
    pub camera: GameOverCamera,
    /// The effect attached to the pawn's root at (0, 0, 0): its asset name and its lights.
    pub effect: &'static str,
    pub lights: &'static [GameOverLight],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameOverPhase {
    /// The plate rises over the world and the cockpit; the world still runs (in slow motion in the game).
    FadeOut,
    /// The plate is full; the void scene is being set up behind it.
    Hold,
    /// The menu is up (the plate leaves in its first half second).
    Menu,
    /// A button was decided: the plate returns.
    ChoiceFade,
    /// The choice waits for the host ([`crate::hud::Hud::take_gameover_result`]) / is being carried out; the
    /// plate is full. The host ends the sequence with [`crate::hud::Hud::end_gameover`].
    Chosen,
    /// The host has carried the choice out: the plate leaves over the world.
    After,
}

/// What a host draws this frame (everything in real time).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameOverView {
    pub phase: GameOverPhase,
    /// Seconds since the death.
    pub seconds: f32,
    /// The plate: 0 = none, 1 = full, and its colour (sRGB 0..1). It lies over the world, the cockpit AND the
    /// void scene, under the menu.
    pub fade: f32,
    pub fade_colour: [f32; 3],
    /// The world's time dilation the game applies in [`GameOverPhase::FadeOut`] (1 -> 0.01 within 0.5 s).
    pub time_dilation: f32,
    /// `Some`: the void scene is what lies under the plate (black background, Sora posed, no world, no cockpit).
    pub scene: Option<GameOverScene>,
    /// The menu is on screen.
    pub menu: bool,
}

/// The sequence's clock and state (owned by the Hud).
#[derive(Clone, Debug)]
pub struct Sequence {
    pub setup: GameOverSetup,
    phase: GameOverPhase,
    /// Stage frames since the death / since the scene began / since the current fade began.
    frames: u32,
    scene_frames: Option<u32>,
    fade_frames: u32,
    result: Option<GameOverResult>,
    /// The cursor's button when the menu was left for the Load screen (it comes back there).
    menu_opened: bool,
}

fn frames_of(seconds: f32) -> u32 {
    (seconds * 60.0).round().max(0.0) as u32
}

/// What the sequence asks of the Hud in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    None,
    /// Open the menu now.
    OpenMenu,
}

impl Sequence {
    pub fn new(setup: GameOverSetup) -> Sequence {
        Sequence { setup, phase: GameOverPhase::FadeOut, frames: 0, scene_frames: None, fade_frames: 0, result: None, menu_opened: false }
    }

    pub fn phase(&self) -> GameOverPhase {
        self.phase
    }

    /// The void scene is what the screen shows (the world is held from here on).
    pub fn scene_on(&self) -> bool {
        self.scene_frames.is_some() && self.phase != GameOverPhase::After
    }

    /// One stage frame.
    pub fn tick(&mut self) -> Step {
        let fade = self.setup.fade;
        self.frames += 1;
        self.fade_frames += 1;
        if let Some(n) = self.scene_frames.as_mut() {
            *n += 1;
        }
        match self.phase {
            GameOverPhase::FadeOut => {
                if self.frames >= frames_of(fade.out_seconds) {
                    self.phase = GameOverPhase::Hold;
                }
            }
            GameOverPhase::Hold => {
                let scene_at = frames_of(fade.out_seconds) + SETUP_FRAMES;
                if self.scene_frames.is_none() && self.frames >= scene_at {
                    self.scene_frames = Some(0);
                }
                if self.frames >= scene_at + frames_of(fade.hold_seconds) {
                    self.phase = GameOverPhase::Menu;
                    self.fade_frames = 0;
                    self.menu_opened = true;
                    return Step::OpenMenu;
                }
            }
            GameOverPhase::Menu | GameOverPhase::Chosen => {}
            GameOverPhase::ChoiceFade => {
                if self.fade_frames >= frames_of(fade.choice_seconds) {
                    self.phase = GameOverPhase::Chosen;
                }
            }
            GameOverPhase::After => {}
        }
        Step::None
    }

    /// The menu's button was decided. Load is handed over at once (the Load screen opens over the scene);
    /// the others after the plate's return.
    pub fn choose(&mut self, result: GameOverResult) {
        self.result = Some(result);
        if result == GameOverResult::Load || result == GameOverResult::GameHelp {
            self.phase = GameOverPhase::Chosen;
            // (the plate is not raised: `fade_frames` past the choice fade means "none" in `view`)
            self.fade_frames = u32::MAX / 2;
        } else {
            self.phase = GameOverPhase::ChoiceFade;
            self.fade_frames = 0;
        }
    }

    /// The choice, once, when its plate is full (Load: at once).
    pub fn take_result(&mut self) -> Option<GameOverResult> {
        if self.phase == GameOverPhase::Chosen { self.result.take() } else { None }
    }

    /// The choice that was handed over or waits (for the plate's colour and the host's log).
    pub fn chosen_load(&self) -> bool {
        self.phase == GameOverPhase::Chosen && self.fade_frames >= u32::MAX / 4
    }

    /// The Load screen was cancelled: the menu is to open again (true: reopen it now).
    pub fn reopen(&mut self) -> bool {
        if self.chosen_load() {
            self.phase = GameOverPhase::Menu;
            self.fade_frames = frames_of(self.setup.fade.in_seconds);
            self.result = None;
            return true;
        }
        false
    }

    /// The host carried the choice out: the scene goes, the plate leaves over the world (`fade_in` false: at once).
    pub fn end(&mut self, fade_in: bool) {
        self.phase = GameOverPhase::After;
        self.fade_frames = if fade_in { 0 } else { u32::MAX / 2 };
    }

    /// The sequence is over (drop it).
    pub fn finished(&self) -> bool {
        self.phase == GameOverPhase::After && self.fade_frames >= frames_of(self.setup.fade.after_seconds)
    }

    pub fn view(&self, menu_open: bool) -> GameOverView {
        let fade = self.setup.fade;
        let ratio = |frames: u32, seconds: f32| if seconds <= 0.0 { 1.0 } else { (frames as f32 / (seconds * 60.0)).clamp(0.0, 1.0) };
        let seconds = self.frames as f32 / 60.0;
        let (plate, colour) = match self.phase {
            GameOverPhase::FadeOut => (ratio(self.frames, fade.out_seconds), fade.colour),
            GameOverPhase::Hold => (1.0, fade.colour),
            GameOverPhase::Menu => (1.0 - ratio(self.fade_frames, fade.in_seconds), fade.colour),
            GameOverPhase::ChoiceFade => (ratio(self.fade_frames, fade.choice_seconds), fade.choice_colour),
            GameOverPhase::Chosen => (if self.chosen_load() { 0.0 } else { 1.0 }, fade.choice_colour),
            GameOverPhase::After => (1.0 - ratio(self.fade_frames, fade.after_seconds), fade.choice_colour),
        };
        let time_dilation = if self.phase == GameOverPhase::FadeOut { 1.0 + (DEATH_TIME_DILATION.0 - 1.0) * ratio(self.frames, DEATH_TIME_DILATION.1) } else { 1.0 };
        let scene = self.scene_frames.filter(|_| self.phase != GameOverPhase::After).map(|frames| {
            let s = frames as f32 / 60.0;
            let mission = self.setup.kind == GameOverKind::MissionFailed;
            let (clip, clip_seconds, face) = if mission { ("0271", 3.0, "BA_15_EL") } else { ("0270", 5.0, "NO_1") };
            let camera = if mission { CAMERA_MISSION_FAILED } else { CAMERA };
            // Stock `VTBlend_EaseOut` with the default exponent 2 (the exponent of the call was not read).
            let t = (s / camera.blend_seconds).clamp(0.0, 1.0);
            let (effect, lights): (&'static str, &'static [GameOverLight]) = if mission { ("vs_so_misn_ui00", &LIGHTS_MISSION_FAILED) } else if self.setup.heart { ("vs_so_dth_ui00_2", &LIGHTS_HEART) } else { ("vs_so_dth_ui00", &LIGHTS) };
            GameOverScene { clip, clip_seconds, clip_time: s % clip_seconds, face, seconds: s, camera_blend: 1.0 - (1.0 - t) * (1.0 - t), camera, effect, lights }
        });
        GameOverView { phase: self.phase, seconds, fade: plate, fade_colour: colour, time_dilation, scene, menu: menu_open }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(seq: &mut Sequence, frames: u32) -> Vec<(u32, Step)> {
        (0..frames).filter_map(|f| match seq.tick() { Step::None => None, step => Some((f + 1, step)) }).collect()
    }

    #[test]
    fn the_sequence_keeps_the_games_times() {
        let mut seq = Sequence::new(GameOverSetup { variant: GameOverVariant::Battle, ..GameOverSetup::default() });
        // The plate rises for a second over the running world; half way it is half there.
        assert_eq!(run(&mut seq, 30), vec![]);
        let v = seq.view(false);
        assert_eq!((v.phase, v.fade, v.scene, v.fade_colour), (GameOverPhase::FadeOut, 0.5, None, [0.0; 3]));
        // (the game's slow motion: 0.01 from half a second on)
        assert!((v.time_dilation - 0.01).abs() < 1.0e-6 && !seq.scene_on());
        // Full at 1.0 s; three frames later the void scene is there behind it; the menu opens a second after that.
        assert_eq!(run(&mut seq, 30 + 3), vec![]);
        let v = seq.view(false);
        assert_eq!((v.phase, v.fade), (GameOverPhase::Hold, 1.0));
        let scene = v.scene.expect("the void scene");
        assert_eq!((scene.clip, scene.clip_time, scene.face, scene.effect, scene.lights.len(), scene.camera), ("0270", 0.0, "NO_1", "vs_so_dth_ui00", 3, CAMERA));
        assert!(seq.scene_on());
        assert_eq!(run(&mut seq, 60), vec![(60, Step::OpenMenu)]);
        // The plate leaves in half a second; the camera's blend (2 s, ease out) is at 1 - 0.5^2 when the menu opens.
        let v = seq.view(true);
        assert_eq!((v.phase, v.fade, v.menu), (GameOverPhase::Menu, 1.0, true));
        assert!((v.scene.unwrap().camera_blend - 0.75).abs() < 1.0e-6);
        run(&mut seq, 15);
        assert_eq!(seq.view(true).fade, 0.5);
        run(&mut seq, 15 + 300);
        let v = seq.view(true);
        assert_eq!((v.fade, v.scene.unwrap().camera_blend), (0.0, 1.0));
        // The clip loops in place: 1 + 0.5 + 5 s after the scene began it is 1.5 s into its second round.
        assert!((v.scene.unwrap().clip_time - 1.5).abs() < 1.0e-4);
        // A choice: the plate returns in half a second (black), then the host has it, once.
        seq.choose(GameOverResult::Retry);
        assert_eq!(seq.take_result(), None);
        run(&mut seq, 30);
        assert_eq!((seq.view(false).phase, seq.view(false).fade), (GameOverPhase::Chosen, 1.0));
        assert_eq!((seq.take_result(), seq.take_result()), (Some(GameOverResult::Retry), None));
        // The host is done: the scene is gone at once, the plate leaves over the world.
        seq.end(true);
        assert_eq!((seq.view(false).scene, seq.view(false).fade, seq.finished(), seq.scene_on()), (None, 1.0, false, false));
        run(&mut seq, 30);
        assert!(seq.finished());
    }

    #[test]
    fn load_is_handed_over_at_once_and_a_cancel_brings_the_menu_back() {
        let mut seq = Sequence::new(GameOverSetup::default());
        run(&mut seq, 60 + 3 + 60 + 30);
        seq.choose(GameOverResult::Load);
        assert_eq!(seq.take_result(), Some(GameOverResult::Load));
        // No plate: the Load screen lies over the void scene.
        let v = seq.view(false);
        assert_eq!((v.phase, v.fade, v.scene.is_some()), (GameOverPhase::Chosen, 0.0, true));
        assert!(seq.reopen());
        assert_eq!((seq.phase(), seq.view(true).fade), (GameOverPhase::Menu, 0.0));
        // A choice that fades cannot be reopened.
        seq.choose(GameOverResult::Continue);
        assert!(!seq.reopen());
    }

    #[test]
    fn the_games_own_plate_is_white_and_a_failed_mission_has_its_own_scene() {
        assert_eq!(GameOverFade::game(GameOverKind::Normal).colour, [1.0; 3]);
        assert_eq!(GameOverFade::game(GameOverKind::MissionFailed).colour, [0.0; 3]);
        let mut seq = Sequence::new(GameOverSetup { kind: GameOverKind::MissionFailed, ..GameOverSetup::default() });
        run(&mut seq, 64);
        let scene = seq.view(false).scene.unwrap();
        assert_eq!((scene.clip, scene.clip_seconds, scene.face, scene.effect, scene.camera.fov_deg), ("0271", 3.0, "BA_15_EL", "vs_so_misn_ui00", 50.0));
        let mut seq = Sequence::new(GameOverSetup { heart: true, ..GameOverSetup::default() });
        run(&mut seq, 64);
        assert_eq!(seq.view(false).scene.unwrap().effect, "vs_so_dth_ui00_2");
    }
}

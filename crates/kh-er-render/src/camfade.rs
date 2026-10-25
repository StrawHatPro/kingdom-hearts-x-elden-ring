//! (NF) KH3'S CAMERA FADE IN THE ELDEN RING RENDERER (kh-camera camfade.rs; build/sora_tmp/party/IMPL_NF_STATE.md): once a
//! frame the fade list is stepped over every KH character drawn (Sora in any outfit or form with his Keyblade / form
//! weapons, every party member with his weapon, any character added to the roster) from his posed bones and the camera, and
//! each character's frame block carries HIS value: `cam_right.w` = 1 - his CameraFade value, `cam_up.w` = the screen
//! door's phase (er_char.wgsl `er_camfade_keep`). Every pass of his reads his frame block (the early depth, the colour, the
//! hybrid's G-buffer, the glow mask, the velocity, the hair passes), so a pixel the fade leaves out is absent from all of
//! them. A block nobody wrote (a portrait, the void, an old host) carries 0: the character drawn whole.
//!
//! The host tells the renderer where the camera looks and who leads ([`set_view`]); without it the look point is the
//! leader's mid-body and the leader is Sora.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use glam::Vec3;
use kh_camera::camfade::{self, CamFade, Subject, Volume};

use crate::Body;

/// Sora's key in the fade list; a party member's is `MEMBER + draw id`.
pub const SORA: u32 = 0;
pub const MEMBER: u32 = 100;

/// The key of a party member (his draw id, `party::DONALD` ..).
pub fn member_key(chara: u8) -> u32 {
    MEMBER + chara as u32
}

/// Where the camera looks and who the player controls, as the host's camera knows it (game space, cm).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct View {
    /// The camera's look point (KH3's: the point the collision pass sweeps from); `None` = the leader's mid-body.
    pub look: Option<Vec3>,
    /// The party member the player controls (his draw id); `None` = Sora.
    pub lead: Option<u8>,
}

static LOOK: Mutex<Option<(Option<Vec3>, Instant)>> = Mutex::new(None);
static LEAD: Mutex<Option<(Option<u8>, Instant)>> = Mutex::new(None);

/// The host camera's look point this frame (game space, cm; `None`: the leader's mid-body). Call once a frame; it lapses
/// half a second after the latest call.
pub fn set_look(look: Option<Vec3>) {
    if let Ok(mut v) = LOOK.lock() {
        *v = Some((look, Instant::now()));
    }
}

/// The party member the player controls (his draw id; `None`: Sora). Call once a frame; it lapses as `set_look`.
pub fn set_lead(lead: Option<u8>) {
    if let Ok(mut v) = LEAD.lock() {
        *v = Some((lead, Instant::now()));
    }
}

/// Both at once.
pub fn set_view(view: View) {
    set_look(view.look);
    set_lead(view.lead);
}

fn view_now() -> View {
    let fresh = |at: &Instant| at.elapsed() < Duration::from_millis(500);
    let look = LOOK.lock().ok().and_then(|v| *v).filter(|(_, at)| fresh(at)).and_then(|(l, _)| l);
    let lead = LEAD.lock().ok().and_then(|v| *v).filter(|(_, at)| fresh(at)).and_then(|(l, _)| l);
    View { look, lead }
}

/// (RSF2 round 4: "a fine dither pattern over Donald very close to the camera") `r.camfade_inside` (default 1): while the
/// camera is INSIDE a character (one of his KH3 camera volumes or the port's limb volumes), he goes all the way out instead
/// of stopping at KH3's 0.5. KH3's 0.5 is a screen door that ITS temporal AA averages into an even see-through; Elden
/// Ring's TAA keeps our still door as it is, so with the camera in him the door covered half the picture as a 1-pixel
/// checker (ERT4 / RSF4 frames: Donald's hat at the camera, the log "down to 0.50 (camera inside his camera volume)").
/// Out entirely there is no door left to see. It ramps with the fade (the same time), so it comes back as smoothly; the
/// look-line case (a friend between the camera and the leader) keeps KH3's 0.5. Every character the same way (Sora in any
/// form, every party member, the leader included: the camera inside the leader is the same case).
static INSIDE_OUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_inside_out(on: bool) {
    INSIDE_OUT.store(on, Ordering::Relaxed);
}

/// The weight of "out entirely" for a touch: 1 with the camera inside him (`r.camfade_inside`), else 0.
fn inside_target(touch: Option<camfade::Touch>) -> f32 {
    match touch {
        Some(camfade::Touch::Inside | camfade::Touch::Limb) if INSIDE_OUT.load(Ordering::Relaxed) => 1.0,
        _ => 0.0,
    }
}

/// His amount (1 - drawn share) from KH3's value and the inside weight: 1 - value x (1 - weight).
pub fn amount_with_inside(value: f32, weight: f32) -> f32 {
    (1.0 - value.clamp(0.0, 1.0) * (1.0 - weight.clamp(0.0, 1.0))).clamp(0.0, 1.0)
}

/// 0 = the sheet's rule (on), 1 = forced on, 2 = forced off (`r.camfade`, the harness's `--camfade`).
static FORCE: AtomicU32 = AtomicU32::new(0);

/// `r.camfade` (absent = the sheet's camfade.on): false draws every character whole.
pub fn set_enabled(on: Option<bool>) {
    FORCE.store(match on {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    }, Ordering::Relaxed);
}

fn enabled() -> bool {
    match FORCE.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => camfade::P.camfade_on,
    }
}

/// 0 = the game's TAA decides (the screen door steps while the game's jittered view-projection is in use), 1 = always
/// step, 2 = never (the harness: `--camfade-temporal`).
static TEMPORAL: AtomicU32 = AtomicU32::new(0);

pub fn set_temporal(on: Option<bool>) {
    TEMPORAL.store(match on {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    }, Ordering::Relaxed);
}

fn temporal(game_jitter: bool) -> bool {
    match TEMPORAL.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => game_jitter,
    }
}

/// The main pack's character id (its pawn blueprint `/Game/Blueprints/Player/<id>/<id>_Pawn`: p_ex001), read once.
fn main_chara() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let pack = crate::SHARED.lock().ok().and_then(|s| s.as_ref().map(|s| s.pack.clone()));
        let text = pack.and_then(|p| std::fs::read_to_string(p.join("manifest.json")).ok()).unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_default();
        json["identification"]["pawn_blueprint"].as_str().and_then(|b| b.rsplit('/').next()).map(|n| n.trim_end_matches("_Pawn").to_string()).unwrap_or_default()
    })
}

/// A bone of a posed body in game space, by name (UE names compare without case).
fn bone_of<'a>(bones: &'a HashMap<String, usize>, body: &'a Body) -> impl Fn(&str) -> Option<camfade::BoneAt> + 'a {
    move |name: &str| {
        let index = bones.get(name).copied().or_else(|| bones.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| *v))?;
        let t = body.component.get(index)?.then(&body.component_to_world);
        let scale = t.scale.abs().max_element();
        (t.translation.is_finite() && t.rotation.is_finite()).then_some((t.translation, t.rotation, if scale > 0.0 { scale } else { 1.0 }))
    }
}

/// The volumes of a posed character (`chara` = his id: the main pack's for Sora, the member's npc for a friend).
pub fn volumes_of(chara: &str, bones: &HashMap<String, usize>, body: &Body) -> Vec<Volume> {
    camfade::volumes(chara, &bone_of(bones, body), body.component_to_world.translation)
}

/// Sora's id for [`volumes_of`].
pub fn sora_chara() -> &'static str {
    main_chara()
}

/// One character to step: key, his volumes, his mid-body (the look point when the host gave none and he leads).
pub struct Drawn {
    pub key: u32,
    pub volumes: Vec<Volume>,
    pub middle: Vec3,
}

/// The middle of a body's volumes (the fallback look point).
pub fn middle_of(volumes: &[Volume], feet: Vec3) -> Vec3 {
    let pts: Vec<Vec3> = volumes.iter().filter(|v| v.kind == camfade::Kind::Camera).flat_map(|v| [v.shape.a, v.shape.b]).collect();
    if pts.is_empty() {
        feet + Vec3::Z * 90.0
    } else {
        pts.iter().copied().sum::<Vec3>() / pts.len() as f32
    }
}

struct State {
    fade: CamFade,
    clock: Option<f32>,
    phase: f32,
    /// for the log: frames with someone faded, the deepest value, by key
    faded: HashMap<u32, (u32, f32, Option<camfade::Touch>)>,
    frames: u32,
    /// (RSF2) per key: the weight of "out entirely" (`r.camfade_inside`), ramped like the fade
    inside: HashMap<u32, f32>,
}

/// (RSF round 18, the user: "make sure Donald and Goofy are rendered using the exact same way as Sora as they have things
/// showing through them unlike Sora") `r.camfade_friends` (default 0): every KH character takes the LEAD's rule, Sora's: only
/// the camera inside his body fades him, never the look line. KH3's look-line fade (FUN_1405D87F0's sweep, friends only) put a
/// friend standing between the camera and the look point at 0.5 behind the screen door, which the game's TAA turns into a
/// see-through body (the user's log: member 2 at 0.50 "between the camera and the look point" in 34 s of a fight). 1: KH3's
/// friend fade as before.
static FRIENDS_LOOK_FADE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_friends_look_fade(on: bool) {
    FRIENDS_LOOK_FADE.store(on, Ordering::Relaxed);
}

/// (RSF round 18) Does this drawn character take the lead's rule (no look-line fade)?
pub fn as_lead(key: u32, lead_key: u32) -> bool {
    key == lead_key || !FRIENDS_LOOK_FADE.load(Ordering::Relaxed)
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// THE FRAME'S STEP (gfx.rs, before the frame blocks are made): `clock` the game clock (s; the step's dt is its change,
/// at most 0.1 s), `camera` the camera's place (game space), `frame` the renderer's frame count, `game_jitter` = the game's
/// own (jittered, TAA) view-projection draws him. Returns Sora's block values (amount, phase).
pub fn step(clock: f32, camera: Vec3, frame: u64, game_jitter: bool, drawn: &[Drawn]) {
    let view = view_now();
    let Ok(mut guard) = STATE.lock() else { return };
    let s = guard.get_or_insert_with(|| State { fade: CamFade::default(), clock: None, phase: 0.0, faded: HashMap::new(), frames: 0, inside: HashMap::new() });
    let dt = s.clock.map_or(0.0, |before| (clock - before).clamp(0.0, 0.1));
    s.clock = Some(clock);
    // (the sheet's camfade.temporal decides; `set_temporal(Some(true))` forces the stepping door for a test)
    s.phase = if TEMPORAL.load(Ordering::Relaxed) == 1 { (frame % camfade::P.camfade_phase_cycle.max(1) as u64) as f32 } else { camfade::phase(frame, temporal(game_jitter)) as f32 };
    let lead_key = view.lead.map_or(SORA, member_key);
    let look = view.look.or_else(|| drawn.iter().find(|d| d.key == lead_key).map(|d| d.middle));
    let subjects: Vec<Subject> = if enabled() { drawn.iter().map(|d| Subject { key: d.key, lead: as_lead(d.key, lead_key) /* (RSF round 18) */, volumes: &d.volumes }).collect() } else { Vec::new() };
    s.fade.step(dt, camera, look, &subjects);
    s.frames += 1;
    // (RSF2) the inside weight runs 0 -> 1 in the time KH3's value runs 1 -> camfade.alpha
    let spu = camfade::P.camfade_seconds_per_unit;
    let span = (1.0 - camfade::P.camfade_alpha.clamp(0.0, 1.0)).max(0.05);
    let rate = if spu > 0.0 { dt / spu / span } else { f32::INFINITY };
    let mut inside: HashMap<u32, f32> = HashMap::new();
    for (key, _, touch) in s.fade.entries() {
        let before = s.inside.get(key).copied().unwrap_or(0.0);
        let target = inside_target(*touch);
        let w = if (target - before).abs() <= rate { target } else { before + (target - before).signum() * rate };
        if w > 0.0 {
            inside.insert(*key, w);
        }
    }
    s.inside = inside;
    for (key, value, touch) in s.fade.entries() {
        let shown = 1.0 - amount_with_inside(*value, s.inside.get(key).copied().unwrap_or(0.0));
        if shown < 1.0 {
            let e = s.faded.entry(*key).or_insert((0, 1.0, None));
            e.0 += 1;
            e.1 = e.1.min(shown);
            e.2 = touch.or(e.2);
        }
    }
}

/// His block values this frame: (1 - CameraFade value, phase).
pub fn block_of(key: u32) -> (f32, f32) {
    STATE.lock().ok().and_then(|g| g.as_ref().map(|s| (amount_with_inside(s.fade.value(key), s.inside.get(&key).copied().unwrap_or(0.0)), s.phase))).unwrap_or((0.0, 0.0))
}

/// Floats of the frame block: `cam_right.w` and `cam_up.w` (er_char.wgsl ErFrame: clip 0..15, camera 16..19, cam_right
/// 20..23, cam_up 24..27).
pub const AMOUNT_AT: usize = 23;
pub const PHASE_AT: usize = 27;
/// `blob_fade` (16 + 4 x 12: after clip, camera, the three axes, viewport, key_dir, key_color, proj, depth, depth_uv,
/// out_mode, blob).
pub const BLOB_FADE_AT: usize = 16 + 4 * 12;

/// Writes his values into a copy of the frame block.
pub fn write_block(frame: &mut [f32], key: u32) {
    let (amount, phase) = block_of(key);
    if frame.len() > PHASE_AT {
        frame[AMOUNT_AT] = amount;
        frame[PHASE_AT] = phase;
    }
    // his shadow on the world (the contact shadow blob_fade.x and the thrown one blob_fade.w) at his CameraFade value: KH3's
    // shadow depth runs the same masked shader, so a half-faded character casts about half a shadow
    if frame.len() > BLOB_FADE_AT + 3 && amount > 0.0 {
        frame[BLOB_FADE_AT] *= 1.0 - amount;
        frame[BLOB_FADE_AT + 3] *= 1.0 - amount;
    }
}

/// One line for the log (once a second), then the counters start again.
pub fn status() -> String {
    let Ok(mut guard) = STATE.lock() else { return String::new() };
    let Some(s) = guard.as_mut() else { return "(NF) camera fade: no frame yet".to_string() };
    let name = |k: u32| if k == SORA { "Sora".to_string() } else { format!("member {}", k - MEMBER) };
    let mut parts: Vec<String> = s.faded.iter().map(|(k, (n, v, t))| format!("{} {} frame(s) down to {:.2} ({})", name(*k), n, v, t.map_or("returning", |t| match t {
        camfade::Touch::Inside => "camera inside his camera volume",
        camfade::Touch::Between => "between the camera and the look point",
        camfade::Touch::Limb => "camera inside a limb",
    }))).collect();
    parts.sort();
    let line = format!("(NF) camera fade ({}{}): {} of {} frame(s); {}", if enabled() { "on" } else { "OFF (r.camfade 0)" }, if INSIDE_OUT.load(Ordering::Relaxed) { ", camera inside him: out entirely (RSF2)" } else { ", camera inside him: KH3's 0.5 (r.camfade_inside 0)" }, s.faded.values().map(|v| v.0).max().unwrap_or(0), s.frames, if parts.is_empty() { "nobody faded".to_string() } else { parts.join("; ") });
    s.faded.clear();
    s.frames = 0;
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_anim::Trs;

    /// (RSF round 18) the tests that set the friends' rule take turns
    static RULE_TESTS: Mutex<()> = Mutex::new(());

    /// (RSF round 18) every KH character takes Sora's (the lead's) rule unless r.camfade_friends = 1: a friend on the look line
    /// is not faded; the camera inside a body still fades it (the lead's camera-inside test is kept)
    #[test]
    fn rsf18_friends_take_soras_fade_rule() {
        let _turn = RULE_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        set_friends_look_fade(false);
        assert!(as_lead(SORA, SORA) && as_lead(7, SORA) && as_lead(SORA, 7));
        let vol = camfade::Volume { kind: camfade::Kind::Camera, shape: camfade::Shape { a: glam::Vec3::new(0.0, 0.0, 0.0), b: glam::Vec3::new(0.0, 0.0, 120.0), r: 40.0 } };
        let (cam, look) = (glam::Vec3::new(-400.0, 0.0, 100.0), glam::Vec3::new(400.0, 0.0, 100.0));
        // on the look line: KH3 fades a friend (lead false), the lead's rule does not
        assert_eq!(camfade::touches(cam, Some(look), false, &[vol]), Some(camfade::Touch::Between));
        assert_eq!(camfade::touches(cam, Some(look), as_lead(7, SORA), &[vol]), None);
        // the camera inside him: faded under both rules
        assert!(camfade::touches(glam::Vec3::new(0.0, 0.0, 60.0), Some(look), as_lead(7, SORA), &[vol]).is_some());
        set_friends_look_fade(true);
        assert!(!as_lead(7, SORA) && as_lead(SORA, SORA));
        set_friends_look_fade(false);
    }

    #[test]
    fn nf_block_floats_are_cam_right_w_and_cam_up_w() {
        // er_char.wgsl ErFrame: clip_from_game (16 floats), camera, cam_right, cam_up: .w at 23 and 27; the shader reads them
        let text = include_str!("../shaders/er_char.wgsl");
        let head = &text[text.find("struct ErFrame {").unwrap()..];
        let fields: Vec<&str> = head.lines().skip(1).map(str::trim).filter(|l| !l.starts_with("//") && l.contains(':')).take(14).collect();
        assert_eq!(fields[..4], ["clip_from_game: mat4x4<f32>,", "camera: vec4<f32>,", "cam_right: vec4<f32>,", "cam_up: vec4<f32>,"]);
        assert_eq!(fields[13], "blob_fade: vec4<f32>,");
        assert!(fields[1..13].iter().all(|f| f.ends_with("vec4<f32>,")), "{fields:?}");
        assert!(text.contains("let value = 1.0 - er.cam_right.w;") && text.contains("u32(max(er.cam_up.w, 0.0))"));
        let mut block = vec![0.0f32; 64];
        write_block(&mut block, 77);
        assert_eq!((block[AMOUNT_AT], block[PHASE_AT]), (0.0, 0.0), "a character not in the list is whole");
    }

    #[test]
    fn nf_shader_constants_are_the_sheets() {
        // the three shader copies (er_char.wgsl er_camfade_keep, char_common.wgsl kh_camfade_keep / _cover) use the rows
        let p = camfade::P;
        assert_eq!((p.camfade_dither_phases, p.camfade_dither_scale, p.camfade_dither_bias), (6, 0.857143, 0.8333));
        for text in [include_str!("../shaders/er_char.wgsl"), kh_look::CHAR_COMMON_WGSL] {
            assert!(text.contains("for (var k = 0u; k < 6u; k++)") && text.contains("/ 6.0 * 0.857143 + value - 0.8333 >= 0.0"));
            assert!(text.contains("let cell = 4u * (((a.x ^ a.y) & 1u) * 2u + a.y) + ((b.x ^ b.y) & 1u) * 2u + b.y;") && text.contains("return (f32(cell) + 0.5) / 16.0 < f32(kept) / 6.0;"));
        }
        // the shaders' pixel test (transcribed) is camfade::keeps at every value and phase
        let shader = |x: u32, y: u32, ph: u32, value: f32| {
            let kept = (0..6u32).filter(|k| *k as f32 / 6.0 * 0.857143 + value - 0.8333 >= 0.0).count() as f32;
            let (px, py) = (x + ph, y + 3 * ph);
            let (ax, ay, bx, by) = (px & 1, py & 1, (px >> 1) & 1, (py >> 1) & 1);
            let cell = 4 * (((ax ^ ay) & 1) * 2 + ay) + ((bx ^ by) & 1) * 2 + by;
            (cell as f32 + 0.5) / 16.0 < kept / 6.0
        };
        for v in [0.5f32, 0.6, 0.75, 0.9, 0.95] {
            for (x, y, ph) in [(0u32, 0u32, 0u32), (5, 9, 3), (1919, 1079, 1), (7, 2, 2)] {
                assert_eq!(camfade::keeps(x, y, ph, v), shader(x, y, ph, v), "{v} {x} {y} {ph}");
            }
        }
    }

    #[test]
    fn nf_every_drawn_character_fades_by_his_own_volumes() {
        // (RSF round 18) this test is KH3's own friend rule (r.camfade_friends = 1); the default is Sora's rule for everyone
        let _turn = RULE_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        set_friends_look_fade(true);
        // a mocked character (no rows: the limb guard) and Goofy's rows, posed upright; the camera inside each in turn
        let up = glam::Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
        let names = ["center", "hara2", "R_te", "koshi", "mune", "kubi", "atama"];
        let heights = [95.0, 100.0, 100.0, 90.0, 120.0, 145.0, 155.0];
        let bones: HashMap<String, usize> = names.iter().enumerate().map(|(k, n)| (n.to_string(), k)).collect();
        let body_at = |x: f32| Body { component_to_world: Trs { translation: Vec3::new(x, 0.0, 0.0), rotation: glam::Quat::IDENTITY, scale: Vec3::ONE }, local: vec![Trs::IDENTITY; names.len()], component: heights.iter().map(|h| Trs { translation: Vec3::new(0.0, 0.0, *h), rotation: up, scale: Vec3::ONE }).collect() };
        let goofy = body_at(0.0);
        let mock = body_at(400.0);
        let drawn = vec![
            Drawn { key: member_key(2), volumes: volumes_of("n_ex002", &bones, &goofy), middle: Vec3::new(0.0, 0.0, 100.0) },
            Drawn { key: member_key(9), volumes: volumes_of("x_none99", &bones, &mock), middle: Vec3::new(400.0, 0.0, 100.0) },
        ];
        assert!(drawn[1].volumes.iter().all(|v| v.kind == camfade::Kind::Limb) && !drawn[1].volumes.is_empty());
        set_view(View { look: Some(Vec3::new(0.0, -800.0, 100.0)), lead: None });
        set_inside_out(false); // (RSF2) KH3's own rule first: 0.5 with the camera inside
        let mut clock = 0.0;
        for _ in 0..30 {
            clock += 1.0 / 60.0;
            step(clock, Vec3::new(5.0, 0.0, 110.0), 1, false, &drawn);
        }
        assert!((block_of(member_key(2)).0 - 0.5).abs() < 1.0e-5, "inside Goofy: 1 - 0.5");
        assert_eq!(block_of(member_key(9)).0, 0.0);
        for _ in 0..30 {
            clock += 1.0 / 60.0;
            step(clock, Vec3::new(400.0, 0.0, 130.0), 1, false, &drawn);
        }
        assert_eq!(block_of(member_key(2)).0, 0.0, "back to whole");
        assert!((block_of(member_key(9)).0 - 0.5).abs() < 1.0e-5, "the mocked character fades by his limb volumes");
        let line = status();
        assert!(line.contains("member 9") && line.contains("member 2"), "{line}");

        // (RSF2 round 4) `r.camfade_inside` 1 (the default): the camera INSIDE a character takes him out entirely (no screen
        // door left to see), ramped in the fade's own time; the look-line case keeps KH3's 0.5; leaving brings him back
        set_inside_out(true);
        let mut ramp = Vec::new();
        for _ in 0..30 {
            clock += 1.0 / 60.0;
            set_view(View { look: Some(Vec3::new(0.0, -800.0, 100.0)), lead: None });
            step(clock, Vec3::new(5.0, 0.0, 110.0), 1, false, &drawn);
            ramp.push(block_of(member_key(2)).0);
        }
        assert!((block_of(member_key(2)).0 - 1.0).abs() < 1.0e-5, "inside Goofy: out entirely, {ramp:?}");
        assert!(ramp.windows(2).all(|w| w[1] >= w[0] - 1.0e-6) && ramp[0] < 0.5, "a ramp, not a jump: {ramp:?}");
        assert!((block_of(member_key(9)).0).abs() < 1.0e-5, "the mocked character returns whole");
        assert!(status().contains("out entirely"));
        // the look line through Goofy (the camera behind him, outside his volumes): KH3's 0.5, not out
        for _ in 0..60 {
            clock += 1.0 / 60.0;
            set_view(View { look: Some(Vec3::new(0.0, -800.0, 100.0)), lead: None });
            step(clock, Vec3::new(0.0, 600.0, 110.0), 1, false, &drawn);
        }
        assert!((block_of(member_key(2)).0 - 0.5).abs() < 1.0e-5, "between the camera and the look point: 0.5, {}", block_of(member_key(2)).0);
        // the mocked character, camera inside his limb volumes: out as well (no names: every character)
        for _ in 0..30 {
            clock += 1.0 / 60.0;
            set_view(View { look: Some(Vec3::new(0.0, -800.0, 100.0)), lead: None });
            step(clock, Vec3::new(400.0, 0.0, 130.0), 1, false, &drawn);
        }
        assert!((block_of(member_key(9)).0 - 1.0).abs() < 1.0e-5, "the mocked character: out entirely");
        let mut back = Vec::new();
        for _ in 0..30 {
            clock += 1.0 / 60.0;
            set_view(View { look: Some(Vec3::new(0.0, -800.0, 100.0)), lead: None });
            step(clock, Vec3::new(1200.0, 0.0, 130.0), 1, false, &drawn);
            back.push(block_of(member_key(9)).0);
        }
        assert!(back.windows(2).all(|w| w[1] <= w[0] + 1.0e-6) && back[0] > 0.5 && back[29] == 0.0, "back whole by a ramp: {back:?}");
        assert_eq!((amount_with_inside(0.5, 0.0), amount_with_inside(0.5, 1.0), amount_with_inside(1.0, 0.0)), (0.5, 1.0, 0.0));
        set_friends_look_fade(false); // (RSF round 18) back to the default
    }
}

//! (IMPL_ER21) KINGDOM HEARTS III'S CONFIG SCREEN DRIVES ELDEN RING'S OPTIONS (`config.kh`, default on; the user,
//! 2026-10-13: "Make Config use Kingdom Heart's config menu and make the settings there actually affect Elden Ring's
//! settings"). `config.kh = 0`: the camp menu's Config entry opens Elden Ring's system menu, as before this pass.
//!
//! WHERE ELDEN RING'S OPTIONS ARE: `GameDataMan -> game_settings` (the SDK's `GameSettings`, one byte a field; two of
//! its fields were written by earlier passes and seen to act in the game: `hud_type` and `show_tutorials`). The
//! game keeps them in the SYSTEM part of its save (start function S4, never refused by `save.points_only`), which
//! it writes at the title and when its own options menu is left: what we write in memory persists the way the
//! game's own changes do.
//!
//! THE TABLE = the rows `config.<KH3 option>` of sheets/port/sora-er-config.json (the compiled table below is the
//! same; a row's `state` other than `live` switches its writes and reads off). Rows without an Elden Ring field are
//! Sora's own (the command menu's options) or have no meaning here.
//!
//! WHEN: while the Config screen is NOT up, Elden Ring's values are read into the KH rows whenever they differ from
//! what the KH rows map to (so the screen opens with the game's values; a change made in Elden Ring's own menu is
//! taken over); when the screen applies (B / leaving it), every mapped row whose value maps to something else than
//! the game holds is written. "Restore defaults" (Y) gives KH3's defaults, which map to Elden Ring's (camera speed 5,
//! brightness 5, volume 10, no flips, subtitles on).
//!
//! OUR OWN SIDE of the same rows: the KH camera's stick (flips, sensitivities) and its auto-follow, and the master
//! level of Sora's own mixer (`camera()`, `master_gain()`).

use std::sync::atomic::{AtomicU32, Ordering};

use kh_core::config::{ConfigOption as O, UserConfig};

/// A field of Elden Ring's `GameSettings`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    CameraSpeed,
    Rumble,
    Brightness,
    MasterVolume,
    Subtitles,
    ReverseX,
    ReverseY,
    AutoRotation,
}

impl Field {
    pub const ALL: [Field; 8] = [Field::CameraSpeed, Field::Rumble, Field::Brightness, Field::MasterVolume, Field::Subtitles, Field::ReverseX, Field::ReverseY, Field::AutoRotation];

    /// The SDK's field name (the sheet's `er` cell).
    pub fn name(self) -> &'static str {
        match self {
            Field::CameraSpeed => "camera_speed",
            Field::Rumble => "controller_rumble_strength",
            Field::Brightness => "brightness",
            Field::MasterVolume => "master_volume",
            Field::Subtitles => "show_subtitles",
            Field::ReverseX => "reverse_camera_xaxis",
            Field::ReverseY => "reverse_camera_yaxis",
            Field::AutoRotation => "camera_auto_rotation",
        }
    }

    /// The largest value the game's own menu can set (0 ..= this).
    pub fn max(self) -> u8 {
        match self {
            Field::CameraSpeed | Field::Rumble | Field::Brightness | Field::MasterVolume => 10,
            _ => 1,
        }
    }
}

/// One row of the table: a KH3 option and the Elden Ring field it drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub kh: O,
    pub field: Field,
    pub live: bool,
}

/// THE COMPILED TABLE (= the sheet's `live` rows). Two KH3 rows drive the one camera speed.
pub const TABLE: [Row; 9] = [
    Row { kh: O::CameraWork, field: Field::AutoRotation, live: true },
    // (ZETTA 2026-10-23, the user, 2026-10-23: "also the game should by default NOT reverse the camera controls in the KH menu settings") the KH camera's flips are KH3's own (Normal by default): Elden Ring's reverse settings are
    // neither read into them nor written from them (the game's own camera keeps the player's own Elden Ring choice)
    Row { kh: O::VerticalCameraControl, field: Field::ReverseY, live: false },
    Row { kh: O::HorizontalCameraControl, field: Field::ReverseX, live: false },
    Row { kh: O::VerticalCameraSensitivity, field: Field::CameraSpeed, live: true },
    Row { kh: O::HorizontalCameraSensitivity, field: Field::CameraSpeed, live: true },
    Row { kh: O::Subtitles, field: Field::Subtitles, live: true },
    Row { kh: O::Vibration, field: Field::Rumble, live: true },
    Row { kh: O::MasterVolume, field: Field::MasterVolume, live: true },
    Row { kh: O::ScreenBrightness, field: Field::Brightness, live: true },
];

/// The KH3 option's name as the sheet's id spells it (`config.<name>`).
pub fn option_name(option: O) -> &'static str {
    match option {
        O::CameraWork => "CameraWork",
        O::VerticalCameraControl => "VerticalCameraControl",
        O::HorizontalCameraControl => "HorizontalCameraControl",
        O::VerticalCameraSensitivity => "VerticalCameraSensitivity",
        O::HorizontalCameraSensitivity => "HorizontalCameraSensitivity",
        O::VerticalReticleControl => "VerticalReticleControl",
        O::HorizontalReticleControl => "HorizontalReticleControl",
        O::VerticalRideControl => "VerticalRideControl",
        O::ComboCam => "ComboCam",
        O::KeybladeCinematics => "KeybladeCinematics",
        O::LinkCinematics => "LinkCinematics",
        O::AttractionCinematics => "AttractionCinematics",
        O::CommandMenus => "CommandMenus",
        O::CommandSelection => "CommandSelection",
        O::CursorPosition => "CursorPosition",
        O::SituationCommands => "SituationCommands",
        O::ControlType => "ControlType",
        O::Subtitles => "Subtitles",
        O::Maps => "Maps",
        O::MapOrientation => "MapOrientation",
        O::Vibration => "Vibration",
        O::MasterVolume => "MasterVolume",
        O::Graphics => "Graphics",
        O::DisplayArea => "DisplayArea",
        O::ScreenBrightness => "ScreenBrightness",
        O::Language => "Language",
    }
}

/// The table with the sheet's states over it: a row whose `state` is not `live` is off. The second value: ids the
/// sheet names that are not understood.
pub fn table_from_sheet(text: &str) -> Result<([Row; 9], Vec<String>), String> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let rows = json.get("rows").and_then(|r| r.as_array()).ok_or("no rows")?;
    let mut table = TABLE;
    let mut problems = Vec::new();
    for row in rows {
        let (Some(id), Some(value)) = (row.get(0).and_then(|v| v.as_str()), row.get(1)) else { continue };
        let Some(name) = id.strip_prefix("config.") else {
            problems.push(id.to_string());
            continue;
        };
        let (er, state) = (value.get("er").and_then(|v| v.as_str()).unwrap_or("-"), value.get("state").and_then(|v| v.as_str()).unwrap_or(""));
        match table.iter_mut().find(|r| option_name(r.kh) == name) {
            Some(r) if r.field.name() == er => r.live = state == "live",
            Some(_) => problems.push(format!("{id}: the field {er} is not the one this build maps")),
            // (a row that only says the option is Sora's own / has no counterpart)
            None if er == "-" && kh_core::config::OPTIONS.iter().any(|info| option_name(info.option) == name) => {}
            None => problems.push(id.to_string()),
        }
    }
    Ok((table, problems))
}

// ---------------------------------------------------------------- the mappings (pure)

/// Elden Ring's camera speed 0 ..= 10 for KH3's two sensitivities 1 ..= 100 (their mean; KH3's default 50 = the
/// game's default 5).
pub fn speed_of(vertical: u8, horizontal: u8) -> u8 {
    (((vertical.clamp(1, 100) as f32 + horizontal.clamp(1, 100) as f32) / 20.0).round() as u8).min(10)
}

/// KH3's sensitivity for Elden Ring's camera speed (both rows the same; 0 = KH3's lowest, 1).
pub fn sensitivity_of(speed: u8) -> u8 {
    (speed.min(10) * 10).max(1)
}

/// Elden Ring's brightness 0 ..= 10 for KH3's 0 ..= 100. KH3's default is 0 and its slider only brightens: 0 = the
/// game's default 5, 100 = its brightest 10 (LOCAL: KH3's own curve of the slider was not read).
pub fn brightness_of(kh: u8) -> u8 {
    5 + ((kh.min(100) as f32 / 20.0).round() as u8).min(5)
}

/// KH3's brightness for Elden Ring's (a value under the game's default 5 reads as KH3's 0 and is left alone until
/// the KH3 row is moved).
pub fn kh_brightness_of(er: u8) -> u8 {
    er.clamp(5, 10).saturating_sub(5) * 20
}

/// What the KH3 block maps to for a field (`now` = the field's present value: Vibration On keeps a strength that
/// is already above 0).
pub fn er_value(config: &UserConfig, field: Field, now: u8) -> u8 {
    match field {
        Field::CameraSpeed => speed_of(config.get(O::VerticalCameraSensitivity), config.get(O::HorizontalCameraSensitivity)),
        Field::Rumble => match (config.vibration(), now) {
            (false, _) => 0,
            (true, 1..=10) => now,
            (true, _) => 10,
        },
        Field::Brightness => {
            let want = brightness_of(config.get(O::ScreenBrightness));
            // (the game's darker half is not reachable from KH3's slider: left as it is while KH3 says "default")
            if want == 5 && now < 5 { now } else { want }
        }
        Field::MasterVolume => config.get(O::MasterVolume).min(10),
        Field::Subtitles => (config.get(O::Subtitles) != 0) as u8,
        Field::ReverseX => config.camera_flip().1 as u8,
        Field::ReverseY => config.camera_flip().0 as u8,
        Field::AutoRotation => config.camera_auto() as u8,
    }
}

/// The game's value of a field taken into the KH3 block (only called when it differs from what the block maps to,
/// so a finer KH3 value that already means the same is kept).
pub fn take_er_value(config: &mut UserConfig, field: Field, er: u8) {
    match field {
        Field::CameraSpeed => {
            config.set(O::VerticalCameraSensitivity, sensitivity_of(er));
            config.set(O::HorizontalCameraSensitivity, sensitivity_of(er));
        }
        Field::Rumble => config.set(O::Vibration, (er > 0) as u8),
        Field::Brightness => config.set(O::ScreenBrightness, kh_brightness_of(er)),
        Field::MasterVolume => config.set(O::MasterVolume, er.min(10)),
        Field::Subtitles => config.set(O::Subtitles, (er != 0) as u8),
        Field::ReverseX => config.set(O::HorizontalCameraControl, (er != 0) as u8),
        Field::ReverseY => config.set(O::VerticalCameraControl, (er != 0) as u8),
        // (Camera Work: choice 0 = Auto)
        Field::AutoRotation => config.set(O::CameraWork, (er == 0) as u8),
    }
}

/// ELDEN RING -> KH3 (pure): `read(field)` = the game's value. Returns the fields that were taken over.
pub fn sync_from(config: &mut UserConfig, table: &[Row], read: &dyn Fn(Field) -> Option<u8>) -> Vec<(Field, u8)> {
    let mut taken = Vec::new();
    for field in Field::ALL {
        if !table.iter().any(|r| r.field == field && r.live) {
            continue;
        }
        let Some(er) = read(field).filter(|v| *v <= field.max()) else { continue };
        if er_value(config, field, er) != er {
            take_er_value(config, field, er);
            taken.push((field, er));
        }
    }
    taken
}

/// KH3 -> ELDEN RING (pure): the writes an apply has to make: (field, the game's value, the new one).
pub fn writes_for(config: &UserConfig, table: &[Row], read: &dyn Fn(Field) -> Option<u8>) -> Vec<(Field, u8, u8)> {
    let mut out = Vec::new();
    for field in Field::ALL {
        if !table.iter().any(|r| r.field == field && r.live) {
            continue;
        }
        let Some(now) = read(field) else { continue };
        let want = er_value(config, field, now).min(field.max());
        if want != now {
            out.push((field, now, want));
        }
    }
    out
}

// ---------------------------------------------------------------- Elden Ring's side (game thread)

/// The byte of a field in the live settings object (the pointer is checked).
fn byte(field: Field) -> Option<*mut u8> {
    use eldenring::cs::GameDataMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game; one-byte fields of the live settings object, read
    // and written as bytes (an odd value must not become a bool or an enum).
    let man = unsafe { GameDataMan::instance_mut() }.ok()?;
    if !crate::game::plausible(crate::word!(man.game_settings)) {
        return None;
    }
    let s = &mut man.game_settings;
    Some(match field {
        Field::CameraSpeed => std::ptr::addr_of_mut!(s.camera_speed),
        Field::Rumble => std::ptr::addr_of_mut!(s.controller_rumble_strength),
        Field::Brightness => std::ptr::addr_of_mut!(s.brightness),
        Field::MasterVolume => std::ptr::addr_of_mut!(s.master_volume),
        Field::Subtitles => std::ptr::addr_of_mut!(s.show_subtitles) as *mut u8,
        Field::ReverseX => std::ptr::addr_of_mut!(s.reverse_camera_xaxis) as *mut u8,
        Field::ReverseY => std::ptr::addr_of_mut!(s.reverse_camera_yaxis) as *mut u8,
        Field::AutoRotation => std::ptr::addr_of_mut!(s.camera_auto_rotation) as *mut u8,
    })
}

pub fn read(field: Field) -> Option<u8> {
    // SAFETY: see `byte`.
    byte(field).map(|p| unsafe { p.read() })
}

/// Writes one field (a value inside the game's own range only). False: the settings object is not there.
pub fn write(field: Field, value: u8) -> bool {
    if value > field.max() {
        return false;
    }
    match byte(field) {
        // SAFETY: see `byte`.
        Some(p) => {
            unsafe { p.write(value) };
            true
        }
        None => false,
    }
}

// ---------------------------------------------------------------- the HUD's frame

struct Live {
    table: [Row; 9],
    sheet: Option<std::path::PathBuf>,
    loaded: bool,
    told: Option<bool>,
}

static LIVE: std::sync::Mutex<Live> = std::sync::Mutex::new(Live { table: TABLE, sheet: None, loaded: false, told: None });

/// Where the sheet is (beside the loadout sheet; hud.rs, once).
pub fn set_sheet(path: std::path::PathBuf) {
    if let Ok(mut live) = LIVE.lock() {
        live.sheet = Some(path);
    }
}

/// KH3's own block is kept beside Sora's saves (the rows Elden Ring has no option for, and the finer values).
fn config_file() -> Option<std::path::PathBuf> {
    crate::save::dir().map(|d| d.join("kh-config.txt"))
}

/// ONCE A HUD FRAME in play (hud.rs): the switch, Elden Ring's values into the KH3 rows while the Config screen is
/// not up, the writes when it applied.
pub fn hud_frame(hud: &mut kh_host::hud::Hud, control: &crate::control::Control) {
    let on = control.flag("config.kh") != Some(false);
    hud.config_external = !on;
    let Ok(mut live) = LIVE.lock() else { return };
    if live.told != Some(on) {
        live.told = Some(on);
        crate::erlog!("config: the Kingdom Hearts menu's Config entry {}", if on { "opens KINGDOM HEARTS III'S CONFIG SCREEN; its rows drive Elden Ring's options (config.kh = 0: Elden Ring's system menu instead; LB + Start opens that menu at any time)" } else { "opens Elden Ring's system menu (config.kh = 0)" });
    }
    if !on {
        return;
    }
    if !live.loaded {
        live.loaded = true;
        if let Some(text) = live.sheet.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            match table_from_sheet(&text) {
                Ok((table, problems)) => {
                    live.table = table;
                    crate::erlog!("config: sheet sora-er-config read: {} of {} mapped rows live{}", table.iter().filter(|r| r.live).count(), table.len(), if problems.is_empty() { String::new() } else { format!("; NOT understood: {problems:?}") });
                }
                Err(why) => crate::erlog!("config: the sheet sora-er-config could not be read ({why}): the compiled table is used"),
            }
        }
        if let Some(mut config) = config_file().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| UserConfig::from_text(&t)) {
            // (ZETTA 2026-10-23, the user, 2026-10-23: "also the game should by default NOT reverse the camera controls in the KH menu settings") ONCE: the camera controls the Elden Ring sync had put there go back to Normal
            let marker = camera_marker_file();
            let marked = marker.as_ref().is_some_and(|m| m.exists());
            let was = config.camera_flip();
            if migrate_camera(&mut config, marked) {
                if let Some(m) = marker {
                    let _ = std::fs::write(&m, "the KH camera controls were set back to Normal once (they had been taken from Elden Ring's reverse settings); a later choice on the KH Config screen is kept\n");
                }
                if let Some(path) = config_file() {
                    let _ = std::fs::write(&path, config.to_text());
                }
                crate::erlog!("config: (once) the KH camera controls (vertical, horizontal flipped: {:?}) are set back to NORMAL: they came from Elden Ring's reverse settings, which no longer drive them", was);
            }
            hud.set_config(config);
            crate::erlog!("config: KH3's own block read from {} (block {:02X?})", config_file().map_or(String::new(), |p| p.display().to_string()), &config.block[..9]);
        }
    }
    let table = live.table;
    if hud.take_config_save() {
        // THE SCREEN APPLIED: every mapped row that now means another value than the game holds is written
        let config = *hud.config();
        let writes = writes_for(&config, &table, &read);
        for (field, was, now) in &writes {
            let done = write(*field, *now);
            crate::erlog!("config: Elden Ring's option {} {} -> {} ({})", field.name(), was, now, if done { "WRITTEN in memory: the game keeps it in its system save" } else { "NOT written: the game's settings are not there" });
        }
        if writes.is_empty() {
            crate::erlog!("config: applied: no Elden Ring option changes (block {:02X?})", &config.block[..9]);
        }
        if let Some(path) = config_file() {
            let _ = path.parent().map(std::fs::create_dir_all);
            if let Err(e) = std::fs::write(&path, config.to_text()) {
                crate::erlog!("config: {} could not be written ({e})", path.display());
            }
        }
    } else if hud.camp_screen() != Some(kh_ui::camp::CampScreen::Config) {
        let mut config = *hud.config();
        let taken = sync_from(&mut config, &table, &read);
        if !taken.is_empty() {
            hud.set_config(config);
            crate::erlog!("config: Elden Ring's own values taken into the Kingdom Hearts rows: {}", taken.iter().map(|(f, v)| format!("{} = {v}", f.name())).collect::<Vec<_>>().join(", "));
        }
    }
    publish(hud.config());
}

/// (ZETTA 2026-10-23) The one-time marker of the camera controls' migration (beside kh-config.txt).
fn camera_marker_file() -> Option<std::path::PathBuf> {
    crate::save::dir().map(|d| d.join("kh-config-camera-normal.v1"))
}

/// (ZETTA 2026-10-23, the user, 2026-10-23: "also the game should by default NOT reverse the camera controls in the KH menu settings") THE ONE-TIME MIGRATION of a saved block: until its marker exists the two camera controls (put
/// there by the old Elden Ring -> KH sync) go back to Normal; true: done now (the caller writes the marker and the block).
/// Once marked, the player's own choice on the KH Config screen stands.
pub fn migrate_camera(config: &mut UserConfig, marked: bool) -> bool {
    if marked {
        return false;
    }
    config.set(O::VerticalCameraControl, 0);
    config.set(O::HorizontalCameraControl, 0);
    true
}

// ---------------------------------------------------------------- our own side

/// bits 0 / 1: vertical / horizontal flip, bit 2: Camera Work Manual; bits 8.. / 16..: the sensitivities.
static CAMERA: AtomicU32 = AtomicU32::new(50 << 8 | 50 << 16);
/// KH3's Master Volume 0 ..= 10.
static MASTER: AtomicU32 = AtomicU32::new(10);

/// The Config rows in force for Sora's own camera and mixer (hud.rs, whenever the configuration changes).
pub fn publish(config: &UserConfig) {
    let (flip_v, flip_h) = config.camera_flip();
    let (sens_v, sens_h) = config.camera_sensitivity();
    CAMERA.store(flip_v as u32 | (flip_h as u32) << 1 | (!config.camera_auto() as u32) << 2 | (sens_v as u32) << 8 | (sens_h as u32) << 16, Ordering::Relaxed);
    MASTER.store(config.get(O::MasterVolume).min(10) as u32, Ordering::Relaxed);
}

/// The KH camera's stick factors (x, y: flips and sensitivities; 1, 1 at KH3's defaults) and whether the camera
/// follows by itself (Camera Work: Auto).
pub fn camera() -> (f32, f32, bool) {
    let c = CAMERA.load(Ordering::Relaxed);
    let scale = |setting: u32| kh_camera::sensitivity(setting.clamp(1, 100) as f32) / kh_camera::sensitivity(50.0);
    (scale(c >> 16 & 0xFF) * if c & 2 != 0 { -1.0 } else { 1.0 }, scale(c >> 8 & 0xFF) * if c & 1 != 0 { -1.0 } else { 1.0 }, c & 4 == 0)
}

/// KH3's Master Volume as a gain on Sora's own mixer (linear, as the test app takes it).
pub fn master_gain() -> f32 {
    MASTER.load(Ordering::Relaxed).min(10) as f32 / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_is_the_table() {
        let text = include_str!("../../../sheets/port/sora-er-config.json");
        let (table, problems) = table_from_sheet(text).unwrap();
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(table, TABLE);
        // every KH3 option has a row
        let json: serde_json::Value = serde_json::from_str(text).unwrap();
        let ids: Vec<String> = json["rows"].as_array().unwrap().iter().map(|r| r[0].as_str().unwrap().to_string()).collect();
        for info in kh_core::config::OPTIONS.iter() {
            assert!(ids.contains(&format!("config.{}", option_name(info.option))), "{:?}", info.option);
        }
        // the offsets the sheet names are the SDK's
        use eldenring::cs::GameSettings;
        let offsets = [("camera_speed", std::mem::offset_of!(GameSettings, camera_speed)), ("controller_rumble_strength", std::mem::offset_of!(GameSettings, controller_rumble_strength)), ("brightness", std::mem::offset_of!(GameSettings, brightness)), ("master_volume", std::mem::offset_of!(GameSettings, master_volume)), ("show_subtitles", std::mem::offset_of!(GameSettings, show_subtitles)), ("reverse_camera_xaxis", std::mem::offset_of!(GameSettings, reverse_camera_xaxis)), ("reverse_camera_yaxis", std::mem::offset_of!(GameSettings, reverse_camera_yaxis)), ("camera_auto_rotation", std::mem::offset_of!(GameSettings, camera_auto_rotation)), ("hud_type", std::mem::offset_of!(GameSettings, hud_type)), ("show_tutorials", std::mem::offset_of!(GameSettings, show_tutorials))];
        for row in json["rows"].as_array().unwrap() {
            let (er, offset) = (row[1]["er"].as_str().unwrap_or("-"), row[1]["offset"].as_u64());
            if let Some((_, want)) = offsets.iter().find(|(name, _)| *name == er) {
                assert_eq!(offset, Some(*want as u64), "{er}");
            }
        }
    }

    #[test]
    fn camera_speed_both_ways() {
        // KH3's defaults (50, 50) are Elden Ring's default 5
        assert_eq!(speed_of(50, 50), 5);
        assert_eq!((speed_of(1, 1), speed_of(100, 100), speed_of(100, 1), speed_of(64, 67)), (0, 10, 5, 7));
        for speed in 0..=10u8 {
            let kh = sensitivity_of(speed);
            assert!((1..=100).contains(&kh));
            assert_eq!(speed_of(kh, kh), speed, "speed {speed}");
        }
    }

    #[test]
    fn brightness_both_ways() {
        assert_eq!((brightness_of(0), brightness_of(100), brightness_of(50), brightness_of(9), brightness_of(10)), (5, 10, 8, 5, 6));
        for er in 5..=10u8 {
            assert_eq!(brightness_of(kh_brightness_of(er)), er);
        }
        // the game's darker half reads as KH3's 0 and is not overwritten by it
        assert_eq!(kh_brightness_of(2), 0);
        assert_eq!(er_value(&UserConfig::default(), Field::Brightness, 2), 2);
        assert_eq!(er_value(&UserConfig::default(), Field::Brightness, 7), 5);
    }

    #[test]
    fn every_value_written_is_inside_the_games_range() {
        let mut config = UserConfig::default();
        for v in [0u8, 1, 37, 50, 99, 100, 255] {
            for option in [O::VerticalCameraSensitivity, O::HorizontalCameraSensitivity, O::ScreenBrightness, O::MasterVolume, O::Vibration, O::Subtitles, O::CameraWork, O::VerticalCameraControl, O::HorizontalCameraControl] {
                config.set(option, v);
            }
            for field in Field::ALL {
                for now in [0u8, 3, 10] {
                    assert!(er_value(&config, field, now) <= field.max(), "{field:?} for {v}");
                }
            }
        }
    }

    /// (ZETTA 2026-10-23, the user, 2026-10-23: "also the game should by default NOT reverse the camera controls in the KH menu settings") a fresh block is Normal; Elden Ring's reverse settings do not flip it; the old block's
    /// flips go back to Normal once; afterwards the player's own choice is kept
    #[test]
    fn zetta_the_kh_camera_is_not_reversed_by_default() {
        assert_eq!(UserConfig::default().camera_flip(), (false, false), "a fresh block: Normal");
        let read = |f: Field| match f {
            Field::ReverseX | Field::ReverseY => Some(1),
            _ => None,
        };
        let mut config = UserConfig::default();
        assert!(sync_from(&mut config, &TABLE, &read).is_empty() && config.camera_flip() == (false, false), "Elden Ring reversed: KH stays Normal");
        assert!(writes_for(&config, &TABLE, &read).is_empty(), "nor is Elden Ring's own written");
        // an old block with the synced flips: Normal once
        let mut old = UserConfig::default();
        old.set(O::VerticalCameraControl, 1);
        old.set(O::HorizontalCameraControl, 1);
        assert!(migrate_camera(&mut old, false) && old.camera_flip() == (false, false));
        // marked: the player's choice stands
        old.set(O::VerticalCameraControl, 1);
        assert!(!migrate_camera(&mut old, true) && old.camera_flip() == (true, false), "the player's Flipped is kept");
        // the sheet's rows are off
        assert!(TABLE.iter().filter(|r| matches!(r.field, Field::ReverseX | Field::ReverseY)).all(|r| !r.live));
    }

    /// The round trip with a pretended game: its values are taken into KH3's rows, an apply writes KH3's back.
    #[test]
    fn the_two_agree() {
        use std::cell::RefCell;
        use std::collections::HashMap;
        // the game: camera speed 7, Y reversed, auto rotation off, rumble 4, brightness 3, master 6, subtitles off
        let game: RefCell<HashMap<&str, u8>> = RefCell::new([("camera_speed", 7), ("reverse_camera_yaxis", 1), ("reverse_camera_xaxis", 0), ("camera_auto_rotation", 0), ("controller_rumble_strength", 4), ("brightness", 3), ("master_volume", 6), ("show_subtitles", 0)].into_iter().collect());
        let read = |f: Field| game.borrow().get(f.name()).copied();
        let mut config = UserConfig::default();
        let taken = sync_from(&mut config, &TABLE, &read);
        assert_eq!(taken.len(), 4, "{taken:?}");
        assert_eq!(config.camera_sensitivity(), (70, 70));
        // (ZETTA 2026-10-23) the game's Y reversed does not flip the KH camera
        assert_eq!(config.camera_flip(), (false, false));
        assert!(!config.camera_auto() && config.vibration());
        assert_eq!((config.get(O::MasterVolume), config.get(O::Subtitles), config.get(O::ScreenBrightness)), (6, 0, 0));
        // now they agree: nothing to write, nothing more to take
        assert!(writes_for(&config, &TABLE, &read).is_empty());
        assert!(sync_from(&mut config, &TABLE, &read).is_empty());
        // a finer KH3 value that means the same is kept
        config.set(O::HorizontalCameraSensitivity, 72);
        config.set(O::VerticalCameraSensitivity, 68);
        assert!(sync_from(&mut config, &TABLE, &read).is_empty());
        assert_eq!(config.camera_sensitivity(), (68, 72));
        // the player changes rows on the KH3 screen: the writes
        config.set(O::Vibration, 0);
        config.set(O::MasterVolume, 9);
        config.set(O::ScreenBrightness, 60);
        config.set(O::HorizontalCameraSensitivity, 100);
        config.set(O::VerticalCameraSensitivity, 100);
        let mut writes = writes_for(&config, &TABLE, &read);
        writes.sort_by_key(|w| w.0.name());
        assert_eq!(writes, vec![(Field::Brightness, 3, 8), (Field::CameraSpeed, 7, 10), (Field::Rumble, 4, 0), (Field::MasterVolume, 6, 9)]);
        // KH3's defaults map to the game's defaults (its brightness stays where the player had it below 5)
        let mut defaults = config;
        defaults.restore_defaults();
        for (field, _, want) in writes_for(&defaults, &TABLE, &read) {
            game.borrow_mut().insert(field.name(), want);
        }
        let g = game.borrow();
        // (ZETTA 2026-10-23) the game's own reverse setting is the player's Elden Ring choice: left as it was
        assert_eq!((g["camera_speed"], 1 - g["reverse_camera_yaxis"], g["camera_auto_rotation"], g["master_volume"], g["show_subtitles"], g["brightness"], g["controller_rumble_strength"]), (5, 0, 1, 10, 1, 3, 4));
        // a row switched off in the sheet is neither read nor written
        let mut off = TABLE;
        off.iter_mut().for_each(|r| r.live = false);
        assert!(writes_for(&config, &off, &read).is_empty());
    }
}

//! (CAMP-OPTIONS, THE USER 2026-11-05: "change the camp's options menu so that they actually reflect Elden Ring's
//! settings, every Elden Ring settings option should be accessible from it, and remove any options that don't do
//! anything. Use tabs like Elden Ring but keep it all looking like KH") ELDEN RING'S OPTIONS SCREEN, AS DATA, AND THE
//! CONTROLLER THAT DRIVES IT THROUGH THE GAME'S OWN CODE. Engine-free: the game is the [`ErGame`] trait (kh-er's
//! `eroptions.rs` is the real one, the tests and the offline sheets use [`MockGame`]).
//!
//! THE CATALOGUE ([`TABS`]) IS ELDEN RING'S OWN, READ FROM eldenring.exe 2.7.1.0 / 2.7.2.0 (offline, read-only:
//! build/sora_tmp/weapons/CAMP-OPTIONS/re-a and re-b, inventory.json + NOTES.md): `CS::OptionSettingTopDialog`
//! (ctor 0x9672c0) makes the tabs in this order, each tab's builder adds its rows in this order (builders 0x959df0
//! Game Options, 0x959320 Camera Options, 0x95d540 Display, 0x959090 Sound, 0x95ad00 Network, 0x953560 Controller
//! Settings, 0x95cb30 Keyboard/Mouse Settings, 0x95c050 Graphics; sub screens 0x95b770 Advanced Settings, 0x95a9c0
//! HDR Adjust Image Quality, 0x95d200 Brightness). Every label, help line and value is the game's own text (GR_MenuText,
//! GR_LineHelp, GR_System_Message_win64, GR_Dialogues), fetched from its message repository in the language the game
//! runs in; the English beside each id is the fallback (and what the offline sheets show). Rows the release flags
//! hide on PC are left out exactly where the game leaves them out (the unused GR_System_Message 103000 toggle, Game
//! Options > Ray Tracing and Performance Settings, Network > Voice Chat and Display Player Names). Quit Game (tab 9)
//! holds no setting: the camp menu has its own Title Screen entry.
//!
//! THE ROUTE (what Elden Ring's own options screen does, NOTES.md 2.1): the screen keeps a MENU_OPTION_DATA copy and
//! on EVERY change runs `apply_menu_option_data(GameSettings, copy)` (0x25c780): the whole copy into GameSettings,
//! the performance setting, the graphics block into the window's pending screen setting (applied by
//! CSWindowImp::Update, the quality preset expanded). We do the same: `load_menu_option_data` (0x25bee0) fills a
//! fresh copy with the values in force, the row's byte is set, the copy is applied; the HDR switch then asks the
//! game's HDR apply (0x67b150). The Keyboard/Mouse rows and the prompt device are written into CSPcKeyConfig as the
//! game's own rows write them (live bytes, no copy). Screen Mode and Resolution call CSWindowImp's own setters
//! (0xe925d0 / 0xe92530) and, as the game's ApplyScreenSettingJob does, ask "Keep current settings?" (GR_Dialogues
//! 111101): No puts the previous ones back. The three screens Elden Ring draws itself (Key Bindings, the pad's Button
//! Settings, the Data Usage Agreement) have no opener that works without its own menu window (re-b NOTES): the row
//! opens Elden Ring's System menu on that tab (`ErGame::open_native`).
//!
//! THE KINGDOM HEARTS PAGE (last tab): the KH3 Config rows the port really reads (command menu: Command Selection,
//! Cursor Position, Control Type; the performance camera: Combo Cam, Keyblade Cinematics). KH3's other rows are gone
//! (STATE.md lists the evidence); the ones Elden Ring has itself (camera, volume, vibration, subtitles, brightness)
//! are Elden Ring's rows now, and the KH camera and KH sounds follow those (kh-er `eroptions.rs`).

use kh_core::config::{Choices, ConfigOption, UserConfig};
use kh_ui::camp_options::{OptAsk, OptEvent, OptItem, OptKind, OptPage};

/// The message files the texts are in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fmg {
    MenuText,
    LineHelp,
    SystemMessage,
    Dialogues,
}

impl Fmg {
    /// The game's message-repository categories: base, dlc01, dlc02 (its lookups try them in that order: GR_MenuText
    /// 0xd120e0 `mov r8d,0xC8` then 0x170 then 0x1D4; GR_LineHelp 0xd11ef0 0xC9 / 0x171 / 0x1D5).
    pub const fn categories(self) -> [u32; 3] {
        match self {
            Fmg::MenuText => [200, 368, 468],
            Fmg::LineHelp => [201, 369, 469],
            Fmg::SystemMessage => [203, 371, 471],
            Fmg::Dialogues => [204, 372, 472],
        }
    }
}

/// One text of the game's: where it is and its English (the fallback).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Text {
    pub fmg: Fmg,
    pub id: u32,
    pub en: &'static str,
}

const fn mt(id: u32, en: &'static str) -> Text {
    Text { fmg: Fmg::MenuText, id, en }
}
const fn lh(id: u32, en: &'static str) -> Text {
    Text { fmg: Fmg::LineHelp, id, en }
}
const fn sm(id: u32, en: &'static str) -> Text {
    Text { fmg: Fmg::SystemMessage, id, en }
}

/// The screens Elden Ring draws itself, reached through its own System menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// Keyboard/Mouse Settings > Controls: Key Bindings (scene 02_160_KeyConfiguration, keyboard).
    KeyBindings,
    /// Controller Settings > Controls: Button Settings (02_160_KeyConfiguration, pad).
    PadButtons,
    /// Network > Data Usage Agreement.
    DataAgreement,
    /// Quit Game > Quit Game (in game only: the game's own save-and-quit, its question first).
    QuitGame,
    /// Quit Game > Return to Desktop.
    Desktop,
}

impl Native {
    /// Where the row is in Elden Ring's System menu on PC: the tab (RB presses from Game Options) and the row (Down
    /// presses from the first).
    pub const fn place(self) -> (u8, u8) {
        match self {
            Native::KeyBindings => (6, 0),
            Native::PadButtons => (5, 0),
            Native::DataAgreement => (4, 3),
            Native::QuitGame => (8, 0),
            Native::Desktop => (8, 1),
        }
    }
}

/// (CAMP-OPTIONS) A KH axis invert's settings-file value (pure): "1" (or any number above 0.5) is Reversed; absent, empty, "0"
/// or anything else is Normal.
pub fn inverted(value: Option<&str>) -> bool {
    value.and_then(|v| v.trim().parse::<f32>().ok()).is_some_and(|v| v > 0.5)
}

/// (CAMP-OPTIONS) The game's own byte a KH axis row also writes: a MENU_OPTION_DATA offset (Camera X / Y-Axis) or a CSPcKeyConfig
/// byte (Mouse X / Y-Axis). Both hold 1 Normal, 0 Reversed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErAxis {
    Md(u16),
    KeyCfg(u16),
}

/// (CAMP-OPTIONS) kh-er.ini's per-axis inverts of the KH camera (1 = Reversed; absent = Normal).
pub const INVERT_X_KEY: &str = "camera.invert_x";
pub const INVERT_Y_KEY: &str = "camera.invert_y";
pub const MOUSE_INVERT_X_KEY: &str = "camera.mouse_invert_x";
pub const MOUSE_INVERT_Y_KEY: &str = "camera.mouse_invert_y";
/// The older single flags (every axis followed the game's bytes once one was set): ignored, dropped from the file.
pub const OLD_AXES_FLAGS: [&str; 2] = ["camera.axes_set", "camera.mouse_axes_set"];

/// Where a row's value lives and how it is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    /// A byte of MENU_OPTION_DATA (load 0x25bee0, set, apply 0x25c780).
    Md(u16),
    /// Graphics > Quality Settings: MENU_OPTION_DATA +0x191 (the apply expands the preset, 0xe945a0).
    Quality,
    /// An Advanced Settings byte: Quality Settings becomes Custom (5) first (lambda 0x9629f0).
    Advanced(u16),
    /// Ray Tracing Quality: +0x19E, and +0x20 (GameSettings enable_ray_tracing) = value != 0.
    RtQuality,
    /// The HDR switch: +0x15 (GameSettings +0x1B), then the game's HDR apply 0x67b150.
    Hdr,
    /// A live byte of CSPcKeyConfig (`at`) and its default (`default_at`).
    KeyCfg { at: u16, default_at: u16 },
    /// CSWindowImp's screen mode (GetScreenMode 0xe913f0 / SetScreenMode 0xe925d0).
    ScreenMode,
    /// CSWindowImp's resolution index for the pending mode (0xe91200 / 0xe92530).
    Resolution,
    /// A KH3 Config row (the KH block, kh-config.txt).
    Kh(ConfigOption),
    /// Opens a sub page.
    Sub(u32),
    /// Opens a screen of Elden Ring's.
    Native(Native),
    /// (THE USER 2026-11-05: "a new graphical option that swaps between rendering the characters in Elden Ring's lighting or
    /// with how they were rendered before") a key of the mod's own settings file (kh-er.ini) and its value when unset; the
    /// renderer reads it live.
    Ini { key: &'static str, unset: u8 },
    /// (BUTTON-ICONS) a key of kh-er.ini holding one of several values (the choice's stored value as a number), and its value
    /// when unset.
    IniValue { key: &'static str, unset: u8 },
    /// (CAMP-OPTIONS, THE USER 2026-11-05: "make sure the default setting for the camera is NOT reversed") AN AXIS INVERT OF THE
    /// KH CAMERA, the mod's own per-axis value in kh-er.ini (`key` = 1: Reversed; absent / 0: Normal, whatever the game's stored
    /// byte says); the row shows and edits it, and also writes the game's own byte (`er`) so its camera matches the row.
    KhAxis { key: &'static str, er: ErAxis },
    /// Holds nothing (the Off-only Ray Tracing Quality of a machine without ray tracing: the game binds it to a dummy).
    Nothing,
}

/// What a row offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// (stored value, its text), in display order.
    Choice(&'static [(u8, Text)]),
    Slider(u8, u8),
    /// A button: the text shown as its value.
    Button(Text),
    /// The game's resolution list (built at run time).
    Resolutions,
    /// KH3's own choices / slider (from kh_core::config::OPTIONS).
    Kh,
}

/// When a row shows / can be changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Always,
    /// Changeable only while HDR is on (the game's enable lambdas 0x962a50 / 0x962f80).
    HdrOn,
    /// Shown when the ray-tracing support object exists (static 0x47f3858, `0x95c46b`).
    RtSupported,
    /// Shown when it does not.
    RtUnsupported,
    /// Shown in game only (a loaded character: the Quit Game tab).
    InGame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub key: u32,
    pub label: Text,
    pub help: Text,
    pub kind: Kind,
    pub store: Store,
    pub gate: Gate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tab {
    pub key: u32,
    pub title: Text,
    pub items: &'static [Item],
}

const ON_OFF: &[(u8, Text)] = &[(1, mt(40300, "On")), (0, mt(40301, "Off"))];
const NORMAL_REVERSED: &[(u8, Text)] = &[(1, mt(40310, "Normal")), (0, mt(40311, "Reversed"))];
const OFF: Text = mt(40441, "Off");
const LOW: Text = mt(40421, "Low");
const MEDIUM: Text = mt(40422, "Medium");
const HIGH: Text = mt(40423, "High");
const MAXIMUM: Text = mt(40424, "Maximum");
const CUSTOM: Text = mt(40420, "Custom");

const fn item(key: u32, label: Text, help: Text, kind: Kind, store: Store) -> Item {
    Item { key, label, help, kind, store, gate: Gate::Always }
}
const fn gated(key: u32, label: Text, help: Text, kind: Kind, store: Store, gate: Gate) -> Item {
    Item { key, label, help, kind, store, gate }
}

// ---- tab and page keys
pub const TAB_GAME: u32 = 1;
pub const TAB_CAMERA: u32 = 2;
pub const TAB_DISPLAY: u32 = 3;
pub const TAB_SOUND: u32 = 4;
pub const TAB_KEYBOARD: u32 = 7;
pub const TAB_GRAPHICS: u32 = 8;
pub const TAB_KH: u32 = 9;
pub const TAB_CONTROLLER: u32 = 6;
/// (CAMP-OPTIONS) the two button tables' pages (kh-host remap.rs).
pub const PAGE_PAD: u32 = 61;
pub const PAGE_KBM: u32 = 71;
pub const TAB_QUIT: u32 = 10;
pub const PAGE_HDR: u32 = 31;
pub const PAGE_ADVANCED: u32 = 81;

// online settings) Elden Ring settings can't change KH gameplay like locking on so remove any that say they do") EVERY ROW
// BELOW CHANGES SOMETHING IN THE MODDED GAME (its evidence beside it); Elden Ring's other rows are in [`REMOVED`] with the
// reason. No new effect was wired into KH gameplay to keep a row.

/// Game Options (builder 0x959df0). Kept: Controller Vibration (GameSettings +1 feeds the game's own pad rumble, reader
/// 0x682104; the mod sends none of its own and leaves XInputSetState alone, so the game's rumble is the one there is).
pub const GAME: &[Item] = &[item(104, sm(110010, "Controller Vibration"), sm(3000, "Set controller vibration strength"), Kind::Slider(0, 10), Store::Md(0x01))];

/// Camera Options (0x959320). Kept: the four rows the KH camera reads (kh-er eroptions.rs `publish` -> erconfig `camera()`
/// -> cam.rs: the stick's flips, its sensitivity, the auto follow), which the game's own camera reads too.
pub const CAMERA: &[Item] = &[
    item(201, mt(110011, "Camera X-Axis"), lh(110002, "Change left/right camera movement"), Kind::Choice(NORMAL_REVERSED), Store::KhAxis { key: INVERT_X_KEY, er: ErAxis::Md(0x0C) }),
    item(202, mt(110010, "Camera Y-Axis"), lh(110001, "Change up/down camera movement"), Kind::Choice(NORMAL_REVERSED), Store::KhAxis { key: INVERT_Y_KEY, er: ErAxis::Md(0x0D) }),
    item(204, mt(110013, "Camera Speed"), lh(110004, "Set camera movement speed"), Kind::Slider(0, 10), Store::Md(0x00)),
    item(205, mt(110118, "Camera Auto Rotation"), lh(110118, "Set automatic camera rotation for left/right movement"), Kind::Choice(ON_OFF), Store::Md(0x0F)),
];

/// Display (0x95d540). "Adjust Brightness" is Elden Ring's brightness screen (0x95d200: one slider, GameSettings +2) as a row
/// of its own here, so it stays in the KH menu.
pub const DISPLAY: &[Item] = &[
    // the game's subtitle display (reader 0x8a2e63) of its cutscenes, which play in the mod
    item(302, mt(110111, "Subtitles"), lh(110102, "Switch in-game subtitles on or off"), Kind::Choice(ON_OFF), Store::Md(0x09)),
    // the parts of Elden Ring's HUD the mod keeps (compass, prompts, banners, boss bars: reader 0x775ff0; Off is honoured,
    // kh-er hud.rs `hud.er.user_off`)
    item(303, mt(110112, "HUD"), lh(110103, "Configure how in-game HUD is displayed"), Kind::Choice(&[(0, mt(40301, "Off")), (1, mt(40300, "On")), (2, mt(40303, "Auto"))]), Store::Md(0x0A)),
    // the game's tutorials (reader 0x7ef401) and the mod's own hold over them (`tutorials.er`, written with the row)
    item(304, mt(111011, "Show Tutorials"), lh(111011, "Set whether tutorials will be shown or not"), Kind::Choice(ON_OFF), Store::Md(0x0B)),
    item(305, mt(110209, "HDR"), lh(110209, "Enable/disable HDR"), Kind::Choice(ON_OFF), Store::Hdr),
    item(306, mt(110210, "Adjust Brightness"), lh(110201, "Adjust screen brightness"), Kind::Slider(0, 10), Store::Md(0x02)),
    gated(307, mt(110211, "Adjust Image Quality"), lh(110202, "Adjust image quality when HDR is enabled"), Kind::Button(mt(110211, "Adjust Image Quality")), Store::Sub(PAGE_HDR), Gate::HdrOn),
    // the glyphs of the game's own prompts (kept by the mod) and of its menus
    item(308, mt(111010, "Device for On-Screen Prompts"), lh(111010, "Set device for on-screen prompts"), Kind::Choice(&[(0, mt(40380, "Game Pad")), (1, mt(40381, "Keyboard and Mouse"))]), Store::KeyCfg { at: 0x4, default_at: 0x0 }),
    // (BUTTON-ICONS) the KH UI's button icons (kh-host button_icons.rs, kh-ui glyphs.rs)
    BUTTON_ICONS_ITEM,
    // the game's Inventory menu (still open from its main menu: readers 0x9008f4 / 0x85d1d0)
    item(309, mt(110601, "Mark New Items"), lh(110601, "Choose whether or not to mark new items with an icon"), Kind::Choice(ON_OFF), Store::Md(0x60)),
    item(310, mt(110603, "Show Recent Items Tab"), lh(110603, "Display/hide the Recent Items tab in the inventory menu"), Kind::Choice(ON_OFF), Store::Md(0x61)),
];

/// (BUTTON-ICONS, THE USER 2026-11-05: "Let the user choose what buttons to display in the settings menu") THE BUTTON ICONS
/// OF THE KH UI AND HUD: Automatic (the device used last, as KH3 PC's own auto setting), Xbox, PlayStation, Keyboard and Mouse;
/// stored in kh-er.ini (`hud.button_icons`, unset = Automatic), applied live (kh-er eroptions.rs `frame`). Texts: KH3's row name
/// (kh3_settings.gamepad.menu_0020 "Button Icons"), Elden Ring's "Keyboard and Mouse" (GR_MenuText 40381, in the game's
/// language); the rest English, as the other mod-only rows (KH3's own choices are "Type 1 / 2 / 3 / Auto", which name no device).
pub const BUTTON_ICONS_ITEM: Item = item(
    311,
    Text { fmg: Fmg::MenuText, id: 0, en: "Button Icons" },
    Text { fmg: Fmg::LineHelp, id: 0, en: "Set which buttons the icons show. Automatic: the device used last" },
    Kind::Choice(&[(0, Text { fmg: Fmg::MenuText, id: 0, en: "Automatic" }), (1, Text { fmg: Fmg::MenuText, id: 0, en: "Xbox" }), (2, Text { fmg: Fmg::MenuText, id: 0, en: "PlayStation" }), (3, mt(40381, "Keyboard and Mouse"))]),
    Store::IniValue { key: crate::button_icons::SETTING_KEY, unset: 0 },
);

/// HDR > Adjust Image Quality (0x95a9c0). Maximum Brightness's choices are made at run time by the game (0x7af630 /
/// 0x94ebe0); its byte runs 0 ..= 10 like the other two (GameSettings +0x1D, default 7).
pub const HDR: &[Item] = &[
    gated(3101, mt(110210, "Adjust Brightness"), lh(110201, "Adjust screen brightness"), Kind::Slider(0, 10), Store::Md(0x16), Gate::HdrOn),
    gated(3102, mt(110212, "Maximum Brightness"), lh(110201, "Adjust screen brightness"), Kind::Slider(0, 10), Store::Md(0x17), Gate::HdrOn),
    gated(3103, mt(110213, "Adjust Saturation"), lh(110201, "Adjust screen brightness"), Kind::Slider(0, 10), Store::Md(0x18), Gate::HdrOn),
];

/// Sound (0x959090): the game's own mix (its music, ambience, voices still play); Master Volume and Sound Effects also
/// scale the KH sounds (kh-er eroptions.rs `sound_gain`).
pub const SOUND: &[Item] = &[
    item(401, mt(110117, "Master Volume"), lh(110117, "Set overall audio volume"), Kind::Slider(0, 10), Store::Md(0x04)),
    item(402, mt(110114, "Music"), lh(110105, "Adjust music level"), Kind::Slider(0, 10), Store::Md(0x05)),
    item(403, mt(110115, "Sound Effects"), lh(110106, "Adjust sound effect level"), Kind::Slider(0, 10), Store::Md(0x06)),
    item(404, mt(110116, "Voice"), lh(110107, "Adjust voice level"), Kind::Slider(0, 10), Store::Md(0x07)),
];

/// (THE USER 2026-11-05: "make button remapping possible on both controller and keyboard, using the settings menu") the
/// controller's button table (remap.rs), on Elden Ring's Controller Settings tab where its own (Tarnished-only) Button Settings
/// stood; KH3's own texts (kh3_settings.gamepad.menu_0030 / detail_0030).
pub const CONTROLLER: &[Item] = &[item(601, Text { fmg: Fmg::MenuText, id: 0, en: "Button Calibration" }, Text { fmg: Fmg::LineHelp, id: 0, en: "Map gamepad buttons." }, Kind::Button(Text { fmg: Fmg::MenuText, id: 0, en: "Button Calibration" }), Store::Sub(PAGE_PAD))];

/// Keyboard/Mouse Settings (0x95cb30). (CAMP-OPTIONS) the keyboard and mouse table (remap.rs; KH3's kh3_settings.keyboard
/// .menu_0020 / detail_0020) where Elden Ring's own Key Bindings stood. Kept: Map Menu Cursor Movement (CSPcKeyConfig +0x883: the game's own map menu, which
/// the save point's Select an Area opens).
/// (CAMP-OPTIONS follow-up, "camera invert must cover mouse look too") Mouse Sensitivity / Mouse X-Axis / Mouse Y-Axis
/// (CSPcKeyConfig +0x880 / +0x881 / +0x882, defaults -8): THE KH CAMERA'S MOUSE LOOK reads them (kh-er eroptions.rs
/// `mouse_look`, cam.rs), as the game's own mouse camera does.
pub const KEYBOARD: &[Item] = &[
    item(701, Text { fmg: Fmg::MenuText, id: 0, en: "Key Configuration" }, Text { fmg: Fmg::LineHelp, id: 0, en: "Map keyboard keys." }, Kind::Button(Text { fmg: Fmg::MenuText, id: 0, en: "Key Configuration" }), Store::Sub(PAGE_KBM)),
    item(702, mt(111020, "Mouse Sensitivity"), lh(111020, "Set mouse sensitivity for camera movement"), Kind::Slider(0, 10), Store::KeyCfg { at: 0x880, default_at: 0x878 }),
    item(703, mt(111022, "Mouse X-Axis"), lh(111022, "Set horizontal camera controls with the mouse"), Kind::Choice(NORMAL_REVERSED), Store::KhAxis { key: MOUSE_INVERT_X_KEY, er: ErAxis::KeyCfg(0x881) }),
    item(704, mt(111023, "Mouse Y-Axis"), lh(111023, "Set vertical camera controls with the mouse"), Kind::Choice(NORMAL_REVERSED), Store::KhAxis { key: MOUSE_INVERT_Y_KEY, er: ErAxis::KeyCfg(0x882) }),
    item(706, mt(111024, "Map Menu Cursor Movement"), lh(111024, "Configure cursor movement on the map menu"), Kind::Choice(&[(0, mt(40382, "Keyboard Only")), (1, mt(40383, "Keyboard and Mouse"))]), Store::KeyCfg { at: 0x883, default_at: 0x87B }),
];

/// ELDEN RING'S ROWS THAT ARE NOT IN THE MENU (the user's rule of 2026-11-05), by their GR_MenuText / GR_System_Message id:
/// (id, the English, why it does not apply in the modded game).
pub const REMOVED: [(u32, &str, &str); 17] = [
    (110014, "Toggle Auto Lock-On", "the KH characters' lock-on is the mod's own (kh-er targets.rs); the game's re-lock reads it only for the hidden Tarnished"),
    (110015, "Auto-Target", "the game's attack auto-aim of the Tarnished's attacks; the KH attacks aim by KH3's rules"),
    (110019, "Manual Attack Aiming", "the Tarnished's large-weapon aiming; the KH characters wield none"),
    (110012, "Reset Camera Y-Axis", "the game's camera reset; the KH camera (cam.kh) has its own and does not read it"),
    (110016, "Camera Auto Wall Recovery", "the game's camera's wall avoidance; the KH camera's own wall handling does not read it"),
    (110113, "Cinematic Effects", "the game's cameras for the Tarnished's special attacks; the KH characters' finishers use KH3's cameras (KINGDOM HEARTS III tab)"),
    (110110, "Display Blood", "the game's blood depiction; the KH characters' blood is the mod's own (kh-er blood.rs) and does not read it; not proven to change a frame of the modded game"),
    (110311, "Cross-Region Play", "online: the mod is played offline"),
    (110315, "Send Summon Sign", "online"),
    (110314, "Launch Setting", "online / offline at launch: the mod is played offline"),
    (110316, "Data Usage Agreement", "online"),
    (110310, "Voice Chat", "online (hidden on PC by the game itself)"),
    (110313, "Display Player Names", "online (hidden on PC by the game itself)"),
    (111001, "Controls: Button Settings", "the game's pad bindings drive the Tarnished's actions; the KH characters read the pad themselves (kh-er pad.rs)"),
    (111006, "Controls: Key Bindings", "the game's key bindings drive the Tarnished's actions; the KH characters' keyboard is the mod's own (inputs.rs KEY_BUTTONS)"),
    (111025, "Target Changing Sensitivity", "the game's mouse lock-on switch; the KH lock-on is the mod's own"),
    (110020, "Performance Settings / Ray Tracing / 103000", "hidden on PC by the game itself (release flags 0x25 / 0x26 / 0x27); Graphics > Ray Tracing Quality is the PC row"),
];

const QUALITIES: &[(u8, Text)] = &[(1, LOW), (2, MEDIUM), (3, HIGH), (4, MAXIMUM), (5, CUSTOM)];
const RT_QUALITIES: &[(u8, Text)] = &[(0, OFF), (1, LOW), (2, MEDIUM), (3, HIGH), (4, MAXIMUM)];

/// (THE USER 2026-11-05) THE STYLE: how the KH characters are drawn. "Elden Ring" = the ER-NATIVE route (the game's light per
/// texel, `r.hybrid_native = 1`); "Kingdom Hearts 3" = the route from before it (`r.hybrid_native = 0`: the look of the
/// user's reference `user_kh3_style_ref.png`, FINAL's comparison shot by `er/native/tools/spot.ps1`). THE USER, later the
/// same day: "Kingdom Hearts 3 rendering should be the default in the settings menu". (HAIR-BLEED, THE USER 2026-11-06: "make
/// Elden Ring's rendering for the KH characters as the default option") unset = 1 (Elden Ring), and the renderer's own fallback
/// (kh-er-render hybrid.rs `Settings::default().native`) says the same; a Style the user saved is kept.
pub const STYLE_KEY: &str = "r.hybrid_native";
pub const STYLE_ITEM: Item = item(
    800,
    Text { fmg: Fmg::MenuText, id: 0, en: "Style" },
    Text { fmg: Fmg::LineHelp, id: 0, en: "Draw the Kingdom Hearts characters in Elden Ring's lighting or as Kingdom Hearts 3 draws them" },
    Kind::Choice(&[(1, Text { fmg: Fmg::MenuText, id: 0, en: "Elden Ring" }), (0, Text { fmg: Fmg::MenuText, id: 0, en: "Kingdom Hearts 3" })]),
    Store::Ini { key: STYLE_KEY, unset: 1 },
);

/// Graphics (0x95c050), with the mod's Style first.
pub const GRAPHICS: &[Item] = &[
    STYLE_ITEM,
    item(801, mt(111110, "Screen Mode"), lh(111110, "Set the screen mode in which to launch the game"), Kind::Choice(&[(1, mt(40400, "Fullscreen")), (0, mt(40401, "Windowed")), (2, mt(40402, "Borderless Windowed"))]), Store::ScreenMode),
    item(802, mt(111111, "Resolution"), lh(111111, "Set the resolution"), Kind::Resolutions, Store::Resolution),
    item(803, mt(111112, "Auto-Detect Best Rendering Settings"), lh(111112, "Set whether to simplify rendering in the case of reduced performance"), Kind::Choice(ON_OFF), Store::Md(0x190)),
    gated(804, mt(111222, "Ray Tracing Quality"), lh(111222, "Select ray tracing quality"), Kind::Choice(RT_QUALITIES), Store::RtQuality, Gate::RtSupported),
    gated(805, mt(111222, "Ray Tracing Quality"), lh(111222, "Select ray tracing quality"), Kind::Choice(&[(0, OFF)]), Store::Nothing, Gate::RtUnsupported),
    item(806, mt(111113, "Quality Settings"), lh(111113, "Set graphics quality"), Kind::Choice(QUALITIES), Store::Quality),
    item(807, mt(111114, "Advanced Settings"), lh(111114, "Set advanced graphics quality"), Kind::Button(mt(111114, "Advanced Settings")), Store::Sub(PAGE_ADVANCED)),
];

/// Graphics > Advanced Settings (0x95b770): each row's choices as its own choice builder lists them.
pub const ADVANCED: &[Item] = &[
    item(8101, mt(111210, "Texture Quality"), lh(111210, "Set texture quality"), Kind::Choice(&[(1, LOW), (2, MEDIUM), (3, HIGH), (4, MAXIMUM)]), Store::Advanced(0x192)),
    item(8102, mt(111211, "Antialiasing Quality"), lh(111211, "Select antialiasing quality"), Kind::Choice(&[(0, OFF), (1, LOW), (3, HIGH)]), Store::Advanced(0x193)),
    item(8103, mt(111212, "SSAO"), lh(111212, "Select SSAO quality"), Kind::Choice(&[(0, OFF), (2, MEDIUM), (3, HIGH), (4, MAXIMUM)]), Store::Advanced(0x194)),
    item(8104, mt(111213, "Depth of Field"), lh(111213, "Select DOF quality"), Kind::Choice(&[(0, OFF), (1, LOW), (2, MEDIUM), (3, HIGH), (4, MAXIMUM)]), Store::Advanced(0x195)),
    item(8105, mt(111214, "Motion Blur"), lh(111214, "Select motion blur quality"), Kind::Choice(&[(0, OFF), (1, LOW), (2, MEDIUM), (3, HIGH)]), Store::Advanced(0x196)),
    item(8106, mt(111215, "Shadow Quality"), lh(111215, "Select shadow quality"), Kind::Choice(&[(1, LOW), (2, MEDIUM), (3, HIGH), (4, MAXIMUM)]), Store::Advanced(0x197)),
    item(8107, mt(111216, "Lighting Quality"), lh(111216, "Select lighting quality"), Kind::Choice(&[(1, LOW), (2, MEDIUM), (3, HIGH), (4, MAXIMUM)]), Store::Advanced(0x198)),
    item(8108, mt(111217, "Effects Quality"), lh(111217, "Select effects quality"), Kind::Choice(&[(1, LOW), (2, MEDIUM), (3, HIGH), (4, MAXIMUM)]), Store::Advanced(0x199)),
    item(8109, mt(111218, "Volumetric Quality"), lh(111218, "Select volumetric quality"), Kind::Choice(&[(1, LOW), (2, MEDIUM), (3, HIGH), (4, MAXIMUM)]), Store::Advanced(0x19D)),
    item(8110, mt(111219, "Reflection Quality"), lh(111219, "Select reflection quality"), Kind::Choice(&[(1, LOW), (3, HIGH), (4, MAXIMUM)]), Store::Advanced(0x19A)),
    item(8111, mt(111220, "Water Surface Quality"), lh(111220, "Select water surface quality"), Kind::Choice(&[(1, LOW), (3, HIGH)]), Store::Advanced(0x19B)),
    item(8112, mt(111221, "Shader Quality"), lh(111221, "Select shader quality"), Kind::Choice(&[(1, LOW), (2, MEDIUM), (3, HIGH)]), Store::Advanced(0x19C)),
    item(8113, mt(111223, "Global Illumination Quality"), lh(111223, "Select global illumination quality"), Kind::Choice(&[(1, LOW), (2, MEDIUM), (3, HIGH)]), Store::Advanced(0x19F)),
    item(8114, mt(111224, "Grass Quality"), lh(111224, "Select grass quality"), Kind::Choice(&[(2, MEDIUM), (3, HIGH), (4, MAXIMUM)]), Store::Advanced(0x1A0)),
];

/// Quit Game (0x959ab0; the tab exists only when the screen is opened in game). Both rows are the game's own: its options
/// screen is opened on them (its question, its save, its quit).
pub const QUIT: &[Item] = &[
    gated(1001, mt(110510, "Quit Game"), lh(110500, "Save the game and return to title menu"), Kind::Button(mt(110510, "Quit Game")), Store::Native(Native::QuitGame), Gate::InGame),
    gated(1002, mt(110511, "Return to Desktop"), lh(110501, "Save the game and return to the desktop"), Kind::Button(mt(110511, "Return to Desktop")), Store::Native(Native::Desktop), Gate::InGame),
];

/// The KH3 Config rows the port reads (STATE.md: every other KH3 row was traced to no reader, or is an Elden Ring row).
pub const KH_ROWS: [ConfigOption; 5] = [ConfigOption::ComboCam, ConfigOption::KeybladeCinematics, ConfigOption::CommandSelection, ConfigOption::CursorPosition, ConfigOption::ControlType];

/// The title of the KH page.
pub const KH_TITLE: &str = "KINGDOM HEARTS III";

/// Elden Ring's tabs, in its order (Quit Game last, in game only), then the KH page (built from [`KH_ROWS`]).
pub const TABS: [Tab; 8] = [
    Tab { key: TAB_GAME, title: mt(110000, "Game Options"), items: GAME },
    Tab { key: TAB_CAMERA, title: mt(110001, "Camera Options"), items: CAMERA },
    Tab { key: TAB_DISPLAY, title: mt(110600, "Display"), items: DISPLAY },
    Tab { key: TAB_SOUND, title: mt(110700, "Sound"), items: SOUND },
    Tab { key: TAB_CONTROLLER, title: sm(110000, "Controller Settings"), items: CONTROLLER },
    Tab { key: TAB_KEYBOARD, title: mt(111005, "Keyboard/Mouse Settings"), items: KEYBOARD },
    Tab { key: TAB_GRAPHICS, title: mt(111100, "Graphics"), items: GRAPHICS },
    Tab { key: TAB_QUIT, title: mt(110500, "Quit Game"), items: QUIT },
];

/// The sub pages.
pub const SUBS: [Tab; 2] = [Tab { key: PAGE_HDR, title: mt(110211, "Adjust Image Quality"), items: HDR }, Tab { key: PAGE_ADVANCED, title: mt(111114, "Advanced Settings"), items: ADVANCED }];

/// "Keep current settings?" (the game's ApplyScreenSettingJob question).
pub const KEEP_QUESTION: Text = Text { fmg: Fmg::Dialogues, id: 111101, en: "Keep current settings?" };
/// "x" between a resolution's width and height (the game's list: "%d" + this + "%d").
pub const RESOLUTION_X: Text = mt(40410, "x");

/// The size of the MENU_OPTION_DATA copy the game's load / apply take (0x1A8 used; 0x1B0 for the ctor's frame).
pub const OPTION_DATA_SIZE: usize = 0x1B0;

/// The copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionData(pub [u8; OPTION_DATA_SIZE]);

impl Default for OptionData {
    fn default() -> OptionData {
        OptionData([0; OPTION_DATA_SIZE])
    }
}

/// THE GAME, as the controller needs it. Every write goes through the game's own code (kh-er eroptions.rs).
pub trait ErGame {
    /// A text in the game's language (`None`: not there; the English is shown).
    fn text(&self, fmg: Fmg, id: u32) -> Option<String>;
    /// `load_menu_option_data(GameSettings, copy)`: the values in force (None: no game data yet).
    fn load(&mut self) -> Option<OptionData>;
    /// `apply_menu_option_data(GameSettings, copy)`.
    fn apply(&mut self, data: &OptionData) -> bool;
    /// The MENU_OPTION_DATA constructor's copy: the game's defaults.
    fn defaults(&mut self) -> Option<OptionData>;
    /// The game's HDR apply request (after the HDR switch).
    fn hdr_changed(&mut self);
    /// HDR in force on the display (the Image Quality rows are changeable).
    fn hdr_active(&self) -> bool;
    /// The ray-tracing support object exists.
    fn rt_supported(&self) -> bool;
    fn key_config(&self, at: u16) -> Option<u8>;
    fn set_key_config(&mut self, at: u16, value: u8) -> bool;
    fn screen_mode(&self) -> Option<u8>;
    fn set_screen_mode(&mut self, mode: u8) -> bool;
    /// The game's resolution list for a screen mode (width, height).
    fn resolutions(&mut self, mode: u8) -> Vec<(i32, i32)>;
    fn resolution(&self, mode: u8) -> Option<u8>;
    fn set_resolution(&mut self, index: u8) -> bool;
    /// Opens one of the game's own screens (the camp menu closes first). False: not now.
    fn open_native(&mut self, screen: Native) -> bool;
    /// The settings reach the disk the way the game's own saves write them (its system data + GraphicsConfig.xml).
    fn request_settings_save(&mut self) -> bool;
    /// A log line.
    fn log(&mut self, line: &str);
    /// A key of the mod's settings file (None: unset).
    fn ini(&self, key: &str) -> Option<String>;
    /// Writes a key into the mod's settings file (the running mod reads the file again: live).
    fn set_ini(&mut self, key: &str, value: &str) -> bool;
    /// A character is loaded (false on the title screen).
    fn in_game(&self) -> bool;
    /// (CAMP-OPTIONS) A button capture begins: the inputs held now do not count (the press that chose the row).
    fn capture_begin(&mut self, _device: crate::remap::Device) {}
    /// The input newly pressed since the capture began: `Some(Some(code))`; `Some(None)` = given up (timeout, Esc);
    /// `None` = nothing yet.
    fn capture_poll(&mut self, _device: crate::remap::Device) -> Option<Option<u16>> {
        Some(None)
    }
    /// A row was written (its key and the stored value): what the mod itself must follow (its own holds over the same
    /// option, kh-er eroptions.rs). Nothing by default.
    fn wrote(&mut self, _key: u32, _value: u8) {}
}

/// What the camp menu does with the controller's answer.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct OptReply {
    /// The tabs again (the values in force now).
    pub tabs: Option<Vec<OptPage>>,
    /// The sub pages again.
    pub subs: Vec<OptPage>,
    /// A sub page to show over the current one.
    pub push: Option<OptPage>,
    /// A question to ask (the camp menu's Yes / No dialog).
    pub ask: Option<OptAsk>,
    /// A KH3 row changed (the HUD applies and saves the KH block).
    pub kh_changed: bool,
    /// The camp menu closes (a screen of the game's opens).
    pub close_menu: bool,
    /// (CAMP-OPTIONS) a button capture begins (true: the screen takes no keys) / ends (false).
    pub capture: Option<bool>,
}

/// The tabbed Config screen's host side, as the HUD calls it.
pub trait OptionsHost {
    /// The screen opens: its pages.
    fn open(&mut self, kh: &mut UserConfig) -> Vec<OptPage>;
    /// One event of the screen.
    fn event(&mut self, event: &OptEvent, kh: &mut UserConfig) -> OptReply;
    /// (CAMP-OPTIONS) Once a frame while the screen is up: an answer of the host's own (a captured button), if any.
    fn tick(&mut self, _kh: &mut UserConfig) -> Option<OptReply> {
        None
    }
}

/// The question key of "Keep current settings?".
pub const ASK_KEEP: u32 = 1;

/// A screen-mode / resolution change waiting for its answer: what to put back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Keep {
    mode: u8,
    resolution: Option<u8>,
}

/// THE CONTROLLER.
pub struct Controller<G: ErGame> {
    pub game: G,
    /// Something was written since the screen opened (the leave asks the settings save).
    changed: bool,
    keep: Option<Keep>,
    /// (CAMP-OPTIONS) the role whose button is being captured, and a swap waiting for the player's Yes.
    capture: Option<(crate::remap::Device, usize)>,
    swap: Option<(crate::remap::Device, usize, u16)>,
}

/// (CAMP-OPTIONS) The question key of a button swap.
pub const ASK_SWAP: u32 = 2;
/// The row value while its button is being captured (KH3's own words, kh3_settings.gamepad.detail_0300).
pub const PRESS_A_BUTTON: &str = "Press a button";

impl<G: ErGame> Controller<G> {
    pub fn new(game: G) -> Controller<G> {
        Controller { game, changed: false, keep: None, capture: None, swap: None }
    }

    /// (CAMP-OPTIONS) A device's table as the settings file holds it.
    pub fn table(&self, device: crate::remap::Device) -> crate::remap::Table {
        crate::remap::Table::parse(device, &self.game.ini(device.ini_key()).unwrap_or_default())
    }

    /// (CAMP-OPTIONS) THE PAGE OF A BUTTON TABLE: a row per role (KH3's name of the default button's function), its value the
    /// input that plays it now.
    pub fn remap_page(&self, device: crate::remap::Device) -> OptPage {
        use crate::remap::{roles, Device};
        // (BUTTON-ICONS) each row's button as its glyph in the chosen set (the button itself, not its role)
        let input_name = crate::button_icons::input_icon;
        let table = self.table(device);
        let (key, title, base) = match device {
            Device::Pad => (PAGE_PAD, "Button Calibration", 5000),
            Device::Kbm => (PAGE_KBM, "Key Configuration", 6000),
        };
        let items = roles(device)
            .iter()
            .enumerate()
            .map(|(k, r)| {
                let capturing = self.capture == Some((device, k));
                let shown = if capturing { PRESS_A_BUTTON.to_string() } else { input_name(device, table.assign[k]) };
                let label = crate::remap::label(r); // (KH3TEXT) KH3's text from the player's pack
                let help = format!("{label}: {} (default {})", if capturing { "press the new one".to_string() } else { shown.clone() }, input_name(device, r.default));
                OptItem { key: base + k as u32, label, help, kind: OptKind::Button(shown), value: 0, enabled: true }
            })
            .collect();
        OptPage { key, title: title.to_string(), items }
    }

    fn save_table(&mut self, table: &crate::remap::Table) {
        let text = table.format();
        self.game.set_ini(table.device.ini_key(), &text);
        self.game.log(&format!("options: the {:?} button table: [{text}]", table.device));
    }

    /// (CAMP-OPTIONS) A captured input for a role: the same / a free one is taken; one another role holds asks the swap.
    fn captured(&mut self, device: crate::remap::Device, role: usize, code: u16, reply: &mut OptReply) {
        use crate::remap::Assign;
        let mut table = self.table(device);
        match table.check(role, code) {
            Assign::Same => {}
            Assign::Free => {
                table.assign(role, code);
                self.save_table(&table);
            }
            Assign::Swap(other) => {
                self.swap = Some((device, role, code));
                reply.ask = Some(OptAsk { key: ASK_SWAP, text: table.swap_question_with(role, code, other, &crate::remap::label, &crate::button_icons::input_icon), cursor: 0 }); // (BUTTON-ICONS) the button's glyph
            }
        }
    }

    fn remap_role(page: u32, item: u32) -> Option<(crate::remap::Device, usize)> {
        use crate::remap::{roles, Device};
        let (device, base) = match page {
            PAGE_PAD => (Device::Pad, 5000),
            PAGE_KBM => (Device::Kbm, 6000),
            _ => return None,
        };
        let k = item.checked_sub(base)? as usize;
        (k < roles(device).len()).then_some((device, k))
    }

    fn text(&self, t: Text) -> String {
        self.game.text(t.fmg, t.id).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| t.en.to_string())
    }

    fn shown(&self, gate: Gate) -> bool {
        match gate {
            Gate::RtSupported => self.game.rt_supported(),
            Gate::RtUnsupported => !self.game.rt_supported(),
            Gate::InGame => self.game.in_game(),
            Gate::Always | Gate::HdrOn => true,
        }
    }

    fn enabled(&self, gate: Gate) -> bool {
        gate != Gate::HdrOn || self.game.hdr_active()
    }

    /// The stored value of a row (None: the game has no value for it now).
    fn stored(&mut self, item: &Item, data: Option<&OptionData>, kh: &UserConfig) -> Option<u8> {
        let md = |off: u16| data.map(|d| d.0[off as usize]);
        match item.store {
            Store::Md(off) | Store::Advanced(off) => md(off),
            Store::Quality => md(0x191),
            Store::RtQuality => md(0x19E),
            Store::Hdr => md(0x15),
            Store::KeyCfg { at, .. } => self.game.key_config(at),
            Store::ScreenMode => self.game.screen_mode(),
            Store::Resolution => self.game.screen_mode().and_then(|m| self.game.resolution(m)),
            Store::Kh(option) => Some(kh.get(option)),
            Store::Ini { key, unset } => Some(self.game.ini(key).and_then(|v| v.trim().parse::<f32>().ok()).map_or(unset, |v| u8::from(v > 0.5))),
            Store::IniValue { key, unset } => Some(self.game.ini(key).and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| (0.0..=255.0).contains(v)).map_or(unset, |v| v.round() as u8)), // (BUTTON-ICONS)
            // (the choice's values are the game's: 1 Normal, 0 Reversed)
            Store::KhAxis { key, .. } => Some(u8::from(!inverted(self.game.ini(key).as_deref()))),
            Store::Sub(_) | Store::Native(_) | Store::Nothing => Some(0),
        }
    }

    fn page_item(&mut self, item: &Item, data: Option<&OptionData>, kh: &UserConfig) -> OptItem {
        let stored = self.stored(item, data, kh);
        let enabled = self.enabled(item.gate) && stored.is_some() && !matches!(item.store, Store::Nothing);
        let (kind, value) = match item.kind {
            Kind::Choice(list) => {
                // Quality Settings lists Custom only while it is Custom (0x86a610)
                let list: Vec<&(u8, Text)> = list.iter().filter(|(v, _)| !(item.store == Store::Quality && *v == 5 && stored != Some(5))).collect();
                let index = stored.and_then(|s| list.iter().position(|(v, _)| *v == s)).unwrap_or(0);
                (OptKind::Choice(list.iter().map(|(_, t)| self.text(*t)).collect()), index as u8)
            }
            Kind::Slider(min, max) => (OptKind::Slider { min, max }, stored.unwrap_or(min).clamp(min, max)),
            Kind::Button(t) => (OptKind::Button(self.text(t)), 0),
            Kind::Resolutions => {
                let x = self.text(RESOLUTION_X);
                let list = self.game.screen_mode().map(|m| self.game.resolutions(m)).unwrap_or_default();
                let names: Vec<String> = list.iter().map(|(w, h)| format!("{w}{x}{h}")).collect();
                let names = if names.is_empty() { vec!["-".to_string()] } else { names };
                let index = stored.unwrap_or(0).min(names.len().saturating_sub(1).min(255) as u8);
                (OptKind::Choice(names), index)
            }
            Kind::Kh => match item.store {
                Store::Kh(option) => {
                    let info = UserConfig::info(option);
                    match info.choices {
                        Choices::List(list) => (OptKind::Choice(list.iter().map(|s| s.to_string()).collect()), stored.unwrap_or(0)),
                        Choices::Slider { min, max } => (OptKind::Slider { min, max }, stored.unwrap_or(min)),
                    }
                }
                _ => (OptKind::Choice(vec!["-".into()]), 0),
            },
        };
        // (a KH3 row's help: KH3's own text, resolved by the view from the option's number; `@` marks it)
        let help = match item.store {
            Store::Kh(option) => format!("@{}", option as u8),
            _ => self.text(item.help),
        };
        OptItem { key: item.key, label: self.text(item.label), help, kind, value, enabled }
    }

    fn page(&mut self, tab: &Tab, data: Option<&OptionData>, kh: &UserConfig) -> OptPage {
        let shown: Vec<&Item> = tab.items.iter().filter(|i| self.shown(i.gate)).collect();
        let items = shown.into_iter().map(|i| self.page_item(i, data, kh)).collect();
        OptPage { key: tab.key, title: self.text(tab.title), items }
    }

    fn kh_page(&mut self, kh: &UserConfig) -> OptPage {
        let items = KH_ROWS
            .iter()
            .map(|option| {
                let info = UserConfig::info(*option);
                let it = Item { key: 900 + *option as u32, label: Text { fmg: Fmg::MenuText, id: 0, en: info.label }, help: Text { fmg: Fmg::LineHelp, id: 0, en: "" }, kind: Kind::Kh, store: Store::Kh(*option), gate: Gate::Always };
                let mut row = self.page_item(&it, None, kh);
                row.label = info.label.to_string();
                row
            })
            .collect();
        OptPage { key: TAB_KH, title: KH_TITLE.to_string(), items }
    }

    /// Every tab with the values in force.
    pub fn tabs(&mut self, kh: &UserConfig) -> Vec<OptPage> {
        let data = self.game.load();
        let mut tabs: Vec<OptPage> = TABS.iter().map(|t| self.page(t, data.as_ref(), kh)).filter(|p| !p.items.is_empty()).collect();
        tabs.push(self.kh_page(kh));
        tabs
    }

    /// Every sub page with the values in force.
    pub fn subs(&mut self, kh: &UserConfig) -> Vec<OptPage> {
        let data = self.game.load();
        let mut subs: Vec<OptPage> = SUBS.iter().map(|t| self.page(t, data.as_ref(), kh)).collect();
        subs.push(self.remap_page(crate::remap::Device::Pad));
        subs.push(self.remap_page(crate::remap::Device::Kbm));
        subs
    }

    fn find(page: u32, key: u32) -> Option<&'static Item> {
        TABS.iter().chain(SUBS.iter()).filter(|t| t.key == page).flat_map(|t| t.items.iter()).find(|i| i.key == key)
    }

    /// The stored value of a row's choice / slider index.
    fn value_of_index(&mut self, item: &Item, index: u8, data: Option<&OptionData>) -> Option<u8> {
        match item.kind {
            Kind::Choice(list) => {
                let current = match item.store {
                    Store::Quality => data.map(|d| d.0[0x191]),
                    _ => None,
                };
                let list: Vec<&(u8, Text)> = list.iter().filter(|(v, _)| !(item.store == Store::Quality && *v == 5 && current != Some(5))).collect();
                list.get(index as usize).map(|(v, _)| *v)
            }
            Kind::Slider(min, max) => Some(index.clamp(min, max)),
            Kind::Resolutions | Kind::Kh => Some(index),
            Kind::Button(_) => None,
        }
    }

    /// WRITES ONE ROW THROUGH THE GAME'S OWN PATH. Returns what the camp menu must do besides refreshing.
    pub fn write(&mut self, item: &Item, index: u8, kh: &mut UserConfig) -> OptReply {
        let mut reply = OptReply::default();
        let data = self.game.load();
        let Some(value) = self.value_of_index(item, index, data.as_ref()) else { return reply };
        let label = item.label.en;
        let through_copy = |c: &mut Controller<G>, edit: &dyn Fn(&mut OptionData)| -> bool {
            let Some(mut d) = c.game.load() else { return false };
            edit(&mut d);
            c.game.apply(&d)
        };
        let done = match item.store {
            Store::Md(off) => through_copy(self, &|d| d.0[off as usize] = value),
            Store::Quality => through_copy(self, &|d| d.0[0x191] = value),
            Store::Advanced(off) => through_copy(self, &|d| {
                d.0[0x191] = 5;
                d.0[off as usize] = value;
            }),
            Store::RtQuality => through_copy(self, &|d| {
                d.0[0x19E] = value;
                d.0[0x20] = u8::from(value != 0);
            }),
            Store::Hdr => {
                let ok = through_copy(self, &|d| d.0[0x15] = value);
                if ok {
                    self.game.hdr_changed();
                }
                ok
            }
            Store::KeyCfg { at, .. } => self.game.set_key_config(at, value),
            Store::ScreenMode => {
                let before = self.game.screen_mode();
                let resolution = before.and_then(|m| self.game.resolution(m));
                let ok = self.game.set_screen_mode(value);
                if let (true, Some(mode)) = (ok, before) {
                    self.keep.get_or_insert(Keep { mode, resolution });
                    reply.ask = Some(OptAsk { key: ASK_KEEP, text: self.text(KEEP_QUESTION), cursor: 0 });
                }
                ok
            }
            Store::Resolution => {
                let before = self.game.screen_mode();
                let resolution = before.and_then(|m| self.game.resolution(m));
                let ok = self.game.set_resolution(value);
                if let (true, Some(mode)) = (ok, before) {
                    self.keep.get_or_insert(Keep { mode, resolution });
                    reply.ask = Some(OptAsk { key: ASK_KEEP, text: self.text(KEEP_QUESTION), cursor: 0 });
                }
                ok
            }
            Store::Kh(option) => {
                kh.set(option, value);
                reply.kh_changed = true;
                true
            }
            Store::Ini { key, .. } | Store::IniValue { key, .. } => self.game.set_ini(key, &value.to_string()), // (BUTTON-ICONS) + IniValue
            Store::KhAxis { key, er } => {
                let ok = self.game.set_ini(key, if value == 0 { "1" } else { "0" });
                // the game's own byte too (its camera, where the game's camera is used, matches the row)
                let _ = match er {
                    ErAxis::Md(off) => through_copy(self, &|d| d.0[off as usize] = value),
                    ErAxis::KeyCfg(at) => self.game.set_key_config(at, value),
                };
                ok
            }
            Store::Sub(_) | Store::Native(_) | Store::Nothing => false,
        };
        self.changed |= done;
        if done {
            self.game.wrote(item.key, value);
        }
        self.game.log(&format!("options: {label} = {value} ({:?}): {}", item.store, if done { "written through the game's own path" } else { "NOT written (the game's object is not there)" }));
        reply
    }

    /// Y + Yes on a page: its rows back to the game's own defaults (MENU_OPTION_DATA ctor 0x86a2f0, CSPcKeyConfig's
    /// default bytes, KH3's block constructor). Screen mode, resolution and the buttons are left as they are.
    pub fn restore_defaults(&mut self, page: u32, kh: &mut UserConfig) -> OptReply {
        let mut reply = OptReply::default();
        if page == TAB_KH {
            let defaults = UserConfig::default();
            for option in KH_ROWS {
                kh.set(option, defaults.get(option));
            }
            reply.kh_changed = true;
            self.game.log("options: the KINGDOM HEARTS page back to KH3's defaults");
            return reply;
        }
        let Some(tab) = TABS.iter().chain(SUBS.iter()).find(|t| t.key == page) else { return reply };
        let (Some(defaults), Some(mut data)) = (self.game.defaults(), self.game.load()) else { return reply };
        let mut copy_changed = false;
        let mut hdr = false;
        for item in tab.items {
            let offsets: &[u16] = match item.store {
                Store::Md(off) | Store::Advanced(off) => &[off][..],
                Store::Quality => &[0x191],
                Store::RtQuality => &[0x19E, 0x20],
                Store::Hdr => &[0x15],
                Store::Ini { key, unset } | Store::IniValue { key, unset } => {
                    // (BUTTON-ICONS) + IniValue
                    self.game.set_ini(key, &unset.to_string());
                    &[]
                }
                // (the user: "the default setting for the camera is NOT reversed") Normal, the KH value and the game's byte
                Store::KhAxis { key, er } => {
                    self.changed |= self.game.set_ini(key, "0");
                    match er {
                        ErAxis::Md(off) => {
                            if data.0[off as usize] != 1 {
                                data.0[off as usize] = 1;
                                copy_changed = true;
                            }
                        }
                        ErAxis::KeyCfg(at) => self.changed |= self.game.set_key_config(at, 1),
                    }
                    &[]
                }
                Store::KeyCfg { at, default_at } => {
                    if let Some(v) = self.game.key_config(default_at) {
                        self.changed |= self.game.set_key_config(at, v);
                    }
                    &[]
                }
                _ => &[],
            };
            for off in offsets {
                let o = *off as usize;
                if data.0[o] != defaults.0[o] {
                    data.0[o] = defaults.0[o];
                    copy_changed = true;
                    hdr |= item.store == Store::Hdr;
                }
            }
        }
        // (the Advanced page's defaults are the default quality's preset: the apply expands it)
        if page == PAGE_ADVANCED && data.0[0x191] != defaults.0[0x191] {
            data.0[0x191] = defaults.0[0x191];
            copy_changed = true;
        }
        if copy_changed && self.game.apply(&data) {
            self.changed = true;
            if hdr {
                self.game.hdr_changed();
            }
        }
        self.game.log(&format!("options: page {page} back to the game's defaults"));
        reply
    }

    fn answer_keep(&mut self, yes: bool) {
        let Some(keep) = self.keep.take() else { return };
        if yes {
            self.game.log("options: the new screen settings are kept");
            return;
        }
        let mode_now = self.game.screen_mode();
        if mode_now != Some(keep.mode) {
            self.game.set_screen_mode(keep.mode);
        }
        if let Some(r) = keep.resolution {
            self.game.set_resolution(r);
        }
        self.game.log(&format!("options: the screen settings are put back (mode {}, resolution {:?})", keep.mode, keep.resolution));
    }

    fn refresh(&mut self, reply: &mut OptReply, kh: &UserConfig) {
        reply.tabs = Some(self.tabs(kh));
        reply.subs = self.subs(kh);
    }
}

impl<G: ErGame> OptionsHost for Controller<G> {
    fn open(&mut self, kh: &mut UserConfig) -> Vec<OptPage> {
        self.changed = false;
        self.keep = None;
        self.capture = None;
        self.swap = None;
        self.tabs(kh)
    }

    fn tick(&mut self, kh: &mut UserConfig) -> Option<OptReply> {
        let (device, role) = self.capture?;
        let got = self.game.capture_poll(device)?;
        self.capture = None;
        let mut reply = OptReply { capture: Some(false), ..OptReply::default() };
        match got {
            Some(code) => self.captured(device, role, code, &mut reply),
            None => self.game.log("options: no button was pressed: the row keeps its button"),
        }
        self.refresh(&mut reply, kh);
        Some(reply)
    }

    fn event(&mut self, event: &OptEvent, kh: &mut UserConfig) -> OptReply {
        let mut reply = match event {
            OptEvent::Changed { page, item, value } => match Self::find(*page, *item) {
                Some(it) => self.write(it, *value, kh),
                None if *page == TAB_KH => match KH_ROWS.iter().find(|o| 900 + **o as u32 == *item) {
                    Some(option) => {
                        let it = Item { key: *item, label: Text { fmg: Fmg::MenuText, id: 0, en: UserConfig::info(*option).label }, help: Text { fmg: Fmg::LineHelp, id: 0, en: "" }, kind: Kind::Kh, store: Store::Kh(*option), gate: Gate::Always };
                        self.write(&it, *value, kh)
                    }
                    None => OptReply::default(),
                },
                None => OptReply::default(),
            },
            OptEvent::Pressed { page, item } => {
                let mut reply = OptReply::default();
                match Self::find(*page, *item).map(|i| i.store) {
                    // (CAMP-OPTIONS) the button tables' pages
                    Some(Store::Sub(PAGE_PAD)) => reply.push = Some(self.remap_page(crate::remap::Device::Pad)),
                    Some(Store::Sub(PAGE_KBM)) => reply.push = Some(self.remap_page(crate::remap::Device::Kbm)),
                    None if Self::remap_role(*page, *item).is_some() => {
                        if let Some((device, role)) = Self::remap_role(*page, *item) {
                            self.capture = Some((device, role));
                            self.game.capture_begin(device);
                            reply.capture = Some(true);
                        }
                    }
                    Some(Store::Sub(sub)) => {
                        let data = self.game.load();
                        if let Some(tab) = SUBS.iter().find(|t| t.key == sub) {
                            reply.push = Some(self.page(tab, data.as_ref(), kh));
                        }
                    }
                    Some(Store::Native(screen)) => {
                        let opened = self.game.open_native(screen);
                        self.game.log(&format!("options: Elden Ring's own screen {screen:?}: {}", if opened { "its System menu is being opened on that row" } else { "NOT opened now" }));
                        reply.close_menu = opened;
                        if opened && self.changed {
                            self.changed = !self.game.request_settings_save();
                        }
                    }
                    _ => {}
                }
                reply
            }
            // (CAMP-OPTIONS) a button table's page: back to its device's defaults (KH3's Xbox layout / KH3's keyboard and mouse)
            OptEvent::Defaults { page } if *page == PAGE_PAD || *page == PAGE_KBM => {
                let device = if *page == PAGE_PAD { crate::remap::Device::Pad } else { crate::remap::Device::Kbm };
                self.save_table(&crate::remap::Table::defaults(device));
                OptReply::default()
            }
            OptEvent::Defaults { page } => self.restore_defaults(*page, kh),
            OptEvent::Answer { ask, yes } => {
                if *ask == ASK_KEEP {
                    self.answer_keep(*yes);
                }
                // (CAMP-OPTIONS) the swap: Yes swaps the two roles, No changes nothing
                if *ask == ASK_SWAP {
                    if let (Some((device, role, code)), true) = (self.swap.take(), *yes) {
                        let mut table = self.table(device);
                        table.assign(role, code);
                        self.save_table(&table);
                    }
                }
                OptReply::default()
            }
            OptEvent::SubLeft { .. } => OptReply::default(),
            OptEvent::Left => {
                self.answer_keep(true);
                if std::mem::take(&mut self.changed) {
                    let asked = self.game.request_settings_save();
                    self.game.log(&format!("options: the screen was left with changes: the settings save {}", if asked { "is asked (the game's system data + GraphicsConfig.xml)" } else { "could NOT be asked" }));
                }
                return OptReply::default();
            }
        };
        if reply.push.is_none() && !reply.close_menu {
            self.refresh(&mut reply, kh);
        }
        reply
    }
}

// ------------------------------------------------------------------------------------------------ the mock game

/// A PRETENDED GAME for the tests and the offline sheets: GameSettings (0x30 bytes), the window's pending screen
/// setting, CSPcKeyConfig's bytes; `load` / `apply` move bytes exactly as the game's 0x25bee0 / 0x25c780 do (the
/// table read from both, re-a NOTES.md), the apply expands the quality preset as 0xe945a0 does.
#[derive(Clone, Debug)]
pub struct MockGame {
    pub settings: [u8; 0x30],
    /// The pending screen setting's dwords: auto-detect, quality, then the 15 advanced values in MENU_OPTION_DATA order.
    pub graphics: [u8; 17],
    pub key_config: Vec<u8>,
    pub mode: u8,
    pub resolution: [u8; 3],
    pub hdr: bool,
    pub rt: bool,
    pub hdr_requests: u32,
    pub saves: u32,
    pub natives: Vec<Native>,
    pub lines: Vec<String>,
    /// The mod's settings file's keys.
    pub ini: std::collections::BTreeMap<String, String>,
    pub in_game: bool,
    /// (CAMP-OPTIONS) the input the next capture poll answers.
    pub capture_next: Option<Option<u16>>,
}

/// MENU_OPTION_DATA byte -> GameSettings byte (0x25bee0 / 0x25c780).
pub const MD_TO_GS: [(u16, u16); 36] = [
    (0x00, 0x00),
    (0x01, 0x01),
    (0x02, 0x02),
    (0x03, 0x03),
    (0x04, 0x20),
    (0x05, 0x04),
    (0x06, 0x05),
    (0x07, 0x06),
    (0x08, 0x07),
    (0x09, 0x08),
    (0x0A, 0x09),
    (0x0B, 0x2E),
    (0x0C, 0x0A),
    (0x0D, 0x0B),
    (0x0E, 0x0C),
    (0x0F, 0x2F),
    (0x10, 0x0D),
    (0x11, 0x0F),
    (0x12, 0x11),
    (0x13, 0x14),
    (0x14, 0x16),
    (0x15, 0x1B),
    (0x16, 0x1C),
    (0x17, 0x1D),
    (0x18, 0x1E),
    (0x19, 0x10),
    (0x1A, 0x17),
    (0x1B, 0x13),
    (0x1C, 0x15),
    (0x1D, 0x18),
    (0x1E, 0x19),
    (0x1F, 0x1A),
    (0x20, 0x21),
    (0x21, 0x1F),
    (0x60, 0x22),
    (0x61, 0x23),
];

/// The quality presets of 0xe945a0 for the 15 advanced bytes (MENU_OPTION_DATA +0x192 .. +0x1A0; +0x19E, ray tracing,
/// is never touched by a preset). Only the shape matters to the tests: a preset overwrites, Custom keeps.
fn preset(quality: u8) -> Option<[u8; 15]> {
    let q = match quality {
        1..=4 => quality,
        _ => return None,
    };
    Some([q; 15])
}

impl Default for MockGame {
    fn default() -> MockGame {
        // the GameSettings ctor 0x25be50 + reset 0x25c120 (re-a NOTES.md 1)
        let mut s = [0u8; 0x30];
        s[0x00] = 5;
        s[0x01] = 10;
        s[0x02] = 5;
        s[0x03] = 2;
        s[0x04] = 7;
        s[0x05] = 7;
        s[0x06] = 7;
        s[0x07] = 1;
        s[0x08] = 1;
        s[0x09] = 2;
        s[0x0A] = 1;
        s[0x0B] = 1;
        s[0x0C] = 1;
        s[0x0D] = 1;
        s[0x11] = 1;
        s[0x17] = 1;
        s[0x19] = 1;
        s[0x1A] = 1;
        s[0x1C] = 5;
        s[0x1D] = 7;
        s[0x1E] = 5;
        s[0x1F] = 1;
        s[0x20] = 10;
        s[0x22] = 1;
        s[0x23] = 1;
        s[0x2E] = 1;
        s[0x2F] = 1;
        let mut g = [3u8; 17];
        g[0] = 1;
        g[1] = 3;
        let mut k = vec![0u8; 0x890];
        k[0x878] = 5;
        k[0x879] = 1;
        k[0x87A] = 1;
        k[0x87C] = 1;
        k[0x880] = 5;
        k[0x881] = 1;
        k[0x882] = 1;
        k[0x884] = 1;
        MockGame { settings: s, graphics: g, key_config: k, mode: 2, resolution: [3, 3, 3], hdr: false, rt: true, hdr_requests: 0, saves: 0, natives: Vec::new(), lines: Vec::new(), ini: std::collections::BTreeMap::new(), in_game: true, capture_next: None }
    }
}

impl ErGame for MockGame {
    fn text(&self, _fmg: Fmg, _id: u32) -> Option<String> {
        None
    }
    fn load(&mut self) -> Option<OptionData> {
        let mut d = OptionData::default();
        for (md, gs) in MD_TO_GS {
            d.0[md as usize] = self.settings[gs as usize];
        }
        d.0[0x190..=0x1A0].copy_from_slice(&self.graphics);
        Some(d)
    }
    fn apply(&mut self, data: &OptionData) -> bool {
        for (md, gs) in MD_TO_GS {
            self.settings[gs as usize] = data.0[md as usize];
        }
        self.graphics.copy_from_slice(&data.0[0x190..=0x1A0]);
        if let Some(p) = preset(self.graphics[1]) {
            for (k, v) in p.iter().enumerate() {
                if k + 2 != 0x19E - 0x190 {
                    self.graphics[k + 2] = *v;
                }
            }
        }
        true
    }
    fn defaults(&mut self) -> Option<OptionData> {
        MockGame::default().load()
    }
    fn hdr_changed(&mut self) {
        self.hdr_requests += 1;
        self.hdr = self.settings[0x1B] != 0;
    }
    fn hdr_active(&self) -> bool {
        self.hdr
    }
    fn rt_supported(&self) -> bool {
        self.rt
    }
    fn key_config(&self, at: u16) -> Option<u8> {
        self.key_config.get(at as usize).copied()
    }
    fn set_key_config(&mut self, at: u16, value: u8) -> bool {
        match self.key_config.get_mut(at as usize) {
            Some(b) => {
                *b = value;
                true
            }
            None => false,
        }
    }
    fn screen_mode(&self) -> Option<u8> {
        Some(self.mode)
    }
    fn set_screen_mode(&mut self, mode: u8) -> bool {
        self.mode = mode.min(2);
        true
    }
    fn resolutions(&mut self, _mode: u8) -> Vec<(i32, i32)> {
        vec![(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)]
    }
    fn resolution(&self, mode: u8) -> Option<u8> {
        self.resolution.get(mode as usize).copied()
    }
    fn set_resolution(&mut self, index: u8) -> bool {
        self.resolution[self.mode.min(2) as usize] = index.min(3);
        true
    }
    fn open_native(&mut self, screen: Native) -> bool {
        self.natives.push(screen);
        true
    }
    fn request_settings_save(&mut self) -> bool {
        self.saves += 1;
        true
    }
    fn log(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }
    fn ini(&self, key: &str) -> Option<String> {
        self.ini.get(key).cloned()
    }
    fn set_ini(&mut self, key: &str, value: &str) -> bool {
        self.ini.insert(key.to_string(), value.to_string());
        true
    }
    fn in_game(&self) -> bool {
        self.in_game
    }
    fn capture_poll(&mut self, _device: crate::remap::Device) -> Option<Option<u16>> {
        self.capture_next.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller() -> Controller<MockGame> {
        Controller::new(MockGame::default())
    }

    fn find_row(tabs: &[OptPage], page: u32, key: u32) -> &OptItem {
        tabs.iter().find(|t| t.key == page).and_then(|t| t.items.iter().find(|i| i.key == key)).unwrap()
    }

    #[test]
    fn co_the_tabs_are_elden_rings_in_its_order() {
        let mut c = controller();
        let mut kh = UserConfig::default();
        let tabs = c.open(&mut kh);
        let titles: Vec<&str> = tabs.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["Game Options", "Camera Options", "Display", "Sound", "Controller Settings", "Keyboard/Mouse Settings", "Graphics", "Quit Game", KH_TITLE]);
        let counts: Vec<usize> = tabs.iter().map(|t| t.items.len()).collect();
        // Graphics: one of the two Ray Tracing Quality rows (this machine has ray tracing)
        assert_eq!(counts, [1, 4, 10, 4, 1, 5, 7, 2, 5]); // (BUTTON-ICONS) Display + Button Icons
        // on the title screen (no character): no Quit Game tab
        let mut title = Controller::new(MockGame { in_game: false, ..MockGame::default() });
        assert_eq!(title.open(&mut kh).len(), 8);
        let advanced = c.subs(&kh);
        assert_eq!(advanced.iter().map(|p| p.items.len()).collect::<Vec<_>>(), [3, 14, 16, 32]);
        // the game's defaults read back as its texts
        let x = find_row(&tabs, TAB_CAMERA, 201);
        assert_eq!((x.label.as_str(), &x.kind, x.value), ("Camera X-Axis", &OptKind::Choice(vec!["Normal".into(), "Reversed".into()]), 0));
        assert_eq!(find_row(&tabs, TAB_CAMERA, 204).value, 5, "camera speed 5");
        assert_eq!(find_row(&tabs, TAB_SOUND, 402).value, 7, "music 7");
        assert_eq!(find_row(&tabs, TAB_DISPLAY, 303).value, 2, "HUD Auto = choice 2");
        assert!(!find_row(&tabs, TAB_DISPLAY, 307).enabled, "HDR's image quality: not while HDR is off");
        let q = find_row(&tabs, TAB_GRAPHICS, 806);
        assert_eq!((&q.kind, q.value), (&OptKind::Choice(vec!["Low".into(), "Medium".into(), "High".into(), "Maximum".into()]), 2), "Custom is not listed while the quality is High");
        let r = find_row(&tabs, TAB_GRAPHICS, 802);
        assert_eq!((&r.kind, r.value), (&OptKind::Choice(vec!["1280x720".into(), "1600x900".into(), "1920x1080".into(), "2560x1440".into()]), 3));
        // a machine without ray tracing shows the Off-only row instead
        let mut c2 = Controller::new(MockGame { rt: false, ..MockGame::default() });
        let t2 = c2.open(&mut kh);
        let rt = find_row(&t2, TAB_GRAPHICS, 805);
        assert!(!rt.enabled && rt.kind == OptKind::Choice(vec!["Off".into()]));
        // the KH page: KH3's own help, by option number
        let kh_tab = tabs.last().unwrap();
        assert_eq!(kh_tab.items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Combo Cam", "Keyblade Cinematics", "Command Selection", "Cursor Position", "Control Type"]);
        assert_eq!(kh_tab.items[0].help, format!("@{}", ConfigOption::ComboCam as u8));
    }

    /// (CAMP-OPTIONS, THE USER 2026-11-05: "make sure the default setting for the camera is NOT reversed") the four axis rows are
    /// the KH camera's own per-axis values: Normal on a fresh file whatever the game's bytes say, one axis at a time, each row
    /// round-trips (and writes the game's byte too), the page defaults give Normal on all four
    #[test]
    fn co_the_camera_axes_are_normal_by_default_and_each_its_own() {
        let mut kh = UserConfig::default();
        // the game's stored bytes Reversed everywhere (Camera X / Y: GameSettings +0xA / +0xB; Mouse X / Y: +0x881 / +0x882)
        let mut game = MockGame::default();
        game.settings[0x0A] = 0;
        game.settings[0x0B] = 0;
        game.key_config[0x881] = 0;
        game.key_config[0x882] = 0;
        // the user's file after the deploy: the older single flags set (ignored)
        game.ini.insert("camera.axes_set".into(), "1".into());
        game.ini.insert("camera.mouse_axes_set".into(), "1".into());
        let mut c = Controller::new(game);
        let tabs = c.open(&mut kh);
        // (choice index 0 = Normal, 1 = Reversed)
        for (page, key) in [(TAB_CAMERA, 201), (TAB_CAMERA, 202), (TAB_KEYBOARD, 703), (TAB_KEYBOARD, 704)] {
            assert_eq!(find_row(&tabs, page, key).value, 0, "row {key} shows Normal on a fresh file");
        }
        // X Reversed: only X; the game's X byte follows the row, Y untouched
        let reply = c.event(&OptEvent::Changed { page: TAB_CAMERA, item: 201, value: 1 }, &mut kh);
        assert_eq!(c.game.ini.get(INVERT_X_KEY).map(String::as_str), Some("1"));
        assert!(!inverted(c.game.ini.get(INVERT_Y_KEY).map(String::as_str)), "Y stays Normal");
        assert_eq!(c.game.settings[0x0A], 0, "the game's own X byte: Reversed as the row");
        let tabs = reply.tabs.expect("the pages again");
        assert_eq!((find_row(&tabs, TAB_CAMERA, 201).value, find_row(&tabs, TAB_CAMERA, 202).value), (1, 0));
        // and back: Normal round-trips (the KH value and the game's byte)
        c.event(&OptEvent::Changed { page: TAB_CAMERA, item: 201, value: 0 }, &mut kh);
        assert_eq!((c.game.ini.get(INVERT_X_KEY).map(String::as_str), c.game.settings[0x0A]), (Some("0"), 1));
        // the mouse rows the same way (CSPcKeyConfig bytes)
        c.event(&OptEvent::Changed { page: TAB_KEYBOARD, item: 704, value: 1 }, &mut kh);
        assert_eq!((c.game.ini.get(MOUSE_INVERT_Y_KEY).map(String::as_str), c.game.key_config[0x882]), (Some("1"), 0));
        assert!(!inverted(c.game.ini.get(MOUSE_INVERT_X_KEY).map(String::as_str)), "mouse X stays Normal");
        // the pages' defaults: Normal on all four, the game's bytes too
        c.event(&OptEvent::Changed { page: TAB_CAMERA, item: 202, value: 1 }, &mut kh);
        c.event(&OptEvent::Defaults { page: TAB_CAMERA }, &mut kh);
        c.event(&OptEvent::Defaults { page: TAB_KEYBOARD }, &mut kh);
        for key in [INVERT_X_KEY, INVERT_Y_KEY, MOUSE_INVERT_X_KEY, MOUSE_INVERT_Y_KEY] {
            assert!(!inverted(c.game.ini.get(key).map(String::as_str)), "{key} Normal after the defaults");
        }
        assert_eq!((c.game.settings[0x0A], c.game.settings[0x0B], c.game.key_config[0x881], c.game.key_config[0x882]), (1, 1, 1, 1));
        let tabs = c.open(&mut kh);
        for (page, key) in [(TAB_CAMERA, 201), (TAB_CAMERA, 202), (TAB_KEYBOARD, 703), (TAB_KEYBOARD, 704)] {
            assert_eq!(find_row(&tabs, page, key).value, 0, "row {key} Normal after the defaults");
        }
        // the settings file's values: "1" Reversed; absent, "0" or junk Normal
        assert!(inverted(Some("1")) && !inverted(None) && !inverted(Some("0")) && !inverted(Some("yes")));
    }

    #[test]
    fn co_values_round_trip_through_the_games_copy() {
        let mut c = controller();
        let mut kh = UserConfig::default();
        c.open(&mut kh);
        // Camera X-Axis -> Reversed: GameSettings +0xA = 0 (1 = Normal, 0 = Reversed: the game's own values)
        c.event(&OptEvent::Changed { page: TAB_CAMERA, item: 201, value: 1 }, &mut kh);
        assert_eq!(c.game.settings[0x0A], 0);
        // Camera Speed 9 -> +0x0
        c.event(&OptEvent::Changed { page: TAB_CAMERA, item: 204, value: 9 }, &mut kh);
        assert_eq!(c.game.settings[0x00], 9);
        // Master Volume (MENU_OPTION_DATA +4) -> GameSettings +0x20
        let reply = c.event(&OptEvent::Changed { page: TAB_SOUND, item: 401, value: 3 }, &mut kh);
        assert_eq!(c.game.settings[0x20], 3);
        assert_eq!(find_row(reply.tabs.as_ref().unwrap(), TAB_SOUND, 401).value, 3, "the answer shows the value in force");
        // Show Recent Items Tab (MENU_OPTION_DATA +0x61) -> +0x23
        c.event(&OptEvent::Changed { page: TAB_DISPLAY, item: 310, value: 1 }, &mut kh);
        assert_eq!(c.game.settings[0x23], 0, "choice 1 = Off");
        // Camera Auto Rotation Off (choice 1) -> +0x2F = 0
        c.event(&OptEvent::Changed { page: TAB_CAMERA, item: 205, value: 1 }, &mut kh);
        assert_eq!(c.game.settings[0x2F], 0);
        // Controller Vibration 4 -> +0x1
        c.event(&OptEvent::Changed { page: TAB_GAME, item: 104, value: 4 }, &mut kh);
        assert_eq!(c.game.settings[0x01], 4);
        // every other GameSettings byte is as it was
        let mut want = MockGame::default().settings;
        want[0x0A] = 0;
        want[0x00] = 9;
        want[0x20] = 3;
        want[0x23] = 0;
        want[0x2F] = 0;
        want[0x01] = 4;
        assert_eq!(c.game.settings, want);
        // HDR on: +0x1B and the game's HDR apply
        c.event(&OptEvent::Changed { page: TAB_DISPLAY, item: 305, value: 0 }, &mut kh);
        assert_eq!((c.game.settings[0x1B], c.game.hdr_requests), (1, 1));
        let tabs = c.tabs(&kh);
        assert!(find_row(&tabs, TAB_DISPLAY, 307).enabled, "HDR on: its image quality page opens");
        // keyboard / mouse: the live byte
        c.event(&OptEvent::Changed { page: TAB_KEYBOARD, item: 706, value: 1 }, &mut kh);
        assert_eq!(c.game.key_config[0x883], 1);
        c.event(&OptEvent::Changed { page: TAB_DISPLAY, item: 308, value: 1 }, &mut kh);
        assert_eq!(c.game.key_config[0x4], 1);
        // leaving with changes asks the settings save once
        c.event(&OptEvent::Left, &mut kh);
        assert_eq!(c.game.saves, 1);
        c.open(&mut kh);
        c.event(&OptEvent::Left, &mut kh);
        assert_eq!(c.game.saves, 1, "nothing changed: no save");
    }

    #[test]
    fn co_graphics_quality_presets_and_custom() {
        let mut c = controller();
        let mut kh = UserConfig::default();
        c.open(&mut kh);
        // Quality Low (choice 0): the apply expands the preset
        c.event(&OptEvent::Changed { page: TAB_GRAPHICS, item: 806, value: 0 }, &mut kh);
        assert_eq!((c.game.graphics[1], c.game.graphics[2]), (1, 1));
        // an Advanced row: Quality becomes Custom, the row keeps its value
        let reply = c.event(&OptEvent::Changed { page: PAGE_ADVANCED, item: 8101, value: 3 }, &mut kh);
        assert_eq!((c.game.graphics[1], c.game.graphics[2]), (5, 4), "Custom; Texture Maximum");
        let q = find_row(reply.tabs.as_ref().unwrap(), TAB_GRAPHICS, 806);
        assert_eq!((q.value, &q.kind), (4, &OptKind::Choice(vec!["Low".into(), "Medium".into(), "High".into(), "Maximum".into(), "Custom".into()])));
        let adv = reply.subs.iter().find(|p| p.key == PAGE_ADVANCED).unwrap();
        assert_eq!(adv.items[0].value, 3);
        // Ray Tracing Quality Off: +0x19E = 0 and GameSettings enable_ray_tracing (+0x21) = 0
        c.game.settings[0x21] = 1;
        c.event(&OptEvent::Changed { page: TAB_GRAPHICS, item: 804, value: 0 }, &mut kh);
        assert_eq!((c.game.graphics[0x19E - 0x190], c.game.settings[0x21]), (0, 0));
        // Advanced defaults: the default quality (High) and its preset
        c.event(&OptEvent::Defaults { page: PAGE_ADVANCED }, &mut kh);
        assert_eq!((c.game.graphics[1], c.game.graphics[2]), (3, 3));
    }

    #[test]
    fn co_screen_mode_asks_to_keep_and_no_puts_it_back() {
        let mut c = controller();
        let mut kh = UserConfig::default();
        c.open(&mut kh);
        // Windowed (choice 1 = value 0)
        let reply = c.event(&OptEvent::Changed { page: TAB_GRAPHICS, item: 801, value: 1 }, &mut kh);
        assert_eq!(c.game.mode, 0);
        assert_eq!(reply.ask.as_ref().map(|a| (a.key, a.text.as_str())), Some((ASK_KEEP, "Keep current settings?")));
        c.event(&OptEvent::Answer { ask: ASK_KEEP, yes: false }, &mut kh);
        assert_eq!(c.game.mode, 2, "No: borderless again");
        // a resolution kept
        c.event(&OptEvent::Changed { page: TAB_GRAPHICS, item: 802, value: 1 }, &mut kh);
        c.event(&OptEvent::Answer { ask: ASK_KEEP, yes: true }, &mut kh);
        assert_eq!(c.game.resolution[2], 1);
    }

    #[test]
    fn co_buttons_open_sub_pages_and_the_games_screens() {
        let mut c = controller();
        let mut kh = UserConfig::default();
        c.open(&mut kh);
        let reply = c.event(&OptEvent::Pressed { page: TAB_GRAPHICS, item: 807 }, &mut kh);
        assert_eq!(reply.push.as_ref().map(|p| (p.key, p.items.len())), Some((PAGE_ADVANCED, 14)));
        let reply = c.event(&OptEvent::Pressed { page: TAB_QUIT, item: 1001 }, &mut kh);
        assert!(reply.close_menu);
        c.event(&OptEvent::Pressed { page: TAB_QUIT, item: 1002 }, &mut kh);
        assert_eq!(c.game.natives, vec![Native::QuitGame, Native::Desktop]);
    }

    #[test]
    fn co_kh_rows_and_defaults() {
        let mut c = controller();
        let mut kh = UserConfig::default();
        c.open(&mut kh);
        let reply = c.event(&OptEvent::Changed { page: TAB_KH, item: 900 + ConfigOption::ControlType as u32, value: 1 }, &mut kh);
        assert!(reply.kh_changed && kh.control_type_b());
        c.event(&OptEvent::Changed { page: TAB_KH, item: 900 + ConfigOption::ComboCam as u32, value: 2 }, &mut kh);
        assert_eq!(kh.get(ConfigOption::ComboCam), 2);
        c.event(&OptEvent::Defaults { page: TAB_KH }, &mut kh);
        assert!(!kh.control_type_b() && kh.get(ConfigOption::ComboCam) == 0);
        // an Elden Ring page's defaults: the game's own (camera speed 5, X-axis Normal)
        c.event(&OptEvent::Changed { page: TAB_CAMERA, item: 204, value: 1 }, &mut kh);
        c.event(&OptEvent::Changed { page: TAB_CAMERA, item: 201, value: 1 }, &mut kh);
        c.event(&OptEvent::Defaults { page: TAB_CAMERA }, &mut kh);
        assert_eq!((c.game.settings[0x00], c.game.settings[0x0A]), (5, 1));
        // keyboard defaults from CSPcKeyConfig's default bytes
        c.game.key_config[0x87B] = 1;
        c.event(&OptEvent::Changed { page: TAB_KEYBOARD, item: 706, value: 0 }, &mut kh);
        c.event(&OptEvent::Defaults { page: TAB_KEYBOARD }, &mut kh);
        assert_eq!(c.game.key_config[0x883], 1);
    }

    /// (THE USER 2026-11-05: "remove any options from our settings menu that don't apply even if they're from Elden Ring")
    /// the menu holds exactly the kept rows: none of [`REMOVED`] (no online row, no lock-on / aim / pad / key / mouse row of
    /// the Tarnished's), the kept list as STATE.md has it
    #[test]
    fn co_the_menu_holds_only_the_rows_that_apply() {
        let mut kh = UserConfig::default();
        for game in [MockGame::default(), MockGame { in_game: false, ..MockGame::default() }, MockGame { hdr: true, rt: false, ..MockGame::default() }] {
            let mut c = Controller::new(game);
            let pages: Vec<OptPage> = c.open(&mut kh).into_iter().chain(c.subs(&kh)).collect();
            let labels: Vec<String> = pages.iter().flat_map(|p| p.items.iter().map(|i| i.label.clone())).collect();
            for (_, en, why) in REMOVED {
                for part in en.split(" / ") {
                    let part = part.trim_start_matches("Controls: ");
                    assert!(!labels.iter().any(|l| l == part), "{part} is shown though it does not apply ({why})");
                }
            }
            assert!(!pages.iter().any(|p| p.title == "Network"), "no online tab");
        }
        let mut c = controller();
        let tabs = c.open(&mut kh);
        let kept: Vec<(String, Vec<String>)> = tabs.iter().map(|t| (t.title.clone(), t.items.iter().map(|i| i.label.clone()).collect())).collect();
        let want: Vec<(&str, Vec<&str>)> = vec![
            ("Game Options", vec!["Controller Vibration"]),
            ("Camera Options", vec!["Camera X-Axis", "Camera Y-Axis", "Camera Speed", "Camera Auto Rotation"]),
            ("Display", vec!["Subtitles", "HUD", "Show Tutorials", "HDR", "Adjust Brightness", "Adjust Image Quality", "Device for On-Screen Prompts", "Button Icons", "Mark New Items", "Show Recent Items Tab"]),
            ("Sound", vec!["Master Volume", "Music", "Sound Effects", "Voice"]),
            ("Controller Settings", vec!["Button Calibration"]),
            ("Keyboard/Mouse Settings", vec!["Key Configuration", "Mouse Sensitivity", "Mouse X-Axis", "Mouse Y-Axis", "Map Menu Cursor Movement"]),
            ("Graphics", vec!["Style", "Screen Mode", "Resolution", "Auto-Detect Best Rendering Settings", "Ray Tracing Quality", "Quality Settings", "Advanced Settings"]),
            ("Quit Game", vec!["Quit Game", "Return to Desktop"]),
            (KH_TITLE, vec!["Combo Cam", "Keyblade Cinematics", "Command Selection", "Cursor Position", "Control Type"]),
        ];
        assert_eq!(kept, want.iter().map(|(t, l)| (t.to_string(), l.iter().map(|s| s.to_string()).collect())).collect::<Vec<_>>());
    }

    /// (CAMP-OPTIONS, the user: "so A is confirm, but you can make confirm B instead"; "they will be swapped in the options menu
    /// after the warning box saying what it will be swapped with is confirmed by the player")
    #[test]
    fn co_confirm_to_b_through_the_menu_asks_then_swaps() {
        use crate::remap::{Device, Table};
        // (KH3TEXT) the rows' names are KH3's texts from the player's pack (as hud.rs sets them when the UI pack loads)
        let from_pack = crate::remap::test_pack_labels().map(crate::remap::set_labels).is_some();
        let (confirm, cancel) = (crate::remap::label(&crate::remap::PAD_ROLES[0]), crate::remap::label(&crate::remap::PAD_ROLES[1]));
        if from_pack {
            assert_eq!(confirm, "Confirm", "the pack's keyconfig_0010");
            assert!(cancel.starts_with("Cancel / Jump"), "the pack's keyconfig_0020: {cancel}");
        }
        let mut c = controller();
        let mut kh = UserConfig::default();
        c.open(&mut kh);
        let reply = c.event(&OptEvent::Pressed { page: TAB_CONTROLLER, item: 601 }, &mut kh);
        let page = reply.push.unwrap();
        assert_eq!((page.key, page.items[0].label.as_str(), &page.items[0].kind), (PAGE_PAD, confirm.as_str(), &OptKind::Button("<icon(Phys_Cross)>".into()))); // (BUTTON-ICONS) the glyph
        // A on "Confirm": the capture begins (the row says so, the screen takes no keys)
        let reply = c.event(&OptEvent::Pressed { page: PAGE_PAD, item: 5000 }, &mut kh);
        assert_eq!(reply.capture, Some(true));
        assert_eq!(reply.subs.iter().find(|p| p.key == PAGE_PAD).unwrap().items[0].kind, OptKind::Button(PRESS_A_BUTTON.into()));
        assert!(c.tick(&mut kh).is_none(), "nothing pressed yet");
        // B pressed: B holds Cancel / Jump: the warning names both
        c.game.capture_next = Some(Some(kh_core::PadButton::FaceRight as u16));
        let reply = c.tick(&mut kh).unwrap();
        assert_eq!(reply.capture, Some(false));
        let ask = reply.ask.unwrap();
        assert_eq!((ask.key, ask.text.as_str()), (ASK_SWAP, format!("<icon(Phys_Circle)> is assigned to {cancel}.\nSwap {confirm} and {cancel}?").as_str()));
        // No: nothing changes
        c.event(&OptEvent::Answer { ask: ASK_SWAP, yes: false }, &mut kh);
        assert_eq!(c.table(Device::Pad), Table::defaults(Device::Pad));
        // again, Yes: swapped and in the settings file
        c.event(&OptEvent::Pressed { page: PAGE_PAD, item: 5000 }, &mut kh);
        c.game.capture_next = Some(Some(kh_core::PadButton::FaceRight as u16));
        c.tick(&mut kh);
        let reply = c.event(&OptEvent::Answer { ask: ASK_SWAP, yes: true }, &mut kh);
        assert_eq!(c.game.ini.get("remap.pad").map(String::as_str), Some("A=0x1 B=0x0"));
        let page = reply.subs.iter().find(|p| p.key == PAGE_PAD).unwrap();
        assert_eq!((&page.items[0].kind, &page.items[1].kind), (&OptKind::Button("<icon(Phys_Circle)>".into()), &OptKind::Button("<icon(Phys_Cross)>".into())));
        // a timeout keeps the row
        c.event(&OptEvent::Pressed { page: PAGE_PAD, item: 5002 }, &mut kh);
        c.game.capture_next = Some(None);
        assert!(c.tick(&mut kh).unwrap().ask.is_none());
        // reset per device
        c.event(&OptEvent::Defaults { page: PAGE_PAD }, &mut kh);
        assert_eq!(c.table(Device::Pad), Table::defaults(Device::Pad));
        // the keyboard: a free key is taken at once, a held one asks
        c.event(&OptEvent::Pressed { page: PAGE_KBM, item: 6000 }, &mut kh);
        c.game.capture_next = Some(Some(b'G' as u16));
        assert!(c.tick(&mut kh).unwrap().ask.is_none());
        assert_eq!(c.table(Device::Kbm).assign[0], b'G' as u16);
        c.event(&OptEvent::Pressed { page: PAGE_KBM, item: 6000 }, &mut kh);
        c.game.capture_next = Some(Some(crate::remap::MOUSE_RIGHT));
        assert!(c.tick(&mut kh).unwrap().ask.unwrap().text.starts_with("<icon(KmKey_RightMouseButton)> Right Click is assigned to Block"));
        c.event(&OptEvent::Defaults { page: PAGE_KBM }, &mut kh);
        assert_eq!(c.table(Device::Kbm), Table::defaults(Device::Kbm));
    }

    #[test]
    // (HAIR-BLEED, the user 2026-11-06) the default Style is Elden Ring
    fn co_style_defaults_to_elden_ring_and_round_trips_through_the_ini() {
        let mut c = controller();
        let mut kh = UserConfig::default();
        let tabs = c.open(&mut kh);
        let style = find_row(&tabs, TAB_GRAPHICS, 800);
        assert_eq!((style.label.as_str(), &style.kind, style.value), ("Style", &OptKind::Choice(vec!["Elden Ring".into(), "Kingdom Hearts 3".into()]), 0), "unset: Elden Ring");
        assert_eq!(tabs.iter().find(|t| t.key == TAB_GRAPHICS).unwrap().items[0].key, 800, "the first row of Graphics");
        // Elden Ring: r.hybrid_native = 1 written; the row reads it back
        let reply = c.event(&OptEvent::Changed { page: TAB_GRAPHICS, item: 800, value: 0 }, &mut kh);
        assert_eq!(c.game.ini.get(STYLE_KEY).map(String::as_str), Some("1"));
        assert_eq!(find_row(reply.tabs.as_ref().unwrap(), TAB_GRAPHICS, 800).value, 0);
        // a saved choice is kept as it is written ("1.0" counts as Elden Ring)
        c.game.ini.insert(STYLE_KEY.into(), "1.0".into());
        assert_eq!(find_row(&c.tabs(&kh), TAB_GRAPHICS, 800).value, 0);
        // Kingdom Hearts 3: 0
        c.event(&OptEvent::Changed { page: TAB_GRAPHICS, item: 800, value: 1 }, &mut kh);
        assert_eq!(c.game.ini.get(STYLE_KEY).map(String::as_str), Some("0"));
        // the Graphics page's defaults: Elden Ring
        c.event(&OptEvent::Defaults { page: TAB_GRAPHICS }, &mut kh);
        assert_eq!(c.game.ini.get(STYLE_KEY).map(String::as_str), Some("1"));
        assert_eq!(find_row(&c.tabs(&kh), TAB_GRAPHICS, 800).value, 0);
    }

    /// (BUTTON-ICONS) The Button Icons row: Automatic unset, each choice through kh-er.ini and back, the Display page's
    /// defaults put it back to Automatic.
    #[test]
    fn bi_button_icons_round_trips_through_the_ini_and_resets() {
        use crate::button_icons::{Setting, SETTING_KEY};
        let mut c = controller();
        let mut kh = UserConfig::default();
        let tabs = c.open(&mut kh);
        let row = find_row(&tabs, TAB_DISPLAY, 311);
        assert_eq!((row.label.as_str(), &row.kind, row.value), ("Button Icons", &OptKind::Choice(vec!["Automatic".into(), "Xbox".into(), "PlayStation".into(), "Keyboard and Mouse".into()]), 0), "unset: Automatic");
        for (index, setting) in [(1u8, Setting::Xbox), (2, Setting::PlayStation), (3, Setting::Kbm), (0, Setting::Automatic)] {
            let reply = c.event(&OptEvent::Changed { page: TAB_DISPLAY, item: 311, value: index }, &mut kh);
            assert_eq!(Setting::parse(c.game.ini.get(SETTING_KEY).map(String::as_str)), setting);
            assert_eq!(find_row(reply.tabs.as_deref().unwrap(), TAB_DISPLAY, 311).value, index, "read back");
        }
        c.event(&OptEvent::Changed { page: TAB_DISPLAY, item: 311, value: 2 }, &mut kh);
        c.event(&OptEvent::Defaults { page: TAB_DISPLAY }, &mut kh);
        assert_eq!(c.game.ini.get(SETTING_KEY).map(String::as_str), Some("0"), "Restore Defaults: Automatic");
        // a hand-edited value out of the list reads as Automatic
        c.game.ini.insert(SETTING_KEY.into(), "9".into());
        assert_eq!(find_row(&c.tabs(&kh), TAB_DISPLAY, 311).value, 0);
    }

    #[test]
    fn co_every_md_offset_is_one_the_game_moves() {
        // every MENU_OPTION_DATA byte a row writes is in the load / apply table or the graphics block
        for tab in TABS.iter().chain(SUBS.iter()) {
            for item in tab.items {
                if let Store::Md(off) | Store::Advanced(off) = item.store {
                    assert!(MD_TO_GS.iter().any(|(md, _)| *md == off) || (0x190..=0x1A0).contains(&off), "{}: {off:#x}", item.label.en);
                }
            }
        }
        // no two rows of the catalogue write the same place
        let mut seen = std::collections::HashSet::new();
        for tab in TABS.iter().chain(SUBS.iter()) {
            for item in tab.items {
                let place = match item.store {
                    Store::Md(o) | Store::Advanced(o) => Some(format!("md{o}")),
                    Store::KeyCfg { at, .. } => Some(format!("kc{at}")),
                    _ => None,
                };
                if let Some(p) = place {
                    assert!(seen.insert(p.clone()), "{} twice", p);
                }
            }
        }
        // keys are unique
        let mut keys = std::collections::HashSet::new();
        for tab in TABS.iter().chain(SUBS.iter()) {
            for item in tab.items {
                assert!(keys.insert(item.key), "key {}", item.key);
            }
        }
        // the KH rows the page keeps all have a reader in the port (kh-er hud.rs / cam.rs / kh-host apply_config)
        assert!(KH_ROWS.iter().all(|o| kh_core::config::OPTIONS[*o as usize - 1].option == *o));
    }
}

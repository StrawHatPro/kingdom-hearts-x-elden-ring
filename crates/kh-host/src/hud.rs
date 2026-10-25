//! THE GAME HUD'S DRIVER, shared by every host (moved from kh-app/src/gfx_hud/drive.rs, 2026-10-12, so that the
//! Bevy test app and the Elden Ring mod run the same code): what feeds kh-ui's movies each drawn frame from the
//! core's `UiSnapshot` (status gauges, popups, the lock-on target, the shotlock aim, prompts), the host's targets
//! ([`HudTargets`]), the command model, the debug words of `hud-cmd`, and the Menu button's flow with the Pause
//! screen and the camp menu (tier 3). The host supplies a [`Frame`] and draws the movies (hud_layers.rs).
//!
//! LOCAL stand-ins (as in the test app): "in battle" look = `hud-cmd battle N`; MP / focus gauges always shown;
//! form id 0; the portrait's damage flash = HP went down between snapshots.

use std::collections::HashMap;
use std::path::Path;

use glam::Mat4;
use kh_core::ui_model::{PopupBoard, PopupDetail};
use kh_core::{Attach, EventKind, SoundHandle, SoundId, SoundPlay, TargetId, UiSnapshot};
use kh_core::config::{Choices, Storage, UserConfig, OPTIONS};
use kh_ui::camp::{AbilityIn, CampAction, CampKey, CampMenu, CampScreen, OptionRow, StatsIn};
use kh_ui::camp_custom::{CommandEntry, CustomizeIn, GROUP_ITEM, GROUP_MAGIC};
use kh_ui::camp_equip::{EquipIn, EquipSlot, InvEntry, Piece, StockEntry, CAT_ACCESSORY, CAT_ARMOUR, CAT_ITEM, CAT_KEY, CAT_MATERIAL, CAT_WEAPON, KIND_ACCESSORY, KIND_ARMOUR, KIND_ITEM, KIND_WEAPON};
use kh_ui::camp_menu::CampMenuView;
use kh_ui::command::{CommandDisplay, CommandStrings};
use kh_ui::menu::{FlowEvent, GateInput, MenuFlow, PauseScreen, Phase, Screen};
use kh_ui::command_debug::DebugCommandSystem;
use kh_ui::hud_command::HudCommand;
use kh_ui::hud_side::{EnemyGauge, EnemyValues, HudLeft, HudRight, SideStrings, SE_ITEM, SE_LEVEL_UP};
use kh_ui::hud_status::{HudStatus, SlotValues, StatusDriver, StatusValues};
use kh_ui::hud_target::{HudTarget, TargetDriver, LOCKON_TYPE_AUTO, LOCKON_TYPE_FLOWSHOTATTACK, LOCKON_TYPE_FLOWSHOTFOCUSCHARGE, LOCKON_TYPE_FLOWSHOTMOVE, LOCKON_TYPE_NORMAL, SFSS_OUT, SFSS_OUT_ATHLETIC_FLOW0, SFSS_OUT_SHOOT_FLOW0};
use kh_ui::{Fonts, Movie, Pack, UiSound};

use kh_core::Sora;

use crate::seam::AudioCommand;

// (party X2) Customize follows the leader (a child module: it reads the Hud's fields)
#[path = "hud_x2.rs"]
mod x2;

// (HUD2) KH3's major item popup "OBTAINED" (key items, Keyblades, new party members): a child module (it reads the Hud's fields)
#[path = "hud_obtained.rs"]
mod obtained;

// (EQ) a friend's camp-menu edits by Sora's rules (item slots' Auto-Reload and refill, the weapon family, the benefit)
#[path = "hud_eq.rs"]
mod eq;
pub use obtained::ObtainedKind;

// (SHOP) the KH shop that stands in for a host's merchants (a child module: it reads the Hud's fields)
#[path = "hud_shop.rs"]
mod shop_ui;
pub use shop_ui::ShopOpen;

/// One target as the HUD needs it (cursor position, the enemy gauge's values).
#[derive(Clone, Copy, Debug)]
pub struct TargetView {
    /// Where its cursor sits: the body's centre, CORE space (cm).
    pub center: kh_core::Vec3,
    pub hp: f32,
    pub hp_max: f32,
    /// A large body (the enemy gauge's wide frame).
    pub large: bool,
}

/// The host's targets by the ids the core's snapshot uses.
pub trait HudTargets {
    fn target(&self, id: TargetId) -> Option<TargetView>;

    /// (HUD2) The HP the enemy gauge starts from when Sora's hit points it at a new target (KH3's `prevHP`, 0x14158A860):
    /// the HP before the hit in the units of [`TargetView::hp`]. The default (a host whose targets take the hit at
    /// once, the core's numbers being the gauge's): `hp_after + damage_dealt`. A host whose gauge values still hold the HP
    /// before the hit at that moment answers `None` (the gauge starts from what it reads).
    fn hit_prev_hp(&self, _id: TargetId, hp_after: f32, damage_dealt: f32) -> Option<i32> {
        Some((hp_after + damage_dealt) as i32)
    }
}

/// No targets (a host without any).
pub struct NoTargets;

impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

/// The camera of the frame.
#[derive(Clone, Copy)]
pub struct View {
    pub clip_from_world: Mat4,
    /// The 3D view rectangle's size in pixels (the stage is stretched over the same rectangle).
    pub size: (f32, f32),
}

pub struct Frame<'a> {
    pub snapshot: &'a UiSnapshot,
    pub sora: &'a Sora,
    /// The events and the state of the latest step, and the seconds of one step.
    pub events: &'a [kh_core::Event],
    pub state: kh_core::StateId,
    pub step_dt: f32,
    /// Play time for the camp menu's clock.
    pub play_seconds: u32,
    pub targets: &'a dyn HudTargets,
    /// CORE space (cm) -> the space `View::clip_from_world` is given in.
    pub to_view: fn(kh_core::Vec3) -> kh_core::Vec3,
    pub view: Option<View>,
    pub words: &'a [String],
    pub dt: f32,
    /// Stage frames (1/60 s) that passed.
    pub ticks: u32,
    /// The pad buttons pressed in the steps of this frame (taken, or skipped by the pause).
    pub pressed: kh_core::PadButtons,
    /// Direction events of those steps (0 up, 1 down, 2 left, 3 right; true = auto-repeat):
    /// D-pad and left stick.
    pub nav: Vec<(u8, bool)>,
}

/// Display overrides of `hud-cmd status ...` (they change what the HUD shows, not his status).
#[derive(Clone, Copy, Debug, Default)]
struct Overrides {
    hp: Option<i32>,
    hp_max: Option<i32>,
    mp: Option<i32>,
    mp_max: Option<i32>,
    focus: Option<i32>,
    charge: Option<f32>,
}

/// LOCAL (not traced; docs/sora-gameover-spec.md 5): stage frames from the start of his Die state to the
/// game-over menu (3 s: the fall, the 2 s camera blend into the game-over scene).
pub const GAMEOVER_MENU_DELAY: u32 = 180;

/// (cutscene) LOCAL: stage frames after Skip Scene in which the Menu button does not open the cutscene pause menu
/// again (the host's skip takes a moment to end the cutscene; the wait also ends when the flag goes false).
pub const CUTSCENE_SKIP_WAIT: u32 = 120;

/// Handles of the UI sounds: far above the core's (which count up from 1).
const UI_SOUND_HANDLES: u32 = 0x7F00_0000;
/// Seam ids of the static SE: this base plus the asset number NNN of `se00000_NNN`.
pub const UI_SOUND_IDS: u32 = 0x5E00_0000;

pub struct Hud {
    status: HudStatus,
    status_driver: StatusDriver,
    command: HudCommand,
    display: CommandDisplay,
    system: DebugCommandSystem,
    strings: CommandStrings,
    target: HudTarget,
    target_driver: TargetDriver,
    left: HudLeft,
    right: HudRight,
    enemy: EnemyGauge,
    side: SideStrings,
    sounds: Vec<UiSound>,
    /// Static SE index -> asset name (`se00000_001` ...).
    se_names: Vec<&'static str>,
    loops: HashMap<u32, SoundHandle>,
    next_handle: u32,
    overrides: Overrides,
    battle: i32,
    /// `None`: LIBRA as his status has it; `Some`: the debug override.
    libra: Option<bool>,
    debug_cursors: Vec<(u32, usize)>,
    last_board: PopupBoard,
    last_step: u64,
    last_style: f32,
    /// The newest situation-command serial seen (for the debug command system's mirror).
    last_serial: u32,
    /// Command kind (row of the command table) -> English name.
    command_names: HashMap<i64, String>,
    started: bool,
    movies: usize,
    flow: MenuFlow,
    pause: PauseScreen,
    camp_view: CampMenuView,
    camp: Option<CampMenu>,
    config: UserConfig,
    status_actions: Vec<StatusAction>,
    /// `m_UIPriority` and the help line of each ability (pack file `data/abilities.json`).
    ability_data: HashMap<i64, (i32, String)>,
    inventory: Inventory,
    /// (loot) Items taken in from `crate::items::give` since the host last asked.
    obtained: Vec<(u32, u32)>,
    config_dirty: bool,
    /// The Config screen applied a change: the host writes the configuration file.
    config_save: bool,
    /// The command menu shows the debug command system (a `hud-cmd` menu word was used)
    /// instead of the core's (`hud-cmd live` goes back).
    debug_commands: bool,
    /// (SP2) A command the host offers in the command menu's event button (KH3's reaction command, the green bar above the
    /// command menu with the Y icon): its command kind (76 = Save at a save point, `kh3_system.situation.save`); `None` = none.
    host_command: Option<i64>,
    /// (RC, the user 2026-10-20: "Make every Y interaction you do in the game use the same HUD element that Save now uses")
    /// EVERY Y INTERACTION THE HOST OFFERS, in Y's order: the first is KH3's reaction command in the event button (the green
    /// bar with the Y icon: what Y does now), the others stack under it as KH3's situation commands (unselected rows). Set
    /// with [`Hud::set_host_reactions`]; `set_host_command` is the one-entry form.
    host_reactions: Vec<HostReaction>,
    /// (integration pass) The shotlock aim as shown (UI spec 7.3, from `UiSnapshot::aim`): the
    /// scope is open, the mark type of this aim (2 shotlock, 3 focus syphon), marks shown per
    /// cursor key, the lock total last sent, the airstep marks on screen.
    aim_open: bool,
    /// (cleanup pass) The shotlock was fired and its state still runs: the shotlock marks stay
    /// on their targets until the controller's mode leaves 2.
    aim_fired: bool,
    /// (fixes pass 1) An airstep left the aim: the scope still owes its second leave state.
    aim_airstep: bool,
    aim_mark_type: usize,
    aim_marks: HashMap<u64, i32>,
    aim_total: i32,
    air_marks: Vec<u64>,
    /// (integration pass) The push-button prompt on screen (`UiSnapshot::prompt`).
    prompt: Option<kh_core::ui_model::ButtonPrompt>,
    play_seconds: u32,
    /// HOST HOOK, called while the camp menu is open after its own frame (the test app: the settings guide's
    /// glyph and the character picture's clip, menu_model.rs).
    pub camp_hook: Option<fn(&mut Movie, &Fonts)>,
    /// (er-host, IMPL_ER15) [`Hud::open_camp_menu`] was called: the camp menu opens in the next frame, whatever the gate says.
    force_camp: u8,
    /// (ERUI) The host runs an event of its own (Elden Ring: a borrowed interaction of the body: a fog gate, a door, a
    /// ladder, a lever ...): KH3's own gate then picks the Pause screen for Start, never the camp menu (`in_event`).
    host_event: bool,
    /// (party P) THE PARTY FORMATION (kh_host::formation): the host's current one, a request to open its screen (the
    /// camp menu's character step), a change made there (taken by the host), whether its screen is up / just closed,
    /// and the characters of the camp menu's panels in panel order (the pictures follow them).
    formation: crate::formation::Formation,
    formation_request: bool,
    formation_change: Option<crate::formation::Formation>,
    formation_open: bool,
    formation_closed: bool,
    /// (ER party S6) the characters the Party Formation screen may not take yet (not unlocked: the host's rule).
    formation_locked: Vec<crate::party::PartyChara>,
    formation_wait: u32,
    camp_panels: Vec<crate::party::PartyChara>,
    /// (party P) THE AI-RUN MEMBERS for Customize's friends list (the user's rule: an entry for every ACTIVE member the
    /// AI runs, Sora too while a friend leads; the leader has none), party order, with their settings. `None` = the
    /// host does not say (the party's friends, as before).
    ai_members: Option<Vec<(crate::party::PartyChara, crate::party::FriendAiSettings)>>,
    /// HOST OVERRIDE (Elden Ring; user decision "the config button just opens Elden Ring's settings and save
    /// menu"): the top screen's Config entry closes the camp menu and raises [`Hud::take_system_menu`] instead
    /// of opening the KH3 Config screen. Off in the test app.
    pub config_external: bool,
    /// (er-host, IMPL_ER21) The camp menu gets "Title Screen" as its last entry (after Config); chosen and confirmed,
    /// [`Hud::take_title_request`] answers true once. The save-point menu's Title Screen asks KH3's question first.
    pub title_entry: bool,
    title_request: bool,
    /// (er-host, IMPL_ER59) Sora's outfits in the camp menu: (id, shown name) and the one worn; `None` = no such entry.
    /// A choice is handed to the host once by [`Hud::take_outfit_choice`].
    pub outfit_menu: Option<(Vec<(String, String)>, String)>,
    outfit_choice: Option<String>,
    system_menu: bool,
    /// THE GAME-OVER MENU (docs/sora-gameover-spec.md): the host's switch; the screen; stage frames since his
    /// Die state began; the button chosen (for the host); the choice was made and he is still dead.
    pub gameover_on: bool,
    /// Buttons the host cannot carry out (shown disabled).
    pub gameover_disabled: Vec<kh_ui::gameover::GameOverButton>,
    gameover: kh_ui::gameover::GameOverScreen,
    dead_frames: u32,
    gameover_choice: Option<kh_ui::gameover::GameOverButton>,
    gameover_done: bool,
    /// (game over) THE GAME-OVER SEQUENCE a host opened with [`Hud::open_gameover`] (crate::gameover): the plate,
    /// the void scene, the menu of the chosen variant; its result, once; the loop SE that plays.
    gameover_seq: Option<crate::gameover::Sequence>,
    gameover_result: Option<kh_ui::gameover::GameOverResult>,
    gameover_loop: Option<u32>,
    /// (game over) The game's hint texts by id (`kh3_system.gameoverhint.<id>`: general_0010 ..., enemy_3010 ...).
    gameover_hints: HashMap<String, String>,
    /// (savepoint) THE SAVE-POINT MENU (docs/sora-savepoint-spec.md 5): the screen (it holds the engine pause while
    /// it is up) and what it told the host last (a command chosen, or closed).
    savepoint: kh_ui::savepoint::SavePointMenu,
    savepoint_result: Option<kh_ui::savepoint::SavePointResult>,
    /// (cutscene) THE CUTSCENE RULE AND ITS PAUSE MENU (build/sora_tmp/er/IMPL_CUTSCENE_MENU.md): the host's
    /// flag of this frame ([`Hud::set_cutscene`]); the menu (Continue / Skip Scene; it holds the pause while it
    /// is up); what it told the host; stage frames left in which Start does not open it again after Skip Scene.
    cutscene: bool,
    /// (cutscene) False: this scene cannot be skipped (Skip Scene is shown disabled and refuses).
    pub cutscene_skip_allowed: bool,
    cutscene_pause: kh_ui::cutscene_pause::CutscenePauseMenu,
    /// (HUD2) KH3's OBTAINED popup (None: the pack has no KeyItem movie), the popups waiting for a free moment, KH3's
    /// names of the party members.
    key_item: Option<kh_ui::key_item::KeyItemPopup>,
    /// (HUD2) The OBTAINED popup is switched on (the host's key `hud.obtained`; OFF by default until it is tested in the
    /// game: off, `show_obtained` refuses and every caller keeps its old notification).
    pub obtained_on: bool,
    obtained_wait: std::collections::VecDeque<kh_ui::key_item::Obtained>,
    chara_names: Vec<(crate::party::PartyChara, String)>,
    cutscene_pause_result: Option<kh_ui::cutscene_pause::CutscenePauseResult>,
    cutscene_skip_wait: u32,
    /// (new game) THE NEW-GAME CHOICE MENU (kh_ui::newgame; build/sora_tmp/IMPL_NEWGAME_STATE.md): the two
    /// questions of the opening; it holds the pause while it is up; its result, once.
    /// `None`: the UI pack has no CutsceneText movie (a pack older than 2026-10-12): the menu cannot open.
    newgame: Option<kh_ui::newgame::NewGameChoice>,
    newgame_result: Option<kh_core::status::OpeningChoice>,
    /// (ER party S8) THE TALK WINDOW (kh_ui::talk): a speaker's lines, a prompt over a character, a banner; drawn on the
    /// "NewGameDialog" layer while the game runs (free then). None: the UI pack cannot show it.
    talk: Option<kh_ui::talk::TalkWindow>,
    /// (CV1) THE SPEECH BUBBLE of the conversation system (kh_ui::bubble; build/sora_tmp/er/IMPL_CV1_STATE.md), on its own
    /// layer "SpeechBubble" (the last of hud_layers::LAYERS); driven through `crate::conv::ConvView`. None: the UI pack
    /// cannot show it.
    bubble: Option<kh_ui::bubble::SpeechBubble>,
    /// (CV1) `hud-cmd conv demo`: a demonstration conversation in the test level / the HUD harness (Sora and the
    /// training dummy as the two speakers; the bubble only, no camera).
    /// `faces=X,Y,R/X,Y,R[/X,Y,R...]`: Sora's and Donald's faces on the stage given (a picture of another camera behind the
    /// HUD), then bystanders' (kept off).
    conv_demo: Option<(crate::conv::Conversation, Option<Vec<kh_ui::bubble::Face>>)>,
    /// (CV3) The bubble takes the D-pad only for its options (not the left stick: the player walks with it during a
    /// conversation; the Elden Ring host sets it, `bubble_dpad_only`).
    bubble_dpad_only: bool,
    /// (saveload) THE SAVE AND LOAD SCREENS (kh_ui::saveload; build/sora_tmp/er/IMPL_SAVELOAD_UI.md): the slot lists of
    /// the movie SaveLoad over summaries the host supplies; they hold the pause while up; the result, once.
    /// `None`: the UI pack cannot show them. `saveload_return`: where a cancel leads (the save-point menu /
    /// the game-over menu the screen was opened from); the two `*_armed` flags remember that the menu's entry
    /// was just chosen.
    saveload: Option<kh_ui::saveload::SaveLoadScreen>,
    saveload_result: Option<kh_ui::saveload::SaveLoadResult>,
    saveload_return: SaveLoadReturn,
    save_armed: bool,
    load_armed: bool,
    /// (SV) the save-point menu's Load was chosen: a Load screen opened next leads back to that menu
    point_load_armed: bool,
    /// (saveload) The screens draw the movie's own dark stage background (a host with nothing to show behind
    /// them: the title). Off: the host's picture stays visible behind the list.
    pub saveload_background: bool,
    /// Where the camp lines go (`camp: ...`): the test app prints them; a host may take them for its log.
    pub log: fn(&str),
    /// (party, stream F) THE FRIENDS IN THE STATUS HUD ([`Hud::set_party`]): slots 1 - 2; empty = Sora alone.
    party: Vec<crate::party::PartyHudSlot>,
    /// (PF2) Each seated character's HP last frame (his portrait's damage flash when it went down), BY CHARACTER: the
    /// flash goes to the slot he sits in now ([`Hud::status_seats`]), never to a fixed Sora / Donald / Goofy index.
    hp_seen: Vec<(crate::party::PartyChara, i32)>,
    /// (PF2) THE STATUS HUD'S SEATS: who sits in each slot (0 = the leader, then the others in formation order), the one
    /// layout the faces, the gauges and the damage flashes all follow. Frozen during the leader switch's shrink.
    seats: [Option<crate::party::PartyChara>; kh_ui::hud_status::STATUS_INFO_MAX],
    /// (party) The friends as the camp menu shows them ([`Hud::set_party_menu`]), party order.
    party_menu: Vec<crate::party::PartyMenuMember>,
    /// (party) What the camp menu changed for a friend ([`Hud::take_party_edits`]).
    party_edits: Vec<crate::party::PartyMenuEdit>,
    /// (party) The menu's actions for a friend (party slot, action), applied with Sora's in `apply_actions`.
    party_pending: Vec<(usize, StatusAction)>,
    /// (party) The friends' weapons as the Equipment screen shows them ([`Hud::set_party_weapons`]).
    party_weapons: Vec<Piece>,
    /// (party) A new snapshot came: the open camp screen is supplied again.
    party_menu_dirty: bool,
    /// (party P) THE LEADER in the main status slot (the character the pad controls) and whether Sora is in the
    /// active party; the face movies by character (0 Sora's PC face, 1 Donald, 2 Goofy) and Sora's NPC face for a
    /// friend slot (FaceNpcExSora, when the UI pack has it).
    leader: crate::party::PartyChara,
    sora_in: bool,
    chara_faces: Vec<(crate::party::PartyChara, Option<std::sync::Arc<kh_ui::def::MovieDef>>)>, // (PF2) by roster row
    sora_npc_face: Option<std::sync::Arc<kh_ui::def::MovieDef>>,
    /// (party P) the leader switch in progress: (new leader, Sora in the party, stage frames of the shrink left)
    leader_anim: Option<(crate::party::PartyChara, bool, u32)>,
    /// (GOOFY-GG) Goofy's GUARD bar above MP (kh_ui::hud_guard) and this frame's values + look from the host.
    guard_bar: kh_ui::hud_guard::GuardBar,
    guard: Option<(kh_ui::hud_guard::GuardValues, kh_ui::hud_guard::GuardLook)>,
    /// (DON-UI) Donald's HUD pieces (kh_ui::hud_resonance / hud_arcane): this frame's view from the host, the look, the
    /// art (KH3's IconText, the style gauge's chevrons) and the three drivers.
    don: Option<crate::party::DonHudView>,
    don_look: crate::party::DonLook,
    don_art: kh_ui::hud_resonance::DonArt,
    don_ring: kh_ui::hud_resonance::ResonanceRing,
    don_stars: kh_ui::hud_arcane::ArcaneStars,
    /// (DON-UI) The Y rows' star cost.
    don_ycost: kh_ui::hud_arcane::YCostStars,
    don_icons: kh_ui::hud_arcane::TargetIcons,
    don_glow: Option<usize>,
    /// The leader slot's focus gauge is hidden for Donald (he has no shotlock: the user, 2026-10-22).
    don_ex_hidden: bool,
    /// The Donald list the command HUD shows (`DonHudView::list`) and whether a re-open is under way.
    don_list: std::sync::atomic::AtomicU8,
    /// (SHOP) the KH shop (hud_shop.rs): its screen, the trades of the frame, its log.
    shop: shop_ui::ShopState,
    /// (party) The friends' level-ups of this frame (level, chara, lines), shown right after Sora's: the cockpit's
    /// array of windows (HudRight +0x1D8; `0x141589330` takes the first free one or updates the one already showing
    /// that character).
    party_levels: std::collections::VecDeque<(i32, usize, Vec<String>)>,
    /// (BN) Each roster character's BONUS! / LEVEL UP! window look (colour, MenuFace portrait), from the roster's rows
    /// (`formation::ROSTER`) and the save-load pack's faces; and the friends' bonus panels of this frame.
    panel_charas: Vec<(crate::party::PartyChara, kh_ui::hud_side::PanelChara)>,
    party_bonuses: std::collections::VecDeque<(crate::party::PartyChara, Vec<String>)>,
}

/// (BN) KH3'S ICON FOR A REWARD LINE (0x141596150 with 0x14157D0E0 / 0x14157D620 / 0x1415B2130): an ability (kind 7)
/// the sparkle; a command (kind 8) its category's icon, and every command a reward line carries is a magic command (a
/// new spell or tier: `Reward::Magic`), so KH3's magic icon; the stat and slot kinds (max HP / MP / AP, armour /
/// accessory / item slot), a shotlock and a host's own text: none.
pub fn bonus_line_icon(line: kh_core::status::BonusLine) -> kh_ui::hud_side::LineIcon {
    use kh_core::status::BonusLine;
    use kh_ui::hud_side::LineIcon;
    match line {
        BonusLine::Ability(_) => LineIcon::Ability,
        BonusLine::Magic(_) => LineIcon::Magic,
        _ => LineIcon::None,
    }
}

/// (saveload) Where a cancelled Save / Load screen leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SaveLoadReturn {
    Nowhere,
    SavePoint,
    GameOver,
    /// (SV) the Load screen out of the save-point menu's Load: its cancel brings that menu back, the cursor on Load
    SavePointLoad,
}

fn print_line(line: &str) {
    println!("{line}");
}

/// Cursor keys of level points (airstep marks): this bit plus the point's id; a target's key is
/// its id.
const POINT_KEY: u64 = 1 << 40;

fn mark_key(mark: &kh_core::ui_model::AimMark) -> u64 {
    if mark.target != TargetId::NONE { mark.target.0 as u64 } else { POINT_KEY | mark.point as u64 }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// (party P) Stage frames of the leader switch's shrink (the slot clips' `out` labels: McSttsPc_18 / McSttsNpc_1 [A];
/// LOCAL: about the length of their out frames).
const LEADER_OUT_FRAMES: u32 = 12;

/// (party P) Stage frames between the Party Formation screen's open and its character step (the root `in` of the camp
/// movie plays first; LOCAL: by eye, the camp menu's own open takes about this long).
const FORMATION_OPEN_FRAMES: u32 = 50;

/// (party P) Customize's entry numbers of the AI-run members: this + the PartyChara value (0 Sora, 1 Donald, 2 Goofy).
const AI_MEMBER_BASE: usize = 100;

impl Hud {
    pub fn load(dir: &Path) -> Result<(Hud, Fonts), String> {
        let mut pack = Pack::open(dir).map_err(|e| e.to_string())?;
        let fonts = Fonts::load(&mut pack).map_err(|e| e.to_string())?;
        let mut movie = |name: &str| pack.movie(name).map_err(|e| e.to_string());
        let (status, command, target, left, right, face) = (movie("HudStatus")?, movie("HudCommand")?, movie("HudTarget")?, movie("HudLeft")?, movie("HudRight")?, movie("FacePcExSora01")?);
        let pause_movie = pack.movie("Pause").map_err(|e| e.to_string())?;
        let camp_view = CampMenuView::load(&mut pack, fonts.clone()).map_err(|e| e.to_string())?;
        let gameover = kh_ui::gameover::GameOverScreen::load(&mut pack, fonts.clone()).map_err(|e| e.to_string())?;
        let savepoint = kh_ui::savepoint::SavePointMenu::load(&mut pack, fonts.clone()).map_err(|e| e.to_string())?; // (savepoint)
        let cutscene_pause = kh_ui::cutscene_pause::CutscenePauseMenu::load(&mut pack, fonts.clone()).map_err(|e| e.to_string())?; // (cutscene)
        let key_item = obtained::load(&mut pack, &fonts); // (HUD2)
        let chara_names = obtained::chara_names(&mut pack); // (HUD2)
        // (new game) an older UI pack (without the CutsceneText movie) keeps its HUD: only this menu is missing
        // (ER party S8) the talk window (its own copy of the Dialog movie)
        let talk = kh_ui::talk::TalkWindow::load(&mut pack, fonts.clone()).map_err(|e| println!("hud: the talk window is NOT available ({e})")).ok();
        // (CV1) the speech bubble (the Dialog movie's text field, the Message / Dialog art)
        let bubble = kh_ui::bubble::SpeechBubble::load(&mut pack, fonts.clone()).map_err(|e| println!("hud: the speech bubble is NOT available ({e})")).ok();
        let newgame = match kh_ui::newgame::NewGameChoice::load(&mut pack, fonts.clone()) {
            Ok(menu) => Some(menu),
            Err(error) => {
                println!("hud: the new-game choice menu is NOT available ({error}): rebuild the UI pack (node tools/sora_ui_pack.js) and copy it to where the host reads it");
                None
            }
        };
        // (saveload) a UI pack without the SaveLoad movie keeps its HUD: only these screens are missing
        let saveload = match kh_ui::saveload::SaveLoadScreen::load(&mut pack, fonts.clone()) {
            Ok(screen) => Some(screen),
            Err(error) => {
                println!("hud: the Save / Load screens are NOT available ({error})");
                None
            }
        };
        let side = SideStrings::load(&mut pack);
        // (game over) the hint texts of the game-over menu's panel
        let gameover_hints: HashMap<String, String> = pack.string_table("kh3_system").into_iter().filter_map(|(key, text)| key.strip_prefix("kh3_system.gameoverhint.").map(|id| (id.to_string(), text))).collect();
        let strings = CommandStrings::load(&mut pack);
        let mut status_driver = StatusDriver::default();
        status_driver.faces[0] = Some(face);
        let mut se_names = Vec::new();
        if let Ok(sounds) = pack.data("ui_sounds") {
            for row in sounds["static_se"].as_array().into_iter().flatten() {
                let name = row["sound"].as_str().unwrap_or("").rsplit(['/', '.']).next().unwrap_or("").to_string();
                se_names.push(leak(name));
            }
        }
        let mut command_names = HashMap::new();
        if let Ok(commands) = pack.data("commands") {
            for row in commands["rows"].as_array().into_iter().flatten() {
                if let (Some(id), Some(name)) = (row["id"].as_i64(), row["name_en"].as_str()) {
                    command_names.insert(id, name.to_string());
                }
            }
        }
        let mut ability_data = HashMap::new();
        if let Ok(abilities) = pack.data("abilities") {
            for row in abilities["rows"].as_array().into_iter().flatten() {
                if let (Some(value), Some(priority)) = (row["value"].as_i64(), row["ui_priority"].as_i64()) {
                    ability_data.insert(value, (priority as i32, row["help"].as_str().unwrap_or("").to_string()));
                }
            }
        }
        let mut hud = Hud {
            status: HudStatus::new(status),
            status_driver,
            command: HudCommand::new(command, fonts.clone()),
            display: CommandDisplay::new(),
            system: DebugCommandSystem::new(Some(&mut pack)),
            strings,
            target: HudTarget::new(target, fonts.clone()),
            target_driver: TargetDriver::default(),
            left: {
                // (loot) the item pictures of the "obtained" popup (pack file data/item_icons.json; a pack without it: no pictures)
                let mut left = HudLeft::new(left, fonts.clone(), &side);
                if let Ok(icons) = pack.data("item_icons") {
                    for (key, file) in icons["icons"].as_object().into_iter().flatten() {
                        if let Some(texture) = file.as_str().and_then(|f| pack.texture(f).ok()) {
                            left.item_icons.insert(key.clone(), texture);
                        }
                    }
                }
                left
            },
            right: HudRight::new(right, fonts.clone(), &side),
            enemy: EnemyGauge::default(),
            side,
            sounds: Vec::new(),
            se_names,
            loops: HashMap::new(),
            next_handle: UI_SOUND_HANDLES,
            overrides: Overrides::default(),
            battle: 0,
            libra: None,
            debug_cursors: Vec::new(),
            last_board: PopupBoard::default(),
            last_step: 0,
            last_style: 0.0,
            last_serial: 0,
            command_names,
            started: false,
            movies: 8,
            flow: MenuFlow::default(),
            pause: PauseScreen::new(pause_movie, fonts.clone()),
            camp_view,
            camp: None,
            config: UserConfig::default(),
            status_actions: Vec::new(),
            ability_data,
            inventory: Inventory::default(),
            obtained: Vec::new(),
            // The configuration's readers get the start values once.
            config_dirty: true,
            config_save: false,
            debug_commands: false,
            host_command: None, // (SP2)
            host_reactions: Vec::new(), // (RC)
            aim_open: false,
            aim_fired: false,
            aim_airstep: false,
            aim_mark_type: LOCKON_TYPE_FLOWSHOTATTACK,
            aim_marks: HashMap::new(),
            aim_total: 0,
            air_marks: Vec::new(),
            prompt: None,
            play_seconds: 0,
            camp_hook: None,
            force_camp: 0,
            host_event: false, // (ERUI)
            formation: crate::formation::Formation::default(),
            formation_request: false,
            formation_change: None,
            formation_open: false,
            formation_closed: false,
            formation_locked: Vec::new(), // (ER party S6)
            formation_wait: 0,
            camp_panels: Vec::new(),
            ai_members: None,
            config_external: false,
            title_entry: false,
            title_request: false,
            outfit_menu: None,
            outfit_choice: None,
            system_menu: false,
            gameover_on: false,
            gameover_disabled: Vec::new(),
            gameover,
            dead_frames: 0,
            gameover_choice: None,
            gameover_done: false,
            // (game over)
            gameover_seq: None,
            gameover_result: None,
            gameover_loop: None,
            gameover_hints,
            savepoint, // (savepoint)
            savepoint_result: None,
            // (cutscene)
            cutscene: false,
            cutscene_skip_allowed: true,
            cutscene_pause,
            key_item, // (HUD2)
            obtained_on: false,
            obtained_wait: Default::default(),
            chara_names,
            cutscene_pause_result: None,
            cutscene_skip_wait: 0,
            // (new game)
            newgame,
            newgame_result: None,
            talk, // (ER party S8)
            bubble, // (CV1)
            conv_demo: None, // (CV1)
            bubble_dpad_only: false, // (CV3)
            // (saveload)
            saveload,
            saveload_result: None,
            saveload_return: SaveLoadReturn::Nowhere,
            save_armed: false,
            load_armed: false,
            point_load_armed: false, // (SV)
            saveload_background: false,
            log: print_line,
            party: Vec::new(),
            hp_seen: Vec::new(),                                                  // (PF2)
            seats: [Some(crate::party::PartyChara::Sora), None, None, None, None], // (PF2) Sora alone until a party comes
            party_menu: Vec::new(),
            party_edits: Vec::new(),
            party_pending: Vec::new(),
            party_weapons: Vec::new(),
            party_menu_dirty: false,
            party_levels: std::collections::VecDeque::new(),
            panel_charas: Vec::new(),                       // (BN)
            party_bonuses: std::collections::VecDeque::new(), // (BN)
            leader: crate::party::PartyChara::Sora,
            sora_in: true,
            chara_faces: Vec::new(), // (PF2)
            sora_npc_face: None,
            leader_anim: None,
            guard_bar: Default::default(), // (GOOFY-GG)
            guard: None,
            // (DON-UI) Donald's HUD pieces: KH3's IconText (the fonts' icons) and the style gauge's chevrons
            don: None,
            don_look: Default::default(),
            don_art: kh_ui::hud_resonance::DonArt { icons: fonts.icons.clone(), chevron: pack.texture("textures/Scaleform/01_hud/hud_command/StylePntGg.png").ok(), base: pack.texture("textures/Scaleform/01_hud/hud_status/HudStatusMpBase9.png").ok(), glow: pack.texture("textures/Scaleform/01_hud/hud_command/EffGlw1.png").ok(), flare: pack.texture("textures/Scaleform/01_hud/hud_status/CrossEff01.png").ok() },
            don_ring: Default::default(),
            don_stars: Default::default(),
            don_ycost: Default::default(),
            don_icons: Default::default(),
            don_glow: None,
            don_ex_hidden: false,
            don_list: std::sync::atomic::AtomicU8::new(0),
            shop: shop_ui::ShopState::load(&mut pack, &fonts), // (SHOP)
        };
        // (BN) the reward windows' look by roster row: colour label and the character's MenuFace bust (pack
        // build/sora_pack/saveload, the save slots' faces by roster key; a pack without it: the windows have no portrait)
        let faces = pack.data("../../saveload/saveload").ok();
        for row in crate::formation::ROSTER {
            let portrait = faces.as_ref().and_then(|d| d["faces"][row.key]["file"].as_str().map(|f| format!("../saveload/{f}"))).and_then(|f| pack.texture(&f).ok());
            if portrait.is_none() {
                println!("hud: the reward windows of {} have NO portrait (the save-load pack has no face '{}')", row.chara.name(), row.key);
            }
            hud.panel_charas.push((row.chara, kh_ui::hud_side::PanelChara { id: row.chara as usize, colour: kh_ui::hud_side::panel_colour(row.panel), portrait }));
        }
        // (party) the face movies of the status HUD by character (UI spec 4: loaded by the pawn's data asset path).
        // (PF2) by the roster's rows (a new character is one row: his `face`); Sora's PC face is the one loaded above.
        // The slots take them from the seats (`apply_seats`), never by a fixed index.
        let pc_face = hud.status_driver.faces[0].clone();
        for row in crate::formation::ROSTER {
            let face = if row.chara == crate::party::PartyChara::Sora {
                pc_face.clone()
            } else {
                match pack.movie(row.face) {
                    Ok(face) => Some(face),
                    Err(error) => {
                        println!("hud: the face of {} is NOT available ({error})", row.chara.name());
                        None
                    }
                }
            };
            hud.chara_faces.push((row.chara, face));
        }
        hud.sora_npc_face = pack.movie("FaceNpcExSora").ok();
        // The cockpit's show: status root `loop`, target plane `loop`, the menu enters.
        hud.status.set_status(0, 0);
        hud.target.native_root("loop");
        hud.target.movie.set_text("flowScope.flowLock.txSFLock.textField", &hud.side.lock.clone(), &fonts);
        Ok((hud, fonts))
    }

    pub fn status_line(&self) -> String {
        format!("{} movies of the UI pack, {} static SE names; command menu from the core (hud-cmd menu words switch to the debug command system)", self.movies, self.se_names.len())
    }

    pub fn movie(&self, name: &str) -> Option<&Movie> {
        // (SHOP) the KH shop is a full screen of its own, on the camp menu's layers (the camp menu is never up with it)
        if let Some(movie) = self.shop_movie(name) {
            return movie;
        }
        if let Some(screen) = self.saveload.as_ref().filter(|s| s.is_open()) {
            // (saveload) the Save / Load screen is alone on the screen. It is drawn on the two layers of the
            // new-game choice menu (never up together): the list on "NewGameText", its message window on
            // "NewGameDialog" (the Elden Ring overlay has 16 layers; none had to be added).
            return match name {
                "NewGameText" => Some(&screen.movie),
                "NewGameDialog" => screen.dialog_movie(),
                _ => None,
            };
        }
        if let Some(newgame) = self.newgame.as_ref().filter(|n| n.is_open()) {
            // (new game) the choice menu is alone on the screen: its words and its window
            return match name {
                "NewGameText" => Some(&newgame.text),
                "NewGameDialog" => Some(&newgame.dialog),
                _ => None,
            };
        }
        if self.gameover.is_open() || self.gameover_seq.as_ref().is_some_and(|s| s.scene_on()) {
            // (the cockpit is not shown over the game-over scene; (game over) nor in the void before / after the menu)
            return (name == "GameOver" && self.gameover.is_open()).then_some(&self.gameover.movie);
        }
        if self.savepoint.is_open() {
            // (savepoint) the save menu is a full screen of its own
            // (er-host, IMPL_ER21) ... and its Title Screen question on the topmost layer (never up with the new-game menu)
            return match name {
                "SavePoint" => Some(&self.savepoint.movie),
                "NewGameDialog" => self.savepoint.dialog_movie(),
                _ => None,
            };
        }
        if self.cutscene_pause.is_open() {
            // (cutscene) the cutscene pause menu is alone on the screen
            return (name == "CutscenePause").then_some(&self.cutscene_pause.movie);
        }
        if self.cutscene && !self.flow.paused() {
            // (cutscene) THE HUD IS HIDDEN IN A CUTSCENE (a menu that was up when it began stays until it is closed)
            return None;
        }
        Some(match name {
            "HudStatus" => &self.status.movie,
            "HudCommand" => &self.command.movie,
            "HudTarget" => &self.target.movie,
            "HudLeft" => &self.left.movie,
            "HudRight" => &self.right.movie,
            "Pause" if self.obtained_movie().is_some() => return self.obtained_movie(), // (HUD2) the OBTAINED popup on the pause menu's layer
            "Pause" if self.pause.is_open() => &self.pause.movie,
            "CampMenu" if self.camp_view.is_open() => &self.camp_view.movie,
            "GameHelp" => return self.camp_view.page_movie(),
            "Dialog" => return self.camp_view.dialog_movie(),
            // (ER party S8) the talk window / prompt / banner on the layer the new-game menu uses (never up together)
            "NewGameDialog" => return self.talk.as_ref().filter(|w| w.is_open()).map(|w| &w.dialog),
            // (CV1) the speech bubble over the play (not over a menu of ours)
            "SpeechBubble" if !self.pause.is_open() && !self.camp_view.is_open() => return self.bubble.as_ref().filter(|b| b.is_open()).map(|b| &b.movie),
            _ => return None,
        })
    }

    /// TEST-LEVEL CHOICE (sheet sora-test-stock): spare equipment into the menu's stock.
    pub fn stock_equipment(&mut self, list: &[(String, u32)]) {
        for (id, count) in list {
            self.inventory.add(id, *count);
        }
    }

    /// The sounds of the frame as audio-seam commands with a line for the event log.
    pub fn take_sounds(&mut self) -> Vec<(AudioCommand, String)> {
        let mut out = Vec::new();
        for sound in std::mem::take(&mut self.sounds) {
            let (index, looping, stop) = match sound {
                UiSound::Play(i) => (i, false, false),
                UiSound::LoopStart(i) => (i, true, false),
                UiSound::LoopStop(i) => (i, false, true),
            };
            let name = self.se_names.get(index as usize).copied().unwrap_or("");
            if stop {
                if let Some(handle) = self.loops.remove(&index) {
                    out.push((AudioCommand::Stop(handle, 0.0), format!("static SE {index} \"{name}\" loop stops")));
                }
                continue;
            }
            let number: u32 = name.rsplit('_').next().and_then(|n| n.parse().ok()).unwrap_or(index + 1);
            let handle = SoundHandle(self.next_handle);
            self.next_handle += 1;
            if looping {
                self.loops.insert(index, handle);
            }
            let play = SoundPlay { id: SoundId(UI_SOUND_IDS + number), name, at: Attach::SORA, volume: 1.0, pitch: 1.0, looping, spatial: false };
            out.push((AudioCommand::Play(handle, play), format!("static SE {index} \"{name}\"{}", if looping { " LOOP" } else { "" })));
        }
        out
    }

    fn dummy_key(targets: &dyn HudTargets, number: u32) -> Option<u64> {
        targets.target(TargetId(number)).map(|_| number as u64)
    }

    /// A debug word of `hud-cmd`. Words for this layer:
    /// `status hp|maxhp|mp|maxmp|focus N`, `status charge V|off`, `status clear` (display only);
    /// `libra on|off|auto`; `lock N` / `auto N` (a lock-on / auto cursor on dummy N), `unlock`;
    /// `exp N [DUMMY]`, `nextlv N`, `levelup LEVEL`, `munny N`, `item NAME`;
    /// `scope on [SECONDS] [syphon]`, `scope off [STATE]`, `locks N MAX`, `mark DUMMY`,
    /// `stylemax`. Everything else goes to the debug command system (its words:
    /// `kh_ui::command_debug::DebugCommandSystem::command`).
    fn word(&mut self, text: &str, f: &Frame) {
        let words: Vec<&str> = text.split_whitespace().collect();
        let number = |i: usize| words.get(i).and_then(|w| w.parse::<f32>().ok());
        let on = |i: usize| matches!(words.get(i).copied(), Some("on" | "1" | "true"));
        match words.as_slice() {
            ["status", "clear"] => self.overrides = Overrides::default(),
            ["status", "charge", "off"] => self.overrides.charge = None,
            ["status", what, _] => {
                let Some(value) = number(2) else { return println!("hud-cmd {text}: a number is expected") };
                match *what {
                    "hp" => self.overrides.hp = Some(value as i32),
                    "maxhp" => self.overrides.hp_max = Some(value as i32),
                    "mp" => self.overrides.mp = Some(value as i32),
                    "maxmp" => self.overrides.mp_max = Some(value as i32),
                    "focus" => self.overrides.focus = Some(value as i32),
                    "charge" => self.overrides.charge = Some(value),
                    _ => println!("hud-cmd {text}: status hp | maxhp | mp | maxmp | focus | charge N"),
                }
            }
            ["libra", "auto"] => self.libra = None,
            ["libra", _] => self.libra = Some(on(1)),
            ["lock" | "auto", _] => {
                let ty = if words[0] == "lock" { LOCKON_TYPE_NORMAL } else { LOCKON_TYPE_AUTO };
                match number(1).and_then(|n| Self::dummy_key(f.targets, n as u32)) {
                    Some(key) => {
                        self.debug_cursors.retain(|c| c.1 != ty);
                        self.debug_cursors.push((key as u32, ty));
                    }
                    None => println!("hud-cmd {text}: no such training dummy"),
                }
            }
            ["unlock"] => self.debug_cursors.clear(),
            ["mark", _] => {
                if let Some(key) = number(1).and_then(|n| Self::dummy_key(f.targets, n as u32)) {
                    self.target_driver.add_cursor(&mut self.target, key, LOCKON_TYPE_FLOWSHOTATTACK, true, &mut self.sounds);
                }
            }
            ["exp", ..] => {
                let dummy = number(2).map_or(1, |n| n as u32);
                if let (Some(value), Some(d)) = (number(1), f.targets.target(TargetId(dummy))) {
                    let position = (f.to_view)(d.center);
                    self.target_driver.add_exp_point(&mut self.target, position, value as i32);
                }
            }
            ["nextlv", _] => {
                if let Some(value) = number(1) {
                    self.left.show_next_lv(value as i32);
                }
            }
            ["levelup", ..] => {
                let lines = self.side.level_up_lines[..2].to_vec();
                let sora = self.panel_for(crate::party::PartyChara::Sora); // (BN)
                self.right.show_level_up_as(number(1).unwrap_or(2.0) as i32, &sora, &lines);
                self.sounds.push(UiSound::Play(SE_LEVEL_UP));
            }
            ["munny", _] => {
                if let Some(value) = number(1) {
                    self.left.show_munny(value as i32);
                }
            }
            ["item", name @ ..] if !name.is_empty() => {
                if self.left.show_item(&name.join(" ").replace('_', " "), false) {
                    self.sounds.push(UiSound::Play(SE_ITEM));
                }
            }
            ["scope", "on", ..] => {
                let syphon = words.contains(&"syphon");
                self.target_driver.open_scope(&mut self.target, number(2).unwrap_or(6.0), syphon, &mut self.sounds);
                self.run_system("shotlock on");
            }
            ["scope", "off", ..] => {
                self.target_driver.close_scope(&mut self.target, number(2).map_or(SFSS_OUT, |n| n as i32));
                let marks: Vec<u64> = self.target_driver.cursors.iter().filter(|c| c.ty == LOCKON_TYPE_FLOWSHOTATTACK).map(|c| c.key).collect();
                for key in marks {
                    self.target_driver.remove_cursor(&mut self.target, key, LOCKON_TYPE_FLOWSHOTATTACK, &mut self.sounds);
                }
                self.run_system("shotlock off");
            }
            ["locks", _, _] => {
                if let (Some(total), Some(max)) = (number(1), number(2)) {
                    self.target_driver.set_lock_count(&mut self.target, total as i32, max as i32, &mut self.sounds);
                }
            }
            // (er-host, IMPL_ER21) `se N`: one of the UI's static sounds by index (kh_ui::menu::SE_*)
            ["se", _] => {
                if let Some(index) = number(1) {
                    self.sounds.push(UiSound::Play(index as u32));
                }
            }
            ["stylemax"] => self.target_driver.set_style_ratio(1.0),
            ["battle", _] => {
                self.battle = number(1).unwrap_or(0.0).clamp(0.0, 2.0) as i32;
                self.run_system(text);
            }
            ["give", what, ..] => self.give(what, number(2).map_or(1, |n| n.max(1.0) as u32)),
            // (IMPL_ER28) `hud-cmd obtain NAME [N]`: any item of the item table (equipment too) THROUGH THE DOOR every
            // found item takes (`crate::items::give`): the popup, the stock
            ["obtain", what, ..] => match crate::items::find(what) {
                Some(item) => drop(crate::items::give(item.item_id, number(2).map_or(1, |n| n.max(1.0) as u32))),
                None => println!("hud-cmd obtain {what}: no such item of the item table"),
            },
            // (IMPL_ER28) `hud-cmd hostkey NAME [N]`: one more of the host's key items (the Items screen's Key Items tab): a test's
            ["hostkey", name, ..] => {
                let mut list = crate::items::key_items();
                list.push(crate::items::KeyItem::new(name.replace('_', " "), number(2).map_or(1, |n| n.max(1.0) as u32)));
                crate::items::set_key_items(list);
            }
            // (IMPL_ER42) `hud-cmd hostkeyhelp WORDS...`: the description of the last host key item (a test's; `|` = a new line)
            ["hostkeyhelp", ..] => {
                let mut list = crate::items::key_items();
                if let Some(last) = list.last_mut() {
                    last.help = words[1..].join(" ").replace(" | ", "\n").replace('|', "\n");
                }
                crate::items::set_key_items(list);
            }
            // (IMPL_ER43) `hud-cmd hostkeyicon N`: the icon number of the last host key item (a test's; Elden Ring's iconId)
            ["hostkeyicon", n] => {
                let mut list = crate::items::key_items();
                if let (Some(last), Ok(n)) = (list.last_mut(), n.parse::<u32>()) {
                    last.icon_id = Some(n);
                }
                crate::items::set_key_items(list);
            }
            ["conv", rest @ ..] => self.conv_word(rest, f), // (CV1) `conv demo` / `conv end`
            ["live"] => self.debug_commands = false,
            ["debug"] => self.debug_commands = true,
            _ => {
                // A word of the debug command system: its menu is the one on show from here on.
                if !matches!(words.first().copied(), Some("battle" | "simple" | "show" | "hide")) {
                    self.debug_commands = true;
                }
                self.run_system(text)
            }
        }
    }

    fn run_system(&mut self, text: &str) {
        if let Err(error) = self.system.command(text) {
            println!("{error}");
        }
    }

    /// One drawn frame.
    pub fn frame(&mut self, f: &mut Frame) {
        self.play_seconds = f.play_seconds;
        let g = &f.snapshot.gauges;
        self.system.begin_frame();
        if !self.started {
            self.started = true;
            self.run_system("show");
        }
        let (menu_words, words): (Vec<String>, Vec<String>) = f.words.iter().cloned().partition(|w| w == "menu" || w.starts_with("menu "));
        let nav = std::mem::take(&mut f.nav);
        self.take_given_items(f.sora); // (loot)
        self.publish_held(f.sora); // (IMPL_ER28)
        if self.saveload_frame(f.pressed, &nav, &menu_words, f.ticks) { // (saveload)
            return;
        }
        if self.newgame_frame(f, &nav, &menu_words) { // (new game)
            return;
        }
        if self.game_over(f, &nav, &menu_words) {
            return;
        }
        if self.save_point(f, &nav, &menu_words) { // (savepoint)
            return;
        }
        if self.shop_frame(f, &nav, &menu_words) { // (SHOP) the KH shop has the pad and the frame
            return;
        }
        if self.cutscene_frame(f, &nav, &menu_words) { // (cutscene)
            return;
        }
        if self.obtained_frame(f, &menu_words) { // (HUD2) the OBTAINED popup holds the world and the pad
            return;
        }
        self.talk_frame(f); // (ER party S8) the talk window: its frames; A goes on while a line is up
        self.bubble_frame(f, &nav, &menu_words); // (CV1) the speech bubble: A / B / D-pad / stick while it is up; the demo conversation
        self.menus(f, f.pressed, &nav, &menu_words);
        if self.flow.paused() {
            // The engine pause: the cockpit movies are frozen, only the menu movies run.
            for _ in 0..f.ticks {
                self.tick_menus();
            }
            return;
        }
        // The pages / dialog layers finish their way out after the menu closed.
        for text in &words {
            self.word(text, f);
        }

        // ---- status HUD: the pawn's values as the native update reads them.
        let o = self.overrides;
        let hp = o.hp.unwrap_or(g.hp);
        let hp_max = o.hp_max.unwrap_or(g.hp_max);
        let mut values = StatusValues {
            focus: o.focus.unwrap_or(g.focus),
            focus_max: g.focus_max,
            game_in_battle: self.battle == 2,
            // LOCAL: the story flags are not in the snapshot; a level-99 Sora has both gauges.
            mp_gauge_unlocked: true,
            focus_gauge_unlocked: true,
            ..StatusValues::default()
        };
        let sora_values = SlotValues {
            // The empty placeholder status has no gauges at all.
            active: hp_max > 0,
            hp,
            hp_max,
            mp: o.mp.unwrap_or(g.mp),
            mp_max: o.mp_max.unwrap_or(g.mp_max),
            mp_charging: o.charge.is_some() || g.mp_charging,
            mp_charge_value: o.charge.unwrap_or(g.mp_charge_value),
            in_battle: self.battle == 2,
        };
        // (party P) the leader switch's shrink runs out: the swap (the slots grow back)
        if let Some((leader, sora_in, left)) = self.leader_anim {
            let left = left.saturating_sub(f.ticks);
            if left == 0 {
                self.leader_anim = None;
                self.apply_leader(leader, sora_in);
            } else {
                self.leader_anim = Some((leader, sora_in, left));
            }
        }
        // (PF2) THE SEATS: the leader in the main slot, the other members in formation order (a member slotted in or
        // out at the save point moves the others up / down; their faces follow). Frozen while the shrink runs.
        if self.leader_anim.is_none() {
            let seats = self.layout(self.leader, self.sora_in);
            self.apply_seats(seats, false);
        }
        // (party) THE SLOTS (UI spec 4): each seated pawn's values as the native update reads them; the HUD's own driver
        // picks the face look (death below 1 HP, dying at a pinch, battle, normal) and the party / solo layout.
        // (party P) A friend leading: his values in the main slot, Sora's in a friend slot (none when Sora is out).
        for (i, seat) in self.seats.iter().enumerate() {
            values.slots[i] = match seat {
                Some(crate::party::PartyChara::Sora) => SlotValues { active: sora_values.active && (i == 0 || self.sora_in), ..sora_values },
                Some(c) => self.party.iter().find(|p| p.chara == *c).map_or(SlotValues::default(), |p| SlotValues { active: p.hp_max > 0, hp: p.hp, hp_max: p.hp_max, mp: p.mp, mp_max: p.mp_max, mp_charging: p.mp_charging, mp_charge_value: p.mp_charge_value, in_battle: p.in_battle }),
                None => SlotValues::default(),
            };
        }
        // LOCAL (the game sets the bit from its damage path): THE PORTRAIT'S DAMAGE FLASH AND BOUNCE when a character's
        // HP went down. (PF2) Kept by character and sent to the slot he sits in NOW: before, a friend leading sent
        // Sora's drop to slot 0 (the friend's face) and the leader's drop to Sora's small slot.
        let mut seen: Vec<(crate::party::PartyChara, i32)> = Vec::new();
        for (i, seat) in self.seats.iter().enumerate() {
            let (Some(c), v) = (seat, &values.slots[i]) else { continue };
            if !v.active {
                continue;
            }
            if self.hp_seen.iter().any(|(who, last)| who == c && v.hp < *last) {
                self.status_driver.notify_damage(i);
            }
            seen.push((*c, v.hp));
        }
        self.hp_seen = seen;
        // (party P) A friend leading: the friends have no focus gauge (KH3's flag that hides the part).
        if self.leader != crate::party::PartyChara::Sora {
            values.focus_gauge_unlocked = false;
            // (party X2-b) his own bar in the focus slot (Donald's Spell Volley focus, Goofy's shield gauge / Barrage focus)
            if let Some((focus, focus_max)) = self.party.iter().find(|p| p.chara == self.leader).and_then(|p| p.focus) {
                (values.focus, values.focus_max, values.focus_gauge_unlocked) = (focus, focus_max, true);
            }
        }
        self.status_driver.update(&mut self.status, &values, f.dt, &mut self.sounds);

        // The formchange gauge and the situation commands: `UiSnapshot::situation` (the combat
        // strand's HUD list). The debug command system mirrors the gauge and the new list
        // entries for its own model; the target HUD shows the "gauge full" notice once.
        let situation = &f.snapshot.situation;
        let style = situation.gauge_ratio.clamp(0.0, 0.999);
        if (style - self.last_style).abs() > 1.0e-4 {
            if situation.gauge_ratio >= 1.0 && self.last_style < 0.999 {
                self.target_driver.set_style_ratio(1.0);
            }
            self.last_style = style;
            let words = if style > 0.0 { format!("style {style}") } else { "style off".to_string() };
            self.run_system(&words);
        }
        if self.debug_commands {
            for entry in situation.list.iter().filter(|e| e.serial > self.last_serial) {
                let name = self.command_names.get(&(entry.kind.value() as i64)).cloned().or_else(|| kh_core::abilities::xs::command_name(entry.kind.value() as i64).map(str::to_string)).unwrap_or_else(|| entry.kind.name().to_string()); // (XSI)
                self.system.add_situation(&name, entry.total.max(0.0), 1);
            }
        }
        self.last_serial = situation.list.iter().map(|e| e.serial).max().unwrap_or(self.last_serial).max(self.last_serial);

        // ---- command menu: the debug command system's model.
        let slot = &values.slots[0];
        self.system.set_status(slot.mp, slot.mp_max, slot.mp_charging);
        self.system.step(f.dt);
        if self.debug_commands {
            self.display.update(&mut self.command, &self.system.model, &self.strings, f.dt, &mut self.sounds);
        } else {
            let model = self.live_model(f);
            self.display.update(&mut self.command, &model, &self.strings, f.dt, &mut self.sounds);
        }

        // ---- targets: cursors, enemy gauge.
        let libra = self.libra.unwrap_or_else(|| f.sora.status().abilities.has(kh_core::enums::ETresAbilityKind::Libra));
        // (DON-UI) while Donald leads, his own Scan decides (DONALD_OVERHAUL.md 9: enemy HP only with Scan, as KH3)
        let libra = match (&self.don, self.libra) {
            (Some(d), None) => d.scan,
            _ => libra,
        };
        let mut wanted: Vec<(u64, usize)> = self.debug_cursors.iter().map(|c| (c.0 as u64, c.1)).collect();
if wanted.is_empty() {
            // The locked target and the AUTO (soft) target of the snapshot (UI spec 7.1: types 1, 0).
            if f.snapshot.lock_on != TargetId::NONE {
                wanted.push((f.snapshot.lock_on.0 as u64, LOCKON_TYPE_NORMAL));
            } else if f.snapshot.soft_target != TargetId::NONE {
                wanted.push((f.snapshot.soft_target.0 as u64, LOCKON_TYPE_AUTO));
            }
        }
        let shown: Vec<(u64, usize)> = self.target_driver.cursors.iter().filter(|c| c.ty <= LOCKON_TYPE_NORMAL).map(|c| (c.key, c.ty)).collect();
        for (key, ty) in &shown {
            if !wanted.contains(&(*key, *ty)) {
                self.target_driver.remove_cursor(&mut self.target, *key, *ty, &mut self.sounds);
                if self.target_driver.target_cursor_count() == 0 {
                    // The last auto / lock-on cursor went: the gauge stays half a second.
                    self.enemy.clear(false);
                }
            }
        }
        for (key, ty) in &wanted {
            if !shown.contains(&(*key, *ty)) && self.target_driver.add_cursor(&mut self.target, *key, *ty, false, &mut self.sounds).is_some() && libra {
                // The gauge points at the pawn that owns the new cursor.
                self.enemy.point_at(*key, false, None);
            }
        }
        // ... and at the enemy he just damaged, with the HP before the hit.
        if libra {
            for event in f.events {
                if let EventKind::Hit { record, result } = &event.kind {
                    if result.accepted && event.step > self.last_step {
                        self.enemy.point_at(record.target.0 as u64, false, f.targets.hit_prev_hp(record.target, result.hp_after, result.damage_dealt)); // (HUD2) in the gauge's units
                    }
                }
            }
        }
        // ---- the shotlock aim and the push-button prompt (integration pass).
        self.aim(f);
        self.push_prompt(f);
        let (dummies, to_view) = (f.targets, f.to_view);
        let points: HashMap<u64, kh_core::Vec3> = f.snapshot.aim.airstep.iter().filter(|m| m.target == TargetId::NONE).map(|m| (mark_key(m), m.position)).collect();
        let position_of = |key: u64| if key & POINT_KEY != 0 { points.get(&key).map(|p| to_view(*p)) } else { dummies.target(TargetId(key as u32)).map(|d| to_view(d.center)) };
        let values_of = |key: u64| dummies.target(TargetId(key as u32)).map(|d| EnemyValues { hp: d.hp as i32, hp_max: d.hp_max as i32, large: d.large, alive: d.hp > 0.0 });
        self.enemy.update(&mut self.right, f.dt, &values_of, &mut self.sounds);

        // ---- popups: what is new on the snapshot's board.
        self.popups(f);
        self.last_step = f.snapshot.step;

        // ---- the projection pass with this frame's camera.
        let view = f.view.map(|v| kh_ui::hud_target::View { clip_from_world: v.clip_from_world, rect: (0.0, 0.0, v.size.0, v.size.1) });
        self.target_driver.update(&mut self.target, f.dt, view.as_ref(), &position_of, &mut self.sounds);

        // ---- stage frames.
        for _ in 0..f.ticks {
            self.status.tick(1.0 / 60.0);
            self.command.tick();
            self.target.tick();
            self.left.tick();
            self.right.tick(1.0 / 60.0);
        }
        // (GOOFY-GG) THE GUARD BAR above MP while Goofy is the leader in the main slot (it replaces his focus bar). After
        // the stage frames: its colours and visibility override what the copied MP clips' timelines set this frame.
        {
            let goofy = self.leader == crate::party::PartyChara::Goofy;
            let guard = self.guard.as_ref().filter(|_| goofy).map(|g| (&g.0, &g.1));
            self.guard_bar.update(&mut self.status, guard, f.dt);
        }
        // (DON-UI) DONALD'S PIECES while he is the leader in the main slot (after the stage frames, as the GUARD bar)
        self.don_frame(f.dt, f.snapshot.command.state);
    }

    /// THE SHOTLOCK AIM (UI spec 7.3) from `UiSnapshot::aim`: the scope opens with the aim time
    /// (static SE 18, or 25 for focus syphon / no time), every new lock adds a mark on its target
    /// (type 2, or 3 while syphoning; several per target) and sends the lock counter (static SE 19,
    /// 20 on the lock that reaches the maximum), the airstep chain's targets carry type-4 marks
    /// (static SE 19 each), and the scope leaves with `out` (cancel), `select` (fired) or
    /// `select2` (airstep). WHEN THE MARKS GO (magic spec A3.3 / A3.4, the calls of 0x14157A7A0 per
    /// candidate): on a cancel and on an airstep all of them with the scope; ON A SHOT only the
    /// airstep marks go with the scope (`select`), the shotlock marks stay on their targets while
    /// the shotlock plays and are removed, target by target, when its state has ended (the
    /// controller's mode leaves 2; the cockpit then gets scope state 3, which the movie ignores).
    /// `hud-cmd scope ...` still drives the same clips by hand while no aim is open.
    fn aim(&mut self, f: &Frame) {
        let aim = &f.snapshot.aim;
        // (fixes pass 1) AFTER AN AIRSTEP the scope clip was left on its `select2` label, whose
        // last frame still shows the full-screen aim filter (user report: "the shotlock screen
        // effects remain after you airstep"). The game sends the scope a second state when the
        // airstep's state has ended (controller mode 3 cleaned up, magic spec A3.4 / F A9:
        // `FUN_141577520(hud, 5)` "airstep finished" = `setFlowScope(5)`, label `out2`).
        if self.aim_airstep && aim.mode != 3 {
            self.target.set_flow_scope(kh_ui::hud_target::SFSS_OUT_ATHLETIC_FLOW1, false, true);
            self.aim_airstep = false;
        }
        if self.aim_fired && aim.mode != 2 {
            // The ShootFlow state ended (controller slot 809 cleared the pending flag).
            for (key, _) in std::mem::take(&mut self.aim_marks) {
                self.target_driver.remove_cursor(&mut self.target, key, self.aim_mark_type, &mut self.sounds);
            }
            self.target.set_flow_scope(3, false, true);
            self.aim_fired = false;
        }
        if aim.open() && !self.aim_open {
            let syphon = aim.mode == 4;
            self.target_driver.open_scope(&mut self.target, aim.aim_time, syphon, &mut self.sounds);
            (self.aim_open, self.aim_total) = (true, 0);
            self.aim_mark_type = if syphon { LOCKON_TYPE_FLOWSHOTFOCUSCHARGE } else { LOCKON_TYPE_FLOWSHOTATTACK };
        }
        if !self.aim_open {
            return;
        }
        if aim.open() {
            for mark in &aim.locks {
                let key = mark_key(mark);
                let shown = self.aim_marks.entry(key).or_insert(0);
                while *shown < mark.count {
                    *shown += 1;
                    if self.target_driver.add_cursor(&mut self.target, key, self.aim_mark_type, true, &mut self.sounds).is_none() {
                        break;
                    }
                }
            }
            if aim.total > self.aim_total {
                self.aim_total = aim.total;
                self.target_driver.set_lock_count(&mut self.target, aim.total, aim.max, &mut self.sounds);
            }
            let wanted: Vec<u64> = aim.airstep.iter().map(mark_key).collect();
            for key in std::mem::take(&mut self.air_marks) {
                if wanted.contains(&key) {
                    self.air_marks.push(key);
                } else {
                    self.target_driver.remove_cursor(&mut self.target, key, LOCKON_TYPE_FLOWSHOTMOVE, &mut self.sounds);
                }
            }
            for key in wanted {
                if !self.air_marks.contains(&key) && self.target_driver.add_cursor(&mut self.target, key, LOCKON_TYPE_FLOWSHOTMOVE, false, &mut self.sounds).is_some() {
                    self.air_marks.push(key);
                }
            }
            return;
        }
        // Released, fired or airstepped (controller +0xA10: 2 fired, 3 airstep requested).
        let state = match aim.mode {
            2 => SFSS_OUT_SHOOT_FLOW0,
            3 => SFSS_OUT_ATHLETIC_FLOW0,
            _ => SFSS_OUT,
        };
        self.target_driver.close_scope(&mut self.target, state);
        self.aim_airstep = aim.mode == 3;
        if aim.mode == 2 {
            self.aim_fired = true;
        } else {
            for (key, _) in std::mem::take(&mut self.aim_marks) {
                self.target_driver.remove_cursor(&mut self.target, key, self.aim_mark_type, &mut self.sounds);
            }
        }
        for key in std::mem::take(&mut self.air_marks) {
            self.target_driver.remove_cursor(&mut self.target, key, LOCKON_TYPE_FLOWSHOTMOVE, &mut self.sounds);
        }
        self.aim_open = false;
    }

    /// THE PUSH-BUTTON PROMPT (`UiSnapshot::prompt`; HUD function 0x1416793E0, kh_core
    /// ui_model/prompts.rs): "DODGE!" on the jump button while the risk-dodge window is open,
    /// "ATTACK!" on the attack button at an airstep's enemy stop. The pad icons are the western
    /// layout's (jump = Circle = B on an Xbox pad, attack = Cross = A).
    fn push_prompt(&mut self, f: &Frame) {
        use kh_core::ui_model::PromptKind;
        let now = f.snapshot.prompt;
        if now == self.prompt {
            return;
        }
        if let Some(before) = self.prompt.take() {
            let used = match before.kind {
                PromptKind::RiskDodge => f.sora.risk_dodge().is_some(),
                PromptKind::AirstepAttack => f.sora.airstep().is_some_and(|a| a.attack_latched) || f.state == kh_core::StateId::Attack,
            };
            self.target_driver.hide_push_mark(&mut self.target, used);
        }
        if let Some(prompt) = now {
            let icon = if prompt.button() == kh_core::PadButton::JUMP { "<icon(Circle)>" } else { "<icon(Cross)>" };
            let label = self.camp_view.string(prompt.label_key());
            let key = (prompt.target != TargetId::NONE).then_some(prompt.target.0 as u64);
            self.target_driver.show_push_mark(&mut self.target, icon, &label, key);
            self.prompt = Some(prompt);
        }
    }

    /// The order within one EXP award: level-up popup, EXP point, Next LV
    /// (docs/sora-spec-reconciliation-2.md; the stats spec's order).
    fn popups(&mut self, f: &Frame) {
        let board = &f.snapshot.board;
        let elapsed = (f.snapshot.step.saturating_sub(self.last_step)) as f32 * f.step_dt + 1.0e-4;
        let fresh = |now: &Option<kh_core::ui_model::TimedPopup>, before: &Option<kh_core::ui_model::TimedPopup>| match (now, before) {
            (Some(n), Some(b)) => n.age < b.age || n.popup.value != b.popup.value,
            (Some(n), None) => n.age <= elapsed.max(0.05),
            _ => false,
        };
        if fresh(&board.level_up, &self.last_board.level_up) {
            let popup = board.level_up.unwrap().popup;
            let mut lines = Vec::new();
            if let PopupDetail::LevelUp(up) = popup.detail {
                // Kinds 1 Strength, 2 Magic, 3 Defense, 4 AP, 7 ability; the panel has three lines.
                for (delta, line) in [(up.strength, 0), (up.magic, 2), (up.defense, 1), (up.ap, 3)] {
                    if delta != 0 {
                        lines.push(self.side.level_up_lines[line].clone());
                    }
                }
                if let Some(ability) = up.ability {
                    // The ability's English name (the stats sheet's table).
                    let name = kh_core::status::tables::ABILITIES.iter().find(|row| row.kind == ability).map_or(ability.name(), |row| row.name);
                    lines.push(kh_ui::hud_side::icon_line(kh_ui::hud_side::LineIcon::Ability, name)); // (BN) kind 7: the sparkle
                }
            }
            let sora = self.panel_for(crate::party::PartyChara::Sora); // (BN) his colour and portrait
            self.right.show_level_up_as(popup.value as i32, &sora, &lines);
            self.sounds.push(UiSound::Play(SE_LEVEL_UP));
        }
        // (party) the friends' windows right after Sora's (CheckLevelUp runs for the player, then each party slot):
        // each takes its own window of HudRight's five (kh_ui::hud_side::LEVEL_UP_WINDOWS), stacked
        while let Some((level, chara, lines)) = self.party_levels.pop_front() {
            let who = self.panel_for_index(chara); // (BN) the roster's colour and portrait
            self.right.show_level_up_as(level, &who, &lines);
            self.sounds.push(UiSound::Play(SE_LEVEL_UP));
        }
        for point in &board.exp_points {
            if point.age <= elapsed && self.last_step != 0 {
                self.target_driver.add_exp_point(&mut self.target, (f.to_view)(point.position), point.value);
            }
        }
        if fresh(&board.next_lv, &self.last_board.next_lv) {
            self.left.show_next_lv(board.next_lv.unwrap().popup.value as i32);
        }
        if fresh(&board.munny, &self.last_board.munny) {
            let now = board.munny.unwrap().popup.value as i32;
            let before = self.last_board.munny.map_or(0, |m| m.popup.value as i32);
            self.left.show_munny(if now > before { now - before } else { now });
        }
        // (new game) the bonus panel of a reward (`Sora::grant`): the game's victory-bonus popup
        if fresh(&board.bonus, &self.last_board.bonus) {
            let mut lines = Vec::new();
            let mut magic = Vec::new();
            if let PopupDetail::Bonus(bonus) = board.bonus.unwrap().popup.detail {
                for line in bonus {
                    if line == kh_core::status::BonusLine::None {
                        continue;
                    }
                    // (BN) KH3's text of the entry with KH3's icon in front (0x141596150)
                    let text = self.bonus_line_text(line);
                    if matches!(line, kh_core::status::BonusLine::Magic(_)) {
                        magic.push(text.clone());
                    }
                    lines.push(text);
                }
            }
            let sora = self.panel_for(crate::party::PartyChara::Sora);
            self.right.show_bonus_panel(&sora, &lines);
            self.sounds.push(UiSound::Play(kh_ui::hud_side::SE_BONUS));
            // (BN) a spell or tier learned is the shared book's: every party member of the roster who casts it (its row's
            // `casts_magic`: Donald) gets it on his own panel too
            if !magic.is_empty() {
                // (the friend slots and the leader: a friend who leads is in the main slot, not in a friend slot)
                let mut casters: Vec<crate::party::PartyChara> = self.party.iter().map(|slot| slot.chara).chain(std::iter::once(self.leader)).filter(|c| *c != crate::party::PartyChara::Sora && crate::formation::entry_of(*c).is_some_and(|r| r.casts_magic)).collect();
                let mut seen = Vec::new();
                casters.retain(|c| !seen.contains(c) && {
                    seen.push(*c);
                    true
                });
                for c in casters {
                    self.party_bonuses.push_back((c, magic.clone()));
                }
            }
        }
        // (BN) the friends' BONUS! panels of this frame (boss rewards, roster boosts, the shared spells): each character in
        // his own window (colour + portrait of his roster row), two lines a panel
        while let Some((chara, lines)) = self.party_bonuses.pop_front() {
            let who = self.panel_for(chara);
            for pair in lines.chunks(2) {
                if self.right.show_bonus_panel(&who, pair) {
                    self.sounds.push(UiSound::Play(kh_ui::hud_side::SE_BONUS));
                }
            }
        }
        if fresh(&board.item, &self.last_board.item) {
            // (new game) the item by its name in the game (Potion), not by its key (ITEM_POTION)
            let key = board.item.unwrap().popup.text;
            let name = kh_core::status::ItemKind::from_name(key).map_or(key.to_string(), item_display_name);
            // (loot) with the item's picture (the popup's text is the item's key)
            let icon = kh_core::status::ItemKind::from_name(key).and_then(|k| crate::items::by_id(k as u32)).map(|i| i.key.as_str());
            if self.left.show_item_icon(icon, &name, false) {
                self.sounds.push(UiSound::Play(SE_ITEM));
            }
        }
        self.last_board = board.clone();
    }
}


// ---------------------------------------------------------------------------------------------
// TIER 3: the Menu button, the engine pause, the Pause screen and the camp menu
// (docs/sora-ui-spec.md section 10; logic in kh_ui::menu / kh_ui::camp / camp_equip / camp_custom).

/// What the camp menu asks of the game while it is paused; applied by [`Hud::apply_actions`], the
/// one place where the HUD layer writes to the simulation.
pub enum StatusAction {
    /// One COPY of an ability (the list row under the cursor), not "one more / one fewer".
    SetAbility { kind: kh_core::enums::ETresAbilityKind, copy: u8, on: bool },
    Equip { kind: u8, slot: usize, id: Option<String> },
    AutoReload { item: String, on: bool },
    UseItem { id: String },
    Discard { id: String },
    SetMagicSlot { slot: usize, kind: u8 },
    SwapMagicSlots(usize, usize),
    SetShortcut { page: usize, slot: usize, kind: u8 },
    /// (XSI, 2026-10-16) Customize's Shotlock choice (`CommandState::shotlock`).
    SetShotlock(u8),
    /// `hud-cmd give ID [N]`: a battle item into the stock.
    GiveItem(kh_core::status::ItemKind, u8),
    /// The menu closed: `Sora::on_unpause` (the pouch's used auto-reload slots are refilled).
    Unpause,
    /// (party X2-b) A friend's kit ability row (id `hud_x2::KIT_ID_BASE` + the row's index) worn / taken off.
    SetKit { id: i32, on: bool },
}

/// What the save data holds beyond the core's status: the equipment stock and the Keyblade
/// slots 1 and 2 (the core has one weapon; the WEAPONCHANGE state is not in the port, so the
/// other slots only exist in this menu).
#[derive(Default)]
pub struct Inventory {
    /// Equipment in stock: (row id of the equipment table, count).
    pub equipment: Vec<(String, u32)>,
    pub spare_weapons: [Option<String>; 2],
    /// (boss rewards, IMPL_ER31) Things that happened once for this character (`boss:<row id>` = that boss's
    /// reward was given: crate::bossrewards); kept by the save (`SoraSave::records`).
    pub records: Vec<String>,
}

impl Inventory {
    pub fn add(&mut self, id: &str, n: u32) {
        match self.equipment.iter_mut().find(|e| e.0 == id) {
            Some(entry) => entry.1 = (entry.1 + n).min(STOCK_MAX),
            None => self.equipment.push((id.to_string(), n.min(STOCK_MAX))),
        }
    }

    pub fn take(&mut self, id: &str) -> bool {
        match self.equipment.iter_mut().find(|e| e.0 == id && e.1 > 0) {
            Some(entry) => {
                entry.1 -= 1;
                true
            }
            None => false,
        }
    }
}

/// The stock maximum of a piece (`TresItemData.m_MaxNum` of battle items; taken for equipment too).
const STOCK_MAX: u32 = 99;

fn option_rows(config: &UserConfig) -> Vec<OptionRow> {
    OPTIONS
        .iter()
        .map(|info| {
            let (choices, range) = match info.choices {
                Choices::List(list) => (list.iter().map(|s| s.to_string()).collect(), (0, list.len().saturating_sub(1) as u8)),
                Choices::Slider { min, max } => (Vec::new(), (min, max)),
            };
            OptionRow { label: info.label.to_string(), choices, range, value: config.get(info.option), stored: info.storage != Storage::None }
        })
        .collect()
}

fn item_display_name(item: kh_core::status::ItemKind) -> String {
    match kh_core::magic::items::command_of_item(item) {
        Some(cmd) => kh_core::magic::spells::command_name(cmd).to_string(),
        None => "Tent".to_string(),
    }
}

/// An equipment row as the menu's piece.
fn piece_of(row: &kh_core::status::tables::EquipmentRow) -> Piece {
    let kind = match row.kind {
        "keyblade" | "keyblade-upgrade" => KIND_WEAPON,
        "armour" => KIND_ARMOUR,
        _ => KIND_ACCESSORY,
    };
    // `m_AttrResist*` by ETresDamageAttribute: 1 fire, 2 blizzard, 3 thunder, 4 water, 5 aero, 6 dark.
    let resist = [1, 2, 3, 4, 5, 6].map(|a| row.resist.get(a).copied().unwrap_or(0));
    let abilities = row.abilities.iter().map(|id| kh_core::status::tables::ABILITIES.iter().find(|a| a.id == *id).map_or(id.to_string(), |a| a.name.to_string())).collect();
    Piece { id: row.id.to_string(), name: row.name.to_string(), kind, ap: row.ap, attack: row.attack, magic: row.magic, defense: row.defense, resist, abilities, help: String::new() }
}

fn item_piece(item: kh_core::status::ItemKind) -> Piece {
    Piece { id: item.name().to_string(), name: item_display_name(item), kind: KIND_ITEM, ..Piece::default() }
}

impl Hud {
    /// True while a menu holds the engine pause.
    pub fn paused(&self) -> bool {
        self.shop.is_open() /* (SHOP) the shop holds the world like the camp menu */ || self.flow.paused() || self.obtained_open() /* (HUD2) the OBTAINED popup pauses the game */ || self.savepoint.is_open() || self.cutscene_pause.is_open() || self.newgame_choice_open() || self.saveload_open() || self.gameover_seq.as_ref().is_some_and(|s| s.scene_on()) // (game over) the void scene holds the world; (saveload) the Save / Load screens too; (new game) the choice menu holds the world too; (savepoint) the save menu pauses like the camp menu; (cutscene) so does the cutscene pause menu
    }

    /// (new game) OPENS THE NEW-GAME CHOICE MENU: the two questions of the opening of Kingdom Hearts III
    /// (kh_ui::newgame says what is the game's and what is composed). The host calls it when a new game begins
    /// and no pair is stored (no save for the character), BEFORE the player gets control, and supplies a dark
    /// screen behind it. While it is up [`Hud::paused`] is true (the host holds its world and does not step
    /// Sora), nothing else of the HUD is drawn, and the pad drives it: D-pad / left stick up and down (the menus'
    /// repeat), A decide, B back one step. The pair comes once from [`Hud::take_newgame_choice`]; the host then
    /// builds the new game with it (`kh_host::save::new_game_loadout_for`). False: a menu is up already, or the
    /// UI pack has no CutsceneText movie ([`Hud::newgame_choice_available`]): the host then starts with a pair of
    /// its own (`kh_host::save::new_game_loadout`).
    pub fn open_newgame_choice(&mut self) -> bool {
        if self.flow.paused() || self.gameover.is_open() || self.savepoint.is_open() || self.cutscene_pause.is_open() || self.newgame_choice_open() {
            return false;
        }
        let Some(newgame) = self.newgame.as_mut() else { return false };
        self.newgame_result = None;
        newgame.open(&mut self.sounds);
        (self.log)("new game: the choice menu opens (its two questions, kh3_level text_0020 / text_0030)");
        true
    }

    /// (new game) The choice menu is on screen.
    pub fn newgame_choice_open(&self) -> bool {
        self.newgame.as_ref().is_some_and(|n| n.is_open())
    }

    /// (new game) False: the UI pack cannot show the choice menu (no CutsceneText movie).
    pub fn newgame_choice_available(&self) -> bool {
        self.newgame.is_some()
    }

    /// (new game) Takes the choice menu off without a result (the host gave up the new game).
    pub fn close_newgame_choice(&mut self) {
        if let Some(newgame) = self.newgame.as_mut() {
            newgame.close();
        }
    }

    /// (new game) Where the choice menu is, for a host's log or test: (step, cursor, the words on screen, the
    /// window's entries).
    pub fn newgame_choice_state(&self) -> Option<(kh_ui::newgame::Step, usize, String, Vec<String>)> {
        self.newgame.as_ref().filter(|n| n.is_open()).map(|n| (n.step(), n.cursor(), n.shown_text(), n.entries().to_vec()))
    }

    /// (new game) The pair the player confirmed, once, when the menu has left the screen.
    /// (ER party S8) THE STARTER QUESTION before the opening's two (kh_ui::newgame::StarterQuestion; None = the game's
    /// menu). Set before [`Hud::open_newgame_choice`].
    pub fn set_newgame_starter_question(&mut self, question: Option<kh_ui::newgame::StarterQuestion>) {
        if let Some(newgame) = self.newgame.as_mut() {
            newgame.set_starter_question(question);
        }
    }

    /// (ER party S8) The starter confirmed with the last pair (0 Sora, 1 Donald, 2 Goofy), once.
    pub fn take_newgame_starter(&mut self) -> Option<usize> {
        self.newgame.as_mut().and_then(|n| n.take_starter())
    }

    // ------------------------------------------------------------------------------- (ER party S8) the talk window

    /// (ER party S8) A static UI sound (`se00000_NNN` by its index: 38 = the BONUS! fanfare) with the HUD's own sounds.
    pub fn play_ui(&mut self, index: u32) {
        self.sounds.push(UiSound::Play(index));
    }

    /// (ER party S8) The talk window can be shown (the UI pack has the Dialog movie).
    pub fn talk_available(&self) -> bool {
        self.talk.is_some()
    }

    /// (ER party S8) A speaker's line over the stage point `at` (1920 x 1080 stage pixels); A shows the rest of it, then
    /// goes on (`TalkEvent::Advanced` from [`Hud::take_talk_events`]).
    pub fn talk_say(&mut self, speaker: Option<&str>, text: &str, at: (f32, f32)) -> bool {
        let Some(w) = self.talk.as_mut() else { return false };
        w.say(speaker, text, at, &mut self.sounds);
        true
    }

    /// (ER party S8) A prompt ("[Y] Talk") over the stage point `at` (again each frame: it follows).
    pub fn talk_prompt(&mut self, text: &str, at: (f32, f32)) -> bool {
        let Some(w) = self.talk.as_mut() else { return false };
        w.prompt(text, at, &mut self.sounds);
        true
    }

    /// (ER party S8) A banner at the top centre for `seconds`.
    pub fn talk_banner(&mut self, text: &str, seconds: f32) -> bool {
        let Some(w) = self.talk.as_mut() else { return false };
        w.banner(text, seconds, &mut self.sounds);
        true
    }

    /// (ER party S8) The speaker moved on screen.
    pub fn talk_anchor(&mut self, at: (f32, f32)) {
        if let Some(w) = self.talk.as_mut() {
            w.set_anchor(at);
        }
    }

    /// (ER party S8) The window leaves (animated); `now` = off at once.
    pub fn talk_close(&mut self, now: bool) {
        if let Some(w) = self.talk.as_mut() {
            if now {
                w.clear();
            } else {
                w.close(&mut self.sounds);
            }
        }
    }

    /// (ER party S8) What is up: None, or the kind and whether a line is all shown.
    pub fn talk_state(&self) -> Option<(kh_ui::talk::TalkKind, bool)> {
        self.talk.as_ref().filter(|w| w.is_open()).map(|w| (w.kind(), w.finished()))
    }

    /// (ER party S8) The window's events since the last call.
    pub fn take_talk_events(&mut self) -> Vec<kh_ui::talk::TalkEvent> {
        self.talk.as_mut().map(|w| w.take_events()).unwrap_or_default()
    }

    /// (ER party S8) The talk window's part of a frame: its stage frames; A (decide) while a line is up.
    fn talk_frame(&mut self, f: &Frame) {
        let Some(w) = self.talk.as_mut() else { return };
        if !w.is_open() {
            return;
        }
        if f.pressed.has(kh_core::PadButton::FaceBottom) {
            w.key(kh_ui::camp::CampKey::Decide, false, &mut self.sounds);
        }
        for _ in 0..f.ticks {
            w.tick();
        }
    }

    pub fn take_newgame_choice(&mut self) -> Option<kh_core::status::OpeningChoice> {
        self.newgame_result.take()
    }

    /// (new game) The pair of a pick: the mirrors' stored values (`1stChoice` 0 Vitality, 1 Balance, 2 Wisdom;
    /// `2ndChoice` = the ability path 0 warrior, 1 mystic, 2 guardian).
    pub fn choice_of_pick(pick: kh_ui::newgame::NewGamePick) -> kh_core::status::OpeningChoice {
        use kh_core::status::{Growth, OpeningChoice, Power};
        let (first, second) = pick.values();
        OpeningChoice { desire: [Growth::Vitality, Growth::Balance, Growth::Wisdom][first.min(2) as usize], power: Power::from_path(second) }
    }

    /// (new game) The choice menu's part of a frame. True while it has the frame.
    fn newgame_frame(&mut self, f: &Frame, nav: &[(u8, bool)], words: &[String]) -> bool {
        use kh_core::PadButton as B;
        let Some(newgame) = self.newgame.as_mut().filter(|n| n.is_open()) else { return false };
        let mut keys: Vec<(CampKey, bool)> = nav.iter().map(|(dir, repeat)| ([CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3], *repeat)).collect();
        if f.pressed.has(B::FaceBottom) {
            keys.push((CampKey::Decide, false));
        }
        if f.pressed.has(B::FaceRight) {
            keys.push((CampKey::Cancel, false));
        }
        for word in words {
            match word.as_str() {
                "menu up" => keys.push((CampKey::Up, false)),
                "menu down" => keys.push((CampKey::Down, false)),
                "menu decide" => keys.push((CampKey::Decide, false)),
                "menu cancel" => keys.push((CampKey::Cancel, false)),
                _ => {}
            }
        }
        for (key, repeat) in keys {
            newgame.key(key, repeat, &mut self.sounds);
        }
        for _ in 0..f.ticks {
            if let Some(pick) = newgame.tick() {
                let choice = Self::choice_of_pick(pick);
                (self.log)(&format!("new game: chosen {} + the {} (ability path {})", choice.desire.name(), choice.power.name(), choice.power.letter()));
                self.newgame_result = Some(choice);
            }
        }
        true
    }

    /// (cutscene) THE CUTSCENE RULE, ONE INPUT: the host says EVERY FRAME, before [`Hud::frame`], whether a
    /// cutscene has the screen. While it is true:
    /// - the HUD is hidden (no layer has a picture) and its movies stand still;
    /// - the Menu button (Start) opens THE CUTSCENE PAUSE MENU (KH3's Pause screen with Continue / Skip Scene),
    ///   never the camp menu or the battle Pause screen;
    /// - the command menu is not drawn and not driven by the HUD's debug words; his own command system reads
    ///   his pad, so the host steps him with [`crate::menu::cutscene_input`] (or not at all);
    /// - [`Hud::paused`] is true while the menu is up: the host holds the cutscene still for that long;
    /// - what the menu told comes once from [`Hud::take_cutscene_pause_result`].
    /// A camp menu / Pause screen that was up when the cutscene began keeps the screen and the pad until it is
    /// closed (it cannot be opened again). When the flag goes false under the open menu (the cutscene ended by
    /// itself) the menu is taken off at once and the host is told `Resume`.
    /// (SP2) The host's reaction command in the command menu's event button (`Some(76)`: a save point's "Save"; see
    /// `host_command`), or none. Shown from the next frame; taken off with KH3's "out" when it goes.
    pub fn set_host_command(&mut self, kind: Option<i64>) {
        self.host_command = kind;
        self.host_reactions = kind.map(HostReaction::kind).into_iter().collect(); // (RC) the one-entry form
    }

    /// (RC) THE HOST'S Y INTERACTIONS (save point Save, a party member's Talk, Elden Ring's own prompts: Open, Pick Up ...),
    /// in Y's order: the first is the event button (KH3's green reaction command, selected, with the Y icon), the rest stack
    /// as KH3's situation commands under it, unselected. Empty = none. Shown from the next frame; KH3's "out" takes a gone
    /// one off.
    pub fn set_host_reactions(&mut self, list: &[HostReaction]) {
        self.host_command = list.first().and_then(|r| r.kind);
        self.host_reactions = list.to_vec();
    }

    /// (RC) The names the host's Y interactions show now, in order (the first = the green event button).
    pub fn host_reaction_names(&self) -> Vec<String> {
        self.host_reactions.iter().map(|r| self.reaction_name(r)).collect()
    }

    /// (RC) A host reaction's text: KH3's own command name when it names a command kind of the pack, else its own text.
    fn reaction_name(&self, r: &HostReaction) -> String {
        r.kind.and_then(|k| self.command_names.get(&k).cloned()).filter(|n| !n.is_empty()).unwrap_or_else(|| r.name.clone())
    }

    /// (RC) KH3's reaction command kinds of the pack (commands.json, `kh3_system.situation.*`): Talk, Open, Examine.
    pub const TALK_COMMAND: i64 = 77;
    pub const OPEN_COMMAND: i64 = 78;
    pub const EXAMINE_COMMAND: i64 = 79;

    /// (SP2) The command kind of a save point's Save (`TRES_CMD_KIND_SAVE`, the pack's commands row 76 "Save").
    pub const SAVE_COMMAND: i64 = 76;

    /// (SP2) What the command menu's event button shows now (its text), for tests and logs.
    pub fn event_button_text(&self) -> Option<String> {
        self.host_reactions.first().map(|r| self.reaction_name(r)) // (RC)
    }

    pub fn set_cutscene(&mut self, on: bool) {
        if !on {
            if self.cutscene && self.cutscene_pause.is_open() {
                self.cutscene_pause.close();
                self.cutscene_pause_result = Some(kh_ui::cutscene_pause::CutscenePauseResult::Resume);
                (self.log)("cutscene pause: the cutscene ended under the menu: Resume");
            }
            self.cutscene_skip_wait = 0;
        }
        self.cutscene = on;
    }

    /// (cutscene) The flag of [`Hud::set_cutscene`].
    pub fn cutscene(&self) -> bool {
        self.cutscene
    }

    /// (cutscene) OPENS THE CUTSCENE PAUSE MENU (a host that routes the button itself; with
    /// [`Hud::set_cutscene`] true the Menu button of `Frame::pressed` does it). False: a menu is up already.
    pub fn open_cutscene_pause(&mut self) -> bool {
        if self.flow.paused() || self.gameover.is_open() || self.savepoint.is_open() || self.cutscene_pause.is_open() {
            return false;
        }
        self.cutscene_pause.open(self.cutscene_skip_allowed, &mut self.sounds);
        (self.log)("cutscene pause: the menu opens (Continue / Skip Scene)");
        true
    }

    /// (cutscene) The cutscene pause menu is on screen (its way out included).
    pub fn cutscene_pause_open(&self) -> bool {
        self.cutscene_pause.is_open()
    }

    /// (cutscene) What the cutscene pause menu told the host, once, when it has left the screen (the pause is
    /// over): `Resume` = the cutscene goes on, `SkipScene` = the host ends the cutscene.
    pub fn take_cutscene_pause_result(&mut self) -> Option<kh_ui::cutscene_pause::CutscenePauseResult> {
        self.cutscene_pause_result.take()
    }

    /// (cutscene) The cutscene's part of a frame. True while the cutscene (or its pause menu) has the frame:
    /// nothing else of the HUD runs.
    fn cutscene_frame(&mut self, f: &Frame, nav: &[(u8, bool)], words: &[String]) -> bool {
        use kh_core::PadButton as B;
        let menu_word = words.iter().any(|w| matches!(w.as_str(), "menu" | "menu open" | "menu menu"));
        if !self.cutscene_pause.is_open() {
            if !self.cutscene || self.flow.paused() {
                // No cutscene; or a menu that was up when it began: that one keeps the frame.
                return false;
            }
            // (a press of the frame before the cutscene must not open the camp menu after it)
            self.flow.phase = Phase::Playing;
            self.cutscene_skip_wait = self.cutscene_skip_wait.saturating_sub(f.ticks);
            if (f.pressed.has(B::SpecialRight) || menu_word) && self.cutscene_skip_wait == 0 {
                self.open_cutscene_pause();
            }
            return true;
        }
        let mut keys: Vec<(CampKey, bool)> = nav.iter().map(|(dir, repeat)| ([CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3], *repeat)).collect();
        for (button, key) in [(B::FaceBottom, CampKey::Decide), (B::FaceRight, CampKey::Cancel), (B::SpecialRight, CampKey::Menu)] {
            if f.pressed.has(button) {
                keys.push((key, false));
            }
        }
        for word in words {
            match word.as_str() {
                "menu up" => keys.push((CampKey::Up, false)),
                "menu down" => keys.push((CampKey::Down, false)),
                "menu decide" => keys.push((CampKey::Decide, false)),
                "menu cancel" => keys.push((CampKey::Cancel, false)),
                "menu" | "menu open" | "menu menu" => keys.push((CampKey::Menu, false)),
                _ => {}
            }
        }
        for (key, repeat) in keys {
            self.cutscene_pause.key(key, repeat, &mut self.sounds);
        }
        for _ in 0..f.ticks {
            if let Some(result) = self.cutscene_pause.tick() {
                (self.log)(&format!("cutscene pause: {result:?}"));
                if result == kh_ui::cutscene_pause::CutscenePauseResult::SkipScene {
                    self.cutscene_skip_wait = CUTSCENE_SKIP_WAIT;
                }
                self.cutscene_pause_result = Some(result);
            }
        }
        true
    }

    /// (saveload) OPENS THE SAVE SCREEN ("Select a slot to save to.") over `slots` (one summary per slot of the
    /// host, in list order; the screen reads nothing itself). `current`: the slot the running game belongs to
    /// (the cursor starts there, as the game starts on the slot of the last save); `None`: the summaries' own
    /// `is_current`. Opened right after the save-point menu's Save was chosen
    /// ([`Hud::take_savepoint_result`]) it comes out of that menu, and a cancel reopens the menu
    /// ([`Hud::savepoint_open`] is true again; the host is still told `Cancel`). While it is up
    /// [`Hud::paused`] is true and the pad drives it: D-pad / left stick up and down (the menus' repeat; a
    /// fresh press wraps), LB / RB one page, A decide, B back. Results: [`Hud::take_saveload_result`].
    /// False: another menu is up, or the UI pack cannot show the screen ([`Hud::saveload_available`]).
    pub fn open_save(&mut self, slots: &[kh_ui::saveload::SlotSummary], current: Option<usize>) -> bool {
        if !self.saveload_may_open() {
            return false;
        }
        let from_menu = std::mem::take(&mut self.save_armed);
        let slots = Self::with_current(slots, current);
        let background = self.saveload_background;
        let Some(screen) = self.saveload.as_mut() else { return false };
        screen.open_save(&slots, from_menu, background, &mut self.sounds);
        self.saveload_result = None;
        self.saveload_return = if from_menu { SaveLoadReturn::SavePoint } else { SaveLoadReturn::Nowhere };
        (self.log)(&format!("save / load: the Save screen opens ({} slots, {} used)", slots.len(), slots.iter().filter(|s| s.is_used()).count()));
        true
    }

    /// (saveload) OPENS THE LOAD SCREEN ("Select a slot to load from.") over `slots`. It needs no world and no
    /// live Sora: a host on its title screen drives it with [`Hud::saveload_frame`] (or with [`Hud::frame`] and
    /// a stand-in) and sets [`Hud::saveload_background`]. Opened right after the game-over menu's Load was
    /// chosen ([`Hud::take_gameover_choice`]) a cancel brings the game-over menu back. An unused slot refuses;
    /// a used one is loaded without a question (as the game). Results: [`Hud::take_saveload_result`].
    pub fn open_load(&mut self, slots: &[kh_ui::saveload::SlotSummary]) -> bool {
        if !self.saveload_may_open() {
            return false;
        }
        let from_gameover = std::mem::take(&mut self.load_armed);
        let from_point = std::mem::take(&mut self.point_load_armed); // (SV)
        let background = self.saveload_background;
        let Some(screen) = self.saveload.as_mut() else { return false };
        screen.open_load(slots, background, &mut self.sounds);
        self.saveload_result = None;
        self.saveload_return = if from_gameover { SaveLoadReturn::GameOver } else if from_point { SaveLoadReturn::SavePointLoad } else { SaveLoadReturn::Nowhere };
        (self.log)(&format!("save / load: the Load screen opens ({} slots, {} used)", slots.len(), slots.iter().filter(|s| s.is_used()).count()));
        true
    }

    fn saveload_may_open(&self) -> bool {
        !(self.flow.paused() || self.gameover.is_open() || self.savepoint.is_open() || self.cutscene_pause.is_open() || self.newgame_choice_open() || self.saveload_open())
    }

    fn with_current(slots: &[kh_ui::saveload::SlotSummary], current: Option<usize>) -> Vec<kh_ui::saveload::SlotSummary> {
        let mut slots = slots.to_vec();
        if let Some(current) = current {
            for slot in &mut slots {
                slot.is_current = slot.index == current;
            }
        }
        slots
    }

    /// (saveload) A Save or Load screen is on screen (its way out and its messages included).
    pub fn saveload_open(&self) -> bool {
        self.saveload.as_ref().is_some_and(|s| s.is_open())
    }

    /// (saveload) False: the UI pack cannot show the screens.
    pub fn saveload_available(&self) -> bool {
        self.saveload.is_some()
    }

    /// (SV) THE SCREENS' HOST SWITCHES (kept across opens): `delete` = KH3's Delete (Y on a used row, Yes / No, then the host
    /// is told `SaveLoadResult::Delete(slot)`); `delete_current` = the running game's own slot may be deleted too (else that row
    /// refuses with a message); `confirm_load` = a used load row asks "Load the save data in this slot?" first.
    pub fn set_saveload_options(&mut self, delete: bool, delete_current: bool, confirm_load: bool) {
        if let Some(screen) = self.saveload.as_mut() {
            (screen.delete_enabled, screen.delete_current, screen.confirm_load) = (delete, delete_current, confirm_load);
        }
    }

    /// (saveload) Where the screen is (mode, phase, the row under the cursor, the message on screen), for a
    /// host's log or test.
    pub fn saveload_state(&self) -> Option<(kh_ui::saveload::SaveLoadMode, kh_ui::saveload::Phase, usize, Option<String>)> {
        self.saveload.as_ref().filter(|s| s.is_open()).map(|s| (s.mode(), s.phase(), s.cursor(), s.message()))
    }

    /// (saveload) What the screen told the host, once: `Save(slot)` (an overwrite was confirmed) or `Load(slot)`
    /// with the host's slot number (`SlotSummary::index`): the screen shows the game's "Saving..." /
    /// "Loading..." and waits for [`Hud::set_saveload_busy`]; or `Cancel`: the screen is gone (after a Save
    /// screen that came out of the save-point menu that menu is open again; after a Load screen that came out
    /// of the game-over menu that one comes back).
    pub fn take_saveload_result(&mut self) -> Option<kh_ui::saveload::SaveLoadResult> {
        self.saveload_result.take()
    }

    /// (saveload) THE HOST'S REPORT on the `Save` / `Load` it was told to carry out. `Working`: still at it
    /// (the state right after the result: need not be said). `Done`: a save -> the game's sound, the message
    /// leaves and the list has the pad again (give the new summaries FIRST: [`Hud::set_saveload_slots`]); a
    /// load -> the screen leaves (the pause ends when it is gone). `Failed`: the game's failure message with
    /// OK, then the list again. The message stays at least `kh_ui::saveload::MESSAGE_MIN_FRAMES`.
    pub fn set_saveload_busy(&mut self, progress: kh_ui::saveload::Progress) {
        if let Some(screen) = self.saveload.as_mut() {
            screen.set_progress(progress);
        }
    }

    /// (saveload) New summaries for the rows of the open screen (the slot just written). The cursor stays.
    pub fn set_saveload_slots(&mut self, slots: &[kh_ui::saveload::SlotSummary], current: Option<usize>) {
        let slots = Self::with_current(slots, current);
        if let Some(screen) = self.saveload.as_mut() {
            screen.set_slots(&slots);
        }
    }

    /// (saveload) Takes the screen off at once, without a result (the host gave up: a load began by other means).
    pub fn close_saveload(&mut self) {
        if let Some(screen) = self.saveload.as_mut() {
            screen.close();
        }
        self.saveload_return = SaveLoadReturn::Nowhere;
    }

    /// (saveload) THE SCREEN'S PART OF A FRAME, callable WITHOUT a [`Frame`] (a host on its title screen: no
    /// world, no Sora): the pad buttons pressed and the direction events of [`crate::menu::MenuInput`], script
    /// words (`menu up | down | decide | cancel | l1 | r1`), and the stage frames (1/60 s) that passed. True
    /// while the screen has the frame. [`Hud::frame`] calls it first, so a host that runs `frame` needs nothing else.
    pub fn saveload_frame(&mut self, pressed: kh_core::PadButtons, nav: &[(u8, bool)], words: &[String], ticks: u32) -> bool {
        use kh_core::PadButton as B;
        let Some(screen) = self.saveload.as_mut().filter(|s| s.is_open()) else { return false };
        let mut keys: Vec<(CampKey, bool)> = nav.iter().map(|(dir, repeat)| ([CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3], *repeat)).collect();
        // (SV) Y = KH3's Triangle: Delete
        for (button, key) in [(B::FaceBottom, CampKey::Decide), (B::FaceRight, CampKey::Cancel), (B::LeftShoulder, CampKey::L1), (B::RightShoulder, CampKey::R1), (B::FaceTop, CampKey::Triangle)] {
            if pressed.has(button) {
                keys.push((key, false));
            }
        }
        for word in words {
            match word.as_str() {
                "menu up" => keys.push((CampKey::Up, false)),
                "menu down" => keys.push((CampKey::Down, false)),
                "menu decide" => keys.push((CampKey::Decide, false)),
                "menu cancel" => keys.push((CampKey::Cancel, false)),
                "menu l1" => keys.push((CampKey::L1, false)),
                "menu r1" => keys.push((CampKey::R1, false)),
                "menu triangle" | "menu delete" => keys.push((CampKey::Triangle, false)), // (SV)
                _ => {}
            }
        }
        let mut results = Vec::new();
        for (key, repeat) in keys {
            results.extend(screen.key(key, repeat, &mut self.sounds));
        }
        for _ in 0..ticks {
            results.extend(screen.tick(&mut self.sounds));
        }
        for result in results {
            (self.log)(&format!("save / load: {result:?}"));
            if result == kh_ui::saveload::SaveLoadResult::Cancel {
                match std::mem::replace(&mut self.saveload_return, SaveLoadReturn::Nowhere) {
                    // (0x1416F7F00: the save-point menu's `inBack`)
                    SaveLoadReturn::SavePoint => {
                        self.savepoint.reopen_back();
                        self.savepoint_result = None;
                    }
                    // (SV)
                    SaveLoadReturn::SavePointLoad => {
                        self.savepoint.reopen_back_at(kh_ui::savepoint::SavePointCommand::Load);
                        self.savepoint_result = None;
                    }
                    // (he is still dead: the game-over menu opens again at the next frame)
                    SaveLoadReturn::GameOver => {
                        self.gameover_done = false;
                        // (game over) the sequence's menu opens again over the void
                        if self.gameover_seq.as_mut().is_some_and(|s| s.reopen()) {
                            self.open_gameover_menu();
                        }
                    }
                    SaveLoadReturn::Nowhere => {}
                }
            }
            self.saveload_result = Some(result);
        }
        true
    }

    /// (savepoint) OPENS THE SAVE-POINT MENU (the host does: he stands in a save point's reactor and pressed the
    /// button of the Save command). `disabled`: the commands this host cannot carry out. False: another menu is up.
    pub fn open_savepoint(&mut self, disabled: &[kh_ui::savepoint::SavePointCommand]) -> bool {
        if self.flow.paused() || self.gameover.is_open() || self.savepoint.is_open() || self.saveload_open() {
            return false;
        }
        self.save_armed = false; // (saveload)
        self.point_load_armed = false; // (SV)
        self.savepoint.confirm_title = self.title_entry;
        self.savepoint.open(&kh_ui::savepoint::SavePointCommand::ALL, disabled);
        self.sounds.push(UiSound::Play(kh_ui::menu::SE_MENU_OPEN));
        true
    }

    /// (er9) As [`Hud::open_savepoint`] for a host with its own list: `commands` from the top, and other
    /// (name, help line) texts for some of them (the Elden Ring host: Save, the map, the site of grace).
    pub fn open_savepoint_with(&mut self, commands: &[kh_ui::savepoint::SavePointCommand], disabled: &[kh_ui::savepoint::SavePointCommand], texts: &[(kh_ui::savepoint::SavePointCommand, &str, &str)]) -> bool {
        if self.flow.paused() || self.gameover.is_open() || self.savepoint.is_open() || self.saveload_open() {
            return false;
        }
        self.save_armed = false; // (saveload)
        self.point_load_armed = false; // (SV)
        self.savepoint.confirm_title = self.title_entry;
        self.savepoint.open_with_texts(commands, disabled, texts);
        self.sounds.push(UiSound::Play(kh_ui::menu::SE_MENU_OPEN));
        true
    }

    pub fn savepoint_open(&self) -> bool {
        self.savepoint.is_open()
    }

    /// (er-host, IMPL_ER26) THE SAVE-POINT MENU OF A HOST AT ANOTHER GAME'S RESTING PLACES (the module savepoint_menu): its entries
    /// in their order with their texts and icon cells, Pass Time with its choice window. False: another menu is up.
    pub fn open_savepoint_grace(&mut self, menu: crate::savepoint_menu::GraceMenu) -> bool {
        let entries = crate::savepoint_menu::entries(menu);
        self.savepoint.host_icons = entries.iter().map(|e| (e.command, e.icon.to_string())).collect();
        self.savepoint.host_choices = vec![crate::savepoint_menu::pass_time_choice()];
        let commands: Vec<kh_ui::savepoint::SavePointCommand> = entries.iter().map(|e| e.command).collect();
        let texts: Vec<(kh_ui::savepoint::SavePointCommand, &str, &str)> = entries.iter().filter_map(|e| Some((e.command, e.name?, kh_gendata::text_or(e.help?)))).collect();
        self.open_savepoint_with(&commands, &[], &texts)
    }

    /// (er-host, IMPL_ER26) The button chosen in a choice window of the save-point menu (Pass Time's), once; it goes with
    /// the Chosen result of that command.
    pub fn take_savepoint_choice(&mut self) -> Option<usize> {
        self.savepoint.take_choice()
    }

    /// (savepoint) What the save-point menu told the host, once: a command chosen (the menu has closed; the host
    /// carries it out) or closed by cancel. Either way the pause is over: the host refills the pouch (`Sora::on_unpause`).
    pub fn take_savepoint_result(&mut self) -> Option<kh_ui::savepoint::SavePointResult> {
        self.savepoint_result.take()
    }

    /// (savepoint) The save-point menu's part of a frame: the pad drives it (up / down / A / B). True while it has the frame.
    fn save_point(&mut self, f: &Frame, nav: &[(u8, bool)], words: &[String]) -> bool {
        use kh_core::PadButton as B;
        if !self.savepoint.is_open() {
            return false;
        }
        let mut keys: Vec<(CampKey, bool)> = nav.iter().map(|(dir, repeat)| ([CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3], *repeat)).collect();
        if f.pressed.has(B::FaceBottom) {
            keys.push((CampKey::Decide, false));
        }
        if f.pressed.has(B::FaceRight) {
            keys.push((CampKey::Cancel, false));
        }
        for word in words {
            match word.as_str() {
                "menu up" => keys.push((CampKey::Up, false)),
                "menu down" => keys.push((CampKey::Down, false)),
                "menu decide" => keys.push((CampKey::Decide, false)),
                "menu cancel" => keys.push((CampKey::Cancel, false)),
                _ => {}
            }
        }
        for (key, repeat) in keys {
            if let Some(result) = self.savepoint.key(key, repeat, &mut self.sounds) {
                self.savepoint_result = Some(result);
            }
        }
        for _ in 0..f.ticks {
            if let Some(result) = self.savepoint.tick() {
                (self.log)(&format!("save point: {result:?}"));
                // (saveload) Save was chosen: a Save screen opened next comes out of this menu and leads back to it
                self.save_armed = result == kh_ui::savepoint::SavePointResult::Chosen(kh_ui::savepoint::SavePointCommand::Save);
                // (SV) ... Load the same for a Load screen
                self.point_load_armed = result == kh_ui::savepoint::SavePointResult::Chosen(kh_ui::savepoint::SavePointCommand::Load);
                self.savepoint_result = Some(result);
            }
        }
        true
    }

    /// THE GAME-OVER MENU's part of a frame (docs/sora-gameover-spec.md 3). His Die state runs: after
    /// [`GAMEOVER_MENU_DELAY`] stage frames the menu opens (menu type 0: Continue / Load / Title Screen), the
    /// pad drives it (up / down / A), and the button chosen is the host's to carry out
    /// ([`Hud::take_gameover_choice`]). True while the menu has the frame (nothing else of the HUD runs).
    fn game_over(&mut self, f: &Frame, nav: &[(u8, bool)], words: &[String]) -> bool {
        use kh_core::PadButton as B;
        // (game over) a sequence the host opened has the frame (the older automatic menu below is then not used)
        if self.gameover_seq.is_some() {
            return self.game_over_sequence(f, nav, words);
        }
        if !self.gameover_on || f.state != kh_core::StateId::Die {
            if self.gameover.is_open() {
                self.gameover.close();
            }
            (self.dead_frames, self.gameover_done) = (0, false);
            return false;
        }
        self.dead_frames += f.ticks;
        if self.gameover_done {
            return false;
        }
        if !self.gameover.is_open() {
            if self.dead_frames < GAMEOVER_MENU_DELAY {
                return false;
            }
            // (a menu that was up when he died is gone: the pause must not outlive him)
            let disabled = self.gameover_disabled.clone();
            self.gameover.open_type(0, &disabled, "");
            self.sounds.push(UiSound::Play(kh_ui::menu::SE_MENU_OPEN));
        }
        let mut keys: Vec<(CampKey, bool)> = nav.iter().map(|(dir, repeat)| ([CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3], *repeat)).collect();
        if f.pressed.has(B::FaceBottom) {
            keys.push((CampKey::Decide, false));
        }
        for word in words {
            match word.as_str() {
                "menu up" => keys.push((CampKey::Up, false)),
                "menu down" => keys.push((CampKey::Down, false)),
                "menu decide" => keys.push((CampKey::Decide, false)),
                _ => {}
            }
        }
        for (key, repeat) in keys {
            self.gameover.key(key, repeat, &mut self.sounds);
        }
        for _ in 0..f.ticks {
            if let Some(button) = self.gameover.tick() {
                (self.log)(&format!("game over: {button:?} was chosen"));
                // (saveload) Load was chosen: a Load screen opened next leads back to this menu on cancel
                self.load_armed = button == kh_ui::gameover::GameOverButton::Load;
                self.gameover_choice = Some(button);
                self.gameover_done = true;
            }
        }
        true
    }

    /// (game over) OPENS KH3'S GAME-OVER SEQUENCE (crate::gameover; docs/sora-gameover-spec.md): the host calls it
    /// at the moment Sora dies. From then on, in real time: the fade plate rises over the world and the cockpit
    /// (1.0 s), the void scene is set up behind it (the world is held: [`Hud::paused`]), a second later the menu
    /// of `setup.variant` opens with the hint and the plate leaves (0.5 s); the pad drives the menu (D-pad /
    /// left stick up and down, A); the choice comes from [`Hud::take_gameover_result`] when the plate has
    /// returned (0.5 s; Load: at once, for [`Hud::open_load`]); the host carries it out and calls
    /// [`Hud::end_gameover`]. What to draw each frame: [`Hud::gameover_view`]. False: a sequence runs already.
    /// The older automatic menu (`gameover_on`: Continue / Load / Title Screen 3 s after the Die state began)
    /// is not used while a sequence runs.
    pub fn open_gameover(&mut self, setup: crate::gameover::GameOverSetup) -> bool {
        if self.gameover_seq.is_some() {
            return false;
        }
        if self.gameover.is_open() {
            self.gameover.close();
        }
        (self.dead_frames, self.gameover_done, self.gameover_choice, self.gameover_result) = (0, false, None, None);
        if setup.dead_se {
            let index = if setup.kind == crate::gameover::GameOverKind::MissionFailed { crate::gameover::SE_MISSION_FAILED_LOOP } else { crate::gameover::SE_GAMEOVER_LOOP };
            self.sounds.push(UiSound::LoopStart(index));
            self.gameover_loop = Some(index);
        }
        (self.log)(&format!("game over: the sequence begins (menu {:?}, hint {}, plate rgb {:?}, {:?}{})", setup.variant, if setup.hint.is_some() { "yes" } else { "none" }, setup.fade.colour, setup.kind, if setup.heart { ", heart effect" } else { "" }));
        self.gameover_seq = Some(crate::gameover::Sequence::new(setup));
        true
    }

    /// (game over) A hint text of the game by its id (`general_0010`, `enemy_3010` ...: the rows of the game's
    /// table `UI/Data/GameOverHintData` name them): what [`crate::gameover::GameOverSetup::hint`] takes.
    pub fn gameover_hint(&self, id: &str) -> Option<String> {
        self.gameover_hints.get(id.trim_start_matches("kh3_system.gameoverhint.")).cloned()
    }

    /// (game over) The ids of the game's hint texts with a prefix (`general_`: the hints that name no fight), sorted.
    pub fn gameover_hint_ids(&self, prefix: &str) -> Vec<String> {
        let mut ids: Vec<String> = self.gameover_hints.keys().filter(|id| id.starts_with(prefix)).cloned().collect();
        ids.sort();
        ids
    }

    /// (game over) A sequence runs (from [`Hud::open_gameover`] until its last plate has left).
    pub fn gameover_active(&self) -> bool {
        self.gameover_seq.is_some()
    }

    /// (game over) WHAT TO DRAW THIS FRAME, as plain data: the plate (0..1 and its colour: over the world, the
    /// cockpit and the void scene, under the menu's layer), and while the void scene is up its pose source
    /// (sequence, time, face row), camera and lights. `None`: no sequence.
    pub fn gameover_view(&self) -> Option<crate::gameover::GameOverView> {
        self.gameover_seq.as_ref().map(|s| s.view(self.gameover.is_open()))
    }

    /// (game over) The choice of the sequence's menu, once: `Retry | PrepareAndRetry | Continue | Load |
    /// TitleScreen` (and the game's other buttons of a variant). The plate is full (black) when it comes, except
    /// for Load, which comes at once: open the Load screen in the same frame ([`Hud::open_load`]; its cancel
    /// brings the menu back). The host carries the choice out, then calls [`Hud::end_gameover`].
    pub fn take_gameover_result(&mut self) -> Option<kh_ui::gameover::GameOverResult> {
        self.gameover_result.take()
    }

    /// (game over) The host has carried the choice out (Sora stands again, a save was loaded ...): the void scene
    /// goes and the plate leaves over the world in 0.5 s (`fade_in` false: at once, e.g. under a full-screen
    /// menu or the host's own loading screen). Also ends a sequence early.
    pub fn end_gameover(&mut self, fade_in: bool) {
        if self.gameover.is_open() {
            self.gameover.close();
        }
        if let Some(index) = self.gameover_loop.take() {
            self.sounds.push(UiSound::LoopStop(index));
        }
        if let Some(seq) = self.gameover_seq.as_mut() {
            seq.end(fade_in);
            if seq.finished() {
                self.gameover_seq = None;
            }
        }
    }

    /// (game over) The sequence's menu: the variant's buttons, the hint, the open sound.
    fn open_gameover_menu(&mut self) {
        let Some(seq) = self.gameover_seq.as_ref() else { return };
        let (menu_type, disabled, hint) = (seq.setup.variant.menu_type(), seq.setup.disabled.clone(), seq.setup.hint.clone().unwrap_or_default());
        self.gameover.open_type(menu_type, &disabled, &hint);
        self.sounds.push(UiSound::Play(kh_ui::menu::SE_MENU_OPEN));
    }

    /// (game over) The sequence's part of a frame. True while the void scene has the frame (nothing else of
    /// the HUD runs); false while the plate rises or leaves over the world (the cockpit goes on under it).
    fn game_over_sequence(&mut self, f: &Frame, nav: &[(u8, bool)], words: &[String]) -> bool {
        use kh_core::PadButton as B;
        if self.gameover.is_open() {
            let mut keys: Vec<(CampKey, bool)> = nav.iter().map(|(dir, repeat)| ([CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3], *repeat)).collect();
            if f.pressed.has(B::FaceBottom) {
                keys.push((CampKey::Decide, false));
            }
            for word in words {
                match word.as_str() {
                    "menu up" => keys.push((CampKey::Up, false)),
                    "menu down" => keys.push((CampKey::Down, false)),
                    "menu decide" => keys.push((CampKey::Decide, false)),
                    _ => {}
                }
            }
            for (key, repeat) in keys {
                self.gameover.key(key, repeat, &mut self.sounds);
            }
        }
        for _ in 0..f.ticks {
            let step = self.gameover_seq.as_mut().map_or(crate::gameover::Step::None, |s| s.tick());
            if step == crate::gameover::Step::OpenMenu {
                self.open_gameover_menu();
                (self.log)("game over: the menu opens, the plate leaves");
            }
            if let Some(button) = self.gameover.tick() {
                let result = button.result();
                (self.log)(&format!("game over: {result:?} was chosen"));
                // (the loop SE ends with the choice, 0x140EA4E90; under the Load screen it goes on)
                if result != kh_ui::gameover::GameOverResult::Load {
                    if let Some(index) = self.gameover_loop.take() {
                        self.sounds.push(UiSound::LoopStop(index));
                    }
                }
                if let Some(seq) = self.gameover_seq.as_mut() {
                    seq.choose(result);
                }
            }
        }
        let Some(seq) = self.gameover_seq.as_mut() else { return false };
        if let Some(result) = seq.take_result() {
            // (saveload) a Load screen opened next leads back to this menu on cancel
            self.load_armed = result == kh_ui::gameover::GameOverResult::Load;
            self.gameover_result = Some(result);
        }
        if seq.finished() {
            self.gameover_seq = None;
            return false;
        }
        seq.scene_on()
    }

    /// (er-host, IMPL_ER15) OPENS THE CAMP MENU DIRECTLY (not the battle Pause screen, whatever the battle flag or his
    /// state say): what KH3`s "Prepare & Retry" leads to. It opens within the next two frames of [`Hud::frame`] (the
    /// Menu button`s own flow with a clear gate). False: a menu or screen is up already.
    /// (er-host, IMPL_ER17) ANOTHER SAVE WAS LOADED INSIDE THE SESSION: the popups of the old one (obtained items, munny, next
    /// level, level up, bonus) and its enemy gauge are taken down at once; nothing else of the HUD is touched.
    // ------------------------------------------------------------------------- (party) Donald and Goofy

    /// (party) THE FRIENDS IN THE STATUS HUD this frame (contract F.1): slots 1 - 2; empty = Sora alone (the solo
    /// layout). The HUD's driver does the rest (gauges, faces and their looks, the layout switch).
    /// (GOOFY-GG) GOOFY'S GUARD BAR this frame: his meter and the look (kh-partyhost `PartyRuntime::leader_guard_hud`);
    /// None = no bar. Drawn only while Goofy is the leader in the main slot.
    pub fn set_guard(&mut self, guard: Option<(kh_ui::hud_guard::GuardValues, kh_ui::hud_guard::GuardLook)>) {
        self.guard = guard;
    }

    /// (DON-UI) DONALD'S HUD THIS FRAME (kh-partyhost `PartyRuntime::don_hud_view`): his Resonance ring, stars,
    /// tracker, target statuses, the Scan gate, the ally target list and Quick Cast's mark. None = not Donald leading.
    pub fn set_don(&mut self, don: Option<crate::party::DonHudView>) {
        if let Some(look) = don.as_ref().and_then(|d| d.look.clone()) {
            self.don_look = look;
        }
        self.don = don;
    }

    /// (DON-UI) The look of Donald's pieces (sheet `party-don-hud`).
    pub fn set_don_look(&mut self, look: crate::party::DonLook) {
        self.don_look = look;
    }

    /// (DON-UI) What Donald's pieces last applied (tests): the rings (fill frame, pips lit, MAX), the ultimate marks, the
    /// stars (lit, blinking, tracker lit, tracker size), the target icons (statuses, insight, position), shown flags.
    #[allow(clippy::type_complexity)]
    pub fn don_state(&self) -> (Vec<(u32, u8, bool)>, Vec<kh_ui::hud_resonance::UltLook>, (u8, bool, u8, u8), (usize, usize, (f32, f32)), [bool; 3]) {
        (self.don_ring.last.clone(), self.don_ring.last_ult.clone(), self.don_ring.last_stars, self.don_icons.last, [self.don_ring.shown, self.don_ring.last_stars_plate.is_some(), self.don_icons.shown])
    }

    /// (DON-UI) The layout drawn (tests): the badges' plates and the stars row's plate (left, top, right, bottom), `sttsPc`
    /// units, and the MP / HP track frames (where the bars start).
    #[allow(clippy::type_complexity)]
    pub fn don_layout(&self) -> (Vec<(f32, f32, f32, f32)>, Option<(f32, f32, f32, f32)>, (i32, i32)) {
        let f = self.status_driver.frames();
        (self.don_ring.last_plates.iter().copied().chain(self.don_ring.last_zetta_plate).collect(), self.don_ring.last_stars_plate, (f.0 - 1, f.2 - 1))
    }

    /// (DON-UI) The Ready ultimate marks (tests): where their glow and glint visibly reach, and the marks' own boxes
    /// (left, top, right, bottom), `sttsPc` units.
    #[allow(clippy::type_complexity)]
    pub fn don_ult_glow(&self) -> (Vec<(f32, f32, f32, f32)>, Vec<(f32, f32, f32, f32)>) {
        (self.don_ring.last_glow.clone(), self.don_ring.last_ult_box.clone())
    }

    /// (DON-UI) The Y rows' star cost drawn: (AS slot, stars, lit) (tests).
    pub fn don_y_costs(&self) -> Vec<(usize, u8, bool)> {
        self.don_ycost.last.clone()
    }

    /// (DON-UI) The worn-spell icons drawn: (status slot, centre x, centre y) in `sttsPc` units (tests).
    pub fn don_worn(&self) -> Vec<(usize, f32, f32)> {
        self.don_ring.last_worn.clone()
    }

    /// (DON-UI) Each Resonance badge's (icon centre x, first pip's left x, centre y) in `sttsPc` units (tests).
    pub fn don_badges(&self) -> Vec<(f32, f32, f32)> {
        self.don_ring.last_place.clone()
    }

    /// (DON-UI) Donald's pieces after the stage frames: the ring on the status HUD (while he is the leader in the main
    /// slot), the stars and the tracker by the command menu, the statuses over the lock-on cursor, Quick Cast's lit
    /// shortcut name, the sub lists' colours of his menu.
    fn don_frame(&mut self, dt: f32, menu: kh_core::command::MenuState) {
        let shortcut_open = menu == kh_core::command::MenuState::Shortcut;
        let donald = self.leader == crate::party::PartyChara::Donald && self.leader_anim.is_none();
        let don = self.don.as_ref().filter(|_| donald);
        self.command.sub_colors = don.map(|_| ["", "mahou", "mahou", "item"]);
        // NO FOCUS / SHOTLOCK GAUGE FOR DONALD (the user, 2026-10-22: "remove the shotlock gauges"): the leader slot's focus
        // gauge stays hidden while he leads; given back to the HUD's driver when he does not
        let ex = "sttsPc.pcEx";
        if don.is_some() {
            self.status.movie.set_visible(ex, false);
            self.don_ex_hidden = true;
        } else if std::mem::take(&mut self.don_ex_hidden) {
            if let Some(n) = self.status.movie.node_mut(ex) {
                n.script_visible = None;
            }
        }
        // the worn spells by each wearer's portrait: his seat now (0 the leader's slot, 1 / 2 the friends')
        let ring = don.map(|d| {
            let mut r = d.ring.clone();
            r.worn = d.worn.iter().filter_map(|(c, b, o)| self.seats.iter().position(|s| *s == Some(*c)).filter(|i| *i < 3).map(|i| (i, *b, *o))).collect();
            r.leader_hp_frames = self.status_driver.frames().0 - 1; // where his HP bar starts (the leader's worn icon)
            // the Arcane Flow stars and the Zettaflare tracker: in this cluster (the user, 2026-10-22: "move all of the
            // pips to the healthbar part of the HUD")
            r.stars = Some(kh_ui::hud_resonance::StarsLook { lit: d.arcane.stars, max: d.arcane.stars_max, left: d.arcane.star_left, zetta: d.arcane.zetta.clone() });
            r
        });
        self.don_ring.update(&mut self.status, &self.don_art, ring.as_ref(), &self.don_look.ring, &self.don_look.arcane, dt);
        // the stars stand by the Y command; a sub list opens over that place: they step aside while it is open
        let list_open = menu == kh_core::command::MenuState::SubSel;
        // (the stars moved to the status HUD's cluster: none by the command menu any more)
        let _ = list_open;
        self.don_stars.update(&mut self.command, &self.don_art, None, &self.don_look.arcane, dt);
        // his Y rows: their star cost in gold stars where the hat sits (the user, 2026-10-22: "the Y magic commands need
        // to indicate how many stars they cost")
        let shown = self.display.situation_buttons();
        let costs: Vec<(i32, u8, bool)> = don.map_or(Vec::new(), |d| d.y_costs.iter().filter_map(|(serial, n, on)| shown.iter().find(|s| s.0 == *serial).map(|s| (s.1, *n, *on))).collect());
        self.don_ycost.update(&mut self.command, &self.don_art, &costs, &self.don_look.arcane);
        let cursor = self.target_driver.cursors.iter().find(|c| c.ty == kh_ui::hud_target::LOCKON_TYPE_NORMAL).map(|c| kh_ui::hud_target::cursor_path(c.ty, c.index));
        self.don_icons.update(&mut self.target, &self.don_art, cursor.as_deref(), don.and_then(|d| d.target.as_ref()), &self.don_look.arcane);
        // Quick Cast: the reacting spell's name lit on the panel (KH3's third name colour), restored when it moves
        let glow = don.and_then(|d| d.quick_glow).filter(|_| shortcut_open);
        if glow != self.don_glow {
            if let Some(old) = self.don_glow.filter(|_| shortcut_open) {
                self.command.native_shortcut_name(old, "col1");
            }
            if let Some(new) = glow {
                self.command.native_shortcut_name(new, "col3");
            }
            self.don_glow = glow;
        }
    }

    /// (GOOFY-GG) The GUARD bar's last applied frames (track, fill, under-fill, broken) and whether it is up (tests).
    pub fn guard_bar_state(&self) -> ((i32, i32, i32, bool), bool) {
        (self.guard_bar.last, self.guard_bar.shown)
    }

    pub fn set_party(&mut self, slots: &[crate::party::PartyHudSlot]) {
        self.party.clear();
        self.party.extend_from_slice(slots);
    }

    /// (party P) THE LEADER in the main status slot: his face movie and gauges there, Sora's face (his NPC face
    /// when the pack has it, else his own) in the leader's friend slot; `sora_in` false = Sora's slot empty.
    ///
    /// THE SWITCH ANIMATES (the user's decision 2026-10-15, "KH style shrink and grow"; OUR DESIGN on the movie's own
    /// clips, KH3 has no leader swap): the main slot and the slot of the incoming leader play their `out` labels
    /// (KH3's `setStatus(11, slot)`: the slot clips shrink away), [`LEADER_OUT_FRAMES`] stage frames later the faces
    /// and gauges are swapped and every slot plays `in` (`setStatus(10, slot)`: they grow back); the camp menu's
    /// member-switch sound (static SE 2) goes with it. Sora's leaving / joining alone needs no swap: his slot's own
    /// `out` / `in` (the native rule when a slot's pawn goes or comes).
    pub fn set_leader(&mut self, leader: crate::party::PartyChara, sora_in: bool) {
        if self.leader_anim.is_some_and(|a| a.0 == leader && a.1 == sora_in) {
            return;
        }
        if leader == self.leader && sora_in == self.sora_in {
            self.leader_anim = None;
            return;
        }
        if leader != self.leader && self.status_driver.faces[0].is_some() && self.leader_anim.is_none() {
            let old = self.leader;
            // (PF2) the incoming leader's slot now; one coming from outside the HUD: the slot the old leader moves to
            let slot = self.seat_of(leader).or_else(|| self.layout(leader, sora_in).iter().position(|s| *s == Some(old)));
            self.status.set_status(11, 0);
            if let Some(slot) = slot.filter(|s| *s != 0) {
                self.status.set_status(11, slot as i32);
            }
            self.sounds.push(UiSound::Play(kh_ui::menu::SE_MENU_OPEN));
            self.leader_anim = Some((leader, sora_in, LEADER_OUT_FRAMES));
            return;
        }
        if self.leader_anim.is_some() {
            // a change during the shrink: the latest is what grows back
            self.leader_anim = Some((leader, sora_in, self.leader_anim.map_or(0, |a| a.2)));
            return;
        }
        self.apply_leader(leader, sora_in);
    }

    /// (party P) The swap itself (the faces, the reload: every slot plays `in`).
    fn apply_leader(&mut self, leader: crate::party::PartyChara, sora_in: bool) {
        self.leader = leader;
        self.sora_in = sora_in;
        // (PF2) the faces from the seats (formation order), every slot reloaded (they all play `in`)
        let seats = self.layout(leader, sora_in);
        self.apply_seats(seats, true);
    }

    /// (PF2) THE STATUS HUD'S LAYOUT for a leader: slot 0 the leader, then the other members who are in the HUD (Sora:
    /// while in the party; a friend: while the host feeds his slot) IN FORMATION ORDER; a member the formation does not
    /// name (a host that sets none) after them in roster order, then in the host's order. No character has a fixed slot:
    /// a new character is placed by the same rule.
    fn layout(&self, leader: crate::party::PartyChara, sora_in: bool) -> [Option<crate::party::PartyChara>; kh_ui::hud_status::STATUS_INFO_MAX] {
        use crate::party::PartyChara as C;
        let mut seats = [None; kh_ui::hud_status::STATUS_INFO_MAX];
        seats[0] = Some(leader);
        let present = |c: C| if c == C::Sora { sora_in } else { self.party.iter().any(|p| p.chara == c) };
        let order = self.formation.active().iter().copied().chain(crate::formation::ROSTER.iter().map(|r| r.chara)).chain(self.party.iter().map(|p| p.chara));
        let mut next = 1;
        for c in order {
            if next >= seats.len() {
                break;
            }
            if present(c) && !seats.contains(&Some(c)) {
                seats[next] = Some(c);
                next += 1;
            }
        }
        seats
    }

    /// (PF2) Seat the characters: a slot whose character changed gets his face (Sora's NPC face in a friend slot, when the
    /// pack has it) and is reloaded; `all` reloads every slot (the leader switch's grow).
    fn apply_seats(&mut self, seats: [Option<crate::party::PartyChara>; kh_ui::hud_status::STATUS_INFO_MAX], all: bool) {
        if seats == self.seats && !all {
            return;
        }
        for (slot, seat) in seats.iter().enumerate() {
            // unchanged (a slot empty before and after stays as it is, even for `all`: no `out` replayed on it)
            if *seat == self.seats[slot] && (!all || seat.is_none()) {
                continue;
            }
            self.status_driver.faces[slot] = seat.and_then(|c| self.face_of(c, slot));
            self.status_driver.reload_face(slot);
        }
        self.seats = seats;
    }

    /// (PF2) A character's face movie for a slot (roster row's face; Sora in a friend slot: his NPC face when the pack has it).
    fn face_of(&self, c: crate::party::PartyChara, slot: usize) -> Option<std::sync::Arc<kh_ui::def::MovieDef>> {
        let own = self.chara_faces.iter().find(|f| f.0 == c).and_then(|f| f.1.clone());
        if c == crate::party::PartyChara::Sora && slot != 0 {
            self.sora_npc_face.clone().or(own)
        } else {
            own
        }
    }

    /// (PF2) The slot a character sits in (None: not in the status HUD).
    pub fn seat_of(&self, c: crate::party::PartyChara) -> Option<usize> {
        self.seats.iter().position(|s| *s == Some(c))
    }

    /// (PF2) Who sits in each status slot (0 = the main slot), the layout the faces, gauges and damage flashes follow.
    pub fn status_seats(&self) -> [Option<crate::party::PartyChara>; kh_ui::hud_status::STATUS_INFO_MAX] {
        self.seats
    }

    /// (PF2) The status movie's interface calls since the last take (`setFaceState(slot, 1)` = the damage flash, `loadFace`
    /// ...): tests and harnesses.
    pub fn take_status_calls(&mut self) -> Vec<String> {
        std::mem::take(&mut self.status.calls)
    }

    /// (party) A friend's level-up window (`0x141589330(cockpit, level, pawn, deltas)`, `chara` 1 Donald, 2 Goofy);
    /// shown in this frame after Sora's, in a window of its own (see `party_levels`).
    pub fn party_level_up(&mut self, chara: crate::party::PartyChara, level: i32, lines: &[String]) {
        (self.log)(&format!("hud: level up of {} to {level}: {lines:?}", chara.name()));
        self.party_levels.push_back((level, chara as usize, lines.to_vec()));
    }

    /// (BN) A CHARACTER'S BONUS! PANEL (KH3's `HudRightGetBonus`, 0x1415874F0): a reward obtained outside levelling (a boss
    /// reward, a roster boost, an item's permanent bonus, a spell of the shared book) for `chara`, in his own window
    /// (his roster row's colour, his portrait), two lines a window, shown at the next frame with the panel's sound.
    /// `lines`: KH3's texts with their icons in front ([`Hud::bonus_line_text`], `kh_ui::hud_side::icon_line`).
    pub fn party_bonus(&mut self, chara: crate::party::PartyChara, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        (self.log)(&format!("hud: bonus of {}: {lines:?}", chara.name()));
        self.party_bonuses.push_back((chara, lines.to_vec()));
    }

    /// (BN) A roster character's window look (its row's colour, its MenuFace portrait); a character the pack has no
    /// face for: its row's colour, no portrait.
    pub fn panel_for(&self, chara: crate::party::PartyChara) -> kh_ui::hud_side::PanelChara {
        self.panel_charas.iter().find(|p| p.0 == chara).map(|p| p.1.clone()).unwrap_or_else(|| kh_ui::hud_side::PanelChara { id: chara as usize, colour: crate::formation::entry_of(chara).map_or(4, |r| kh_ui::hud_side::panel_colour(r.panel)), portrait: None })
    }

    fn panel_for_index(&self, chara: usize) -> kh_ui::hud_side::PanelChara {
        self.panel_charas.iter().find(|p| p.0 as usize == chara).map(|p| p.1.clone()).unwrap_or_else(|| kh_ui::hud_side::PanelChara::plain(chara))
    }

    /// (BN) THE TEXT OF A REWARD LINE AS KH3 WRITES IT (0x141596150): the stat and slot kinds their `kh3_system` texts with
    /// no icon ("Maximum HP increased!"), an ability its name after the sparkle, a magic command its name after KH3's magic
    /// icon ([`bonus_line_icon`]); a host's own text as it is (a text that starts with an icon tag keeps it).
    pub fn bonus_line_text(&self, line: kh_core::status::BonusLine) -> String {
        use kh_core::status::BonusLine;
        let text = match line {
            BonusLine::None => return String::new(),
            BonusLine::MaxHp(_) => self.side.bonus_lines[0].clone(),
            BonusLine::MaxMp(_) => self.side.bonus_lines[1].clone(),
            BonusLine::ArmorSlot => self.side.bonus_lines[2].clone(),
            BonusLine::AccessorySlot => self.side.bonus_lines[3].clone(),
            BonusLine::ItemSlot => self.side.bonus_lines[4].clone(),
            BonusLine::Magic(command) => kh_core::magic::spells::command_name(command).to_string(),
            BonusLine::Ability(ability) => kh_core::status::tables::ABILITIES.iter().find(|row| row.kind == ability).map_or(ability.name(), |row| row.name).to_string(),
            BonusLine::MaxAp(_) => self.side.level_up_lines[3].clone(),
            BonusLine::Shotlock(name) | BonusLine::Text(name) => name.to_string(), // (Text: boss rewards, IMPL_ER31)
        };
        kh_ui::hud_side::icon_line(bonus_line_icon(line), &text)
    }

    /// (party) The window's lines for a level-up as the game words them (the same texts as Sora's: Strength,
    /// Defense, Magic, AP up, then an ability's name).
    pub fn level_up_lines(&self, strength: i32, magic: i32, defense: i32, ap: i32, ability: Option<kh_core::enums::ETresAbilityKind>) -> Vec<String> {
        let mut lines = Vec::new();
        for (delta, line) in [(strength, 0), (magic, 2), (defense, 1), (ap, 3)] {
            if delta != 0 {
                lines.push(self.side.level_up_lines[line].clone());
            }
        }
        if let Some(ability) = ability {
            let name = kh_core::status::tables::ABILITIES.iter().find(|row| row.kind == ability).map_or(ability.name(), |row| row.name);
            lines.push(kh_ui::hud_side::icon_line(kh_ui::hud_side::LineIcon::Ability, name)); // (BN) kind 7: the sparkle
        }
        lines
    }

    /// (party) The friends as the camp menu shows them (contract F.2), party order. Cheap to call every frame: the
    /// open screen is supplied again only when something changed.
    pub fn set_party_menu(&mut self, members: Vec<crate::party::PartyMenuMember>) {
        if members != self.party_menu {
            self.party_menu = members;
            self.party_menu_dirty = true;
        }
    }

    /// (party) The friends' weapons (staffs, shields) as Equipment pieces: id = the equip key ("WEP_DONALD_00").
    pub fn set_party_weapons(&mut self, mut pieces: Vec<Piece>) {
        // A piece without a name gets the game's: `WEP_DONALD_00` -> `kh3_item.weapon.donald_00` (TresItemData m_NameTextID).
        for p in pieces.iter_mut().filter(|p| p.name.is_empty() || p.name == p.id) {
            let key = format!("kh3_item.weapon.{}", p.id.trim_start_matches("WEP_").to_ascii_lowercase());
            let name = self.camp_view.string(&key);
            if !name.is_empty() && name != key {
                p.name = name;
            }
        }
        self.party_weapons = pieces;
    }

    pub fn has_party_weapons(&self) -> bool {
        !self.party_weapons.is_empty()
    }

    /// (party) What the camp menu changed for the friends since the last call (contract F.2).
    pub fn take_party_edits(&mut self) -> Vec<crate::party::PartyMenuEdit> {
        std::mem::take(&mut self.party_edits)
    }

    /// (AUD) THE CAMP MENU'S PARTY SLOTS (the user, 2026-10-17: "only active party members appear"): the ACTIVE party
    /// members in formation slot order, each with the data the menu needs (Sora: while he is in the party; a friend:
    /// while the host gives his snapshot). Every party list of the camp menu (the top panels, Equipment, Abilities,
    /// Stats, the Items targets, Customize's friends) is indexed by this list, so slot 0 is Sora only when he is in the
    /// party and first. Driven by the formation (`set_formation`) and the roster's characters, no names. Never empty:
    /// a host with no party (or no snapshot yet) gets Sora alone, as before.
    pub fn camp_roster(&self) -> Vec<crate::party::PartyChara> {
        use crate::party::PartyChara;
        let mut out: Vec<PartyChara> = self.formation.active().iter().copied().filter(|c| if *c == PartyChara::Sora { self.sora_in } else { self.party_menu.iter().any(|m| m.chara == *c) }).collect();
        if out.is_empty() {
            out.push(PartyChara::Sora);
        }
        // (EQ) SLOT 0 IS THE PLAYER (KH3's party table `m_PartyParams`, 0x14154C710: slot 0 the player pawn, then the
        // friends): the leader first, the others in formation order. Before, a leader behind Sora in the formation
        // (Sora, Donald, Goofy with Goofy leading) opened Equipment / Abilities / Stats on SORA's page, the Items target
        // on Sora, the first card Sora's: the friend leader's menus did not start where Sora's start (on himself).
        if let Some(i) = out.iter().position(|c| *c == self.formation.leader()).filter(|i| *i > 0) {
            let leader = out.remove(i);
            out.insert(0, leader);
        }
        out
    }

    /// (AUD) The index into `party_menu` of the friend in camp slot `member` (None: Sora's slot, or no such slot).
    fn friend_index(&self, member: usize) -> Option<usize> {
        let chara = *self.camp_roster().get(member)?;
        if chara == crate::party::PartyChara::Sora {
            return None;
        }
        self.party_menu.iter().position(|m| m.chara == chara)
    }

    /// (party) The party table of the camp menu (`m_PartyParams`): (AUD) the active members in formation order.
    fn party_stats(&self, sora: &Sora) -> Vec<StatsIn> {
        let mut out = Vec::new();
        for c in self.camp_roster() {
            if c == crate::party::PartyChara::Sora {
                out.push(self.stats_in(sora));
                continue;
            }
            let Some(m) = self.party_menu.iter().find(|m| m.chara == c) else { continue };
            out.push(StatsIn {
                name: m.chara.name().to_string(),
                level: m.level,
                exp: m.exp,
                next_level_exp: m.exp_next,
                level_exp: m.exp_level,
                hp: m.hp,
                hp_max: m.hp_max,
                mp: m.mp,
                mp_max: m.mp_max,
                // The focus gauge is Sora's only (10.7: "Sora only"). (GOOFY-GG) Goofy's GUARD takes that row (kh_ui::camp
                // `third_gauge_label`: "GUARD" for him).
                focus: m.guard.map_or(0, |g| g.0),
                focus_max: m.guard.map_or(0, |g| g.1),
                focus_unlocked: m.guard.is_some(),
                ap_max: m.ap_max,
                strength: m.strength,
                magic: m.magic,
                defense: m.defense,
                resists: m.resists,
                munny: sora.status().munny,
                play_seconds: self.play_seconds,
            });
        }
        out
    }

    fn friend(&self, member: usize) -> Option<&crate::party::PartyMenuMember> {
        self.friend_index(member).and_then(|i| self.party_menu.get(i)) // (AUD) by the camp roster, not "slot 0 = Sora"
    }

    /// (party) A friend's Abilities rows (10.4: the same list rules as Sora's, the friend's own record).
    fn friend_ability_rows(&self, m: &crate::party::PartyMenuMember) -> Vec<AbilityIn> {
        m.abilities
            .iter()
            .enumerate()
            .filter(|(_, a)| a.1 > 0)
            .map(|(order, &(kind, owned, equipped))| {
                let row = kh_core::status::tables::ABILITIES.iter().find(|r| r.kind == kind);
                let extra = self.ability_data.get(&(kind.value() as i64));
                AbilityIn {
                    id: kind.value(),
                    name: row.map_or(kind.name().to_string(), |r| r.name.to_string()),
                    category: match row.map_or("SUPPORT", |r| r.category) {
                        "ACTION" => 0,
                        "MOVE" => 1,
                        _ => 2,
                    },
                    ap: row.and_then(|r| r.ap).unwrap_or(0),
                    priority: extra.map_or(100_000 + order as i32, |e| e.0),
                    owned,
                    equipped,
                    help: extra.map_or(String::new(), |e| e.1.clone()),
                }
            })
            .chain(self.x2_kit_rows(m)) // (party X2-b) his kit abilities
            .collect()
    }

    /// (party) Whose weapons a friend can equip: Donald the staffs, Goofy the shields (`m_EquipSubclass`
    /// `n_ex001/w_do..` / `n_ex002/w_gu..` of TresEquipItemData [A]).
    #[allow(dead_code)] // (EQ) replaced by eq::weapon_family (no per-name table)
    fn friend_weapon_prefix(chara: crate::party::PartyChara) -> &'static str {
        match chara {
            crate::party::PartyChara::Donald => "WEP_DONALD",
            crate::party::PartyChara::Goofy => "WEP_GOOFY",
            crate::party::PartyChara::Sora => "WEP_KEYBLADE",
        }
    }

    /// (party) A friend's Equipment screen: one weapon slot, the open armour / accessory / item slots; the stock
    /// lists that friend's weapons, the shared armour / accessories and the battle items.
    fn friend_equip_in(&self, sora: &Sora, m: &crate::party::PartyMenuMember) -> EquipIn {
        use kh_core::status::tables::equipment_row;
        let weapon_piece = |id: &str| self.party_weapons.iter().find(|p| p.id == id).cloned();
        let mut slots = vec![EquipSlot { kind: KIND_WEAPON, slot: 0, piece: weapon_piece(&m.weapon), current: true, ..EquipSlot::default() }];
        for (kind, worn) in [(KIND_ARMOUR, &m.armour), (KIND_ACCESSORY, &m.accessories)] {
            for (slot, id) in worn.iter().enumerate() {
                slots.push(EquipSlot { kind, slot, piece: id.as_deref().and_then(equipment_row).map(piece_of), ..EquipSlot::default() });
            }
        }
        slots.extend(eq::item_rows(m)); // (EQ) Sora's item rows: Auto-Reload, a used slot greyed with its item
        let prefix = eq::weapon_family(&m.weapon); // (EQ) the family of the weapon he wears, no names
        let mut stock: Vec<StockEntry> = Vec::new();
        for (id, count) in self.inventory.equipment.iter().filter(|e| e.1 > 0) {
            if id.starts_with(prefix) {
                if let Some(piece) = weapon_piece(id) {
                    stock.push(StockEntry { piece, count: *count });
                }
            } else if let Some(row) = equipment_row(id).filter(|r| r.kind == "armour" || r.kind == "accessory") {
                stock.push(StockEntry { piece: piece_of(row), count: *count });
            }
        }
        let pouch = Self::pouch(sora);
        stock.extend(pouch.stock.iter().filter(|s| s.1 > 0 && kh_core::magic::items::command_of_item(s.0).is_some()).map(|(item, count)| StockEntry { piece: item_piece(*item), count: *count as u32 }));
        EquipIn { slots, stock, ap_in_use: m.ap_in_use, stock_max: STOCK_MAX }
    }

    /// (party) The menu's actions on a friend: the shared stock gives and takes the pieces (as for Sora: the commit
    /// `0x1415ECC00`), the friend's own record changes become [`PartyMenuEdit`]s for the host, and the snapshot is
    /// updated at once so the screen shows the change before the host answers (its next `set_party_menu` wins).
    fn apply_party_actions(&mut self, sora: &mut Sora, actions: Vec<(usize, StatusAction)>) {
        use crate::party::PartyMenuEdit as E;
        use kh_core::status::ItemKind;
        let take_item = |sora: &mut Sora, item: ItemKind| -> bool {
            kh_core::command::customize::ensure(sora);
            match sora.magic_mut().pouch.stock.iter_mut().find(|s| s.0 == item && s.1 > 0) {
                Some(entry) => {
                    entry.1 -= 1;
                    true
                }
                None => false,
            }
        };
        for (member, action) in actions {
            let Some(index) = self.friend_index(member) else { continue }; // (AUD) the camp roster's slot
            let chara = self.party_menu[index].chara;
            match action {
                StatusAction::SetAbility { kind, copy, on } => {
                    let m = &mut self.party_menu[index];
                    if let Some(a) = m.abilities.iter_mut().find(|a| a.0 == kind) {
                        a.2 = if on { a.2 | (1 << copy) } else { a.2 & !(1 << copy) };
                    }
                    let cost = kh_core::status::tables::ABILITIES.iter().find(|r| r.kind == kind).and_then(|r| r.ap).unwrap_or(0);
                    m.ap_in_use += if on { cost } else { -cost };
                    (self.log)(&format!("camp: {} {} {} copy {}", chara.name(), if on { "equipped" } else { "removed" }, kind.name(), copy + 1));
                    self.party_edits.push(E::Ability { chara, kind, copy, on });
                }
                StatusAction::Equip { kind: KIND_ITEM, slot, id } => {
                    let new = id.as_deref().and_then(ItemKind::from_name);
                    if let Some(item) = new {
                        if !take_item(sora, item) {
                            continue;
                        }
                    }
                    let old = self.party_menu[index].items.get(slot).copied().flatten();
                    if let Some(old) = old {
                        sora.magic_mut().pouch.stock_add(old, 1);
                    }
                    self.eq_item_slot(index, slot, new); // (EQ) Sora's rules: the Auto-Reload inherited / kept, a used slot cleared
                    (self.log)(&format!("camp: {} item slot {slot}: {} -> {}", chara.name(), old.map_or("-", |i| i.name()), new.map_or("-", |i| i.name())));
                    self.party_edits.push(E::ItemSlot { chara, slot, item: new });
                }
                StatusAction::Equip { kind, slot, id } => {
                    if let Some(new) = &id {
                        if !self.inventory.take(new) {
                            continue;
                        }
                    }
                    let m = &mut self.party_menu[index];
                    let old = match kind {
                        KIND_WEAPON => {
                            // The weapon slot cannot be emptied (Remove is refused for the current weapon, 10.5).
                            let Some(new) = id.clone() else { continue };
                            Some(std::mem::replace(&mut m.weapon, new))
                        }
                        _ => {
                            let worn = if kind == KIND_ARMOUR { &mut m.armour } else { &mut m.accessories };
                            worn.get_mut(slot).and_then(|s| std::mem::replace(s, id.clone()))
                        }
                    };
                    if let Some(old) = old.as_ref().filter(|o| !o.is_empty()) {
                        self.inventory.add(old, 1);
                    }
                    (self.log)(&format!("camp: {} equipment kind {kind} slot {slot}: {} -> {}", chara.name(), old.as_deref().unwrap_or("-"), id.as_deref().unwrap_or("-")));
                    self.party_edits.push(match kind {
                        KIND_WEAPON => E::Weapon { chara, id: id.unwrap_or_default() },
                        KIND_ARMOUR => E::Armour { chara, slot, id },
                        _ => E::Accessory { chara, slot, id },
                    });
                }
                // (party X2-b) a kit ability of the friend
                StatusAction::SetKit { id, on } => self.x2_kit_edit(index, id, on),
                // (EQ) Triangle on his item row: HIS slots (was: Sora's pouch's Auto-Reload)
                StatusAction::AutoReload { item, on } => self.eq_auto_reload(index, &item, on),
                StatusAction::UseItem { id } => {
                    let Some(item) = ItemKind::from_name(&id) else { continue };
                    if take_item(sora, item) {
                        (self.log)(&format!("camp: used {} on {}", item.name(), chara.name()));
                        self.party_edits.push(E::UseItem { chara, item });
                    }
                }
                _ => {}
            }
        }
        self.party_menu_dirty = true;
    }

    /// (party P) The members the AI runs now (Customize's list follows it at once).
    pub fn set_ai_members(&mut self, list: Vec<(crate::party::PartyChara, crate::party::FriendAiSettings)>) {
        if self.ai_members.as_ref() != Some(&list) {
            self.ai_members = Some(list);
            self.party_menu_dirty = true;
        }
    }

    /// (party) Customize's friends: one entry per friend, its settings.
    fn friends_in(&self) -> Vec<kh_ui::camp_custom::FriendIn> {
        // (party P) the AI-run members (Sora too while a friend leads); `member` = AI_MEMBER_BASE + the chara
        if let Some(list) = &self.ai_members {
            return list.iter().map(|(c, ai)| kh_ui::camp_custom::FriendIn { member: AI_MEMBER_BASE + *c as usize, name: c.name().to_string(), styles: [ai.battle, ai.ability, ai.cure], targets: ai.cure_targets }).collect();
        }
        // (AUD) the friends of the active party, `member` = their camp slot
        // (EQ) never the leader himself (a friend leading is the player, not an AI friend: his own entry was listed)
        self.camp_roster().iter().enumerate().filter(|(_, c)| **c != self.formation.leader()).filter_map(|(slot, c)| self.party_menu.iter().find(|m| m.chara == *c).map(|m| (slot, m))).map(|(slot, m)| kh_ui::camp_custom::FriendIn { member: slot, name: m.chara.name().to_string(), styles: [m.ai.battle, m.ai.ability, m.ai.cure], targets: m.ai.cure_targets }).collect()
    }

    /// (party) The benefit test of an item on a friend (`0x1415C3C30` was not read; the port's rule as for Sora:
    /// a recovery item helps when what it restores is not full).
    fn friend_benefit(m: &crate::party::PartyMenuMember, item: kh_core::status::ItemKind) -> bool {
        // (EQ) Sora's rule (`would_benefit`) on his own gauges: a knocked-out member gets nothing, Tent is an Elixir
        // (was: the recovery items alone, a KO'd friend took Potions)
        eq::benefit(m, item)
    }

    pub fn reset_popups(&mut self) {
        self.left.reset(&self.side);
        self.right.reset(&self.side);
        self.enemy = EnemyGauge::default();
        self.obtained_reset(); // (HUD2) the old session's OBTAINED popups go too
    }

    /// (ERUI) See `host_event`: while it is set, Start opens KH3's Pause screen (as in a battle), not the camp menu.
    pub fn set_host_event(&mut self, on: bool) {
        self.host_event = on;
    }

    pub fn open_camp_menu(&mut self) -> bool {
        if self.paused() || self.gameover.is_open() || self.gameover_seq.is_some() {
            return false;
        }
        self.force_camp = 2;
        true
    }

    /// The game-over menu is on screen.
    pub fn gameover_open(&self) -> bool {
        self.gameover.is_open()
    }

    /// Stage frames since his Die state began (0: he is not dead, or the game-over menu is off).
    pub fn dead_frames(&self) -> u32 {
        self.dead_frames
    }

    /// The button chosen in the game-over menu, once: the host carries it out (Continue = he starts again).
    pub fn take_gameover_choice(&mut self) -> Option<kh_ui::gameover::GameOverButton> {
        self.gameover_choice.take()
    }

    /// (er-host, IMPL_ER21) True once after "Title Screen" of the camp menu was chosen and confirmed (the menu is closing).
    pub fn take_title_request(&mut self) -> bool {
        std::mem::take(&mut self.title_request)
    }

    /// (er-host, IMPL_ER59) The outfit chosen in the camp menu (its id), once.
    pub fn take_outfit_choice(&mut self) -> Option<String> {
        self.outfit_choice.take()
    }

    /// True once after the Config entry was chosen with `config_external` (the menu is closing).
    pub fn take_system_menu(&mut self) -> bool {
        std::mem::take(&mut self.system_menu)
    }

    /// The camp menu's screen while it is open.
    /// (party) The party slot the open camp screen shows (0 Sora, 1 Donald, 2 Goofy).
    /// (party P) The host's formation (shown by the Party Formation screen; a host sets it each frame or on change).
    pub fn set_formation(&mut self, f: &crate::formation::Formation) {
        if !self.formation_open {
            self.formation = f.clone();
        }
    }

    pub fn formation(&self) -> &crate::formation::Formation {
        &self.formation
    }

    /// (ER party S6) THE CHARACTERS NOT UNLOCKED YET (the host's unlocks, Elden Ring: stream S8): the Party Formation
    /// screen shows their row as "???" and refuses to take them in or make them lead. Empty (the default) = the whole
    /// roster, as before.
    pub fn set_formation_locked(&mut self, locked: &[crate::party::PartyChara]) {
        if self.formation_locked != locked {
            self.formation_locked = locked.to_vec();
        }
    }

    /// (party P) OPENS THE PARTY FORMATION SCREEN (the save point's entry): the camp menu on its character step with
    /// the roster. False: another menu is up.
    pub fn open_party_formation(&mut self, f: &crate::formation::Formation) -> bool {
        if self.flow.paused() || self.gameover.is_open() || self.gameover_seq.is_some() || self.savepoint.is_open() {
            return false;
        }
        self.formation = f.clone();
        self.formation_request = true;
        self.force_camp = 2;
        true
    }

    /// (party P) The formation the player changed on the screen, once per change (in force at once).
    pub fn take_formation_change(&mut self) -> Option<crate::formation::Formation> {
        self.formation_change.take()
    }

    /// (party P) True once after the Party Formation screen closed (a host goes back to its save-point menu).
    pub fn take_formation_closed(&mut self) -> bool {
        std::mem::take(&mut self.formation_closed)
    }

    /// (party P) The characters of the camp menu's panels, panel order (`charaWindow01` ..): which picture goes where.
    pub fn camp_panels(&self) -> &[crate::party::PartyChara] {
        &self.camp_panels
    }

    /// (party P) The Party Formation step's texts and the active members' panels.
    fn formation_view(&self, sora: &Sora, error: Option<&str>) -> (Vec<StatsIn>, kh_ui::camp::FormationRows, Vec<crate::party::PartyChara>) {
        use crate::formation::ROSTER;
        let f = &self.formation;
        let mut stats = self.party_stats(sora);
        // (AUD) the roster rows show Sora's level even while he is out of the party (the table holds the active only)
        if !stats.iter().any(|s| s.name == crate::party::PartyChara::Sora.name()) {
            stats.push(self.stats_in(sora));
        }
        let stats_of = |c: crate::party::PartyChara| stats.iter().find(|s| s.name == c.name()).cloned();
        let mut party = Vec::new();
        let mut panels = Vec::new();
        for c in f.active() {
            let s = match stats_of(*c) {
                Some(s) => s,
                // a friend out of the party has no snapshot: his name on the panel
                None => StatsIn { name: c.name().to_string(), ..StatsIn::default() },
            };
            let mut s = s;
            if *c == f.leader() {
                s.name = format!("{} (Leader)", c.name());
            }
            party.push(s);
            panels.push(*c);
        }
        let rows = ROSTER
            .iter()
            .map(|r| {
                let place = match f.slot_of(r.chara) {
                    Some(_) if r.chara == f.leader() => "Leader".to_string(),
                    Some(n) => format!("Slot {n}"),
                    None => "-".to_string(),
                };
                // (ER party S6) a character not unlocked yet
                if self.formation_locked.contains(&r.chara) {
                    return format!("{:<8}-", "???");
                }
                format!("{:<8}{place}", r.chara.name())
            })
            .collect();
        // (party PF) the icon screen: the controls are the frame's button guide (pad glyphs); the help line says what the
        // screen is for, or why an edit was refused
        let help = match error {
            Some(e) => e.to_string(),
            None => "Choose up to three party members. The leader is the one you control.".to_string(),
        };
        // (party PF) THE GENERIC FEED: the roster (key, name, level, locked), the active members, the leader
        let members = ROSTER
            .iter()
            .map(|r| kh_ui::camp::FormationMember { key: r.key.to_string(), name: r.chara.name().to_string(), level: stats_of(r.chara).map_or(0, |s| s.level.max(0) as u32), locked: self.formation_locked.contains(&r.chara) })
            .collect();
        let index = |c: crate::party::PartyChara| ROSTER.iter().position(|r| r.chara == c);
        let active = f.active().iter().filter_map(|c| index(*c)).collect();
        let leader = index(f.leader()).unwrap_or(0);
        (party, kh_ui::camp::FormationRows { rows, help, stamp: 0, members, active, leader, refused: error.map(str::to_string) }, panels)
    }

    pub fn camp_member(&self) -> usize {
        self.camp.as_ref().map_or(0, |c| c.member)
    }

    /// (EQ) The open camp menu's state (tests and host checks: the rows, the cursor, the layers).
    pub fn camp_menu(&self) -> Option<&CampMenu> {
        self.camp.as_ref()
    }

    /// (EQ) The friends' snapshot the camp menu shows now (the host's last, with the menu's own edits on it).
    pub fn party_menu(&self) -> &[crate::party::PartyMenuMember] {
        &self.party_menu
    }

    pub fn camp_screen(&self) -> Option<CampScreen> {
        self.camp.as_ref().map(|c| c.screen)
    }

    /// (IMPL_ER34) The camp menu's screen stack as a word for a host's log ("Top", "Equipment/Stock", "Items+dialog"...).
    pub fn camp_trace(&self) -> Option<String> {
        self.camp.as_ref().map(|c| {
            let layer = if c.screen == CampScreen::Equipment { format!("/{:?}", c.equipment.layer) } else { String::new() };
            format!("{:?}{layer}{}{}", c.screen, if c.dialog.is_some() { "+dialog" } else { "" }, if self.camp_view.is_open() { "" } else { " (view closed)" })
        })
    }

    /// What the save data holds beyond the core's status (the equipment stock, spare Keyblades).
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    pub fn inventory_mut(&mut self) -> &mut Inventory {
        &mut self.inventory
    }

    /// The user configuration (Config screen); the port's readers take it from here.
    pub fn config(&self) -> &UserConfig {
        &self.config
    }

    fn pouch(sora: &Sora) -> kh_core::magic::Pouch {
        let pouch = &sora.magic().pouch;
        if pouch.filled {
            pouch.clone()
        } else {
            kh_core::magic::Pouch::from_status(sora.status())
        }
    }

    fn stats_in(&self, sora: &Sora) -> StatsIn {
        let status = sora.status();
        use kh_core::enums::ETresDamageAttribute as A;
        StatsIn {
            name: "Sora".into(),
            level: status.level as i32,
            exp: status.exp,
            next_level_exp: status.exp_next,
            level_exp: status.exp_level,
            hp: status.hp,
            hp_max: status.max_hp(),
            mp: status.mp,
            mp_max: status.max_mp(),
            focus: status.focus,
            focus_max: status.max_focus(),
            // The story flag (player state byte +0x95B0 bit 4) as the magic strand holds it.
            focus_unlocked: sora.magic().shotlock_unlocked,
            ap_max: status.max_ap(),
            strength: status.strength(),
            magic: status.magic(),
            defense: status.defense(),
            resists: [A::Fire, A::Blizzard, A::Thunder, A::Water, A::Aero, A::Dark].map(|a| status.resist(a)),
            munny: status.munny,
            play_seconds: self.play_seconds,
        }
    }

    fn ability_rows(&self, sora: &Sora) -> Vec<AbilityIn> {
        let set = &sora.status().abilities;
        kh_core::status::tables::ABILITIES
            .iter()
            .enumerate()
            .filter(|(_, row)| set.owned(row.kind) > 0)
            .map(|(order, row)| {
                // `m_UIPriority` and `m_HelpTextID` from the pack's copy of the ability sheet.
                let extra = self.ability_data.get(&(row.kind.value() as i64));
                AbilityIn {
                    id: row.kind.value(),
                    name: row.name.to_string(),
                    category: match row.category {
                        "ACTION" => 0,
                        "MOVE" => 1,
                        _ => 2,
                    },
                    ap: row.ap.unwrap_or(0),
                    priority: extra.map_or(100_000 + order as i32, |e| e.0),
                    owned: set.owned(row.kind),
                    equipped: set.equipped_mask(row.kind),
                    // (XSI) the port's own abilities are not in the UI pack's data: the sheet's help line
                    help: extra.map_or_else(|| if row.help_text == "-" { String::new() } else { row.help_text.to_string() }, |e| e.1.clone()),
                }
            })
            .collect()
    }

    /// The Equipment screen's rows: weapons, armour, accessories, the pouch (spec 10.5).
    fn equip_in(&self, sora: &Sora) -> EquipIn {
        use kh_core::status::tables::equipment_row;
        use kh_core::status::BonusCounts;
        let status = sora.status();
        let mut slots = Vec::new();
        let weapons = [Some(status.weapon.clone()).filter(|w| !w.is_empty()), self.inventory.spare_weapons[0].clone(), self.inventory.spare_weapons[1].clone()];
        for (slot, id) in weapons.into_iter().enumerate() {
            slots.push(EquipSlot { kind: KIND_WEAPON, slot, piece: id.as_deref().and_then(equipment_row).map(piece_of), current: slot == 0, ..EquipSlot::default() });
        }
        for (kind, worn, bonus) in [(KIND_ARMOUR, &status.armour, BonusCounts::DEF_SLOT_UP1), (KIND_ACCESSORY, &status.accessories, BonusCounts::ACC_SLOT_UP1)] {
            // One slot at the start, one more per slot bonus.
            let open = (1 + status.bonuses.0[bonus] as usize).max(worn.len()).min(8);
            for slot in 0..open {
                slots.push(EquipSlot { kind, slot, piece: worn.get(slot).and_then(|id| equipment_row(id)).map(piece_of), ..EquipSlot::default() });
            }
        }
        let pouch = Self::pouch(sora);
        for (slot, entry) in pouch.slots.iter().enumerate().filter(|(_, s)| s.exists) {
            slots.push(EquipSlot { kind: KIND_ITEM, slot, piece: entry.item.map(item_piece), auto_reload: entry.auto_reload, used: entry.used, current: false });
        }
        let mut stock: Vec<StockEntry> = self.inventory.equipment.iter().filter(|e| e.1 > 0).filter_map(|(id, count)| equipment_row(id).map(|row| StockEntry { piece: piece_of(row), count: *count })).collect();
        stock.extend(pouch.stock.iter().filter(|s| s.1 > 0 && kh_core::magic::items::command_of_item(s.0).is_some()).map(|(item, count)| StockEntry { piece: item_piece(*item), count: *count as u32 }));
        EquipIn { slots, stock, ap_in_use: status.ap_in_use(), stock_max: STOCK_MAX }
    }

    /// The Items screen's inventory: battle items in stock, equipment in stock.
    fn items_in(&self, sora: &Sora) -> Vec<InvEntry> {
        use kh_core::status::ItemKind;
        let status = sora.status();
        let mut out = Vec::new();
        for (item, count) in Self::pouch(sora).stock.iter().filter(|s| s.1 > 0) {
            // The command strand's rule, INFERRED there (the native test 0x1415C3C30 was not read).
            let benefit = kh_core::command::customize::would_benefit(status, *item);
            // (party) the test per party slot: Sora's rule, then each friend's
            // (AUD) one answer per camp slot (the active members in formation order)
            let benefits = if self.party_menu.is_empty() { Vec::new() } else { self.camp_roster().iter().map(|c| if *c == crate::party::PartyChara::Sora { benefit } else { self.party_menu.iter().find(|m| m.chara == *c).is_some_and(|m| Self::friend_benefit(m, *item)) }).collect() };
            out.push(InvEntry { id: item.name().to_string(), name: item_display_name(*item), category: CAT_ITEM, count: *count as u32, usable: *item != ItemKind::Panacea, benefit, help: String::new(), benefits });
        }
        for (id, count) in self.inventory.equipment.iter().filter(|e| e.1 > 0) {
            if let Some(row) = kh_core::status::tables::equipment_row(id) {
                let category = match row.kind {
                    "keyblade" => CAT_WEAPON,
                    "armour" => CAT_ARMOUR,
                    _ => CAT_ACCESSORY,
                };
                out.push(InvEntry { id: id.clone(), name: row.name.to_string(), category, count: *count, ..InvEntry::default() });
            } else if let Some(item) = crate::items::by_key(id) {
                // (loot) materials (the Materials tab) and camp items (Items; nothing uses them yet)
                let category = if item.class == crate::items::Class::Material { CAT_MATERIAL } else { CAT_ITEM };
                out.push(InvEntry { id: id.clone(), name: item.name.clone(), category, count: *count, ..InvEntry::default() });
            }
        }
        // (IMPL_ER28) the host's key items (Elden Ring's): the Key Items tab, read-only rows
        // (IMPL_ER42) with the host's description for the item pane; (IMPL_ER43) and its picture: the row id carries the
        // icon number (`HOSTKEY_n.iconN`), `kh_ui::item_art` finds Elden Ring's picture of it in the pack
        for (k, key) in crate::items::key_items().into_iter().enumerate() {
            out.push(InvEntry { id: kh_ui::item_art::host_key_id(k, key.icon_id), name: key.name, category: CAT_KEY, count: key.count, help: key.help, ..InvEntry::default() });
        }
        out
    }

    /// What Customize edits: the command system's magic slots and shortcut pages (spec 10.6).
    fn customize_in(&self, sora: &Sora) -> CustomizeIn {
        // (party X2) a friend leading: his own page and sets
        if let Some(data) = self.x2_customize_in(sora) {
            return data;
        }
        use kh_core::magic::{items, spells, Element};
        let status = sora.status();
        let command = sora.command();
        let mut entries = Vec::new();
        // Asset order: Fire, Fira, Firaga, Blizzard .., Thunder .., Water .., Aero .., Cure ..
        for (index, element) in Element::ALL.into_iter().enumerate() {
            let learned = status.magic.iter().find(|m| Element::from_name(&m.name) == Some(element)).map_or(0, |m| m.tier.min(3));
            for tier in 1..=learned as u8 {
                let kind = element.command(tier);
                let help = self.camp_view.string(&format!("kh3_camp.customize.detail_{}", 1000 + index * 10 + tier as usize));
                // (party X2) "Costs {0} MP.": the cost with his MP Thrift (the {0} was shown as is)
                let help = x2::fill_cost(help, kh_core::magic::caster::effective_cost(kind, 0, &status.abilities, status.mp_cost_scale));
                entries.push(CommandEntry { kind, name: spells::command_name(kind).to_string(), group: GROUP_MAGIC, auto_reload: false, help });
            }
        }
        // Item commands of the items he has (pouch or stock).
        let pouch = Self::pouch(sora);
        for kind in 58u8..=68 {
            let Some(item) = items::item_of_command(kind) else { continue };
            let slots = pouch.slots.iter().filter(|s| s.exists && s.item == Some(item));
            if slots.clone().next().is_some() || pouch.stock_of(item) > 0 {
                entries.push(CommandEntry { kind, name: spells::command_name(kind).to_string(), group: GROUP_ITEM, auto_reload: slots.clone().any(|s| s.auto_reload), help: String::new() });
            }
        }
        let names = (27u8..=68).map(|kind| (kind, spells::command_name(kind).to_string())).collect();
        // (XSI, 2026-10-16) THE SHOTLOCK CHOICE (XS 2.8): once Spectral Rays is owned, the Keyblade's own (Ragnarok, its MAX
        // form) and Spectral Rays; KH3's own names (kh3_system.command.sf_ragnarok_0 / sf_laser_0 / shootflow).
        let shotlock = (status.abilities.owned(kh_core::enums::ETresAbilityKind::ShotlockSpectralRays) > 0).then(|| kh_ui::camp_custom::ShotlockIn {
            label: self.camp_view.string("kh3_system.command.shootflow"),
            choices: kh_core::abilities::xs::spectral::choices(true).into_iter().map(|k| (k, self.camp_view.string(if k == kh_core::abilities::xs::spectral::KIND { "kh3_system.command.sf_laser_0" } else { "kh3_system.command.sf_ragnarok_0" }))).collect(),
            current: command.shotlock,
            help: if status.has(kh_core::enums::ETresAbilityKind::ShotlockSpectralRays) { "Choose the shotlock R1 performs.".to_string() } else { "Choose the shotlock R1 performs (Spectral Rays needs its ability set on the Abilities screen).".to_string() },
        });
        CustomizeIn { magic_slots: command.magic_slots, shortcuts: command.shortcuts, entries, names, friends: self.friends_in(), shotlock, ..CustomizeIn::default() } // (party X2) the leader's fields: none for Sora
    }

    /// Supplies the open screen's data again (after an action changed the game).
    fn refresh_camp(&mut self, sora: &Sora) {
        // (party P) the Party Formation step keeps its own panels
        if self.formation_open {
            return;
        }
        let Some((screen, member)) = self.camp.as_ref().map(|c| (c.screen, c.member)) else { return };
        // (party) the party table first (the chosen member's copy becomes the screen's `stats`)
        let party = self.party_stats(sora);
        let stats = party.get(member).cloned().unwrap_or_else(|| self.stats_in(sora));
        let friend = self.friend(member).cloned();
        let data = match (screen, &friend) {
            (CampScreen::Abilities, Some(m)) => Some(CampData::Abilities(self.friend_ability_rows(m), m.ap_max)),
            (CampScreen::Equipment, Some(m)) => Some(CampData::Equipment(self.friend_equip_in(sora, m))),
            (CampScreen::Abilities, None) => Some(CampData::Abilities(self.ability_rows(sora), stats.ap_max)),
            (CampScreen::Equipment, None) => Some(CampData::Equipment(self.equip_in(sora))),
            (CampScreen::Items, _) => Some(CampData::Items(self.items_in(sora))),
            (CampScreen::Customize, _) => Some(CampData::Customize(self.customize_in(sora))),
            _ => None,
        };
        // (AUD) the panels follow the active party while the menu is open (a member joins / leaves, the leader changes)
        self.camp_panels = self.camp_roster();
        let Some(camp) = self.camp.as_mut() else { return };
        camp.set_party(party);
        camp.stats = stats;
        match data {
            Some(CampData::Abilities(rows, max_ap)) => camp.set_abilities(&rows, max_ap),
            Some(CampData::Equipment(data)) => camp.equipment.set(data),
            Some(CampData::Items(entries)) => camp.items.set(entries),
            Some(CampData::Customize(data)) => camp.customize.set(data),
            None => {}
        }
    }

    /// Applies what the menus asked for to the simulation and supplies the open screen again.
    /// The ONE write of the HUD layer to the game (the menus' effects are in force at once).
    pub fn apply_actions(&mut self, sora: &mut Sora) {
        use kh_core::command::customize;
        use kh_core::status::ItemKind;
        self.shop_apply(sora); // (SHOP) the shop's purchases and sales of the frame
        let actions = std::mem::take(&mut self.status_actions);
        // (party) the friends' actions: stock moves here, the status edits go to the host (`take_party_edits`)
        let party = std::mem::take(&mut self.party_pending);
        if !party.is_empty() {
            self.apply_party_actions(sora, party);
        }
        if actions.is_empty() {
            return;
        }
        customize::ensure(sora);
        for action in actions {
            // (party X2) a friend leader's Customize edit goes to his own block
            let Some(action) = self.x2_custom_action(sora, action) else { continue };
            match action {
                StatusAction::SetAbility { kind, copy, on } => {
                    let status = sora.status_mut();
                    status.abilities.set_copy(kind, copy, on);
                    status.rebuild();
                    (self.log)(&format!("camp: {} {} copy {} (copies equipped: mask {:03b}; AP {} / {})", if on { "equipped" } else { "removed" }, kind.name(), copy + 1, status.abilities.equipped_mask(kind), status.abilities.ap_in_use(), status.stats.ap_max));
                }
                // The commit of 0x1415ECC00: the old piece goes back to the stock, the new one
                // leaves it, the status is rebuilt.
                StatusAction::Equip { kind, slot, id } => {
                    if kind == KIND_ITEM {
                        let new = id.as_deref().and_then(ItemKind::from_name);
                        match customize::equip_item(sora, slot, new) {
                            Ok(()) => (self.log)(&format!("camp: item slot {slot} = {}", new.map_or("-", |i| i.name()))),
                            Err(refusal) => (self.log)(&format!("camp: item slot {slot}: refused ({refusal:?})")),
                        }
                        continue;
                    }
                    if let Some(new) = &id {
                        if !self.inventory.take(new) {
                            continue;
                        }
                    }
                    let status = sora.status_mut();
                    let old = match kind {
                        KIND_WEAPON if slot == 0 => Some(std::mem::replace(&mut status.weapon, id.clone().unwrap_or_default())).filter(|w| !w.is_empty()),
                        KIND_WEAPON => std::mem::replace(&mut self.inventory.spare_weapons[(slot - 1).min(1)], id.clone()),
                        _ => {
                            let worn = if kind == KIND_ARMOUR { &mut status.armour } else { &mut status.accessories };
                            let old = if slot < worn.len() { Some(worn.remove(slot)) } else { None };
                            if let Some(new) = &id {
                                worn.insert(slot.min(worn.len()), new.clone());
                            }
                            old
                        }
                    };
                    if let Some(old) = &old {
                        self.inventory.add(old, 1);
                    }
                    status.rebuild();
                    (self.log)(&format!("camp: equipment kind {kind} slot {slot}: {} -> {} (Strength {} Magic {} Defense {} max AP {})", old.as_deref().unwrap_or("-"), id.as_deref().unwrap_or("-"), status.strength(), status.magic(), status.defense(), status.max_ap()));
                }
                StatusAction::AutoReload { item, on } => {
                    if let Some(item) = ItemKind::from_name(&item) {
                        // The entry point toggles; the menu asked for the other state.
                        let now = customize::toggle_auto_reload(sora, item);
                        if now.is_some_and(|now| now != on) {
                            customize::toggle_auto_reload(sora, item);
                        }
                        (self.log)(&format!("camp: auto-reload of {} {}", item.name(), if on { "on" } else { "off" }));
                    }
                }
                // `0x140EBE1E0(GI, item, character, 1)`: the stock count drops, the item
                // command's effect applies.
                StatusAction::UseItem { id } => {
                    let Some(item) = ItemKind::from_name(&id) else { continue };
                    match customize::use_item_from_menu(sora, item) {
                        Ok(outcome) => (self.log)(&format!("camp: used {} (HP {:+}, MP {:+}, focus {:+})", item.name(), outcome.hp, outcome.mp, outcome.focus)),
                        Err(refusal) => (self.log)(&format!("camp: {} not used ({refusal:?})", item.name())),
                    }
                }
                StatusAction::Discard { id } => {
                    match ItemKind::from_name(&id) {
                        Some(item) => drop(customize::discard(sora, item, u8::MAX)),
                        None => self.inventory.equipment.retain(|e| e.0 != id),
                    }
                    (self.log)(&format!("camp: discarded {id}"));
                }
                StatusAction::SetMagicSlot { slot, kind } => {
                    let ok = customize::set_magic_tier(sora, slot, kind);
                    (self.log)(&format!("camp: magic slot {slot} = {} ({})", kh_core::magic::spells::command_name(kind), if ok { "set" } else { "refused" }));
                }
                StatusAction::SwapMagicSlots(a, b) => drop(customize::swap_magic_slots(sora, a, b)),
                StatusAction::SetShortcut { page, slot, kind } => {
                    let ok = if kind == 0 { customize::clear_shortcut(sora, page, slot) } else { customize::set_shortcut(sora, page, slot, kind) };
                    (self.log)(&format!("camp: shortcut {} slot {slot} = {} ({})", ["A", "B", "C"][page.min(2)], if kind == 0 { "-" } else { kh_core::magic::spells::command_name(kind) }, if ok { "set" } else { "refused" }));
                }
                StatusAction::GiveItem(item, n) => sora.magic_mut().pouch.stock_add(item, n),
                StatusAction::SetKit { .. } => {} // (party X2-b) friends only
                // (XSI) the Shotlock choice, in force at once
                StatusAction::SetShotlock(kind) => {
                    sora.command_mut().shotlock = kind;
                    (self.log)(&format!("camp: shotlock = {kind} ({})", if kind == kh_core::abilities::xs::spectral::KIND { "Spectral Rays" } else { "the Keyblade's own" }));
                }
                StatusAction::Unpause => {
                    let refilled = sora.on_unpause();
                    if refilled > 0 {
                        (self.log)(&format!("camp: {refilled} pouch slot(s) refilled on unpause"));
                    }
                    // (EQ) every friend's used auto-reload slots too, from the same stock (Sora's refill per character)
                    self.eq_refill_friends(sora);
                }
            }
        }
        self.refresh_camp(sora);
    }

    /// The Config screen's values in force: what has an owner in the tree reads them here.
    fn apply_config(&mut self) {
        // The command menu re-reads its bits (Control Type B = legacy mode).
        self.system.model.legacy_mode = self.config.control_type_b();
        self.config_dirty = true;
    }

    /// (integration pass) The saved configuration, at start.
    pub fn set_config(&mut self, config: UserConfig) {
        self.config = config;
        self.apply_config();
    }

    /// True once after the Config screen applied (mod.rs writes the file).
    pub fn take_config_save(&mut self) -> bool {
        std::mem::take(&mut self.config_save)
    }

    /// True once after the configuration changed (mod.rs pushes it to the readers).
    pub fn take_config_change(&mut self) -> bool {
        std::mem::take(&mut self.config_dirty)
    }

    /// (loot) WHAT THE HOST HANDED HIM SINCE THE LAST FRAME (`crate::items::give`: prize boxes, rewards): a battle item
    /// into the pouch's stock (applied with the frame's other status actions), anything else into the camp menu's
    /// stock; each with the game's item popup, in its stock-full variant when the stock already held the maximum
    /// (`m_MaxNum`; nothing is added then).
    fn take_given_items(&mut self, sora: &Sora) {
        for (item_id, count) in crate::items::take_given() {
            // (ER party S6, U4) a friend's staff or shield: into the camp menu's stock under its equip key
            if let Some(key) = crate::items::party_weapon_key(item_id) {
                let held = self.inventory.equipment.iter().find(|e| e.0 == key).map_or(0, |e| e.1);
                let added = count.min(STOCK_MAX.saturating_sub(held));
                if added > 0 {
                    self.inventory.add(&key, added);
                }
                let name = self.party_weapons.iter().find(|p| p.id == key).map(|p| p.name.clone()).filter(|n| !n.is_empty() && *n != key).unwrap_or_else(|| {
                    let text = format!("kh3_item.weapon.{}", key.trim_start_matches("WEP_").to_ascii_lowercase());
                    let s = self.camp_view.string(&text);
                    if s.is_empty() || s == text { key.clone() } else { s }
                });
                (self.log)(&format!("item: obtained {name} x{count} (a friend's weapon {key}, item {item_id:#x}){}", if added < count { format!(": STOCK FULL ({held} held, {added} added)") } else { String::new() }));
                self.obtained.push((item_id, added));
                if self.left.show_item_icon(None, &name, added < count) {
                    self.sounds.push(UiSound::Play(SE_ITEM));
                }
                continue;
            }
            let Some(item) = crate::items::by_id(item_id) else { continue };
            let (held, name) = match crate::items::battle_kind(item_id) {
                Some(kind) => (Self::pouch(sora).stock_of(kind) as u32, item_display_name(kind)),
                None => (self.inventory.equipment.iter().find(|e| e.0 == item.key).map_or(0, |e| e.1), item.name.clone()),
            };
            let room = item.max.min(STOCK_MAX).saturating_sub(held);
            let added = count.min(room);
            if added > 0 {
                match crate::items::battle_kind(item_id) {
                    Some(kind) => self.status_actions.push(StatusAction::GiveItem(kind, added.min(99) as u8)),
                    None => self.inventory.add(&item.key, added),
                }
            }
            let full = added < count;
            (self.log)(&format!("item: obtained {} x{count} (item {item_id:#x}, {:?}){}", name, item.class, if full { format!(": STOCK FULL ({held} held, {added} added)") } else { format!(": {} in stock now", held + added) }));
            self.obtained.push((item_id, added));
            if self.left.show_item_icon(Some(&item.key), &name, full) {
                self.sounds.push(UiSound::Play(SE_ITEM));
            }
        }
    }

    /// (IMPL_ER28) What he holds, for a host's item rules (`crate::items::held`): every piece in the camp menu's stock
    /// plus the worn ones, and his slot bonuses.
    fn publish_held(&self, sora: &Sora) {
        use kh_core::status::BonusCounts;
        let status = sora.status();
        let mut held: Vec<(String, u32)> = self.inventory.equipment.iter().filter(|e| e.1 > 0).cloned().collect();
        held.extend(status.armour.iter().chain(status.accessories.iter()).map(|id| (id.clone(), 1)));
        held.push(("@acc_slots".to_string(), status.bonuses.0[BonusCounts::ACC_SLOT_UP1] as u32));
        held.push(("@def_slots".to_string(), status.bonuses.0[BonusCounts::DEF_SLOT_UP1] as u32));
        crate::items::publish_held(held);
    }

    /// (loot) The items taken in since the last call: (item id, how many were added). For a host's log or test.
    pub fn take_obtained(&mut self) -> Vec<(u32, u32)> {
        std::mem::take(&mut self.obtained)
    }

    /// `hud-cmd give ID [N]`: equipment (by row id or English name) or a battle item into the stock.
    fn give(&mut self, what: &str, n: u32) {
        let wanted = what.replace('_', " ");
        if let Some(item) = kh_core::status::ItemKind::from_name(what).or_else(|| kh_core::status::ItemKind::ALL.into_iter().find(|i| item_display_name(*i).eq_ignore_ascii_case(&wanted))) {
            self.status_actions.push(StatusAction::GiveItem(item, n.min(99) as u8));
            return;
        }
        match kh_core::status::tables::EQUIPMENT.iter().find(|row| row.kind != "slot" && row.kind != "keyblade-upgrade" && (row.id.eq_ignore_ascii_case(what) || row.name.eq_ignore_ascii_case(&wanted))) {
            Some(row) => self.inventory.add(row.id, n),
            // (loot) a material or a camp item by key, name or item id: through the door every found item takes
            None => match crate::items::find(what) {
                Some(item) => {
                    crate::items::give(item.item_id, n);
                }
                None => println!("hud-cmd give {what}: no such equipment row, battle item or item of the item table"),
            },
        }
    }

    /// The menus' part of a frame. `pressed` = the pad buttons pressed in this frame's steps.
    pub fn menus(&mut self, f: &Frame, pressed: kh_core::PadButtons, nav: &[(u8, bool)], words: &[String]) {
        use kh_core::PadButton as B;
        // UI pad keys (spec 5.10 / 10.2): 10 - 13 the D-pad and 14 - 17 the stick (both through
        // `nav`, with the repeat flag), 27 decide = A, 28 cancel = B, 0 = Y, 3 = X, 4 / 5 =
        // L1 / R1, 6 / 7 = L2 / R2, 22 = Menu.
        let mut keys: Vec<(CampKey, bool)> = nav.iter().map(|(dir, repeat)| ([CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3], *repeat)).collect();
        for (button, key) in [(B::FaceBottom, CampKey::Decide), (B::FaceRight, CampKey::Cancel), (B::FaceTop, CampKey::Triangle), (B::FaceLeft, CampKey::Square), (B::LeftShoulder, CampKey::L1), (B::RightShoulder, CampKey::R1), (B::LeftTrigger, CampKey::L2), (B::RightTrigger, CampKey::R2), (B::SpecialRight, CampKey::Menu)] {
            if pressed.has(button) {
                keys.push((key, false));
            }
        }
        for word in words {
            keys.push((match word.as_str() {
                "menu" | "menu open" | "menu menu" => CampKey::Menu,
                "menu up" => CampKey::Up,
                "menu down" => CampKey::Down,
                "menu left" => CampKey::Left,
                "menu right" => CampKey::Right,
                "menu decide" => CampKey::Decide,
                "menu cancel" => CampKey::Cancel,
                "menu triangle" => CampKey::Triangle,
                "menu square" => CampKey::Square,
                "menu l1" => CampKey::L1,
                "menu r1" => CampKey::R1,
                "menu l2" => CampKey::L2,
                "menu r2" => CampKey::R2,
                other => {
                    println!("hud-cmd {other}: menu open | up | down | left | right | decide | cancel | triangle | square | l1 | r1 | l2 | r2");
                    continue;
                }
            }, false));
        }
        let menu_key = keys.iter().any(|k| k.0 == CampKey::Menu);
        if !self.flow.paused() {
            // The gate (10.1): "enemies engaged" is the actions strand's battle flag (the host's
            // engaged-enemy count plus 1.0 s; `hud-cmd battle 2` forces it for a test); aiming a
            // shotlock blocks the camp menu. The pending state, the jump phase and the attack
            // test are not exposed by the core (taken as clear).
            let aiming = f.sora.aim().mode != 0 || self.target_driver.scope_open();
            let gate = GateInput { in_battle: self.battle == 2 || f.sora.actions().battle, in_event: self.host_event /* (ERUI) */, state: f.state.value(), command_blocked: aiming, ..GateInput::default() };
            // (er-host, IMPL_ER15) a direct request for the CAMP menu (Prepare & Retry): as a Menu press with a clear gate
            let forced = self.force_camp > 0;
            let (menu_key, gate) = if forced { self.force_camp -= 1; (true, GateInput::default()) } else { (menu_key, gate) };
            match self.flow.update(menu_key, &gate) {
                // (party P) the save point's Party Formation: the camp menu on its character step
                Some(FlowEvent::Opened(Screen::Camp)) if std::mem::take(&mut self.formation_request) => {
                    let (party, rows, panels) = self.formation_view(f.sora, None);
                    let mut camp = CampMenu::open_formation(party, rows, &mut self.sounds);
                    self.camp_panels = panels;
                    self.formation_open = true;
                    // the view opens on the top screen (root `in`) and goes to the character step once that has played, as
                    // the camp menu does (`inChara`): FORMATION_OPEN_FRAMES stage frames later, the keys wait till then
                    camp.screen = CampScreen::Top;
                    self.camp_view.open(&camp);
                    self.formation_wait = FORMATION_OPEN_FRAMES;
                    self.camp = Some(camp);
                }
                Some(FlowEvent::Opened(Screen::Camp)) => {
                    let mut camp = CampMenu::open(false, false, self.stats_in(f.sora), &mut self.sounds);
                    // (party) the party table `m_PartyParams` (0x14154C710): Sora, then the friends
                    camp.set_party(self.party_stats(f.sora));
                    // (party P) the panels' characters: (AUD) the active members in formation order
                    self.camp_panels = self.camp_roster();
                    self.party_menu_dirty = false;
                    if self.title_entry {
                        camp.entries.push(kh_ui::camp::TopEntry::TitleScreen);
                    }
                    // (er-host, IMPL_ER59) Sora's outfits, when the host offers them; (AUD) only while Sora is in the party
                    if let Some((outfits, worn)) = self.outfit_menu.clone().filter(|_| self.camp_panels.contains(&crate::party::PartyChara::Sora)) {
                        camp.set_outfits(outfits, &worn);
                    }
                    self.camp_view.open(&camp);
                    self.camp = Some(camp);
                }
                Some(FlowEvent::Opened(Screen::Pause(_))) => self.pause.open(&mut self.sounds),
                _ => {}
            }
            return;
        }
        // Paused: the keys go to the screen that is up.
        if self.pause.is_open() {
            if menu_key {
                self.pause.close(&mut self.sounds);
                self.flow.begin_close();
            }
        } else if self.camp.is_some() && matches!(self.flow.phase, Phase::Open(_)) {
            // (party P) the Party Formation screen goes to its character step when the camp's open has played
            let keys = if self.formation_wait > 0 {
                self.formation_wait = self.formation_wait.saturating_sub(f.ticks.max(1));
                if self.formation_wait == 0 {
                    if let Some(camp) = self.camp.as_mut() {
                        camp.screen = CampScreen::Character(kh_ui::camp::TopEntry::PartyFormation);
                    }
                }
                Vec::new()
            } else {
                keys
            };
            for (key, repeat) in keys {
                let Some(camp) = self.camp.as_mut() else { break };
                if self.config_external && key == CampKey::Decide && camp.screen == CampScreen::Top && camp.focused_entry() == Some(kh_ui::camp::TopEntry::Config) {
                    // The host's own settings menu: this one closes first (the pause ends with it).
                    self.system_menu = true;
                    self.camp_view.close();
                    self.flow.begin_close();
                    break;
                }
                // (party) whose screen the actions are for: the chosen member; on Items the target button
                let (member, target) = (camp.member, camp.items.target_cursor);
                for action in camp.key(key, repeat, &mut self.sounds) {
                    let friend_action = match &action {
                        // (AUD) a friend's slot of the camp roster (slot 0 is Sora's only when he is in the party and first)
                        // (EQ) + Auto-Reload: his item rows' Triangle is his (it went to Sora's pouch)
                        CampAction::SetAbility { .. } | CampAction::Equip { .. } | CampAction::AutoReload { .. } if self.friend_index(member).is_some() => Some(member),
                        CampAction::UseItem { .. } if self.friend_index(target).is_some() => Some(target),
                        _ => None,
                    };
                    if let Some(slot) = friend_action {
                        let pending = match action {
                            CampAction::SetAbility { id, copy, on } => kh_core::enums::ETresAbilityKind::from_value(id).map(|kind| StatusAction::SetAbility { kind, copy, on }).or_else(|| x2::kit_action(id, on)), // (party X2-b) a kit row
                            CampAction::Equip { kind, slot, id } => Some(StatusAction::Equip { kind, slot, id }),
                            CampAction::UseItem { id } => Some(StatusAction::UseItem { id }),
                            CampAction::AutoReload { item, on } => Some(StatusAction::AutoReload { item, on }), // (EQ)
                            _ => None,
                        };
                        self.party_pending.extend(pending.map(|p| (slot, p)));
                        continue;
                    }
                    match action {
                        // (party) Customize -> a friend: in force at once (the host passes it to the AI)
                        // (party P) an entry of the AI-run members (Sora's among them while a friend leads)
                        CampAction::SetFriendAi { member, styles, targets } if member >= AI_MEMBER_BASE => {
                            let chara = [crate::party::PartyChara::Sora, crate::party::PartyChara::Donald, crate::party::PartyChara::Goofy].get(member - AI_MEMBER_BASE).copied();
                            if let (Some(chara), Some(list)) = (chara, self.ai_members.as_mut()) {
                                let settings = crate::party::FriendAiSettings { battle: styles[0], ability: styles[1], cure: styles[2], cure_targets: targets };
                                if let Some(e) = list.iter_mut().find(|e| e.0 == chara) {
                                    e.1 = settings;
                                }
                                (self.log)(&format!("camp: {} AI settings: {}", chara.name(), settings.describe()));
                                self.party_edits.push(crate::party::PartyMenuEdit::Ai { chara, settings });
                            }
                        }
                        CampAction::SetFriendAi { member, styles, targets } => {
                            if let Some(m) = self.friend_index(member).and_then(|i| self.party_menu.get_mut(i)) { // (AUD) the camp roster's slot
                                m.ai = crate::party::FriendAiSettings { battle: styles[0], ability: styles[1], cure: styles[2], cure_targets: targets };
                                (self.log)(&format!("camp: {} AI settings: {}", m.chara.name(), m.ai.describe()));
                                self.party_edits.push(crate::party::PartyMenuEdit::Ai { chara: m.chara, settings: m.ai });
                            }
                        }
                        CampAction::SetAbility { id, copy, on } => {
                            if let Some(kind) = kh_core::enums::ETresAbilityKind::from_value(id) {
                                self.status_actions.push(StatusAction::SetAbility { kind, copy, on });
                            }
                        }
                        CampAction::Opened(CampScreen::Config) => {
                            let rows = option_rows(&self.config);
                            if let Some(camp) = self.camp.as_mut() {
                                camp.set_options(rows, true);
                            }
                        }
                        CampAction::Opened(CampScreen::GameHelp) => {
                            let categories = self.camp_view.help_categories();
                            if let Some(camp) = self.camp.as_mut() {
                                camp.help.set(categories);
                            }
                        }
                        CampAction::Opened(_) => self.refresh_camp(f.sora),
                        CampAction::ApplyConfig(values) => {
                            for (info, value) in OPTIONS.iter().zip(values) {
                                self.config.set(info.option, value);
                            }
                            self.apply_config();
                            self.config_save = true;
                            println!("config: applied; block {:02X?}", &self.config.block[..9]);
                        }
                        CampAction::RestoreConfigDefaults => {
                            let mut defaults = self.config;
                            defaults.restore_defaults();
                            let rows = option_rows(&defaults);
                            if let Some(camp) = self.camp.as_mut() {
                                camp.set_options(rows, false);
                            }
                        }
                        CampAction::Equip { kind, slot, id } => self.status_actions.push(StatusAction::Equip { kind, slot, id }),
                        CampAction::AutoReload { item, on } => self.status_actions.push(StatusAction::AutoReload { item, on }),
                        CampAction::UseItem { id } => self.status_actions.push(StatusAction::UseItem { id }),
                        CampAction::Discard { id } => self.status_actions.push(StatusAction::Discard { id }),
                        CampAction::SetMagicSlot { slot, kind } => self.status_actions.push(StatusAction::SetMagicSlot { slot, kind }),
                        CampAction::SwapMagicSlots(a, b) => self.status_actions.push(StatusAction::SwapMagicSlots(a, b)),
                        CampAction::SetShortcut { page, slot, kind } => self.status_actions.push(StatusAction::SetShortcut { page, slot, kind }),
                        CampAction::SetShotlock(kind) => self.status_actions.push(StatusAction::SetShotlock(kind)), // (XSI)
                        CampAction::Close => {
                            self.camp_view.close();
                            self.flow.begin_close();
                            // (party P) the Party Formation screen closes: the host goes back to its save-point menu
                            if std::mem::take(&mut self.formation_open) {
                                self.formation_closed = true;
                            }
                        }
                        // (party P) Party Formation: the rules (kh_host::formation), the answer's sound and texts
                        CampAction::Formation { row, lead } => {
                            let mut next = self.formation.clone();
                            let result = match crate::formation::ROSTER.get(row) {
                                Some(r) if self.formation_locked.contains(&r.chara) => Err(crate::formation::FormationError::Unknown), // (ER party S6) not unlocked
                                Some(r) if lead => next.set_leader(r.chara),
                                Some(r) => next.toggle(r.chara).map(|_| ()),
                                None => Err(crate::formation::FormationError::Unknown),
                            };
                            let error = result.err().map(|e| e.text());
                            if error.is_none() {
                                (self.log)(&format!("formation: {:?}, leader {}", next.active().iter().map(|c| c.name()).collect::<Vec<_>>(), next.leader().name()));
                                self.formation = next.clone();
                                self.formation_change = Some(next);
                            } else {
                                (self.log)(&format!("formation: refused: {}", error.unwrap_or("")));
                            }
                            let (party, rows, panels) = self.formation_view(f.sora, error);
                            self.camp_panels = panels;
                            if let Some(camp) = self.camp.as_mut() {
                                camp.formation_answer(error.is_none(), party, rows, &mut self.sounds);
                            }
                        }
                        CampAction::TitleScreen => {
                            self.title_request = true;
                            (self.log)("camp: TITLE SCREEN chosen and confirmed");
                        }
                        CampAction::SetOutfit(id) => {
                            (self.log)(&format!("camp: OUTFIT {id} chosen"));
                            if let Some((_, worn)) = self.outfit_menu.as_mut() {
                                worn.clone_from(&id);
                            }
                            self.outfit_choice = Some(id);
                        }
                    }
                }
            }
            // (party) a new snapshot of the friends: the open screen shows it
            if std::mem::take(&mut self.party_menu_dirty) {
                self.refresh_camp(f.sora);
            }
            if let Some(camp) = self.camp.as_mut() {
                self.camp_view.sync(camp);
            }
        }
    }

    /// Stage frames of the menu movies (they run while the game is paused).
    pub fn tick_menus(&mut self) {
        if self.pause.is_open() && self.pause.tick() {
            self.flow.closed();
        }
        if self.camp_view.is_open() {
            if let Some(camp) = self.camp.as_mut() {
                self.camp_view.sync(camp);
            }
            // (character-material implementer; marked) the settings guide's glyph by the device used
            // last, and the character picture of the Abilities / Stats screens (menu_model.rs).
            if let Some(hook) = self.camp_hook {
                hook(&mut self.camp_view.movie, &self.camp_view.fonts);
            }
            if self.camp_view.tick() {
                self.camp = None;
                self.flow.closed();
                // The first unpaused frame: the pouch refill (magic spec J 7.5).
                self.status_actions.push(StatusAction::Unpause);
            }
        }
    }
}

enum CampData {
    Abilities(Vec<AbilityIn>, i32),
    Equipment(EquipIn),
    Items(Vec<InvEntry>),
    Customize(CustomizeIn),
}

// ---------------------------------------------------------------------------------------------
// The core's command system (kh_core::command, pad-driven) as the display's model.

impl Hud {
    /// `UiSnapshot::command` + `UiSnapshot::situation` -> the display model. The one-shot events
    /// are taken only in a frame that saw a new step (a frame with several steps shows the last
    /// step's events only).
    fn live_model(&self, f: &Frame) -> kh_ui::command::CommandModel {
        use kh_core::command as core;
        use kh_ui::command::{CloseKind, CommandModel, EventButton, MenuEvent, MenuRow, MenuState, ShootFlow, Situation};
        let c = &f.snapshot.command;
        let row = |r: &core::CommandRow| MenuRow { text: r.name.to_string(), enabled: r.enabled, mp_short: r.mp_short, quantity: r.quantity, flash: false };
        // (XSI) the XS abilities' command without a KH3 text (Light Wave)
        let name_of = |kind: i64, fallback: &str| self.command_names.get(&kind).cloned().or_else(|| kh_core::abilities::xs::command_name(kind).map(str::to_string)).unwrap_or_else(|| fallback.to_string());
        let mut model = CommandModel {
            state: match c.state {
                core::MenuState::MainSel => MenuState::MainSel,
                core::MenuState::SubSel => MenuState::SubSel,
                core::MenuState::Shortcut => MenuState::Shortcut,
                core::MenuState::ShootFlow => MenuState::ShootFlow,
            },
            simple_hud: self.system.model.simple_hud,
            battle_mode: self.battle,
            link_none: c.link_none,
            main_cursor: c.main_cursor,
            list: c.list,
            row_cursor: c.row_cursor,
            shortcut_page: c.shortcut_page,
            legacy_mode: self.config.control_type_b(),
            // (cleanup pass) the sneeze latch: the Attack slot shows command kind 21's name.
            attack_bad_status: f.snapshot.sneeze.then(|| name_of(21, "Sneeze")),
            ..CommandModel::default()
        };
        for (to, from) in model.main.iter_mut().zip(&c.main) {
            *to = row(from);
        }
        for (to, from) in model.lists.iter_mut().zip(&c.lists) {
            *to = from.iter().map(row).collect();
        }
        for (to, from) in model.shortcut.iter_mut().zip(&c.shortcut) {
            *to = row(from);
        }
        if let Some(panel) = &c.shoot_flow {
            model.shoot_flow = Some(ShootFlow { rows: self.system.shoot_rows().to_vec(), enabled: panel.enabled, from_shortcut: false });
        }
        // Situation commands: the command snapshot's rows when its owner fills them, else the
        // combat queue's HUD list.
        // (YHUD) while a friend leads (or the HUD is swapping to him) his rows alone: `y_stack`
        let s = &f.snapshot.situation;
        let friend_leads = self.leader_anim.map_or(self.leader, |a| a.0) != crate::party::PartyChara::Sora;
        let y = y_stack(c, s, friend_leads, &name_of);
        model.situations = y.situations;
        model.situation_selected = y.selected;
        model.situation_show_triangle = !model.situations.is_empty();
        if y.event_button.is_some() {
            model.event_button = y.event_button;
        }
        // (SP2) THE HOST'S REACTION COMMAND (a save point's Save, TRES_RCID_SAVE / command kind 76): KH3's reactor offers it
        // as the event button (slot A: the green bar over the command menu, `addActionSelectEventButton`), selected, with
        // the pad icon; Y is the host's while it stands (the save point's Y rule). Whoever leads: this is the one model.
        // (RC) every Y interaction of the host: the first in the event button (green, Y), the others stacked as situation
        // commands (KH3's rows, unselected; their serials are the host's own range so they never meet the core's)
        if let Some((first, rest)) = self.host_reactions.split_first() {
            model.event_button = Some(EventButton { name: self.reaction_name(first), has_gauge: false, slot_b: false });
            model.situation_selected = None;
            model.situation_show_triangle = true;
            for r in rest {
                let name = self.reaction_name(r);
                model.situations.push(Situation { serial: host_reaction_serial(&name), name, icon_type: 0, in_effect: 0, total: 0.0, remaining: 0.0, enabled: true, refreshed: 0 });
            }
        }
        // (DON-UI) Donald's menu: the ally target list (KH3's party target list), his team-attack rows' names (the pack's, as
        // Sora's own rows)
        if let Some(d) = self.don.as_ref().filter(|_| self.leader == crate::party::PartyChara::Donald) {
            if let (Some((names, cursor)), true) = (&d.targets, model.state == MenuState::SubSel) {
                model.target = Some(kh_ui::command::TargetList { names: names.clone(), cursor: *cursor });
            }
            for row in model.situations.iter_mut() {
                if let Some((_, kind)) = d.team_serials.iter().find(|t| t.0 == row.serial) {
                    row.name = name_of(*kind, &row.name);
                }
            }
            // Magic's list changed in place (an element opened, or back to the elements): one frame closed (KH3's sub
            // list leaves), the next opens it again with its own rows (the count of bars and the cursor match)
            let shown = self.don_list.load(std::sync::atomic::Ordering::Relaxed);
            let magic = |k: u8| (1..=8).contains(&k);
            if model.state == MenuState::SubSel && magic(shown) && magic(d.list) && shown != d.list && model.target.is_none() {
                model.state = MenuState::MainSel;
                model.events.retain(|e| !matches!(e, MenuEvent::SubClosed(_)));
                model.events.push(MenuEvent::SubClosed(CloseKind::Back));
                self.don_list.store(0, std::sync::atomic::Ordering::Relaxed);
            } else {
                self.don_list.store(if model.state == MenuState::SubSel { d.list } else { 0 }, std::sync::atomic::Ordering::Relaxed);
            }
        } else {
            self.don_list.store(0, std::sync::atomic::Ordering::Relaxed);
        }
        // (YHUD) Sora's formchange gauge and form timer (his Y commands' own gauges) are his: not while a friend leads
        if s.gauge_ratio > 0.0 && !friend_leads {
            model.style_ratio = Some(s.gauge_ratio.clamp(0.0, 0.999));
        }
        if let Some(form) = s.form.as_ref().filter(|_| !friend_leads) {
            model.form_gauge = Some((form.left, form.total));
        }
        if f.snapshot.step != self.last_step {
            for event in &c.events {
                match event {
                    core::MenuEvent::Refused => model.events.push(MenuEvent::Refused),
                    core::MenuEvent::SubClosed(kind) => model.events.push(MenuEvent::SubClosed(match kind {
                        core::CloseKind::Cancel => CloseKind::Cancel,
                        core::CloseKind::Back => CloseKind::Back,
                        core::CloseKind::Decide => CloseKind::Decide,
                    })),
                    core::MenuEvent::ShortcutExecuted => model.events.push(MenuEvent::ShortcutExecuted { style_change: false }),
                    core::MenuEvent::SubOpened | core::MenuEvent::Executed(_) => {}
                }
            }
        }
        model
    }
}

/// (RC) One Y interaction the host offers in the command menu (see [`Hud::set_host_reactions`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostReaction {
    /// A KH3 command kind of the pack whose own name is shown (76 Save, 77 Talk, 78 Open, 79 Examine ...), if any.
    pub kind: Option<i64>,
    /// The text shown when the kind is none or the pack lacks it (a KH3-style short name: "Pick Up", "Climb" ...).
    pub name: String,
}

impl HostReaction {
    /// A KH3 command kind (its name from the pack; "Save" / "Talk" / ... when the pack lacks it).
    pub fn kind(kind: i64) -> HostReaction {
        let name = match kind {
            76 => "Save",
            77 => "Talk",
            78 => "Open",
            79 => "Examine",
            _ => "?",
        };
        HostReaction { kind: Some(kind), name: name.to_string() }
    }

    /// A text of the host's own.
    pub fn named(name: &str) -> HostReaction {
        HostReaction { kind: None, name: name.to_string() }
    }
}

/// (YHUD) The situation stack and the event button of one HUD frame (before the host's reactions are laid on).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct YStack {
    pub situations: Vec<kh_ui::command::Situation>,
    pub selected: Option<usize>,
    pub event_button: Option<kh_ui::command::EventButton>,
}

/// (YHUD) THE Y STACK IS THE CONTROLLED CHARACTER'S (the user, 2026-10-23, Donald leading: "ONLY the Y moves for the
/// character you're playing as have their Y moves show up on the HUD"). Both hosts' HUD (`Hud::live_model`):
/// - Sora leads (`friend_leads` false): the command snapshot's rows when its owner fills them, else Sora's combat queue
///   (`UiSnapshot::situation`) with its event button (slot B's finish / slot A's reaction command), as before;
/// - a friend leads: HIS command snapshot's rows alone (his own Y and the team attacks with him: kh_party::yhud), also
///   when they are none. Sora's queue never shows under an empty list (it did: Second Form, a finish, Grand Magic, the
///   Bombardier offer of Goofy and Sora on Donald's HUD) and Sora's event button never stands. A row without a name (a
///   team attack) is named by its kind (`name_of`, the pack's text).
pub fn y_stack(c: &kh_core::command::CommandSnapshot, s: &kh_core::ui_model::SituationHud, friend_leads: bool, name_of: &dyn Fn(i64, &str) -> String) -> YStack {
    use kh_ui::command::{EventButton, Situation};
    let mut y = YStack::default();
    if friend_leads || !c.situations.is_empty() {
        y.situations = c.situations.iter().map(|r| Situation { serial: r.serial, name: if r.name.is_empty() { name_of(r.kind as i64, "") } else { r.name.to_string() }, icon_type: r.icon_type, in_effect: 0, total: r.total, remaining: r.remaining, enabled: r.enabled, refreshed: r.refreshed }).collect();
        y.selected = c.situation_selected.filter(|i| *i < y.situations.len());
    } else {
        y.situations = s
            .list
            .iter()
            .map(|e| Situation {
                serial: e.serial,
                name: name_of(e.kind.value() as i64, e.kind.name()),
                // HUD class (combat spec 11.1) -> icon: 1 grand magic = magic, 3 formchange and 4 finish = weapon.
                icon_type: match e.class {
                    1 => 3,
                    3 | 4 => 1,
                    _ => 0,
                },
                in_effect: 0,
                total: e.total.max(0.0),
                remaining: e.remaining.max(0.0),
                enabled: !e.appearing,
                refreshed: 0,
            })
            .collect();
        y.selected = usize::try_from(s.selected).ok().filter(|i| *i < y.situations.len());
    }
    if let Some((entry, slot_b)) = s.slot_b.map(|e| (e, true)).or(s.slot_a.map(|e| (e, false))).filter(|_| !friend_leads) {
        // While a finish extra is offered it replaces slot B's text.
        let kind = s.extra.filter(|_| slot_b).unwrap_or(entry.kind);
        y.event_button = Some(EventButton { name: name_of(kind.value() as i64, kind.name()), has_gauge: entry.total > 0.0, slot_b });
    }
    y
}

/// (RC) A stacked host reaction's situation serial: by its text (the same text keeps its row), in a range the core's
/// serials never reach.
fn host_reaction_serial(name: &str) -> u32 {
    0x7E00_0000 | (name.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619)) & 0x00FF_FFFF)
}

// ------------------------------------------------------------------------------------------------- (CV1) the speech bubble

/// (CV1) The conversation runner's view is the HUD's speech bubble (its sounds go with the HUD's).
impl crate::conv::ConvView for Hud {
    fn conv_line(&mut self, name: Option<&str>, text: &str, reveal_cps: Option<f32>) {
        if let Some(b) = self.bubble.as_mut() {
            b.say(name, text, reveal_cps, &mut self.sounds);
        }
    }

    fn conv_choice(&mut self, name: Option<&str>, prompt: &str, options: &[String], start: usize, cancel: Option<usize>) {
        if let Some(b) = self.bubble.as_mut() {
            b.choose(name, prompt, options, start, cancel, &mut self.sounds);
        }
    }

    fn conv_cue(&mut self, cue: Option<bool>) {
        if let Some(b) = self.bubble.as_mut() {
            b.set_cue(cue);
        }
    }

    fn conv_close(&mut self) {
        if let Some(b) = self.bubble.as_mut() {
            b.close(&mut self.sounds);
        }
    }

    fn conv_events(&mut self) -> Vec<kh_ui::bubble::BubbleEvent> {
        self.bubble.as_mut().map(|b| b.take_events()).unwrap_or_default()
    }

    fn conv_line_shown(&self) -> bool {
        self.bubble.as_ref().is_some_and(|b| b.finished())
    }

    fn conv_passive(&mut self, passive: bool) {
        if let Some(b) = self.bubble.as_mut() {
            b.set_passive(passive);
        }
    }
}

impl Hud {
    /// (CV1) The speech bubble can be shown (the UI pack has its art).
    pub fn bubble_available(&self) -> bool {
        self.bubble.is_some()
    }

    /// (CV1) The bubble is up (popping in, shown or popping out).
    pub fn bubble_open(&self) -> bool {
        self.bubble.as_ref().is_some_and(|b| b.is_open())
    }

    /// (CV1) The speaker's face on the 1920 x 1080 stage this frame (None: not on screen): the tail follows it.
    pub fn bubble_speaker(&mut self, face: Option<kh_ui::bubble::Face>) {
        if let Some(b) = self.bubble.as_mut() {
            b.set_speaker(face);
        }
    }

    /// (CV1) The other faces of the conversation on the stage (the bubble keeps off them).
    pub fn bubble_faces(&mut self, faces: &[kh_ui::bubble::Face]) {
        if let Some(b) = self.bubble.as_mut() {
            b.set_faces(faces);
        }
    }

    /// (CV1) The stage area the bubble stays in (default kh_ui::bubble::SAFE).
    pub fn bubble_safe(&mut self, safe: kh_ui::Rect) {
        if let Some(b) = self.bubble.as_mut() {
            b.set_safe(safe);
        }
    }

    /// (CV3) The bubble's options take the D-pad only (true: the stick moves the player; the Elden Ring host), or the
    /// D-pad and the stick (false: the default).
    pub fn bubble_dpad_only(&mut self, on: bool) {
        self.bubble_dpad_only = on;
    }

    /// (CV1) Off at once.
    pub fn bubble_clear(&mut self) {
        if let Some(b) = self.bubble.as_mut() {
            b.clear();
        }
    }

    /// (CV1) What is drawn: the bubble's rectangle and tail (testing / logs).
    pub fn bubble_drawn(&self) -> Option<(kh_ui::Rect, kh_ui::bubble::Tail)> {
        self.bubble.as_ref().and_then(|b| b.drawn)
    }

    /// (CV1) The bubble's text as shown (testing / logs).
    pub fn bubble_text(&self) -> String {
        self.bubble.as_ref().map(|b| b.shown_text.clone()).unwrap_or_default()
    }

    /// (CV1) The bubble's part of a frame: A (decide), B (cancel), the D-pad / stick (the options) while it is up and no
    /// menu of ours is; its stage frames; the demonstration conversation.
    fn bubble_frame(&mut self, f: &Frame, nav: &[(u8, bool)], words: &[String]) {
        use kh_ui::camp::CampKey;
        if self.conv_demo.is_some() {
            self.conv_demo_frame(f);
        }
        let menu_up = self.pause.is_open() || self.camp_view.is_open() || self.flow.paused();
        let Some(b) = self.bubble.as_mut() else { return };
        if !b.is_open() {
            return;
        }
        if !menu_up {
            if f.pressed.has(kh_core::PadButton::FaceBottom) {
                b.key(CampKey::Decide, false, &mut self.sounds);
            }
            if f.pressed.has(kh_core::PadButton::FaceRight) {
                b.key(CampKey::Cancel, false, &mut self.sounds);
            }
            if self.bubble_dpad_only {
                // (CV3) the D-pad's presses only: the stick is the player's movement during a conversation
                for (button, key) in [(kh_core::PadButton::DPadUp, CampKey::Up), (kh_core::PadButton::DPadDown, CampKey::Down)] {
                    if f.pressed.has(button) {
                        b.key(key, false, &mut self.sounds);
                    }
                }
            } else {
                for (dir, repeat) in nav {
                    let key = [CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3];
                    b.key(key, *repeat, &mut self.sounds);
                }
            }
            for word in words {
                match word.as_str() {
                    "menu up" => b.key(CampKey::Up, false, &mut self.sounds),
                    "menu down" => b.key(CampKey::Down, false, &mut self.sounds),
                    "menu decide" => b.key(CampKey::Decide, false, &mut self.sounds),
                    "menu cancel" => b.key(CampKey::Cancel, false, &mut self.sounds),
                    _ => {}
                }
            }
        }
        for _ in 0..f.ticks {
            b.tick();
        }
    }

    /// (CV1) `hud-cmd conv demo` (a demonstration: Donald's lines and a choice; the training dummy speaks as Donald,
    /// Sora answers), `hud-cmd conv end`.
    fn conv_word(&mut self, rest: &[&str], f: &Frame) {
        use crate::conv::*;
        match rest {
            ["demo", ..] => {
                let actors = vec![Actor { id: 0, name: "Sora".into(), role: Role::Leader }, Actor { id: 1, name: "Donald".into(), role: Role::Npc }];
                let steps = vec![
                    Step::Line(Line::new(1, "Aw, phooey! That old crone's a fake!")),
                    Step::Line(Line::new(1, "I let her read my fingers, and she says, \"A dreadful sign! Truly dreadful!\"")),
                    Step::Line(Line::new(0, "Huh?")),
                    Step::Choice(Choice { speaker: Some(1), prompt: "Do you believe her?".into(), options: vec![ChoiceOption { text: "No way!".into(), branch: Branch::Goto("no".into()) }, ChoiceOption { text: "Maybe a little...".into(), branch: Branch::Next }], start: 0, cancel: None, external: false }),
                    Step::Line(Line::new(1, "Hmph!")),
                    Step::Label("no".into()),
                    Step::Line(Line::new(1, "Hmph! I'm not stayin' here another minute. {leader}, I'm comin' with ya!")),
                ];
                let faces = rest.iter().find_map(|w| w.strip_prefix("faces=")).and_then(|v| {
                    let n: Vec<f32> = v.split([',', '/']).filter_map(|x| x.parse().ok()).collect();
                    (n.len() >= 6 && n.len() % 3 == 0).then(|| n.chunks(3).map(|c| kh_ui::bubble::Face { x: c[0], y: c[1], r: c[2] }).collect::<Vec<_>>())
                });
                self.conv_demo = Some((Conversation::new(actors, steps), faces));
                println!("hud-cmd conv demo: a conversation of Sora and Donald (the training dummy speaks as Donald)");
            }
            ["end", ..] => {
                if let Some((mut c, _)) = self.conv_demo.take() {
                    c.end(self, "hud-cmd conv end");
                }
            }
            _ => println!("hud-cmd conv demo | end"),
        }
        let _ = f;
    }

    /// (CV1) The demonstration's frame: the faces from the test level's view (Sora's head, the first training dummy's
    /// or a point 2 m in front of Sora), the runner's step.
    fn conv_demo_frame(&mut self, f: &Frame) {
        let Some((mut c, given)) = self.conv_demo.take() else { return };
        let out = f.sora.output();
        let sora_head = out.position + kh_core::Vec3::Z * 150.0;
        let ahead = kh_core::Vec3::new(out.yaw.to_radians().cos(), out.yaw.to_radians().sin(), 0.0) * 200.0;
        let donald_head = f.targets.target(kh_core::TargetId(1)).map_or(out.position + ahead + kh_core::Vec3::Z * 90.0, |t| t.center + kh_core::Vec3::Z * 40.0);
        let project = |p: kh_core::Vec3, r_cm: f32| -> Option<kh_ui::bubble::Face> {
            let v = f.view?;
            let c = v.clip_from_world * (f.to_view)(p).extend(1.0);
            if c.w <= 1e-3 {
                return None;
            }
            let c2 = v.clip_from_world * (f.to_view)(p + kh_core::Vec3::Z * r_cm).extend(1.0);
            let (x, y) = ((c.x / c.w + 1.0) * 960.0, (1.0 - c.y / c.w) * 540.0);
            let y2 = (1.0 - c2.y / c2.w.max(1e-3)) * 540.0;
            Some(kh_ui::bubble::Face { x, y, r: (y - y2).abs().max(14.0) })
        };
        let faces = match given.as_deref() {
            Some([s, d, ..]) => [(0u32, Some(*s)), (1u32, Some(*d))],
            _ => [(0u32, project(sora_head, 16.0)), (1u32, project(donald_head, 18.0))],
        };
        let speaker = c.speaker();
        let face_of = |id: Option<u32>| faces.iter().find(|(k, _)| Some(*k) == id).and_then(|(_, f)| *f);
        let mut others: Vec<kh_ui::bubble::Face> = faces.iter().filter(|(k, _)| Some(*k) != speaker).filter_map(|(_, f)| *f).collect();
        others.extend(given.iter().flatten().skip(2).copied());
        self.bubble_speaker(face_of(speaker));
        self.bubble_faces(&others);
        c.step(self, &crate::conv::ConvInput { dt: f.dt, combat: false, voice_playing: None, apart: None });
        // the new speaker's face at once (the line just shown)
        let speaker = c.speaker();
        self.bubble_speaker(face_of(speaker));
        for e in c.take_events() {
            println!("hud-cmd conv demo: {e:?}");
        }
        if !c.is_over() {
            self.conv_demo = Some((c, given));
        }
    }
}

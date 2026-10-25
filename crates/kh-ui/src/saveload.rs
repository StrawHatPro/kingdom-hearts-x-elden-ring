//! (saveload) THE SAVE AND LOAD SCREENS (docs/sora-saveload-spec.md): the slot lists of the movie
//! `17_saveload/SaveLoad` (`UTresUIP_SaveLoad`), driven by a list of slot summaries the HOST supplies.
//! The screen never reads a file: it shows what it is given, runs the game's key flow and tells the host
//! `Save(slot)` / `Load(slot)` / `Cancel`; while the host writes or loads it shows the game's access message.
//!
//! THE GAME'S (spec sections 2 - 5; [A] asset, [C] decompiled C):
//! - Save list: clip `menuSaveData` (labels in / inNext / loop / outBack), list `saveDataObj.coList`
//!   (`CoSaveDataList`: 5 rows of 120 px, scroll loop, paging), rows `CoSaveDataListItem` [A].
//! - A row: the slot number, the world's title image, LV + level, munny + "MUNNY", the difficulty's name,
//!   play time "h:mm", the area's name, the face image, the crown of clear data; an unused slot shows
//!   "NO DATA" (`dataLabel` on `empty`) [A clips, C fill function 0x1416FDB00].
//! - Keys (0x141708F70) [C]: decide on an EMPTY save row saves at once; on a used row the question
//!   "Overwrite the save data in this slot?" Yes / No with the cursor on No; decide on an empty load row is
//!   refused (static SE 4); a used load row loads AT ONCE (the game asks nothing); cancel leaves (the save
//!   list plays `outBack`); L1 / R1 page.
//! - While the data is written the UI manager shows "Saving..." (`kh3_system.general.dialog_0001_xb1`), while
//!   it loads "Loading..." (`dialog_0006_xb1`) [C 0x1415A3620]; a finished save plays static SE 14 and the
//!   list has the keys again with the row redrawn [C 0x141707000]. No "saved" message exists.
//!
//! COMPOSED / LOCAL (the task's host has ten equal slots and no autosave):
//! - the LOAD screen uses the game's five-row load list WITHOUT the autosave button, which the movie has as
//!   `menuLoadClearData` (the "Select clear data." list); title and help are the load screen's
//!   (`kh3_menu.save.title_0002`, `detail_0002`). The game's own load screen (`menuLoadData`) is the autosave
//!   button above a four-row list;
//! - the list holds as many rows as the host gives (the game: 9);
//! - a world code the pack has no title for shows the game's own "no world" title image (`MenuWorldTitleNone`);
//! - the access message is the Dialog window with the game's text and no button; it stays at least
//!   [`MESSAGE_MIN_FRAMES`]; a failure shows `kh3_settings.top.system_0030` / `system_0040` with OK;
//! - the stage frame (play time, munny, area, the "Delete" button guide) is not shown;
//! - (SV, the user 2026-10-17: "the ability to delete save data in the save menu (without saving)") with
//!   `delete_enabled` KH3's Delete works on both lists: Triangle / Y on a used row asks `kh3_system.general.dialog_0003`
//!   "Delete the save data in this slot?" (Yes / No, the cursor on No), Yes shows "Deleting..." (`dialog_0004_xb1`) and
//!   tells the host `Delete(slot)`; its guide "<icon(Triangle)> Delete" (`kh3_menu.save.button_0001`) is a second line
//!   of the help text; a slot the host protects (`delete_current` off: the `is_current` row) refuses with a message;
//! - (SV) with `confirm_load` a used load row asks "Load the save data in this slot?" first (COMPOSED: KH3 asks nothing);
//! - slot numbers are drawn with two digits (the authored field holds "00"); the row cursor's colour variant
//!   (`color1` .. `color3`) is left as authored.

use std::collections::HashMap;
use std::sync::Arc;

use crate::camp::CampKey;
use crate::def::{BitmapFill, Char, Edge, Fill, Op, Paint, Rect0, Shape, SpriteDef};
use crate::list::{ListView, RowState};
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN, SE_MENU_REFUSED};
use crate::{Fonts, Matrix, Movie, MovieDef, Pack, PackError, UiSound};

/// Static SE 14 (`se00000_015`): a save finished (0x141707000).
pub const SE_SAVE_DONE: u32 = 14;
/// LOCAL: stage frames the access message stays at least (so that a save of a few milliseconds is seen).
pub const MESSAGE_MIN_FRAMES: u32 = 45;
/// The cursor's `decide` label of a row runs this long before the choice counts (LOCAL, as the other menus).
const DECIDE_FRAMES: u32 = 12;
/// `menuSaveData` `outBack`: frames 51 .. 63.
const OUT_FRAMES: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SlotState {
    #[default]
    Empty,
    Used,
}

/// The face image of a row (`/Game/UI/Icon/MenuFace`): Sora's costumes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Portrait {
    /// No image.
    None,
    #[default]
    Sora,
    SoraSecond,
    SoraCaribbean,
    SoraMonstropolis,
    SoraToyBox,
    SoraSanFransokyo,
    /// (party P) The save's party leader was Donald / Goofy (MenuFaceExDonald / MenuFaceExGoofy: the same art set).
    Donald,
    Goofy,
    /// (SV) Any other party leader of the host's roster, by his roster key (his face is the pack's `faces.<key>` of
    /// saveload.json when it has one; else the row shows no face): a new character needs no new variant.
    Chara(&'static str),
}

impl Portrait {
    fn key(self) -> Option<&'static str> {
        Some(match self {
            Portrait::None => return None,
            Portrait::Sora => "sora",
            Portrait::SoraSecond => "sora2",
            Portrait::SoraCaribbean => "sora_ca",
            Portrait::SoraMonstropolis => "sora_mi",
            Portrait::SoraToyBox => "sora_ts",
            Portrait::SoraSanFransokyo => "sora_bx",
            Portrait::Donald => "donald",
            Portrait::Goofy => "goofy",
            Portrait::Chara(key) => key, // (SV)
        })
    }
}

/// The mode mark of a row (the game's difficulty field).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum SlotMode {
    /// Nothing is written.
    #[default]
    None,
    /// 0 Beginner, 1 Standard, 2 Proud, 3 Critical (`kh3_menu.save.difficulty_N`).
    Difficulty(u8),
    /// A host's own word in the same field.
    Text(String),
}

/// One slot as the host knows it. The screen shows it and reads nothing else.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SlotSummary {
    /// The host's slot number, from 0 (drawn as index + 1).
    pub index: usize,
    pub state: SlotState,
    /// The character's name: for the host's records and logs (KH3's row has no name field: not drawn).
    pub name: String,
    pub level: u32,
    pub munny: u32,
    pub play_time_seconds: u64,
    /// The area's name as it is to be read ("Stranded Graveyard").
    pub location: String,
    /// A KH world code of the pack ("tt", "he", ...): its title image. `None` / unknown: the game's "no world" image.
    pub world: Option<String>,
    pub mode: SlotMode,
    /// The crown of clear data.
    pub clear: bool,
    /// The slot the running game was loaded from / last saved to: the cursor starts here (the game starts
    /// on the slot of the last save).
    pub is_current: bool,
    pub portrait: Portrait,
}

impl SlotSummary {
    pub fn empty(index: usize) -> SlotSummary {
        SlotSummary { index, ..SlotSummary::default() }
    }

    pub fn is_used(&self) -> bool {
        self.state == SlotState::Used
    }
}

/// "h:mm" of the rows (`kh3_system.general.system_0009` "{0}:{1}", minutes `%02d`; 0x1416FDB00).
pub fn play_time_text(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 3600, (seconds / 60) % 60)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveLoadMode {
    Save,
    Load,
}

/// What the screen told the host, once each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveLoadResult {
    /// Write the running game into this slot (`SlotSummary::index`); an overwrite was confirmed. The screen
    /// shows "Saving..." until the host reports [`Progress`].
    Save(usize),
    /// Load this slot. The screen shows "Loading..." until the host reports [`Progress`].
    Load(usize),
    /// The screen was left with cancel (it is gone).
    Cancel,
    /// (SV) Delete this slot's data (the question "Delete the save data in this slot?" was answered Yes). Nothing is
    /// saved. The screen shows "Deleting..." until the host reports [`Progress`] (give the new summaries first).
    Delete(usize),
}

/// What the host reports while it carries out a `Save` / `Load`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Still at it (the default after the result: the host need not say it).
    Working,
    /// Done. After a save the list has the keys again (give it the new summaries first:
    /// [`SaveLoadScreen::set_slots`]); after a load the screen leaves.
    Done,
    /// It failed: the game's failure message with OK, then the list again.
    Failed,
}

/// Where the screen is, for a host's log or test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Closed,
    /// The way in (keys do not count yet).
    Opening,
    List,
    /// "Overwrite the save data in this slot?" with the cursor on this entry (0 Yes, 1 No).
    Confirm(usize),
    /// (SV) "Delete the save data in this slot?" with the cursor on this entry (0 Yes, 1 No).
    ConfirmDelete(usize),
    /// (SV) "Load the save data in this slot?" (a host's [`SaveLoadScreen::confirm_load`]) with the cursor on this entry.
    ConfirmLoad(usize),
    /// (SV) A message with OK (a slot the host protects from deletion).
    Notice,
    /// A row's decide animation runs.
    Deciding,
    /// The access message is up; the host works.
    Busy,
    /// The failure message with OK.
    Failed,
    /// The way out.
    Leaving,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Closed,
    Opening,
    List,
    Confirm(usize, usize),
    /// (SV) the delete question / the load question: (row, cursor)
    ConfirmDelete(usize, usize),
    ConfirmLoad(usize, usize),
    Deciding(usize, u32),
    /// (SV) `job`: what the host is doing (a save, a load or a delete)
    Busy { shown: u32, progress: Progress, job: Job },
    Failed,
    /// (SV) a message with OK; then the list again
    Notice,
    Leaving(u32, Option<SaveLoadResult>),
}

/// (SV) What the access message stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Job {
    Save,
    Load,
    Delete,
}

/// (SV) KH3's question before a slot's data is deleted (0x141708F70 event 0xE) and its access message.
pub const DELETE_QUESTION: &str = "kh3_system.general.dialog_0003";
pub const DELETING: &str = "kh3_system.general.dialog_0004_xb1";
/// (SV) The stage frame's guide of the lists (`kh3_menu.save.button_0001`: "<icon(Triangle)> Delete").
pub const DELETE_GUIDE: &str = "kh3_menu.save.button_0001";
/// (SV) COMPOSED (KH3's load list asks nothing; the user asked for a confirmation, 2026-10-17): the question in the words
/// of KH3's other two slot questions ("Overwrite / Delete the save data in this slot?") and of its Title Screen question
/// ("All unsaved game progress will be lost.").
pub const LOAD_QUESTION: &str = "Load the save data in this slot?\nAll unsaved game progress will be lost.";
/// (SV) COMPOSED: a slot the host will not delete (the running game's own slot in the Elden Ring host).
pub const DELETE_IN_USE: &str = "This save data is in use and cannot be deleted.";
/// (SV) COMPOSED: a delete that failed (KH3's failure texts name only saving and loading).
pub const DELETE_FAILED: &str = "Failed to delete the save data.";

struct Clip {
    root: &'static str,
    list: &'static str,
    help: &'static str,
    item_class: &'static str,
}

const SAVE: Clip = Clip { root: "menuSaveData", list: "menuSaveData.saveDataObj.coList", help: "menuSaveData.saveDataObj.helpText.txHelpText.textField", item_class: "CoSaveDataListItem" };
const LOAD: Clip = Clip { root: "menuLoadClearData", list: "menuLoadClearData.loadClearDataObj.coList", help: "menuLoadClearData.loadClearDataObj.helpText.txHelpText.textField", item_class: "CoLoadDataListItem" };
const MENUS: [&str; 6] = ["menuSavePoint", "menuLoadClearData", "menuLoadData", "menuSaveData", "menuTutorial", "menuAreaSelect"];

/// A one-picture movie: what `tres.controls.Icon` draws for a loaded image (the image stretched over the
/// clip's authored size, through its clip rectangle).
fn image_def(name: &str, texture: Arc<crate::def::Texture>, size: (f32, f32), clip: (f32, f32, f32, f32)) -> Arc<MovieDef> {
    let (x0, y0, x1, y1) = (clip.0, clip.1, clip.0 + clip.2, clip.1 + clip.3);
    let fill = BitmapFill { scale: (texture.width as f32 / size.0, texture.height as f32 / size.1), texture, matrix: Matrix::IDENTITY, repeat: false, smooth: true };
    let shape = Shape {
        id: 1,
        bounds: Rect0 { x0, y0, x1, y1 },
        fills: vec![Fill { paint: Paint::Bitmap(fill), edges: vec![Edge::Line([x0, y0, x1, y0]), Edge::Line([x1, y0, x1, y1]), Edge::Line([x1, y1, x0, y1]), Edge::Line([x0, y1, x0, y0])] }],
        strokes: Vec::new(),
        scale9: None,
    };
    let root = SpriteDef { id: 0, frames: 1, ops: vec![vec![Op { depth: 1, ch: Some(1), matrix: Some(Matrix::IDENTITY), name: Some(Arc::from("image")), ..Op::default() }]], scripts: vec![Vec::new()], ..SpriteDef::default() };
    let mut chars = HashMap::new();
    chars.insert(1, Char::Shape(Arc::new(shape)));
    Arc::new(MovieDef { name: name.to_string(), stage: size, fps: 60.0, chars, root: Arc::new(root), symbols: HashMap::new(), bitmaps: HashMap::new(), consts: HashMap::new(), unknown_statements: Vec::new() })
}

/// The Save screen and the Load screen: one type, opened as either ([`SaveLoadScreen::open_save`],
/// [`SaveLoadScreen::open_load`]).
pub struct SaveLoadScreen {
    /// The slot list (movie SaveLoad) and the message window (movie Dialog): two layers.
    pub movie: Movie,
    pub dialog: Movie,
    fonts: Fonts,
    strings: HashMap<String, String>,
    faces: HashMap<String, Arc<MovieDef>>,
    worlds: HashMap<String, Arc<MovieDef>>,
    no_world: Option<Arc<MovieDef>>,
    mode: SaveLoadMode,
    state: State,
    slots: Vec<SlotSummary>,
    list: ListView,
    dialog_open: bool,
    dialog_closing: bool,
    background: bool,
    /// (SV) KH3's Delete (Triangle / Y on a used row of either list: the question, then "Deleting..."). Off by default:
    /// a host that can delete a slot switches it on. The guide "<icon(Triangle)> Delete" stands under the help line.
    pub delete_enabled: bool,
    /// (SV) The slot marked `is_current` may be deleted too (the test level: its files only). Off: that row refuses
    /// with [`DELETE_IN_USE`] (the Elden Ring host: the running character belongs to it).
    pub delete_current: bool,
    /// (SV) A used load row asks [`LOAD_QUESTION`] (Yes / No, the cursor on No) before the load (KH3 asks nothing).
    pub confirm_load: bool,
}

pub type SaveScreen = SaveLoadScreen;
pub type LoadScreen = SaveLoadScreen;

impl SaveLoadScreen {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<SaveLoadScreen, PackError> {
        let def: Arc<MovieDef> = pack.movie_linked("SaveLoad", &crate::camp_menu::LIBRARIES)?;
        let dialog = pack.movie_linked("Dialog", &crate::camp_menu::LIBRARIES)?;
        let mut strings = pack.string_table("kh3_menu");
        strings.extend(pack.string_table("kh3_system"));
        strings.extend(pack.string_table("kh3_settings"));
        // The rows' external images (tools/sora_saveload_pack.js: build/sora_pack/saveload, beside the UI pack).
        // A pack without them shows the rows without pictures.
        let (mut faces, mut worlds, mut no_world) = (HashMap::new(), HashMap::new(), None);
        if let Ok(data) = pack.data("../../saveload/saveload") {
            let file = |v: &serde_json::Value| v["file"].as_str().map(|f| format!("../saveload/{f}"));
            // (party P) + the friends; (SV) + every other face the pack lists (a new party leader's)
            let listed: Vec<String> = data["faces"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
            for key in ["sora", "sora2", "sora_ca", "sora_mi", "sora_ts", "sora_bx", "donald", "goofy"].iter().map(|k| k.to_string()).chain(listed) {
                if faces.contains_key(&key) {
                    continue;
                }
                if let Some(texture) = file(&data["faces"][key.as_str()]).and_then(|f| pack.texture(&f).ok()) {
                    let def = image_def(&key, texture, (256.0, 256.0), (0.0, 60.0, 256.0, 109.0));
                    faces.insert(key, def);
                }
            }
            for (code, world) in data["worlds"].as_object().into_iter().flatten() {
                if let Some(texture) = file(world).and_then(|f| pack.texture(&f).ok()) {
                    worlds.insert(code.clone(), image_def(code, texture, (164.0, 128.0), (0.0, 0.0, 164.0, 128.0)));
                }
            }
            no_world = file(&data["none"]).and_then(|f| pack.texture(&f).ok()).map(|t| image_def("none", t, (164.0, 128.0), (0.0, 0.0, 164.0, 128.0)));
        }
        Ok(SaveLoadScreen {
            movie: Movie::new(def),
            dialog: Movie::new(dialog),
            fonts,
            strings,
            faces,
            worlds,
            no_world,
            mode: SaveLoadMode::Save,
            state: State::Closed,
            slots: Vec::new(),
            list: ListView::default(),
            dialog_open: false,
            dialog_closing: false,
            background: false,
            delete_enabled: false,
            delete_current: false,
            confirm_load: false,
        })
    }

    pub fn is_open(&self) -> bool {
        self.state != State::Closed
    }

    pub fn mode(&self) -> SaveLoadMode {
        self.mode
    }

    pub fn phase(&self) -> Phase {
        match self.state {
            State::Closed => Phase::Closed,
            State::Opening => Phase::Opening,
            State::List => Phase::List,
            State::Confirm(_, cursor) => Phase::Confirm(cursor),
            State::ConfirmDelete(_, cursor) => Phase::ConfirmDelete(cursor),
            State::ConfirmLoad(_, cursor) => Phase::ConfirmLoad(cursor),
            State::Notice => Phase::Notice,
            State::Deciding(..) => Phase::Deciding,
            State::Busy { .. } => Phase::Busy,
            State::Failed => Phase::Failed,
            State::Leaving(..) => Phase::Leaving,
        }
    }

    /// The row under the cursor (an index into the list given, not the host's slot number).
    pub fn cursor(&self) -> usize {
        self.list.focus.max(0) as usize
    }

    pub fn slots(&self) -> &[SlotSummary] {
        &self.slots
    }

    /// The dialog movie while a message is up (its own layer, above the list).
    pub fn dialog_movie(&self) -> Option<&Movie> {
        self.dialog_open.then_some(&self.dialog)
    }

    /// The text of the message window.
    pub fn message(&self) -> Option<String> {
        self.dialog_open.then(|| self.dialog.text("dialogText.textField")).flatten()
    }

    fn string(&self, key: &str) -> String {
        // (<br> = a line break, as the other menus read the pack's strings)
        self.strings.get(key).map(|s| s.replace("<br>", "\n")).unwrap_or_default()
    }

    fn clip(&self) -> &'static Clip {
        match self.mode {
            SaveLoadMode::Save => &SAVE,
            SaveLoadMode::Load => &LOAD,
        }
    }

    /// THE SAVE SCREEN: "Select a slot to save to." `from_menu`: it comes out of the save-point menu (the
    /// list's `inNext`; else its `in`). `background`: the movie's own dark stage background is shown (a host
    /// with nothing behind the screen).
    pub fn open_save(&mut self, slots: &[SlotSummary], from_menu: bool, background: bool, sounds: &mut Vec<UiSound>) {
        self.open(SaveLoadMode::Save, slots, from_menu, background, sounds);
    }

    /// THE LOAD SCREEN: "Select a slot to load from."
    pub fn open_load(&mut self, slots: &[SlotSummary], background: bool, sounds: &mut Vec<UiSound>) {
        self.open(SaveLoadMode::Load, slots, false, background, sounds);
    }

    fn open(&mut self, mode: SaveLoadMode, slots: &[SlotSummary], from_menu: bool, background: bool, sounds: &mut Vec<UiSound>) {
        self.mode = mode;
        self.slots = slots.to_vec();
        self.background = background;
        self.hide_dialog_now();
        let clip = self.clip();
        self.movie.goto_and_play("", "in");
        for menu in MENUS {
            self.movie.set_visible(menu, menu == clip.root);
            if menu != clip.root {
                self.movie.goto_and_stop(menu, "off");
            }
        }
        self.movie.set_visible("stageBg", background);
        self.movie.goto_and_play("stageBg", if background { "in" } else { "off" });
        self.movie.set_visible("stageFrame", false);
        self.movie.goto_and_play(clip.root, if from_menu && mode == SaveLoadMode::Save { "inNext" } else { "in" });
        let (title, help) = match mode {
            SaveLoadMode::Save => ("kh3_menu.save.title_0001", "kh3_menu.save.detail_0001"),
            SaveLoadMode::Load => ("kh3_menu.save.title_0002", "kh3_menu.save.detail_0002"),
        };
        let (title, help) = (self.string(title), self.string(help));
        // (SV) the stage frame's Delete guide (KH3: `kh3_menu.save.button_0001` in the frame, which this port does not
        // show) stands as a second line under the help line
        let help = if self.delete_enabled { format!("{help}\n{}", self.string(DELETE_GUIDE)) } else { help };
        let fonts = self.fonts.clone();
        self.movie.set_text(&format!("{}.stageCategory.category.categoryName.txName", clip.root), &title, &fonts);
        self.movie.set_text(clip.help, &help, &fonts);
        // `CoSaveDataList`: rowNum 5, enableScrollLoop, repeatStop on both ends; the clear-data load list the same
        // without the loop (`CoLoadDataList_R5` of McLoadClearDataObj_6).
        let mut list = ListView::new(clip.list, clip.item_class);
        list.rows = 5;
        list.scroll_loop = mode == SaveLoadMode::Save;
        list.repeat_stop = true;
        list.set_count(self.slots.len());
        // 0x141708F70 event 7 / 0: the cursor starts on the slot of the last save.
        if let Some(current) = self.slots.iter().position(|s| s.is_current) {
            list.set_focus(current);
        }
        self.list = list;
        self.state = State::Opening;
        sounds.push(UiSound::Play(SE_MENU_OPEN));
        self.refresh();
    }

    /// New summaries for the rows (after a save: the row written shows its new data). The cursor stays.
    pub fn set_slots(&mut self, slots: &[SlotSummary]) {
        self.slots = slots.to_vec();
        let (focus, scroll) = (self.list.focus, self.list.scroll);
        self.list.set_count(self.slots.len());
        if focus >= 0 && (focus as usize) < self.slots.len() {
            self.list.focus = focus;
            self.list.scroll = scroll.min(self.list.scroll_max());
        }
        self.list.invalidate();
        if self.is_open() {
            self.refresh();
        }
    }

    /// The host's report on the `Save` / `Load` it was told to carry out.
    pub fn set_progress(&mut self, now: Progress) {
        if let State::Busy { progress, .. } = &mut self.state {
            *progress = now;
        }
    }

    /// The rows of the window are written (the native list-refresh handler 0x1416FDB00 and the row's proxy).
    fn refresh(&mut self) {
        let fonts = self.fonts.clone();
        let (lv, munny, no_data) = (self.string("kh3_menu.save.level"), self.string("kh3_menu.save.munny"), self.string("kh3_menu.save.manual_0001"));
        let difficulty: Vec<String> = (0..4).map(|n| self.string(&format!("kh3_menu.save.difficulty_{n}"))).collect();
        let (slots, faces, worlds, no_world) = (&self.slots, &self.faces, &self.worlds, &self.no_world);
        self.list.has_focus = !matches!(self.state, State::Closed);
        self.list.refresh(&mut self.movie, |movie, row, index| {
            let slot = &slots[index];
            movie.set_text(&format!("{row}.txNum.textField"), &format!("{:02}", slot.index + 1), &fonts);
            let label = format!("{row}.dataLabel");
            let wanted = if slot.is_used() { "info" } else { "empty" };
            if movie.current_label(&label) != Some(wanted) {
                movie.goto_and_play(&label, wanted);
            }
            movie.set_text(&format!("{label}.txDataEmpty"), &no_data, &fonts);
            let info = format!("{label}.dataInfo");
            if slot.is_used() {
                movie.set_text(&format!("{info}.txLv"), &lv, &fonts);
                movie.set_text(&format!("{info}.txNumLv"), &slot.level.to_string(), &fonts);
                movie.set_text(&format!("{info}.txMunny"), &munny, &fonts);
                movie.set_text(&format!("{info}.txNumMunny"), &slot.munny.to_string(), &fonts);
                let mode = match &slot.mode {
                    SlotMode::None => String::new(),
                    SlotMode::Difficulty(n) => difficulty.get(*n as usize).cloned().unwrap_or_default(),
                    SlotMode::Text(text) => text.clone(),
                };
                movie.set_text(&format!("{info}.txDifficulty"), &mode, &fonts);
                movie.set_text(&format!("{info}.txPlayTime"), &play_time_text(slot.play_time_seconds), &fonts);
                movie.set_text(&format!("{info}.txAreaName"), &slot.location, &fonts);
                let mark = format!("{info}.clearMark");
                let wanted = if slot.clear { "loop" } else { "off" };
                if movie.current_label(&mark) != Some(wanted) {
                    movie.goto_and_play(&mark, wanted);
                }
                // `tres.controls.Icon`: the placeholder goes (`removeChildren`), the image is drawn.
                let face = slot.portrait.key().and_then(|k| faces.get(k));
                let world = slot.world.as_ref().and_then(|w| worlds.get(&w.to_ascii_lowercase())).or(no_world.as_ref());
                for (path, image) in [(format!("{info}.charaImage.coImage"), face), (format!("{info}.worldTitleImage.coImage"), world)] {
                    if let Some(icon) = movie.sprite_mut(&path) {
                        icon.remove_children();
                    }
                    if let Some(image) = image {
                        movie.attach_first_child(&path, image);
                    }
                }
            }
            RowState { enabled: true }
        });
    }

    // ------------------------------------------------------------------------------------ the message window

    fn hide_dialog_now(&mut self) {
        self.dialog_open = false;
        self.dialog_closing = false;
        self.dialog.goto_and_stop("dialog", "off");
    }

    /// `0x1415A1C70(mgr, kind, text, ...)`: the window with its text and up to two buttons (the first selected
    /// by `cursor`).
    fn show_dialog(&mut self, text: &str, buttons: &[String], cursor: usize) {
        if !self.dialog_open || self.dialog_closing {
            self.dialog.goto_and_play("dialog", "in");
            self.dialog.add_instance("", "DialogButton", "dialogButton2");
        }
        self.dialog_open = true;
        self.dialog_closing = false;
        let fonts = self.fonts.clone();
        self.dialog.set_visible("dialogText", true);
        self.dialog.set_text("dialogText.textField", text, &fonts);
        for (n, name) in ["dialogButton", "dialogButton2"].into_iter().enumerate() {
            self.dialog.set_visible(name, n < buttons.len());
            if let Some(label) = buttons.get(n) {
                self.dialog.set_text(&format!("{name}.dialogBtnText.textField"), label, &fonts);
                self.dialog.goto_and_play(name, if cursor == n { "select" } else { "normal" });
            }
        }
        self.place_dialog_parts();
    }

    fn place_dialog_parts(&mut self) {
        let holder = "dialog.dialogAnm.dialogPoAll.dialogPoAll2";
        let stage = |movie: &Movie, path: &str| -> Option<(f32, f32)> {
            let mut m = Matrix::IDENTITY;
            let mut at = String::new();
            for part in path.split('.') {
                at = if at.is_empty() { part.to_string() } else { format!("{at}.{part}") };
                m = m.mul(&movie.node(&at)?.matrix);
            }
            Some((m.tx, m.ty))
        };
        if let Some((x, y)) = stage(&self.dialog, &format!("{holder}.poDialogText")) {
            self.dialog.set_position("dialogText", x, y, 0.0);
        }
        if let Some((x, y)) = stage(&self.dialog, &format!("{holder}.poDialogButton")) {
            self.dialog.set_position("dialogButton", x, y, 0.0);
            self.dialog.set_position("dialogButton2", x, y + 35.0, 0.0);
        }
    }

    fn leave_dialog(&mut self) {
        if self.dialog_open && !self.dialog_closing {
            self.dialog.goto_and_play("dialog", "out");
            self.dialog.set_visible("dialogText", false);
            for name in ["dialogButton", "dialogButton2"] {
                self.dialog.set_visible(name, false);
            }
            self.dialog_closing = true;
        }
    }

    fn yes_no(&self) -> Vec<String> {
        vec![self.string("kh3_system.general.choice_0001"), self.string("kh3_system.general.choice_0002")]
    }

    fn row_path(&self, index: usize) -> String {
        self.list.item_path(index.saturating_sub(self.list.scroll))
    }

    /// The host is told and the access message goes up (0x141715DE0 -> 0x1415A6970 / 0x1415A3620).
    fn start(&mut self, index: usize) -> SaveLoadResult {
        let job = match self.mode {
            SaveLoadMode::Save => Job::Save,
            SaveLoadMode::Load => Job::Load,
        };
        self.start_job(index, job)
    }

    /// (SV) The host is told (save, load or delete) and the access message goes up.
    fn start_job(&mut self, index: usize, job: Job) -> SaveLoadResult {
        let slot = self.slots[index].index;
        let (key, result) = match job {
            Job::Save => ("kh3_system.general.dialog_0001_xb1", SaveLoadResult::Save(slot)),
            Job::Load => ("kh3_system.general.dialog_0006_xb1", SaveLoadResult::Load(slot)),
            Job::Delete => (DELETING, SaveLoadResult::Delete(slot)),
        };
        let text = self.string(key);
        self.show_dialog(&text, &[], 0);
        self.state = State::Busy { shown: 0, progress: Progress::Working, job };
        result
    }

    /// (SV) A question with Yes / No, the cursor on No.
    fn ask(&mut self, text: &str) {
        let buttons = self.yes_no();
        self.show_dialog(text, &buttons, 1);
    }

    /// (SV) Moves the cursor of a Yes / No question.
    fn question_cursor(&mut self, cursor: usize, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) -> Option<usize> {
        let next = crate::camp::move_wrapping(cursor, 2, matches!(key, CampKey::Down | CampKey::Right), repeat)?;
        sounds.push(UiSound::Play(SE_MENU_CURSOR));
        for (n, name) in ["dialogButton", "dialogButton2"].into_iter().enumerate() {
            self.dialog.goto_and_play(name, if n == next { "select" } else { "normal" });
        }
        Some(next)
    }

    // ------------------------------------------------------------------------------------------------- keys

    /// A menu key. Up / down move (a fresh press wraps in the save list; a held direction stops at the end),
    /// L1 / R1 page, decide chooses, cancel leaves.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) -> Option<SaveLoadResult> {
        match self.state {
            State::List => match key {
                CampKey::Up | CampKey::Down => {
                    if self.list.move_anchor(key == CampKey::Down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        self.refresh();
                    }
                }
                CampKey::L1 | CampKey::R1 => {
                    let moved = if key == CampKey::R1 { self.list.next_page(repeat) } else { self.list.prev_page(repeat) };
                    if moved {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        self.refresh();
                    }
                }
                CampKey::Decide => {
                    let Some(slot) = self.slots.get(self.cursor()) else { return None };
                    let index = self.cursor();
                    match (self.mode, slot.is_used()) {
                        // (0x141708F70 event 7, list 8: an empty slot plays the refuse sound)
                        (SaveLoadMode::Load, false) => sounds.push(UiSound::Play(SE_MENU_REFUSED)),
                        // (list 7, a used slot: the question, default entry 1 = No)
                        (SaveLoadMode::Save, true) => {
                            sounds.push(UiSound::Play(SE_MENU_DECIDE));
                            let text = self.string("kh3_system.general.dialog_0002");
                            let buttons = self.yes_no();
                            self.show_dialog(&text, &buttons, 1);
                            self.state = State::Confirm(index, 1);
                        }
                        // (SV) a host's load question (the cursor on No)
                        (SaveLoadMode::Load, true) if self.confirm_load => {
                            sounds.push(UiSound::Play(SE_MENU_DECIDE));
                            self.ask(LOAD_QUESTION);
                            self.state = State::ConfirmLoad(index, 1);
                        }
                        _ => {
                            sounds.push(UiSound::Play(SE_MENU_DECIDE));
                            let row = self.row_path(index);
                            self.movie.goto_and_play(&row, "decide");
                            self.state = State::Deciding(index, DECIDE_FRAMES);
                        }
                    }
                }
                CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    let clip = self.clip();
                    // (event 8, list 7: the save list's `outBack`; the load lists close the part)
                    if !self.movie.goto_and_play(clip.root, "outBack") {
                        self.movie.goto_and_play("", "out");
                    }
                    self.state = State::Leaving(OUT_FRAMES, Some(SaveLoadResult::Cancel));
                }
                // (SV) TRIANGLE / Y: KH3's Delete (0x141708F70 event 0xE, both lists): a used row asks "Delete the save
                // data in this slot?" (Yes / No, the cursor on No); an empty row refuses; a row the host protects says why
                CampKey::Triangle if self.delete_enabled => {
                    let index = self.cursor();
                    let Some(slot) = self.slots.get(index) else { return None };
                    if !slot.is_used() {
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    } else if slot.is_current && !self.delete_current {
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                        let ok = vec![self.string("kh3_system.general.choice_0003")];
                        self.show_dialog(DELETE_IN_USE, &ok, 0);
                        self.state = State::Notice;
                    } else {
                        sounds.push(UiSound::Play(SE_MENU_DECIDE));
                        let text = self.string(DELETE_QUESTION);
                        self.ask(&text);
                        self.state = State::ConfirmDelete(index, 1);
                    }
                }
                _ => {}
            },
            // (SV) the delete question: Yes deletes (nothing is saved), No / cancel gives the row its focus back
            State::ConfirmDelete(index, cursor) => match key {
                CampKey::Up | CampKey::Down | CampKey::Left | CampKey::Right => {
                    if let Some(next) = self.question_cursor(cursor, key, repeat, sounds) {
                        self.state = State::ConfirmDelete(index, next);
                    }
                }
                CampKey::Decide if cursor == 0 => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    return Some(self.start_job(index, Job::Delete));
                }
                CampKey::Decide | CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.leave_dialog();
                    self.state = State::List;
                }
                _ => {}
            },
            // (SV) the load question: Yes plays the row's decide and loads, No / cancel gives the row its focus back
            State::ConfirmLoad(index, cursor) => match key {
                CampKey::Up | CampKey::Down | CampKey::Left | CampKey::Right => {
                    if let Some(next) = self.question_cursor(cursor, key, repeat, sounds) {
                        self.state = State::ConfirmLoad(index, next);
                    }
                }
                CampKey::Decide if cursor == 0 => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.leave_dialog();
                    let row = self.row_path(index);
                    self.movie.goto_and_play(&row, "decide");
                    self.state = State::Deciding(index, DECIDE_FRAMES);
                }
                CampKey::Decide | CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.leave_dialog();
                    self.state = State::List;
                }
                _ => {}
            },
            State::Notice => {
                if matches!(key, CampKey::Decide | CampKey::Cancel) {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.leave_dialog();
                    self.state = State::List;
                }
            }
            State::Confirm(index, cursor) => match key {
                CampKey::Up | CampKey::Down => {
                    if let Some(next) = crate::camp::move_wrapping(cursor, 2, key == CampKey::Down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        for (n, name) in ["dialogButton", "dialogButton2"].into_iter().enumerate() {
                            self.dialog.goto_and_play(name, if n == next { "select" } else { "normal" });
                        }
                        self.state = State::Confirm(index, next);
                    }
                }
                CampKey::Decide if cursor == 0 => {
                    // (0x141708080 with choice 0: the save)
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    return Some(self.start(index));
                }
                CampKey::Decide | CampKey::Cancel => {
                    // (choice 1 / cancel: the row has the focus again)
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.leave_dialog();
                    self.state = State::List;
                }
                _ => {}
            },
            State::Failed => {
                if matches!(key, CampKey::Decide | CampKey::Cancel) {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.leave_dialog();
                    self.state = State::List;
                }
            }
            _ => {}
        }
        None
    }

    /// One stage frame.
    pub fn tick(&mut self, sounds: &mut Vec<UiSound>) -> Option<SaveLoadResult> {
        if self.state == State::Closed {
            return None;
        }
        self.movie.tick();
        let events = self.movie.take_events();
        if self.dialog_open {
            self.dialog.tick();
            self.dialog.take_events();
            if self.dialog_closing && self.dialog.current_label("dialog") == Some("off") {
                self.dialog_open = false;
                self.dialog_closing = false;
            } else if !self.dialog_closing {
                self.place_dialog_parts();
            }
        }
        match self.state {
            State::Opening => {
                // `ON_END_IN_SAVE_LIST` / `ON_END_IN_LOAD_LIST`: the list takes the keys (0x141705A60).
                let label = self.movie.current_label(self.clip().root);
                if events.iter().any(|e| e.name.starts_with("ON_END_IN_")) || label == Some("loop") {
                    self.state = State::List;
                }
            }
            State::Deciding(index, left) => {
                if left > 0 {
                    self.state = State::Deciding(index, left - 1);
                } else {
                    return Some(self.start(index));
                }
            }
            State::Busy { shown, progress, job } => {
                let shown = shown + 1;
                self.state = State::Busy { shown, progress, job };
                if shown >= MESSAGE_MIN_FRAMES {
                    match (progress, job) {
                        (Progress::Working, _) => {}
                        // (SV) a delete is over: the game's access-done sound (LOCAL: the save's, 0x141707000; the delete's
                        // own end was not traced), the row shows "NO DATA" (the host gave the new summaries), the keys again
                        (Progress::Done, Job::Delete) => {
                            sounds.push(UiSound::Play(SE_SAVE_DONE));
                            self.leave_dialog();
                            self.state = State::List;
                            self.list.invalidate();
                            self.refresh();
                        }
                        (Progress::Failed, Job::Delete) => {
                            sounds.push(UiSound::Play(SE_MENU_REFUSED));
                            let ok = vec![self.string("kh3_system.general.choice_0003")];
                            self.show_dialog(DELETE_FAILED, &ok, 0);
                            self.state = State::Failed;
                        }
                        (Progress::Done, Job::Save) => {
                            // 0x141707000 result 1: the sound, the row again, the keys again.
                            sounds.push(UiSound::Play(SE_SAVE_DONE));
                            self.leave_dialog();
                            self.state = State::List;
                            self.list.invalidate();
                            self.refresh();
                        }
                        (Progress::Done, Job::Load) => {
                            self.leave_dialog();
                            self.movie.goto_and_play("", "out");
                            self.state = State::Leaving(OUT_FRAMES, None);
                        }
                        (Progress::Failed, job) => {
                            sounds.push(UiSound::Play(SE_MENU_REFUSED));
                            let text = self.string(if job == Job::Save { "kh3_settings.top.system_0030" } else { "kh3_settings.top.system_0040" });
                            let ok = vec![self.string("kh3_system.general.choice_0003")];
                            self.show_dialog(&text, &ok, 0);
                            self.state = State::Failed;
                        }
                    }
                }
            }
            State::Leaving(left, result) => {
                if left > 0 {
                    self.state = State::Leaving(left - 1, result);
                } else {
                    self.close();
                    return result;
                }
            }
            _ => {}
        }
        None
    }

    /// Takes the screen off at once.
    pub fn close(&mut self) {
        self.state = State::Closed;
        self.hide_dialog_now();
        for menu in MENUS {
            self.movie.goto_and_stop(menu, "off");
        }
        self.movie.goto_and_stop("stageBg", "off");
        self.movie.goto_and_play("", "off");
    }
}

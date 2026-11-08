//! (weapons P6b) THE WORKSHOP'S DISPLAY: KH3's movie `41_shop/Shop` (its Workshop clips `menuMixTop`, `menuSynthesis`,
//! `menuKeyb`, `menuPhoto`, `menuCollection`, `menuMaterial`, the synthesis confirmation `mesWin2`, the stage `stageBg`
//! on its Workshop tint `mixIn`, the frame `stageFrame`) and KH3's system window (movie `Dialog`: the entry dialogs, the
//! Forge's "You lack the needed materials." and its Yes / No), driven from [`WorkshopMenu`] each frame. The paths and
//! labels are the movie's (`build/sora_pack/ui/layout/Shop.json`, the library symbols as the player builds them).

use std::collections::HashMap;
use std::sync::Arc;

use super::texts;
use super::{picker_label, picker_offsets, Layer, RewardKind, Screen, WorkshopMenu, MATERIAL_COLUMNS, MATERIAL_LINES, TABS};
use crate::list::{ListView, RowState, TabBar};
use crate::shop::{category_icon, stamp_of};
use crate::{Fonts, Movie, MovieDef, Pack, PackError};

pub const MOVIE: &str = crate::shop_moogle::MOVIE;
const FRAME: &str = "stageFrame.stageObjR";
const GUIDE: &str = "stageFrame.stageObjR.btnInfo.btnInfoText.txText.textField";
const TOP: &str = "menuMixTop.menuObj";
const LOG: &str = "menuMixTop.menuObj.createLog";
const SYNTH: &str = "menuSynthesis.menuObj";
const KEYB: &str = "menuKeyb.menuObj";
const PHOTO: &str = "menuPhoto.menuObj";
const GOALS: &str = "menuCollection.menuObj";
const MATS: &str = "menuMaterial.menuObj";
const MES2: &str = "mesWin2.mesWinAnm.mesWinObj";
/// The picker's step animations (`pNL` / `pNR`: 14 frames).
const PICKER_FRAMES: u32 = 14;
/// The power-up's effects at the longest (a stop-gap when no callback came).
const EFFECT_FRAMES: u32 = 240;
/// The Material List's columns stand this far apart (`txItem2.x - txItem1.x`).
const MATERIAL_COLUMN_X: f32 = 768.0;
/// Frames after a clip came in before the pad drives it (KH3 enables input on the clips' `in` callbacks).
const SETTLE: u32 = 12;
/// (ASH-ROWS) The system window's frame and content holder (`Dialog.<init>`: `m_mesWin`, `m_mesPoAll`).
const DIALOG_WIN: &str = "dialog.dialogAnm.dialogWin.dialogWin2";
const DIALOG_PO_ALL: &str = "dialog.dialogAnm.dialogPoAll.dialogPoAll2";
const DIALOG_FIELD: &str = "dialogText.textField";

/// A clear / COMPLETE mark (`McClearMark`, `McCompleteMark`): `loop` shown, `off` hidden (their frame scripts set
/// `visible`; a jump to the label sets it the same way here).
fn mark(movie: &mut Movie, path: &str, on: bool) {
    movie.goto_and_stop(path, if on { "loop" } else { "off" });
    movie.set_visible(path, on);
}

/// The part of a Workshop command's name field left of its NEW mark (KH3's own: "Synthesize Items" ends there).
const NEW_MARK_ROOM: f32 = 0.84;

/// (the user: "never cut off") A one-line row text: set, and squeezed sideways as KH3's `TextFieldAdjust` does when it is
/// wider than its field (no lower limit here: a COMPOSED milestone line may be longer than KH3's own).
pub fn fit_line(movie: &mut Movie, path: &str, text: &str, fonts: &Fonts) {
    fit_line_in(movie, path, text, fonts, 1.0);
}

/// [`fit_line`] into the first `part` of the field (a NEW mark drawn over the field's end keeps its room).
fn fit_line_in(movie: &mut Movie, path: &str, text: &str, fonts: &Fonts, part: f32) {
    movie.set_text(path, text, fonts);
    let Some(crate::player::Node { kind: crate::player::Kind::Text(t), .. }) = movie.node_mut(path) else { return };
    let field_w = (t.def.bounds.x1 - t.def.bounds.x0) * part;
    let width = fonts.get(t.def.font_class.as_deref()).map_or(0.0, |font| crate::render::markup_width(fonts, font, &crate::controls::display_string(text), t.def.size)) + 5.0;
    t.adjust = Some(if width > field_w && field_w > 0.0 { (field_w / width, 0.0, width + 5.0) } else { (1.0, 0.0, field_w) });
}

/// (ASH-ROWS) `Dialog.<init>`: the window frame's authored matrix and the window's authored size (the content holder sits
/// at its top left corner, the window centred on the clip's origin: as `newgame` reads it).
fn dialog_frame(dialog: &Movie) -> (crate::geom::Matrix, (f32, f32)) {
    let base = dialog.node(DIALOG_WIN).map_or(crate::geom::Matrix::IDENTITY, |n| n.matrix);
    let size = dialog.node("dialog.dialogAnm.dialogPoAll").map_or((510.0, 186.0), |n| (-2.0 * n.matrix.tx, -2.0 * n.matrix.ty));
    (base, size)
}

/// (ASH-ROWS) A position holder's size (`poDialogText.width / height`: its one shape's bounds).
fn holder_size(dialog: &Movie, path: &str) -> (f32, f32) {
    let Some(holder) = dialog.sprite(path) else { return (0.0, 0.0) };
    let mut r = crate::geom::Rect::EMPTY;
    for n in &holder.children {
        if let crate::player::Kind::Shape(s) = &n.kind {
            r = r.union(&s.bounds.rect().transform(&n.matrix));
        }
    }
    if r.is_empty() {
        (0.0, 0.0)
    } else {
        (r.x1 - r.x0, r.y1 - r.y0)
    }
}

/// (FORGE-PREVIEW, the user's KH3 reference) The Forge's Keyblade name as KH3 shows it: the Keyblade mark (the icon font's
/// `Weapon` glyph, KH3's weapon category mark) before the name, on `infoKeyb.txName` and in the help line's `{0}`.
pub fn forge_name(name: &str) -> String {
    format!("<icon(Weapon)>{name}")
}

/// The rarity mark of a material's rank (KH3's `<icon(s_*)>`: RANK03 shards normal .. RANK00 ssrare).
pub fn rarity_icon(rank: u8) -> &'static str {
    match rank {
        3 => "<icon(s_normal)>",
        2 => "<icon(s_rare)>",
        1 => "<icon(s_srare)>",
        _ => "<icon(s_ssrare)>",
    }
}

pub struct WorkshopView {
    pub movie: Movie,
    /// KH3's system window (the `Dialog` movie).
    pub dialog: Movie,
    pub fonts: Fonts,
    strings: HashMap<String, String>,
    pub art: crate::item_art::ItemArt,
    /// (FORGE-PREVIEW) KH3's Forge pictures: the selector's keychains, the preview's whole Keyblades.
    pub forge_art: super::forge_art::ForgeArt,
    open: bool,
    closing: bool,
    age: u32,
    /// The screen whose clip is in, and since when.
    shown: Option<Screen>,
    shown_at: u32,
    stamp: u64,
    tabs: TabBar,
    guide: String,
    pane_item: Option<Option<String>>,
    photo_item: Option<Option<String>>,
    mes2_open: bool,
    mes2_stamp: u64,
    dialog_open: bool,
    dialog_closing: bool,
    dialog_stamp: u64,
    /// (ASH-ROWS) KH3's `Dialog.setText` layout of the window on show (None: the authored one), the frame's authored
    /// matrix and the window's authored size (`m_defaultWindowSize`).
    dialog_layout: Option<crate::newgame::DialogLayout>,
    dialog_win: (crate::geom::Matrix, (f32, f32)),
    tone_down: bool,
    /// The picker: the move playing (frames left, moved right), the static placement shown.
    picker_anim: Option<(u32, bool)>,
    picker_stamp: u64,
    effect_left: u32,
    materials: ListView,
    materials_placed: bool,
}

impl WorkshopView {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<WorkshopView, PackError> {
        let def: Arc<MovieDef> = pack.movie_linked(MOVIE, &crate::shop_moogle::LIBRARIES)?;
        let dialog = Movie::new(pack.movie_linked("Dialog", &crate::camp_menu::LIBRARIES)?);
        let dialog_win = dialog_frame(&dialog); // (ASH-ROWS)
        let mut strings = HashMap::new();
        for table in ["kh3_menu", "kh3_system", "kh3_item", "kh3_camp"] {
            strings.extend(pack.string_table(table));
        }
        let art = crate::item_art::ItemArt::load(pack);
        let forge_art = super::forge_art::ForgeArt::load(pack); // (FORGE-PREVIEW)
        let mut materials = ListView::new(super::MATERIAL_LIST, "CoMaterialListItem");
        materials.rows = MATERIAL_COLUMNS * MATERIAL_LINES;
        materials.has_focus = false;
        Ok(WorkshopView {
            movie: Movie::new(def),
            dialog_win,
            dialog_layout: None,
            dialog,
            fonts,
            strings,
            art,
            forge_art,
            open: false,
            closing: false,
            age: 0,
            shown: None,
            shown_at: 0,
            stamp: 0,
            tabs: TabBar::new(format!("{SYNTH}.synthesisList.coTab")),
            guide: String::new(),
            pane_item: None,
            photo_item: None,
            mes2_open: false,
            mes2_stamp: 0,
            dialog_open: false,
            dialog_closing: false,
            dialog_stamp: 0,
            tone_down: false,
            picker_anim: None,
            picker_stamp: 0,
            effect_left: 0,
            materials,
            materials_placed: false,
        })
    }

    /// A string (`<br>` a line break): the mod's own ([`texts::MOD`]: "Journey to the Erdtree"), else the pack's, else
    /// the id itself (KH3's text is never compiled in).
    pub fn string(&self, id: &str) -> String {
        texts::lines(&self.raw(id))
    }

    /// A string as KH3 wrote it (markup kept): the mod's own ([`texts::MOD`]), else the pack's, else the id itself.
    pub fn raw(&self, id: &str) -> String {
        if let Some(own) = texts::composed(id) {
            return own.to_string();
        }
        self.strings.get(id).filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| texts::missing(id))
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The pad drives the menu (the clips are in and no picker move / effect is playing).
    pub fn ready(&self) -> bool {
        self.open && !self.closing && self.age > SETTLE + 2 && self.age >= self.shown_at + SETTLE && self.picker_anim.is_none()
    }

    /// The system window when it is up (drawn over the screen).
    pub fn dialog_movie(&self) -> Option<&Movie> {
        self.dialog_open.then_some(&self.dialog)
    }

    fn text(&mut self, path: &str, text: &str) {
        self.movie.set_text(path, text, &self.fonts);
    }

    /// THE MENU OPENS straight on the Workshop's list (the Moogle) or on the Keyblade Forge (a smith): the root `in`; the
    /// stage on its Workshop tint, the frame and the screen's clip come in once the root placed them.
    pub fn open(&mut self, menu: &mut WorkshopMenu) {
        self.movie.goto_and_play("", "in");
        for clip in ["menuTop", "chara", "menuList", "mesWin", "mesWin2", "menuMixTop", "menuSynthesis", "menuKeyb", "menuPhoto", "menuCollection", "menuMaterial", "screenEffBlack"] {
            self.movie.goto_and_play(clip, "off");
        }
        self.open = true;
        self.closing = false;
        self.age = 0;
        self.shown = None;
        self.shown_at = 0;
        self.stamp = 0;
        self.guide.clear();
        self.pane_item = None;
        self.photo_item = None;
        self.mes2_open = false;
        self.mes2_stamp = 0;
        self.tone_down = false;
        self.picker_anim = None;
        self.picker_stamp = 0;
        self.effect_left = 0;
        self.materials_placed = false;
        menu.leaving = None;
        for list in [&mut menu.synth, &mut menu.photo, &mut menu.goals, &mut self.materials] {
            list.invalidate();
        }
    }

    fn start_parts(&mut self, screen: Screen) {
        self.movie.goto_and_play("stageBg", "mixIn");
        self.movie.goto_and_play("stageFrame", "in");
        self.movie.goto_and_play(screen.clip(), "inNext");
        self.shown = Some(screen);
        self.shown_at = self.age;
        self.stamp = 0;
    }

    /// The menu leaves (the screen's `outBack`, the frame's `out`, the root's `out`); [`Self::tick`] says when it is gone.
    pub fn close(&mut self) {
        if self.open && !self.closing {
            if let Some(screen) = self.shown {
                self.movie.goto_and_play(screen.clip(), "outBack");
            }
            if self.mes2_open {
                self.movie.goto_and_play("mesWin2", "out");
                self.mes2_open = false;
            }
            self.hide_dialog();
            self.movie.goto_and_play("stageFrame", "out");
            self.movie.goto_and_play("", "out");
            self.closing = true;
        }
    }

    fn set_guide(&mut self, text: &str) {
        if self.guide == text {
            return;
        }
        let plate = format!("{FRAME}.btnInfo");
        let shown = matches!(self.movie.current_label(&plate), Some("in" | "loop" | "in2"));
        if text.is_empty() {
            if shown {
                self.movie.goto_and_play(&plate, "out");
            }
        } else {
            if !shown {
                self.movie.goto_and_play(&plate, "in");
            }
            self.text(GUIDE, text);
        }
        self.guide = text.to_string();
    }

    /// The category strip of a screen: its name and icon.
    fn strip(&mut self, screen: Screen, name: &str, icon: &str) {
        let clip = screen.clip();
        self.text(&format!("{clip}.stageCategory.category.categoryName.txName.textField"), name);
        self.movie.goto_and_stop(&format!("{clip}.stageCategory.category.categoryIcon"), icon);
        self.movie.goto_and_stop(&format!("{clip}.stageCategory.stageObjL.bgCategoryIcon"), icon);
    }

    /// (THE USER 2026-11-03: "...never cut off") a description: shrunk to fit, then scrolled by the right stick.
    fn help(&mut self, path: &str, text: &str) {
        self.movie.set_text_fit(path, text, &self.fonts);
    }

    /// Reflects the logic: the screen changes, the frame, the screen's parts, the windows, the effects.
    pub fn sync(&mut self, menu: &mut WorkshopMenu) {
        if !self.open || self.closing {
            return;
        }
        if self.shown.is_none() {
            return;
        }
        // a screen change: the old clip leaves, the new one comes in (KH3's outNext / inNext, outBack / inBack)
        if let Some(from) = menu.leaving.take() {
            if from == Screen::Top {
                self.movie.goto_and_play("menuMixTop", "outNext");
                self.movie.goto_and_play(menu.screen.clip(), "inNext");
            } else {
                self.movie.goto_and_play(from.clip(), "outBack");
                self.movie.goto_and_play("menuMixTop", "inBack");
            }
            self.shown = Some(menu.screen);
            self.shown_at = self.age;
            self.stamp = 0;
            self.pane_item = None;
            self.photo_item = None;
            self.picker_stamp = 0;
            self.materials_placed = false;
            for list in [&mut menu.synth, &mut menu.photo, &mut menu.goals, &mut self.materials] {
                list.invalidate();
            }
        }
        if self.movie.exists(&format!("{FRAME}.munny.munnyNum.textField")) {
            let munny = menu.data.munny.clamp(0, 999_999).to_string();
            self.text(&format!("{FRAME}.munny.munnyNum.textField"), &munny);
            let word = self.string("kh3_menu.shop.munny_0010");
            self.text(&format!("{FRAME}.munny.txMunny.coText.textField"), &word);
            let title = if menu.data.title.is_empty() { self.string("kh3_menu.synthesis.title_0010") } else { menu.data.title.clone() };
            self.text(&format!("{FRAME}.areaInfo.areaName.txName.textField"), &title);
            self.movie.goto_and_stop(&format!("{FRAME}.areaInfo.worldTitle"), "off");
            self.movie.goto_and_stop(&format!("{FRAME}.playTime"), "off");
        }
        let stamp = stamp_of(&((&menu.data, menu.screen, menu.layer, menu.top, menu.tab, &menu.synth), (&menu.qty, menu.keyblade, &menu.photo, &menu.goals, menu.material_line, menu.confirm, menu.messages.len()))) ^ u64::from(self.age.saturating_sub(self.shown_at) < SETTLE + 4);
        if stamp != self.stamp {
            self.stamp = stamp;
            match menu.screen {
                Screen::Top => self.sync_top(menu),
                Screen::Synthesis => self.sync_synthesis(menu),
                Screen::Forge => self.sync_forge(menu),
                Screen::Photo => self.sync_photo(menu),
                Screen::Goals => self.sync_goals(menu),
                Screen::Materials => self.sync_materials(menu),
            }
            let guide = self.guide_of(menu);
            if self.age > SETTLE {
                self.set_guide(&guide);
            }
        }
        if menu.screen == Screen::Forge {
            self.sync_picker(menu);
            self.sync_effects(menu);
        }
        self.sync_mes2(menu);
        self.sync_dialog(menu);
    }

    /// The button guide plate (COMPOSED: KH3 sets none natively on these screens; the mod's screens all carry one).
    fn guide_of(&self, menu: &WorkshopMenu) -> String {
        let (yes, back) = ("<icon(Decide)>", "<icon(Cancel)>");
        if !menu.messages.is_empty() || menu.layer == Layer::Notice {
            return format!("{yes} {}", self.string("kh3_system.general.choice_0003"));
        }
        match (menu.screen, menu.layer) {
            (_, Layer::Effect) => String::new(),
            (_, Layer::Confirm) => format!("<icon(KeyLeftRight)> {} / {}  {yes} OK", self.string("kh3_system.general.choice_0001"), self.string("kh3_system.general.choice_0002")),
            (Screen::Top, _) => format!("{yes} Select  {back} Back"),
            (Screen::Synthesis, _) => format!("<icon(L2)><icon(R2)> Tabs  <icon(KeyLeftRight)> Change Quantity  {yes} Synthesize  {back} Back"),
            (Screen::Forge, _) => format!("<icon(KeyLeftRight)> Keyblade  {yes} Power Up  {back} Back"),
            (Screen::Materials, _) => format!("<icon(KeyUpDown)> Scroll  <icon(L1)><icon(R1)> Page  {back} Back"),
            (Screen::Photo | Screen::Goals, _) => format!("<icon(L1)><icon(R1)> Page  {back} Back"),
        }
    }

    fn sync_top(&mut self, menu: &WorkshopMenu) {
        let icon = menu.data.commands.get(menu.top).map_or(Screen::Top.icon(), |c| c.0.icon());
        let title = self.string("kh3_menu.synthesis.title_0010");
        self.strip(Screen::Top, &title, icon);
        for k in 0..5 {
            let path = format!("{TOP}.cmd{}", k + 1);
            match menu.data.commands.get(k).copied() {
                Some((screen, new)) => {
                    self.movie.set_visible(&path, true);
                    let name = self.string(&format!("kh3_menu.synthesis.menu_00{}0", screen.number()));
                    // (JOURNEY) a long name ("Journey to the Erdtree") is fitted to its field: never cut off, never under the NEW
                    // mark (drawn over the selected field's last seventh, where KH3's own names end)
                    fit_line_in(&mut self.movie, &format!("{path}.mainCmdNameAnm.mainCmdName.textField"), &name, &self.fonts, if new && k == menu.top { NEW_MARK_ROOM } else { 1.0 });
                    // the selected command's help line (detail_00n0)
                    let detail = self.string(&format!("kh3_menu.synthesis.detail_00{}0", screen.number()));
                    self.help(&format!("{path}.helpMessageAnm.helpMessage.textField"), &detail);
                    self.movie.goto_and_stop(&format!("{path}.categoryIcon"), screen.icon());
                    let state = if k == menu.top { "select" } else { "normal" };
                    if self.movie.current_label(&path) != Some(state) {
                        self.movie.goto_and_play(&path, state);
                    }
                    let mark = format!("{path}.newMark");
                    if self.movie.exists(&mark) {
                        self.movie.goto_and_play(&mark, if new { "loop" } else { "off" });
                    }
                }
                None => {
                    self.movie.set_visible(&path, false);
                }
            }
        }
        // the SYNTH LOG
        let log = menu.data.log;
        let photo_shown = menu.data.commands.iter().any(|c| c.0 == Screen::Photo);
        let title = self.string("kh3_menu.synthesis.log_0010");
        self.text(&format!("{LOG}.txTitle.textField"), &title);
        for (k, (id, n)) in [("kh3_menu.synthesis.log_0030", log.created), ("kh3_menu.synthesis.log_0040", log.photos), ("kh3_menu.synthesis.log_0050", log.goals)].into_iter().enumerate() {
            let name = if k == 1 && !photo_shown { self.string("kh3_menu.shop.item_0070") } else { self.string(id) };
            self.text(&format!("{LOG}.txLogName{}", k + 1), &name);
            self.text(&format!("{LOG}.num{}", k + 1), &n.to_string());
            self.movie.set_visible(&format!("{LOG}.num{}", k + 1), k != 1 || photo_shown);
            let complete = log.complete[k];
            let word = self.string("kh3_menu.synthesis.log_0020");
            self.text(&format!("{LOG}.completeMark{}.txComplete.textField", k + 1), &word);
            mark(&mut self.movie, &format!("{LOG}.completeMark{}", k + 1), complete);
        }
    }

    /// The materials pane (synthesis: need x quantity; Forge: the next level's).
    fn materials_pane(&mut self, root: &str, needs: &[super::MatNeed], times: u32, disabled: bool) {
        let pane = format!("{root}.infoMaterials");
        let heads = [("txItem", "kh3_menu.synthesis.item_0040"), ("txKosuu", "kh3_menu.synthesis.item_0050"), ("txShojisuu", "kh3_menu.synthesis.item_0030")];
        for (field, id) in heads {
            let t = self.string(id);
            self.text(&format!("{pane}.{field}"), &t);
        }
        let state = if disabled { "disable" } else { "normal" };
        if self.movie.current_label(&pane) != Some(state) {
            self.movie.goto_and_play(&pane, state);
        }
        let disable = format!("{pane}.labelDisable");
        self.movie.set_visible(&disable, disabled);
        if disabled {
            let t = self.string("kh3_menu.synthesis.materials_0010");
            self.text(&format!("{disable}.txText"), &t);
        }
        for k in 0..6 {
            let label = format!("{pane}.label{}", k + 1);
            match needs.get(k).filter(|_| !disabled) {
                Some(m) => {
                    self.movie.set_visible(&label, true);
                    let need = m.need.saturating_mul(times.max(1));
                    self.text(&format!("{label}.txIcon"), "<icon(MixItem)>");
                    self.text(&format!("{label}.txName"), &m.name);
                    self.text(&format!("{label}.numReq"), &need.to_string());
                    self.text(&format!("{label}.numShojisuu.textField"), &m.stock.to_string());
                    let lack = if m.stock < need { "lack" } else { "normal" };
                    if self.movie.current_label(&label) != Some(lack) {
                        self.movie.goto_and_play(&label, lack);
                    }
                }
                None => {
                    self.movie.set_visible(&label, false);
                }
            }
        }
    }

    /// The synthesis item pane (`ItemStatusA2`): the item's picture and its description (never cut: shrunk, scrolled).
    fn item_pane(&mut self, item: Option<&str>, help: &str) {
        let pane = format!("{SYNTH}.itemStatus");
        let texture = item.and_then(|id| self.art.icon(id));
        let label = if help.is_empty() && texture.is_none() { "off" } else { "loop" };
        if self.movie.current_label(&pane) != Some(label) {
            self.movie.goto_and_play(&pane, label);
        }
        self.movie.goto_and_stop(&format!("{pane}.parameters"), "off");
        self.help(&format!("{pane}.txHelp.textField"), help);
        let shown = texture.as_ref().and(item).map(str::to_string);
        if self.pane_item.as_ref() != Some(&shown) {
            let image = format!("{pane}.itemImage");
            self.movie.set_visible(&image, shown.is_some());
            self.movie.goto_and_play(&image, if shown.is_some() { "in" } else { "off" });
            if let Some(sprite) = self.movie.sprite_mut(&format!("{image}.imageAnm.coImage")) {
                crate::controls::icon_load(sprite, texture);
            }
            self.pane_item = Some(shown);
        }
    }

    fn list_lines(&mut self, list_root: &str, empty: bool) {
        let lines = format!("{list_root}.listLines");
        let want = if empty { "empty" } else { "loop" };
        if self.movie.current_label(&lines) != Some(want) {
            self.movie.goto_and_stop(&lines, want);
        }
        if empty {
            let t = self.string("kh3_system.general.system_0011");
            self.text(&format!("{lines}.txDataEmpty"), &t);
        }
    }

    fn sync_synthesis(&mut self, menu: &mut WorkshopMenu) {
        let name = self.string("kh3_menu.synthesis.menu_0010");
        self.strip(Screen::Synthesis, &name, Screen::Synthesis.icon());
        let list_root = format!("{SYNTH}.synthesisList");
        for (field, id) in [("txItem", "kh3_menu.synthesis.item_0010"), ("txKosuu", "kh3_menu.synthesis.item_0020"), ("txShojisuu", "kh3_menu.synthesis.item_0030")] {
            let t = self.string(id);
            self.text(&format!("{list_root}.{field}.textField"), &t);
        }
        // the six tabs
        let icons: Vec<i32> = TABS.iter().map(|t| t.1).collect();
        self.tabs.build(&mut self.movie, TABS.len(), &icons);
        self.tabs.set_switch_icons(&mut self.movie, "<icon(L2)>", "<icon(R2)>", &self.fonts);
        let tab_name = self.string(TABS[menu.tab].0);
        let fonts = self.fonts.clone();
        self.tabs.select(&mut self.movie, menu.tab, &tab_name, menu.tab_flash.take(), &fonts);
        let help = self.string("kh3_menu.synthesis.detail_0110");
        self.help(&format!("{SYNTH}.helpText.txHelpText.textField"), &help);
        let rows: Vec<super::RecipeRow> = menu.recipe_rows().into_iter().cloned().collect();
        self.list_lines(&list_root, rows.is_empty());
        let qty: Vec<u32> = rows.iter().map(|r| menu.qty_of(r)).collect();
        menu.synth.has_focus = menu.layer == Layer::Main || menu.layer == Layer::Confirm;
        menu.synth.refresh(&mut self.movie, |movie, path, index| {
            let row = &rows[index];
            movie.set_text(&format!("{path}.itemLabel.txIcon"), category_icon(row.category), &fonts);
            movie.set_text(&format!("{path}.itemLabel.txName.textField"), &row.name, &fonts);
            movie.set_text(&format!("{path}.numInput.textField"), &qty[index].to_string(), &fonts);
            movie.set_text(&format!("{path}.numShojisuu.textField"), &row.held.to_string(), &fonts);
            movie.goto_and_stop(&format!("{path}.arrow"), if row.max > 1 { "on" } else { "off" });
            mark(movie, &format!("{path}.clearMark"), row.made > 0);
            movie.goto_and_play(&format!("{path}.newMark"), if row.new { "loop" } else { "off" });
            RowState { enabled: row.max > 0 }
        });
        let focused = menu.recipe().cloned();
        match focused {
            Some(row) => {
                let n = menu.qty_of(&row);
                self.materials_pane(SYNTH, &row.materials, n, false);
                self.item_pane(Some(&row.key), &row.help);
            }
            None => {
                self.materials_pane(SYNTH, &[], 1, false);
                self.item_pane(None, "");
            }
        }
    }

    /// (FORGE-PREVIEW) The Forge preview's picture of a Keyblade: KH3's `KeybladeIconAsset` (512 px); without the pack's
    /// Forge pictures, the item picture (also the whole Keyblade).
    fn preview_picture(&mut self, key: &str) -> Option<Arc<crate::def::Texture>> {
        self.forge_art.keyblade(key).or_else(|| self.art.icon(key))
    }

    fn sync_forge(&mut self, menu: &mut WorkshopMenu) {
        let name = self.string("kh3_menu.synthesis.menu_0020");
        self.strip(Screen::Forge, &name, Screen::Forge.icon());
        let Some(row) = menu.forge_row().cloned() else {
            self.movie.goto_and_play(&format!("{KEYB}.infoKeyb"), "off");
            self.materials_pane(KEYB, &[], 1, true);
            return;
        };
        // (FORGE-PREVIEW) KH3 names the Keyblade with its Keyblade mark before it, on the name line and in the help line
        let shown = forge_name(&row.name);
        let help = texts::lines(&self.raw("kh3_menu.synthesis.detail_0210").replace("{0}", &shown));
        self.help(&format!("{KEYB}.helpText.txHelpText.textField"), &help);
        // the Keyblade's name and picture
        let info = format!("{KEYB}.infoKeyb");
        if !matches!(self.movie.current_label(&info), Some("in" | "loop")) {
            self.movie.goto_and_play(&info, "in");
        }
        self.text(&format!("{info}.txName"), &shown);
        let texture = self.preview_picture(&row.key); // (FORGE-PREVIEW) KH3's KeybladeIconAsset
        if let Some(sprite) = self.movie.sprite_mut(&format!("{info}.img.coImg")) {
            crate::controls::icon_load(sprite, texture);
        }
        // the level icons and the glow of the level reached
        let enh = format!("{KEYB}.infoEnhance");
        for k in 1..=10u8 {
            let state = if k > row.max_level {
                "off"
            } else if k <= row.level {
                if k == row.max_level { "unlockF" } else { "unlock" }
            } else if k == row.max_level {
                "lockF"
            } else {
                "lock"
            };
            let icon = format!("{enh}.icon{k}");
            if self.movie.current_label(&icon) != Some(state) {
                self.movie.goto_and_play(&icon, state);
            }
        }
        let glow = if row.level == 0 { "off".to_string() } else { format!("lv{}", row.level.min(10)) };
        if self.movie.current_label(&format!("{enh}.effLv")).map(str::to_string) != Some(glow.clone()) {
            self.movie.goto_and_play(&format!("{enh}.effLv"), &glow);
        }
        // Strength / Magic: now -> after the next level
        let at_max = row.next.is_none();
        for (k, (id, now, next)) in [("kh3_system.general.window_0110", row.strength, row.next.as_ref().map(|n| n.strength)), ("kh3_system.general.window_0120", row.magic, row.next.as_ref().map(|n| n.magic))].into_iter().enumerate() {
            let label = format!("{enh}.label{}", k + 1);
            let word = self.string(id);
            self.text(&format!("{label}.txName"), &word);
            self.text(&format!("{label}.txIcon"), "");
            self.text(&format!("{label}.num1.textField"), &now.to_string());
            self.text(&format!("{label}.num2.textField"), &next.unwrap_or(now).to_string());
            let state = if at_max { "final" } else if next.is_some_and(|n| n != now) { "up" } else { "normal" };
            if self.movie.current_label(&label) != Some(state) {
                self.movie.goto_and_play(&label, state);
            }
        }
        // the ability the next level adds
        let ability = row.next.as_ref().and_then(|n| n.ability.clone());
        let label3 = format!("{enh}.label3");
        self.movie.set_visible(&label3, ability.is_some());
        match ability {
            Some(a) => {
                if self.movie.current_label(&label3) != Some("loop") {
                    self.movie.goto_and_play(&label3, "loop");
                }
                self.text(&format!("{label3}.txIcon"), "<icon(Ability)>");
                self.text(&format!("{label3}.txName"), &a);
            }
            None => {
                self.movie.goto_and_play(&label3, "off");
            }
        }
        let needs = row.next.as_ref().map(|n| n.materials.clone()).unwrap_or_default();
        self.materials_pane(KEYB, &needs, 1, at_max);
    }

    /// The Keyblade picker: the owned Keyblades around the centre (p0), a step's slide (pNL / pNR, the entering one's
    /// `inL` / `inR`, the leaving one's `outL` / `outR`), the arrows when more than seven.
    fn sync_picker(&mut self, menu: &mut WorkshopMenu) {
        let n = menu.data.keyblades.len();
        let container = format!("{KEYB}.picker.container");
        if !self.movie.exists(&format!("{container}.coItem1")) {
            return;
        }
        if let Some(right) = menu.picker_move.take() {
            if n >= 2 {
                // the slots still show the old placement (the focus before the move): each slides one step
                let side = if right { "L" } else { "R" };
                let offsets = picker_offsets(n);
                for (slot, offset) in (-4..=4).enumerate() {
                    let path = format!("{container}.coItem{}", slot + 1);
                    if offsets.contains(&offset) {
                        self.movie.goto_and_play(&path, &format!("{}{side}", picker_label(offset)));
                        let edge = if right { offset == offsets[0] } else { Some(&offset) == offsets.last() };
                        if edge && n > offsets.len() {
                            self.movie.goto_and_play(&format!("{path}.anim"), &format!("out{side}"));
                        }
                    }
                }
                // the one coming in at the far side
                if n > offsets.len() {
                    let (enter, slot) = if right { (*offsets.last().unwrap_or(&0), 8) } else { (offsets[0], 0) };
                    let path = format!("{container}.coItem{}", slot + 1);
                    let k = (menu.keyblade as i32 + enter).rem_euclid(n as i32) as usize;
                    self.movie.set_visible(&path, true);
                    self.movie.goto_and_stop(&path, picker_label(enter));
                    self.movie.goto_and_play(&format!("{path}.anim"), &format!("in{side}"));
                    let texture = menu.data.keyblades.get(k).and_then(|r| self.forge_art.picker(&r.key)); // (FORGE-PREVIEW) the keychain
                    if let Some(sprite) = self.movie.sprite_mut(&format!("{path}.anim.btn.img.coImg")) {
                        crate::controls::icon_load(sprite, texture);
                    }
                }
                self.picker_anim = Some((PICKER_FRAMES, right));
                self.picker_stamp = 0;
                return;
            }
        }
        if self.picker_anim.is_some() {
            return;
        }
        let stamp = stamp_of(&(menu.keyblade, menu.data.keyblades.iter().map(|k| &k.key).collect::<Vec<_>>()));
        if stamp == self.picker_stamp {
            return;
        }
        self.picker_stamp = stamp;
        let offsets = picker_offsets(n);
        for (slot, offset) in (-4..=4).enumerate() {
            let path = format!("{container}.coItem{}", slot + 1);
            if !offsets.contains(&offset) {
                self.movie.set_visible(&path, false);
                continue;
            }
            self.movie.set_visible(&path, true);
            self.movie.goto_and_stop(&path, picker_label(offset));
            self.movie.goto_and_play(&format!("{path}.anim"), "loop");
            self.movie.goto_and_play(&format!("{path}.anim.btn"), if offset == 0 { "select" } else { "normal" });
            let k = (menu.keyblade as i32 + offset).rem_euclid(n.max(1) as i32) as usize;
            let texture = menu.data.keyblades.get(k).and_then(|r| self.forge_art.picker(&r.key)); // (FORGE-PREVIEW) the keychain
            if let Some(sprite) = self.movie.sprite_mut(&format!("{path}.anim.btn.img.coImg")) {
                crate::controls::icon_load(sprite, texture);
            }
        }
        let arrows = if n > 7 { "loop" } else { "off" };
        for side in ["arrowL", "arrowR"] {
            self.movie.goto_and_play(&format!("{KEYB}.picker.{side}"), arrows);
        }
    }

    /// The Forge's tone-down behind its windows and the power-up's effects.
    fn sync_effects(&mut self, menu: &mut WorkshopMenu) {
        let down = matches!(menu.layer, Layer::Confirm | Layer::Notice | Layer::Effect);
        if down != self.tone_down {
            self.movie.goto_and_play(&format!("{KEYB}.effStage"), if down { "in" } else { "out" });
            self.tone_down = down;
        }
        if let Some(level) = menu.enhance.take() {
            let max = menu.forge_row().map_or(10, |r| r.max_level);
            let fx = format!("{KEYB}.effEnhance");
            self.movie.goto_and_play(&fx, &format!("lv{}", level.clamp(1, 10)));
            self.movie.goto_and_play(&format!("{fx}.anmKeyb"), "in");
            self.movie.goto_and_play(&format!("{fx}.anmLock"), if level >= max { "inF" } else { "in" });
            if level >= max {
                self.movie.goto_and_play(&format!("{fx}.anmKeybF"), "in");
            }
            self.movie.goto_and_play(&format!("{fx}.anmWhiteout"), "in");
            let texture = menu.forge_row().map(|r| r.key.clone()).and_then(|k| self.preview_picture(&k)); // (FORGE-PREVIEW)
            for img in ["img1", "img2"] {
                if let Some(sprite) = self.movie.sprite_mut(&format!("{fx}.anmKeyb.{img}.coImg")) {
                    crate::controls::icon_load(sprite, texture.clone());
                }
            }
            self.effect_left = EFFECT_FRAMES;
        }
    }

    fn sync_photo(&mut self, menu: &mut WorkshopMenu) {
        let name = self.string("kh3_menu.synthesis.menu_0030");
        self.strip(Screen::Photo, &name, Screen::Photo.icon());
        let rows = menu.data.photos.clone();
        self.list_lines(&format!("{PHOTO}.photoList"), rows.is_empty());
        let fonts = self.fonts.clone();
        menu.photo.has_focus = true;
        menu.photo.refresh(&mut self.movie, |movie, path, index| {
            let row = &rows[index];
            fit_line(movie, &format!("{path}.txText.textField"), &row.text, &fonts); // (the user: never cut off)
            mark(movie, &format!("{path}.clearMark"), row.done);
            movie.goto_and_play(&format!("{path}.newMark"), if row.new { "loop" } else { "off" });
            RowState::default()
        });
        let focused = menu.photo_row().cloned();
        let help = focused.as_ref().map(|r| texts::lines(&r.help)).unwrap_or_default();
        self.help(&format!("{PHOTO}.helpText.txHelpText.textField"), &help);
        let info = format!("{PHOTO}.infoItem");
        let balloon = format!("{PHOTO}.fukidasi");
        let Some(row) = focused else {
            self.movie.goto_and_play(&info, "off");
            self.movie.goto_and_play(&balloon, "off");
            self.photo_item = None;
            return;
        };
        let shown = row.done.then(|| row.reward_key.clone());
        if self.photo_item.as_ref() != Some(&shown) {
            self.movie.goto_and_play(&info, "in");
            self.movie.goto_and_play(&balloon, "in");
            self.movie.goto_and_stop(&format!("{info}.body"), if row.done { "item" } else { "unknown" });
            let texture = shown.as_deref().and_then(|k| self.art.icon(k));
            if let Some(sprite) = self.movie.sprite_mut(&format!("{info}.body.coImage")) {
                crate::controls::icon_load(sprite, texture);
            }
            self.photo_item = Some(shown);
        }
        let reward = if row.done { row.reward_name.clone() } else { self.string("kh3_menu.shop.item_0070") };
        self.text(&format!("{info}.body.txName"), &reward);
    }

    fn sync_goals(&mut self, menu: &mut WorkshopMenu) {
        let name = self.string("kh3_menu.synthesis.menu_0040");
        self.strip(Screen::Goals, &name, Screen::Goals.icon());
        let help = self.string("kh3_menu.synthesis.detail_0310");
        self.help(&format!("{GOALS}.helpText.txHelpText.textField"), &help);
        let rows = menu.data.goals.clone();
        self.list_lines(&format!("{GOALS}.collectionList"), rows.is_empty());
        let fonts = self.fonts.clone();
        menu.goals.has_focus = true;
        menu.goals.refresh(&mut self.movie, |movie, path, index| {
            let row = &rows[index];
            fit_line(movie, &format!("{path}.txText.textField"), &row.text, &fonts); // (the user: never cut off)
            mark(movie, &format!("{path}.clearMark"), row.done);
            movie.goto_and_play(&format!("{path}.newMark"), "off");
            RowState::default()
        });
        let bonus = format!("{GOALS}.infoBonus");
        let Some(row) = menu.goal_row().cloned() else {
            self.movie.set_visible(&bonus, false);
            return;
        };
        self.movie.set_visible(&bonus, true);
        let text = match row.reward {
            RewardKind::Synthesis => self.string("kh3_menu.synthesis.bonus_0010"),
            RewardKind::Item => self.string("kh3_menu.synthesis.bonus_0020"),
            RewardKind::Buy => texts::lines(texts::MERCHANT_BONUS),
        };
        self.help(&format!("{bonus}.txText"), &text);
        for k in 0..4 {
            let label = format!("{bonus}.label{}", k + 1);
            match row.items.get(k) {
                Some((category, name)) => {
                    self.movie.set_visible(&label, true);
                    self.text(&format!("{label}.txIcon"), category_icon(*category));
                    self.text(&format!("{label}.txName"), name);
                }
                None => {
                    self.movie.set_visible(&label, false);
                }
            }
        }
    }

    fn sync_materials(&mut self, menu: &mut WorkshopMenu) {
        let name = self.string("kh3_menu.synthesis.menu_0050");
        self.strip(Screen::Materials, &name, Screen::Materials.icon());
        let help = self.string("kh3_menu.synthesis.dialog_0310");
        self.help(&format!("{MATS}.helpText.txHelpText.textField"), &help);
        let root = format!("{MATS}.materialList");
        for (field, id) in [("txItem1", "kh3_menu.synthesis.item_0010"), ("txItem2", "kh3_menu.synthesis.item_0010"), ("txShojisuu1", "kh3_menu.synthesis.item_0030"), ("txShojisuu2", "kh3_menu.synthesis.item_0030")] {
            let t = self.string(id);
            self.text(&format!("{root}.{field}.textField"), &t);
        }
        let rows = menu.data.materials.clone();
        self.list_lines(&root, rows.is_empty());
        let fonts = self.fonts.clone();
        self.materials.has_focus = false;
        self.materials.focus = -1;
        self.materials.count = rows.len();
        self.materials.scroll = menu.material_line * MATERIAL_COLUMNS;
        self.materials.refresh(&mut self.movie, |movie, path, index| {
            let row = &rows[index];
            movie.set_text(&format!("{path}.itemLabel.txIcon"), "<icon(MixItem)>", &fonts);
            movie.set_text(&format!("{path}.itemLabel.txName.textField"), &row.name, &fonts);
            movie.set_text(&format!("{path}.numShojisuu.textField"), &row.stock.to_string(), &fonts);
            movie.set_text(&format!("{path}.txRareIcon.txIcon"), rarity_icon(row.rank), &fonts);
            RowState::default()
        });
        // the grid: the list widget lays its rows in one column; KH3's has two (columnNum 2).
        // (SYNTH-MOOGLE, the user 2026-11-07: "???" drawn over "Lightning Crystal") The grid came from reading slots i0
        // and i1 back from the movie; once placed, i1 sits on i0's line, so the second visit (or the next talk to the
        // Moogle) read a pitch of 0 and stacked every line of a column on its first one. The places now come from the
        // list's own one-column layout (`ListView::slot_origin`), the same on every visit.
        if !self.materials_placed && self.movie.exists(&self.materials.item_path(0)) {
            let (x0, y0) = self.materials.slot_origin(0);
            let pitch = self.materials.slot_origin(1).1 - y0;
            for slot in 0..MATERIAL_COLUMNS * MATERIAL_LINES {
                let (col, line) = (slot % MATERIAL_COLUMNS, slot / MATERIAL_COLUMNS);
                self.movie.set_position(&self.materials.item_path(slot), x0 + col as f32 * MATERIAL_COLUMN_X, y0 + line as f32 * pitch, 0.0);
            }
            self.materials_placed = true;
        }
    }

    /// The synthesis confirmation (`mesWin2`): the row, "x n", "Proceed with synthesis?", Yes / No.
    fn sync_mes2(&mut self, menu: &WorkshopMenu) {
        let row = (menu.screen == Screen::Synthesis && menu.layer == Layer::Confirm).then(|| menu.recipe().cloned()).flatten();
        let Some(row) = row else {
            if self.mes2_open {
                self.movie.goto_and_play("mesWin2", "out");
                self.mes2_open = false;
                self.mes2_stamp = 0;
            }
            return;
        };
        if !self.mes2_open {
            self.movie.goto_and_play("mesWin2", "in");
            self.mes2_open = true;
        }
        let qty = menu.qty_of(&row);
        let stamp = stamp_of(&(&row, qty, menu.confirm));
        if stamp == self.mes2_stamp {
            return;
        }
        self.mes2_stamp = stamp;
        for (field, id) in [("txItem", "kh3_menu.synthesis.item_0010"), ("txKosuu", "kh3_menu.synthesis.item_0020")] {
            let t = self.string(id);
            self.text(&format!("{MES2}.{field}.textField"), &t);
        }
        self.text(&format!("{MES2}.itemLabel.txIcon"), category_icon(row.category));
        self.text(&format!("{MES2}.itemLabel.txName.textField"), &row.name);
        let times = self.string("kh3_menu.shop.item_0060").replace("{0}", &qty.to_string());
        self.text(&format!("{MES2}.numKosuu.textField"), &times);
        let ask = self.string("kh3_menu.synthesis.dialog_0110");
        self.text(&format!("{MES2}.txMesHelp.textField"), &ask);
        for (n, (button, id)) in [("mesBtn1", "kh3_system.general.choice_0001"), ("mesBtn2", "kh3_system.general.choice_0002")].into_iter().enumerate() {
            let label = self.string(id);
            self.text(&format!("{MES2}.{button}.mesBtnText.txMesBtnText.textField"), &label);
            let state = if menu.confirm == n { "select" } else { "normal" };
            if self.movie.current_label(&format!("{MES2}.{button}")) != Some(state) {
                self.movie.goto_and_play(&format!("{MES2}.{button}"), state);
            }
        }
    }

    /// KH3's system window: an entry dialog (OK), the Forge's lack of materials (OK), the Forge's Yes / No.
    fn sync_dialog(&mut self, menu: &WorkshopMenu) {
        let content: Option<(String, Vec<String>, usize)> = if let Some(m) = menu.message() {
            Some((texts::lines(&m.text), vec![self.string("kh3_system.general.choice_0003")], 0))
        } else if menu.screen == Screen::Forge && menu.layer == Layer::Notice {
            Some((self.string("kh3_menu.synthesis.dialog_0210"), vec![self.string("kh3_system.general.choice_0003")], 0))
        } else if menu.screen == Screen::Forge && menu.layer == Layer::Confirm {
            let name = menu.forge_row().map(|r| r.name.clone()).unwrap_or_default();
            let ask = texts::lines(&self.raw("kh3_menu.synthesis.dialog_0220").replace("{0}", &name));
            Some((ask, vec![self.string("kh3_system.general.choice_0001"), self.string("kh3_system.general.choice_0002")], menu.confirm))
        } else {
            None
        };
        let Some((text, buttons, cursor)) = content else {
            self.hide_dialog();
            return;
        };
        let stamp = stamp_of(&(&text, &buttons, cursor));
        if self.dialog_open && !self.dialog_closing && stamp == self.dialog_stamp {
            self.place_dialog_parts(buttons.len());
            return;
        }
        if !self.dialog_open || self.dialog_closing {
            self.dialog.goto_and_play("dialog", "in");
            if !self.dialog.exists("dialogButton2") {
                self.dialog.add_instance("", "DialogButton", "dialogButton2");
            }
        }
        self.dialog_open = true;
        self.dialog_closing = false;
        self.dialog_stamp = stamp;
        self.dialog.set_visible("dialogText", true);
        self.lay_dialog(&text, &buttons); // (ASH-ROWS) KH3's Dialog.setText: the window grows with its text
        for (n, name) in ["dialogButton", "dialogButton2"].into_iter().enumerate() {
            match buttons.get(n) {
                Some(label) => {
                    self.dialog.set_visible(name, true);
                    self.dialog.set_text(&format!("{name}.dialogBtnText.textField"), label, &self.fonts);
                    self.dialog.goto_and_play(name, if cursor == n { "select" } else { "normal" });
                }
                None => {
                    self.dialog.set_visible(name, false);
                }
            }
        }
        self.place_dialog_parts(buttons.len());
    }

    /// (ASH-ROWS) `Dialog.setText(text)` (method 192, [`crate::newgame::dialog_text_layout`]): the text set in a field as
    /// wide as its widest line, the window (`m_mesWin.width / height`, its scaling grid keeps the rim) and the content holder
    /// (`m_mesPoAll`) laid out around it, the buttons under its last line (`place_dialog_parts`). Without the field or
    /// its font: the text through the fit rule in the authored window, as before.
    fn lay_dialog(&mut self, text: &str, buttons: &[String]) {
        let def = match self.dialog.node(DIALOG_FIELD) {
            Some(crate::player::Node { kind: crate::player::Kind::Text(t), .. }) => Some(t.def.clone()),
            _ => None,
        };
        let button_def = match self.dialog.node("dialogButton.dialogBtnText.textField") {
            Some(crate::player::Node { kind: crate::player::Kind::Text(t), .. }) => Some(t.def.clone()),
            _ => None,
        };
        let Some((def, font)) = def.and_then(|d| self.fonts.get(d.font_class.as_deref()).cloned().filter(|_| d.size > 0.0).map(|f| (d, f))) else {
            self.dialog_layout = None;
            self.dialog.set_text_fit(DIALOG_FIELD, text, &self.fonts);
            return;
        };
        let fonts = self.fonts.clone();
        let text_width = text.lines().map(|line| crate::render::markup_width(&fonts, &font, line, def.size)).fold(0.0, f32::max);
        let buttons_width = button_def.and_then(|b| fonts.get(b.font_class.as_deref()).cloned().map(|f| (f, b.size))).map_or(0.0, |(f, size)| buttons.iter().map(|b| crate::render::markup_width(&fonts, &f, b, size)).fold(0.0, f32::max));
        let height = |field_w: f32| crate::newgame::text_height(&font, def.size, def.leading, crate::render::wrap_text(&fonts, &font, text, def.size, field_w - 4.0).lines().count());
        let holder = |movie: &Movie, name: &str| movie.node(&format!("{DIALOG_PO_ALL}.{name}")).map_or((0.0, 0.0), |n| (n.matrix.tx, n.matrix.ty));
        let (text_pos, button_pos) = (holder(&self.dialog, "poDialogText"), holder(&self.dialog, "poDialogButton"));
        let default_text = holder_size(&self.dialog, &format!("{DIALOG_PO_ALL}.poDialogText"));
        let (base, default_window) = self.dialog_win;
        let d = crate::newgame::dialog_text_layout(text_width, height, (buttons.len(), buttons_width), default_window, [text_pos, button_pos], default_text);
        let (sx, sy) = (d.layout.window.0 / default_window.0.max(1.0), d.layout.window.1 / default_window.1.max(1.0));
        if let Some(node) = self.dialog.node_mut(DIALOG_WIN) {
            node.matrix = crate::geom::Matrix::new(base.a * sx, base.b * sx, base.c * sy, base.d * sy, base.tx, base.ty);
            node.script_matrix = true;
        }
        self.dialog.set_position(DIALOG_PO_ALL, d.layout.holder.0, d.layout.holder.1, 0.0);
        self.dialog.set_text(DIALOG_FIELD, text, &self.fonts);
        // the field's width (`m_textTextField.width`): where the renderer wraps it; its left edge stays where it is
        if let Some(node) = self.dialog.node_mut(DIALOG_FIELD) {
            let tx = node.matrix.tx;
            if let crate::player::Kind::Text(t) = &mut node.kind {
                t.adjust = Some((1.0, tx + t.def.bounds.x0, d.field.0));
            }
        }
        self.dialog_layout = Some(d.layout);
    }

    fn hide_dialog(&mut self) {
        if self.dialog_open && !self.dialog_closing {
            self.dialog.goto_and_play("dialog", "out");
            self.dialog.set_visible("dialogText", false);
            for name in ["dialogButton", "dialogButton2"] {
                self.dialog.set_visible(name, false);
            }
            self.dialog_closing = true;
        }
    }

    /// The text and the buttons at the window's position holders (as the camp menu's dialogs).
    fn place_dialog_parts(&mut self, buttons: usize) {
        let holder = "dialog.dialogAnm.dialogPoAll.dialogPoAll2";
        let stage = |movie: &Movie, path: &str| -> Option<(f32, f32)> {
            let mut m = crate::geom::Matrix::IDENTITY;
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
            // (ASH-ROWS) the button canvas (`m_buttonCanvas.x / y` of Dialog.setText): under the text's last line, centred
            let (cx, cy) = self.dialog_layout.map_or((0.0, 0.0), |l| l.canvas);
            let (x, y) = (x + cx, y + cy);
            self.dialog.set_position("dialogButton", x, y, 0.0);
            if buttons > 1 {
                self.dialog.set_position("dialogButton2", x, y + crate::newgame::BUTTON_HEIGHT, 0.0);
            }
        }
    }

    /// One stage frame. True when the close animation ended (the menu is gone). `menu`: the power-up's end gives the
    /// pad back.
    pub fn tick(&mut self, menu: Option<&mut WorkshopMenu>) -> bool {
        if !self.open {
            return false;
        }
        self.age = self.age.saturating_add(1);
        self.movie.tick();
        if self.dialog_open {
            self.dialog.tick();
            if self.dialog_closing && self.dialog.current_label("dialog") == Some("off") {
                self.dialog_open = false;
                self.dialog_closing = false;
            }
        }
        self.dialog.take_events();
        if self.age == 2 && !self.closing {
            let screen = menu.as_ref().map_or(Screen::Top, |m| m.screen);
            self.start_parts(screen);
        }
        if let Some((left, right)) = self.picker_anim {
            self.picker_anim = (left > 1).then_some((left - 1, right));
            if self.picker_anim.is_none() {
                self.picker_stamp = 0;
            }
        }
        let mut ended = false;
        let mut effect_end = false;
        for event in self.movie.take_events() {
            match event.name.as_str() {
                "ANIM_END_OUT" if self.closing => ended = true,
                "ANIM_END_KEY_ENHANCE_EFFECT" => effect_end = true,
                _ => {}
            }
        }
        if self.effect_left > 0 {
            self.effect_left -= 1;
            if self.effect_left == 0 {
                effect_end = true;
            }
        }
        if effect_end {
            self.effect_left = 0;
            if let Some(m) = menu {
                m.effect_done();
            }
        }
        if self.closing && self.age > 4 && self.movie.current_label("") == Some("off") {
            ended = true;
        }
        if ended {
            self.open = false;
            self.closing = false;
            self.dialog_open = false;
            self.dialog_closing = false;
        }
        ended
    }
}

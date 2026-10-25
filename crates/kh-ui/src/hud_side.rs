//! `HudLeft` and `HudRight` (docs/sora-ui-spec.md sections 6 - 8): "bound" movies whose native
//! parts hold references to clips and text fields. Ported here:
//!
//! - HudLeft: Next LV (`HudLeftNextLv`), munny (`HudLeftGetMunny`), obtained item
//!   (`HudLeftGetItem`), and the left stack (`UTresUIP_HudLeft` update `0x14164B2D0`: a visible
//!   part's row is what the visible parts before it leave free);
//! - HudRight: the enemy gauge (`HudRightEnmHpPack` + the native update `0x14165F820` and the
//!   target rule `0x14158A860` / `0x14165A050`), the level-up popup (`HudRightLevelUp`).
//!
//! AS3 sources: `build/sora_pack/ui/as3/HudLeft.as3.txt`, `HudRight.as3.txt`.
//! NOT PORTED: information line, mission gauge, score, timers, mission counter, the minimap canvas
//! (host-supplied). (BN) The bonus windows (`HudRightGetBonus` x 5) and the windows' character image
//! (the host's `UI/Icon/MenuFace` bust) ARE ported: see the (BN) block below.

use std::sync::Arc;

use crate::controls::{self, FollowGauge};
use crate::def::MovieDef;
use crate::frames::gauge_frame;
use crate::player::{Movie, NumberProps};
use crate::render::Fonts;
use crate::UiSound;

/// Texts of the side HUDs' fixed labels. The movies' own component parameters hold a string
/// table key with an authoring fallback (`^kh3_system.general.system_0004{...}`); who replaces
/// it by the localised text in the game is not traced: here the host passes the English strings.
#[derive(Clone, Debug, PartialEq)]
pub struct SideStrings {
    /// `kh3_system.general.system_0004`.
    pub next_level: String,
    /// `kh3_system.general.window_0010`.
    pub obtained_munny: String,
    /// `kh3_system.general.window_0020`.
    pub level_up: String,
    /// `kh3_system.cockpit.shoot_0001`.
    pub lock: String,
    /// `kh3_system.general.levelup_0010` .. `0040`: Strength, Defense, Magic, Maximum AP.
    pub level_up_lines: [String; 4],
    /// (new game) `kh3_system.general.window_0040` "BONUS!" and `bonus_0010` .. `0050`: Maximum HP,
    /// Maximum MP, armor slot, accessory slot, item slot.
    pub bonus: String,
    pub bonus_lines: [String; 5],
}

impl Default for SideStrings {
    fn default() -> Self {
        SideStrings { next_level: "NEXT LEVEL".into(), obtained_munny: "OBTAINED MUNNY!".into(), level_up: "LEVEL UP!".into(), lock: "LOCK".into(), level_up_lines: ["".into(), "".into(), "".into(), "".into()], bonus: "BONUS!".into(), bonus_lines: ["".into(), "".into(), "".into(), "".into(), "".into()] }
    }
}

impl SideStrings {
    pub fn load(pack: &mut crate::def::Pack) -> SideStrings {
        let d = SideStrings::default();
        let mut get = |key: &str, fallback: &String| pack.string(key).unwrap_or_else(|| fallback.clone());
        let lines = [get("kh3_system.general.levelup_0010", &d.level_up_lines[0]), get("kh3_system.general.levelup_0020", &d.level_up_lines[1]), get("kh3_system.general.levelup_0030", &d.level_up_lines[2]), get("kh3_system.general.levelup_0040", &d.level_up_lines[3])];
        let bonus_lines = [get("kh3_system.general.bonus_0010", &d.bonus_lines[0]), get("kh3_system.general.bonus_0020", &d.bonus_lines[1]), get("kh3_system.general.bonus_0030", &d.bonus_lines[2]), get("kh3_system.general.bonus_0040", &d.bonus_lines[3]), get("kh3_system.general.bonus_0050", &d.bonus_lines[4])];
        let bonus = get("kh3_system.general.window_0040", &d.bonus);
        SideStrings { bonus, bonus_lines, next_level: get("kh3_system.general.system_0004", &d.next_level), obtained_munny: get("kh3_system.general.window_0010", &d.obtained_munny), level_up: get("kh3_system.general.window_0020", &d.level_up), lock: get("kh3_system.cockpit.shoot_0001", &d.lock), level_up_lines: lines }
    }
}

/// Static SE of the popups (audio spec 5.6): level up 12, item 33.
pub const SE_LEVEL_UP: u32 = 12;
pub const SE_ITEM: u32 = 33;

const LEFT: &str = "hudleft";
/// The stacked parts in the order of `HudLeft.parts` (only these three are driven).
const LEFT_PARTS: [&str; 3] = ["getItemS", "nextLv", "getMunny"];
/// Heights of the parts' `rect` clips (the row a visible part takes): shape 100 x 100 scaled
/// 1.16 for the item popup, 250 x 74 for the two counters.
const LEFT_HEIGHTS: [f32; 3] = [116.0, 74.0, 74.0];
/// The parts' authored y inside `hudleft`.
const LEFT_Y: f32 = 122.0;
/// At most four rows.
pub const LEFT_ROWS: usize = 4;

pub struct HudLeft {
    pub movie: Movie,
    pub fonts: Fonts,
    /// A part is shown (from `in` until its out callback).
    visible: [bool; 3],
    munny: i32,
    item_queue: Vec<(String, bool, Option<String>)>,
    /// (loot) The item pictures by item key (pack file data/item_icons.json: `TresItemData.m_IconAsset`); empty = no pictures.
    pub item_icons: std::collections::HashMap<String, Arc<crate::def::Texture>>,
    pub calls: Vec<String>,
}

impl HudLeft {
    pub fn new(def: Arc<MovieDef>, fonts: Fonts, strings: &SideStrings) -> HudLeft {
        let mut hud = HudLeft { movie: Movie::new(def), fonts, visible: [false; 3], munny: 0, item_queue: Vec::new(), item_icons: std::collections::HashMap::new(), calls: Vec::new() };
        for label in ["textNextLv", "textNextLv2"] {
            hud.movie.set_text(&format!("{LEFT}.nextLv.{label}.txNextLv.coTxNextLv"), &strings.next_level, &hud.fonts);
        }
        for label in ["textGetMunny", "textGetMunny2"] {
            hud.movie.set_text(&format!("{LEFT}.getMunny.{label}.txGetMunny.coTextGetMunny"), &strings.obtained_munny, &hud.fonts);
        }
        hud.movie.goto_and_play("", "loop");
        hud
    }

    /// (er-host, IMPL_ER17: a host that loads another save inside one session) Everything shown or queued is gone at once:
    /// the movie starts again as at creation (the item popup, the next-level and munny counters belonged to the old session).
    pub fn reset(&mut self, strings: &SideStrings) {
        let icons = std::mem::take(&mut self.item_icons);
        *self = HudLeft::new(self.movie.def.clone(), self.fonts.clone(), strings);
        self.item_icons = icons;
    }

    fn log(&mut self, call: String) {
        if self.calls.len() >= 128 {
            self.calls.remove(0);
        }
        self.calls.push(call);
    }

    /// One frame; the parts' `onAnimEnd` report their end (`OnCallback(id, CLOSE)`).
    pub fn tick(&mut self) {
        self.movie.tick();
        for event in self.movie.take_events() {
            let part = match event.name.as_str() {
                "ANIM_END_GETITEM_OUT" => 0,
                "ANIM_END_NEXTLV_OUT" => 1,
                "ANIM_END_GETMUNNY_OUT" => 2,
                _ => continue,
            };
            self.visible[part] = false;
            if part == 2 {
                self.munny = 0;
            }
            if part == 0 && !self.item_queue.is_empty() {
                // Further items were queued: the next one shows as this one closes.
                let (name, full, icon) = self.item_queue.remove(0);
                self.show_item_icon(icon.as_deref(), &name, full);
            }
            self.stack();
        }
    }

    /// `0x14164B2D0`: the y of each visible part = the first visible part's own y plus the
    /// heights of the visible parts before it; a fifth row is not shown. (The game slides a
    /// part to its new row; here it is placed at once.)
    fn stack(&mut self) {
        let mut y = LEFT_Y;
        let mut row = 0;
        for part in 0..3 {
            if !self.visible[part] {
                continue;
            }
            if row >= LEFT_ROWS {
                self.movie.goto_and_play(&format!("{LEFT}.{}", LEFT_PARTS[part]), "off");
                self.visible[part] = false;
                continue;
            }
            if let Some(node) = self.movie.node_mut(&format!("{LEFT}.{}", LEFT_PARTS[part])) {
                node.matrix.ty = y.round();
                node.script_matrix = true;
            }
            y += LEFT_HEIGHTS[part];
            row += 1;
        }
    }

    fn show(&mut self, part: usize) {
        self.movie.goto_and_play(&format!("{LEFT}.{}", LEFT_PARTS[part]), "in");
        self.visible[part] = true;
        self.stack();
    }

    /// `0x141589570(cockpit, n)`: `HudLeftNextLv.setParam(n)` and show (146 frames).
    pub fn show_next_lv(&mut self, a1: i32) {
        self.log(format!("nextLv.setParam({a1})"));
        let text = a1.to_string();
        for count in ["nextLvCountP", "nextLvCountP2"] {
            self.movie.set_text(&format!("{LEFT}.nextLv.{count}.nextLvCountC.textField"), &text, &self.fonts);
        }
        self.show(1);
    }

    /// `0x141587530(cockpit, amount)`: the counter accumulates while visible and restarts
    /// otherwise; it shows at most 9999. Returns the amount shown.
    pub fn show_munny(&mut self, amount: i32) -> i32 {
        let visible = self.visible[2];
        self.munny = (if visible { self.munny } else { 0 } + amount).min(9999);
        self.log(format!("getMunny.setParam({})", self.munny));
        let text = self.munny.to_string();
        for count in ["munnyCountP", "munnyCountP2"] {
            self.movie.set_text(&format!("{LEFT}.getMunny.{count}.munnyCountC.textField"), &text, &self.fonts);
        }
        if !visible {
            self.show(2);
        }
        self.munny
    }

    /// `0x141587510(cockpit, itemId, stockFull)`: one popup at a time, further items queue.
    /// `HudLeftGetItem.setParam(icon, name, stockFull)`. Returns true when it shows now.
    pub fn show_item(&mut self, name: &str, stock_full: bool) -> bool {
        self.show_item_icon(None, name, stock_full)
    }

    /// (loot) [`HudLeft::show_item`] with the item's picture: `icon` = the item key (ITEM_POTION, MAT_ITEM01 ...) whose
    /// texture `m_icon.load(path)` draws into the clip `itemIconSAnm.itemIconS` (`tres.controls.Icon`: scaled to the
    /// clip's default size). `None` or an unknown key: the picture is unloaded (nothing is drawn there).
    pub fn show_item_icon(&mut self, icon: Option<&str>, name: &str, stock_full: bool) -> bool {
        if self.visible[0] {
            self.item_queue.push((name.to_string(), stock_full, icon.map(str::to_string)));
            return false;
        }
        self.log(format!("getItemS.setParam({icon:?}, {name:?}, {stock_full})"));
        let part = format!("{LEFT}.getItemS");
        let texture = icon.and_then(|key| self.item_icons.get(key)).cloned();
        if let Some(sprite) = self.movie.sprite_mut(&format!("{part}.itemIconSAnm.itemIconS")) {
            crate::controls::icon_load(sprite, texture);
        }
        self.movie.set_text(&format!("{part}.txItemNameS.coItemNameSTxt"), name, &self.fonts);
        self.movie.goto_and_stop(&format!("{part}.itemFullMes"), if stock_full { "on" } else { "off" });
        let col = if stock_full { "col1" } else { "col0" };
        self.movie.goto_and_stop(&format!("{part}.getCol"), col);
        self.movie.goto_and_stop(&format!("{part}.getItemLineCol"), col);
        self.show(0);
        true
    }

    pub fn is_visible(&self, part: usize) -> bool {
        self.visible.get(part).copied().unwrap_or(false)
    }

    pub fn munny_shown(&self) -> i32 {
        self.munny
    }
}

const ENM: &str = "hudRight.enmHpPack";
const ENM_SET: &str = "hudRight.enmHpPack.enmHpSetAll.enmHpSet";
const LEVEL_UP: &str = "hudRight.levelUp";
/// (party) `HudRight.PARTS_LEVELUP_MAX` [A]: the movie's document class duplicates `levelUp` four times
/// (`Utility.duplicateMulti`, same x / y) so that several characters' windows show at once.
pub const LEVEL_UP_WINDOWS: usize = 5;
/// (party) The windows' stacking: the native layout (`0x141660210` with `0x141652A20`) puts each visible part at
/// the first visible part's base y plus the heights (part +0x104) of the visible parts before it.
/// (party I2) The heights READ: UTresUIP_HudRightLevelUp init 0x141653B10 measures the clip's members `rect1` and
/// `rect3` (0x14163B6F0: GFx GetMember(name) then its "height" member = AS3 `DisplayObject.height`, the bounds in
/// the parent's space) into part +0x114 / +0x118; setParam 0x14165D4B0 sets +0x104 = +0x118 when a third line is
/// given, else +0x114. `LEVEL_UP_STEP` is only the fallback when the movie lacks the two rects.
const LEVEL_UP_BASE: (f32, f32) = (0.0, 71.0);
const LEVEL_UP_STEP: f32 = 80.0;

fn level_up_path(i: usize) -> String {
    if i == 0 { LEVEL_UP.to_string() } else { format!("{LEVEL_UP}{}", i + 1) }
}
const GET_BONUS: &str = "hudRight.getBonus";

// ------------------------------------------------------------------------------------------------------------------
// (BN) KH3'S BONUS! / LEVEL UP! PANELS AS THE GAME BUILDS THEM
//
// - `HudRight.PARTS_GETBONUS_MAX` [A] = 5: the document class duplicates `getBonus` four times, as it does `levelUp`.
// - 0x1415874F0 (cockpit, pawn, entries): the first bonus window that is free is filled (0x14165D2C0) and shown; a window
//   already showing the same character queues the entries behind its panel (0x141659B40: list at part +0x120).
// - 0x14165D2C0: chara = 0x141661740(pawn) (the window colour, `CHARA_LABLES`), icon = the pawn data's character image
//   (pawn +0x6F0 -> +0xA8, a soft texture path: the `UI/Icon/MenuFace` bust, 256 x 256), each entry's text from
//   0x141596150: kinds 1 - 4 `levelup_0010 .. 0040`, 5 / 6 `bonus_0010 / 0020`, 9 - 11 `bonus_0030 .. 0050` (NO icon),
//   7 the ability `bonus_0060` = 0x14157D0E0(..., icon 1) = `<icon(Ability)>` + name (0x14157CD70: table 0x1415969F0
//   index 0x11), 8 the command `bonus_0070` = 0x14157D620(..., icon 1) = icon of the command's category + name
//   (0x1415B2130: magic -> 0xE `<icon(Magic)>`, item -> 5 `<icon(BattleItem)>`, link -> 0x10 `<icon(Link)>`).
// - THE STACK (UTresUIP_HudRight init 0x141653170 + layout 0x141660210 / 0x141652A20): one list of the right-hand parts
//   in the movie's order (mission counter, timer, the five level-up windows, the five bonus windows); a part shown
//   anew is put at once at the y of (the first visible part's own y + the heights of the visible parts before it); a
//   part already shown whose row changes SLIDES there (0x141659E80: tween type 1 = linear, 0.15 s, from its current y).
// ------------------------------------------------------------------------------------------------------------------

/// (BN) `HudRightGetBonus.CHARA_LABLES` = `HudRightLevelUp.CHARA_LABLES` [A]: the window colour frames of
/// `lvupBonusWin1Col` / `lvupBonusWin3Col` (0x141661740: Sora 0, Donald 1, Goofy 2, the other friends 3 / 4).
pub const PANEL_COLOURS: [&str; 5] = ["sora", "donald", "goofy", "fnpc3", "fnpc4"];

/// (BN) A colour label of [`PANEL_COLOURS`] -> its index (an unknown label: `fnpc4`, 0x141661740's default).
pub fn panel_colour(label: &str) -> usize {
    PANEL_COLOURS.iter().position(|c| c.eq_ignore_ascii_case(label)).unwrap_or(4)
}

/// (BN) `HudRight.PARTS_GETBONUS_MAX` [A].
pub const GET_BONUS_WINDOWS: usize = 5;

/// (BN) 0x141659E80: a part moving to a new row slides there in 0.15 s (linear).
pub const STACK_SLIDE_SECONDS: f32 = 0.15;

fn get_bonus_path(i: usize) -> String {
    if i == 0 { GET_BONUS.to_string() } else { format!("{GET_BONUS}{}", i + 1) }
}

/// (BN) WHO A WINDOW IS FOR (0x14165D2C0's `chara` + `icon`): the host fills it from its roster data, never by name.
#[derive(Clone, Default)]
pub struct PanelChara {
    /// The identity (KH3: the pawn's unique id at part +0x114): one window per character, further panels queue on it.
    pub id: usize,
    /// The window colour, an index of [`PANEL_COLOURS`].
    pub colour: usize,
    /// The portrait (`charaImage.coImage`, a `tres.controls.Icon`): the character's `UI/Icon/MenuFace` bust.
    pub portrait: Option<Arc<crate::def::Texture>>,
}

impl PanelChara {
    /// A window without a portrait whose colour is `chara`'s (0 Sora, 1 Donald, 2 Goofy ...): the older calls.
    pub fn plain(chara: usize) -> PanelChara {
        PanelChara { id: chara, colour: chara.min(PANEL_COLOURS.len() - 1), portrait: None }
    }
}

impl std::fmt::Debug for PanelChara {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PanelChara {{ id: {}, colour: {}, portrait: {} }}", self.id, PANEL_COLOURS.get(self.colour).copied().unwrap_or("?"), self.portrait.as_ref().map_or("none".to_string(), |t| t.file.clone()))
    }
}

/// (BN) THE ICON IN FRONT OF A REWARD LINE: KH3's `IconText` tags (table 0x1415969F0), as 0x141596150 picks them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineIcon {
    /// A stat or slot line (kinds 1 - 6, 9 - 11: "Maximum HP increased!"): no icon.
    None,
    /// An ability (kind 7): the sparkle `<icon(Ability)>`.
    Ability,
    /// A magic command (kind 8, category magic): `<icon(Magic)>`.
    Magic,
    /// The other command categories of kind 8 and the item kinds of the same table.
    BattleItem,
    CampItem,
    Link,
    Weapon,
    Armor,
    Accessory,
    KeyItem,
    Food,
    MixItem,
}

impl LineIcon {
    /// The text tag the panel's text field draws (`<icon(Name)>`, the `IconTextData` atlas); "" for none.
    pub const fn tag(self) -> &'static str {
        match self {
            LineIcon::None => "",
            LineIcon::Ability => "<icon(Ability)>",
            LineIcon::Magic => "<icon(Magic)>",
            LineIcon::BattleItem => "<icon(BattleItem)>",
            LineIcon::CampItem => "<icon(CampItem)>",
            LineIcon::Link => "<icon(Link)>",
            LineIcon::Weapon => "<icon(Weapon)>",
            LineIcon::Armor => "<icon(Armor)>",
            LineIcon::Accessory => "<icon(Accessory)>",
            LineIcon::KeyItem => "<icon(KeyItem)>",
            LineIcon::Food => "<icon(Food)>",
            LineIcon::MixItem => "<icon(MixItem)>",
        }
    }
}

/// (BN) A panel line with its icon in front ("{icon}{name}", 0x14157D2B0); a text that has an icon already keeps it.
pub fn icon_line(icon: LineIcon, text: &str) -> String {
    if text.is_empty() || text.starts_with("<icon(") {
        return text.to_string();
    }
    format!("{}{text}", icon.tag())
}
/// (new game) Static SE of the bonus popup (audio spec 5.6: index 38, `se00000_073`).
pub const SE_BONUS: u32 = 38;
/// `HudRightEnmHpPack.ENEMY_HP_STOCK_MAX`.
pub const ENEMY_HP_STOCK_MAX: i32 = 20;

pub struct HudRight {
    pub movie: Movie,
    pub fonts: Fonts,
    e_hp_follow: FollowGauge,
    e_hpg_stock_max: i32,
    e_hpg_stock: i32,
    visible_add_s: bool,
    level_up_visible: bool,
    /// (party) Each level-up window's character while it is on screen (`+0x11C`), None = free.
    level_ups: [Option<usize>; LEVEL_UP_WINDOWS],
    /// (party I2) Per window: a third line was given (its height is rect3's), and the measured (rect1, rect3) heights.
    level_up_three: [bool; LEVEL_UP_WINDOWS],
    level_up_height: (f32, f32),
    get_bonus_visible: bool,
    enemy_shown: bool,
    pub calls: Vec<String>,
    /// (BN) Each bonus window's character while it is on screen, None = free; the panels queued behind it (0x141659B40).
    bonuses: [Option<usize>; GET_BONUS_WINDOWS],
    bonus_queue: [std::collections::VecDeque<(PanelChara, Vec<String>)>; GET_BONUS_WINDOWS],
    /// (BN) The bonus window's height (its `rect1`, 0x141653920), the authored y of the two window kinds.
    bonus_height: f32,
    base_y: (f32, f32),
    /// (BN) The stack (level-up windows 0 - 4, then bonus windows 0 - 4): row (-1 = not placed), y now, slide.
    stack_row: [i32; LEVEL_UP_WINDOWS + GET_BONUS_WINDOWS],
    stack_y: [f32; LEVEL_UP_WINDOWS + GET_BONUS_WINDOWS],
    stack_slide: [Option<(f32, f32, f32)>; LEVEL_UP_WINDOWS + GET_BONUS_WINDOWS],
}

impl HudRight {
    pub fn new(def: Arc<MovieDef>, fonts: Fonts, strings: &SideStrings) -> HudRight {
        let mut hud = HudRight { movie: Movie::new(def), fonts, e_hp_follow: FollowGauge::with_scale(2.0), e_hpg_stock_max: 0, e_hpg_stock: 0, visible_add_s: false, level_up_visible: false, level_ups: [None; LEVEL_UP_WINDOWS], level_up_three: [false; LEVEL_UP_WINDOWS], level_up_height: (LEVEL_UP_STEP, LEVEL_UP_STEP), get_bonus_visible: false, enemy_shown: false, calls: Vec::new(), bonuses: [None; GET_BONUS_WINDOWS], bonus_queue: Default::default(), bonus_height: LEVEL_UP_STEP, base_y: (LEVEL_UP_BASE.1, LEVEL_UP_BASE.1), stack_row: [-1; LEVEL_UP_WINDOWS + GET_BONUS_WINDOWS], stack_y: [0.0; LEVEL_UP_WINDOWS + GET_BONUS_WINDOWS], stack_slide: [None; LEVEL_UP_WINDOWS + GET_BONUS_WINDOWS] };
        // HudRightEnmHpPack.<init>
        for gauge in ["enmHpGg", "enmHpGgR", "enmHpDodai"] {
            hud.movie.stop(&format!("{ENM_SET}.enmHp.{gauge}"));
        }
        hud.movie.stop(&format!("{ENM}.armorHpSet.armorHp.armorHpGg"));
        hud.movie.stop(&format!("{ENM}.armorHpSet.armorHp.armorHpGgR"));
        hud.movie.goto_and_play(&format!("{ENM}.enmHpSetAll"), "loop");
        // McEnmHpAddSSet_60.__setProp_enmHpAddSNum_*: bitmap HudRightEnmHPNum 24 x 23, 2 digits,
        // kerning -4, aligned right, origin centre.
        if let Some(s) = hud.movie.sprite_mut(&format!("{ENM_SET}.enmHpAddS.enmHpAddSCount.enmHpAddSSet.enmHpAddSNum")) {
            controls::number_configure(s, NumberProps { bmp_name: "HudRightEnmHPNum".into(), bmp_width: 24, bmp_height: 23, bmp_start_x: 0, bmp_start_y: 0, num_digit: 2, zero_padding: false, kerning: -4, align: "right", origin_anim: "center", whole_anim: true, number: 0 });
        }
        // (party) the four duplicates of the level-up window (HudRight document class init)
        for i in 1..LEVEL_UP_WINDOWS {
            let name = format!("levelUp{}", i + 1);
            if hud.movie.add_instance("hudRight", "McLevelUp", &name) {
                hud.movie.set_position(&level_up_path(i), LEVEL_UP_BASE.0, LEVEL_UP_BASE.1, 0.0);
            }
        }
        for i in 0..LEVEL_UP_WINDOWS {
            hud.movie.set_text(&format!("{}.levelUpSet.levelUpTitle.coLevelUpTitle", level_up_path(i)), &strings.level_up, &hud.fonts);
        }
        // (BN) the four duplicates of the bonus window (HudRight document class init: `Utility.duplicateMulti(getBonus,
        // PARTS_GETBONUS_MAX - 1)`, at the original's place)
        let bonus_at = hud.movie.node(GET_BONUS).map(|n| n.matrix);
        if let Some(m) = bonus_at {
            hud.base_y.1 = m.ty;
            for i in 1..GET_BONUS_WINDOWS {
                let name = format!("getBonus{}", i + 1);
                if hud.movie.add_instance("hudRight", "McGetBonus", &name) {
                    if let Some(node) = hud.movie.node_mut(&get_bonus_path(i)) {
                        node.matrix = m;
                        node.script_matrix = true;
                    }
                }
            }
        }
        if let Some(n) = hud.movie.node(LEVEL_UP) {
            hud.base_y.0 = n.matrix.ty;
        }
        for i in 0..GET_BONUS_WINDOWS {
            hud.movie.set_text(&format!("{}.getBonusSet.getBonusTitle.coGetBonusTitle", get_bonus_path(i)), &strings.bonus, &hud.fonts);
        }
        // (BN) the lines' icons at the IconText table's size, as the game's HTML text draws them
        for i in 0..GET_BONUS_WINDOWS {
            for k in 1..=2 {
                hud.movie.set_text_icon_native(&format!("{}.getBonusSet.txGetBonusCapacity{k}.textField", get_bonus_path(i)), true);
            }
        }
        for i in 0..LEVEL_UP_WINDOWS {
            for k in 1..=3 {
                hud.movie.set_text_icon_native(&format!("{}.levelUpSet.txLevelUpCapacity{k}.textField", level_up_path(i)), true);
            }
        }
        hud.movie.goto_and_play("", "loop");
        // (party I2) 0x141653B10: the window heights from the clip's rect1 / rect3
        let measure = |movie: &Movie, name: &str| -> Option<f32> {
            let node = movie.node(&format!("{LEVEL_UP}.{name}"))?;
            let b = crate::list::local_bounds(node.sprite()?).transform(&node.matrix);
            (!b.is_empty()).then_some(b.y1 - b.y0)
        };
        if let (Some(two), Some(three)) = (measure(&hud.movie, "rect1"), measure(&hud.movie, "rect3")) {
            hud.level_up_height = (two, three);
        }
        // (BN) 0x141653920: the bonus window's height is its `rect1`'s
        let bonus_rect = hud.movie.node(&format!("{GET_BONUS}.rect1")).and_then(|node| {
            let b = crate::list::local_bounds(node.sprite()?).transform(&node.matrix);
            (!b.is_empty()).then_some(b.y1 - b.y0)
        });
        if let Some(h) = bonus_rect {
            hud.bonus_height = h;
        }
        hud
    }

    /// (BN) The bonus window's measured height (its rect1), stage pixels.
    pub fn bonus_window_height(&self) -> f32 {
        self.bonus_height
    }

    /// (BN) Bonus window `i`'s stage y (tests).
    pub fn bonus_y(&self, i: usize) -> f32 {
        self.movie.node(&get_bonus_path(i)).map_or(0.0, |n| n.matrix.ty)
    }

    /// (BN) The character each bonus window shows (None = free) and how many panels wait behind it (tests, logs).
    pub fn bonus_windows(&self) -> Vec<(Option<usize>, usize)> {
        (0..GET_BONUS_WINDOWS).map(|w| (self.bonuses[w], self.bonus_queue[w].len())).collect()
    }

    /// (BN) The text of a bonus window's line (tests).
    pub fn bonus_line(&self, window: usize, line: usize) -> Option<String> {
        self.movie.text(&format!("{}.getBonusSet.txGetBonusCapacity{}.textField", get_bonus_path(window), line + 1))
    }

    /// (BN) The colour label a bonus / level-up window stands on (tests).
    pub fn window_colour(&self, bonus: bool, window: usize) -> Option<String> {
        let path = if bonus { format!("{}.getBonusSet.lvupBonusWin1.lvupBonusWin1Col", get_bonus_path(window)) } else { format!("{}.levelUpSet.lvupBonusWin1.lvupBonusWin1Col", level_up_path(window)) };
        self.movie.current_label(&path).map(str::to_string)
    }

    /// (BN) Whether a window's portrait holds a picture (tests).
    pub fn window_has_portrait(&self, bonus: bool, window: usize) -> bool {
        let path = if bonus { format!("{}.getBonusSet.charaImage.coImage", get_bonus_path(window)) } else { format!("{}.levelUpSet.charaImage.coImage", level_up_path(window)) };
        self.movie.sprite(&path).is_some_and(|s| !s.gfx.is_empty())
    }

    fn stack_path(p: usize) -> String {
        if p < LEVEL_UP_WINDOWS { level_up_path(p) } else { get_bonus_path(p - LEVEL_UP_WINDOWS) }
    }

    fn stack_visible(&self, p: usize) -> bool {
        if p < LEVEL_UP_WINDOWS { self.level_ups[p].is_some() } else { self.bonuses[p - LEVEL_UP_WINDOWS].is_some() }
    }

    /// Part +0x100 (its own y) and +0x104 (its height) of stack part `p`.
    fn stack_base_height(&self, p: usize) -> (f32, f32) {
        if p < LEVEL_UP_WINDOWS {
            (self.base_y.0, if self.level_up_three[p] { self.level_up_height.1 } else { self.level_up_height.0 })
        } else {
            (self.base_y.1, self.bonus_height)
        }
    }

    /// 0x141652A20: the y of part `p` = the first visible part's own y + the heights of the visible parts before it
    /// (none visible before: its own y).
    fn stack_target(&self, p: usize) -> f32 {
        let mut y = 0.0f32;
        for q in 0..p {
            if self.stack_visible(q) {
                let (base, height) = self.stack_base_height(q);
                if y.abs() <= 1e-8 {
                    y = base;
                }
                y += height;
            }
        }
        if y.abs() <= 1e-8 { self.stack_base_height(p).0 } else { y }
    }

    fn stack_place(&mut self, p: usize) {
        let y = self.stack_y[p].round();
        if let Some(node) = self.movie.node_mut(&Self::stack_path(p)) {
            node.matrix.ty = y;
            node.script_matrix = true;
        }
    }

    /// 0x141660210 (the dirty pass): a part shown anew goes to its row at once; a part already placed slides to its row
    /// (0x141659E80, linear, 0.15 s); a hidden part loses its row.
    fn layout_stack(&mut self) {
        let mut row = 0;
        for p in 0..LEVEL_UP_WINDOWS + GET_BONUS_WINDOWS {
            if !self.stack_visible(p) {
                self.stack_row[p] = -1;
                self.stack_slide[p] = None;
                continue;
            }
            let target = self.stack_target(p);
            if self.stack_row[p] < 0 {
                self.stack_y[p] = target;
                self.stack_slide[p] = None;
                self.stack_place(p);
            } else if (target - self.stack_y[p]).abs() > 1e-3 || self.stack_slide[p].is_some_and(|s| (s.1 - target).abs() > 1e-3) {
                self.stack_slide[p] = Some((self.stack_y[p], target, 0.0));
            }
            self.stack_row[p] = row;
            row += 1;
        }
    }

    /// The slides of one frame (0x141660210's first loop).
    fn slide_stack(&mut self, dt: f32) {
        for p in 0..LEVEL_UP_WINDOWS + GET_BONUS_WINDOWS {
            let Some((from, to, t)) = self.stack_slide[p] else { continue };
            let t = t + dt / STACK_SLIDE_SECONDS;
            self.stack_y[p] = if t >= 1.0 { to } else { from + (to - from) * t };
            self.stack_slide[p] = (t < 1.0).then_some((from, to, t));
            self.stack_place(p);
        }
    }

    /// (party I2) The level-up window heights (rect1: one or two lines, rect3: three lines), stage pixels.
    pub fn level_up_heights(&self) -> (f32, f32) {
        self.level_up_height
    }

    /// (party I2) Window `i`'s stage y (tests).
    pub fn level_up_y(&self, i: usize) -> f32 {
        self.movie.node(&level_up_path(i)).map_or(0.0, |n| n.matrix.ty)
    }

    /// (er-host, IMPL_ER17) As `HudLeft::reset`: the level-up and bonus popups and the enemy gauge of the old session are gone.
    pub fn reset(&mut self, strings: &SideStrings) {
        *self = HudRight::new(self.movie.def.clone(), self.fonts.clone(), strings);
    }

    fn log(&mut self, call: String) {
        if self.calls.len() >= 128 {
            self.calls.remove(0);
        }
        self.calls.push(call);
    }

    /// One frame: timelines, the enemy gauge's red lag, the popups' end callbacks.
    pub fn tick(&mut self, dt: f32) {
        self.movie.tick();
        let was_begun = self.e_hp_follow.begun;
        if let Some(frame) = self.e_hp_follow.advance(dt as f64 * 1000.0) {
            self.movie.goto_frame(&format!("{ENM_SET}.enmHp.enmHpGgR"), frame);
            if !was_begun && self.e_hp_follow.begun {
                self.on_ehp_begin_follow();
            }
        }
        self.slide_stack(dt); // (BN)
        for event in self.movie.take_events() {
            if event.name == "ANIM_END_LEVELUP_OUT_END" {
                // (party) the window that sent it is free again (its path names it)
                let window = (0..LEVEL_UP_WINDOWS).rev().find(|i| event.path.starts_with(&level_up_path(*i))).unwrap_or(0);
                self.level_ups[window] = None;
                self.level_up_visible = self.level_ups.iter().any(Option::is_some);
                // (BN) HudRightLevelUp.onAnimEnd: `m_iconChara.unload()`
                if let Some(s) = self.movie.sprite_mut(&format!("{}.levelUpSet.charaImage.coImage", level_up_path(window))) {
                    crate::controls::icon_load(s, None);
                }
                self.layout_level_ups();
            }
            if event.name == "ANIM_END_GETBONUS_OUT_END" {
                // (BN) the window that sent it: its next queued panel (0x141659B40's list), else it is free again
                let window = (0..GET_BONUS_WINDOWS).rev().find(|i| event.path.starts_with(&get_bonus_path(*i))).unwrap_or(0);
                if let Some((who, lines)) = self.bonus_queue[window].pop_front() {
                    self.fill_bonus(window, &who, &lines);
                    self.movie.goto_and_play(&get_bonus_path(window), "in");
                } else {
                    self.bonuses[window] = None;
                    if let Some(s) = self.movie.sprite_mut(&format!("{}.getBonusSet.charaImage.coImage", get_bonus_path(window))) {
                        crate::controls::icon_load(s, None);
                    }
                    self.layout_stack();
                }
                self.get_bonus_visible = self.bonuses.iter().any(Option::is_some);
            }
        }
    }

    fn stock_clip(i: i32) -> String {
        format!("{ENM_SET}.enmHpS{}", i + 1)
    }

    /// `setEHPMax(a1, a2)`: stock count of the full gauge, the top bar's base frame.
    pub fn set_ehp_max(&mut self, a1: i32, a2: i32) {
        self.log(format!("setEHPMax({a1}, {a2})"));
        self.e_hpg_stock = -1;
        self.e_hpg_stock_max = a1;
        let mut l3 = a1;
        if a1 > ENEMY_HP_STOCK_MAX {
            self.visible_add_s = true;
            l3 = ENEMY_HP_STOCK_MAX;
            self.set_add_stock_num(a1 - ENEMY_HP_STOCK_MAX);
        }
        self.movie.goto_and_stop(&format!("{ENM_SET}.enmHpAddS"), if self.visible_add_s { "loop" } else { "off" });
        for l4 in 0..l3.max(0) {
            self.movie.set_visible(&Self::stock_clip(l4), true);
            self.movie.goto_and_play(&format!("{}.enmHpGgS", Self::stock_clip(l4)), if self.visible_add_s { "on" } else { "off" });
        }
        for l5 in l3.max(0)..ENEMY_HP_STOCK_MAX {
            self.movie.set_visible(&Self::stock_clip(l5), false);
        }
        self.movie.goto_frame(&format!("{ENM_SET}.enmHp.enmHpGg"), 1);
        self.e_hp_follow.reset();
        self.movie.goto_frame(&format!("{ENM_SET}.enmHp.enmHpGgR"), 1);
        self.movie.goto_frame(&format!("{ENM_SET}.enmHp.enmHpDodai"), a2.max(1) as u32);
        self.movie.goto_and_play(&format!("{ENM_SET}.enmHpDummy"), "off");
        self.visible_add_s = false;
        self.movie.goto_and_stop(&format!("{ENM_SET}.enmHpAddS"), "off");
    }

    /// `setEHPBase(a1)`.
    pub fn set_ehp_base(&mut self, a1: i32) {
        self.log(format!("setEHPBase({a1})"));
        self.movie.goto_frame(&format!("{ENM_SET}.enmHp.enmHpDodai"), a1.max(1) as u32);
    }

    /// `setEHP(a1, a2, a3)`: stock squares lit, the top bar's frame, the red gauge's start.
    pub fn set_ehp(&mut self, a1: i32, a2: i32, a3: i32) {
        self.log(format!("setEHP({a1}, {a2}, {a3})"));
        if self.e_hpg_stock != a1 {
            let mut l4 = false;
            if a1 > ENEMY_HP_STOCK_MAX {
                l4 = true;
                self.set_add_stock_num(a1 - ENEMY_HP_STOCK_MAX);
                self.movie.goto_and_play(&format!("{ENM_SET}.enmHpAddS.enmHpAddSCount"), if a2 == a3 { "loop" } else { "damage" });
            } else {
                let l5 = self.e_hpg_stock_max.min(ENEMY_HP_STOCK_MAX);
                for l6 in 0..l5.max(0) {
                    let label = if l6 < a1 {
                        "on"
                    } else if a3 == a2 {
                        "off"
                    } else if l6 < self.e_hpg_stock {
                        "damage"
                    } else {
                        "off"
                    };
                    self.movie.goto_and_play(&format!("{}.enmHpGgS", Self::stock_clip(l6)), label);
                }
            }
            self.e_hpg_stock = a1;
            let dummy = if self.e_hpg_stock_max > 0 && self.e_hpg_stock == self.e_hpg_stock_max { "on" } else { "off" };
            self.movie.goto_and_play(&format!("{ENM_SET}.enmHpDummy"), dummy);
            if self.visible_add_s != l4 {
                self.visible_add_s = l4;
                self.movie.goto_and_stop(&format!("{ENM_SET}.enmHpAddS"), if l4 { "loop" } else { "off" });
            }
        }
        self.movie.goto_frame(&format!("{ENM_SET}.enmHp.enmHpGg"), a2.max(1) as u32);
        if let Some(frame) = self.e_hp_follow.set_gauge(a3, a2) {
            self.movie.goto_frame(&format!("{ENM_SET}.enmHp.enmHpGgR"), frame);
        }
    }

    /// `onEHPBeginFollow()`: when the red gauge starts to move, the squares that showed
    /// `damage` go off.
    fn on_ehp_begin_follow(&mut self) {
        if self.visible_add_s {
            self.movie.goto_and_play(&format!("{ENM_SET}.enmHpAddS.enmHpAddSCount"), "loop");
            return;
        }
        for l2 in self.e_hpg_stock.max(0)..self.e_hpg_stock_max.min(ENEMY_HP_STOCK_MAX) {
            self.movie.goto_and_play(&format!("{}.enmHpGgS", Self::stock_clip(l2)), "off");
        }
    }

    /// `setAddStockNum(a1)`: the "+N" counter above 20 squares.
    pub fn set_add_stock_num(&mut self, a1: i32) {
        let set = format!("{ENM_SET}.enmHpAddS.enmHpAddSCount.enmHpAddSSet");
        self.movie.goto_and_stop(&set, if a1 < 10 { "digit1" } else { "digit10" });
        if let Some(s) = self.movie.sprite_mut(&format!("{set}.enmHpAddSNum")) {
            controls::number_set(s, a1);
        }
    }

    /// The part's show (vtable +0x280: label `in`) / hide (+0x288: `out`).
    pub fn show_enemy_gauge(&mut self, show: bool) {
        if show != self.enemy_shown {
            self.enemy_shown = show;
            self.log(format!("enmHpPack.{}", if show { "in" } else { "out" }));
            self.movie.goto_and_play(ENM, if show { "in" } else { "out" });
        }
    }

    pub fn enemy_gauge_shown(&self) -> bool {
        self.enemy_shown
    }

    /// The stock squares lit and the top bar's frame (tests).
    pub fn enemy_gauge_state(&self) -> (i32, u32) {
        (self.e_hpg_stock, self.movie.current_frame(&format!("{ENM_SET}.enmHp.enmHpGg")).unwrap_or(0))
    }

    /// `0x141589330(cockpit, level, pawn, deltas)`: `HudRightLevelUp.setParam(level, chara,
    /// icon, line1, line2, line3)` and show; a popup already on screen is updated in place
    /// (`0x141659CD0`). `chara`: 0 Sora, 1 Donald, 2 Goofy. The clip runs 272 frames.
    pub fn show_level_up(&mut self, level: i32, chara: usize, lines: &[String]) {
        self.show_level_up_as(level, &PanelChara::plain(chara), lines);
    }

    /// (BN) [`HudRight::show_level_up`] for a character of the host's roster: its window colour and its portrait
    /// (`m_iconChara.load(icon)`, the `UI/Icon/MenuFace` bust drawn into `levelUpSet.charaImage.coImage`).
    pub fn show_level_up_as(&mut self, level: i32, who: &PanelChara, lines: &[String]) {
        let chara = who.id;
        self.log(format!("levelUp.setParam({level}, {chara}, icon, {lines:?})"));
        // (party) 0x141589330: a window showing this character is updated in place, else the first free one is
        // used; with every window busy nothing is shown.
        let window = match self.level_ups.iter().position(|c| *c == Some(chara)).or_else(|| self.level_ups.iter().position(Option::is_none)) {
            Some(w) => w,
            None => return,
        };
        let root = level_up_path(window);
        let set = format!("{root}.levelUpSet");
        self.movie.set_text(&format!("{set}.levelUpStatus.txLvNumber.textField"), &level.to_string(), &self.fonts);
        let line = |i: usize| lines.get(i).map_or("", String::as_str);
        self.movie.set_text(&format!("{set}.txLevelUpCapacity1.textField"), line(0), &self.fonts);
        let mut l7 = 0;
        let mut win = "lvupBonusWin1.lvupBonusWin1Col";
        if !line(1).is_empty() {
            l7 += 1;
            self.movie.set_text(&format!("{set}.txLevelUpCapacity2.textField"), line(1), &self.fonts);
            if !line(2).is_empty() {
                l7 += 1;
                win = "lvupBonusWin3.lvupBonusWin3Col";
                self.movie.set_text(&format!("{set}.txLevelUpCapacity3.textField"), line(2), &self.fonts);
            }
        }
        self.movie.goto_and_stop(&set, ["set1", "set2", "set3"][l7]);
        // (party I2) setParam 0x14165D4B0: +0x104 = rect3's height with a third line, else rect1's
        let three = l7 == 2;
        let relayout = self.level_up_three[window] != three && self.level_ups[window].is_some();
        self.level_up_three[window] = three;
        self.movie.goto_and_stop(&format!("{set}.{win}"), PANEL_COLOURS[who.colour.min(PANEL_COLOURS.len() - 1)]); // (BN) the roster's colour
        // (BN) `m_iconChara.unload(); m_iconChara.load(icon)`: the character's portrait
        if let Some(s) = self.movie.sprite_mut(&format!("{set}.charaImage.coImage")) {
            crate::controls::icon_load(s, who.portrait.clone());
        }
        if self.level_ups[window].is_none() {
            self.level_ups[window] = Some(chara);
            self.level_up_visible = true;
            self.movie.goto_and_play(&root, "in");
            self.layout_level_ups();
        } else if relayout {
            self.layout_level_ups();
        }
    }

    /// (party) The visible windows stacked downward in window order (see `LEVEL_UP_STEP`).
    /// (BN) One stack with the bonus windows below them, as the game's (see [`HudRight::layout_stack`]).
    fn layout_level_ups(&mut self) {
        self.layout_stack();
    }

    /// (party) How many level-up windows are on screen.
    pub fn level_ups_shown(&self) -> usize {
        self.level_ups.iter().filter(|c| c.is_some()).count()
    }

    pub fn level_up_visible(&self) -> bool {
        self.level_up_visible
    }

    /// (new game) `0x1415874F0(cockpit, pawn, entries)`: `HudRightGetBonus.setParam(chara, icon, line1,
    /// line2)` and show (the clip has the level-up popup's timeline: 272 frames from `in`). `chara`: 0 Sora.
    /// The window art has one or two lines (`set1` / `set2`).
    pub fn show_get_bonus(&mut self, chara: usize, lines: &[String]) {
        self.show_bonus_panel(&PanelChara::plain(chara), lines);
    }

    /// (BN) 0x1415874F0 (cockpit, pawn, entries): THE BONUS! PANEL of a character of the host's roster. The first
    /// window that is free is filled and slides in (the stack places it under the level-up windows and the other bonus
    /// windows); a window already showing this character takes the panel into its queue (0x141659B40) and shows it
    /// when its current panel has gone. Every window busy with other characters: nothing is shown (as the game).
    /// `lines`: one or two, each with its icon already in front ([`icon_line`]). Returns false when nothing is shown
    /// or queued.
    pub fn show_bonus_panel(&mut self, who: &PanelChara, lines: &[String]) -> bool {
        self.log(format!("getBonus.setParam({}, icon, {lines:?})", who.colour));
        for w in 0..GET_BONUS_WINDOWS {
            match self.bonuses[w] {
                None => {
                    self.fill_bonus(w, who, lines);
                    self.bonuses[w] = Some(who.id);
                    self.get_bonus_visible = true;
                    self.movie.goto_and_play(&get_bonus_path(w), "in");
                    self.layout_stack();
                    return true;
                }
                Some(id) if id == who.id => {
                    self.bonus_queue[w].push_back((who.clone(), lines.to_vec()));
                    return true;
                }
                Some(_) => {}
            }
        }
        false
    }

    /// (BN) `HudRightGetBonus.setParam(chara, icon, line1, line2)` on window `w`.
    fn fill_bonus(&mut self, w: usize, who: &PanelChara, lines: &[String]) {
        let set = format!("{}.getBonusSet", get_bonus_path(w));
        let line = |i: usize| lines.get(i).map_or("", String::as_str);
        self.movie.set_text(&format!("{set}.txGetBonusCapacity1.textField"), line(0), &self.fonts);
        let two = !line(1).is_empty();
        if two {
            self.movie.set_text(&format!("{set}.txGetBonusCapacity2.textField"), line(1), &self.fonts);
        }
        if let Some(s) = self.movie.sprite_mut(&format!("{set}.charaImage.coImage")) {
            crate::controls::icon_load(s, who.portrait.clone());
        }
        self.movie.goto_and_stop(&set, if two { "set2" } else { "set1" });
        self.movie.goto_and_stop(&format!("{set}.lvupBonusWin1.lvupBonusWin1Col"), PANEL_COLOURS[who.colour.min(PANEL_COLOURS.len() - 1)]);
    }

    pub fn get_bonus_visible(&self) -> bool {
        self.get_bonus_visible
    }
}

/// What the enemy gauge reads from its target each frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyValues {
    pub hp: i32,
    pub hp_max: i32,
    /// vt+0x17D0: the large gauge (2000 HP per bar, cap 240,000).
    pub large: bool,
    pub alive: bool,
}

/// The native enemy gauge (`UTresUIP_HudRightEnmHpPack`).
#[derive(Clone, Debug, Default)]
pub struct EnemyGauge {
    /// The target's key (the host's id).
    target: Option<u64>,
    locked: bool,
    /// +0x148: seconds until a cleared target's gauge hides.
    hide_timer: f32,
    /// +0x12C bit 0: the target is new (max to be sent).
    fresh: bool,
    max_hp: i32,
    hp: i32,
    shown_stock: i32,
    base_rest: i32,
    unit: i32,
    /// The HP before the hit that pointed the gauge at this target.
    prev_hp: Option<i32>,
}

/// Seconds the gauge stays after the last cursor went (`0x14165A050`).
pub const ENEMY_GAUGE_HIDE_DELAY: f32 = 0.5;

impl EnemyGauge {
    pub fn target(&self) -> Option<u64> {
        self.target
    }

    /// `0x14158A860(cockpit, pawn, locked, prevHP)` -> `0x14165A050`: point the gauge at a
    /// target. The caller has checked LIBRA. A new target replaces the shown one unless the
    /// shown one was set with `locked` and the new one was not. `prev_hp` = the HP before the
    /// hit (so that the red lag starts there).
    pub fn point_at(&mut self, key: u64, locked: bool, prev_hp: Option<i32>) {
        if self.target == Some(key) {
            self.hide_timer = 0.0;
            self.locked |= locked;
            return;
        }
        if self.target.is_some() && self.locked && !locked && self.hide_timer <= 0.0 {
            return;
        }
        self.target = Some(key);
        self.locked = locked;
        self.hide_timer = 0.0;
        self.fresh = true;
        self.prev_hp = prev_hp;
    }

    /// The last auto / lock-on cursor was removed: the gauge stays 0.5 s (`force`: hides at once).
    pub fn clear(&mut self, force: bool) {
        if self.target.is_none() {
            return;
        }
        if force {
            self.target = None;
            self.hide_timer = 0.0;
        } else if self.hide_timer <= 0.0 {
            self.hide_timer = ENEMY_GAUGE_HIDE_DELAY;
        }
    }

    /// `0x14165F820`: one update. `values_of` gives the target's values (None = it is gone).
    pub fn update(&mut self, hud: &mut HudRight, dt: f32, values_of: &dyn Fn(u64) -> Option<EnemyValues>, _sounds: &mut Vec<UiSound>) {
        let values = self.target.and_then(values_of).filter(|v| v.alive);
        let mut expired = false;
        if self.hide_timer > 0.0 {
            self.hide_timer -= dt;
            expired = self.hide_timer <= 0.0;
        }
        let Some(v) = values.filter(|_| !expired) else {
            // No target, it died, or the hide timer ran out.
            self.target = None;
            self.hide_timer = 0.0;
            self.locked = false;
            hud.show_enemy_gauge(false);
            return;
        };
        let was_fresh = self.fresh;
        if self.fresh {
            hud.show_enemy_gauge(true);
            self.unit = if v.large { 2000 } else { 200 };
        }
        let cap = self.unit * 120;
        let hp = v.hp.clamp(0, cap);
        let max_hp = v.hp_max.clamp(0, cap);
        if self.fresh || max_hp != self.max_hp {
            self.fresh = false;
            self.max_hp = max_hp;
            let (mut stock_max, mut rem) = (max_hp / self.unit, max_hp % self.unit);
            if rem == 0 && stock_max > 0 {
                rem = self.unit;
                stock_max -= 1;
            }
            self.shown_stock = stock_max;
            self.base_rest = rem;
            hud.set_ehp_max(stock_max, gauge_frame(rem, self.unit, 500) + 1);
            self.hp = self.prev_hp.take().map_or(hp, |p| p.clamp(0, cap));
            self.send_hp(hud, hp);
        } else if hp != self.hp || was_fresh {
            self.send_hp(hud, hp);
        }
    }

    fn send_hp(&mut self, hud: &mut HudRight, hp: i32) {
        let split = |value: i32, unit: i32| {
            let (mut stock, mut rest) = (value / unit, value % unit);
            if rest == 0 && stock > 0 {
                stock -= 1;
                rest = unit;
            }
            (stock, rest)
        };
        let u = self.unit;
        let before = self.hp.max(hp);
        let (stock, rest) = split(hp, u);
        let (prev_stock, prev_rest) = split(before, u);
        // The red gauge starts at the previous rest, or at a full bar when a whole bar went.
        let red = if prev_stock <= stock { prev_rest } else { u };
        if stock < self.shown_stock {
            if self.base_rest < u {
                self.base_rest = u;
                hud.set_ehp_base(gauge_frame(u, u, 500) + 1);
            }
            self.shown_stock = stock;
        }
        hud.set_ehp(stock, gauge_frame(rest, u, 500) + 1, gauge_frame(red, u, 500) + 1);
        self.hp = hp;
    }
}

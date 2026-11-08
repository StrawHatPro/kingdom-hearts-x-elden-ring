//! kh-ui: a small Scaleform GFx timeline player for KH3's HUD, engine-free.
//!
//! docs/sora-ui-spec.md is the specification; `build/sora_pack/ui` (written by
//! `tools/sora_ui_pack.js`) is the data. Layers:
//!
//! - [`def`]: the pack as plain data (movies, textures, fonts, strings);
//! - [`player`]: display-list evaluation (frames, labels, frame scripts, nested timelines);
//! - [`raster`] / [`render`]: the software compositor (the rules of `tools/lib/gfx_render.js`);
//! - [`controls`]: the shared AS3 helper classes (`FollowGauge`, `TextFieldAdjust`, `BitmapNumber`);
//! - [`frames`]: the native drivers' frame formulas;
//! - [`hud_status`], [`hud_command`] + [`command`], [`hud_target`], [`hud_side`]: the HUD movies' AS3
//!   classes ported method by method, each with the native driver that calls them;
//! - [`command_debug`]: a DEBUG stand-in for the command system; [`camp_menu`]: a stub.
//!
//! The crate knows nothing of Bevy or of `kh-core`: a host hands it plain values and draws
//! the RGBA it returns.

// (CV1) the KH speech bubble of the conversation system (KH2 shape, tail on the speaker, Keyblade cue, choices)
pub mod bubble;
pub mod camp;
pub mod camp_custom;
pub mod camp_equip;
pub mod camp_menu;
pub mod camp_options; // (CAMP-OPTIONS) the tabbed Config screen: Elden Ring's options in its own tabs, KH3's look
pub mod camp_parry; // (GOOFY, THE USER 2026-11-02) Goofy's Parry Techniques page: Abilities > Goofy > Y "Show Next List"
pub mod command;
pub mod command_debug;
pub mod controls;
// (cutscene) the cutscene pause menu (movie Pause, menu type 1: Continue / Skip Scene)
pub mod cutscene_pause;
pub mod def;
pub mod frames;
pub mod gameover;
// (savepoint) the save-point menu (movie SaveLoad, clip menuSavePoint)
pub mod savepoint;
// (saveload) the Save and Load screens (movie SaveLoad: the slot lists; docs/sora-saveload-spec.md)
pub mod saveload;
pub mod geom;
pub mod glyphs; // (BUTTON-ICONS) which glyph a button icon draws: the remap tables, the chosen set (Xbox / PlayStation / keyboard + mouse)
pub mod hud_command;
// (F-HUD) the formchange HUD: the ShooterMode scope, the multi-lock markers (forms_hud), the form style / logo of the
// command menu (forms_hud_style), the operation guide HudOperation (forms_hud_operation). INT: keep these three lines.
pub mod forms_hud;
pub mod forms_hud_operation;
pub mod forms_hud_style;
pub mod hud_goofy; // (GOOFY) the Stagger gauge under the enemy's HP gauge, the red "!" over a perilous attacker
pub mod hud_guard; // (GOOFY-GG) Goofy's silver GUARD bar above MP
pub mod hud_resonance; // (DON-UI) Donald's Resonance ring, pips, ultimate mark (status HUD)
pub mod hud_buffs; // (ALLY-BUFF, THE USER 2026-11-03) Donald's helpful spells on whoever has them, whoever leads (status HUD)
pub mod hud_arcane; // (DON-UI) Donald's Arcane Flow stars, Zettaflare tracker (command HUD), target status icons
pub mod hud_side;
pub mod hud_spirit; // (SUMMON, THE USER 2026-11-02) the summoned spirit's KH gauge: name, HP, time left (KH3's guest slot)
pub mod hud_status;
pub mod hud_target;
// (IMPL_ER37) the item pictures and help-text ids by item key (data/item_icons.json)
pub mod item_art;
// (HUD2) KH3's major item popup "OBTAINED" (movie KeyItem): key items, Keyblades, new party members
pub mod key_item;
pub mod list;
pub mod menu;
// (new game) the new-game choice menu (movies CutsceneText + Dialog): the two questions of the opening
pub mod newgame;
// (party PF) the save point's Party Formation screen, icon-based (inside the camp menu's movie)
pub mod party_formation;
pub mod player;
// (ALLYJOIN) a host's Yes / No question in KH3's own Dialog window, alone in the world (a new ally: join the party?)
pub mod question;
// (ER party S8) a KH-style talk window over a speaker (movie Dialog in its text form), a prompt, a banner
pub mod talk;
pub mod raster;
pub mod render;
// (SHOP) the KH shop screen that stands in for a host's merchants (the camp menu's Items screen + the Dialog movie)
pub mod shop;
// (SHOP) KH3's own Moogle shop screen (movie 41_shop/Shop) on the same logic
pub mod shop_moogle;
// (THE USER 2026-11-02) the Spirit Tuning screen (Roderika's spirit tuning in the Elden Ring mod: gloveworts power up ashes)
pub mod tuning;
// (weapons P6b) KH3's Workshop screens (movie 41_shop/Shop): synthesis, Keyblade Forge, milestones, goals, materials
pub mod workshop;

pub use def::{MovieDef, Pack, PackError};
pub use geom::{Cx, Matrix, Rect};
pub use player::Movie;
pub use raster::Canvas;
pub use render::{Fonts, Renderer};

/// A UI sound request: a static SE index (`TresStaticSESet.m_Assets[index]` = `se00000_NNN`,
/// docs/sora-ui-spec.md section 9; `data/ui_sounds.json` of the pack names the assets).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiSound {
    /// One shot.
    Play(u32),
    /// A loop that runs until `LoopStop` of the same index (the low-HP alarm).
    LoopStart(u32),
    LoopStop(u32),
}

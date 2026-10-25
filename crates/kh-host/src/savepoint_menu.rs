//! (er-host, IMPL_ER26) THE SAVE-POINT MENU OF A HOST WHOSE SAVE POINTS STAND AT ANOTHER GAME'S RESTING PLACES (the Elden
//! Ring mod: sites of grace). Pure rules, shared by the mod (`kh-er` savepoint.rs) and the offline picture harness
//! (`kh-er-render` example `hud`), so both show the same menu:
//!
//! | entry | KH3 command that stands for it (its clip slot, sounds, decide) | text | icon cell |
//! |---|---|---|---|
//! | Save | Save | "Save" (KH3's string would do; the help line is the host's) | save1 (the pencil) |
//! | Load (SV, the user 2026-10-17) | Load (a port command) | KH3's title-menu LOAD entry and help line | shopGm2 |
//! | Select an Area | WorldMap | KH3's name of another entry, host help line | save3 (the globe) |
//! | Pass Time | Bistro | COMPOSED (no KH3 string) | [`ICON_PASS_TIME`] |
//! | Respawn Enemies | Tutorials | COMPOSED (no KH3 string) | [`ICON_RESPAWN`] |
//! | Site of Grace (only with `sit`) | SelectArea | COMPOSED | save2 (the ring of light) |
//! | Talk to Melina (only while the host says the other game offers a talk; IMPL_ER33) | SelectArea | COMPOSED | save2 |
//! | Party Formation (party P, the user's request 2026-10-15) | PartyFormation (a port command) | COMPOSED | camp4 |
//! | Title Screen | TitleScreen | KH3's own | save6 (the return arrow) |
//!
//! The clip `menuSavePoint` has six command slots (`cmd1` .. `cmd6`, 96 px apart): every combination fits.
//! Pass Time asks a choice first (Morning / Noon / Nightfall: the three the other game's own menu offers).

use kh_ui::savepoint::{HostChoice, SavePointCommand};

/// The icon cells (labels of the category icon clip, MainLibrary sprite 66) of the two entries KH3 has no icon for.
/// Pass Time: `camp8`, the cogwheel (the camp menu's Config cell): the nearest thing to clockwork among the 26 cells.
/// Respawn Enemies: `camp3`, the sparkles in a ring (the camp menu's Abilities cell): things appearing again.
/// LOCAL: no cell of the clip shows a clock or an enemy (all 26 were looked at: er26/harness/icons/).
pub const ICON_PASS_TIME: &str = "camp8";
pub const ICON_RESPAWN: &str = "camp3";

/// Which entries the menu has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraceMenu {
    /// "Pass Time" (`savepoint.passtime`).
    pub passtime: bool,
    /// "Respawn Enemies" (`savepoint.respawn`).
    pub respawn: bool,
    /// "Site of Grace": the other game's own rest and menu (`savepoint.sit = 1`).
    pub sit: bool,
    /// "Title Screen" (`menu.title`).
    pub title: bool,
    /// (IMPL_ER33) "Talk to Melina": the other game offers a talk that only its own resting-place menu holds (the
    /// host's story table says when). It stands on the same KH3 command as "Site of Grace", so `sit` leaves it out
    /// (that entry leads to the same menu).
    pub talk: bool,
    /// (party P) "Party Formation": who is in the active party and who leads (the HUD's screen).
    pub party: bool,
    /// (SV) "Load": the Load screen (`savepoint.load`).
    pub load: bool,
}

impl GraceMenu {
    /// The shipped menu: Save, Load (SV), Select an Area, Pass Time, Respawn Enemies, Party Formation, Title Screen.
    pub const DEFAULT: GraceMenu = GraceMenu { passtime: true, respawn: true, sit: false, title: true, talk: false, party: true, load: true };
}

/// What an entry of the menu asks of the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraceAction {
    Save,
    /// (SV) The Load screen (a slot chosen there, confirmed, is loaded by the host).
    Load,
    Map,
    PassTime,
    Respawn,
    Rest,
    /// (IMPL_ER33) The other game's own rest and menu, asked for a talk it offers there (the host makes it unseen).
    Talk,
    /// (party P) The Party Formation screen.
    Party,
    Title,
}

/// One entry: the action, the KH3 command that stands for it, the host's texts (None = KH3's own), the icon cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraceEntry {
    pub action: GraceAction,
    pub command: SavePointCommand,
    pub name: Option<&'static str>,
    pub help: Option<&'static str>,
    pub icon: &'static str,
}

const SAVE: GraceEntry = GraceEntry { action: GraceAction::Save, command: SavePointCommand::Save, name: Some("Save"), help: Some("kh3_menu.savepoint.detail_0001"), icon: "save1" };
const LOAD: GraceEntry = GraceEntry { action: GraceAction::Load, command: SavePointCommand::Load, name: Some(kh_ui::savepoint::LOAD_TEXT.0), help: Some(kh_ui::savepoint::LOAD_TEXT.1), icon: kh_ui::savepoint::LOAD_ICON }; // (SV)
const MAP: GraceEntry = GraceEntry { action: GraceAction::Map, command: SavePointCommand::WorldMap, name: Some("Select an Area"), help: Some("Open the map and travel to another save point."), icon: "save3" };
const PASS_TIME: GraceEntry = GraceEntry { action: GraceAction::PassTime, command: SavePointCommand::Bistro, name: Some("Pass Time"), help: Some("Let time pass until morning, noon or nightfall."), icon: ICON_PASS_TIME };
const RESPAWN: GraceEntry = GraceEntry { action: GraceAction::Respawn, command: SavePointCommand::Tutorials, name: Some("Respawn Enemies"), help: Some("Bring back the enemies of the world.\nNothing is saved."), icon: ICON_RESPAWN };
const REST: GraceEntry = GraceEntry { action: GraceAction::Rest, command: SavePointCommand::SelectArea, name: Some("Site of Grace"), help: Some("Rest at the site of grace: its own menu."), icon: "save2" };
const TALK: GraceEntry = GraceEntry { action: GraceAction::Talk, command: SavePointCommand::SelectArea, name: Some("Talk to Melina"), help: Some("Melina has something to say.
The site of grace's own menu opens."), icon: "save2" };
const PARTY: GraceEntry = GraceEntry { action: GraceAction::Party, command: SavePointCommand::PartyFormation, name: Some(kh_ui::savepoint::PARTY_FORMATION_TEXT.0), help: Some(kh_ui::savepoint::PARTY_FORMATION_TEXT.1), icon: "camp4" };
const TITLE: GraceEntry = GraceEntry { action: GraceAction::Title, command: SavePointCommand::TitleScreen, name: None, help: None, icon: "save6" };

/// THE MENU'S ENTRIES, from the top.
pub fn entries(menu: GraceMenu) -> Vec<GraceEntry> {
    let mut out = vec![SAVE];
    if menu.load {
        out.push(LOAD); // (SV)
    }
    out.push(MAP);
    if menu.passtime {
        out.push(PASS_TIME);
    }
    if menu.respawn {
        out.push(RESPAWN);
    }
    if menu.sit {
        out.push(REST);
    } else if menu.talk {
        out.push(TALK);
    }
    if menu.party {
        out.push(PARTY);
    }
    if menu.title {
        out.push(TITLE);
    }
    out
}

/// The action a chosen command stands for (in a menu without "Talk to Melina").
pub fn action_of(command: SavePointCommand) -> Option<GraceAction> {
    [SAVE, LOAD, MAP, PASS_TIME, RESPAWN, REST, PARTY, TITLE].into_iter().find(|e| e.command == command).map(|e| e.action)
}

/// (IMPL_ER33) The action a chosen command stands for IN THIS MENU (two entries share a KH3 command: only one of them
/// is ever in a menu).
pub fn action_in(menu: GraceMenu, command: SavePointCommand) -> Option<GraceAction> {
    entries(menu).into_iter().find(|e| e.command == command).map(|e| e.action)
}

/// The three times of day of "Pass Time", in the order of the choice window's buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeChoice {
    Morning,
    Noon,
    Nightfall,
}

impl TimeChoice {
    pub const ALL: [TimeChoice; 3] = [TimeChoice::Morning, TimeChoice::Noon, TimeChoice::Nightfall];

    /// The button of the choice window (0 = the top one).
    pub fn from_button(button: usize) -> Option<TimeChoice> {
        Self::ALL.get(button).copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            TimeChoice::Morning => "Morning",
            TimeChoice::Noon => "Noon",
            TimeChoice::Nightfall => "Nightfall",
        }
    }

    /// The control key that overrides its clock time (`H` or `H:M`).
    pub fn key(self) -> &'static str {
        match self {
            TimeChoice::Morning => "savepoint.passtime.morning",
            TimeChoice::Noon => "savepoint.passtime.noon",
            TimeChoice::Nightfall => "savepoint.passtime.nightfall",
        }
    }

    /// The clock time (hour, minute) it stands for. NOT MEASURED in the other game (its own menu's three times were
    /// not read): 7:00, 12:00 and 20:00, each well inside the part of the day it names.
    pub fn clock(self) -> (u32, u32) {
        match self {
            TimeChoice::Morning => (7, 0),
            TimeChoice::Noon => (12, 0),
            TimeChoice::Nightfall => (20, 0),
        }
    }
}

/// `H`, `H:M` or `H.fraction` -> (hour 0..=23, minute 0..=59).
pub fn parse_clock(text: &str) -> Option<(u32, u32)> {
    let mut parts = text.split(':').map(|p| p.trim().parse::<f32>().ok());
    let hours = parts.next()??;
    if !hours.is_finite() || !(0.0..24.0).contains(&hours) {
        return None;
    }
    let minutes = match parts.next() {
        Some(m) => m?,
        None => hours.fract() * 60.0,
    };
    if !(0.0..60.0).contains(&minutes) {
        return None;
    }
    Some((hours.floor() as u32, minutes.floor() as u32))
}

/// TIME TO PASS from the clock `now` (h, m, s) forward to `target` (h, m): (hours, minutes, seconds), always forward
/// and never nothing: a target less than a minute ahead is a whole day away.
pub fn time_to_pass(now: (u32, u32, u32), target: (u32, u32)) -> (i32, i32, i32) {
    const DAY: i64 = 86_400;
    let now_s = (now.0 as i64 % 24) * 3600 + (now.1 as i64 % 60) * 60 + now.2 as i64 % 60;
    let target_s = (target.0 as i64 % 24) * 3600 + (target.1 as i64 % 60) * 60;
    let mut delta = (target_s - now_s).rem_euclid(DAY);
    if delta < 60 {
        delta += DAY;
    }
    ((delta / 3600) as i32, (delta % 3600 / 60) as i32, (delta % 60) as i32)
}

/// The choice window of Pass Time.
pub fn pass_time_choice() -> HostChoice {
    HostChoice { command: PASS_TIME.command, text: "Pass time until:".to_string(), options: TimeChoice::ALL.iter().map(|t| t.label().to_string()).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(menu: GraceMenu) -> Vec<&'static str> {
        entries(menu).iter().map(|e| e.name.unwrap_or("Title Screen")).collect()
    }

    #[test]
    fn the_menu_has_its_entries_in_the_decided_order() {
        assert_eq!(names(GraceMenu::DEFAULT), ["Save", "Load", "Select an Area", "Pass Time", "Respawn Enemies", "Party Formation", "Title Screen"]);
        // (SV) `savepoint.load = 0`: the menu before Load
        assert_eq!(names(GraceMenu { load: false, ..GraceMenu::DEFAULT }), ["Save", "Select an Area", "Pass Time", "Respawn Enemies", "Party Formation", "Title Screen"]);
        assert_eq!(action_in(GraceMenu::DEFAULT, SavePointCommand::Load), Some(GraceAction::Load));
        assert_eq!(action_in(GraceMenu { load: false, ..GraceMenu::DEFAULT }, SavePointCommand::Load), None);
        // savepoint.sit = 1 brings the game's own rest back, before Title Screen; (party P) with Party Formation the clip
        // gets its seventh slot (kh-ui savepoint MAX_COMMANDS)
        assert_eq!(names(GraceMenu { sit: true, ..GraceMenu::DEFAULT }), ["Save", "Load", "Select an Area", "Pass Time", "Respawn Enemies", "Site of Grace", "Party Formation", "Title Screen"]);
        assert_eq!(names(GraceMenu { passtime: false, respawn: false, sit: false, title: false, talk: false, party: false, load: false }), ["Save", "Select an Area"]);
        assert_eq!(names(GraceMenu { respawn: false, ..GraceMenu::DEFAULT }), ["Save", "Load", "Select an Area", "Pass Time", "Party Formation", "Title Screen"]);
        assert_eq!(names(GraceMenu { party: false, ..GraceMenu::DEFAULT }), ["Save", "Load", "Select an Area", "Pass Time", "Respawn Enemies", "Title Screen"]);
        // every entry stands on a command of its own (the clip slot, the result) and never more than the clip's slots
        for sit in [false, true] {
            let list = entries(GraceMenu { sit, ..GraceMenu::DEFAULT });
            assert!(list.len() <= kh_ui::savepoint::MAX_COMMANDS);
            for (k, e) in list.iter().enumerate() {
                assert!(list.iter().skip(k + 1).all(|o| o.command != e.command), "{e:?}");
                assert_eq!(action_of(e.command), Some(e.action));
            }
        }
        // (IMPL_ER33) "Talk to Melina" while the host says a talk is offered: before Title Screen, six entries, its own action
        let talk = GraceMenu { talk: true, ..GraceMenu::DEFAULT };
        assert_eq!(names(talk), ["Save", "Load", "Select an Area", "Pass Time", "Respawn Enemies", "Talk to Melina", "Party Formation", "Title Screen"]);
        assert_eq!(action_in(talk, SavePointCommand::PartyFormation), Some(GraceAction::Party));
        assert_eq!(action_in(talk, SavePointCommand::SelectArea), Some(GraceAction::Talk));
        assert_eq!(action_in(GraceMenu { sit: true, talk: true, ..GraceMenu::DEFAULT }, SavePointCommand::SelectArea), Some(GraceAction::Rest));
        assert_eq!(names(GraceMenu { sit: true, talk: true, ..GraceMenu::DEFAULT }).len(), 8);
        assert_eq!(action_in(GraceMenu::DEFAULT, SavePointCommand::SelectArea), None);
        assert!(entries(talk).iter().all(|e| action_in(talk, e.command) == Some(e.action)));
        // no entry of the default menu rests
        assert!(entries(GraceMenu::DEFAULT).iter().all(|e| e.action != GraceAction::Rest));
    }

    #[test]
    fn pass_time_maps_its_three_buttons_and_always_goes_forward() {
        assert_eq!(pass_time_choice().options, ["Morning", "Noon", "Nightfall"]);
        assert_eq!([0, 1, 2, 3].map(TimeChoice::from_button), [Some(TimeChoice::Morning), Some(TimeChoice::Noon), Some(TimeChoice::Nightfall), None]);
        assert_eq!(TimeChoice::ALL.map(TimeChoice::clock), [(7, 0), (12, 0), (20, 0)]);
        // 10:30:15 -> noon: 1 h 29 min 45 s; -> morning: tomorrow's; -> nightfall
        assert_eq!(time_to_pass((10, 30, 15), (12, 0)), (1, 29, 45));
        assert_eq!(time_to_pass((10, 30, 15), (7, 0)), (20, 29, 45));
        assert_eq!(time_to_pass((10, 30, 15), (20, 0)), (9, 29, 45));
        // already that time: a whole day, never nothing
        assert_eq!(time_to_pass((12, 0, 0), (12, 0)), (24, 0, 0));
        assert_eq!(time_to_pass((11, 59, 30), (12, 0)), (24, 0, 30));
        assert_eq!(time_to_pass((23, 59, 59), (0, 1)), (0, 1, 1));
        // the keys' values
        assert_eq!(parse_clock("6"), Some((6, 0)));
        assert_eq!(parse_clock("19:30"), Some((19, 30)));
        assert_eq!(parse_clock("5.5"), Some((5, 30)));
        assert_eq!(parse_clock("24"), None);
        assert_eq!(parse_clock("x"), None);
        assert_eq!(parse_clock("7:75"), None);
    }
}

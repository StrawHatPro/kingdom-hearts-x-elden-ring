//! (weapons P6b, documentation/KH3_WEAPONS_PLAN.md 1.5 / 1.6 / 6a) KH3'S WORKSHOP IN THE HUD: the screens (kh_ui::workshop:
//! KH3's `41_shop/Shop` movie) on the rules (crate::workshop, P6a). A host opens it with [`open`]:
//! - the Synthesis Moogle at Roundtable Hold (P8): `open(Entry::Workshop, "Roundtable Hold", &flags)`: KH3's Workshop list
//!   (Synthesize Items, Journey to the Erdtree = KH3's Photo Missions as the user's 20 milestones, Collector's Goals,
//!   Material List, the SYNTH LOG); no Buy / Sell / M.O.G. Card (the user, 2026-11-03: "The moogle in the round table is
//!   only for synthesis"); no Keyblade Forge (the user, 2026-11-07: "that is what the blacksmiths are for");
//! - every blacksmith (P7): `open(Entry::Forge, "<smith>", &flags)`: straight on the Keyblade Forge; B closes it.
//!
//! Then [`take_result`] says once that it closed ([`Outcome::Closed`]: the Moogle's / the smith's talk goes on). While it is
//! up [`is_open`] is true and the HUD holds the world (as the KH shop). `flags` is the host's event-flag reader (Elden Ring's
//! defeat flags); the HUD reads every flag the rules name once, at the opening (the world is held while it is up).
//!
//! THE OPENING IS KH3'S WORKSHOP VISIT (`crate::workshop::visit`, KH3 0x14172EFF0): the carried materials go into the
//! Workshop's stock, the Collector's Goals and the milestones are tested, the Moogle level follows the flags; its dialogs are
//! shown one by one in KH3's system window (dialog_0010, per goal dialog_0020 once / dialog_0030 with static SE 10 /
//! the COMPOSED merchants' line for KH3's dialog_0040, then dialog_0050). A SMITH'S FORGE shows only the Forge ([`enter`]:
//! the materials go into the stock, which the Forge counts, silently; the goals, the milestones and every dialog wait for
//! the next Moogle visit, as KH3 shows them on entering the Workshop). THE CHOICES are carried out through the rules on the
//! running game (`crate::workshop::with_live`): a synthesis consumes exactly the recipe from the stock and hands the item
//! over through the item door, then KH3's large item-get popup (movie KeyItem, static SE 10) shows it (a weapon's OBTAINED
//! popup, P9b's, is held meanwhile: `crate::bossrewards::hold_weapon_popups`, plan Q4); a Forge level takes its materials
//! (no munny, WD 6d) and raises the Keyblade's level in P1's record.

use std::collections::BTreeSet;
#[cfg(not(test))]
use std::sync::Mutex;

use kh_ui::camp::CampKey;
use kh_ui::camp_equip::{CAT_ACCESSORY, CAT_ARMOUR, CAT_ITEM, CAT_MATERIAL, CAT_WEAPON};
pub use kh_ui::workshop::Entry;
use kh_ui::workshop::view::WorkshopView;
use kh_ui::workshop::{texts, ForgeNext, ForgeRow, GoalRow, LogRow, MatNeed, MaterialRow, Message, PhotoRow, RecipeRow, RewardKind, Screen, WorkshopData, WorkshopEvent, WorkshopMenu, SE_ITEM_GET};
use kh_ui::UiSound;

use crate::weapons::Workshop;
use crate::workshop::{self as rules, GoalCondition, GoalReward, Milestone, ShopStep, Visit, WorkshopHost};

/// How the menu ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It closed (B on the Workshop's list, or on a smith's Forge; or the host closed it): the talk goes on.
    Closed(Entry),
}

struct Request {
    entry: Entry,
    title: String,
    flags: BTreeSet<u32>,
}

/// What the host and the HUD share: the request, "up", a close asked for, the result.
struct Shared {
    request: Option<Request>,
    up: bool,
    close: bool,
    result: Option<Outcome>,
}

impl Shared {
    const NEW: Shared = Shared { request: None, up: false, close: false, result: None };
}

/// The game: one Workshop for the process (the host's talk and the HUD may run on different threads).
#[cfg(not(test))]
static SHARED: Mutex<Shared> = Mutex::new(Shared::NEW);

// The crate's unit tests: one per test thread, so a test's request never meets another test's HUD (every `Hud` frame
// asks this module; the tests run in parallel).
#[cfg(test)]
thread_local! {
    static SHARED: std::cell::RefCell<Shared> = const { std::cell::RefCell::new(Shared::NEW) };
}

#[cfg(not(test))]
fn shared<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
    f(&mut SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

#[cfg(test)]
fn shared<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
    SHARED.with(|s| f(&mut s.borrow_mut()))
}

/// Every event flag the rules read (the milestones' defeat flags, the Moogle level's steps, the Great Rune holders).
pub fn flags_read() -> Vec<u32> {
    let t = rules::tables();
    let mut out = BTreeSet::new();
    for p in &t.photos {
        match &p.done {
            Milestone::Flag(f) => {
                out.insert(*f);
            }
            Milestone::All(list) => out.extend(list.iter().copied()),
            Milestone::Never => {}
        }
    }
    for (_, step) in &t.shop_levels {
        if let ShopStep::Flag(f) = step {
            out.insert(*f);
        }
    }
    out.extend(t.great_rune_flags.iter().copied());
    out.into_iter().collect()
}

/// OPENS THE WORKSHOP (the Moogle: [`Entry::Workshop`]; a smith: [`Entry::Forge`]) at the HUD's next frame. `title` is
/// the frame's area line ("" = KH3's "WORKSHOP"); `flags` the host's event-flag reader, read now for every flag the rules
/// name. False: one is already asked for or up.
pub fn open(entry: Entry, title: &str, flags: &dyn Fn(u32) -> bool) -> bool {
    if is_open() {
        return false;
    }
    let set: BTreeSet<u32> = flags_read().into_iter().filter(|f| flags(*f)).collect();
    shared(|s| {
        s.request = Some(Request { entry, title: title.to_string(), flags: set });
        s.result = None;
    });
    true
}

/// Asked for or up (the host holds its talk; the HUD holds the world).
pub fn is_open() -> bool {
    shared(|s| s.request.is_some() || s.up)
}

/// The host closes it from outside (a load, a cutscene, the talk was cut off).
pub fn close() {
    shared(|s| {
        s.request = None;
        s.close = true;
    });
}

/// The HUD has taken the request and the menu is up (opening, open or closing). A host that asked with [`open`] and
/// sees this stay false (no HUD, another screen up) can give the request up with [`close`].
pub fn is_up() -> bool {
    shared(|s| s.up)
}

/// Once, after it closed.
pub fn take_result() -> Option<Outcome> {
    shared(|s| s.result.take())
}

fn set_up(up: bool) {
    shared(|s| s.up = up);
}

fn set_result(outcome: Outcome) {
    shared(|s| s.result = Some(outcome));
}

/// The pad's keys of a frame as the menus take them (the KH shop's map: A decide, B back, LB / RB, LT / RT; the D-pad /
/// left stick steps; the host's `menu ...` words).
pub fn keys(pressed: kh_core::PadButtons, nav: &[(u8, bool)], words: &[String]) -> Vec<(CampKey, bool)> {
    use kh_core::PadButton as B;
    let mut keys: Vec<(CampKey, bool)> = nav.iter().map(|(dir, repeat)| ([CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3], *repeat)).collect();
    for (button, key) in [(B::FaceBottom, CampKey::Decide), (B::FaceRight, CampKey::Cancel), (B::LeftShoulder, CampKey::L1), (B::RightShoulder, CampKey::R1), (B::LeftTrigger, CampKey::L2), (B::RightTrigger, CampKey::R2)] {
        if pressed.has(button) {
            keys.push((key, false));
        }
    }
    for word in words {
        let key = match word.as_str() {
            "menu up" => CampKey::Up,
            "menu down" => CampKey::Down,
            "menu left" => CampKey::Left,
            "menu right" => CampKey::Right,
            "menu decide" => CampKey::Decide,
            "menu cancel" => CampKey::Cancel,
            "menu l1" => CampKey::L1,
            "menu r1" => CampKey::R1,
            "menu l2" => CampKey::L2,
            "menu r2" => CampKey::R2,
            _ => continue,
        };
        keys.push((key, false));
    }
    keys
}

// ------------------------------------------------------------------------------------------------ the rows (pure)

/// What the rows read besides the rules: KH3's strings (as written, markup kept), an item's description, an ability's name.
pub struct Lookup<'a> {
    pub raw: &'a dyn Fn(&str) -> String,
    pub help: &'a dyn Fn(&str) -> String,
    pub ability: &'a dyn Fn(&str) -> String,
}

/// An item's name and category icon by key (a material, an item, a weapon).
pub fn item_name(key: &str) -> (String, u8) {
    if let Some(m) = rules::tables().material(key) {
        return (m.name.clone(), CAT_MATERIAL);
    }
    if let Some(w) = crate::weapons::by_key(key) {
        return (w.name.clone(), CAT_WEAPON);
    }
    match crate::items::by_key(key) {
        Some(i) => (
            i.name.clone(),
            match i.class {
                crate::items::Class::Armour => CAT_ARMOUR,
                crate::items::Class::Accessory => CAT_ACCESSORY,
                crate::items::Class::Material => CAT_MATERIAL,
                _ => CAT_ITEM,
            },
        ),
        None => (key.to_string(), CAT_ITEM),
    }
}

/// A recipe's reward kind as the list's icon.
fn category_of(kind: &str) -> u8 {
    match kind {
        "weapon" => CAT_WEAPON,
        "armour" => CAT_ARMOUR,
        "accessory" => CAT_ACCESSORY,
        "material" => CAT_MATERIAL,
        _ => CAT_ITEM,
    }
}

fn needs(ws: &Workshop, list: &[(String, u32)]) -> Vec<MatNeed> {
    list.iter().map(|(k, n)| MatNeed { key: k.clone(), name: item_name(k).0, need: *n, stock: rules::stock_of(ws, k) }).collect()
}

/// A goal's list text (KH3's collection_00n0 with its numbers).
pub fn goal_text(goal: &rules::Goal, raw: &dyn Fn(&str) -> String) -> String {
    let template = raw(&goal.text);
    match &goal.condition {
        GoalCondition::KindNum(n) | GoalCondition::TotalNum(n) => texts::format(&template, *n, ""),
        GoalCondition::SpKindNum { material, n } => texts::format(&template, *n, &item_name(material).0),
        GoalCondition::GetRank(_) | GoalCondition::KindAll { .. } => texts::format(&template, 0, ""),
    }
}

/// THE ROWS OF EVERY SCREEN from the rules (`ws` the Workshop's record, `host` Sora's bag and Keyblades, `flags` the
/// event flags).
pub fn build(ws: &Workshop, host: &dyn WorkshopHost, flags: &dyn Fn(u32) -> bool, look: &Lookup) -> WorkshopData {
    let t = rules::tables();
    let recipe_views = rules::recipes(ws, host);
    let recipes: Vec<RecipeRow> = recipe_views
        .iter()
        .filter_map(|v| {
            let r = t.recipe(&v.id)?;
            let keyblade = t.is_keyblade(r.item_id);
            let held = if keyblade { u32::from(host.keyblade_level(&r.reward).is_some()) } else { host.carried(&r.reward) };
            Some(RecipeRow { id: r.id.clone(), key: r.reward.clone(), name: r.name.clone(), category: category_of(&r.kind), tab: kh_ui::workshop::tab_of(&r.kind), held, max: v.max, made: v.made, new: v.new, materials: needs(ws, &r.materials), help: (look.help)(&r.reward) })
        })
        .collect();
    let keyblades: Vec<ForgeRow> = rules::forge_list(ws, host)
        .into_iter()
        .map(|e| {
            let name = t.keyblade(&e.key).map_or_else(|| e.key.clone(), |k| k.name.clone());
            let next = e.next.as_ref().map(|n| {
                let s = e.next_stats.clone().unwrap_or_default();
                ForgeNext { level: n.level, strength: s.strength, magic: s.magic, ability: n.ability.as_deref().map(|a| (look.ability)(a)), materials: needs(ws, &n.materials), ready: e.ready }
            });
            ForgeRow { key: e.key.clone(), name, level: e.level, max_level: e.max, strength: e.stats.strength, magic: e.stats.magic, next }
        })
        .collect();
    let photo_views = rules::photo_list(ws, flags);
    let photos: Vec<PhotoRow> = photo_views
        .iter()
        .filter_map(|v| {
            let p = t.photo(&v.id)?;
            let help = match v.progress {
                Some((have, need)) => p.help.replace("{0}", &have.to_string()).replace("{1}", &need.to_string()),
                None => p.help.clone(),
            };
            let reward = p.recipes.first().and_then(|id| t.recipe(id));
            let text = p.name_from.as_ref().map_or_else(|| p.name.clone(), |r| r.resolve(look.raw)); // (KH3TEXT) KH3's line from the pack
            Some(PhotoRow { id: p.id.clone(), text, help, done: v.done, new: v.new, reward_key: reward.map(|r| r.reward.clone()).unwrap_or_default(), reward_name: reward.map(|r| r.name.clone()).unwrap_or_default() })
        })
        .collect();
    let goals = t
        .goals
        .iter()
        .map(|g| {
            let (reward, items): (RewardKind, Vec<String>) = match &g.reward {
                GoalReward::GetItem(item) => (RewardKind::Item, vec![item.clone()]),
                GoalReward::EnableSynthesis(list) => (RewardKind::Synthesis, list.iter().filter_map(|id| t.recipe(id).map(|r| r.reward.clone())).collect()),
                GoalReward::EnableBuy(list) => (RewardKind::Buy, list.clone()),
            };
            let items = items.iter().map(|k| { let (name, cat) = item_name(k); (cat, name) }).collect();
            GoalRow { id: g.id.clone(), text: goal_text(g, look.raw), done: rules::goal_done(ws, &g.id), reward, items }
        })
        .collect();
    let materials = rules::material_list(ws).into_iter().map(|m| MaterialRow { rank: t.material(&m.key).map_or(3, |x| x.rank), key: m.key, name: m.name, known: m.known, stock: m.stock }).collect();
    let log = rules::log(ws);
    let commands = rules::menu()
        .iter()
        .filter_map(|e| {
            let screen = match e.entry.as_str() {
                "synthesize" => Screen::Synthesis,
                "forge" => Screen::Forge,
                "photo" => Screen::Photo,
                "goals" => Screen::Goals,
                "materials" => Screen::Materials,
                _ => return None,
            };
            let new = match screen {
                Screen::Synthesis => recipe_views.iter().any(|v| v.new),
                Screen::Photo => photo_views.iter().any(|v| v.new),
                _ => false,
            };
            Some((screen, new))
        })
        .collect();
    WorkshopData { title: String::new(), munny: 0, commands, log: LogRow { created: log.created, photos: log.photos, goals: log.goals, complete: log.complete() }, recipes, keyblades, photos, goals, materials }
}

/// The record's `ext` key of a deposit made at a smith's Forge and not yet told (dialog_0010 waits for the Moogle).
const EXT_DEPOSIT_UNTOLD: &str = "p6b_deposit_untold";

/// THE OPENING'S VISIT by entry (the coordinator, 2026-11-03: a smith's Forge shows only the Forge):
/// - the Moogle ([`Entry::Workshop`]): KH3's whole Workshop visit (`crate::workshop::visit`); a deposit made earlier at a
///   smith's Forge is told now (dialog_0010), as KH3 tells it on entering the Workshop;
/// - a smith ([`Entry::Forge`]): the carried materials go into the Workshop's stock (the Forge counts the stock, P6a) and the
///   Moogle level follows the flags; the Collector's Goals and the milestones are NOT tested here: KH3 tests them, with
///   their dialogs, on entering the Workshop, so they wait for the next Moogle visit (the materials ever obtained keep
///   counting through P1's hook). The returned visit then says nothing to show.
pub fn enter(entry: Entry, ws: &mut Workshop, host: &mut dyn WorkshopHost, flags: &dyn Fn(u32) -> bool) -> Visit {
    match entry {
        Entry::Workshop => {
            let mut visit = rules::visit(ws, host, flags);
            if ws.ext.get(EXT_DEPOSIT_UNTOLD).is_some_and(|v| v.as_bool() == Some(true)) {
                visit.deposited = true;
            }
            if let Some(o) = ws.ext.as_object_mut() {
                o.remove(EXT_DEPOSIT_UNTOLD);
            }
            visit
        }
        Entry::Forge => {
            if rules::deposit(ws, host) {
                if !ws.ext.is_object() {
                    ws.ext = serde_json::json!({});
                }
                ws.ext[EXT_DEPOSIT_UNTOLD] = serde_json::json!(true);
            }
            rules::raise_shop_level(ws, flags);
            Visit::default()
        }
    }
}

/// THE VISIT'S DIALOGS in KH3's order (0x14172EFF0): dialog_0010; per goal achieved, in table order, dialog_0020 (once) /
/// dialog_0030 (static SE 10) / the merchants' line for each purchase; dialog_0050 once.
pub fn messages(visit: &Visit, raw: &dyn Fn(&str) -> String) -> Vec<Message> {
    let t = rules::tables();
    let mut out = Vec::new();
    if visit.deposited {
        out.push(Message { text: raw("kh3_menu.synthesis.dialog_0010"), sound: None });
    }
    let mut recipes_said = false;
    for id in &visit.goals {
        let Some(goal) = t.goal(id) else { continue };
        match &goal.reward {
            GoalReward::EnableSynthesis(_) if !recipes_said => {
                recipes_said = true;
                out.push(Message { text: raw("kh3_menu.synthesis.dialog_0020"), sound: None });
            }
            GoalReward::EnableSynthesis(_) => {}
            GoalReward::GetItem(item) => out.push(Message { text: raw("kh3_menu.synthesis.dialog_0030").replace("{0}", &item_name(item).0), sound: Some(SE_ITEM_GET) }),
            GoalReward::EnableBuy(items) => {
                for item in items {
                    out.push(Message { text: texts::MERCHANT_DIALOG.replace("{0}", &item_name(item).0), sound: None });
                }
            }
        }
    }
    if !visit.photos.is_empty() {
        out.push(Message { text: raw("kh3_menu.synthesis.dialog_0050"), sound: None });
    }
    out
}

/// What a choice did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Applied {
    /// A synthesis: the item made (its key) and how many.
    Made { reward: String, count: u32 },
    Forged { key: String, level: u8 },
    Refused(String),
    Marked,
    Closed,
}

/// CARRIES OUT A CHOICE through the rules.
pub fn apply(event: &WorkshopEvent, ws: &mut Workshop, host: &mut dyn WorkshopHost) -> Applied {
    match event {
        WorkshopEvent::Synthesize { id, count } => match rules::synthesize(ws, host, id, *count) {
            Ok(s) => Applied::Made { reward: s.reward, count: s.count },
            Err(e) => Applied::Refused(format!("synthesis {id} x{count}: {e:?}")),
        },
        WorkshopEvent::Forge { key } => match rules::forge(ws, host, key) {
            Ok(f) => Applied::Forged { key: f.key, level: f.level },
            Err(e) => Applied::Refused(format!("forge {key}: {e:?}")),
        },
        WorkshopEvent::SeenRecipe(id) => {
            rules::mark_recipe_seen(ws, id);
            Applied::Marked
        }
        WorkshopEvent::SeenPhoto(id) => {
            rules::mark_photo_seen(ws, id);
            Applied::Marked
        }
        WorkshopEvent::Closed => Applied::Closed,
    }
}

// ------------------------------------------------------------------------------------------------ the HUD's state

/// The Workshop in the HUD: KH3's Shop movie and system window, the result popup, the menu's logic.
#[derive(Default)]
pub struct WorkshopState {
    view: Option<WorkshopView>,
    menu: Option<WorkshopMenu>,
    /// KH3's large item-get popup (movie KeyItem) for a synthesis.
    popup: Option<kh_ui::key_item::KeyItemPopup>,
    entry: Entry,
    title: String,
    flags: BTreeSet<u32>,
    /// Ability ids -> KH3's names (pack `data/abilities.json`).
    abilities: Vec<(String, String)>,
    /// What it did (for the host's log and tests).
    pub log: Vec<String>,
    /// It closed since the HUD last asked ([`WorkshopState::take_closed`]).
    closed: bool,
    /// (P9b's hold, plan Q4) a synthesized weapon's OBTAINED popup is held until KH3's own result popup closes.
    holding: bool,
}

impl WorkshopState {
    pub fn load(pack: &mut kh_ui::Pack, fonts: &kh_ui::Fonts) -> WorkshopState {
        let view = WorkshopView::load(pack, fonts.clone()).map_err(|e| println!("hud: KH3's Workshop (movie Shop) is NOT available ({e})")).ok();
        let popup = pack.movie_linked(kh_ui::key_item::MOVIE, &kh_ui::camp_menu::LIBRARIES).ok().map(|def| kh_ui::key_item::KeyItemPopup::new(def, fonts.clone()));
        let abilities = pack.data("abilities").ok().and_then(|v| v["rows"].as_array().cloned()).unwrap_or_default().iter().filter_map(|r| Some((r["id"].as_str()?.to_string(), r["name"].as_str()?.to_string()))).collect();
        WorkshopState { view, popup, abilities, ..WorkshopState::default() }
    }

    /// Up (opening, open, closing, or its popup).
    pub fn is_open(&self) -> bool {
        self.view.as_ref().is_some_and(WorkshopView::is_open) || self.popup.as_ref().is_some_and(|p| p.is_open())
    }

    /// The menu's logic while it is up (tests, the host's log).
    pub fn menu(&self) -> Option<&WorkshopMenu> {
        self.menu.as_ref().filter(|_| self.is_open())
    }

    /// Its movies on the HUD's layers (drawn in `hud_layers::LAYERS` order): the screen on "CampMenu"; ABOVE it, on
    /// "Dialog", KH3's item-get result popup while it is up, else KH3's system window (never up together). (In-game bug
    /// of the combined round, D3 / D4: the result popup was routed to "Pause", the OBTAINED popup's usual layer, which lies
    /// UNDER "CampMenu": the full-screen Workshop covered it.) `None`: not up (the HUD's other routes apply).
    pub fn movie(&self, name: &str) -> Option<Option<&kh_ui::Movie>> {
        let view = self.view.as_ref().filter(|v| v.is_open())?;
        Some(match name {
            "CampMenu" => Some(&view.movie),
            "Dialog" => self.popup.as_ref().filter(|p| p.is_open()).map(|p| &p.movie).or_else(|| view.dialog_movie()),
            _ => None,
        })
    }

    /// (THE USER 2026-11-03: "right stick scroll") the right stick's lines on the open screen's descriptions.
    pub fn scroll_help(&mut self, lines: i32) {
        if let Some(v) = self.view.as_mut().filter(|v| v.is_open()) {
            v.movie.scroll_fits(lines);
            v.dialog.scroll_fits(lines);
        }
    }

    /// True once after it closed (the HUD then unpauses as after the shop: the pouch's refill).
    pub fn take_closed(&mut self) -> bool {
        std::mem::take(&mut self.closed)
    }

    /// The host closes it from outside.
    pub fn close(&mut self) {
        if let Some(v) = self.view.as_mut() {
            v.close();
        }
        if let Some(p) = self.popup.as_mut() {
            p.force_close();
        }
        self.release_hold();
    }

    /// (P9b, plan Q4: "synthesis shows it after KH3's own result") KH3's result is over: the held OBTAINED popup may follow.
    fn release_hold(&mut self) {
        if std::mem::take(&mut self.holding) {
            crate::bossrewards::hold_weapon_popups(false);
        }
    }

    fn lookup_rows(&self, ws: &Workshop, host: &dyn WorkshopHost) -> WorkshopData {
        let Some(view) = self.view.as_ref() else { return WorkshopData::default() };
        let raw = |id: &str| view.raw(id);
        let help = |key: &str| {
            let own = crate::items::own_help(key);
            if !own.is_empty() {
                return own;
            }
            view.art.help_id(key).map(|id| view.string(id)).unwrap_or_default()
        };
        let ability = |id: &str| self.abilities.iter().find(|a| a.0 == id).map_or_else(|| id.to_string(), |a| a.1.clone());
        let flags = |f: u32| self.flags.contains(&f);
        build(ws, host, &flags, &Lookup { raw: &raw, help: &help, ability: &ability })
    }

    /// The rows now (the running game's record and bag).
    fn data(&self, munny: i64) -> WorkshopData {
        let mut data = rules::with_live(|ws, host| self.lookup_rows(ws, host));
        data.title.clone_from(&self.title);
        data.munny = munny;
        data
    }

    /// THE WORKSHOP'S PART OF A FRAME: a request opens it (unless `blocked`: another screen of the HUD is up), the pad
    /// drives it, its movies run. True while it has the frame.
    pub fn frame(&mut self, keys: &[(CampKey, bool)], ticks: u32, munny: i64, blocked: bool, sounds: &mut Vec<UiSound>, log: &dyn Fn(&str)) -> bool {
        if shared(|s| std::mem::take(&mut s.close)) && self.is_open() {
            self.close();
            log("workshop: closed by the host");
        }
        if !self.is_open() && !blocked {
            let request = shared(|s| s.request.take());
            if let Some(r) = request {
                self.start(r, munny, sounds, log);
            }
        }
        if !self.is_open() {
            return false;
        }
        // the item-get popup has the pad while it is up
        let popup_up = self.popup.as_ref().is_some_and(|p| p.is_open());
        if popup_up {
            if keys.iter().any(|k| k.0 == CampKey::Decide && !k.1) {
                if let Some(p) = self.popup.as_mut() {
                    p.decide(sounds);
                }
            }
        } else if let Some(menu) = self.menu.as_mut() {
            if self.view.as_ref().is_some_and(WorkshopView::ready) {
                for (key, repeat) in keys {
                    menu.key(*key, *repeat, sounds);
                }
            }
        }
        self.events(munny, sounds, log);
        for _ in 0..ticks.max(1) {
            if let Some(p) = self.popup.as_mut() {
                p.tick(sounds);
            }
            let (Some(view), Some(menu)) = (self.view.as_mut(), self.menu.as_mut()) else { break };
            view.sync(menu);
            if view.tick(Some(menu)) {
                self.menu = None;
                self.closed = true;
                set_up(false);
                set_result(Outcome::Closed(self.entry));
                log("workshop: closed");
                break;
            }
        }
        if self.holding && !self.popup.as_ref().is_some_and(|p| p.is_open()) {
            self.release_hold();
            log("workshop: KH3's result closed: the synthesized weapon's OBTAINED popup may follow");
        }
        true
    }

    fn start(&mut self, r: Request, munny: i64, sounds: &mut Vec<UiSound>, log: &dyn Fn(&str)) {
        if self.view.is_none() {
            log("workshop: REFUSED (no Shop movie in the UI pack)");
            set_result(Outcome::Closed(r.entry));
            return;
        }
        self.entry = r.entry;
        self.title = r.title;
        self.flags = r.flags;
        let flags = self.flags.clone();
        let entry = self.entry;
        let visit = rules::with_live(|ws, host| enter(entry, ws, host, &|f| flags.contains(&f)));
        let raw = |id: &str| self.view.as_ref().map(|v| v.raw(id)).unwrap_or_else(|| texts::composed(id).map_or_else(|| texts::missing(id), str::to_string));
        let said = messages(&visit, &raw);
        let data = self.data(munny);
        let mut menu = WorkshopMenu::new(data, self.entry);
        if let Some(view) = self.view.as_mut() {
            view.open(&mut menu);
        }
        sounds.push(UiSound::Play(kh_ui::menu::SE_MENU_OPEN));
        menu.push_messages(said, sounds);
        let line = format!("workshop: OPENS ({:?}, \"{}\"): visit {:?}", self.entry, self.title, visit);
        log(&line);
        self.log.push(line);
        self.menu = Some(menu);
        set_up(true);
    }

    /// The menu's choices, carried out on the running game; the rows again.
    fn events(&mut self, munny: i64, sounds: &mut Vec<UiSound>, log: &dyn Fn(&str)) {
        let Some(events) = self.menu.as_mut().map(WorkshopMenu::take_events) else { return };
        let mut changed = false;
        for event in events {
            // (P9b) a weapon's OBTAINED popup waits for KH3's own result popup
            let weapon = matches!(&event, WorkshopEvent::Synthesize { id, .. } if rules::tables().recipe(id).is_some_and(|r| crate::weapons::by_key(&r.reward).is_some()));
            if weapon && self.popup.is_some() {
                crate::bossrewards::hold_weapon_popups(true);
                self.holding = true;
            }
            let done = rules::with_live(|ws, host| apply(&event, ws, host));
            if !matches!(done, Applied::Made { .. }) {
                self.release_hold();
            }
            let line = format!("workshop: {event:?} -> {done:?}");
            match &done {
                Applied::Made { reward, count } => {
                    // KH3's large item-get popup (static SE 10): the picture, the name (x n), the description
                    if let (Some(view), Some(popup)) = (self.view.as_ref(), self.popup.as_mut()) {
                        let (name, _) = item_name(reward);
                        let name = if *count > 1 { format!("{name} x {count}") } else { name };
                        let caption = view.art.help_id(reward).map(|id| view.string(id)).unwrap_or_else(|| crate::items::own_help(reward));
                        log(&format!("workshop: KH3's item-get result popup: \"{name}\" (layer Dialog, static SE 10)"));
                        popup.show(kh_ui::key_item::Obtained { picture: view.art.icon(reward), name, caption }, sounds);
                    }
                    changed = true;
                }
                Applied::Forged { .. } | Applied::Marked => changed = true,
                Applied::Refused(_) => sounds.push(UiSound::Play(kh_ui::menu::SE_MENU_REFUSED)),
                Applied::Closed => {
                    if let Some(v) = self.view.as_mut() {
                        v.close();
                    }
                }
            }
            if !matches!(done, Applied::Marked) {
                log(&line);
                self.log.push(line);
            }
        }
        if changed {
            let data = self.data(munny);
            if let Some(menu) = self.menu.as_mut() {
                menu.set_data(data);
            }
        }
    }
}

#[cfg(test)]
mod tests;

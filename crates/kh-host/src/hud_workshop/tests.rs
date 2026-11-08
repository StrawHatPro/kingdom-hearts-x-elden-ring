//! (weapons P6b) The Workshop's screens on P6a's rules: the rows, the visit's dialogs, every choice carried out.

use std::collections::BTreeMap;

use super::*;

/// Sora's bag and Keyblades, as a host keeps them.
#[derive(Default)]
struct Bag {
    items: BTreeMap<String, u32>,
    keyblades: BTreeMap<String, u8>,
}

impl WorkshopHost for Bag {
    fn carried(&self, key: &str) -> u32 {
        self.items.get(key).copied().unwrap_or(0)
    }
    fn take_carried(&mut self, key: &str, count: u32) {
        let n = self.carried(key).saturating_sub(count);
        self.items.insert(key.to_string(), n);
    }
    fn give(&mut self, key: &str, count: u32) {
        if let Some(k) = rules::tables().keyblade(key) {
            self.keyblades.insert(key.to_string(), k.init);
        } else {
            *self.items.entry(key.to_string()).or_default() += count;
        }
    }
    fn keyblade_level(&self, key: &str) -> Option<u8> {
        self.keyblades.get(key).copied()
    }
    fn set_keyblade_level(&mut self, key: &str, level: u8) {
        self.keyblades.insert(key.to_string(), level);
    }
}

/// (KH3TEXT) The pack's text of an id, as the tests see it: the id and KH3's {0} (KH3's text is not compiled in; the real
/// texts come from the player's pack at run time).
fn raw(id: &str) -> String {
    format!("<{id}> {{0}}")
}

fn look() -> Lookup<'static> {
    Lookup { raw: &raw, help: &|_| String::new(), ability: &|id| format!("[{id}]") }
}

fn no_flags(_: u32) -> bool {
    false
}

fn run(menu: &mut WorkshopMenu, keys: &[CampKey]) {
    let mut s = Vec::new();
    for k in keys {
        menu.key(*k, false, &mut s);
    }
}

/// SYNTHESIZE ITEMS through the screen and the rules: the visit moves the bag's materials into the stock (dialog_0010);
/// IS_0 (Ether, always listed) at the quantity chosen on the row consumes EXACTLY count x each material from the stock,
/// nothing else, and hands count Ethers over; the clear mark and the SYNTH LOG follow; no munny anywhere.
#[test]
fn synthesis_consumes_exactly_the_recipe() {
    let t = rules::tables();
    let recipe = t.recipe("IS_0").expect("IS_0");
    let mut ws = Workshop::default();
    let mut bag = Bag::default();
    for (k, n) in &recipe.materials {
        bag.items.insert(k.clone(), n * 5);
    }
    bag.items.insert("MAT_ITEM60".into(), 3);
    let visit = rules::visit(&mut ws, &mut bag, &no_flags);
    let said = messages(&visit, &raw);
    assert_eq!(said.first().map(|m| m.text.as_str()), Some(raw("kh3_menu.synthesis.dialog_0010").as_str()), "the deposit's dialog first");
    let before: BTreeMap<String, u32> = ws.stock.clone();
    let data = build(&ws, &bag, &no_flags, &look());
    let row = data.recipes.iter().find(|r| r.id == "IS_0").expect("listed");
    assert_eq!((row.max, row.made, row.key.as_str()), (5.min(recipe.max), 0, recipe.reward.as_str()));
    let mut menu = WorkshopMenu::new(data, Entry::Workshop);
    menu.push_messages(said, &mut Vec::new());
    run(&mut menu, &[CampKey::Decide]); // the dialog
    run(&mut menu, &[CampKey::Decide]); // Synthesize Items
    assert_eq!(menu.screen, Screen::Synthesis);
    let k = menu.recipe_rows().iter().position(|r| r.id == "IS_0").unwrap();
    menu.synth.set_focus(k);
    run(&mut menu, &[CampKey::Right, CampKey::Right, CampKey::Decide, CampKey::Left, CampKey::Decide]);
    let events: Vec<WorkshopEvent> = menu.take_events().into_iter().filter(|e| matches!(e, WorkshopEvent::Synthesize { .. })).collect();
    assert_eq!(events, vec![WorkshopEvent::Synthesize { id: "IS_0".into(), count: 3 }]);
    let done = apply(&events[0], &mut ws, &mut bag);
    assert_eq!(done, Applied::Made { reward: recipe.reward.clone(), count: 3 });
    for (key, n) in &before {
        let used = recipe.materials.iter().find(|m| &m.0 == key).map_or(0, |m| m.1 * 3);
        assert_eq!(rules::stock_of(&ws, key), n - used, "{key}: exactly the recipe");
    }
    assert_eq!(bag.carried(&recipe.reward), 3);
    let data = build(&ws, &bag, &no_flags, &look());
    assert_eq!((data.recipes.iter().find(|r| r.id == "IS_0").unwrap().made, data.log.created), (3, 1));
    // a count past what the stock holds is refused by the rules (nothing moves)
    let again = apply(&WorkshopEvent::Synthesize { id: "IS_0".into(), count: 50 }, &mut ws, &mut bag);
    assert!(matches!(again, Applied::Refused(_)), "{again:?}");
}

/// THE KEYBLADE FORGE (a smith's entry): the Kingdom Key's next level for its materials (no munny); the screen shows
/// now -> after; Yes raises the level in the record and takes exactly the level's materials; at +10 nothing more.
#[test]
fn forge_level_up_through_the_screen() {
    let t = rules::tables();
    let key = crate::weapons::KINGDOM_KEY;
    let kb = t.keyblade(key).expect("Kingdom Key");
    let level1 = &kb.levels[0];
    let mut ws = Workshop::default();
    let mut bag = Bag::default();
    bag.keyblades.insert(key.into(), 0);
    for (m, n) in &level1.materials {
        ws.stock.insert(m.clone(), *n + 1);
    }
    let data = build(&ws, &bag, &no_flags, &look());
    let row = data.keyblades.iter().find(|r| r.key == key).expect("owned: listed");
    let next = row.next.clone().expect("+1 offered");
    let stats1 = rules::keyblade_stats(key, 1).unwrap();
    assert_eq!((next.level, next.strength, next.magic, next.ready), (1, stats1.strength, stats1.magic, true));
    assert_eq!(next.ability, level1.ability.as_ref().map(|a| format!("[{a}]")));
    let mut menu = WorkshopMenu::new(data, Entry::Forge);
    let k = menu.data.keyblades.iter().position(|r| r.key == key).unwrap();
    menu.keyblade = k;
    run(&mut menu, &[CampKey::Decide, CampKey::Decide]);
    let events = menu.take_events();
    assert_eq!(events, vec![WorkshopEvent::Forge { key: key.into() }]);
    assert_eq!(apply(&events[0], &mut ws, &mut bag), Applied::Forged { key: key.into(), level: 1 });
    assert_eq!(bag.keyblade_level(key), Some(1));
    for (m, _) in &level1.materials {
        assert_eq!(rules::stock_of(&ws, m), 1, "{m}: the level's count taken");
    }
    // the screen's next data: +2 offered, not ready (its materials are not in the stock)
    let data = build(&ws, &bag, &no_flags, &look());
    let row = data.keyblades.iter().find(|r| r.key == key).unwrap();
    assert_eq!((row.level, row.next.as_ref().map(|n| n.level)), (1, Some(2)));
    bag.keyblades.insert(key.into(), 10);
    let data = build(&ws, &bag, &no_flags, &look());
    assert!(data.keyblades.iter().find(|r| r.key == key).unwrap().next.is_none(), "+10: nothing more");
    assert!(matches!(apply(&WorkshopEvent::Forge { key: key.into() }, &mut ws, &mut bag), Applied::Refused(_)));
}

/// COLLECTOR'S GOALS, KH3's three reward kinds: every material obtained enough makes every goal pass at one visit; the
/// dialogs are KH3's (dialog_0020 once, dialog_0030 per item with static SE 10) and the COMPOSED merchants' line per
/// purchase (no "shop"); the item reaches the bag, the recipes are listed, the purchases are unlocked for the merchants;
/// the goal rows carry their kind and items, KH3's texts with their numbers.
#[test]
fn goals_claim_each_reward_kind() {
    let t = rules::tables();
    let mut ws = Workshop::default();
    for m in &t.materials {
        ws.obtained.insert(m.key.clone(), 999);
        ws.total_obtained += 999;
    }
    let mut bag = Bag::default();
    let before = build(&ws, &bag, &no_flags, &look());
    let kinds = |d: &WorkshopData, k: RewardKind| d.goals.iter().filter(|g| g.reward == k).count();
    assert_eq!((kinds(&before, RewardKind::Item), kinds(&before, RewardKind::Synthesis), kinds(&before, RewardKind::Buy)), (24, 24, 30));
    assert!(before.goals.iter().all(|g| !g.done && !g.text.is_empty() && !g.text.contains("{0}") && !g.text.contains("plural")), "KH3's texts with their numbers");
    let visit = rules::visit(&mut ws, &mut bag, &no_flags);
    assert_eq!(visit.goals.len(), 78);
    let said = messages(&visit, &raw);
    let count = |text: &str| said.iter().filter(|m| m.text.starts_with(text)).count();
    assert_eq!(count(raw("kh3_menu.synthesis.dialog_0020").as_str()), 1, "dialog_0020 once");
    let items: Vec<&Message> = said.iter().filter(|m| m.text.starts_with("<kh3_menu.synthesis.dialog_0030> ")).collect();
    assert_eq!(items.len(), 24);
    assert!(items.iter().all(|m| m.sound == Some(SE_ITEM_GET)));
    let buys = said.iter().filter(|m| m.text.starts_with("You've finished a Collector's Goal!<br>Merchants now sell")).count();
    assert_eq!(buys, visit.buy_unlocked.len());
    assert!(buys >= 30 && said.iter().all(|m| !m.text.contains("added to the shop")));
    // the item kind: the first GET_ITEM goal's item is in the bag (or its stock)
    let first_item = t.goals.iter().find_map(|g| if let GoalReward::GetItem(i) = &g.reward { Some(i.clone()) } else { None }).unwrap();
    assert!(bag.carried(&first_item) + rules::stock_of(&ws, &first_item) + u32::from(bag.keyblade_level(&first_item).is_some()) >= 1, "{first_item}");
    // the recipe kind: every goal recipe listed; the purchase kind: the record's buy list
    assert!(t.recipes.iter().filter(|r| matches!(r.unlock, rules::Unlock::Goal(_))).all(|r| rules::recipe_listed(&ws, r)));
    assert!(!ws.buy_unlocked.is_empty());
    let after = build(&ws, &bag, &no_flags, &look());
    assert!(after.goals.iter().all(|g| g.done) && after.log.goals == 78 && after.log.complete[2]);
    let buy = after.goals.iter().find(|g| g.reward == RewardKind::Buy).unwrap();
    assert!(!buy.items.is_empty() && buy.items.iter().all(|i| i.0 == CAT_MATERIAL));
    // a second visit grants nothing again (once each); only the materials the goals handed over are left at the Workshop
    let again = messages(&rules::visit(&mut ws, &mut bag, &no_flags), &raw);
    assert!(again.iter().all(|m| m.text == raw("kh3_menu.synthesis.dialog_0010")), "{again:?}");
}

/// PHOTO MISSIONS = the milestones: a milestone's defeat flag makes it done at the next visit (dialog_0050 once), its
/// recipe listed; the row shows the Moogle's line, COMPLETE and the reward; NEW goes once seen; the menu's flags are the
/// rules' flags.
#[test]
fn milestones_complete_at_the_visit() {
    let t = rules::tables();
    let photo = t.photos.iter().find(|p| matches!(p.done, Milestone::Flag(_))).expect("a one-boss milestone");
    let Milestone::Flag(flag) = photo.done else { unreachable!() };
    assert!(flags_read().contains(&flag));
    let flags = move |f: u32| f == flag;
    let mut ws = Workshop::default();
    let mut bag = Bag::default();
    let visit = rules::visit(&mut ws, &mut bag, &flags);
    assert_eq!(visit.photos, vec![photo.id.clone()]);
    let said = messages(&visit, &raw);
    assert_eq!(said.last().map(|m| m.text.as_str()), Some(raw("kh3_menu.synthesis.dialog_0050").as_str()));
    let data = build(&ws, &bag, &flags, &look());
    let row = data.photos.iter().find(|p| p.id == photo.id).expect("listed");
    assert!(row.done && row.new && row.text == photo.name && !row.reward_name.is_empty());
    assert!(data.commands.iter().any(|c| c.0 == Screen::Photo && c.1), "the command's NEW mark");
    assert!(data.recipes.iter().any(|r| Some(&r.id) == photo.recipes.first()), "its recipe listed");
    apply(&WorkshopEvent::SeenPhoto(photo.id.clone()), &mut ws, &mut bag);
    let data = build(&ws, &bag, &flags, &look());
    assert!(!data.photos.iter().find(|p| p.id == photo.id).unwrap().new);
    assert_eq!((data.log.photos, data.commands.len()), (1, 4), "the SYNTH LOG; KH3's commands without the Forge");
    assert!(data.commands.iter().all(|c| c.0 != Screen::Forge), "the Moogle has no Keyblade Forge (the user 2026-11-07)");
}

/// THE HUD'S STATE on the real UI pack (skipped without it): a host's `open(Entry::Forge)` opens it at the next frame
/// straight on the Forge, the pad drives it, B closes it, the host is told once (`take_result`).
#[test]
fn the_host_opens_the_forge_and_is_told_it_closed() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    let Ok(mut pack) = kh_ui::Pack::open(&dir) else {
        eprintln!("SKIPPED: no UI pack");
        return;
    };
    let Ok(fonts) = kh_ui::Fonts::load(&mut pack) else { return };
    let mut state = WorkshopState::load(&mut pack, &fonts);
    if state.view.is_none() {
        eprintln!("SKIPPED: no Shop movie");
        return;
    }
    let _door = crate::items::TEST_DOOR.lock().unwrap_or_else(|e| e.into_inner());
    assert!(open(Entry::Forge, "Smithing Master Hewg", &no_flags));
    assert!(is_open() && !open(Entry::Workshop, "", &no_flags), "one at a time");
    let mut sounds = Vec::new();
    let quiet = |_: &str| {};
    assert!(!state.frame(&[], 1, 0, true, &mut sounds, &quiet), "blocked: it waits");
    assert!(state.frame(&[], 1, 100, false, &mut sounds, &quiet));
    assert_eq!(state.menu().map(|m| (m.entry, m.screen)), Some((Entry::Forge, Screen::Forge)));
    assert!(state.movie("CampMenu").is_some_and(|m| m.is_some()));
    for _ in 0..40 {
        state.frame(&[], 1, 100, false, &mut sounds, &quiet);
    }
    // any entry dialogs of the visit first, then B
    for _ in 0..20 {
        if state.menu().is_some_and(|m| m.message().is_none()) {
            break;
        }
        state.frame(&[(CampKey::Decide, false)], 1, 100, false, &mut sounds, &quiet);
        state.frame(&[], 1, 100, false, &mut sounds, &quiet);
    }
    state.frame(&[(CampKey::Cancel, false)], 1, 100, false, &mut sounds, &quiet);
    let mut told = None;
    for _ in 0..200 {
        state.frame(&[], 1, 100, false, &mut sounds, &quiet);
        if let Some(r) = take_result() {
            told = Some(r);
            break;
        }
    }
    assert_eq!(told, Some(Outcome::Closed(Entry::Forge)));
    assert!(!is_open() && !state.is_open() && take_result().is_none(), "told once");
    assert!(state.take_closed() && !state.take_closed(), "the HUD is told once too (its unpause)");
}

/// (pictures) THE SCREENS ON THE REAL ROWS (P6a's sheet, the UI pack's strings and art; skipped without the pack): a Sora
/// some bosses in, every screen and KH3's item-get popup of a synthesis. build/sora_tmp/weapons/P6b/shots/2x_*.png.
#[test]
fn pictures_on_the_real_rows() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    let Ok(mut pack) = kh_ui::Pack::open(&dir) else { return };
    let Ok(fonts) = kh_ui::Fonts::load(&mut pack) else { return };
    let Ok(mut view) = WorkshopView::load(&mut pack, fonts.clone()) else { return };
    let t = rules::tables();
    let mut ws = Workshop::default();
    let mut bag = Bag::default();
    for (k, m) in t.materials.iter().enumerate().filter(|(k, _)| k % 3 != 2) {
        bag.items.insert(m.key.clone(), (k as u32 % 7) * 4 + 3);
        ws.obtained.insert(m.key.clone(), 40);
        ws.total_obtained += 40;
    }
    for (key, level) in [("WEP_KEYBLADE_SO_00", 2), ("WEP_KEYBLADE_SO_01", 0), ("WEP_KEYBLADE_SO_02", 10), ("WEP_KEYBLADE_SO_17", 0), ("WEP_KEYBLADE_SO_05", 2)] {
        bag.keyblades.insert(key.into(), level);
    }
    let milestones: Vec<u32> = t.photos.iter().take(4).filter_map(|p| if let Milestone::Flag(f) = p.done { Some(f) } else { None }).collect();
    let flags = move |f: u32| milestones.contains(&f);
    let visit = rules::visit(&mut ws, &mut bag, &flags);
    let reward = "WEP_DONALD_03";
    let (said, mut data, caption) = {
        let raw = |id: &str| view.raw(id);
        let help = |key: &str| view.art.help_id(key).map(|id| view.string(id)).unwrap_or_default();
        let abilities: Vec<(String, String)> = pack.data("abilities").ok().and_then(|v| v["rows"].as_array().cloned()).unwrap_or_default().iter().filter_map(|r| Some((r["id"].as_str()?.to_string(), r["name"].as_str()?.to_string()))).collect();
        let ability = |id: &str| abilities.iter().find(|a| a.0 == id).map_or_else(|| id.to_string(), |a| a.1.clone());
        (messages(&visit, &raw), build(&ws, &bag, &flags, &Lookup { raw: &raw, help: &help, ability: &ability }), help(reward))
    };
    data.title = "Roundtable Hold".into();
    data.munny = 48_210;
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/P6b/shots");
    std::fs::create_dir_all(&out).ok();
    let shot = |view: &WorkshopView, extra: Option<&kh_ui::Movie>, name: &str| {
        let mut canvas = kh_ui::Canvas::new(1920, 1080);
        let root = kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        let mut r = kh_ui::Renderer::new(fonts.clone());
        r.draw(&mut canvas, &view.movie, &root);
        if let Some(d) = view.dialog_movie() {
            r.draw(&mut canvas, d, &root);
        }
        if let Some(m) = extra {
            r.draw(&mut canvas, m, &root);
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.12, 0.13, 0.16], &mut rgba);
        kh_ui::def::write_png(&out.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
    };
    let run = |view: &mut WorkshopView, menu: &mut WorkshopMenu, n: usize| {
        for _ in 0..n {
            view.sync(menu);
            view.tick(Some(menu));
        }
    };
    let mut s = Vec::new();
    let mut menu = WorkshopMenu::new(data, Entry::Workshop);
    view.open(&mut menu);
    menu.push_messages(said.clone(), &mut s);
    run(&mut view, &mut menu, 40);
    shot(&view, None, "20_real_entry_dialog");
    // the merchants' line among the dialogs, if a purchase came with the visit
    if let Some(k) = said.iter().position(|m| m.text.contains("Merchants now sell")) {
        for _ in 0..k {
            menu.key(CampKey::Decide, false, &mut s);
        }
        run(&mut view, &mut menu, 20);
        shot(&view, None, "21_real_merchant_dialog");
    }
    while menu.message().is_some() {
        menu.key(CampKey::Decide, false, &mut s);
    }
    run(&mut view, &mut menu, 20);
    shot(&view, None, "22_real_top");
    menu.key(CampKey::Decide, false, &mut s);
    run(&mut view, &mut menu, 40);
    shot(&view, None, "23_real_synthesis");
    for (name, keys) in [("24_real_synthesis_weapons", vec![CampKey::R2, CampKey::R2]), ("25_real_synthesis_accessories", vec![CampKey::R2, CampKey::R2])] {
        for k in keys {
            menu.key(k, false, &mut s);
        }
        run(&mut view, &mut menu, 20);
        shot(&view, None, name);
    }
    menu.key(CampKey::Cancel, false, &mut s);
    run(&mut view, &mut menu, 30);
    for (screen, name) in [(Screen::Photo, "27_real_photo"), (Screen::Goals, "28_real_goals"), (Screen::Materials, "29_real_materials")] {
        while menu.data.commands.get(menu.top).map(|c| c.0) != Some(screen) {
            menu.key(CampKey::Down, false, &mut s);
        }
        menu.key(CampKey::Decide, false, &mut s);
        run(&mut view, &mut menu, 40);
        shot(&view, None, name);
        if screen == Screen::Goals {
            menu.key(CampKey::R1, false, &mut s);
            menu.key(CampKey::R1, false, &mut s);
            run(&mut view, &mut menu, 15);
            shot(&view, None, "28b_real_goals_page3");
        }
        menu.key(CampKey::Cancel, false, &mut s);
        run(&mut view, &mut menu, 30);
    }
    // KH3's large item-get popup after a synthesis (movie KeyItem, static SE 10)
    if let Ok(def) = pack.movie_linked(kh_ui::key_item::MOVIE, &kh_ui::camp_menu::LIBRARIES) {
        let mut popup = kh_ui::key_item::KeyItemPopup::new(def, fonts.clone());
        popup.show(kh_ui::key_item::Obtained { picture: view.art.icon(reward), name: item_name(reward).0, caption }, &mut s);
        for _ in 0..90 {
            popup.tick(&mut s);
        }
        assert!(s.contains(&UiSound::Play(SE_ITEM_GET)));
        shot(&view, Some(&popup.movie), "30_real_synthesis_result_popup");
    }
}

/// (the coordinator, 2026-11-03) A SMITH'S FORGE SHOWS ONLY THE FORGE: `enter(Entry::Forge)` moves the carried materials
/// into the stock (the Forge counts it) and says nothing: no deposit, goal or milestone dialog, no goal granted; the next
/// Moogle visit grants the goals with KH3's dialogs and tells the deposit made at the smith (dialog_0010 first).
#[test]
fn a_smith_s_forge_shows_only_the_forge() {
    let t = rules::tables();
    let mut ws = Workshop::default();
    for m in &t.materials {
        ws.obtained.insert(m.key.clone(), 999);
        ws.total_obtained += 999;
    }
    let mut bag = Bag::default();
    bag.items.insert("MAT_ITEM01".into(), 12);
    let photo = t.photos.iter().find(|p| matches!(p.done, Milestone::Flag(_))).unwrap();
    let Milestone::Flag(flag) = photo.done else { unreachable!() };
    let flags = move |f: u32| f == flag;
    let at_smith = enter(Entry::Forge, &mut ws, &mut bag, &flags);
    assert_eq!(at_smith, Visit::default(), "nothing to tell at a smith");
    assert!(messages(&at_smith, &raw).is_empty());
    assert_eq!((rules::stock_of(&ws, "MAT_ITEM01"), bag.carried("MAT_ITEM01")), (12, 0), "the Forge counts the stock");
    assert!(ws.goals.is_empty() && ws.photos.is_empty(), "goals and milestones wait for the Moogle");
    // the next Moogle visit: the deposit told, every goal and the milestone granted with their dialogs
    let at_moogle = enter(Entry::Workshop, &mut ws, &mut bag, &flags);
    let said = messages(&at_moogle, &raw);
    assert_eq!(said.first().map(|m| m.text.as_str()), Some(raw("kh3_menu.synthesis.dialog_0010").as_str()));
    assert_eq!((at_moogle.goals.len(), at_moogle.photos.clone()), (78, vec![photo.id.clone()]));
    assert_eq!(said.last().map(|m| m.text.as_str()), Some(raw("kh3_menu.synthesis.dialog_0050").as_str()));
    // told once (the bag emptied of what the goals handed over: no new deposit)
    bag.items.clear();
    assert!(messages(&enter(Entry::Workshop, &mut ws, &mut bag, &flags), &raw).iter().all(|m| m.text != raw("kh3_menu.synthesis.dialog_0010")));
}

/// (pictures, JOURNEY 2026-11-07) "JOURNEY TO THE ERDTREE" ON THE REAL ROWS AND PACK (skipped without the pack): a fresh save
/// (all 20 listed, none done), a mid-game save (Godrick .. Morgott and the Fire Giant beaten: done rows, the finale's count),
/// the Moogle's top (no Keyblade Forge) and a smith's Forge. build/sora_tmp/journey/shots/*.png.
#[test]
fn journey_pictures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    let Ok(mut pack) = kh_ui::Pack::open(&dir) else { return };
    let Ok(fonts) = kh_ui::Fonts::load(&mut pack) else { return };
    let Ok(mut view) = WorkshopView::load(&mut pack, fonts.clone()) else { return };
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/journey/shots");
    std::fs::create_dir_all(&out).ok();
    let shot = |view: &WorkshopView, name: &str| {
        let mut canvas = kh_ui::Canvas::new(1920, 1080);
        let root = kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        let mut r = kh_ui::Renderer::new(fonts.clone());
        r.draw(&mut canvas, &view.movie, &root);
        if let Some(d) = view.dialog_movie() {
            r.draw(&mut canvas, d, &root);
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.12, 0.13, 0.16], &mut rgba);
        kh_ui::def::write_png(&out.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
    };
    let run = |view: &mut WorkshopView, menu: &mut WorkshopMenu, n: usize| {
        for _ in 0..n {
            view.sync(menu);
            view.tick(Some(menu));
        }
    };
    let boss = |row: &str| {
        let json: serde_json::Value = serde_json::from_str(crate::bossrewards::SHEET).unwrap();
        json["rows"].as_array().unwrap().iter().find(|r| r[0].as_str() == Some(&format!("boss.{row}"))).and_then(|r| r[1]["flag"].as_u64()).unwrap() as u32
    };
    let mid: Vec<u32> = ["godrick", "rennala", "radahn", "mimic_tear", "rykard", "morgott", "fire_giant"].iter().map(|r| boss(r)).collect();
    for (save, beaten) in [("fresh", Vec::new()), ("midgame", mid)] {
        let mut ws = Workshop::default();
        let mut bag = Bag::default();
        bag.keyblades.insert("WEP_KEYBLADE_SO_00".into(), 2);
        let flags = move |f: u32| beaten.contains(&f);
        let visit = rules::visit(&mut ws, &mut bag, &flags);
        let (said, mut data) = {
            let raw = |id: &str| view.raw(id);
            let help = |_: &str| String::new();
            let ability = |id: &str| id.to_string();
            (messages(&visit, &raw), build(&ws, &bag, &flags, &Lookup { raw: &raw, help: &help, ability: &ability }))
        };
        data.title = "Roundtable Hold".into();
        assert_eq!(data.photos.len(), 20, "{save}: all 20 listed");
        assert!(data.commands.iter().all(|c| c.0 != Screen::Forge), "{save}: the Moogle has no Forge");
        let mut s = Vec::new();
        let mut menu = WorkshopMenu::new(data, Entry::Workshop);
        view.open(&mut menu);
        menu.push_messages(said.clone(), &mut s);
        run(&mut view, &mut menu, 40);
        if !said.is_empty() {
            assert!(said.iter().any(|m| m.text.starts_with("Journey milestone complete!")), "{save}: dialog_0050 renamed");
            shot(&view, &format!("{save}_0_dialog"));
        }
        while menu.message().is_some() {
            menu.key(CampKey::Decide, false, &mut s);
        }
        menu.key(CampKey::Down, false, &mut s);
        run(&mut view, &mut menu, 30);
        assert_eq!(view.movie.text("menuMixTop.menuObj.cmd2.mainCmdNameAnm.mainCmdName.textField").unwrap_or_default(), "Journey to the Erdtree");
        assert!(view.movie.node("menuMixTop.menuObj.cmd5").is_some_and(|n| !n.is_visible()), "four commands");
        shot(&view, &format!("{save}_1_top"));
        menu.key(CampKey::Decide, false, &mut s);
        run(&mut view, &mut menu, 40);
        assert_eq!(view.movie.text("menuPhoto.stageCategory.category.categoryName.txName.textField").unwrap_or_default(), "Journey to the Erdtree");
        shot(&view, &format!("{save}_2_journey_top"));
        for _ in 0..9 {
            menu.key(CampKey::Down, false, &mut s);
        }
        run(&mut view, &mut menu, 20);
        shot(&view, &format!("{save}_3_journey_middle"));
        for _ in 0..10 {
            menu.key(CampKey::Down, false, &mut s);
        }
        run(&mut view, &mut menu, 20);
        assert_eq!(menu.photo_row().map(|r| r.id.as_str()), Some("IM_6"), "{save}: the finale last");
        shot(&view, &format!("{save}_4_journey_finale"));
        view.close();
        for _ in 0..120 {
            if view.tick(Some(&mut menu)) {
                break;
            }
        }
    }
    // a smith still opens the Forge
    let mut ws = Workshop::default();
    let mut bag = Bag::default();
    bag.keyblades.insert("WEP_KEYBLADE_SO_00".into(), 2);
    let data = {
        let raw = |id: &str| view.raw(id);
        let help = |_: &str| String::new();
        let ability = |id: &str| id.to_string();
        build(&ws, &bag, &no_flags, &Lookup { raw: &raw, help: &help, ability: &ability })
    };
    enter(Entry::Forge, &mut ws, &mut bag, &no_flags);
    let mut menu = WorkshopMenu::new(WorkshopData { title: "Smithing Master Hewg".into(), ..data }, Entry::Forge);
    view.open(&mut menu);
    run(&mut view, &mut menu, 40);
    assert_eq!(menu.screen, Screen::Forge);
    shot(&view, "smith_forge");
}

/// (KH3TEXT, THE USER 2026-11-08: "keep radhan's milestone text what it was") The Journey list shows Radahn's line as the
/// user approved it, made at run time from KH3's mission_0080 in the player's pack (nothing compiled in); without the pack
/// it shows nothing. Skipped without build/sora_pack.
#[test]
fn radahns_milestone_shows_the_packs_line() {
    let ws = Workshop::default();
    let bag = Bag::default();
    let missing = |id: &str| texts::missing(id); // (the view's lookup when the pack lacks an id: the id)
    let none = build(&ws, &bag, &no_flags, &Lookup { raw: &missing, help: &|_| String::new(), ability: &|id| format!("[{id}]") });
    assert_eq!(none.photos.iter().find(|p| p.id == "IM_17").map(|p| p.text.as_str()), Some(""), "the pack lacks it: nothing");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui/strings/en/kh3_menu.json");
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let line = json["namespaces"]["kh3_menu.shop"]["mission_0080"].as_str().unwrap_or("").to_string();
    let pack = move |id: &str| if id == "kh3_menu.shop.mission_0080" { line.clone() } else { id.to_string() };
    let lookup = Lookup { raw: &pack, help: &|_| String::new(), ability: &|id| format!("[{id}]") };
    let data = build(&ws, &bag, &no_flags, &lookup);
    assert_eq!(data.photos.iter().find(|p| p.id == "IM_17").map(|p| p.text.as_str()), Some("Wish I could ditch work and see the festival, kupo!"));
}

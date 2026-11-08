//! (LEADER RESET) A NEW CHARACTER / A NEW LEADER REBUILDS THE COMMAND MENU AND THE LEADER'S HUD PIECES FROM SCRATCH (THE USER
//! 2026-11-02, with a picture of Donald's menu reading "Attack / Magicunement / (empty) / Items" after he died as Goofy and loaded
//! a Donald save: "Make it so this kind of thing never happens loading from any character after dying as another character").
//!
//! The real HUD (kh-host `Hud` + the UI pack, skipped without it) fed as the Elden Ring host feeds it (kh-er partyui
//! `hud_frame`: the party slots, the leader, his own command snapshot and pieces every frame, also while the game-over scene
//! or the title holds the HUD's frame), for every pair of leaders Sora / Donald / Goofy, each way, through:
//! - death -> KH3's game-over scene -> Load (the in-session load: kh-er hud.rs `SESSION_LOAD`, `end_gameover`);
//! - quit to title -> the title's Load screen -> load (the title's frames run the HUD on an empty snapshot);
//! - a plain leader swap (the party menu's), for comparison.
//!
//! After each the command menu must read exactly as a HUD that started with the new leader: the same rows, texts, places and
//! labels, every shown row named, no two rows on one line. Pictures: build/sora_tmp/party/LEADER-RESET/shots/.

use kh_core::input::PadButton;
use kh_core::{PadButtons, Sora, TargetId, UiSnapshot, Vec3};
use kh_host::gameover::GameOverSetup;
use kh_host::hud::{Frame, Hud, HudTargets, TargetView, View};
use kh_host::party::{GoofyHudView, GuardLook, GuardValues, PartyChara, PartyHudSlot};
use kh_party::donald::{DonLearned, StatusBook};
use kh_party::pad::menu::{AllyRow, DonFeed, FriendMenu};
use kh_party::{Chara, FriendStatus, MemberId};
use kh_partyhost::custom::{don_view_of, gof_look, DonHudFeed};

const MAIN: &str = "cmdMenu.cmdMainTop.cmdMain";

/// The command menu's drawn main rows: (row 1 ..= 4, its text, its y x10).
type Rows = Vec<(usize, String, i32)>;
const SETTLE: usize = 150;

struct Dummy;
impl HudTargets for Dummy {
    fn target(&self, _: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn out_dir() -> std::path::PathBuf {
    let d = root().join("build/sora_tmp/party/LEADER-RESET/shots");
    std::fs::create_dir_all(&d).ok();
    d
}

fn tag(c: PartyChara) -> &'static str {
    match c {
        PartyChara::Sora => "sora",
        PartyChara::Donald => "donald",
        _ => "goofy",
    }
}

/// What feeds the HUD this frame: a character's world (his menu, his pieces) or the title's (no world: an empty snapshot).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Feed {
    Leader,
    Title,
}

struct Rig {
    hud: Hud,
    fonts: kh_ui::Fonts,
    sora: Sora,
    leader: PartyChara,
    /// The leader's own menu (a friend's: kh-party `FriendMenu`; a load makes a new one, as the loaded party does).
    menu: FriendMenu,
    don: FriendStatus,
    gof: FriendStatus,
    held: PadButtons,
    state: kh_core::StateId,
    feed: Feed,
}

impl Rig {
    fn new(leader: PartyChara) -> Option<Rig> {
        let Ok((hud, fonts)) = Hud::load(&root().join("build/sora_pack/ui")) else {
            eprintln!("no UI pack: skipped");
            return None;
        };
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
        let mut don = FriendStatus::new(Chara::Donald, 0);
        don.donald.as_mut().unwrap().learned = DonLearned::all();
        don.mp = don.max_mp() * 8 / 10;
        let gof = FriendStatus::new(Chara::Goofy, 0);
        let mut rig = Rig { hud, fonts, sora, leader, menu: FriendMenu::default(), don, gof, held: PadButtons::NONE, state: kh_core::StateId::Idle, feed: Feed::Leader };
        rig.lead(leader);
        let f = kh_host::formation::Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], leader).unwrap();
        rig.hud.set_formation(&f);
        Some(rig)
    }

    /// A new leader as a load (or a swap) brings him: his own menu, new.
    fn lead(&mut self, leader: PartyChara) {
        self.leader = leader;
        let mut menu = FriendMenu::default();
        if leader == PartyChara::Donald {
            menu.don.feed = DonFeed {
                on: true,
                members: vec![AllyRow { id: MemberId(0), name: "Donald", hp: 140, hp_max: 200 }, AllyRow { id: MemberId(1), name: "Goofy", hp: 160, hp_max: 220 }, AllyRow { id: MemberId(7), name: "Sora", hp: 120, hp_max: 160 }],
                ..DonFeed::default()
            };
        }
        self.menu = menu;
        if let Ok(f) = kh_host::formation::Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], leader) {
            self.hud.set_formation(&f);
        }
    }

    /// `n` frames, the first with `buttons` pressed: the host's feed (kh-er partyui `hud_frame`), then the HUD's frame.
    fn frames(&mut self, buttons: &[PadButton], n: usize) {
        for k in 0..n {
            let now = if k == 0 { buttons.iter().fold(PadButtons::NONE, |p, b| p.with(*b)) } else { PadButtons::NONE };
            let pad = kh_core::input::PadState::from_held(now, self.held);
            self.held = now;
            let mut snapshot = if self.feed == Feed::Title { UiSnapshot::default() } else { self.sora.ui_snapshot() };
            if self.feed == Feed::Leader {
                let slots = [
                    PartyHudSlot { chara: PartyChara::Donald, hp: 140, hp_max: 200, mp: 80, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: false, focus: None },
                    PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: false, focus: None },
                ];
                self.hud.set_party(&slots);
                let (guard, goofy, don) = match self.leader {
                    PartyChara::Goofy => {
                        let _ = self.menu.tick(Chara::Goofy, &self.gof, &pad, false);
                        snapshot.command = self.menu.snapshot(Chara::Goofy, &self.gof);
                        let p = kh_party::guard::GP;
                        let look = GuardLook { fill: p.hud_color_fill, shine: p.hud_color_shine, loss: p.hud_color_loss, regen: p.hud_color_regen, broken: p.hud_color_broken, blink_hz: p.hud_broken_blink, offset: (p.hud_offset_x, p.hud_offset_y) };
                        (Some((GuardValues { value: 110.0, max: 150.0, broken: false, regenerating: false, rearm_at: 0.5 }, look)), Some(GoofyHudView { look: gof_look(), ..GoofyHudView::default() }), None)
                    }
                    PartyChara::Donald => {
                        let _ = self.menu.tick(Chara::Donald, &self.don, &pad, false);
                        snapshot.command = self.menu.snapshot(Chara::Donald, &self.don);
                        let st = self.don.donald.as_ref().unwrap();
                        let hud = kh_party::donald::hud::DonHud::with(st, &StatusBook::default(), None, None, &kh_party::donald::data());
                        let feed = DonHudFeed { hud, menu: self.menu.don_view(Chara::Donald, &self.don) };
                        (None, None, Some(don_view_of(&feed, &[])))
                    }
                    _ => {
                        // the main slots as a running Sora shows them (a fresh core outside the world has no names: "? ? ?")
                        for (row, (kind, name)) in snapshot.command.main.iter_mut().zip([(1u8, "Attack"), (28, "Magic"), (57, "Items"), (0, "Link")]) {
                            (row.kind, row.name, row.enabled) = (kind, name, kind != 0);
                        }
                        (None, None, None)
                    }
                };
                self.hud.set_guard(guard);
                self.hud.set_goofy(goofy);
                self.hud.set_don(don);
                self.hud.set_leader(self.leader, true);
            }
            snapshot.step = self.sora.ui_snapshot().step + k as u64 + 1;
            let view = View { clip_from_world: glam::Mat4::orthographic_rh(0.0, 1920.0, 1080.0, 0.0, -1.0, 1.0), size: (1920.0, 1080.0) };
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state: self.state, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &Dummy, to_view: same, view: Some(view), words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            self.hud.apply_actions(&mut self.sora);
        }
    }

    /// The command menu's main rows as they stand: (row 1 ..= 4, its text, its y) of every row that draws.
    fn rows(&self) -> Rows {
        let m = self.hud.movie("HudCommand").unwrap();
        (1..=4)
            .filter_map(|n| {
                let path = format!("{MAIN}.cmdMain{n}");
                let node = m.node(&path)?;
                let drawn = node.is_visible() && !matches!(m.current_label(&path), Some("off" | "out"));
                let text = m.text(&format!("{path}.cmdText1.txCmdName.txCmdNameCol.coCmdName.textField")).unwrap_or_default();
                drawn.then(|| (n, text, (node.matrix.ty * 10.0).round() as i32))
            })
            .collect()
    }

    /// The stage with the HUD over a mid-tone floor, written as `name` (and `name_cmd`, the command menu x2).
    fn picture(&self, name: &str) -> Vec<u8> {
        let (w, h) = (1920usize, 1080usize);
        let mut canvas = kh_ui::Canvas::new(w, h);
        let mut r = kh_ui::Renderer::new(self.fonts.clone());
        for mv in ["HudTarget", "HudRight", "HudCommand", "HudStatus"] {
            let (root, _) = kh_host::hud_layers::stage_root(w as f32, h as f32);
            r.draw(&mut canvas, self.hud.movie(mv).unwrap(), &root);
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, y| { let t = y as f32 / 1080.0; [0.30 + 0.08 * t, 0.27 + 0.05 * t, 0.24] }, &mut rgba);
        let _ = kh_ui::def::write_png(&out_dir().join(format!("{name}.png")), w as u32, h as u32, &rgba);
        let (x0, y0, cw, ch, z) = (0usize, 700usize, 560usize, 380usize, 2usize);
        let mut out = vec![0u8; cw * ch * z * z * 4];
        for y in 0..ch * z {
            for x in 0..cw * z {
                let i = ((y0 + y / z) * w + x0 + x / z) * 4;
                let o = (y * cw * z + x) * 4;
                out[o..o + 4].copy_from_slice(&rgba[i..i + 4]);
            }
        }
        let _ = kh_ui::def::write_png(&out_dir().join(format!("{name}_cmd.png")), (cw * z) as u32, (ch * z) as u32, &out);
        rgba
    }

    /// THE DEATH: the leader's Die state, KH3's game-over scene (kh-er hud.rs `HudHost::frame`: `open_gameover`), its frames
    /// (the HUD's frame is the scene's: the command menu is not driven), then the Load: the loaded save's party is fed while
    /// the plate is still black (the new leader, his new menu), the session's load clears the popups (`SESSION_LOAD`), and
    /// the scene ends once the new character has the picture (`end_gameover`).
    fn die_and_load(&mut self, to: PartyChara) {
        self.state = kh_core::StateId::Die;
        assert!(self.hud.open_gameover(GameOverSetup::default()));
        self.frames(&[], 240);
        // Load chosen: KH3's Load screen over the game-over menu, a slot decided
        let slots = [kh_ui::saveload::SlotSummary { index: 0, state: kh_ui::saveload::SlotState::Used, name: "save".into(), level: 10, ..Default::default() }];
        let _ = self.hud.open_load(&slots);
        self.frames(&[], 30);
        self.hud.close_saveload();
        // the game loads: the new character (a new Sora / party, the loaded leader), the old session's popups cleared
        self.sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
        self.state = kh_core::StateId::Idle;
        self.lead(to);
        // (kh-er hud.rs) the session's load (`SESSION_LOAD`) and the new character's HUD half (`HudHost::new`)
        self.hud.reset_popups();
        self.hud.new_character();
        self.frames(&[], 60);
        self.hud.end_gameover(true);
        let mut n = 0;
        while self.hud.gameover_active() && n < 600 {
            self.frames(&[], 1);
            n += 1;
        }
        assert!(!self.hud.gameover_active(), "the scene ended");
        self.frames(&[], SETTLE);
    }

    /// QUIT TO TITLE, LOAD: the world goes (no feed of the party; the title's frames run the HUD on an empty snapshot, kh-er
    /// hud.rs `title_load_frame`, the Load screen up), a slot is loaded, the title ends (`title_end`), the loaded character's
    /// party is fed.
    fn title_and_load(&mut self, to: PartyChara) {
        self.feed = Feed::Title;
        self.sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
        self.frames(&[], 60);
        let slots = [kh_ui::saveload::SlotSummary { index: 0, state: kh_ui::saveload::SlotState::Used, name: "save".into(), level: 10, ..Default::default() }];
        self.hud.saveload_background = true;
        let _ = self.hud.open_load(&slots);
        self.frames(&[], 60);
        self.hud.close_saveload();
        self.frames(&[], 20);
        self.hud.close_newgame_choice();
        self.hud.new_character(); // (kh-er hud.rs `title_end`)
        self.hud.saveload_background = false;
        self.feed = Feed::Leader;
        self.sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
        self.lead(to);
        // (kh-er hud.rs) the loaded character's HUD half (`HudHost::new`)
        self.hud.new_character();
        self.frames(&[], SETTLE);
    }

    /// A plain swap (the party menu's leader change): the next frames feed the new leader.
    fn swap(&mut self, to: PartyChara) {
        self.lead(to);
        self.frames(&[], SETTLE);
    }
}

/// A HUD that started with `leader`: the reference of his menu.
fn reference(leader: PartyChara) -> Option<Rows> {
    let mut rig = Rig::new(leader)?;
    rig.frames(&[], 60 + SETTLE);
    rig.picture(&format!("00_ref_{}", tag(leader)));
    Some(rig.rows())
}

/// The rows read right: as the reference, each named, no two on a line.
fn check(rig: &Rig, want: &[(usize, String, i32)], what: &str) {
    let got = rig.rows();
    assert_eq!(got, want, "{what}: the command menu's rows (row, text, y x10) are the new leader's own");
    for (n, text, _) in &got {
        assert!(!text.is_empty(), "{what}: row {n} draws with no name");
    }
    let mut ys: Vec<i32> = got.iter().map(|r| r.2).collect();
    ys.sort_unstable();
    ys.dedup();
    assert_eq!(ys.len(), got.len(), "{what}: two rows on one line: {got:?}");
}

/// The per-leader pieces: Goofy's GUARD bar and his Resolve / Stagger / Danger Sense pieces only while he leads.
fn check_pieces(rig: &Rig, leader: PartyChara, what: &str) {
    let goofy = leader == PartyChara::Goofy;
    assert_eq!(rig.hud.guard_bar_state().1, goofy, "{what}: Goofy's GUARD bar");
    assert_eq!(rig.hud.gof_state().0.shown, goofy, "{what}: Goofy's Resolve pips");
    assert!(rig.hud.gof_state().2.showing().is_empty(), "{what}: no Danger Sense mark left");
    if !goofy {
        assert!(!rig.hud.movie("HudStatus").unwrap().node("sttsPc.gofPipPlate").is_some_and(|n| n.is_visible()), "{what}: the pip plate is gone");
    }
    if leader != PartyChara::Donald {
        assert!(rig.hud.don_badges().is_empty(), "{what}: no Resonance badge of Donald's left");
    }
}

const ALL: [PartyChara; 3] = [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy];

fn run(route: &str, go: fn(&mut Rig, PartyChara)) {
    let Some(_) = Rig::new(PartyChara::Sora) else { return };
    let refs: Vec<(PartyChara, Rows)> = ALL.iter().map(|c| (*c, reference(*c).unwrap())).collect();
    let want = |c: PartyChara| refs.iter().find(|r| r.0 == c).unwrap().1.clone();
    // the references themselves: Sora Attack / Magic / Items (/ Link), Donald Attack / Magic / Attunement / Items, Goofy Attack / Items
    let names = |c: PartyChara| want(c).into_iter().map(|r| r.1).collect::<Vec<_>>();
    assert_eq!(&names(PartyChara::Donald), &["Attack", "Magic", "Attunement", "Items"], "Donald's menu");
    assert_eq!(&names(PartyChara::Goofy), &["Attack", "Items"], "Goofy's menu");
    assert!(names(PartyChara::Sora).starts_with(&["Attack".to_string(), "Magic".to_string(), "Items".to_string()]), "Sora's menu: {:?}", names(PartyChara::Sora));
    for from in ALL {
        for to in ALL {
            if from == to {
                continue;
            }
            let mut rig = Rig::new(from).unwrap();
            rig.frames(&[], 60 + SETTLE);
            check(&rig, &want(from), &format!("{route}: {} before", tag(from)));
            go(&mut rig, to);
            let what = format!("{route}: {} -> {}", tag(from), tag(to));
            rig.picture(&format!("{route}_{}_to_{}", tag(from), tag(to)));
            check(&rig, &want(to), &what);
            check_pieces(&rig, to, &what);
            // the cursor walks the new menu: down from Attack lands on the second row of HIS menu
            if to != PartyChara::Sora {
                rig.frames(&[PadButton::DPadDown], 20);
                assert_eq!(rig.hud.command_cursor() as usize + 1, want(to)[1].0, "{what}: the cursor's second stop is his second row");
                rig.frames(&[PadButton::DPadUp], 20);
            }
        }
    }
}

#[test]
fn death_then_load_from_the_game_over_menu_gives_the_new_leaders_menu_exactly() {
    run("death", Rig::die_and_load);
}

#[test]
fn quit_to_title_then_load_gives_the_new_leaders_menu_exactly() {
    run("title", Rig::title_and_load);
}

#[test]
fn a_plain_leader_swap_gives_the_new_leaders_menu_exactly() {
    run("swap", Rig::swap);
}

/// THE USER'S CASE (2026-11-02): Goofy leads, Goofy dies, a Donald save is loaded from the game-over menu; and then Donald dies
/// and a Goofy save comes back, and again: the menu is right every time, with no row of an earlier menu left.
#[test]
fn goofy_dies_a_donald_save_loads_and_back_again() {
    let Some(mut rig) = Rig::new(PartyChara::Goofy) else { return };
    let (don, gof) = (reference(PartyChara::Donald).unwrap(), reference(PartyChara::Goofy).unwrap());
    rig.frames(&[], 60 + SETTLE);
    for round in 0..3 {
        rig.die_and_load(PartyChara::Donald);
        rig.picture(&format!("user_case_{round}_goofy_died_donald_loaded"));
        check(&rig, &don, &format!("round {round}: Goofy died, Donald loaded"));
        check_pieces(&rig, PartyChara::Donald, "Donald loaded");
        rig.die_and_load(PartyChara::Goofy);
        check(&rig, &gof, &format!("round {round}: Donald died, Goofy loaded"));
        check_pieces(&rig, PartyChara::Goofy, "Goofy loaded");
    }
}

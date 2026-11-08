//! (ALLY-BUFF) DONALD'S HELPFUL SPELLS ON WHOEVER HAS THEM, WHOEVER LEADS, OFFLINE (the user, 2026-11-03: "Donald's helpful
//! spells like Aero shield should appear on whoever has it in the HUD regardless of who is party leader. Positioning
//! should all match Donald."): the real HUD (kh-host `Hud` + the UI pack, skipped without it) fed as both hosts feed it
//! (`Hud::set_ally_buffs` from kh-partyhost's `PartyRuntime::ally_buffs_view`). Pictures:
//! build/sora_tmp/party/ALLY-BUFF/shots/ (the whole stage and a x2 close-up of the status HUD, `_status`).
//!
//! - Donald leading (the reference: his place since 2026-10-22), Sora leading, Goofy leading (his GUARD bar and Resolve
//!   pips up): the leader's icon at the same place of the leader's slot, a friend's at the same place of his slot;
//! - several at once (a worn spell and Sanctuary's invincibility): the same column on every slot, the worn spell first;
//! - the runtime's own feed: the members' worn spells and Sanctuary, whoever leads, Sora included.

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView, View};
use kh_host::party::{AllyBuff, AllyBuffsView, GoofyHudView, GuardValues, PartyChara, PartyHudSlot};
use kh_party::donald::allyfx::{Barrier, Orbs};
use kh_party::donald::attune::Slot;
use kh_party::donald::cast::SORA_MEMBER;
use kh_party::donald::{DonLearned, Element, StatusBook};
use kh_party::{Chara, FriendStatus};
use kh_partyhost::custom::{ally_buffs_of, don_view_of, gof_look, DonHudFeed};
use kh_partyhost::{PartyRuntime, PartyTest, RuntimeConfig};

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
    let d = root().join("build/sora_tmp/party/ALLY-BUFF/shots");
    std::fs::create_dir_all(&d).ok();
    d
}

fn floor(y: usize) -> [f32; 3] {
    let t = y as f32 / 1080.0;
    [0.30 + 0.08 * t, 0.27 + 0.05 * t, 0.24]
}

const AERO: &str = "Aero";
const THUNDER: &str = "Thunder";
/// Light's icon (Sanctuary's invincibility).
const LIGHT: &str = "Ability";

struct Rig {
    hud: Hud,
    fonts: kh_ui::Fonts,
    sora: Sora,
    leader: PartyChara,
    don: FriendStatus,
    buffs: Vec<(PartyChara, &'static str)>,
}

impl Rig {
    fn new(leader: PartyChara) -> Option<Rig> {
        let Ok((mut hud, fonts)) = Hud::load(&root().join("build/sora_pack/ui")) else {
            eprintln!("no UI pack: skipped");
            return None;
        };
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
        let mut don = FriendStatus::new(Chara::Donald, 0);
        don.donald.as_mut().unwrap().learned = DonLearned::all();
        // Donald's own HUD as he plays it: attuned to Fire (his badge and the Arcane Flow row up, the reference cluster)
        don.donald.as_mut().unwrap().core.attune.slots = [Some(Slot { element: Element::Fire, res: 2.0, period: 1, ult_used: false, full_at: None }), None];
        let f = kh_host::formation::Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], leader).unwrap();
        hud.set_formation(&f);
        let slots = [
            PartyHudSlot { chara: PartyChara::Donald, hp: 140, hp_max: 200, mp: 80, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
            PartyHudSlot { chara: PartyChara::Goofy, hp: 300, hp_max: 376, mp: 60, mp_max: 136, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
        ];
        hud.set_party(&slots);
        hud.set_leader(leader, true);
        let mut rig = Rig { hud, fonts, sora, leader, don, buffs: Vec::new() };
        rig.frames(60);
        Some(rig)
    }

    fn view(&self) -> AllyBuffsView {
        AllyBuffsView { buffs: self.buffs.iter().map(|(chara, icon)| AllyBuff { chara: *chara, icon: icon.to_string(), left: 10.0, total: 20.0 }).collect(), look: None }
    }

    fn frames(&mut self, n: usize) {
        self.frames_with(n, None);
    }

    /// `n` frames; `feed` = the runtime's own view (else the rig's list).
    fn frames_with(&mut self, n: usize, feed: Option<&AllyBuffsView>) {
        for k in 0..n {
            let mut snapshot = self.sora.ui_snapshot();
            snapshot.step += k as u64 + 1;
            // each leader's own pieces, as the hosts feed them
            let don = (self.leader == PartyChara::Donald).then(|| {
                let st = self.don.donald.as_ref().unwrap();
                let hud = kh_party::donald::hud::DonHud::with(st, &StatusBook::default(), None, None, &kh_party::donald::data());
                don_view_of(&DonHudFeed { hud, menu: None }, &[])
            });
            self.hud.set_don(don);
            let goofy = self.leader == PartyChara::Goofy;
            let p = kh_party::guard::GP;
            let look = kh_host::party::GuardLook { fill: p.hud_color_fill, shine: p.hud_color_shine, loss: p.hud_color_loss, regen: p.hud_color_regen, broken: p.hud_color_broken, blink_hz: p.hud_broken_blink, offset: (p.hud_offset_x, p.hud_offset_y) };
            self.hud.set_guard(goofy.then_some((GuardValues { value: 110.0, max: 150.0, broken: false, regenerating: false, rearm_at: 0.5 }, look)));
            self.hud.set_goofy(goofy.then(|| GoofyHudView { look: gof_look(), ..GoofyHudView::default() }));
            let buffs = feed.cloned().unwrap_or_else(|| self.view());
            self.hud.set_ally_buffs(buffs);
            let view = View { clip_from_world: glam::Mat4::orthographic_rh(0.0, 1920.0, 1080.0, 0.0, -1.0, 1.0), size: (1920.0, 1080.0) };
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &Dummy, to_view: same, view: Some(view), words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
        }
    }

    /// The stage over a mid-tone floor (`name`.png) and a x2 close-up of the status HUD (`name`_status.png).
    fn picture(&self, name: &str) {
        let (w, h) = (1920usize, 1080usize);
        let mut canvas = kh_ui::Canvas::new(w, h);
        let mut r = kh_ui::Renderer::new(self.fonts.clone());
        for mv in ["HudTarget", "HudRight", "HudCommand", "HudStatus"] {
            let (root, _) = kh_host::hud_layers::stage_root(w as f32, h as f32);
            r.draw(&mut canvas, self.hud.movie(mv).unwrap(), &root);
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, y| floor(y), &mut rgba);
        let _ = kh_ui::def::write_png(&out_dir().join(format!("{name}.png")), w as u32, h as u32, &rgba);
        let ((x0, y0, cw, ch), z) = ((1440usize, 820usize, 480usize, 260usize), 2usize);
        let mut out = vec![0u8; cw * ch * z * z * 4];
        for y in 0..ch * z {
            for x in 0..cw * z {
                let i = ((y0 + y / z) * w + x0 + x / z) * 4;
                let o = (y * cw * z + x) * 4;
                out[o..o + 4].copy_from_slice(&rgba[i..i + 4]);
            }
        }
        let _ = kh_ui::def::write_png(&out_dir().join(format!("{name}_status.png")), (cw * z) as u32, (ch * z) as u32, &out);
    }

    /// The icons drawn, each as (slot, index, icon, x, y, size) with x / y IN ITS SLOT'S OWN UNITS (a friend's: from his
    /// part's origin; the leader's: `sttsPc`), the units the placing rule is written in.
    fn icons(&self) -> Vec<(usize, usize, String, f32, f32, f32)> {
        let m = self.hud.movie("HudStatus").unwrap();
        let at = |p: &str| m.node(p).map(|n| (n.matrix.tx, n.matrix.ty)).unwrap();
        let pc = at("sttsPc");
        self.hud
            .ally_buff_icons()
            .iter()
            .map(|(slot, k, icon, x, y, size)| {
                let (dx, dy) = if *slot == 0 {
                    (0.0, 0.0)
                } else {
                    let s = at(kh_ui::hud_buffs::SLOT_PARTS[*slot]);
                    (s.0 - pc.0, s.1 - pc.1)
                };
                (*slot, *k, icon.clone(), x - dx, y - dy, *size)
            })
            .collect()
    }

    /// The pixels the icon `k` of slot `slot` draws (over black, against the frame without it).
    fn pixels(&self, slot: usize, k: usize) -> f32 {
        self.mask(&[format!("sttsPc.allyBuff{slot}_{k}")], &[]).iter().filter(|p| **p).count() as f32
    }

    /// Which stage pixels the parts `paths` of the status HUD draw (over black, against the frame without them), with the
    /// parts `base` hidden in both.
    fn mask(&self, paths: &[String], base: &[String]) -> Vec<bool> {
        let draw = |m: &kh_ui::Movie| {
            let mut canvas = kh_ui::Canvas::new(1920, 1080);
            let (root, _) = kh_host::hud_layers::stage_root(1920.0, 1080.0);
            kh_ui::Renderer::new(self.fonts.clone()).draw(&mut canvas, m, &root);
            let mut rgba = Vec::new();
            canvas.to_rgba8_over(|_, _| [0.0, 0.0, 0.0], &mut rgba);
            rgba
        };
        let mut m = self.hud.movie("HudStatus").unwrap().clone();
        for p in base {
            assert!(m.set_visible(p, false), "{p}");
        }
        let full = draw(&m);
        let mut hidden = m.clone();
        for p in paths {
            assert!(hidden.set_visible(p, false), "{p}");
        }
        let without = draw(&hidden);
        full.as_chunks::<4>().0.iter().zip(without.as_chunks::<4>().0).map(|(p, q)| p.iter().zip(q.iter()).take(3).any(|(x, y)| x.abs_diff(*y) > 24)).collect()
    }

    /// The pixels the leader's icons share with his own pieces (Goofy's GUARD bar and Resolve pips; Sora's focus gauge):
    /// 0 = clear.
    fn leader_overlap(&self) -> usize {
        let icons: Vec<String> = self.hud.ally_buff_icons().iter().filter(|i| i.0 == 0).map(|i| format!("sttsPc.allyBuff0_{}", i.1)).collect();
        let rows: Vec<String> = match self.leader {
            PartyChara::Goofy => ["sttsPc.pcGuard", "sttsPc.gofPipPlate"].iter().map(|s| s.to_string()).chain((0..3).map(|i| format!("sttsPc.gofPipBase{i}"))).collect(),
            _ => vec!["sttsPc.pcEx".to_string()],
        };
        let (a, b) = (self.mask(&icons, &rows), self.mask(&rows, &icons));
        a.iter().zip(&b).filter(|(x, y)| **x && **y).count()
    }
}

/// The seat of a character in the rig's HUD.
fn seat(rig: &Rig, c: PartyChara) -> usize {
    rig.hud.seat_of(c).unwrap()
}

/// The same place and size (a friend's place goes through his part's origin and back: float rounding).
fn near(a: (f32, f32, f32), b: (f32, f32, f32)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01 && (a.2 - b.2).abs() < 0.01
}

/// DONALD'S PLACE ON EVERY LEADER: Donald leading is the reference; with Sora and with Goofy leading, the leader's icon and
/// each friend's sit exactly where Donald's HUD puts them (same place of the slot, same size), and every icon draws.
#[test]
fn every_leader_shows_the_icons_at_donalds_place() {
    let Some(mut don) = Rig::new(PartyChara::Donald) else { return };
    let look = kh_ui::hud_resonance::ResonanceLayout::default().buff_look();
    // THE REFERENCE: Donald leading, a barrier on himself and on Goofy, Ball Lightning on Sora
    don.buffs = vec![(PartyChara::Donald, AERO), (PartyChara::Goofy, AERO), (PartyChara::Sora, THUNDER)];
    don.frames(10);
    don.picture("01_donald_leads_reference");
    let reference = don.icons();
    assert_eq!(reference.len(), 3, "{reference:?}");
    for (slot, k, _, x, y, size) in &reference {
        assert!(near((*x, *y, *size), look.place(*slot, *k)), "Donald's own place on slot {slot}: ({x}, {y}, {size})");
        assert!(don.pixels(*slot, *k) > 40.0, "the icon draws on slot {slot}");
    }
    // the same place by seat: every leader, every friend slot
    for leader in [PartyChara::Sora, PartyChara::Goofy] {
        let Some(mut rig) = Rig::new(leader) else { return };
        rig.buffs = vec![(PartyChara::Sora, AERO), (PartyChara::Donald, AERO), (PartyChara::Goofy, THUNDER)];
        rig.frames(10);
        rig.picture(&format!("02_{}_leads_all_three", leader.name().to_lowercase()));
        let got = rig.icons();
        assert_eq!(got.len(), 3, "{leader:?}: {got:?}");
        let lift = rig.hud.ally_buff_room().lift;
        for (slot, k, icon, x, y, size) in &got {
            let r = reference.iter().find(|r| r.0 == *slot && r.1 == *k).unwrap();
            // the leader's: Donald's place raised by the lift (none but Goofy's)
            let dy = if *slot == 0 { lift } else { 0.0 };
            assert!(near((*x, *y, *size), (r.3, r.4 + dy, r.5)), "{leader:?} slot {slot}: ({x}, {y}, {size}) vs Donald's {r:?}; {:?}", rig.hud.ally_buff_room());
            assert!(rig.pixels(*slot, *k) > 40.0, "{leader:?}: the {icon} icon draws on slot {slot}");
        }
        // each on his own seat
        for (c, icon) in &rig.buffs {
            assert!(got.iter().any(|g| g.0 == seat(&rig, *c) && g.2 == *icon), "{leader:?}: {c:?}'s {icon} by his own portrait");
        }
        // Sora: Donald's very place (his focus gauge beside it, clear); Goofy: the same column raised over his GUARD bar and
        // Resolve pips, clear of them
        let room = rig.hud.ally_buff_room();
        if leader == PartyChara::Goofy {
            assert!(room.lift < 0.0, "raised over his rows");
        } else {
            assert_eq!(room.lift, 0.0);
            assert_eq!(room.beside.len(), 1, "Sora's focus gauge is a piece beside the column: {room:?}");
        }
        assert_eq!(rig.leader_overlap(), 0, "{leader:?}: not a pixel on his own pieces");
    }
    assert_eq!(*don.hud.ally_buff_room(), kh_ui::hud_buffs::LeaderRoom::default(), "Donald's own: all of his place");
}

/// THE USER'S CASES: Sora leading with Aero Barrier on himself and on Goofy (through the runtime's own feed); Goofy leading
/// with it on Sora; the icon goes with the status.
#[test]
fn sora_and_goofy_leading_show_the_barrier_where_it_is() {
    // the runtime: Donald and Goofy in the party, barriers on Sora and Goofy (a second, spent Ball Lightning set on
    // Donald: no icon)
    let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
    rt.party = kh_party::Party::new(kh_party::PartyConfig { level99: false, total_exp: 0, seed: 1 });
    rt.spawn_one(PartyChara::Donald, Vec3::new(200.0, 0.0, 0.0), 0.0);
    rt.spawn_one(PartyChara::Goofy, Vec3::ZERO, 0.0);
    let (don, gof) = (rt.id(PartyChara::Donald).unwrap(), rt.id(PartyChara::Goofy).unwrap());
    let barrier = |member| Barrier { member, caster: don, level: 2, left: 12.0, total: 30.0, cut: 0.4, contact: None, touched: Vec::new(), effect: None };
    rt.party.don_fx.worn.barriers = vec![barrier(SORA_MEMBER), barrier(gof)];
    rt.party.don_fx.worn.orbs = vec![Orbs { member: don, caster: don, level: 1, orbs: 0, max: 3, left: 3.0, total: 5.0, ball_radius: 50.0, power: 1.0, spin: 0.0, recharge_wait: 0.0, chain: false, balls: Vec::new() }];
    // no leader: Sora leads
    let v = rt.ally_buffs_view();
    let who: Vec<(PartyChara, &str)> = v.buffs.iter().map(|b| (b.chara, b.icon.as_str())).collect();
    assert_eq!(who, [(PartyChara::Sora, AERO), (PartyChara::Goofy, AERO)], "the spent set shows nothing");
    assert!(v.buffs.iter().all(|b| b.left == 12.0 && b.total == 30.0));
    assert!(v.look.is_some(), "Donald's look goes with it");
    // Goofy leading: the very same view (it does not depend on who leads)
    rt.leader = Some(gof);
    assert_eq!(rt.ally_buffs_view(), v, "the same whoever leads");
    rt.leader = None;
    let Some(mut rig) = Rig::new(PartyChara::Sora) else { return };
    rig.frames_with(10, Some(&v));
    rig.picture("03_sora_leads_barrier_on_sora_and_goofy");
    let got = rig.icons();
    assert_eq!(got.iter().map(|g| (g.0, g.2.as_str())).collect::<Vec<_>>(), [(0, AERO), (seat(&rig, PartyChara::Goofy), AERO)], "Sora's in the leader's slot, Goofy's by his portrait");
    // Goofy leading, the barrier on Sora only (Sora in a friend slot now)
    let Some(mut rig) = Rig::new(PartyChara::Goofy) else { return };
    rig.buffs = vec![(PartyChara::Sora, AERO)];
    rig.frames(10);
    rig.picture("04_goofy_leads_barrier_on_sora");
    let got = rig.icons();
    let sora_seat = seat(&rig, PartyChara::Sora);
    assert!(sora_seat != 0);
    assert_eq!(got.iter().map(|g| (g.0, g.2.as_str())).collect::<Vec<_>>(), [(sora_seat, AERO)]);
    assert!(rig.pixels(sora_seat, 0) > 40.0);
    // it ends: the icon goes
    rig.buffs.clear();
    rig.frames(2);
    assert!(rig.icons().is_empty());
    assert!(!rig.hud.movie("HudStatus").unwrap().node(&format!("sttsPc.allyBuff{sora_seat}_0")).is_some_and(|n| n.is_visible()), "hidden once it ends");
}

/// SEVERAL AT ONCE: a worn spell and Sanctuary's invincibility on one member stack in one column, the worn spell at
/// Donald's place and the next one above it, the same on the leader's slot and a friend's, whoever leads.
#[test]
fn several_statuses_stack_the_same_on_every_slot() {
    let look = kh_ui::hud_resonance::ResonanceLayout::default().buff_look();
    for leader in [PartyChara::Donald, PartyChara::Sora, PartyChara::Goofy] {
        let Some(mut rig) = Rig::new(leader) else { return };
        let friends: Vec<PartyChara> = [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy].into_iter().filter(|c| *c != leader).collect();
        rig.buffs = vec![(leader, AERO), (leader, LIGHT), (friends[0], THUNDER), (friends[0], LIGHT), (friends[1], LIGHT)];
        rig.frames(10);
        rig.picture(&format!("05_{}_leads_several_at_once", leader.name().to_lowercase()));
        let got = rig.icons();
        assert_eq!(got.len(), 5, "{leader:?}: {got:?}");
        let room = rig.hud.ally_buff_room().clone();
        for (slot, k, icon, x, y, size) in &got {
            let want = if *slot == 0 { look.leader_place(*k, &room) } else { look.place(*slot, *k) };
            assert!(near((*x, *y, *size), want), "{leader:?}: {icon} at the rule's place on slot {slot}: ({x}, {y})");
            assert!(rig.pixels(*slot, *k) > 40.0, "{leader:?}: {icon} draws on slot {slot}");
        }
        // the first at Donald's place (Goofy's raised over his rows); the column straight up from it but where a piece
        // of the leader's own stands beside it (Sora's focus gauge: the second slides left of its tip)
        let first = got.iter().find(|g| g.0 == 0 && g.1 == 0).unwrap();
        let p = look.place(0, 0);
        assert!(near((first.3, first.4, first.5), (p.0, p.1 + room.lift, p.2)), "{leader:?}: {first:?}");
        let second = got.iter().find(|g| g.0 == 0 && g.1 == 1).unwrap();
        if leader == PartyChara::Sora {
            assert!(second.3 < first.3, "{leader:?}: the second slid left of his focus gauge's tip: {second:?}");
        } else {
            assert!((second.3 - first.3).abs() < 0.01, "{leader:?}: straight above: {second:?}");
        }
        // the order given: the worn spell first (at Donald's place), Sanctuary above it
        let order = |slot: usize| got.iter().filter(|g| g.0 == slot).map(|g| g.2.clone()).collect::<Vec<_>>();
        assert_eq!(order(0), [AERO, LIGHT]);
        assert_eq!(order(seat(&rig, friends[0])), [THUNDER, LIGHT]);
        assert_eq!(order(seat(&rig, friends[1])), [LIGHT], "a lone Sanctuary sits at Donald's place");
        if leader != PartyChara::Donald {
            assert_eq!(rig.leader_overlap(), 0, "{leader:?}: the column clear of his own pieces");
        }
    }
}

/// The runtime's per-member list: the worn spell first (Aero Barrier, else an active Ball Lightning), then Sanctuary's
/// invincibility while it lasts; nothing once they end.
#[test]
fn a_members_buffs_are_the_worn_spell_then_sanctuary() {
    let mut party = kh_party::Party::new(kh_party::PartyConfig { level99: false, total_exp: 0, seed: 1 });
    let m = kh_party::MemberId(2);
    assert!(ally_buffs_of(&party, m).is_empty());
    party.don_fx.worn.orbs = vec![Orbs { member: m, caster: m, level: 2, orbs: 2, max: 4, left: 8.0, total: 12.0, ball_radius: 50.0, power: 1.0, spin: 0.0, recharge_wait: 0.0, chain: false, balls: Vec::new() }];
    party.don_fx.worn.grant_invincible(m, 1.2);
    let got = ally_buffs_of(&party, m);
    assert_eq!(got.iter().map(|g| g.0).collect::<Vec<_>>(), [THUNDER, LIGHT]);
    assert_eq!((got[0].1, got[0].2), (8.0, 12.0));
    assert!(got[1].1 == 1.2 && got[1].2 >= 1.2, "Sanctuary's seconds left of its whole: {:?}", got[1]);
    // the barrier comes instead of the balls (one worn spell at a time is allyfx's rule; the barrier wins the icon)
    party.don_fx.worn.barriers = vec![Barrier { member: m, caster: m, level: 3, left: 30.0, total: 40.0, cut: 0.4, contact: None, touched: Vec::new(), effect: None }];
    assert_eq!(ally_buffs_of(&party, m)[0].0, AERO);
    party.don_fx.worn = Default::default();
    assert!(ally_buffs_of(&party, m).is_empty(), "nothing once they end");
}

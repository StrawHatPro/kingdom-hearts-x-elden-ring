//! (RC, the user 2026-10-20: "fix the Y icon for the Y commands not being properly positioned, they should be centered on
//! the Y axis, the mp bar for friends' HUD doesn't fill all the way like it should fix that. 100% mp should mean a full mp
//! bar for them. Make every Y interaction you do in the game use the same HUD element that Save now uses")
//! THE OFFLINE HARNESS of stream RC, real Hud + UI pack (skipped without it). Pictures: build/sora_tmp/party/rc/.
//! - the Y icon of the event button and of a selected situation command is centred on its bar (KH3's inline-image rule:
//!   `<img src='img://icontext:%s' vspace='%d'>`, IconTextData 30 x 30, VSpace -7, on the $sougei 23 line);
//! - several Y interactions at once: the first in the green event button, the others stacked as situation rows;
//! - the friends' MP arc: 100 % MP = the clip's last frame (361 of 361), for every leader arrangement.

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, HostReaction, Hud, HudTargets, TargetView};
use kh_host::hud_layers::{draw_movie_in, stage_root, Draw, Drawn, Perf};
use kh_host::party::{PartyChara, PartyHudSlot};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

const EV: &str = "cmdMenu.cmdActEvTop.cmdActEvShort";
const EV_TEXT: &str = "cmdMenu.cmdActEvTop.cmdActEvShort.cmdActEv.cmdActEvText.cmdText1.txCmdName.txCmdNameCol.coCmdName";

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn out_dir() -> std::path::PathBuf {
    let d = root().join("build/sora_tmp/party/rc");
    std::fs::create_dir_all(&d).ok();
    d
}

/// A movie of the HUD over a dark floor (1920 x 1080 RGBA).
fn picture(fonts: &kh_ui::Fonts, movie: &kh_ui::Movie) -> Vec<u8> {
    let (stage, view) = stage_root(1920.0, 1080.0);
    let mut draw = Draw::default();
    let mut renderer = kh_ui::render::Renderer::new(fonts.clone());
    let rect = match draw_movie_in(&mut draw, &mut renderer, movie, &stage, (1920.0, 1080.0), true, Perf::default(), Some(view)) {
        Drawn::Picture(rect, _) => rect,
        _ => panic!("the movie drew nothing"),
    };
    let mut out = vec![0u8; 1920 * 1080 * 4];
    for (i, o) in out.chunks_exact_mut(4).enumerate() {
        let y = (i / 1920) as f32 / 1080.0;
        o.copy_from_slice(&[(60.0 + 40.0 * y) as u8, (30.0 + 10.0 * y) as u8, 22, 255]);
    }
    for y in 0..rect.3 {
        for x in 0..rect.2 {
            let (fx, fy) = (rect.0 + x as i32, rect.1 + y as i32);
            if (0..1920).contains(&fx) && (0..1080).contains(&fy) {
                let d = (fy as usize * 1920 + fx as usize) * 4;
                let s = (y * rect.2 + x) * 4;
                let a = draw.rgba[s + 3] as f32 / 255.0;
                for c in 0..3 {
                    out[d + c] = (draw.rgba[s + c] as f32 * a + out[d + c] as f32 * (1.0 - a)).round() as u8;
                }
            }
        }
    }
    out
}

fn save(name: &str, w: usize, h: usize, rgba: &[u8]) {
    kh_ui::def::write_png(&out_dir().join(name), w as u32, h as u32, rgba).unwrap();
}

/// A crop (x, y, w, h) of a 1920-wide picture, zoomed `z` times.
fn crop(src: &[u8], sw: usize, ch: usize, (x0, y0, w, h): (usize, usize, usize, usize), z: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * z * h * z * 4];
    for y in 0..h * z {
        for x in 0..w * z {
            let s = ((y0 + y / z) * sw + x0 + x / z) * ch;
            let d = (y * w * z + x) * 4;
            out[d..d + 3].copy_from_slice(&src[s..s + 3]);
            out[d + 3] = 255;
        }
    }
    out
}

fn side_by_side(a: &[u8], b: &[u8], w: usize, h: usize) -> Vec<u8> {
    let ww = w * 2 + 8;
    let mut out = vec![255u8; ww * h * 4];
    for y in 0..h {
        out[(y * ww) * 4..(y * ww + w) * 4].copy_from_slice(&a[y * w * 4..(y + 1) * w * 4]);
        out[(y * ww + w + 8) * 4..(y * ww + w + 8 + w) * 4].copy_from_slice(&b[y * w * 4..(y + 1) * w * 4]);
    }
    out
}

/// The vertical middle (in px) of the pixels of `pred` inside the box, and their count.
fn middle(src: &[u8], sw: usize, ch: usize, (x0, y0, x1, y1): (usize, usize, usize, usize), pred: impl Fn(&[u8]) -> bool) -> Option<(f32, usize, usize, usize)> {
    let (mut top, mut bottom, mut n) = (usize::MAX, 0usize, 0usize);
    for y in y0..y1 {
        for x in x0..x1 {
            let s = (y * sw + x) * ch;
            if pred(&src[s..s + 3]) {
                top = top.min(y);
                bottom = bottom.max(y);
                n += 1;
            }
        }
    }
    (n > 0).then(|| ((top + bottom) as f32 / 2.0, n, top, bottom))
}

fn green(p: &[u8]) -> bool {
    p[1] as i32 > p[0] as i32 + 40 && p[1] as i32 > p[2] as i32 + 40
}

/// The Xbox Y glyph's yellow.
fn yellow(p: &[u8]) -> bool {
    p[0] > 150 && p[1] > 120 && (p[2] as i32) < 110 && p[0] as i32 - p[2] as i32 > 90 && (p[1] as i32) < p[0] as i32 + 30
}

fn step(hud: &mut Hud, sora: &Sora, snapshot: &kh_core::UiSnapshot, n: usize, words: &[String]) {
    for k in 0..n {
        let w: &[String] = if k == 0 { words } else { &[] };
        hud.frame(&mut Frame { snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

fn sora() -> Sora {
    Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)))
}

fn snapshot(sora: &Sora) -> kh_core::UiSnapshot {
    let mut snapshot = sora.ui_snapshot();
    for (row, (kind, name)) in snapshot.command.main.iter_mut().zip([(1u8, "Attack"), (28, "Magic"), (57, "Items"), (0, "Link")]) {
        (row.kind, row.name, row.enabled) = (kind, name, kind != 0);
    }
    snapshot
}

#[test]
fn rc_the_y_icon_sits_in_the_middle_of_every_y_bar_and_interactions_stack() {
    let Ok((mut hud, fonts)) = Hud::load(&root().join("build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let sora = sora();
    let mut snap = snapshot(&sora);
    step(&mut hud, &sora, &snap, 120, &[]);
    // ---- 1. the event button (Save): the icon's middle on the bar's middle
    hud.set_host_command(Some(Hud::SAVE_COMMAND));
    step(&mut hud, &sora, &snap, 40, &[]);
    let m = hud.movie("HudCommand").unwrap();
    assert_eq!(m.text(EV_TEXT).as_deref(), Some("Save"));
    let shown = picture(&fonts, m);
    save("y_1_save.png", 1920, 1080, &shown);
    let bar = middle(&shown, 1920, 4, (150, 840, 300, 940), green).expect("the green bar");
    let icon = middle(&shown, 1920, 4, (0, 840, 120, 940), yellow).expect("the Y icon");
    eprintln!("ours: Save bar rows {}..{} (middle {:.1}), Y icon rows {}..{} (middle {:.1}, {} px)", bar.2, bar.3, bar.0, icon.2, icon.3, icon.0, icon.1);
    assert!((icon.0 - bar.0).abs() <= 1.5, "the Y icon's middle {:.1} is the bar's middle {:.1}", icon.0, bar.0);
    // beside KH3's own picture (the user's reference of the save prompt, 1920 x 1080)
    if let Ok((rw, _, buf)) = kh_ui::def::read_png(&root().join("build/sora_tmp/party/ui2/ref/kh3_savepoint_enter_and_prompt.png")) {
        let ch = buf.len() / (rw as usize * 1080).max(1);
        // (the reference is KH3's Monstropolis skin: its world art above the bar defeats a colour count, so it is shown, not measured)
        let a = crop(&buf, rw as usize, ch, (0, 840, 400, 100), 3);
        let b = crop(&shown, 1920, 4, (0, 840, 400, 100), 3);
        save("y_icon_vs_kh3_reference.png", 1200 * 2 + 8, 300, &side_by_side(&a, &b, 1200, 300));
    }
    // ---- 2. a selected situation command (Second Form, with its timer) under the Save: the same rule on its own bar
    snap.command.situations = vec![kh_core::command::SituationRow { serial: 7, kind: 0, name: "Second Form", icon_type: 1, total: 20.0, remaining: 5.0, enabled: true, refreshed: 0 }];
    snap.command.situation_selected = Some(0);
    hud.set_host_command(None);
    step(&mut hud, &sora, &snap, 60, &[]);
    let m = hud.movie("HudCommand").unwrap();
    let sit = picture(&fonts, m);
    save("y_2_situation_selected.png", 1920, 1080, &sit);
    if let Some(icon) = middle(&sit, 1920, 4, (0, 700, 180, 1000), yellow) {
        // the situation bar's own rows: the dark bar around the icon (its row band, read from the icon's columns' neighbours)
        eprintln!("situation: Y icon rows {}..{} (middle {:.1})", icon.2, icon.3, icon.0);
        let sit_bar = middle(&sit, 1920, 4, (200, icon.2.saturating_sub(20), 330, icon.3 + 20), |p| p[0] as i32 + p[1] as i32 + p[2] as i32 > 120 && !(p[0] > 50 && p[0] < 110 && p[1] < 45 && p[2] < 30));
        if let Some(b) = sit_bar {
            eprintln!("situation bar rows {}..{} (middle {:.1}); icon offset {:+.1}", b.2, b.3, b.0, icon.0 - b.0);
        }
    }
    let a = crop(&sit, 1920, 4, (0, 760, 400, 200), 3);
    save("y_2_situation_zoom.png", 1200, 600, &a);
    snap.command.situations.clear();
    snap.command.situation_selected = None;
    // ---- 3. EVERY Y INTERACTION ON THE SAME ELEMENT, several at once: Save first (Y), then Talk (a party member's), then
    // Elden Ring's "Pick up item" as KH3's "Pick Up", stacked
    hud.set_host_reactions(&[HostReaction::kind(Hud::SAVE_COMMAND), HostReaction::kind(Hud::TALK_COMMAND), HostReaction::named("Pick Up")]);
    step(&mut hud, &sora, &snap, 60, &[]);
    assert_eq!(hud.host_reaction_names(), vec!["Save".to_string(), "Talk".to_string(), "Pick Up".to_string()]);
    let m = hud.movie("HudCommand").unwrap();
    assert_eq!(m.text(EV_TEXT).as_deref(), Some("Save"), "the first is the green event button");
    let stack = picture(&fonts, m);
    save("y_3_stack_save_talk_pickup.png", 1920, 1080, &stack);
    save("y_3_stack_zoom.png", 1200, 900, &crop(&stack, 1920, 4, (0, 680, 400, 300), 3));
    // the single interactions as the game offers them: Talk (an NPC's or Donald's), Open, Pick Up, Climb
    let mut singles = Vec::new();
    for r in [HostReaction::kind(Hud::TALK_COMMAND), HostReaction::kind(Hud::OPEN_COMMAND), HostReaction::named("Pick Up"), HostReaction::named("Climb")] {
        let name = if r.kind == Some(77) { "Talk" } else if r.kind == Some(78) { "Open" } else { r.name.as_str() }.to_string();
        hud.set_host_reactions(&[r]);
        step(&mut hud, &sora, &snap, 60, &[]);
        let m = hud.movie("HudCommand").unwrap();
        assert_eq!(m.text(EV_TEXT).as_deref(), Some(name.as_str()));
        assert!(m.node(EV).is_some_and(|n| n.is_visible()));
        let p = picture(&fonts, m);
        let g = middle(&p, 1920, 4, (150, 840, 300, 940), green).expect("green bar");
        let i = middle(&p, 1920, 4, (0, 840, 120, 940), yellow).expect("Y icon");
        assert!((i.0 - g.0).abs() <= 1.5, "{name}: icon {:.1} bar {:.1}", i.0, g.0);
        singles.push(crop(&p, 1920, 4, (0, 850, 400, 80), 2));
    }
    // one sheet: the four, top to bottom
    let mut sheet = Vec::new();
    for s in &singles {
        sheet.extend_from_slice(s);
    }
    save("y_4_talk_open_pickup_climb.png", 800, 160 * singles.len(), &sheet);
    // ---- 4. Donald's recruit talk with Goofy leading (every leader: the one command model)
    hud.set_party(&[PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: false, focus: None }]);
    hud.set_leader(PartyChara::Goofy, false);
    hud.set_host_reactions(&[HostReaction::kind(Hud::TALK_COMMAND), HostReaction::kind(Hud::TALK_COMMAND)]);
    step(&mut hud, &sora, &snap, 150, &[]);
    let m = hud.movie("HudCommand").unwrap();
    assert_eq!(m.text(EV_TEXT).as_deref(), Some("Talk"), "Donald's talk on the green bar, the crone's under it");
    save("y_5_donald_talk_goofy_leads.png", 1920, 1080, &picture(&fonts, m));
    // gone: KH3's out
    hud.set_host_reactions(&[]);
    step(&mut hud, &sora, &snap, 60, &[]);
    assert!(matches!(hud.movie("HudCommand").unwrap().current_label(EV), Some("out" | "off")));
}

#[test]
fn rc_a_friends_mp_arc_is_full_at_full_mp_for_every_leader() {
    let Ok((mut hud, fonts)) = Hud::load(&root().join("build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let sora = sora();
    let snap = snapshot(&sora);
    // Sora's own MP full as well (he sits in a friend slot when a friend leads)
    let words = vec!["status maxmp 120".to_string(), "status mp 120".to_string()];
    let full = |chara| PartyHudSlot { chara, hp: 200, hp_max: 200, mp: 100, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: false, focus: None };
    let half = |chara| PartyHudSlot { mp: 50, ..full(chara) };
    let mp_clip = |i: usize| format!("sttsNpc{i}.sttsNpc.npcMp");
    let mut shots = Vec::new();
    for (name, leader) in [("sora", PartyChara::Sora), ("donald", PartyChara::Donald), ("goofy", PartyChara::Goofy)] {
        for (fill, slots) in [("full", [full(PartyChara::Donald), full(PartyChara::Goofy)]), ("half", [half(PartyChara::Donald), half(PartyChara::Goofy)])] {
            hud.set_party(&slots);
            hud.set_leader(leader, true);
            step(&mut hud, &sora, &snap, 150, &words);
            let m = hud.movie("HudStatus").unwrap();
            for i in 1..=2 {
                let frame = m.current_frame(&mp_clip(i));
                eprintln!("{name} leads, {fill}: friend slot {i} MP clip frame {frame:?} of 361");
                if fill == "full" {
                    assert_eq!(frame, Some(361), "{name} leads: friend slot {i} at 100 % MP is the clip's last frame (the whole arc)");
                } else if leader == PartyChara::Sora {
                    assert_eq!(frame, Some(181), "{name} leads: 50 % MP = half the arc");
                }
            }
            let p = picture(&fonts, m);
            save(&format!("mp_{name}_leads_{fill}.png"), 1920, 1080, &p);
            if name == "sora" {
                shots.push(crop(&p, 1920, 4, (1800, 880, 120, 200), 4));
            }
        }
    }
    // full beside half (Sora leading): the zoomed friends' slots
    if shots.len() == 2 {
        save("mp_friends_full_vs_half.png", 480 * 2 + 8, 800, &side_by_side(&shots[0], &shots[1], 480, 800));
    }
}

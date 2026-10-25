//! (SP2, the user 2026-10-20: "There also needs to be a proper prompt for using the save point when inside it, currently
//! there is none. (The prompt needs to look like the second picture I sent, but using the appropriate visuals ...)")
//! THE SAVE POINT'S PROMPT = KH3's reaction command "Save" (command kind 76, `kh3_system.situation.save`) in the command
//! menu's event button (the green bar over Attack / Magic / Items / Link with the Y icon), from the default HudCommand
//! movie (the cockpit's own, no world skin), through `Hud::set_host_command`, whoever leads. Gone with KH3's "out".
//! Pictures: build/sora_tmp/party/sp2/shots/prompt_*.png (+ prompt_vs_reference.png beside the user's picture).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
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

/// The command menu over a dark floor-like background (1920 x 1080 RGBA).
fn picture(fonts: &kh_ui::Fonts, movie: &kh_ui::Movie) -> Vec<u8> {
    let (stage, view) = stage_root(1920.0, 1080.0);
    let mut draw = Draw::default();
    let mut renderer = kh_ui::render::Renderer::new(fonts.clone());
    let rect = match draw_movie_in(&mut draw, &mut renderer, movie, &stage, (1920.0, 1080.0), true, Perf::default(), Some(view)) {
        Drawn::Picture(rect, _) => rect,
        _ => panic!("the command menu drew nothing"),
    };
    // the picture of its rectangle (x, y, w, h) into the whole frame
    let mut layer = vec![0u8; 1920 * 1080 * 4];
    for y in 0..rect.3 {
        for x in 0..rect.2 {
            let (fx, fy) = (rect.0 + x as i32, rect.1 + y as i32);
            if (0..1920).contains(&fx) && (0..1080).contains(&fy) {
                let d = (fy as usize * 1920 + fx as usize) * 4;
                let s = (y * rect.2 + x) * 4;
                layer[d..d + 4].copy_from_slice(&draw.rgba[s..s + 4]);
            }
        }
    }
    // straight-alpha over a dark red-brown floor (the reference's room)
    let mut out = vec![0u8; 1920 * 1080 * 4];
    for (i, (o, p)) in out.chunks_exact_mut(4).zip(layer.chunks_exact(4)).enumerate() {
        let y = (i / 1920) as f32 / 1080.0;
        let bg = [60.0 + 40.0 * y, 30.0 + 10.0 * y, 22.0];
        let a = p[3] as f32 / 255.0;
        for c in 0..3 {
            o[c] = (p[c] as f32 * a + bg[c] * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
        }
        o[3] = 255;
    }
    out
}

fn save(name: &str, w: usize, h: usize, rgba: &[u8]) {
    let dir = root().join("build/sora_tmp/party/sp2/shots");
    std::fs::create_dir_all(&dir).ok();
    kh_ui::def::write_png(&dir.join(name), w as u32, h as u32, rgba).unwrap();
}

#[test]
fn the_save_point_prompt_is_kh3s_save_reaction_command_for_every_leader() {
    let Ok((mut hud, fonts)) = Hud::load(&root().join("build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let mut snapshot = sora.ui_snapshot();
    // the main slots as a running Sora shows them (a fresh core outside the world has no names: "? ? ?")
    for (row, (kind, name)) in snapshot.command.main.iter_mut().zip([(1u8, "Attack"), (28, "Magic"), (57, "Items"), (0, "Link")]) {
        (row.kind, row.name, row.enabled) = (kind, name, kind != 0);
    }
    let step = |hud: &mut Hud, n: usize| {
        for _ in 0..n {
            hud.frame(&mut Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
        }
    };
    step(&mut hud, 120);
    let before = picture(&fonts, hud.movie("HudCommand").unwrap());
    save("prompt_0_none.png", 1920, 1080, &before);
    // he steps in: the host offers its command
    hud.set_host_command(Some(Hud::SAVE_COMMAND));
    step(&mut hud, 40);
    let m = hud.movie("HudCommand").unwrap();
    assert_eq!(m.text(EV_TEXT).as_deref(), Some("Save"), "KH3's own name of command 76 (kh3_system.situation.save)");
    assert!(m.node(EV).is_some_and(|n| n.is_visible()), "the event button is up");
    let shown = picture(&fonts, m);
    save("prompt_1_save.png", 1920, 1080, &shown);
    // what it adds: a green bar over the command menu, bottom left
    let (mut x0, mut y0, mut x1, mut y1, mut n, mut green) = (1920usize, 1080usize, 0usize, 0usize, 0usize, 0usize);
    for (i, (a, b)) in before.chunks_exact(4).zip(shown.chunks_exact(4)).enumerate() {
        if a.iter().zip(b).take(3).any(|(p, q)| p.abs_diff(*q) > 24) {
            let (x, y) = (i % 1920, i / 1920);
            (x0, y0, x1, y1, n) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y), n + 1);
            if b[1] as i32 > b[0] as i32 + 40 && b[1] as i32 > b[2] as i32 + 40 {
                green += 1;
            }
        }
    }
    eprintln!("the Save command changed {n} px in x {x0}..{x1}, y {y0}..{y1}; {green} of them green");
    assert!(n > 4000 && green > n / 4, "a green bar ({green} green of {n})");
    assert!(x1 < 960 && y0 > 540, "bottom left, over the command menu (x {x0}..{x1}, y {y0}..{y1})");
    // beside the reference (its prompt region: bottom left of a 1920 x 1080 frame)
    let reference = root().join("build/sora_tmp/party/ui2/ref/kh3_savepoint_enter_and_prompt.png");
    if let Ok((rw, _, buf)) = kh_ui::def::read_png(&reference) {
        let (rw, ch) = (rw as usize, 4usize);
        let (cx, cy, cw, chh) = (0usize, 820usize, 480usize, 240usize);
        let w = cw * 2 + 8;
        let mut out = vec![255u8; w * chh * 4];
        for y in 0..chh {
            for x in 0..cw {
                let s = ((cy + y) * rw + cx + x) * ch;
                let d = (y * w + x) * 4;
                out[d..d + 3].copy_from_slice(&buf[s..s + 3]);
                out[d + 3] = 255;
                let s2 = ((cy + y) * 1920 + cx + x) * 4;
                let d2 = (y * w + cw + 8 + x) * 4;
                out[d2..d2 + 4].copy_from_slice(&shown[s2..s2 + 4]);
            }
        }
        save("prompt_vs_reference.png", w, chh, &out);
    }
    // a friend leads (Goofy, Sora out of the party): the same command in the same place
    hud.set_party(&[PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: false, focus: None }]);
    hud.set_leader(PartyChara::Goofy, false);
    step(&mut hud, 150);
    let m = hud.movie("HudCommand").unwrap();
    assert_eq!(m.text(EV_TEXT).as_deref(), Some("Save"));
    assert!(m.node(EV).is_some_and(|n| n.is_visible()));
    save("prompt_2_goofy_leads.png", 1920, 1080, &picture(&fonts, m));
    // he leaves (or combat hides the save point): KH3's "out", the bar goes
    hud.set_host_command(None);
    step(&mut hud, 60);
    let m = hud.movie("HudCommand").unwrap();
    assert!(matches!(m.current_label(EV), Some("out" | "off")), "taken off with KH3's out (then off): {:?}", m.current_label(EV));
    let after = picture(&fonts, m);
    let green_left = after.chunks_exact(4).enumerate().filter(|(i, p)| i % 1920 < 960 && i / 1920 > 540 && p[1] as i32 > p[0] as i32 + 60 && p[1] as i32 > p[2] as i32 + 60).count();
    assert!(green_left < green / 10, "no green bar left ({green_left} px)");
    save("prompt_3_gone.png", 1920, 1080, &after);
}

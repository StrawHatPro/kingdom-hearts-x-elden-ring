//! (GOOFY-GG) THE GUARD BAR, OFFLINE: the status HUD drawn through the Elden Ring host's layer path
//! (`kh_host::hud_layers::draw_movie_in`, as hb_gauge_check) with Sora leading and with Goofy leading in the GUARD
//! states (full, draining, broken, regen), laid over a mid scene, cropped to the HUD.
//!
//!   cargo run -p kh-host --release --example gg_guard_hud -- OUT_DIR [tree]

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::{draw_movie_in, stage_root, Draw, Drawn, Perf};
use kh_host::party::{GuardLook, GuardValues, PartyChara, PartyHudSlot};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn walk(s: &kh_ui::player::Sprite, path: &str, filter: &str) {
    for n in s.children.iter().chain(&s.extra) {
        let p = format!("{path}.{}", n.name.as_deref().map(str::to_string).unwrap_or_else(|| format!("#{}", n.ch)));
        let vis = if n.is_visible() { "" } else { " HIDDEN" };
        let cx = if n.cx.mul != [1.0; 4] || n.cx.add != [0.0; 4] { format!(" cx mul {:?} add {:?}", n.cx.mul, n.cx.add) } else { String::new() };
        let kind = match &n.kind {
            kh_ui::player::Kind::Shape(_) => "shape".to_string(),
            kh_ui::player::Kind::Image(_) => "image".to_string(),
            kh_ui::player::Kind::Sprite(c) => format!("sprite {} f{}/{} gfx {}", c.def.class.as_deref().unwrap_or("-"), c.frame, c.def.frames, c.gfx.len()),
            kh_ui::player::Kind::Text(_) => "text".to_string(),
            _ => "other".to_string(),
        };
        if p.contains(filter) {
            let m = n.matrix;
            println!("{p} | {kind} | m [{:.3} {:.3} {:.3} {:.3} {:.1} {:.1}] blend {} clip {}{cx}{vis}", m.a, m.b, m.c, m.d, m.tx, m.ty, n.blend, n.clip);
        }
        if let kh_ui::player::Kind::Sprite(c) = &n.kind {
            walk(c, &p, filter);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = args.first().map(std::path::PathBuf::from).unwrap_or_else(|| root.join("build/sora_tmp/party/gg/shots"));
    let which = args.get(1).cloned().unwrap_or_default();
    std::fs::create_dir_all(&out).unwrap();
    let (mut hud, fonts) = Hud::load(&root.join("build/sora_pack/ui")).expect("the UI pack");
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let mut snapshot = sora.ui_snapshot();
    let g = &mut snapshot.gauges;
    g.hp = g.hp_max * 7 / 10;
    g.mp = g.mp_max * 6 / 10;
    g.focus = g.focus_max / 2;
    let goofy = |focus: Option<(i32, i32)>| PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus };
    let donald = PartyHudSlot { chara: PartyChara::Donald, hp: 140, hp_max: 200, mp: 70, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None };
    let step = |hud: &mut Hud, n: usize| {
        for _ in 0..n {
            hud.frame(&mut Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
        }
    };
    let shot = |hud: &Hud, name: &str| {
        let movie = hud.movie("HudStatus").expect("HudStatus");
        let (w, h) = (1920usize, 1080usize);
        let (stage, view) = stage_root(w as f32, h as f32);
        let mut draw = Draw::default();
        let mut renderer = kh_ui::render::Renderer::new(fonts.clone());
        let Drawn::Picture(rect, _) = draw_movie_in(&mut draw, &mut renderer, movie, &stage, (w as f32, h as f32), true, Perf::default(), Some(view)) else { panic!("nothing drawn") };
        let bg = [0.38f32, 0.36, 0.32];
        let z = 2;
        let (zw, zh) = (rect.2 * z, rect.3 * z);
        let mut big = vec![0u8; zw * zh * 4];
        for y in 0..zh {
            for x in 0..zw {
                let p = &draw.rgba[((y / z) * rect.2 + x / z) * 4..][..4];
                let a = p[3] as f32 / 255.0;
                let o = (y * zw + x) * 4;
                for c in 0..3 {
                    big[o + c] = ((p[c] as f32 / 255.0 * a + bg[c] * (1.0 - a)) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
                }
                big[o + 3] = 255;
            }
        }
        let _ = kh_ui::def::write_png(&out.join(format!("{name}.png")), zw as u32, zh as u32, &big);
        println!("{name}: rect {rect:?}");
    };
    if which == "menu" {
        // the camp menu's Stats page of a Goofy start (formation [Goofy]): the third gauge row reads GUARD
        let downs: usize = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(4);
        let f = kh_host::formation::Formation::new(&[PartyChara::Goofy], PartyChara::Goofy).unwrap();
        hud.set_formation(&f);
        hud.set_leader(PartyChara::Goofy, false);
        let m = kh_host::party::PartyMenuMember { chara: PartyChara::Goofy, level: 20, exp: 28_841, exp_level: 28_841, exp_next: 33_000, hp: 160, hp_max: 220, mp: 60, mp_max: 100, ap_max: 20, ap_in_use: 0, strength: 20, magic: 0, defense: 25, resists: [0; 6], weapon: "WEP_GOOFY_00".into(), armour: vec![None], accessories: vec![None], items: vec![None], abilities: Vec::new(), ai: Default::default(), custom: None, custom_costs: Vec::new(), kit: Vec::new(), guard: Some((84, 120)), item_reload: vec![false], item_spent: vec![None] /* (EQ) */ };
        hud.set_party_menu(vec![m]);
        let words_step = |hud: &mut Hud, w: &[&str], n: usize| {
            for k in 0..n {
                let words: Vec<String> = if k == 0 { w.iter().map(|s| s.to_string()).collect() } else { Vec::new() };
                hud.frame(&mut Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &words, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            }
        };
        words_step(&mut hud, &[], 30);
        words_step(&mut hud, &["menu open"], 80);
        for _ in 0..downs {
            words_step(&mut hud, &["menu down"], 12);
        }
        words_step(&mut hud, &["menu decide"], 60);
        words_step(&mut hud, &["menu decide"], 80);
        let movie = hud.movie("CampMenu").expect("CampMenu");
        let (w, h) = (1920usize, 1080usize);
        let (root, _) = stage_root(w as f32, h as f32);
        let mut canvas = kh_ui::Canvas::new(w, h);
        kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, movie, &root);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.1, 0.1, 0.12], &mut rgba);
        let _ = kh_ui::def::write_png(&out.join(format!("menu_down{downs}.png")), w as u32, h as u32, &rgba);
        println!("menu: {downs} downs written");
        return;
    }
    if which == "start" || which == "start-noguard" {
        // (GOOFY-GG ER fix) A GOOFY START as Elden Ring runs it: formation [Goofy], Sora out, Donald not in the party
        let f = kh_host::formation::Formation::new(&[PartyChara::Goofy], PartyChara::Goofy).unwrap();
        hud.set_formation(&f);
        hud.set_party(&[goofy(None)]);
        hud.set_leader(PartyChara::Goofy, false);
        let p = kh_party::guard::GP;
        let look = GuardLook { fill: p.hud_color_fill, shine: p.hud_color_shine, loss: p.hud_color_loss, regen: p.hud_color_regen, broken: p.hud_color_broken, blink_hz: p.hud_broken_blink, offset: (p.hud_offset_x, p.hud_offset_y) };
        let g = kh_party::Guard::for_level(1).view();
        let gv = GuardValues { value: g.value, max: g.max, broken: g.broken, regenerating: g.regenerating, rearm_at: g.rearm_at };
        for i in 0..240 {
            hud.set_party(&[goofy(None)]);
            hud.set_leader(PartyChara::Goofy, false);
            hud.set_guard(if which == "start" { Some((gv, look.clone())) } else { None });
            step(&mut hud, 1);
            if i % 30 == 29 {
                let m = hud.movie("HudStatus").unwrap();
                let vis = |p: &str| m.node(p).map(|n| (n.is_visible(), n.cx.mul[3]));
                println!("{i}: sttsPc {:?} pcHp {:?} pcMp {:?} pcGuard {:?} frame sttsPc {:?} state {:?}", vis("sttsPc"), vis("sttsPc.pcHp"), vis("sttsPc.pcMp"), vis("sttsPc.pcGuard"), m.current_frame("sttsPc"), m.current_label("sttsPc"));
            }
        }
        shot(&hud, &format!("er_{which}"));
        return;
    }
    // Sora leading (the style reference)
    hud.set_party(&[donald, goofy(None)]);
    hud.set_leader(PartyChara::Sora, true);
    step(&mut hud, 150);
    shot(&hud, "00_sora_leads");
    if which == "tree" {
        walk(&hud.movie("HudStatus").unwrap().root, "", args.get(2).map(String::as_str).unwrap_or("sttsPc"));
        return;
    }
    // Goofy leading, the X3G shield gauge in the focus slot (the build before GG)
    hud.set_party(&[donald, goofy(Some((70, 100)))]);
    hud.set_leader(PartyChara::Goofy, true);
    step(&mut hud, 150);
    shot(&hud, "01_goofy_leads_before");
    // GG: Goofy leading with the GUARD bar (no focus-slot bar), the look of the sheet party-guard
    let p = kh_party::guard::GP;
    let look = GuardLook { fill: p.hud_color_fill, shine: p.hud_color_shine, loss: p.hud_color_loss, regen: p.hud_color_regen, broken: p.hud_color_broken, blink_hz: p.hud_broken_blink, offset: (p.hud_offset_x, p.hud_offset_y) };
    // a real meter, driven through its own API (kh-party guard.rs), one 60 Hz step per HUD frame
    let mut meter = kh_party::Guard::for_level(1);
    let feed = |hud: &mut Hud, meter: &kh_party::Guard| {
        let v = meter.view();
        hud.set_guard(Some((GuardValues { value: v.value, max: v.max, broken: v.broken, regenerating: v.regenerating, rearm_at: v.rearm_at }, look.clone())));
    };
    hud.set_party(&[donald, goofy(None)]);
    feed(&mut hud, &meter);
    step(&mut hud, 150);
    shot(&hud, "02_goofy_guard_full");
    if which == "tree2" {
        walk(&hud.movie("HudStatus").unwrap().root, "", args.get(2).map(String::as_str).unwrap_or("sttsPc.pcGuard"));
        return;
    }
    // a blocked hit of 45: the lost part in red, then it follows the fill down (KH3's FollowGauge)
    meter.absorb(45.0);
    for i in 0..30 {
        meter.tick(1.0 / 60.0);
        feed(&mut hud, &meter);
        step(&mut hud, 1);
        if i == 10 {
            shot(&hud, "03_goofy_guard_hit_loss");
        }
    }
    println!("after the hit: {:?}", hud.guard_bar_state());
    // draining while held (a drain of 40 / s for 0.5 s)
    for _ in 0..30 {
        meter.drain_per_sec(40.0, 1.0 / 60.0);
        meter.tick(1.0 / 60.0);
        feed(&mut hud, &meter);
        step(&mut hud, 1);
    }
    shot(&hud, "04_goofy_guard_draining");
    println!("draining: value {:.1} {:?}", meter.value, hud.guard_bar_state());
    // regen: out of use past the delay
    for _ in 0..(60.0 * (p.guard_refill_delay + 0.5)) as i32 {
        meter.tick(1.0 / 60.0);
        feed(&mut hud, &meter);
        step(&mut hud, 1);
    }
    shot(&hud, "05_goofy_guard_regen");
    println!("regen: value {:.1} regenerating {} {:?}", meter.value, meter.regenerating(), hud.guard_bar_state());
    // broken: a guard-breaking hit, then the broken refill (blinking) until it rearms
    meter.break_now();
    for i in 0..((p.guard_broken_delay + 0.6) * 60.0) as i32 {
        meter.tick(1.0 / 60.0);
        feed(&mut hud, &meter);
        step(&mut hud, 1);
        if i == 20 {
            shot(&hud, "06_goofy_guard_broken");
        }
    }
    shot(&hud, "07_goofy_guard_broken_refill");
    println!("broken refill: value {:.1} broken {} {:?}", meter.value, meter.broken, hud.guard_bar_state());
    for _ in 0..240 {
        meter.tick(1.0 / 60.0);
        feed(&mut hud, &meter);
        step(&mut hud, 1);
    }
    shot(&hud, "08_goofy_guard_rearmed_full");
    // level 99: GUARD 195 (the bar grows)
    let mut big = kh_party::Guard::for_level(99);
    big.spend(30.0);
    feed(&mut hud, &big);
    step(&mut hud, 60);
    shot(&hud, "09_goofy_guard_level99");
    println!("level 99: {:?}", hud.guard_bar_state());
    // Sora leading again: no GUARD bar
    hud.set_guard(None);
    hud.set_leader(PartyChara::Sora, true);
    step(&mut hud, 150);
    shot(&hud, "10_sora_leads_again");
    println!("sora: {:?}", hud.guard_bar_state());
}

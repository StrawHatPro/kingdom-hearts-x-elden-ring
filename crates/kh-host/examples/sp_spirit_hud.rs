//! (SUMMON) THE SUMMONED SPIRIT'S GAUGE, OFFLINE (THE USER 2026-11-02: "For Spirit Ashes, give them their own KH-styled HUD
//! element instead of using Elden Ring's HUD for them."): the whole cockpit (status HUD, command menu, side HUDs) drawn at
//! the Elden Ring host's stage root, with Sora's `command().summon.out` set as the host sets it, in the gauge's states
//! (full time, half time and damaged, the warning under 10 s, the Mimic Tear, Donald / Goofy leading, Sora alone, gone),
//! each as the full 1920 x 1080 frame and a 2x crop of the bottom-right corner, so the placement is judged against the
//! party gauges, the command menu and the leaders' own pieces.
//!
//!   cargo run -p kh-host --example sp_spirit_hud -- [OUT_DIR] [tree]
//!
//! Default OUT_DIR: build/sora_tmp/party/SUMMON/pics.

use kh_core::command::summon::SpiritOut;
use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::stage_root;
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

fn step(hud: &mut Hud, sora: &Sora, n: usize) {
    let mut snapshot = sora.ui_snapshot();
    let g = &mut snapshot.gauges;
    g.hp = g.hp_max * 7 / 10;
    g.mp = g.mp_max * 6 / 10;
    g.focus = g.focus_max / 2;
    for _ in 0..n {
        hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

fn walk(s: &kh_ui::player::Sprite, path: &str, filter: &str) {
    for n in s.children.iter().chain(&s.extra) {
        let p = format!("{path}.{}", n.name.as_deref().map(str::to_string).unwrap_or_else(|| format!("#{}", n.ch)));
        if p.contains(filter) {
            let m = n.matrix;
            let gfx = match &n.kind {
                kh_ui::player::Kind::Sprite(c) => format!("f{}/{} gfx {}", c.frame, c.def.frames, c.gfx.len()),
                _ => String::new(),
            };
            println!("{p} | {gfx} | m [{:.3} {:.3} {:.3} {:.3} {:.1} {:.1}] visible {} alpha {:.2}", m.a, m.b, m.c, m.d, m.tx, m.ty, n.is_visible(), n.cx.mul[3]);
        }
        if let kh_ui::player::Kind::Sprite(c) = &n.kind {
            walk(c, &p, filter);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = args.first().filter(|a| *a != "tree").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("build/sora_tmp/party/SUMMON/pics"));
    std::fs::create_dir_all(&out).unwrap();
    let (mut hud, fonts) = Hud::load(&root.join("build/sora_pack/ui")).expect("the UI pack");
    let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let donald = PartyHudSlot { chara: PartyChara::Donald, hp: 140, hp_max: 200, mp: 70, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None };
    let goofy = PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None };
    let shot = |hud: &Hud, name: &str| {
        let (w, h) = (1920usize, 1080usize);
        let (stage, _) = stage_root(w as f32, h as f32);
        let mut canvas = kh_ui::Canvas::new(w, h);
        let mut renderer = kh_ui::Renderer::new(fonts.clone());
        for movie in ["HudStatus", "HudCommand", "HudTarget", "HudLeft", "HudRight"] {
            if let Some(m) = hud.movie(movie) {
                renderer.draw(&mut canvas, m, &stage);
            }
        }
        let mut rgba = Vec::new();
        // a mid scene: a sky-to-ground ramp, so light and dark parts both read
        canvas.to_rgba8_over(|_, y| { let t = y as f32 / 1080.0; [0.42 - 0.12 * t, 0.44 - 0.1 * t, 0.46 - 0.16 * t] }, &mut rgba);
        let _ = kh_ui::def::write_png(&out.join(format!("{name}.png")), w as u32, h as u32, &rgba);
        // the bottom-right corner, 2x
        let (x0, y0, cw, ch, z) = (1160usize, 600usize, 760usize, 480usize, 2usize);
        let mut big = vec![0u8; cw * z * ch * z * 4];
        for y in 0..ch * z {
            for x in 0..cw * z {
                let s = ((y0 + y / z) * w + x0 + x / z) * 4;
                let d = (y * cw * z + x) * 4;
                big[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
            }
        }
        let _ = kh_ui::def::write_png(&out.join(format!("{name}_corner2x.png")), (cw * z) as u32, (ch * z) as u32, &big);
        // the spirit's element at the real HUD size (1x, 1920 x 1080 stage pixels)
        let (sx, sy, sw, sh) = (1560usize, 760usize, 360usize, 150usize);
        let mut real = vec![0u8; sw * sh * 4];
        for y in 0..sh {
            let s = ((sy + y) * w + sx) * 4;
            real[y * sw * 4..(y + 1) * sw * 4].copy_from_slice(&rgba[s..s + sw * 4]);
        }
        let _ = kh_ui::def::write_png(&out.join(format!("{name}_slot1x.png")), sw as u32, sh as u32, &real);
        println!("{name}: {:?}", hud.spirit_gauge_state());
    };
    // (THE USER 2026-11-03: "use the Elden Ring picture of the spirit ash") real icon numbers, Elden Ring 2.7.2.0
    // EquipParamGoods.iconId: Black Knife Tiche (row 200000) 113, the Mimic Tear (row 207000) 117
    let wolf = |hp: i32, left: f32| SpiritOut { slot: 0, name: "Black Knife Tiche +3".to_string(), cost: 66, left, full: 69.0, hp, hp_max: 600, mimic: false, icon: Some(113) };
    // Sora leading, Donald and Goofy in the party: no spirit yet (the reference)
    hud.set_party(&[donald, goofy]);
    hud.set_leader(PartyChara::Sora, true);
    step(&mut hud, &sora, 150);
    shot(&hud, "00_no_spirit");
    // 01: just summoned (after the slot's `in`), full time, full HP
    sora.command_mut().summon.out = Some(wolf(600, 69.0));
    step(&mut hud, &sora, 150);
    shot(&hud, "01_full_time");
    if args.iter().any(|a| a == "tree") {
        walk(&hud.movie("HudStatus").unwrap().root, "", "sttsSnpc1");
        return;
    }
    // 02: half the time gone, and a hit: the lost HP in red (it follows down), the slot's bounce
    sora.command_mut().summon.out = Some(wolf(600, 34.5));
    step(&mut hud, &sora, 2);
    sora.command_mut().summon.out = Some(wolf(230, 34.5));
    step(&mut hud, &sora, 8);
    shot(&hud, "02_half_time_damaged_hit");
    step(&mut hud, &sora, 120);
    shot(&hud, "03_half_time_damaged");
    // 04 / 05: under 10 s, the warning (the arc and the time text pulse to red): the pulse's peak and its low
    let mut left = 7.6f32;
    for i in 0..48 {
        left -= 1.0 / 60.0;
        sora.command_mut().summon.out = Some(wolf(230, left));
        step(&mut hud, &sora, 1);
        if i == 35 {
            shot(&hud, "04_warning_under_10s_peak");
        }
        if i == 47 {
            shot(&hud, "05_warning_under_10s_low");
        }
    }
    // 06: the spirit leaves (time out): the slot's `out`
    sora.command_mut().summon.out = None;
    step(&mut hud, &sora, 120);
    shot(&hud, "06_gone");
    // 07: the Mimic Tear (Mimic Sora): the Mimic Tear ash's own icon
    sora.command_mut().summon.out = Some(SpiritOut { slot: 1, name: "Mimic Tear +10".to_string(), cost: 99, left: 80.0, full: 90.0, hp: 900, hp_max: 1200, mimic: true, icon: Some(117) });
    step(&mut hud, &sora, 150);
    shot(&hud, "07_mimic_tear");
    // 08: Goofy leading with his GUARD bar (his leader HUD), the spirit out
    sora.command_mut().summon.out = Some(wolf(500, 50.0));
    hud.set_leader(PartyChara::Goofy, true);
    let p = kh_party::guard::GP;
    let look = GuardLook { fill: p.hud_color_fill, shine: p.hud_color_shine, loss: p.hud_color_loss, regen: p.hud_color_regen, broken: p.hud_color_broken, blink_hz: p.hud_broken_blink, offset: (p.hud_offset_x, p.hud_offset_y) };
    let gv = kh_party::Guard::for_level(20).view();
    for _ in 0..150 {
        hud.set_guard(Some((GuardValues { value: gv.value, max: gv.max, broken: gv.broken, regenerating: gv.regenerating, rearm_at: gv.rearm_at }, look.clone())));
        step(&mut hud, &sora, 1);
    }
    shot(&hud, "08_goofy_leads");
    hud.set_guard(None);
    // 09: Donald leading with two Resonance badges and the Arcane stars row (his leader HUD), the spirit out
    hud.set_leader(PartyChara::Donald, true);
    let ring = |color: [f32; 4], icon: &str, pips: f32| kh_ui::hud_resonance::RingLook { color, pips, pips_max: 3, full: pips >= 3.0, ult: Default::default(), icon: icon.to_string() };
    let don = kh_host::party::DonHudView {
        ring: kh_ui::hud_resonance::ResonanceView { rings: vec![ring([1.0, 0.45, 0.2, 1.0], "Fire", 2.0), ring([0.5, 0.8, 1.0, 1.0], "Blizzard", 1.0)], stars: Some(kh_ui::hud_resonance::StarsLook { lit: 2, max: 3, left: Some(5.0), zetta: Vec::new() }), ..Default::default() },
        ..Default::default()
    };
    for _ in 0..150 {
        hud.set_don(Some(don.clone()));
        step(&mut hud, &sora, 1);
    }
    shot(&hud, "09_donald_leads");
    hud.set_don(None);
    // 10: Sora alone (no party): the solo layout stays solo
    hud.set_leader(PartyChara::Sora, true);
    hud.set_party(&[]);
    step(&mut hud, &sora, 150);
    shot(&hud, "10_sora_alone");
    // 11: an ash whose icon number is not known: KH3's Link icon (the fallback)
    sora.command_mut().summon.out = Some(SpiritOut { icon: None, ..wolf(500, 50.0) });
    step(&mut hud, &sora, 30);
    shot(&hud, "11_no_icon_link_fallback");
    // 12 / 13: (THE USER 2026-11-03: "have the spirit ashes HUD element fade away after they either die or time out") the
    // fades, with Donald and Goofy back: the host's `leaving` held as kh-er holds it; frames of KH3's slot `out` (held 25
    // frames on the last values, then the roll and the alpha fall, then off)
    hud.set_party(&[donald, goofy]);
    for (name, reason, hp, left) in [("12_fade_died", kh_core::command::summon::LeaveReason::Died, 120, 40.0), ("13_fade_timeout", kh_core::command::summon::LeaveReason::Timeout, 500, 1.5)] {
        sora.command_mut().summon.leaving = None;
        sora.command_mut().summon.out = Some(wolf(hp, left));
        step(&mut hud, &sora, 150);
        shot(&hud, &format!("{name}_0_before"));
        let out = sora.command_mut().summon.out.take();
        sora.command_mut().summon.leaving = out.map(|o| (o, reason));
        let mut done = 0;
        for (k, at) in [1usize, 26, 38, 48, 58, 70].into_iter().enumerate() {
            step(&mut hud, &sora, at - done);
            done = at;
            shot(&hud, &format!("{name}_{}_frame{at:02}", k + 1));
        }
    }
    sora.command_mut().summon.leaving = None;
}

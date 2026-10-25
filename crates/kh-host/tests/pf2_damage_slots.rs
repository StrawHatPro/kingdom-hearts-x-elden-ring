//! (PF2) THE PORTRAITS' DAMAGE FLASH AND BOUNCE LAND ON THE CHARACTER WHO WAS HIT, for every leader and formation.
//! HUD2's finding: while a friend led, kh-host `hud.rs` sent Sora's HP drop to slot 0 (the friend's face) and the
//! leader's drop to Sora's small slot. The real `Hud` (UI pack) is driven: formations and leaders in turn (the leader
//! switch's shrink and grow included), then each member's HP is lowered alone; the status movie's interface calls must
//! show `setFaceState(slot, 1)` (the flash and the slot clip's bounce) on exactly ONE slot, the slot whose loaded face
//! movie is the character hit, and the seat the formation order gives him. Frames under build/sora_tmp/party/pf2/
//! (skipped without the UI pack).

use std::collections::HashMap;

use kh_core::{PadButtons, Sora, TargetId, UiSnapshot, Vec3};
use kh_host::formation::Formation;
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::{draw_movie_in, stage_root, Draw, Drawn, Perf};
use kh_host::party::{PartyChara, PartyHudSlot};
use PartyChara::*;

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn step(hud: &mut Hud, sora: &Sora, snapshot: &UiSnapshot, n: u32) {
    for _ in 0..n {
        hud.frame(&mut Frame { snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

/// The calls since the last take: the faces loaded (kept in `faces`, slot -> movie name) and the damage flashes' slots.
fn calls(hud: &mut Hud, faces: &mut HashMap<usize, String>) -> Vec<usize> {
    let mut flashes = Vec::new();
    for c in hud.take_status_calls() {
        if let Some(rest) = c.strip_prefix("loadFace(") {
            let slot: usize = rest.split(',').next().unwrap().trim().parse().unwrap();
            match rest.split("Some(\"").nth(1).and_then(|r| r.split('"').next()) {
                Some(name) => faces.insert(slot, name.to_string()),
                None => faces.remove(&slot),
            };
        } else if let Some(rest) = c.strip_prefix("setFaceState(") {
            let mut it = rest.trim_end_matches(')').split(',').map(|x| x.trim().parse::<i32>().unwrap());
            let (slot, state) = (it.next().unwrap(), it.next().unwrap());
            if state == 1 {
                flashes.push(slot as usize);
            }
        }
    }
    flashes
}

fn hp_max(c: PartyChara) -> i32 {
    match c {
        Sora => 0, // the snapshot's own
        Donald => 180,
        Goofy => 220,
    }
}

#[test]
fn the_damage_flash_lands_on_the_character_hit_for_every_leader() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("build/sora_pack/ui");
    let Ok((mut hud, fonts)) = Hud::load(&dir) else {
        eprintln!("no UI pack at {}: skipped", dir.display());
        return;
    };
    let out = root.join("build/sora_tmp/party/pf2");
    let _ = std::fs::create_dir_all(&out);
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let mut snapshot = sora.ui_snapshot();
    let sora_max = snapshot.gauges.hp_max;
    assert!(sora_max > 0);
    let (w, h) = (1920.0f32, 1080.0f32);
    let (stage, view) = stage_root(w, h);
    let mut renderer = kh_ui::render::Renderer::new(fonts.clone());
    let mut faces: HashMap<usize, String> = HashMap::new();
    let mut log = String::from("case\tseats\thit\tflash slots\tface there\n");

    // (name, formation in slot order, leader, Sora in the party, the seats the formation order gives)
    let cases: Vec<(&str, Vec<PartyChara>, PartyChara, bool, Vec<PartyChara>)> = vec![
        ("sora_leads", vec![Sora, Donald, Goofy], Sora, true, vec![Sora, Donald, Goofy]),
        ("goofy_leads", vec![Sora, Donald, Goofy], Goofy, true, vec![Goofy, Sora, Donald]),
        ("donald_leads", vec![Sora, Donald, Goofy], Donald, true, vec![Donald, Sora, Goofy]),
        ("sora_leads_again", vec![Sora, Donald, Goofy], Sora, true, vec![Sora, Donald, Goofy]),
        ("goofy_leads_order_gds", vec![Goofy, Donald, Sora], Goofy, true, vec![Goofy, Donald, Sora]),
        ("sora_leads_order_sgd", vec![Sora, Goofy, Donald], Sora, true, vec![Sora, Goofy, Donald]),
        ("donald_leads_goofy_out", vec![Sora, Donald], Donald, true, vec![Donald, Sora]),
        ("goofy_leads_sora_out", vec![Donald, Goofy], Goofy, false, vec![Goofy, Donald]),
        ("donald_leads_sora_out", vec![Goofy, Donald], Donald, false, vec![Donald, Goofy]),
        ("goofy_alone", vec![Goofy], Goofy, false, vec![Goofy]),
    ];
    let mut checked = 0;
    for (name, active, leader, sora_in, want) in &cases {
        let formation = Formation::new(active, *leader).unwrap();
        let friends: Vec<PartyChara> = active.iter().copied().filter(|c| *c != Sora).collect();
        let mut hp: HashMap<PartyChara, i32> = friends.iter().map(|c| (*c, hp_max(*c))).collect();
        let feed = |hp: &HashMap<PartyChara, i32>| -> Vec<PartyHudSlot> { friends.iter().map(|c| PartyHudSlot { chara: *c, hp: hp[c], hp_max: hp_max(*c), mp: 80, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None }).collect() };
        snapshot.gauges.hp = sora_max;
        hud.set_formation(&formation);
        hud.set_party(&feed(&hp));
        hud.set_leader(*leader, *sora_in);
        // the switch (shrink, swap, grow) and the HP refilled: no flash anywhere
        step(&mut hud, &sora, &snapshot, 150);
        let flashes = calls(&mut hud, &mut faces);
        assert!(flashes.is_empty(), "{name}: no damage flash at the switch / the refill ({flashes:?})");
        let seats: Vec<PartyChara> = hud.status_seats().iter().flatten().copied().collect();
        assert_eq!(&seats, want, "{name}: the seats are the leader then the formation order");
        for (slot, c) in want.iter().enumerate() {
            let face = faces.get(&slot).cloned().unwrap_or_default();
            assert!(face.contains(c.name()), "{name}: slot {slot} shows {face:?}, not {}", c.name());
        }
        if let Some(movie) = hud.movie("HudStatus") {
            let mut draw = Draw::default();
            if let Drawn::Picture(rect, _) = draw_movie_in(&mut draw, &mut renderer, movie, &stage, (w, h), true, Perf::default(), Some(view)) {
                let _ = kh_ui::def::write_png(&out.join(format!("{name}_0_before.png")), rect.2 as u32, rect.3 as u32, &draw.rgba);
            }
        }
        // each member hit alone, in turn (a third of his HP, never to 0)
        for (k, c) in want.iter().enumerate() {
            if *c == Sora {
                snapshot.gauges.hp = sora_max * 2 / 3;
            } else {
                hp.insert(*c, hp_max(*c) * 2 / 3);
                hud.set_party(&feed(&hp));
            }
            step(&mut hud, &sora, &snapshot, 1);
            let flashes = calls(&mut hud, &mut faces);
            let seat = hud.seat_of(*c);
            let face = seat.and_then(|s| faces.get(&s).cloned()).unwrap_or_default();
            log.push_str(&format!("{name}\t{}\t{}\t{flashes:?}\t{face}\n", seats.iter().map(|c| c.name()).collect::<Vec<_>>().join(","), c.name()));
            assert_eq!(flashes, vec![k], "{name}: {} was hit: the flash must play on his slot {k} only", c.name());
            assert_eq!(seat, Some(k));
            assert!(face.contains(c.name()), "{name}: the flashed slot {k} shows {face:?}, not {}", c.name());
            // the bounce a few frames on
            step(&mut hud, &sora, &snapshot, 5);
            if let Some(movie) = hud.movie("HudStatus") {
                let mut draw = Draw::default();
                if let Drawn::Picture(rect, _) = draw_movie_in(&mut draw, &mut renderer, movie, &stage, (w, h), true, Perf::default(), Some(view)) {
                    let _ = kh_ui::def::write_png(&out.join(format!("{name}_{}_hit_{}_slot{k}.png", k + 1, c.name().to_lowercase())), rect.2 as u32, rect.3 as u32, &draw.rgba);
                }
            }
            // healed back: no flash
            if *c == Sora {
                snapshot.gauges.hp = sora_max;
            } else {
                hp.insert(*c, hp_max(*c));
                hud.set_party(&feed(&hp));
            }
            step(&mut hud, &sora, &snapshot, 60);
            let after = calls(&mut hud, &mut faces);
            assert!(after.is_empty(), "{name}: a heal flashes nothing ({after:?})");
            checked += 1;
        }
    }
    let _ = std::fs::write(out.join("flashes.tsv"), &log);
    eprintln!("{log}{checked} hits checked; frames and flashes.tsv in {}", out.display());
    assert_eq!(checked, 3 * 6 + 2 * 3 + 1);
}

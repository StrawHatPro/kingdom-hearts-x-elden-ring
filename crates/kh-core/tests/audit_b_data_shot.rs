//! (AUDIT-B W-DATA) F-HAMMER's "not in this session" rows, traced: Classic Tone's rush shotlock (no form held; the drill
//! w_so113 as the shot weapon, weapon form 0x15 = `wf21`). KH3's w_so113 plays its L481 / L483 rows of M_6481 / M_6483 and
//! their notifies fire on it: vw_so111_chg_lit00 (L481 @0.0001, group 1900), vw_so113_top_fls02 (L483 state, group 1900) and
//! two se02011_020 (L483). The `SHOT` rows of sheets/port/sora-form-hammer.json are gated on the shot weapon form
//! (combat/forms/hammer.rs `wnotify_live`) and played by the shared pass on their `kb:w_so113:..` anchors. Trace
//! build/sora_tmp/weapons/AUDIT-B/W-DATA/traces/classic_tone_rush_rows.trace.

mod shotlocks_common;

use std::fmt::Write as _;

use kh_core::host::recording::Call;
use kh_core::StateId;
use shotlocks_common::{dummy, level99_with, Bench};

#[test]
fn classic_tone_rush_plays_the_drills_own_l481_l483_rows() {
    let targets = [dummy(1, 800.0, 0.0), dummy(2, 2000.0, 0.0)];
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_11"), &targets);
    b.aim_until(0, 900);
    let kind = b.sora.aim().kind;
    // (the shot's first clip M_6481 starts within `fire`'s steps: its calls are read from here)
    let mut before = b.host.calls.len();
    b.fire();
    let form = b.sora.magic().shot_weapon_form;
    let mut text = format!("# (AUDIT-B W-DATA) Classic Tone's MAX shotlock (kind {kind}), shot weapon form {form}\n");
    let mut spawned: Vec<(String, String)> = Vec::new();
    let mut sounds: Vec<(String, String)> = Vec::new();
    for _ in 0..1200 {
        let o = b.step(&[]);
        for c in &b.host.calls[before..] {
            match c {
                Call::Spawn(_, s) if format!("{:?}", s.at).contains("kb:w_so113") => {
                    let _ = writeln!(text, "{:?} {:.3} spawn {} at {:?}", o.anim.name, o.anim.time, s.name, s.at);
                    spawned.push((s.name.to_string(), format!("{:?}", s.at)));
                }
                Call::Play(_, p) if format!("{:?}", p.at).contains("kb:w_so113") => {
                    let _ = writeln!(text, "{:?} {:.3} sound {} at {:?}", o.anim.name, o.anim.time, p.name, p.at);
                    sounds.push((p.name.to_string(), format!("{:?}", p.at)));
                }
                _ => {}
            }
        }
        before = b.host.calls.len();
        if o.state != StateId::Shootflow {
            break;
        }
    }
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_tmp/weapons/AUDIT-B/W-DATA/traces/classic_tone_rush_rows.trace"), &text).ok();
    assert_eq!(form, 21, "the drill's weapon form 0x15\n{text}");
    assert!(spawned.iter().any(|(n, a)| n == "vw_so111_chg_lit00" && a.contains(":w_so052")), "L481's vw_so111_chg_lit00 on w_so113's w_so052\n{text}");
    assert!(spawned.iter().any(|(n, a)| n == "vw_so113_top_fls02" && a.contains(":ATK_01")), "L483's vw_so113_top_fls02 on ATK_01\n{text}");
    assert_eq!(sounds.iter().filter(|(n, a)| n == "se02011_020" && a.contains(":wep")).count(), 2, "L483's two se02011_020 on `wep`\n{text}");
}

/// (AUDIT-B W-DATA) F-NANO's 12 untraced rows: Nano Gear's Zone Connector shotlock plays M_8481 .. 8483 and the weapon
/// w_so172's own clips of them (`wfx.weapon.848x.*`: vw_so172_gte_glw00 on R_wep / L_wep, vw_so172_gte_yoy01 and
/// vw_so064_gte_fsh00 on R_y_center / L_y_center) spawn through nano.rs's clip player. Trace `nano_zone_connector_rows.trace`.
#[test]
fn nano_gear_zone_connector_plays_the_weapons_own_848x_rows() {
    let targets = [dummy(1, 800.0, 0.0), dummy(2, 1400.0, 200.0)];
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_08"), &targets);
    b.aim_until(0, 900);
    let kind = b.sora.aim().kind;
    let mut before = b.host.calls.len();
    b.fire();
    let mut text = format!("# (AUDIT-B W-DATA) Nano Gear's MAX shotlock (kind {kind})\n");
    let mut seen: Vec<(String, String)> = Vec::new();
    let mut clips = Vec::new();
    for _ in 0..1500 {
        let o = b.step(&[]);
        let name = format!("{:?}", o.anim.name);
        if !clips.contains(&name) {
            clips.push(name.clone());
        }
        for c in &b.host.calls[before..] {
            if let Call::Spawn(_, s) = c {
                if s.name.starts_with("vw_so172_gte") || s.name.starts_with("vw_so064_gte_fsh") {
                    let _ = writeln!(text, "{name} {:.3} spawn {} at {:?}", o.anim.time, s.name, s.at);
                    seen.push((name.clone(), s.name.to_string()));
                }
            }
        }
        before = b.host.calls.len();
        if o.state != StateId::Shootflow {
            break;
        }
    }
    let _ = writeln!(text, "# clips {clips:?}");
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_tmp/weapons/AUDIT-B/W-DATA/traces/nano_zone_connector_rows.trace"), &text).ok();
    for (clip, effect, n) in [("A8481", "vw_so172_gte_glw00", 2), ("A8481", "vw_so172_gte_yoy01", 2), ("A8482", "vw_so172_gte_glw00", 2), ("A8482", "vw_so064_gte_fsh00", 2), ("A8483", "vw_so172_gte_yoy01", 2), ("A8483", "vw_so172_gte_glw00", 2)] {
        let got = seen.iter().filter(|(c, e)| c == clip && e == effect).count();
        assert!(got >= n, "{clip}: {effect} x{n} (got {got})\n{text}");
    }
}

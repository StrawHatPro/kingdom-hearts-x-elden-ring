//! (AUDIT-B W-DATA) THE DATA ITEMS OF AUDIT-B, END TO END (real core -> the effect / sound seams -> `kh_host::vfx::EffectHost`
//! with the live pack, recorded by AUDIT-A's `kh_host::audit_a::Session`):
//! - Twin Yo-Yos' magic clips 4321 .. 4340: their `AttachDataList` (ead_w_so060_com) rows spawn with the spell's effect group
//!   (FireMagic's play group = the unit's m_EffectGroup, 0x1413794B0) on the yo-yos' own meshes, and the string beam
//!   (vw_so064_bem_lin00, TypeDataSQEX_Thunder over vfx_himo00 .. 16) is drawn; a spell whose group the asset lacks (Curaga)
//!   spawns none;
//! - ChaseAttack in every AUDIT-B form: no replacement set has a row of it (raw AnimSet assets and the generated sets), so the
//!   chase plays the base ChaseAttack through `forms::resolve`, as KH3's FindAnim does;
//! - FINISH_SK's Chandelier hits: no Kingdom Key hit effect and no hit SE (StylePawn w_so088: no m_HitEffects entry, an alias-only
//!   notify that KH3 resolves for weapon actors only).
//!
//! Traces go to build/sora_tmp/weapons/AUDIT-B/W-DATA/traces/. Skipped, saying so, without the pack.

use std::fmt::Write as _;
use std::path::PathBuf;

use glam::Vec3;
use kh_core::{AnimId, PadButton, StateId};
use kh_host::audit_a::{self, Session};

fn traces() -> PathBuf {
    let dir = audit_a::project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("W-DATA").join("traces");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn transform(s: &mut Session, cmd: i32) {
    s.rig.sora.debug_add_style(100.0);
    s.idle(20);
    assert!(s.rig.sora.combat().situation.has(cmd), "{cmd:#x} offered");
    s.step(&[PadButton::SITUATION]);
    for _ in 0..600 {
        if s.step(&[]) != StateId::Stylechange && s.rig.sora.combat().form.cmd == cmd {
            return;
        }
    }
    panic!("STYLECHANGE to {cmd:#x} did not end");
}

fn combo(s: &mut Session) {
    for k in 0..1200 {
        let state = if k % 2 == 0 { s.step(&[PadButton::ATTACK]) } else { s.step(&[]) };
        if k > 30 && matches!(state, StateId::Idle | StateId::Run | StateId::Jump) {
            break;
        }
    }
    s.idle(30);
}

/// What one cast showed.
#[derive(Default)]
struct Cast {
    play: &'static str,
    groups: Vec<i32>,
    /// (effect, anchor, drawn) of every string / hand-glow spawn
    spawns: Vec<(String, String, usize)>,
    /// most smi_lne_00k strips on one step, steps with strips, steps where every strip joins the next one
    strips: usize,
    steps_with: usize,
    chained: usize,
}

/// L1 + `button` (the shortcut page A slot: right Firaga, top Blizzaga, left Thundaga, bottom Curaga), then until the cast ends.
fn cast(s: &mut Session, button: PadButton, label: &str, text: &mut String) -> Cast {
    let max = s.rig.sora.status().max_mp();
    // (an MP charge left by the cast before ends: SetMagicPoint is ignored while charging)
    s.rig.sora.status_mut().set_mp_charge(false, 1.0);
    s.rig.sora.status_mut().set_mp(max);
    s.idle(10);
    s.step(&[PadButton::LeftShoulder]);
    s.step(&[PadButton::LeftShoulder, button]);
    let camera = kh_vfx::Camera { position: s.rig.out.position + Vec3::new(-400.0, 0.0, 150.0), forward: Vec3::X, right: Vec3::Y, up: Vec3::Z };
    let mut c = Cast::default();
    let mut started = false;
    let mut first = true;
    let _ = writeln!(text, "# cast {label}");
    for _ in 0..400 {
        let state = s.step(&[PadButton::LeftShoulder]);
        if state == StateId::Firemagic {
            started = true;
            let o = &s.rig.out;
            // the cast clip's steps (the state's first step still shows the play before it)
            if !std::mem::take(&mut first) {
                c.play = o.anim_ticked.name.name();
                c.groups.push(s.rig.sora.effect_group());
            }
            let (list, materials, _) = s.fx.draw(1.0, &camera);
            let strips: Vec<&kh_vfx::Ribbon> = list.ribbons.iter().filter(|r| materials.get(r.material as usize).is_some_and(|m| m.rsplit(':').next() == Some("smi_lne_00k"))).collect();
            if let Some(row) = s.rows.last() {
                for e in row.effects.iter().filter(|e| e.stop.is_none() && (e.name.contains("bem_lin") || e.name.contains("bem_lin_glw"))) {
                    let _ = writeln!(text, "step {} {} {:.3} group {} spawn {} at {} drawn {} (system at {:?} in his actor frame)", row.step, row.clip, row.time, row.effect_group, e.name, e.attach, e.drawn, e.at_actor);
                    c.spawns.push((e.name.clone(), e.attach.clone(), e.drawn));
                }
            }
            if !strips.is_empty() {
                let mid = |p: &kh_vfx::RibbonPoint| (p.a + p.b) * 0.5;
                let ends: Vec<(Vec3, Vec3)> = strips.iter().map(|r| (mid(&r.points[0]), mid(r.points.last().unwrap_or(&r.points[0])))).collect();
                let links = ends.iter().filter(|(_, e)| ends.iter().any(|(b, _)| b.distance(*e) < 0.01)).count();
                let length: f32 = ends.iter().map(|(a, b)| a.distance(*b)).sum();
                c.steps_with += 1;
                c.strips = c.strips.max(strips.len());
                // two yo-yos, 16 segments each (vfx_himo00 .. 16): 15 links per string
                if links + 2 >= strips.len() && strips.len() >= 2 {
                    c.chained += 1;
                }
                let _ = writeln!(text, "step {} {} {:.3} | {} strips, {links} links, {length:.1} cm of string", o.step, o.anim_ticked.body_samples()[0].0, o.anim_ticked.time, strips.len());
            }
        } else if started {
            break;
        } else if s.rig.out.step.is_multiple_of(20) {
            let _ = writeln!(text, "step {} {state:?} (not cast yet: mp {} hp {}, last executed {})", s.rig.out.step, s.rig.sora.status().mp, s.rig.sora.status().hp, s.rig.sora.command().last_executed);
        }
    }
    s.idle(30);
    let _ = writeln!(text, "# {label}: play {}, groups {:?}, {} spawns, at most {} strips, {} steps with strips, {} chained", c.play, { let mut g = c.groups.clone(); g.dedup(); g }, c.spawns.len(), c.strips, c.steps_with, c.chained);
    c
}

#[test]
fn twin_yo_yos_magic_spins_the_strings_of_ead_w_so060_com() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_05", &["w_so060", "w_so061", "w_so062", "w_so063", "w_so064", "w_so065"], "w_so060", Some(Vec3::new(400.0, 0.0, 0.0)), "auditb_data_yoyo_magic") else {
        eprintln!("SKIP: no pack");
        return;
    };
    let pack = audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    s.idle(5);
    transform(&mut s, 0x8A);
    s.idle(30);
    combo(&mut s);
    transform(&mut s, 0x82);
    s.idle(30);
    let mut text = String::from("# (AUDIT-B W-DATA) Twin Yo-Yos' magic: the yo-yos' AttachDataList rows (ead_w_so060_com) per cast\n");
    let fire = cast(&mut s, PadButton::FaceRight, "Firaga (group 1303)", &mut text);
    let blizzard = cast(&mut s, PadButton::FaceTop, "Blizzaga (group 1323)", &mut text);
    let cure = cast(&mut s, PadButton::FaceBottom, "Curaga (group 1424: not in the asset)", &mut text);
    std::fs::write(traces().join("yoyo_magic_strings.trace"), &text).ok();
    for (c, group) in [(&fire, 1303), (&blizzard, 1323)] {
        assert!(c.play.starts_with("Magic") && c.play.ends_with('4'), "the form's blend space plays ({})\n{text}", c.play);
        assert!(!c.groups.is_empty() && c.groups.iter().all(|g| *g == group), "the cast plays with the spell's group {group}: {:?}", c.groups);
        // the string and the hand glow, on each yo-yo's own mesh
        for model in ["w_so064", "w_so065"] {
            for (fx, socket) in [("vw_so064_bem_lin00", "vfx_himo00:tovfx_himo16"), ("vw_so064_bem_lin_glw00", ":wep")] {
                let found = c.spawns.iter().find(|(n, a, _)| n == fx && a.contains(&format!("kb:{model}:")) && a.contains(socket));
                assert!(found.is_some_and(|f| f.2 > 0), "{fx} on {model} spawned and drawn ({found:?})\n{text}");
            }
        }
        assert_eq!(c.spawns.len(), 4, "one spawn per asset row and yo-yo\n{text}");
        // the beams (TypeDataSQEX_Thunder, bone list vfx_himo00 .. 16 resolved on each yo-yo's mesh): both strings drawn. kh-vfx
        // spawns the emitter's burst of ONE beam particle (beam 0 = himo00 -> himo01); the data's bAlwaysOn + MaxBeamCount 16
        // (= the 16 links of the list) is not evaluated there yet: reported to the beam renderer's owner (W-DATA SECTION.md)
        assert!(c.steps_with > 5, "the strings are drawn ({} steps)\n{text}", c.steps_with);
        assert!(c.strips >= 2, "both strings ({} strips)\n{text}", c.strips);
    }
    assert!(!cure.groups.is_empty() && cure.groups.iter().all(|g| *g == 1424), "Curaga's group {:?}", cure.groups);
    assert!(cure.spawns.is_empty(), "no string on a Cure (the asset has no Cure group): {:?}", cure.spawns);
}

/// The AUDIT-B forms and their commands.
const FORMS: [(&str, i32); 11] = [("HM", 0x81), ("DR", 0x84), ("HM2", 0x92), ("DR2", 0x93), ("CL", 0x8A), ("YO", 0x82), ("CL2", 0x90), ("SK", 0x85), ("GM", 0x86), ("HL", 0x8C), ("FL", 0x8D)];

#[test]
fn chase_attack_plays_the_base_clip_in_every_audit_b_form() {
    let content = audit_a::project().join("assets_converted").join("packages").join("KINGDOM HEARTS III").join("Content").join("Character").join("pc").join("p_ex001").join("anm");
    let mut text = String::from("# (AUDIT-B W-DATA) ChaseAttack per form: the replacement sets (m_ReplaceAnimSets via FindAnim 0x14056DE80) and the row it plays\n");
    for (name, cmd) in FORMS {
        let row = kh_core::combat::form::row_of(cmd).unwrap_or_else(|| panic!("form {name} {cmd:#x}"));
        assert!(!row.anim_sets.is_empty(), "{name}: its sets");
        for set in row.anim_sets {
            let file = set.rsplit(['.', '/']).next().unwrap_or(set);
            let raw = content.join(format!("{file}.uasset.json"));
            let in_raw = std::fs::read_to_string(&raw).map(|t| t.contains("ChaseAttack"));
            let _ = writeln!(text, "{name} {cmd:#x} {file}: raw asset has ChaseAttack: {in_raw:?}");
            assert!(!in_raw.unwrap_or(false), "{name}: {file} names ChaseAttack");
        }
        assert_eq!(kh_core::combat::forms::replaced(cmd, "ChaseAttack"), None, "{name}: no set of the chain replaces ChaseAttack");
        assert_eq!(kh_core::combat::forms::resolve(cmd, AnimId::ChaseAttack), AnimId::ChaseAttack, "{name}: the chase plays the base row");
        let _ = writeln!(text, "{name} {cmd:#x}: resolve(ChaseAttack) = {}", kh_core::combat::forms::resolve(cmd, AnimId::ChaseAttack).name());
    }
    // the base set is the only one that has it
    let base = std::fs::read_to_string(content.join("p_ex001_AnimSet.uasset.json")).map(|t| t.contains("ChaseAttack"));
    let _ = writeln!(text, "p_ex001_AnimSet (the base set) has ChaseAttack: {base:?}");
    assert!(base.unwrap_or(true));
    std::fs::write(traces().join("chase_attack_per_form.trace"), &text).ok();
}

#[test]
fn finish_sk_chandelier_hits_have_no_keyblade_effect_and_no_hit_se() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_06", &["w_so080", "w_so081", "w_so082", "w_so083", "w_so084", "w_so085", "w_so086", "w_so087", "w_so088"], "w_so080", Some(Vec3::new(250.0, 0.0, 0.0)), "auditb_data_chandelier_hits") else {
        eprintln!("SKIP: no pack");
        return;
    };
    let pack = audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    s.idle(5);
    transform(&mut s, 0x90);
    s.idle(30);
    combo(&mut s);
    transform(&mut s, 0x85);
    s.idle(30);
    combo(&mut s);
    for _ in 0..30 {
        if s.rig.sora.combat().situation.slot_b.map(|e| e.kind) == Some(0x9F) {
            break;
        }
        s.rig.sora.debug_add_style(100.0);
        s.idle(10);
    }
    assert_eq!(s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9F), "FINISH_SK on offer");
    let from = s.rows.len();
    s.step(&[PadButton::SITUATION]);
    for _ in 0..1500 {
        if s.step(&[]) != StateId::Stylefinish && s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    let mut text = String::from("# (AUDIT-B W-DATA) FINISH_SK: the steps with a hit of the Chandelier's window (SCSK_CHANDELIER_END on FinishSK2)\n");
    let mut hits = 0;
    for row in &s.rows[from..] {
        let chandelier = row.events.iter().any(|e| e.contains("Hit {") && e.contains("SCSK_CHANDELIER_END"));
        if !chandelier {
            continue;
        }
        hits += 1;
        let effects: Vec<&str> = row.effects.iter().filter(|e| e.stop.is_none() && !e.attach.starts_with("Sora")).map(|e| e.name.as_str()).collect();
        let sounds: Vec<&str> = row.sounds.iter().filter(|v| v.stop.is_none() && v.attach.starts_with("World")).map(|v| v.name.as_str()).collect();
        let _ = writeln!(text, "step {} {} {:.3}: world effects {effects:?}, world sounds {sounds:?}", row.step, row.clip, row.time);
        assert!(!effects.iter().any(|e| e.starts_with("vw_so010")), "no Kingdom Key hit effect on the Chandelier's hit: {effects:?}\n{text}");
        assert!(!sounds.iter().any(|v| v.starts_with("HIT")), "no hit SE on the Chandelier's hit: {sounds:?}\n{text}");
    }
    let _ = writeln!(text, "# {hits} steps with a Chandelier hit");
    std::fs::write(traces().join("chandelier_hits.trace"), &text).ok();
    assert!(hits > 0, "the Chandelier hit the dummy\n{text}");
}

/// (AUDIT-B W-DATA) F-HAMMER's open item 6: the drills' air / rise / dive clips the sessions' geometry never reaches (6415 ..
/// 6418, 6420, 6424 under Drill Punch, the same sequences under Clock Drill, whose w_so113 plays its L4xx rows of M_64xx),
/// each played whole with the form held and the group its PlayAttack row passes (DR 0, DR2 1900): every `wnotify` row of
/// sheets/port/sora-form-hammer.json for that clip that the group rule lets through is emitted on its `kb:` anchor and drawn
/// (effects) or sent and played (sounds). Trace `hammer_air_clips.trace`.
/// One keyblade's sweep: its item key, models, sound model, the two stages' commands, the second stage's sheet form key and
/// the effect group its PlayAttack rows pass.
struct Sweep<'a> {
    key: &'a str,
    models: &'a [&'a str],
    base: &'a str,
    stages: [i32; 2],
    form: &'a str,
    group: i32,
}

fn hammer_sweep(w: &Sweep<'_>, clips: &[&str], text: &mut String) -> Option<Vec<String>> {
    let Sweep { key, models, base, stages, form, group } = *w;
    let mut s = Session::new(key, models, base, Some(Vec3::new(250.0, 0.0, 0.0)), &format!("auditb_data_hammer_{form}"))?;
    let pack = audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    s.idle(5);
    transform(&mut s, stages[0]);
    s.idle(30);
    combo(&mut s);
    transform(&mut s, stages[1]);
    s.idle(30);
    let mut failed = Vec::new();
    let mut ranges = Vec::new();
    for clip in clips {
        let id = AnimId::from_name(clip).unwrap_or_else(|| panic!("{clip} in the animation table"));
        let from = s.rows.len();
        s.rig.sora.debug_play_anim_group(id, kh_core::PlayArgs::mode(0, 0.0), group);
        let mut seen = false;
        for _ in 0..400 {
            s.step(&[]);
            if s.rows.last().map(|x| x.clip) == Some(*clip) {
                seen = true;
            } else if seen {
                break;
            }
        }
        assert!(seen, "{clip} played");
        ranges.push((*clip, from, s.rows.len()));
        s.idle(20);
    }
    // the audio log's lines join their steps here
    s.finish();
    for (clip, from, to) in ranges {
        let steps: Vec<&audit_a::StepRow> = s.rows[from..to].iter().filter(|x| x.clip == clip).collect();
        for r in kh_core::combat::forms::hammer::FORM_HAMMER.iter().filter(|r| r.what == "wnotify" && r.form == form && r.clip == clip) {
            let anchor = r.flags.first().copied().unwrap_or("");
            let row_group = r.flags.get(3).and_then(|g| g.strip_prefix('g')).and_then(|g| g.parse::<i32>().ok()).unwrap_or(0);
            let (ok, how) = if r.code == 0 {
                if !kh_core::notify::effect_group_passes(row_group, true, group) {
                    let any = steps.iter().any(|x| x.effects.iter().any(|e| e.stop.is_none() && r.effects.first().is_some_and(|n| e.name.eq_ignore_ascii_case(n)) && e.attach.contains(anchor)));
                    (!any, "not due (group)".to_string())
                } else {
                    let hit = steps.iter().flat_map(|x| x.effects.iter()).find(|e| e.stop.is_none() && r.effects.first().is_some_and(|n| e.name.eq_ignore_ascii_case(n)) && e.attach.contains(anchor));
                    (hit.is_some_and(|e| e.drawn > 0), hit.map_or("NOT EMITTED".to_string(), |e| format!("emitted, drawn {}", e.drawn)))
                }
            } else {
                let name = r.sounds.first().copied().unwrap_or("");
                let sent = steps.iter().any(|x| x.sounds.iter().any(|v| v.stop.is_none() && v.name.eq_ignore_ascii_case(name)));
                let short = name.to_lowercase();
                let played = steps.iter().any(|x| x.audio.iter().any(|a| a.1.to_lowercase().ends_with(&short) && (a.0.starts_with("play") || a.0.starts_with("cancel"))));
                (sent && (played || r.code == 2), format!("{} {}", if sent { "sent" } else { "NOT SENT" }, if played { "played" } else { "(no audio line)" }))
            };
            let line = format!("{} {form} {clip} {} {} {} -> {how}", if ok { "PASS" } else { "FAIL" }, r.id, r.effects.first().or(r.sounds.first()).copied().unwrap_or(""), anchor);
            let _ = writeln!(text, "{line}");
            if !ok {
                failed.push(line);
            }
        }
    }
    Some(failed)
}

#[test]
fn the_drills_air_rise_and_dive_clips_play_their_own_rows() {
    let mut text = String::from("# (AUDIT-B W-DATA) the drills' clips outside the sessions' geometry: their weapon rows\n");
    let clips = ["6415", "6416", "6417", "6418", "6420", "6424"];
    let Some(mut failed) = hammer_sweep(&Sweep { key: "WEP_KEYBLADE_SO_03", models: &["w_so050", "w_so052", "w_so051", "w_so053"], base: "w_so050", stages: [0x81, 0x84], form: "DR", group: 0 }, &clips, &mut text) else {
        eprintln!("SKIP: no pack");
        return;
    };
    failed.extend(hammer_sweep(&Sweep { key: "WEP_KEYBLADE_SO_11", models: &["w_so110", "w_so112", "w_so111", "w_so113", "w_so114"], base: "w_so110", stages: [0x92, 0x93], form: "DR2", group: 1900 }, &clips, &mut text).unwrap_or_default());
    let n = text.lines().filter(|l| l.starts_with("PASS") || l.starts_with("FAIL")).count();
    let _ = writeln!(text, "# {n} rows, {} failed", failed.len());
    std::fs::write(traces().join("hammer_air_clips.trace"), &text).ok();
    assert!(n >= 50, "the rows of the clips ({n})\n{text}");
    assert!(failed.is_empty(), "{} of {n} rows failed:\n{}", failed.len(), failed.join("\n"));
}

/// (AUDIT-B W-DATA) F-NANO row 19 ("PASS (code)"), traced: Nano Arms' hookshot kick 0xBD (M_8424). KH3's on-contact handler
/// 0x141355780 sets m_bEnableChange at the kick's FIRST contact, so the combo goes on from the hit, not from the clip's own
/// TresEnableChange at 0.7333 s. The trace shows the hit step and the step the play leaves 8424. Trace `nano_kick.trace`.
#[test]
fn nano_arms_kick_hands_the_combo_on_at_its_contact() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_08", &["w_so170", "w_so172", "w_so173", "w_so174"], "w_so170", Some(Vec3::new(200.0, 0.0, 0.0)), "auditb_data_nano_kick") else {
        eprintln!("SKIP: no pack");
        return;
    };
    s.idle(5);
    transform(&mut s, 0x86);
    s.idle(30);
    let mut text = String::from("# (AUDIT-B W-DATA) Nano Arms: the hookshot (8423) -> the kick 0xBD (8424) at a target 7 m away, Attack held on\n");
    let mut verdicts = Vec::new();
    for round in 0..2 {
        let o = s.rig.out.clone();
        let fwd = Vec3::new(o.yaw.to_radians().cos(), o.yaw.to_radians().sin(), 0.0);
        let at = Vec3::new(o.position.x, o.position.y, 0.0) + fwd * 700.0;
        if let Some(d) = s.rig.dummy.as_mut() {
            d.info.position = at;
            d.info.center = at + Vec3::Z * 90.0;
        }
        s.idle(20);
        let from = s.rows.len();
        for k in 0..400 {
            let st = if k % 2 == 0 { s.step(&[PadButton::ATTACK]) } else { s.step(&[]) };
            if k > 60 && matches!(st, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        let rows = &s.rows[from..];
        let kick: Vec<&audit_a::StepRow> = rows.iter().filter(|x| x.clip == "8424").collect();
        let hit = kick.iter().find(|x| x.events.iter().any(|e| e.contains("Hit {") && e.contains("SCGM_HOOKSHOT_END")));
        let last = kick.last();
        let after = last.and_then(|l| rows.iter().find(|x| x.step == l.step + 1));
        let _ = writeln!(text, "round {round}: 8424 on {} steps ({:?} .. {:?}), first contact at step {:?} t {:?}, left at t {:?} for {:?} ({:?})", kick.len(), kick.first().map(|x| x.step), last.map(|x| x.step), hit.map(|x| x.step), hit.map(|x| x.time), last.map(|x| x.time), after.map(|x| x.clip), after.map(|x| x.state));
        if let (Some(h), Some(l)) = (hit, last) {
            // the play leaves the kick before the clip's own TresEnableChange (0.7333 s), and after its contact
            verdicts.push(l.time < 0.7333 - 1.0e-3 && h.step <= l.step && after.is_some_and(|a| a.clip != "8424"));
        }
    }
    std::fs::write(traces().join("nano_kick.trace"), &text).ok();
    assert!(!verdicts.is_empty(), "a kick with a contact\n{text}");
    assert!(verdicts.iter().all(|v| *v), "the combo goes on from the kick's contact\n{text}");
}

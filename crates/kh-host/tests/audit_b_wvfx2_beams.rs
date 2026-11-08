//! (AUDIT-B W-VFX2) THE BEAMS OF THE CORE-SIDE `wfx` ROUTES, END TO END: Twin Yo-Yos' strings (vw_so064_bem_lin00 / 03,
//! vw_so064_gte_bem_lin01), Blizzard Claws' transformation sparks (vw_so062_chg_thd07) and Nano Arms' yo-yo lines
//! (vw_so172_yoy_lin00 / 01) are TypeDataSQEX_Thunder effects. KH3's thunder tick (0x144250690) reads their ends from the
//! mesh the particle component is attached to: SetThunderAttachBones' SocketName / EndSocketName, or the bone list (beam i
//! from BoneList[i] to BoneList[i + 1]). The rows (tools/sora_form_claws.js / sora_form_nano.js) now carry that mesh as a
//! `kb:<model>:<mount>:<socket>[:to<end>]` anchor (tools/lib/wnotify_rows.js `thunderAnchor`), kh-core claws.rs / nano.rs
//! spawn the rows there, and kh-host `EffectHost::beam_ends` resolves the ends on the weapon's own skeleton every step.
//!
//! Checked through the real core -> the effect seam -> `EffectHost` with the live pack (AUDIT-A's Session): every strip of
//! the beams' materials drawn runs between two consecutive bones of its effect's list (or its socket and EndSocketName) on
//! the weapon mesh its row names, at those bones' places this step; the node sits where the old FK track put it. Traces in
//! build/sora_tmp/weapons/AUDIT-B/W-VFX2/traces/item2/. Skipped without the pack.

use std::collections::HashMap;
use std::fmt::Write as _;

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{self, Session};
use kh_host::vfx::Scene;

fn traces() -> std::path::PathBuf {
    let dir = audit_a::project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("W-VFX2").join("traces").join("item2");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// The bone list of a pack effect's thunder emitter (empty: a socket pair) and its material's short name.
fn beam_def(name: &str) -> Option<(Vec<String>, String)> {
    let path = audit_a::project().join("build").join("sora_pack").join("keyblades").join("vfx").join("effects").join(format!("{name}.json"));
    let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(path).ok()?.trim_start_matches('\u{feff}')).ok()?;
    let e = json["emitters"].as_array()?.iter().find(|e| e["typedata"]["kind"] == "sqex_thunder")?;
    let p = &e["typedata"]["params"];
    let bones = if p["bUseBoneList"].as_bool() == Some(true) { p["BoneList"].as_array()?.iter().filter_map(|b| b.as_str().map(str::to_string)).collect() } else { Vec::new() };
    let material = e["material"].as_str().or_else(|| e["required"]["material"].as_str()).unwrap_or("").rsplit(['.', '/']).next().unwrap_or("").to_string();
    Some((bones, material))
}

/// `kb:<model>:<mount>:<bone>[:flags]` -> (`kb:<model>:<mount>:`, bone, EndSocketName).
fn split_anchor(anchor: &str) -> Option<(String, String, Option<String>)> {
    let rest = anchor.strip_prefix("kb:")?;
    let mut parts = rest.split(':');
    let (model, mount, bone) = (parts.next()?, parts.next()?, parts.next().unwrap_or(""));
    let to = parts.find_map(|f| f.strip_prefix("to")).map(str::to_string);
    Some((format!("kb:{model}:{mount}:"), bone.to_string(), to))
}

/// The anchor of a spawn's `Attach` (Debug text `Sora { bone: Some("kb:..."), .. }`).
fn anchor_of(attach: &str) -> Option<String> {
    let start = attach.find("Some(\"kb:")? + 6;
    let end = attach[start..].find('"')? + start;
    Some(attach[start..end].to_string())
}

#[derive(Default)]
struct Tally {
    /// effect -> (steps drawn, strips, strips on their bones, longest strip cm, most strips in one step)
    per: HashMap<String, (usize, usize, usize, f32, usize)>,
    /// effect -> the clips it was drawn in
    clips: HashMap<String, Vec<&'static str>>,
    /// effect -> spawns on a `kb:` anchor / spawns elsewhere
    spawns: HashMap<String, (usize, usize)>,
    /// the kb: node vs the old FK track (claws rows) on the steps at one of the clip's frames (the track holds the frames and
    /// blends their positions linearly between them): (compared, within 1 cm, the largest gap)
    node_gap: (usize, usize, f32),
    text: String,
}

struct Beams {
    /// live beam spawns: handle -> (effect, anchor, the step of its stop)
    live: HashMap<u32, (String, String, Option<u64>)>,
    defs: HashMap<String, (Vec<String>, String)>,
    t: Tally,
}

impl Beams {
    fn new(names: &[&str]) -> Beams {
        let defs = names.iter().filter_map(|n| Some((n.to_string(), beam_def(n)?))).collect();
        Beams { live: HashMap::new(), defs, t: Tally::default() }
    }

    /// After a step: record the step's beam spawns, then check every drawn strip of the beams' materials against the bones
    /// of the live beams' meshes.
    fn check(&mut self, s: &mut Session) {
        let row = s.rows.last().expect("a step row");
        for e in &row.effects {
            if e.stop.is_some() {
                if let Some(l) = self.live.get_mut(&e.handle) {
                    l.2 = l.2.or(Some(row.step));
                }
            }
            if e.stop.is_none() && self.defs.contains_key(&e.name) {
                let spawned = self.t.spawns.entry(e.name.clone()).or_default();
                match anchor_of(&e.attach) {
                    Some(a) => {
                        spawned.0 += 1;
                        self.live.insert(e.handle, (e.name.clone(), a, None));
                    }
                    None => spawned.1 += 1,
                }
            }
        }
        if self.live.is_empty() {
            return;
        }
        let (step, clip, time) = (s.rig.out.step, s.rig.out.anim_ticked.body_samples()[0].0, s.rig.out.anim_ticked.time);
        let out = s.rig.out.clone();
        let target = |_: kh_core::TargetId| None;
        let scene = Scene { out: &out, sora: &s.rig.sora, tap: &s.audio.tap, target: &target, ground: Some(0.0), eye: out.position + Vec3::new(-500.0, 0.0, 200.0) };
        // the links each live beam may draw: (effect, from, to)
        let mut links: Vec<(String, Vec3, Vec3)> = Vec::new();
        for (name, anchor, _) in self.live.values() {
            let Some((base, bone, to)) = split_anchor(anchor) else { continue };
            let Some((bones, _)) = self.defs.get(name) else { continue };
            let at = |b: &str| s.fx.socket_point(&scene, &format!("{base}{b}"));
            if bones.is_empty() {
                if let Some(to) = &to {
                    links.push((name.clone(), at(&bone), at(to)));
                }
            } else {
                for w in bones.windows(2) {
                    links.push((name.clone(), at(&w[0]), at(&w[1])));
                }
            }
            // the node: where the old FK track (the row's per-frame socket transform) put it, on a frame of the clip
            let on_frame = (time * 30.0 - (time * 30.0).round()).abs() < 1.0e-3;
            if let (true, Some(r)) = (on_frame, kh_core::combat::forms::claws::FORM_CLAWS.iter().find(|r| r.code > 1 && r.clip == clip && kh_core::combat::forms::claws::wfx_anchor(r) == Some(anchor.as_str()))) {
                let (fk, _) = kh_core::combat::forms::claws::wfx_world(r, time, out.position, out.yaw);
                let gap = fk.distance(at(&bone));
                if gap > 0.5 {
                    let _ = writeln!(self.t.text, "step {step} {clip} {time:.3}: {anchor} node {} vs the FK track {fk}: {gap:.2} cm", at(&bone));
                }
                let g = &mut self.t.node_gap;
                (g.0, g.1, g.2) = (g.0 + 1, g.1 + usize::from(gap < 1.0), g.2.max(gap));
            }
        }
        let camera = kh_vfx::Camera { position: out.position + Vec3::new(-400.0, 0.0, 150.0), forward: Vec3::X, right: Vec3::Y, up: Vec3::Z };
        let (list, materials, _) = s.fx.draw(1.0, &camera);
        let mut drawn: HashMap<String, (usize, usize, f32)> = HashMap::new();
        for r in &list.ribbons {
            let Some(material) = materials.get(r.material as usize).and_then(|m| m.rsplit(':').next()) else { continue };
            let names: Vec<&String> = self.defs.iter().filter(|(_, d)| d.1 == material).map(|(n, _)| n).collect();
            if names.is_empty() || r.points.len() < 2 {
                continue;
            }
            let mid = |p: &kh_vfx::RibbonPoint| (p.a + p.b) * 0.5;
            let (a, b) = (mid(&r.points[0]), mid(r.points.last().unwrap()));
            let hit = links.iter().find(|(n, f, t)| names.contains(&n) && f.distance(a) < 0.05 && t.distance(b) < 0.05);
            let name = hit.map_or_else(|| names[0].clone(), |h| h.0.clone());
            let d = drawn.entry(name).or_default();
            d.0 += 1;
            d.1 += usize::from(hit.is_some());
            d.2 = d.2.max(a.distance(b));
            if hit.is_none() {
                let _ = writeln!(self.t.text, "step {step} {clip} {time:.3}: a {material} strip {a} -> {b} on NO bone link");
            }
        }
        for (name, (n, ok, long)) in drawn {
            let p = self.t.per.entry(name.clone()).or_default();
            p.0 += 1;
            p.1 += n;
            p.2 += ok;
            p.3 = p.3.max(long);
            p.4 = p.4.max(n);
            let c = self.t.clips.entry(name.clone()).or_default();
            if !c.contains(&clip) {
                c.push(clip);
            }
            let _ = writeln!(self.t.text, "step {step} {clip} {time:.3}: {name} {n} strips, {ok} on their bone links, longest {long:.1} cm");
        }
        // a stopped beam plays out its LOOPEND tail (well under a second), then drops out
        self.live.retain(|_, l| l.2.is_none_or(|at| step < at + 90));
    }

    fn report(&self, title: &str) -> String {
        let mut out = format!("# {title}\n# effect: spawns on kb: / elsewhere | steps drawn, strips, strips on their bone links, longest cm, most strips in a step | clips\n");
        let mut names: Vec<&String> = self.defs.keys().collect();
        names.sort();
        for n in names {
            let sp = self.t.spawns.get(n).copied().unwrap_or_default();
            let p = self.t.per.get(n).copied().unwrap_or_default();
            let _ = writeln!(out, "{n}: spawns {} / {} | {} steps, {} strips, {} on links, {:.1} cm, {} at most | {:?}", sp.0, sp.1, p.0, p.1, p.2, p.3, p.4, self.t.clips.get(n).cloned().unwrap_or_default());
        }
        let g = self.t.node_gap;
        let _ = writeln!(out, "# the kb: node vs the old FK track (claws rows) on the clip frames: {} compared, {} within 1 cm, largest gap {:.3} cm", g.0, g.1, g.2);
        out.push_str(&self.t.text);
        out
    }
}

fn step(s: &mut Session, b: &mut Beams, held: &[PadButton]) -> StateId {
    let st = s.step(held);
    b.check(s);
    st
}

fn idle(s: &mut Session, b: &mut Beams, n: usize) {
    for _ in 0..n {
        step(s, b, &[]);
    }
}

fn transform(s: &mut Session, b: &mut Beams, cmd: i32) {
    s.rig.sora.debug_add_style(100.0);
    idle(s, b, 20);
    assert!(s.rig.sora.combat().situation.has(cmd), "{cmd:#x} offered");
    step(s, b, &[PadButton::SITUATION]);
    for _ in 0..600 {
        if step(s, b, &[]) != StateId::Stylechange && s.rig.sora.combat().form.cmd == cmd {
            return;
        }
    }
    panic!("STYLECHANGE to {cmd:#x} did not end");
}

fn combo(s: &mut Session, b: &mut Beams) {
    for k in 0..1200 {
        let st = if k % 2 == 0 { step(s, b, &[PadButton::ATTACK]) } else { step(s, b, &[]) };
        if k > 30 && matches!(st, StateId::Idle | StateId::Run | StateId::Jump) {
            break;
        }
    }
    idle(s, b, 30);
}

fn play(s: &mut Session, b: &mut Beams, clip: &str) {
    let id = kh_core::AnimId::from_name(clip).unwrap_or_else(|| panic!("{clip} in the animation table"));
    s.rig.sora.debug_play_anim(id, kh_core::PlayArgs::mode(0, 0.0));
    let mut seen = false;
    for _ in 0..400 {
        step(s, b, &[]);
        let now = s.rows.last().map(|x| x.clip);
        if now == Some(clip) {
            seen = true;
        } else if seen {
            break;
        }
    }
    assert!(seen, "{clip} played");
    idle(s, b, 10);
}

fn place_dummy(s: &mut Session, ahead: f32) {
    let o = s.rig.out.clone();
    let a = o.yaw.to_radians();
    let at = o.position + Vec3::new(a.cos(), a.sin(), 0.0) * ahead;
    if let Some(d) = s.rig.dummy.as_mut() {
        d.info.position = Vec3::new(at.x, at.y, 0.0);
        d.info.center = Vec3::new(at.x, at.y, 90.0);
    }
}

/// Every strip of a beam drawn lies on its own bone links; `names` were drawn. `strips` = KH3's count of links (one beam
/// per particle; Beam2's tick 0x144252AC0 spawns every missing beam at once with bAlwaysOn, kh-vfx sim.rs since (AUDIT-B
/// W-VFX2)): the whole string is drawn at once (several effects of the kind overlap: both yo-yos, LOOPEND tails).
fn assert_on_links(b: &Beams, names: &[(&str, usize)], report: &str) {
    for (name, strips) in names {
        let p = b.t.per.get(*name).copied().unwrap_or_default();
        let sp = b.t.spawns.get(*name).copied().unwrap_or_default();
        assert!(sp.0 > 0 && sp.1 == 0, "{name}: every spawn on its kb: anchor ({sp:?})\n{report}");
        assert!(p.0 > 5, "{name}: drawn ({} steps)\n{report}", p.0);
        assert_eq!(p.1, p.2, "{name}: every strip between its bones\n{report}");
        // (AUDIT-B W-VFX2) the whole string at once
        assert!(p.4 >= *strips, "{name}: {} strips at most (KH3: {strips} per beam effect)\n{report}", p.4);
        assert!(p.3 > 1.0, "{name}: the strips have length ({:.2} cm)\n{report}", p.3);
    }
}

#[test]
fn twin_yo_yos_strings_run_between_the_yo_yos_himo_bones() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_05", &["w_so060", "w_so061", "w_so062", "w_so063", "w_so064", "w_so065"], "w_so060", Some(Vec3::new(200.0, 0.0, 0.0)), "auditb_wvfx2_yoyo") else {
        eprintln!("SKIP: no pack");
        return;
    };
    let pack = audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    let mut b = Beams::new(&["vw_so064_bem_lin00", "vw_so064_bem_lin03", "vw_so064_gte_bem_lin01", "vw_so062_chg_thd07"]);
    idle(&mut s, &mut b, 5);
    transform(&mut s, &mut b, 0x8A);
    idle(&mut s, &mut b, 30);
    transform(&mut s, &mut b, 0x82);
    idle(&mut s, &mut b, 30);
    combo(&mut s, &mut b);
    for clip in ["4481", "4482", "4483"] {
        play(&mut s, &mut b, clip);
    }
    place_dummy(&mut s, 250.0);
    idle(&mut s, &mut b, 10);
    for _ in 0..30 {
        if s.rig.sora.combat().situation.slot_b.map(|e| e.kind) == Some(0x9C) {
            break;
        }
        s.rig.sora.debug_add_style(100.0);
        idle(&mut s, &mut b, 10);
    }
    assert_eq!(s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9C), "FINISH_YO on offer");
    step(&mut s, &mut b, &[PadButton::SITUATION]);
    for _ in 0..1500 {
        let st = step(&mut s, &mut b, &[]);
        if st != StateId::Stylefinish && s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    s.finish();
    let report = b.report("Happy Gear: Agile Claws (C401 sparks) -> Twin Yo-Yos (4401 strings, combo, 4481-4483, FINISH_YO 4420)");
    std::fs::write(traces().join("yoyo_beams.txt"), &report).ok();
    // bem_lin00: 17 bones -> 16 strips (MaxBeamCount 16) per yo-yo; gte_bem_lin01: 18 -> 17
    assert_on_links(&b, &[("vw_so064_bem_lin00", 16), ("vw_so064_gte_bem_lin01", 17)], &report);
    let fin = b.t.clips.get("vw_so064_bem_lin00").cloned().unwrap_or_default();
    assert!(fin.contains(&"4420"), "the giant yo-yo's string in FINISH_YO's M_4420: {fin:?}\n{report}");
    assert!(fin.iter().any(|c| c.starts_with("441")), "the combo's strings (M_441x): {fin:?}\n{report}");
    // the transformations' beams: spawned on their unit's mesh (w_so061); bem_lin03 / thd07 are drawn when their emitter
    // can be (thd07 has BeamNoise: drawn once kh-vfx evaluates the noise), and then on their links
    for name in ["vw_so064_bem_lin03", "vw_so062_chg_thd07"] {
        let sp = b.t.spawns.get(name).copied().unwrap_or_default();
        assert!(sp.0 > 0 && sp.1 == 0, "{name}: spawned on its kb: anchor ({sp:?})\n{report}");
        let p = b.t.per.get(name).copied().unwrap_or_default();
        assert_eq!(p.1, p.2, "{name}: any strip drawn lies between its bones\n{report}");
    }
    // (AUDIT-B W-VFX2) thd07's BeamNoise is evaluated now: its beam (two sheets) is drawn from socket to end socket
    let p = b.t.per.get("vw_so062_chg_thd07").copied().unwrap_or_default();
    assert!(p.0 > 0 && p.4 >= 2 && p.4 % 2 == 0, "vw_so062_chg_thd07: the noise beam is drawn, 2 sheets a beam ({p:?})\n{report}");
    // on the clip's frames the kb: node is where the old FK track put it (both: the weapon's socket under its clip on his
    // socket); between the frames the track blended positions linearly and missed the drawn blend between two plays,
    // which the host follows
    let g = b.t.node_gap;
    assert!(g.0 > 100 && g.1 * 100 >= g.0 * 95, "the kb: node on the FK track ({g:?})\n{report}");
}

#[test]
fn nano_arms_yo_yo_lines_run_between_w_so172s_himo_bones() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_08", &["w_so170", "w_so172", "w_so173", "w_so174"], "w_so170", Some(Vec3::new(200.0, 0.0, 0.0)), "auditb_wvfx2_nano") else {
        eprintln!("SKIP: no pack");
        return;
    };
    let mut b = Beams::new(&["vw_so172_yoy_lin00", "vw_so172_yoy_lin01"]);
    idle(&mut s, &mut b, 5);
    transform(&mut s, &mut b, 0x86);
    idle(&mut s, &mut b, 30);
    for clip in ["8414", "8423", "8424"] {
        play(&mut s, &mut b, clip);
    }
    s.finish();
    let report = b.report("Nano Arms: the yo-yos 8414, the hookshot 8423 / 8424");
    std::fs::write(traces().join("nano_beams.txt"), &report).ok();
    // 17 bones (R_wep / L_wep, R/L_y_himo01..16) -> 16 strips each
    assert_on_links(&b, &[("vw_so172_yoy_lin00", 16), ("vw_so172_yoy_lin01", 16)], &report);
    let clips = b.t.clips.get("vw_so172_yoy_lin01").cloned().unwrap_or_default();
    assert!(clips.contains(&"8414") && clips.contains(&"8423"), "the yo-yo attack and the hookshot: {clips:?}\n{report}");
}

/// (AUDIT-B W-VFX2) Hero's Origin's lightning (AUDIT-A's Counter Shield): the transform unit w_so031's own B401
/// (StyleChange11) notifies vw_so030_und (vfx_thd00 -> L_thund3, vfx_thd01 -> R_thund3), vw_so030_top (vfx_thd02 ->
/// vfx_thd03, vfx_thd04 -> vfx_thd05) and vw_so030_naka (Z_03 -> the four stone groups), and its GuardChange B285 / B286
/// vw_so030_top rows (six stones -> center): BeamNoise beams (kh-vfx `noise_strip`), on the `kb:w_so031:R_wep_skt:..:to..`
/// anchors tools/sora_form_shield.js now writes for them. Every drawn strip starts on its socket and ends on its
/// EndSocketName (the noise moves the points between), up to MaxBeamCount beams x 2 sheets per effect.
#[test]
fn heros_origin_lightning_runs_between_w_so031s_sockets() {
    const HERO: &[&str] = &["w_so030", "w_so036", "w_so035", "w_so032", "w_so031", "w_so033", "w_so037", "w_so034"];
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_01", HERO, "w_so030", Some(Vec3::new(500.0, 0.0, 0.0)), "auditb_wvfx2_hero") else {
        eprintln!("SKIP: no pack");
        return;
    };
    let mut b = Beams::new(&["vw_so030_und", "vw_so030_top", "vw_so030_naka"]);
    idle(&mut s, &mut b, 5);
    transform(&mut s, &mut b, 0x89);
    idle(&mut s, &mut b, 30);
    for clip in ["B285", "B286"] {
        play(&mut s, &mut b, clip);
    }
    s.finish();
    let report = b.report("Hero's Origin: Counter Shield's transformation B401 (w_so031), GuardChange B285 / B286");
    std::fs::write(traces().join("hero_beams.txt"), &report).ok();
    for (name, beams) in [("vw_so030_und", 2), ("vw_so030_top", 2), ("vw_so030_naka", 3)] {
        let sp = b.t.spawns.get(name).copied().unwrap_or_default();
        let p = b.t.per.get(name).copied().unwrap_or_default();
        assert!(sp.0 > 0 && sp.1 == 0, "{name}: every spawn on its kb: anchor ({sp:?})\n{report}");
        assert!(p.0 > 3, "{name}: drawn ({} steps)\n{report}", p.0);
        assert_eq!(p.1, p.2, "{name}: every strip from its socket to its EndSocketName\n{report}");
        assert!(p.4 >= 2 && p.4 % 2 == 0 && p.4 >= beams * 2 - 2, "{name}: {} strips at most (MaxBeamCount {beams} x 2 sheets per effect)\n{report}", p.4);
    }
    let clips = b.t.clips.get("vw_so030_top").cloned().unwrap_or_default();
    assert!(clips.contains(&"B401") && clips.contains(&"B285"), "the transformation's and the guard change's: {clips:?}\n{report}");
}

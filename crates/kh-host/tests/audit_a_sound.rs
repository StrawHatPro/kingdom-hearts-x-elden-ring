//! (AUDIT-A SOUND) HOW THE HOSTS TURN EVERY SOUND NAME OF AUDIT-A'S FORMS INTO A REAL ASSET (Second Form L1 / L2 / L5,
//! Counter Shield, Frying Pan, Double Arrowguns, Magic Launcher, Hunny Blasters, Hunny Launcher, Mirage Staff).
//!
//! 1. THE STATIC INVENTORY (`every_form_sound_resolves_to_kh3s_asset`): every clip the forms play (their replacement anim
//!    sets `p_ex001_AnimSet2 / 5 / 10 / 11 / 16 / 17 / 19`, the StyleChange clips, Second Form's main-set clips, the
//!    finishers, M_1501), every sound / voice / weapon-SE / effect notify of it, resolved through Sora's real routes
//!    (`kh_audio::Routes`, the live audio pack) with what the host knows when it fires (the row the core plays, its time,
//!    the weapon on the notify's unit) and compared with KH3's own data: the asset is packed and playable; a weighted
//!    random voice list is found as ITS list (else the host plays its first entry every time instead of KH3's draw
//!    0x14056CAE0); a weapon SE plays from the set of the weapon actor on the notify's unit (`m_SoundAliasUnit` ->
//!    pawn vt+0x1428 -> that actor's `m_SoundAssets_Swing` / `_Hit`, 0x14178BED0 / 0x14178BEB0: no set = silence); every
//!    particle system the notifies spawn carries in the routes exactly the sounds of its own `ParticleSeAssets`
//!    (`SQEX_EffectSeAttachDataAsset` SEDatas, read from the converted UE assets).
//! 2. THE RECORDER SESSIONS (`*_session_sounds`): each form through the real core, effect side and audio side
//!    (`kh_host::audit_a::Session`, its sound sets following kh-er's `select_sounds`): every sound notify the played clips
//!    fired is emitted AND played (or silent by KH3's own data), every random voice drew from its list, no seam name is
//!    without a route, every voice-group voice found its unit.
//!
//! Reports: build/sora_tmp/weapons/AUDIT-A/logs/sound_static.report.txt, sound_<form>.report.txt
//! (build/sora_tmp/weapons/AUDIT-A/SOUND.md). Skipped, saying so, without the pack.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use glam::Vec3;
use kh_audio::routes::{localized, Context, Resolved};
use kh_audio::{Language, Pack, Routes};
use kh_core::anim::{AnimId, NotifyData, FORM_ANIM_SETS, SEQUENCES};
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{self, coverage, Session};

/// One form of AUDIT-A: its clips and the weapon models on its units.
struct Form {
    id: &'static str,
    /// The anim sets it replaces with (`m_ReplaceAnimSets`).
    sets: &'static [&'static str],
    /// The other rows it plays (AnimData names of the main table).
    rows: &'static [&'static str],
    /// RH_WEAPON / LH_WEAPON: the form weapons' models (the actors of those equip parts during the form).
    rh: &'static str,
    lh: Option<&'static str>,
}

const SECOND_ROWS: &[&str] = &[
    "StyleChange", "StyleChangeLoop", "StyleChangeEnd", "StyleChangeEasy", "M_1411", "M_1412", "M_1413", "M_1414", "M_1415", "M_1416", "M_1421", "M_1422", "M_1423", "M_1424", "M_1425", "M_1426", "M_1427", "M_1428", "M_1429", "M_1430",
    "M_1431", "M_1432", "M_1433", "M_1434", "M_1435", "M_1501",
];

const FORMS: &[Form] = &[
    Form { id: "L1", sets: &[], rows: SECOND_ROWS, rh: "w_so010", lh: None },
    Form { id: "L2", sets: &[], rows: SECOND_ROWS, rh: "w_so150", lh: None },
    Form { id: "L5", sets: &[], rows: SECOND_ROWS, rh: "w_so121", lh: None },
    Form { id: "SH", sets: &["p_ex001_AnimSet11"], rows: &["StyleChange11", "M_1501"], rh: "w_so032", lh: None },
    Form { id: "SH2", sets: &["p_ex001_AnimSet11", "p_ex001_AnimSet19"], rows: &["StyleChange19", "M_1501"], rh: "w_so142", lh: None },
    Form { id: "DW", sets: &["p_ex001_AnimSet2"], rows: &["StyleChange2"], rh: "w_so022", lh: Some("w_so023") },
    Form { id: "BZ", sets: &["p_ex001_AnimSet5"], rows: &["StyleChange5"], rh: "w_so024", lh: None },
    Form { id: "DW2", sets: &["p_ex001_AnimSet2", "p_ex001_AnimSet16"], rows: &["StyleChange16"], rh: "w_so042", lh: Some("w_so043") },
    Form { id: "BZ2", sets: &["p_ex001_AnimSet5", "p_ex001_AnimSet17"], rows: &["StyleChange17"], rh: "w_so044", lh: None },
    Form { id: "WA", sets: &["p_ex001_AnimSet10"], rows: &["StyleChange10", "M_1501"], rh: "w_so072", lh: None },
];

fn project() -> PathBuf {
    audit_a::project()
}

fn pack() -> Option<Pack> {
    let dir = project().join("build").join("sora_pack").join("audio");
    if !dir.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no audio pack at {}", dir.display());
        return None;
    }
    let mut pack = Pack::load(&dir).expect("the audio pack");
    // the roots kh-er fx.rs merges for Sora: the XS clips' sounds and the keyblade forms' (PACK/keyblades/audio)
    let root = project().join("build").join("sora_pack");
    for extra in [root.join("xsi").join("audio"), root.join("keyblades").join("audio")] {
        pack.merge_party_root(&extra).expect("an audio root merges");
    }
    Some(pack)
}

/// The asset is in the pack, its sound is in its bank, and a material of the sound is a file of the pack: it plays.
fn playable(pack: &Pack, path: &str) -> Result<String, String> {
    let Some(asset) = pack.asset(path) else { return Err(format!("{path}: not in the pack")) };
    let Some(def) = pack.sound(asset.bank, asset.index) else { return Err(format!("{path}: sound {} of bank {} missing", asset.index, pack.bank(asset.bank).name)) };
    let bank = pack.bank(asset.bank);
    let file = def.tracks.values().filter_map(|t| bank.materials.get(&t.material)).find(|m| pack.dir.join(&m.file).is_file());
    match file {
        Some(m) => Ok(format!("{}#{} ({})", bank.name, asset.index, m.file)),
        None if def.tracks.is_empty() => Ok(format!("{}#{} (no track: silent by its own data)", bank.name, asset.index)),
        None => Err(format!("{path}: no material file of {}#{}", bank.name, asset.index)),
    }
}

/// Every row (AnimId) a form plays: its sets' rows, then its own rows; each once.
fn rows_of(form: &Form) -> (Vec<(String, AnimId)>, Vec<String>) {
    let mut out: Vec<(String, AnimId)> = Vec::new();
    let mut missing = Vec::new();
    for set in form.sets {
        let Some(s) = FORM_ANIM_SETS.iter().find(|s| s.name == *set) else {
            missing.push(format!("anim set {set}"));
            continue;
        };
        for (logical, id) in s.rows {
            if !out.iter().any(|(_, o)| o == id) {
                out.push((logical.to_string(), *id));
            }
        }
    }
    for row in form.rows {
        match AnimId::from_name(row) {
            Some(id) if !out.iter().any(|(_, o)| *o == id) => out.push((row.to_string(), id)),
            Some(_) => {}
            None => missing.push(format!("row {row}")),
        }
    }
    (out, missing)
}

/// KH3's SeAttach sounds of every particle system (by lower-case name): (sound asset, at end) from the PS's
/// `ParticleSeAssets` -> `SQEX_EffectSeAttachDataAsset.SEDatas` (the converted UE assets). None: no converted assets.
struct Kh3Effects {
    ps: HashMap<String, PathBuf>,
    cache: HashMap<String, Option<Vec<(String, bool)>>>,
}

impl Kh3Effects {
    fn load() -> Option<Kh3Effects> {
        let content = project().join("assets_converted").join("packages").join("KINGDOM HEARTS III").join("Content");
        let root = content.join("Effects");
        if !root.is_dir() {
            return None;
        }
        let mut ps = HashMap::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if let Some(name) = p.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".uasset.json")) {
                    ps.entry(name.to_ascii_lowercase()).or_insert(p);
                }
            }
        }
        Some(Kh3Effects { ps, cache: HashMap::new() })
    }

    fn json(path: &Path) -> Option<serde_json::Value> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    /// The sounds the particle system `name` carries in KH3 (None: no such PS among the converted assets).
    fn sounds(&mut self, name: &str) -> Option<Vec<(String, bool)>> {
        let key = name.rsplit(['/', '.']).next().unwrap_or(name).to_ascii_lowercase();
        if let Some(c) = self.cache.get(&key) {
            return c.clone();
        }
        let found = self.ps.get(&key).cloned().map(|p| {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let mut out = Vec::new();
            let mut at = 0;
            while let Some(i) = text[at..].find("/Game/Sound/SE/_Data/SeAttach/") {
                let start = at + i;
                let end = text[start..].find('"').map_or(text.len(), |e| start + e);
                let reference = &text[start..end];
                at = end;
                let rel = reference.trim_start_matches("/Game/");
                let rel = rel.split('.').next().unwrap_or(rel);
                let file = project().join("assets_converted").join("packages").join("KINGDOM HEARTS III").join("Content").join(format!("{rel}.uasset.json"));
                for s in Self::se_attach(&file) {
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
            // an effect attach data list (ead_*): the particle systems of its rows carry the sounds
            if key.starts_with("ead_") {
                let mut at = 0;
                while let Some(i) = text[at..].find("/Game/Effects/") {
                    let start = at + i;
                    let end = text[start..].find('"').map_or(text.len(), |e| start + e);
                    let child = text[start..end].rsplit(['/', '.']).next().unwrap_or("").to_ascii_lowercase();
                    at = end;
                    if child.starts_with("ead_") || child == key {
                        continue;
                    }
                    if let Some(p) = self.ps.get(&child).cloned() {
                        let t = std::fs::read_to_string(&p).unwrap_or_default();
                        let mut a = 0;
                        while let Some(j) = t[a..].find("/Game/Sound/SE/_Data/SeAttach/") {
                            let s0 = a + j;
                            let e0 = t[s0..].find('"').map_or(t.len(), |e| s0 + e);
                            let rel = t[s0..e0].trim_start_matches("/Game/");
                            let rel = rel.split('.').next().unwrap_or(rel);
                            a = e0;
                            let file = project().join("assets_converted").join("packages").join("KINGDOM HEARTS III").join("Content").join(format!("{rel}.uasset.json"));
                            for s in Self::se_attach(&file) {
                                let tagged = (format!("{} (via {child})", s.0), s.1);
                                if !out.contains(&tagged) {
                                    out.push(tagged);
                                }
                            }
                        }
                    }
                }
            }
            out
        });
        self.cache.insert(key, found.clone());
        found
    }

    /// The SEDatas of an SeAttach asset: (sound package path, bPlayAtEnd).
    fn se_attach(file: &Path) -> Vec<(String, bool)> {
        let Some(d) = Self::json(file) else { return Vec::new() };
        let imports = d["imports"].as_array().cloned().unwrap_or_default();
        let resolve = |index: i64| -> Option<String> {
            let imp = imports.get(usize::try_from(-index - 1).ok()?)?;
            let outer = imp["outerIndex"].as_i64()?;
            let pkg = imports.get(usize::try_from(-outer - 1).ok()?)?;
            pkg["objectName"].as_str().map(str::to_string)
        };
        let mut out = Vec::new();
        for e in d["exports"].as_array().into_iter().flatten() {
            if e["class"].as_str() != Some("SQEX_EffectSeAttachData") {
                continue;
            }
            let props = e["data"]["props"].as_array().cloned().unwrap_or_default();
            let get = |n: &str| props.iter().find(|p| p["name"].as_str() == Some(n)).map(|p| p["value"].clone());
            let Some(sound) = get("Sound").and_then(|v| v.as_i64()).and_then(resolve) else { continue };
            let at_end = get("bPlayAtEnd").and_then(|v| v.as_bool()).unwrap_or(false);
            out.push((sound, at_end));
        }
        out
    }
}

fn cx(anim: Option<(&str, f32)>) -> Context<'_> {
    Context { surface: 0, language: Language::En, voice_group_no: 0, anim }
}

#[derive(Default)]
struct Tally {
    checked: usize,
    failed: Vec<String>,
    lines: Vec<String>,
}

impl Tally {
    fn row(&mut self, ok: bool, line: String) {
        self.checked += 1;
        if !ok {
            self.failed.push(line.clone());
        }
        self.lines.push(format!("{} {line}", if ok { "PASS" } else { "FAIL" }));
    }
}

/// The weapon set the port's routes play for `model` (kh-er `kbdraw::select_sounds`): `Routes::select_weapon_kh3`.
fn routes_for(base: &Routes, model: &str) -> Routes {
    let mut r = base.clone();
    r.select_weapon_kh3(model);
    r
}

/// Checks every sound notify of one form; returns its tally.
fn check_form(pack: &Pack, base: &Routes, kh3: &mut Option<Kh3Effects>, form: &Form) -> Tally {
    let mut t = Tally::default();
    let (rows, missing) = rows_of(form);
    for m in missing {
        t.row(false, format!("{}: {m} is not in the core's anim table", form.id));
    }
    let rh = routes_for(base, form.rh);
    let lh = form.lh.map(|m| routes_for(base, m));
    for (logical, id) in &rows {
        let data = id.data();
        for &(_, index) in data.samples {
            let seq = &SEQUENCES[index];
            for n in seq.notifies {
                let at = format!("{} {} ({logical} = row {}, clip {}) @{:.3}", form.id, n.class.game_name(), data.name, seq.name, n.time);
                // the host's context when this notify fires: the row the core plays and its time
                let here = Some((data.name, n.time));
                match n.data {
                    NotifyData::Sound { sound, loop_end_sound, .. } => {
                        for s in std::iter::once(sound).chain(loop_end_sound) {
                            match base.resolve(pack, s, &cx(here)) {
                                Resolved::Asset { path, .. } => {
                                    let p = playable(pack, &path);
                                    t.row(p.is_ok(), format!("{at} sound {s} -> {}", p.unwrap_or_else(|e| e)));
                                }
                                other => t.row(false, format!("{at} sound {s} -> {other:?}")),
                            }
                        }
                    }
                    NotifyData::Voice { group, asset, random, .. } => {
                        if let Some(asset) = asset {
                            match base.resolve(pack, asset, &cx(here)) {
                                Resolved::Asset { path, voice: true } => {
                                    let en = playable(pack, &path);
                                    let ja = playable(pack, &localized(asset, Language::Ja));
                                    t.row(en.is_ok() && ja.is_ok(), format!("{at} voice {asset} -> en {} | ja {}", en.unwrap_or_else(|e| e), ja.unwrap_or_else(|e| e)));
                                }
                                other => t.row(false, format!("{at} voice {asset} -> {other:?}")),
                            }
                        } else if let Some(group) = group {
                            let units = base.voice_groups.get(group).map_or(0, Vec::len);
                            let all: Vec<String> = base.voice_groups.get(group).into_iter().flatten().map(|(no, a)| format!("{no}={}", playable(pack, &localized(a, Language::En)).map_or_else(|e| format!("MISSING {e}"), |_| a.rsplit('/').next().unwrap_or(a).to_string()))).collect();
                            t.row(units > 0 && !all.iter().any(|a| a.contains("MISSING")), format!("{at} voice group {group}: units {}", all.join(" ")));
                        } else if let Some(first) = random.first() {
                            match base.resolve(pack, first.0, &cx(here)) {
                                Resolved::RandomVoice { index } => {
                                    let list = &base.random_voices[index];
                                    let same = list.entries.len() == random.len() && list.entries.iter().zip(random.iter()).all(|(a, b)| a.0 == kh_audio::pack::asset_key(b.0) && i64::from(a.1) == i64::from(b.1));
                                    let bad: Vec<String> = list.entries.iter().filter_map(|e| playable(pack, &localized(&e.0, Language::En)).err()).collect();
                                    t.row(same && bad.is_empty(), format!("{at} random voice x{} -> list {} ({}){}", random.len(), list.id, list.entries.iter().map(|e| format!("{}:{}", e.0.rsplit('/').next().unwrap_or(&e.0), e.1)).collect::<Vec<_>>().join(" "), if same { String::new() } else { " NOT KH3'S LIST".to_string() } + &bad.join("; ")));
                                }
                                Resolved::Asset { path, .. } if random.len() == 1 => {
                                    let p = playable(pack, &path);
                                    t.row(p.is_ok(), format!("{at} random voice x1 {} -> {}", first.0, p.unwrap_or_else(|e| e)));
                                }
                                other => t.row(false, format!("{at} random voice x{} {} -> {other:?}: KH3's weighted draw is lost (the first entry always)", random.len(), first.0)),
                            }
                        }
                    }
                    NotifyData::WeaponSe { hit, alias, unit, .. } => {
                        let (routes, model) = match unit {
                            "LH_WEAPON" => (lh.as_ref().unwrap_or(&rh), form.lh.unwrap_or(form.rh)),
                            _ => (&rh, form.rh),
                        };
                        let has_set = base.weapon_sets.contains_key(model);
                        let res = routes.resolve(pack, alias, &cx(here));
                        let line = format!("{at} {} {alias} on {unit} ({model}{})", if hit { "hit-se" } else { "swing-se" }, if has_set { "" } else { ": no set in KH3" });
                        match res {
                            Resolved::Asset { path, .. } => {
                                let p = playable(pack, &path);
                                t.row(has_set && p.is_ok(), format!("{line} -> {}", p.unwrap_or_else(|e| e)));
                            }
                            Resolved::Silent(why) => t.row(true, format!("{line} -> silent ({why})")),
                            other => t.row(false, format!("{line} -> {other:?}")),
                        }
                    }
                    NotifyData::AutoSe { motion, source, .. } => {
                        let name = motion.or(source).unwrap_or("");
                        let r = base.resolve(pack, name, &cx(here));
                        t.row(!matches!(r, Resolved::Unknown), format!("{at} auto-se {name} -> {r:?}"));
                    }
                    NotifyData::Effect { entries } => {
                        for e in entries {
                            let Some(effect) = e.effect else { continue };
                            let short = effect.rsplit(['/', '.']).next().unwrap_or(effect).to_ascii_lowercase();
                            let port: Vec<(String, bool)> = base.effect_sounds(&short).iter().map(|s| (kh_audio::pack::asset_key(&s.asset).to_string(), s.at_end)).collect();
                            let Some(k) = kh3.as_mut() else { continue };
                            let Some(want) = k.sounds(&short) else {
                                t.row(port.is_empty(), format!("{at} effect {short}: no such particle system among the converted assets; port {port:?}"));
                                continue;
                            };
                            if want.is_empty() && port.is_empty() {
                                continue;
                            }
                            let same = want.len() == port.len() && want.iter().all(|w| port.contains(w));
                            let bad: Vec<String> = port.iter().filter_map(|p| playable(pack, &p.0).err()).collect();
                            t.row(same && bad.is_empty(), format!("{at} effect {short} carries {want:?} -> port {port:?} {}", bad.join("; ")));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    t
}

#[test]
fn every_form_sound_resolves_to_kh3s_asset() {
    let Some(pack) = pack() else { return };
    // Sora's routes as the hosts build them (HostAudio: the pack's + the core clips' random lists)
    let mut base = Routes::new(&pack);
    let added = kh_host::audio::core_lists::add_core_random_voices(&mut base);
    eprintln!("{added} random voice lists of the core's clips added to the pack's {}", base.random_voices.len() - added);
    let mut kh3 = Kh3Effects::load();
    if kh3.is_none() {
        eprintln!("(no converted UE effects: the effect-carried sounds are not compared)");
    }
    let mut text = String::new();
    let mut failed = Vec::new();
    let mut counts = BTreeMap::new();
    for form in FORMS {
        let t = check_form(&pack, &base, &mut kh3, form);
        let _ = writeln!(text, "# form {} ({} checked, {} failed)", form.id, t.checked, t.failed.len());
        for l in &t.lines {
            let _ = writeln!(text, "{l}");
        }
        counts.insert(form.id, (t.checked, t.failed.len()));
        failed.extend(t.failed);
    }
    let _ = writeln!(text, "# totals {counts:?}");
    std::fs::write(audit_a::path_in_logs("sound_static.report.txt"), &text).ok();
    eprintln!("sound static inventory: {counts:?}");
    assert!(failed.is_empty(), "{} sound rows failed:\n{}", failed.len(), failed.join("\n"));
}

/// Every particle system of the effect folders of AUDIT-A's forms' weapons (and the Limit / Second Form folder) that carries a
/// sound in KH3 carries exactly it in Sora's routes, whether or not today's port spawns it (a form worker who adds an effect
/// finds its sound already routed).
#[test]
fn every_form_weapon_effect_carries_its_kh3_sound() {
    let Some(pack) = pack() else { return };
    let routes = Routes::new(&pack);
    let Some(mut kh3) = Kh3Effects::load() else {
        eprintln!("SKIPPED: no converted UE effects");
        return;
    };
    let dirs = ["w_so010", "w_so020", "w_so030", "w_so040", "w_so070", "w_so120", "w_so140", "w_so150", "limit"];
    let mut names: Vec<String> = kh3
        .ps
        .iter()
        .filter(|(_, p)| {
            let s = p.to_string_lossy().replace('\\', "/");
            dirs.iter().any(|d| s.contains(&format!("/Effects/wep/{d}/")))
        })
        .map(|(n, _)| n.clone())
        .collect();
    names.sort();
    let (mut checked, mut failed, mut text) = (0usize, Vec::new(), String::new());
    for n in &names {
        let Some(want) = kh3.sounds(n) else { continue };
        if want.is_empty() || n.starts_with("ead_") {
            continue;
        }
        checked += 1;
        let port: Vec<(String, bool)> = routes.effect_sounds(n).iter().map(|s| (kh_audio::pack::asset_key(&s.asset).to_string(), s.at_end)).collect();
        let bad: Vec<String> = port.iter().filter_map(|p| playable(&pack, &p.0).err()).collect();
        let ok = want.len() == port.len() && want.iter().all(|w| port.contains(w)) && bad.is_empty();
        let _ = writeln!(text, "{} {n}: KH3 {want:?} -> routes {port:?} {}", if ok { "PASS" } else { "FAIL" }, bad.join("; "));
        if !ok {
            failed.push(format!("{n}: KH3 {want:?} -> routes {port:?} {}", bad.join("; ")));
        }
    }
    std::fs::write(audit_a::path_in_logs("sound_effects.report.txt"), &text).ok();
    eprintln!("form weapons' effects with a KH3 sound: {checked} checked, {} failed", failed.len());
    assert!(checked > 100, "the effect folders were read ({checked})");
    assert!(failed.is_empty(), "{} effects:\n{}", failed.len(), failed.join("\n"));
}

/// The voice notifies of the form pawns' own clips (the Mirage Staff tower w_so073's A417: a 50 / 50 list of bt1010740 /
/// bt1010750 at 0.126 s and bt1010770 at 4.293 s, KH3's clip `/Game/Character/wep/w_so073/anm/rt/A417`) resolve as KH3's:
/// the list under Sora's row A417 (the core plays the pawn's notifies during it), the line as its asset.
#[test]
fn the_tower_pawns_voice_lines_resolve_as_kh3s() {
    let Some(pack) = pack() else { return };
    let routes = Routes::new(&pack);
    let first = "/Game/Sound/VOICE/Localization/ja/battle/PC/bt1010740sr0.bt1010740sr0";
    match routes.resolve(&pack, first, &cx(Some(("A417", 0.126)))) {
        Resolved::RandomVoice { index } => {
            let l = &routes.random_voices[index];
            let names: Vec<(&str, u32)> = l.entries.iter().map(|e| (e.0.rsplit('/').next().unwrap_or(""), e.1)).collect();
            assert_eq!(names, [("bt1010740sr0", 50), ("bt1010750sr0", 50)], "{l:?}");
            for e in &l.entries {
                playable(&pack, &localized(&e.0, Language::En)).expect("the en line is packed");
            }
        }
        other => panic!("the tower's list is not found: {other:?} (build/sora_pack/keyblades/audio rows forms.notify.w_so073.*)"),
    }
    let line = "/Game/Sound/VOICE/Localization/ja/battle/PC/bt1010770sr0.bt1010770sr0";
    let Resolved::Asset { path, voice: true } = routes.resolve(&pack, line, &cx(Some(("A417", 4.293)))) else { panic!("bt1010770 is a voice asset") };
    playable(&pack, &path).expect("bt1010770 en is packed");
}

/// (for AUDIT-B's generic seam of the form weapons' and pawns' OWN clip notifies) Every sound a clip of a keyblade model
/// names (build/sora_pack/keyblades/<w_soNNN>/model.json clips[].notifies: PlaySound / AttachSound / PlayVoice / an Attack
/// window's m_HitSEAsset) resolves through Sora's routes, as the hosts build them, to a packed, playable asset.
#[test]
fn every_keyblade_model_clip_sound_resolves() {
    let Some(pack) = pack() else { return };
    let routes = Routes::new(&pack);
    let dir = project().join("build").join("sora_pack").join("keyblades");
    let Ok(models) = std::fs::read_dir(&dir) else { return };
    let (mut checked, mut failed, mut text) = (0usize, Vec::new(), String::new());
    let mut models: Vec<PathBuf> = models.flatten().map(|e| e.path().join("model.json")).filter(|p| p.is_file()).collect();
    models.sort();
    for file in models {
        let Some(j) = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else { continue };
        let model = file.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("?").to_string();
        for clip in j["clips"].as_array().into_iter().flatten() {
            let seq = clip["sequence"].as_str().unwrap_or("?");
            for n in clip["notifies"].as_array().into_iter().flatten() {
                let props = n["properties"].to_string();
                let mut at = 0;
                while let Some(i) = props[at..].find("/Game/Sound/") {
                    let start = at + i;
                    let end = props[start..].find('"').map_or(props.len(), |e| start + e);
                    let name = &props[start..end];
                    at = end;
                    checked += 1;
                    let time = n["trigger_time"].as_f64().unwrap_or(0.0) as f32;
                    let r = routes.resolve(&pack, name, &cx(Some((seq, time))));
                    let res = match &r {
                        Resolved::Asset { path, .. } => playable(&pack, path),
                        Resolved::RandomVoice { index } => routes.random_voices[*index].entries.iter().map(|e| playable(&pack, &localized(&e.0, Language::En))).collect::<Result<Vec<_>, _>>().map(|v| v.join(" ")),
                        other => Err(format!("{other:?}")),
                    };
                    let line = format!("{model} {seq} {} @{time:.3} {name} -> {}", n["class"].as_str().unwrap_or("?"), res.clone().unwrap_or_else(|e| e));
                    let _ = writeln!(text, "{} {line}", if res.is_ok() { "PASS" } else { "FAIL" });
                    if res.is_err() {
                        failed.push(line);
                    }
                }
            }
        }
    }
    std::fs::write(audit_a::path_in_logs("sound_model_clips.report.txt"), &text).ok();
    eprintln!("keyblade model clip sounds: {checked} checked, {} failed", failed.len());
    assert!(checked > 100, "the models' clips were read ({checked})");
    assert!(failed.is_empty(), "{} sounds:
{}", failed.len(), failed.join("
"));
}

// ------------------------------------------------------------------------------------------------ the recorder sessions

/// A session driven like a player (the recorder follows kh-er's swing / hit set choice itself: `Session::step_input`).
struct Run {
    s: Session,
}

impl Run {
    fn step(&mut self, held: &[PadButton]) -> StateId {
        self.s.step(held)
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn transform(&mut self, cmd: i32) -> bool {
        self.s.rig.sora.debug_add_style(100.0);
        self.idle(20);
        if !self.s.rig.sora.combat().situation.has(cmd) {
            return false;
        }
        self.step(&[PadButton::SITUATION]);
        for _ in 0..700 {
            if self.step(&[]) != StateId::Stylechange && self.s.rig.sora.combat().form.cmd == cmd {
                return true;
            }
        }
        false
    }

    fn combo(&mut self) {
        for k in 0..900 {
            let state = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 40 && matches!(state, StateId::Idle | StateId::Run) {
                break;
            }
        }
        self.idle(40);
    }

    /// The form's finish (situation), its extras pressed whenever offered, until the form is over.
    fn finish(&mut self) {
        self.step(&[PadButton::SITUATION]);
        for k in 0..3000 {
            let press = self.s.rig.sora.combat().situation.extra.is_some() && k % 2 == 0;
            if press {
                self.step(&[PadButton::SITUATION]);
            } else {
                self.step(&[]);
            }
            if k > 30 && self.s.rig.sora.combat().form.cmd == 0 && !matches!(self.s.rig.out.state, StateId::Stylefinish) && self.s.rig.sora.finish().is_none() {
                break;
            }
        }
        self.idle(150);
    }
}

/// One form's session: offer -> transformation -> combo (x2) -> the finish when offered -> the end of the form.
fn session(weapon: &str, models: &[&str], base: &'static str, cmds: &[i32], log: &str) -> Option<Session> {
    let s = Session::new(weapon, models, base, Some(Vec3::new(250.0, 0.0, 0.0)), log)?;
    let mut r = Run { s };
    r.idle(5);
    for &cmd in cmds {
        assert!(r.transform(cmd), "{log}: the form {cmd:#x} starts");
        r.idle(30);
        r.combo();
        r.combo();
    }
    for _ in 0..4 {
        if r.s.rig.sora.combat().situation.slot_b.is_some() || r.s.rig.sora.combat().form.cmd == 0 {
            break;
        }
        r.s.rig.sora.debug_add_style(100.0);
        r.combo();
    }
    if r.s.rig.sora.combat().form.cmd != 0 {
        r.finish();
    }
    r.idle(60);
    r.s.finish();
    Some(r.s)
}

/// The sound rows of a finished session: every sound notify PASS, random voices drawn from their list, no name without a
/// route, every voice group unit found.
fn judge(s: &Session, log: &str) -> Vec<String> {
    let mut failed = Vec::new();
    let mut text = String::new();
    for c in coverage(&s.rows).iter().filter(|c| !matches!(c.want, audit_a::Want::Effect { .. })) {
        let line = format!("step {:5} {} @{:.3} {} {} -> {} {}", c.step, c.clip, c.time, c.want.kind(), c.want.seam_name(), if c.emitted { "emitted" } else { "NOT EMITTED" }, c.resolved);
        let random_ok = match &c.want {
            audit_a::Want::Voice { random, .. } if *random > 1 => c.resolved.contains("random voice") || !c.ok,
            _ => true,
        };
        let ok = c.ok && random_ok;
        let _ = writeln!(text, "{} {line}{}", if ok { "PASS" } else { "FAIL" }, if random_ok { "" } else { " (the random list was not found: its first entry always)" });
        if !ok {
            failed.push(line);
        }
    }
    let _ = writeln!(text, "# every audio line that is not a play (silent / unknown / voice group / random voice)");
    for r in &s.rows {
        for a in &r.audio {
            if a.0 == "play" || a.0 == "voice" {
                continue;
            }
            let _ = writeln!(text, "step {:5} {:?} {} {} {} {} {}", r.step, r.state, r.clip, a.0, a.1, a.2, a.3);
            if a.0 == "unknown" || a.3.contains("no unit with the pawn's voice group number") {
                failed.push(format!("step {} {} {} -> {} {}", r.step, r.clip, a.1, a.0, a.3));
            }
        }
    }
    for name in &s.audio.unknown {
        failed.push(format!("seam name without a route: {name}"));
    }
    // every effect the session spawned (clip notifies AND code: hit effects, bullets, finish effects) carries in the routes
    // exactly the sounds of its KH3 particle system (`ParticleSeAssets`)
    let _ = writeln!(text, "# the sounds the spawned effects carry (KH3 ParticleSeAssets -> the routes)");
    if let Some(mut kh3) = Kh3Effects::load() {
        let mut seen: Vec<String> = Vec::new();
        for r in &s.rows {
            for e in r.effects.iter().filter(|e| e.stop.is_none() && !e.name.is_empty()) {
                if seen.contains(&e.name) {
                    continue;
                }
                seen.push(e.name.clone());
                let port: Vec<(String, bool)> = s.audio.engine.routes.effect_sounds(&e.name).iter().map(|x| (kh_audio::pack::asset_key(&x.asset).to_string(), x.at_end)).collect();
                let line = match kh3.sounds(&e.name) {
                    Some(want) => {
                        let same = want.len() == port.len() && want.iter().all(|w| port.contains(w));
                        let bad: Vec<String> = port.iter().filter_map(|p| playable(s.audio.engine.pack(), &p.0).err()).collect();
                        if !(same && bad.is_empty()) {
                            failed.push(format!("effect {} carries {want:?} in KH3, the routes {port:?} {}", e.name, bad.join("; ")));
                        }
                        format!("{} effect {} (step {}): KH3 {want:?} -> routes {port:?}", if same && bad.is_empty() { "PASS" } else { "FAIL" }, e.name, r.step)
                    }
                    None => format!("PASS effect {} (step {}): no KH3 particle system of that name (an ead / a port name): routes {port:?}", e.name, r.step),
                };
                let _ = writeln!(text, "{line}");
            }
        }
    }
    let _ = writeln!(text, "# unknown seam names: {:?}", s.audio.unknown);
    text.push_str(&audit_a::report(&s.rows));
    std::fs::write(audit_a::path_in_logs(&format!("{log}.report.txt")), &text).ok();
    failed
}

fn run_form(weapon: &str, models: &[&str], base: &'static str, cmds: &[i32], log: &str) {
    let Some(s) = session(weapon, models, base, cmds, log) else { return };
    let failed = judge(&s, log);
    assert!(failed.is_empty(), "{log}: {} sound rows failed:\n{}", failed.len(), failed.join("\n"));
}

#[test]
fn second_form_kingdom_key_session_sounds() {
    run_form("WEP_KEYBLADE_SO_00", &[], "w_so010", &[0x94], "sound_l1");
}

#[test]
fn second_form_starlight_session_sounds() {
    run_form("WEP_KEYBLADE_SO_17", &["w_so150"], "w_so150", &[0x95], "sound_l2");
}

#[test]
fn second_form_dead_of_night_session_sounds() {
    run_form("WEP_KEYBLADE_SO_18", &["w_so120", "w_so121"], "w_so120", &[0x98], "sound_l5");
}

#[test]
fn counter_shield_session_sounds() {
    run_form("WEP_KEYBLADE_SO_01", &["w_so030", "w_so036", "w_so035", "w_so032", "w_so031", "w_so033", "w_so037", "w_so034"], "w_so030", &[0x89], "sound_sh");
}

#[test]
fn frying_pan_session_sounds() {
    run_form("WEP_KEYBLADE_SO_10", &["w_so140", "w_so145", "w_so142", "w_so141", "w_so143", "w_so144"], "w_so140", &[0x91], "sound_sh2");
}

#[test]
fn double_arrowguns_magic_launcher_session_sounds() {
    run_form("WEP_KEYBLADE_SO_02", &["w_so020", "w_so021", "w_so022", "w_so023", "w_so024"], "w_so020", &[0x80, 0x83], "sound_dw");
}

#[test]
fn hunny_blasters_hunny_launcher_session_sounds() {
    run_form("WEP_KEYBLADE_SO_07", &["w_so040", "w_so041", "w_so042", "w_so043", "w_so044"], "w_so040", &[0x8E, 0x8F], "sound_dw2");
}

#[test]
fn mirage_staff_session_sounds() {
    run_form("WEP_KEYBLADE_SO_04", &["w_so070", "w_so071", "w_so072", "w_so073"], "w_so070", &[0x88], "sound_wa");
}

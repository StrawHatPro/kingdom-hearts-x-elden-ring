//! (party stream E) THE PARTY'S SOUND COVERAGE: every party row of the audio pack (Donald's and Goofy's triggers:
//! their clips' notifies, voice group units, footstep table entries, the 14 + 14 weapons' sets, their effects' and
//! spell actors' sounds) resolved the way the host resolves it at run time (`Routes::for_owner`, the seam name the
//! party code emits), and whether the test level ever runs it.
//!
//!   cargo run -p kh-audio --example party_coverage -- [OUT.md]   (default build/sora_tmp/party/impl_e/PARTY_SOUND_COVERAGE.md)

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use kh_audio::pack::Trigger;
use kh_audio::routes::{localized, Context, Resolved};
use kh_audio::{Language, Pack, Routes};

/// Why a clip never runs in the test level (plan 2.6, 2.10, N5), by its logical anim-set row; `None` = it can run.
fn never_in_test_level(logical: &[String]) -> Option<&'static str> {
    let all = |f: &dyn Fn(&str) -> bool| !logical.is_empty() && logical.iter().all(|l| f(l));
    if all(&|l| ["MCurling", "SCurling", "Mbowl", "SHold", "Deck", "Catch", "SCSwing"].iter().any(|p| l.starts_with(p))) {
        return Some("another world's team attack (out of scope, plan 2.6)");
    }
    if all(&|l| ["M_108", "M_109", "Water", "Swim", "Dive"].iter().any(|p| l.starts_with(p))) {
        return Some("under water / water flow (no water in the test level, N5)");
    }
    if all(&|l| ["M_0811", "M_0812", "M_0813", "M_0814"].iter().any(|p| l.starts_with(p))) {
        return Some("field chat talk motion (world scripts)");
    }
    if all(&|l| l.starts_with("CM_") || l.starts_with("GM_")) {
        return Some("camp / gummi set");
    }
    None
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_tmp/party/impl_e/PARTY_SOUND_COVERAGE.md").to_string());
    let pack = Pack::load(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack/audio"))).expect("the audio pack");
    let owners = [("n_ex001", "Donald"), ("n_ex002", "Goofy")];
    let mut routes: BTreeMap<&str, Routes> = owners.iter().map(|(o, _)| (*o, Routes::for_owner(&pack, o))).collect();
    let cx = Context { surface: 10, language: Language::En, voice_group_no: 0, anim: None };
    // (route, owner) -> [plays, never, silent by data, not in pack, n/a]
    let mut table: BTreeMap<(String, String), [usize; 5]> = BTreeMap::new();
    let mut never_why: BTreeMap<String, usize> = BTreeMap::new();
    let mut problems = Vec::new();
    let in_pack = |path: &str| pack.asset(path).is_some_and(|a| pack.sound(a.bank, a.index).is_some());
    let party_code = |t: &Trigger| t.route == "code.effect_se_npc" || t.trigger.starts_with("n_ex00");
    for t in pack.triggers.iter().filter(|t| t.owner.is_some() || party_code(t)) {
        let owner = t.owner.clone().unwrap_or_else(|| if t.trigger.contains("n_ex002") { "n_ex002".into() } else if t.trigger.contains("n_ex001") || t.trigger.contains("vn_ex001") { "n_ex001".into() } else { "shared".into() });
        let slot = |k: usize, table: &mut BTreeMap<(String, String), [usize; 5]>| table.entry((t.route.clone(), owner.clone())).or_insert([0; 5])[k] += 1;
        // what the party code puts on the seam for this row, resolved with the member's routes
        let first = t.sounds.first().map(|s| s.asset.clone());
        let status: Result<(), String> = match t.route.as_str() {
            "notify.auto_se_enable" | "notify.actor_voice_null" | "notify.attack_hit_se" if t.sounds.is_empty() => {
                slot(4, &mut table);
                continue;
            }
            "weapon.swing_set" | "weapon.hit_set" => {
                let r = routes.get_mut(owner.as_str()).unwrap();
                r.select_weapon(t.weapon.as_deref().unwrap_or(""));
                let n = t.sounds[0].alias_index.unwrap_or(0);
                let alias = format!("{}{n}", if t.route == "weapon.swing_set" { "SWING" } else { "HIT" });
                match r.resolve(&pack, &alias, &cx) {
                    Resolved::Asset { path, .. } if in_pack(&path) => Ok(()),
                    other => Err(format!("{alias} -> {other:?}")),
                }
            }
            "voice.group" => {
                let s = &t.sounds[0];
                let name = format!("{}:{}", s.group.clone().unwrap_or_default(), s.group_no.unwrap_or(0));
                match routes[owner.as_str()].resolve(&pack, &name, &cx) {
                    Resolved::Asset { path, .. } if in_pack(&path) => Ok(()),
                    other => Err(format!("{name} -> {other:?}")),
                }
            }
            "autose.per_surface" | "autose.aux_surface" | "autose.non_surface" => match &first {
                Some(a) if in_pack(a) => Ok(()),
                other => Err(format!("{other:?} not in the pack")),
            },
            _ => match &first {
                // notifies and code rows: the asset (voices per language)
                Some(a) => {
                    let p = if a.contains("/VOICE/") { localized(a, Language::En) } else { a.clone() };
                    if in_pack(&p) { Ok(()) } else { Err(format!("{p} not in the pack")) }
                }
                None if t.route == "notify.auto_se" => Ok(()),
                None => {
                    slot(4, &mut table);
                    continue;
                }
            },
        };
        match status {
            Err(e) => {
                slot(3, &mut table);
                problems.push(format!("{} ({}): {e}", t.id, t.route));
            }
            Ok(()) => match never_in_test_level(&t.logical) {
                Some(why) => {
                    slot(1, &mut table);
                    *never_why.entry(why.to_string()).or_default() += 1;
                }
                None => slot(0, &mut table),
            },
        }
    }
    let mut md = String::new();
    writeln!(md, "# Sound coverage of the party (Donald, Goofy)\n").unwrap();
    writeln!(md, "Generated by `cargo run -p kh-audio --example party_coverage` (crates/kh-audio/examples/party_coverage.rs) from the audio pack\n(`node tools/party_audio_pack.js` + `node tools/sora_audio_pack.js`); do not edit by hand.\n").unwrap();
    writeln!(md, "Unit: one party trigger row of the pack (sheet `sheets/port/party-sounds.json` has the same rows). Status: **resolves** =\nthe name the party code puts on the seam (`kh_core::host::party`, `Attach::Party`) resolves with the member's routes\n(`Routes::for_owner`: its weapon's sets, its voice group set, its footstep table) to a sound in the pack, and the test\nlevel can run it once the party's states play the clip / the code path; **never here** = resolves, but its clip\nnever runs in the test level (reason below); **not in pack** = a problem (listed); **n/a** = not a sound (enable\nwindows, empty notifies, attack windows without a sound of their own).\n").unwrap();
    writeln!(md, "| route | owner | resolves | never here | not in pack | n/a |\n|---|---|---|---|---|---|").unwrap();
    let mut total = [0usize; 5];
    for ((route, owner), c) in &table {
        writeln!(md, "| `{route}` | {owner} | {} | {} | {} | {} |", c[0], c[1], c[3], c[4]).unwrap();
        for k in 0..5 {
            total[k] += c[k];
        }
    }
    writeln!(md, "| **total** | | **{}** | **{}** | **{}** | **{}** |\n", total[0], total[1], total[3], total[4]).unwrap();
    writeln!(md, "## Why rows never run in the test level\n").unwrap();
    for (why, n) in &never_why {
        writeln!(md, "- {n}: {why}").unwrap();
    }
    writeln!(md, "\nAlso never in the test level, not rows of the pack: the level-scripted battle / field chat lines\n(`ActorVoice/bt/av_bt_chat_donald_*`, referenced by `Levels/bt/...` only) and the gummi-ship lines `bt407*do0 / go0`\n(`SpecialShip_VoiceGroupSet`). Cloth rustles: `FsTable_Donald` / `_Goofy` have no `NonSurfaceInfos` [A]: the\ndetector's rustle events are silent for both (logged \"no table entry\").\n").unwrap();
    writeln!(md, "## Problems ({})\n", problems.len()).unwrap();
    for p in &problems {
        writeln!(md, "- {p}").unwrap();
    }
    std::fs::write(&out, md).expect("write");
    println!("party_coverage: {} rows resolve, {} never in the test level, {} not in pack, {} n/a -> {out}", total[0], total[1], total[3], total[4]);
}

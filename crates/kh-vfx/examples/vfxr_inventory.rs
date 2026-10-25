//! (VFXR) The skipped-module inventory: parses every effect file of the given effect folders with the evaluator's own
//! parser and prints, per skipped module class, how many effects / emitters use it and the effects' names.
//! `cargo run -p kh-vfx --example vfxr_inventory -- DIR [DIR ...]`
use std::collections::BTreeMap;

fn main() {
    let mut by: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    let mut files = 0;
    for dir in std::env::args().skip(1) {
        let Ok(rd) = std::fs::read_dir(&dir) else { eprintln!("no dir {dir}"); continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            let Ok(def) = kh_vfx::EffectDef::parse(&text) else { continue };
            files += 1;
            let mut seen: Vec<String> = Vec::new();
            for e in def.emitters.iter().filter(|e| e.enabled) {
                for u in &e.unknown {
                    let entry = by.entry(u.clone()).or_default();
                    entry.0 += 1;
                    if !seen.contains(u) {
                        seen.push(u.clone());
                        entry.1.push(def.name.clone());
                    }
                }
            }
        }
    }
    println!("{files} effects");
    for (k, (n, effects)) in &by {
        println!("{k}\t{} effects\t{n} emitters\t{}", effects.len(), effects.join(" "));
    }
}

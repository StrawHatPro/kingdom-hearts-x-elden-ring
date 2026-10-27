//! kh-er-selfcheck (crate kh-selfcheck): the offline check of kh_er.dll (its `kh_er_selfcheck` export, crates/kh-er/src/entry.rs) for the Linux
//! setup. On Windows, release-tools/selfcheck.ps1 loads the DLL and calls the export; on Linux the DLL only runs inside
//! Proton, so this program runs the same check with the same crates (kh-gendata, kh-core, kh-party, kh-camera), built
//! for Linux against the same table list (KH_GEN_RUST_DIR): it finds the pack as the DLL does (er/mods/kh-er.cfg `pack`,
//! else KH_SORA_PACK, else build/sora_pack), reads every generated table made from KH3 data and decodes them through the
//! code that uses them. Writes `<mods>/kh-er-selfcheck.txt` like the DLL; exit code 0 = the tables were read.
//!
//!   kh-er-selfcheck <the er/mods folder>
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

/// er/mods/kh-er.cfg's `key = value` lines (the rules of crates/kh-er/src/control.rs `parse`): the last `pack`
fn cfg_pack(file: &Path) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let strip = |s: &str| -> String {
        let mut prev_space = false;
        for (i, c) in s.char_indices() {
            if (c == '#' || c == ';') && prev_space {
                return s[..i].trim().to_string();
            }
            prev_space = c == ' ' || c == '\t';
        }
        s.trim().to_string()
    };
    let mut pack = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') || (line.starts_with('[') && strip(line).ends_with(']')) {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() != "pack" {
                continue;
            }
            let v = v.trim();
            let quoted = v.strip_prefix('"').and_then(|r| r.find('"').map(|e| (r[..e].to_string(), r[e + 1..].trim().to_string())));
            pack = Some(match quoted {
                Some((inner, after)) if after.is_empty() || after.starts_with('#') || after.starts_with(';') => inner,
                _ => strip(v),
            });
        }
    }
    pack
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "a panic".to_string())
}

fn main() {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: kh-er-selfcheck <er/mods folder>");
        std::process::exit(2);
    };
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    // (as the DLL: `root` is er/, the project the repository)
    let root = dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| dir.clone());
    let report = dir.join("kh-er-selfcheck.txt");
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<Vec<String>, String> {
        let project = root.parent().unwrap_or(&root).to_path_buf();
        let pack = cfg_pack(&root.join("mods").join("kh-er.cfg")).map(PathBuf::from).or_else(|| std::env::var_os("KH_SORA_PACK").map(PathBuf::from)).unwrap_or_else(|| project.join("build").join("sora_pack"));
        let mut lines = vec![format!("kh-er {} self-check (Linux: the DLL's check, built natively), pack {}", env!("CARGO_PKG_VERSION"), pack.display())];
        kh_gendata::set_project(&project);
        lines.push(kh_gendata::init(&pack.join("gen"))?);
        lines.push(format!("game text (the pack's ui/strings): {:?}", kh_gendata::line("kh3_menu.shop.dialog_0020")));
        use kh_core::anim::{ANIMS, NOTIFY_ASSETS, SEQUENCES};
        lines.push(format!("sora_anim: {} sequences, {} animations, {} notify assets; IdleN = {:?}", SEQUENCES.len(), ANIMS.len(), NOTIFY_ASSETS.len(), kh_core::anim::AnimId::from_name("IdleN")));
        use kh_party::Chara;
        for c in [Chara::Donald, Chara::Goofy] {
            let set = kh_party::tables::anim_set(c);
            lines.push(format!("party_clips / party_data {c:?}: {} clips, level 1 row {:?}, attack hit SE {:?}", set.clips.len(), kh_party::tables::level_row(c, 1), kh_party::notify_seam::attack_hit_se(c, "-", "-")));
        }
        lines.push(format!("party_team_gshoot: frame {:?}", kh_party::team::link::gshoot_frame(0, 0.0).root));
        lines.push(format!("party_team_cameras: {} camera anims, {} shakes", kh_camera::camera_anim::TEAM_CAMERA_ANIMS.len(), kh_camera::camera_anim::TEAM_CAMERA_SHAKES.len()));
        Ok(lines)
    }));
    let (text, n) = match result {
        Ok(Ok(lines)) => {
            let n = lines.len() as u32 - 2;
            (lines.join("\n"), n.max(1))
        }
        Ok(Err(e)) => (format!("FAILED: {e}"), 0),
        Err(payload) => (format!("FAILED: {}", panic_text(&payload)), 0),
    };
    let _ = std::fs::write(&report, text.clone() + "\n");
    for l in text.lines() {
        println!("  {l}");
    }
    if n == 0 {
        println!("kh-er-selfcheck: the generated tables were NOT read");
        std::process::exit(1);
    }
    println!("kh-er-selfcheck: {n} table group(s) read");
}

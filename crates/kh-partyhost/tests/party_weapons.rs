//! (PARTY-WEAPONS) EVERY STAFF AND SHIELD, WALKED: the user (2026-11-05) asked "will this apply to all of their weapons?".
//! Each equippable weapon of the equipment table (sheets/port/kh3-party-weapons.json, every row, DLC included: the table
//! has 14 + 14) resolves in kh-party's weapon_draw list to its model, its socket and its appear / put-away look; the pack
//! has its model (manifest + mesh), the socket on the member's row of party.json and the effect; and the look is the one of
//! its own blueprints (`<model>_Wep` and its base: `m_AppearEffect`, `m_DisappearEffect`, the dither times;
//! `<model>_Equip`: `AttachBoneName`), read from the converted packages when they are on this machine.

use std::path::{Path, PathBuf};

use kh_party::weapon_draw::{weapon_row, WEAPONS};
use kh_party::Chara;
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A blueprint CDO's property along its class chain (own first): (value, the package it came from).
fn chain_prop(content: &Path, rel: &str, name: &str) -> Option<(Value, Value)> {
    let mut rel = rel.to_string();
    for _ in 0..4 {
        let d = json(&content.join(format!("{rel}.uasset.json")));
        let exports = d["exports"].as_array()?;
        let cdo = exports.iter().find(|e| e["objectName"].as_str().is_some_and(|n| n.starts_with("Default__")))?;
        if let Some(p) = cdo["data"]["props"].as_array().and_then(|ps| ps.iter().find(|p| p["name"] == name)) {
            let v = p["value"].clone();
            // an object: its import's name
            let named = match v.as_i64() {
                Some(i) if i < 0 => d["imports"][(-i - 1) as usize]["objectName"].clone(),
                _ => v.clone(),
            };
            return Some((named, Value::String(rel)));
        }
        // the parent blueprint
        let class = exports.iter().find(|e| e["class"] == "BlueprintGeneratedClass")?;
        let sup = class["superIndex"].as_i64().filter(|i| *i < 0)?;
        let mut im = &d["imports"][(-sup - 1) as usize];
        while let Some(o) = im["outerIndex"].as_i64().filter(|o| *o < 0) {
            im = &d["imports"][(-o - 1) as usize];
        }
        let pkg = im["objectName"].as_str()?;
        rel = pkg.strip_prefix("/Game/")?.to_string();
    }
    None
}

#[test]
fn every_equippable_staff_and_shield_resolves_to_its_model_socket_and_effect() {
    let root = root();
    let sheet = json(&root.join("sheets/port/kh3-party-weapons.json"));
    let cols: Vec<&str> = sheet["columns"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    let col = |name: &str| cols.iter().position(|c| *c == name).unwrap();
    let rows = sheet["rows"].as_array().unwrap();
    assert_eq!(rows.len(), WEAPONS.len(), "the table's every weapon is in the list (14 staves + 14 shields)");
    let pack = root.join("build/sora_pack/party");
    let party = json(&pack.join("party.json"));
    let content = root.join("assets_converted/packages/KINGDOM HEARTS III/Content");
    let have_assets = content.is_dir();
    let mut lines = Vec::new();
    for r in rows {
        let key = r[col("id")].as_str().unwrap();
        let chara = match r[col("chara")].as_str().unwrap() {
            "donald" => Chara::Donald,
            "goofy" => Chara::Goofy,
            other => panic!("{key}: chara {other}"),
        };
        let bp = r[col("weapon_bps")][0].as_str().unwrap();
        let model = bp.rsplit('/').next().unwrap().trim_end_matches("_Wep");
        let w = weapon_row(key).unwrap_or_else(|| panic!("{key}: not in weapon_draw::WEAPONS"));
        assert_eq!((w.chara, w.model), (chara, model), "{key}");
        // the pack: model, socket, effect
        let dir = pack.join("weapons").join(model);
        assert!(dir.join("manifest.json").is_file() && dir.join("weapon.glb").is_file(), "{key}: the pack's model {model}");
        let member = party["members"].as_array().unwrap().iter().find(|m| m["weapons"].as_array().is_some_and(|ws| ws.iter().any(|x| x["model"] == model))).unwrap_or_else(|| panic!("{key}: no member of party.json holds {model}"));
        let pw = member["weapons"].as_array().unwrap().iter().find(|x| x["model"] == model).unwrap();
        assert_eq!(pw["socket"].as_str(), Some(w.socket), "{key}: the socket on party.json");
        assert!(pack.join("vfx/effects").join(format!("{}.json", w.look.effect)).is_file(), "{key}: the effect {} in the pack", w.look.effect);
        // the blueprints, when this machine has the converted packages
        if have_assets {
            let owner = if chara == Chara::Donald { "n_ex001" } else { "n_ex002" };
            let rel = format!("Blueprints/Npc/{owner}/{model}_Wep");
            let (appear, from) = chain_prop(&content, &rel, "m_AppearEffect").unwrap_or_else(|| panic!("{key}: no m_AppearEffect"));
            let (disappear, _) = chain_prop(&content, &rel, "m_DisappearEffect").unwrap();
            assert_eq!((appear.as_str(), disappear.as_str()), (Some(w.look.effect), Some(w.look.effect)), "{key}: its blueprint's effects (from {from})");
            for name in ["m_AppearDitherAnimTime", "m_DisappearDitherAnimTime"] {
                let (t, _) = chain_prop(&content, &rel, name).unwrap();
                assert!((t.as_f64().unwrap() as f32 - w.look.dither).abs() < 1e-6, "{key}: {name} {t}");
            }
            // no visible delay and no scale: neither the weapon nor its base sets them (the native constructor's 0 / off)
            for name in ["m_AppearVisibleDelayTime", "m_DisappearVisibleDelayTime", "m_bIsEnableAppearScaleAnim", "m_bIsEnableDisappearScaleAnim"] {
                assert!(chain_prop(&content, &rel, name).is_none(), "{key}: {name} set");
            }
            assert_eq!((w.look.delay, w.look.hide_delay), (0.0, 0.0));
            let equip = json(&root.join(format!("sheets/entities/Blueprints/Npc/{owner}/{model}_Equip.json")));
            let props = equip["objects"][0]["m_Equips"]["items"][0]["props"].as_array().unwrap();
            let bone = props.iter().find(|p| p["tag"]["name"] == "AttachBoneName").map(|p| p["value"].clone());
            assert_eq!(bone.as_ref().and_then(|b| b.as_str()), Some(w.socket), "{key}: AttachBoneName of {model}_Equip");
        }
        lines.push(format!("{key} {model} {:?} socket {} effect {} dither {}", chara, w.socket, w.look.effect, w.look.dither));
    }
    assert!(have_assets || std::env::var("KH_ASSETS_OPTIONAL").is_ok(), "the converted packages are not on this machine (KH_ASSETS_OPTIONAL=1 to walk the pack only)");
    let out = root.join("build/sora_tmp/weapons/PARTY-WEAPONS/traces");
    if std::fs::create_dir_all(&out).is_ok() {
        let _ = std::fs::write(out.join("every_weapon.txt"), lines.join("\n"));
    }
}

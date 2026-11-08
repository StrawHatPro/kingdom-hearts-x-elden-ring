//! (weapons P6b) The Workshop's texts: KH3's string ids (`kh3_menu.synthesis.*`, `kh3_menu.shop.*`, the UI pack) and the
//! two COMPOSED lines that replace KH3's "added to the shop" (the user, 2026-11-03: a goal's purchase is sold by the
//! converted Elden Ring merchants, not by the Moogle), and [`MOD`]: KH3's Photo Missions renamed "Journey to the Erdtree"
//! (the user, 2026-11-07). Every other text is KH3's, read by its id from the player's own UI pack ([`IDS`]); KH3's text
//! is never compiled in (KH3TEXT 2026-11-08), and the composed lines here are the mod's own wording (no KH3 sentence).

/// In place of KH3's bonus_0030 (the item is added to the Moogle's shop): COMPOSED, KH3's markup, the mod's own words: the
/// merchants sell it.
pub const MERCHANT_BONUS: &str = "Finish this Collector's Goal and <color(strong)>merchants<color><br>will sell the item below.";
/// In place of KH3's dialog_0040 (a new item in the shop): COMPOSED, KH3's markup and {0}, the mod's own words.
pub const MERCHANT_DIALOG: &str = "You've finished a Collector's Goal!<br>Merchants now sell a new item:<br><color(strong)>{0}<color>";

/// THE MOD'S OWN TEXTS for KH3 string ids, read BEFORE the pack's (the user, 2026-11-07: "rename the menu itself to
/// Journey to the Erdtree"): every line that says "Photo Missions" where the player sees it (the Workshop's command and the
/// screen's title, its help line, the SYNTH LOG's count, the visit's dialog). COMPOSED in KH3's tone and markup.
pub const MOD: &[(&str, &str)] = &[
    ("kh3_menu.synthesis.menu_0030", "Journey to the Erdtree"),
    ("kh3_menu.synthesis.detail_0030", "Show the moogles your great victories."),
    ("kh3_menu.synthesis.log_0040", "Journey Milestones Completed"),
    ("kh3_menu.synthesis.dialog_0050", "Journey milestone complete!<br>New items can now be synthesized."),
];

/// The mod's own text of an id ([`MOD`]), if it has one.
pub fn composed(id: &str) -> Option<&'static str> {
    MOD.iter().find(|(k, _)| *k == id).map(|(_, v)| *v)
}

/// The KH3 string ids the Workshop reads (the pack's `kh3_menu` / `kh3_system` tables). KH3'S TEXT IS NEVER COMPILED IN
/// (KH3TEXT 2026-11-08: the release checker refuses a DLL with KH3's game text): the view reads each id from the player's
/// own UI pack (made by the setup from their KH3 install); an id the pack lacks shows as the id itself ([`missing`]).
pub const IDS: &[&str] = &[
    "kh3_menu.synthesis.title_0010",
    "kh3_menu.synthesis.menu_0010",
    "kh3_menu.synthesis.menu_0020",
    "kh3_menu.synthesis.menu_0030",
    "kh3_menu.synthesis.menu_0040",
    "kh3_menu.synthesis.menu_0050",
    "kh3_menu.synthesis.log_0010",
    "kh3_menu.synthesis.log_0020",
    "kh3_menu.synthesis.log_0030",
    "kh3_menu.synthesis.log_0040",
    "kh3_menu.synthesis.log_0050",
    "kh3_menu.synthesis.bonus_0010",
    "kh3_menu.synthesis.bonus_0020",
    "kh3_menu.synthesis.item_0010",
    "kh3_menu.synthesis.item_0020",
    "kh3_menu.synthesis.item_0030",
    "kh3_menu.synthesis.item_0040",
    "kh3_menu.synthesis.item_0050",
    "kh3_menu.synthesis.materials_0010",
    "kh3_menu.synthesis.detail_0010",
    "kh3_menu.synthesis.detail_0020",
    "kh3_menu.synthesis.detail_0030",
    "kh3_menu.synthesis.detail_0040",
    "kh3_menu.synthesis.detail_0050",
    "kh3_menu.synthesis.detail_0110",
    "kh3_menu.synthesis.detail_0210",
    "kh3_menu.synthesis.detail_0310",
    "kh3_menu.synthesis.collection_0010",
    "kh3_menu.synthesis.collection_0020",
    "kh3_menu.synthesis.collection_0030",
    "kh3_menu.synthesis.collection_0040",
    "kh3_menu.synthesis.collection_0050",
    "kh3_menu.synthesis.collection_0060",
    "kh3_menu.synthesis.collection_0070",
    "kh3_menu.synthesis.collection_0080",
    "kh3_menu.synthesis.collection_0090",
    "kh3_menu.synthesis.collection_0100",
    "kh3_menu.synthesis.collection_0110",
    "kh3_menu.synthesis.collection_0120",
    "kh3_menu.synthesis.collection_0130",
    "kh3_menu.synthesis.collection_0140",
    "kh3_menu.synthesis.collection_0150",
    "kh3_menu.synthesis.collection_0160",
    "kh3_menu.synthesis.collection_0170",
    "kh3_menu.synthesis.collection_0180",
    "kh3_menu.synthesis.collection_0190",
    "kh3_menu.synthesis.collection_0200",
    "kh3_menu.synthesis.collection_0210",
    "kh3_menu.synthesis.dialog_0010",
    "kh3_menu.synthesis.dialog_0020",
    "kh3_menu.synthesis.dialog_0030",
    "kh3_menu.synthesis.dialog_0050",
    "kh3_menu.synthesis.dialog_0110",
    "kh3_menu.synthesis.dialog_0210",
    "kh3_menu.synthesis.dialog_0220",
    "kh3_menu.synthesis.dialog_0310",
    "kh3_menu.shop.tab_0010",
    "kh3_menu.shop.tab_0020",
    "kh3_menu.shop.tab_0030",
    "kh3_menu.shop.tab_0040",
    "kh3_menu.shop.tab_0050",
    "kh3_menu.shop.tab_0060",
    "kh3_menu.shop.item_0060",
    "kh3_menu.shop.item_0070",
    "kh3_menu.shop.munny_0010",
    "kh3_system.general.choice_0001",
    "kh3_system.general.choice_0002",
    "kh3_system.general.choice_0003",
];

/// The text shown for an id the pack lacks: the id itself (never a compiled KH3 line).
pub fn missing(id: &str) -> String {
    id.to_string()
}

/// KH3's `{0}|plural(one=..,other=..)` and `{0}` / `{1}`: a goal's text with its numbers (`collection_0010` "Obtain
/// {0}|plural(one=a synthesis material,other={0} different synthesis materials).", `collection_0040` "Obtain at least {0}
/// <color(strong)>{1}s<color>.").
pub fn format(template: &str, n: u32, name: &str) -> String {
    let mut text = template.to_string();
    if let Some(start) = text.find("{0}|plural(") {
        let body_at = start + "{0}|plural(".len();
        if let Some(len) = text[body_at..].find(')') {
            let body = &text[body_at..body_at + len];
            let pick = |form: &str| body.split(',').find_map(|p| p.strip_prefix(form).map(str::to_string));
            let form = if n == 1 { pick("one=") } else { pick("other=") }.or_else(|| pick("other=")).unwrap_or_default();
            text.replace_range(start..body_at + len + 1, &form);
        }
    }
    text.replace("{0}", &n.to_string()).replace("{1}", name)
}

/// KH3's markup to the screen's: `<br>` a line break.
pub fn lines(text: &str) -> String {
    text.replace("<br>", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_texts_take_their_numbers_and_plurals() {
        let t = kh3_collection_0010();
        assert_eq!(format(t, 1, ""), "Obtain a synthesis material.");
        assert_eq!(format(t, 25, ""), "Obtain 25 different synthesis materials.");
        assert_eq!(format("Obtain at least {0} <color(strong)>{1}s<color>.", 30, "Blazing Shard"), "Obtain at least 30 <color(strong)>Blazing Shards<color>.");
        assert_eq!(format("Obtain at least {0} total synthesis materials.", 500, ""), "Obtain at least 500 total synthesis materials.");
    }

    fn kh3_collection_0010() -> &'static str {
        "Obtain {0}|plural(one=a synthesis material,other={0} different synthesis materials)."
    }

    /// The player's UI pack (build/sora_pack, made by the setup from the KH3 install): its English string tables, or
    /// `None` on a checkout without a pack.
    fn pack_strings() -> Option<std::collections::HashMap<String, String>> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui/strings/en");
        let mut out = std::collections::HashMap::new();
        for table in ["kh3_menu", "kh3_system"] {
            let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{table}.json"))).ok()?).ok()?;
            for (ns, rows) in json["namespaces"].as_object()? {
                for (k, v) in rows.as_object()? {
                    if let Some(v) = v.as_str() {
                        out.insert(format!("{ns}.{k}"), v.to_string());
                    }
                }
            }
        }
        Some(out)
    }

    /// "Journey to the Erdtree" replaces every "Photo Missions" line the player sees (the pack's KH3 text of those ids).
    #[test]
    fn photo_missions_read_journey_to_the_erdtree() {
        assert_eq!(composed("kh3_menu.synthesis.menu_0030"), Some("Journey to the Erdtree"));
        for id in ["kh3_menu.synthesis.menu_0030", "kh3_menu.synthesis.log_0040", "kh3_menu.synthesis.dialog_0050", "kh3_menu.synthesis.detail_0030"] {
            let shown = composed(id).unwrap_or_else(|| panic!("{id} has no composed text"));
            assert!(!shown.to_lowercase().contains("photo"), "{id}: {shown}");
        }
        if let Some(pack) = pack_strings() {
            for id in IDS.iter().filter(|id| pack.get(**id).is_some_and(|v| v.to_lowercase().contains("photo"))) {
                let shown = composed(id).unwrap_or_else(|| panic!("{id} still says photo"));
                assert!(!shown.to_lowercase().contains("photo") && shown.contains("Journey"), "{id}: {shown}");
            }
        }
        assert!(composed("kh3_menu.synthesis.detail_0030").is_some_and(|s| !s.contains("inspiration")), "the help line is the milestones'");
        let dialog = composed("kh3_menu.synthesis.dialog_0050").unwrap();
        assert!(dialog.starts_with("Journey milestone complete!<br>"), "KH3's markup");
    }

    /// The composed lines keep KH3's shape (the same markup, the same {0}) and say the merchants sell it.
    #[test]
    fn merchant_lines_replace_the_shop() {
        assert!(MERCHANT_DIALOG.contains("<br>") && MERCHANT_DIALOG.contains("<color(strong)>{0}<color>"));
        assert!(MERCHANT_BONUS.contains("merchants") && !MERCHANT_BONUS.contains("shop"));
        assert!(!MERCHANT_DIALOG.contains("shop"));
    }

    /// (KH3TEXT) Every id the Workshop reads is in the player's pack (so no compiled fallback is needed), and an id the
    /// pack lacks shows as the id, never a KH3 line.
    #[test]
    fn every_id_comes_from_the_pack_none_is_compiled() {
        assert_eq!(missing("kh3_menu.synthesis.title_0010"), "kh3_menu.synthesis.title_0010");
        let mut seen = std::collections::HashSet::new();
        assert!(IDS.iter().all(|id| seen.insert(*id)), "an id twice");
        let Some(pack) = pack_strings() else { return };
        for id in IDS {
            assert!(pack.get(*id).is_some_and(|v| !v.is_empty()), "{id} is not in the pack");
        }
        for (id, own) in MOD {
            assert_ne!(pack.get(*id).map(String::as_str), Some(*own), "{id}: the mod's own text is not KH3's");
        }
    }
}

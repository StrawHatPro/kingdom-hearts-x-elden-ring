//! (IMPL_ER48) ELDEN RING'S STATUS AILMENTS AND SORA: the pure rules (`status.rs` is the game half).
//!
//! - WHAT CLEARS THEM: Cure / Cura / Curaga landing on Sora (the core's heal effect of the tier, `kh_core::host::audio::
//!   cure_heal_effect`), the KH3 items whose OWN KH3 help text says they remove status effects (decided from the pack's
//!   text: `ui/data/item_icons.json` help ids -> `ui/strings/en/kh3_item.json`, [`text_clears_status`]), and a KH3 save
//!   point while it heals him (`savepoint.rs`).
//! - THE TICKS: an HP loss of the body while one of Elden Ring's damage-over-time ailments (poison, scarlet rot) runs on
//!   it is a STATUS TICK: it reaches Sora's HP without a hit reaction ([`classify`]). Everything else is a hit as before;
//!   a loss below `damage.in.min_share` that is not a status tick (burning: fire pots, standing in fire) is a SMALL hit,
//!   KH3's own small damage reaction (the user, 2026-10-13: "status effects like poison should not make Sora flinch,
//!   burn should though").
//! - THE METERS: `PlayerGameData::resistance_gauges` / `resistance_gauge_max` (SDK names, order Poison, Rot, Bleed,
//!   Death, Frost, Sleep, Madness). Which way they count is NOT KNOWN (buildup from 0, or resistance left from the
//!   maximum): [`GaugeRest`] learns the value a meter rests at, and only a learnt rest value is ever written.

use kh_core::status::ItemKind;

/// The seven ailments in the order of the SDK's gauge and timer arrays.
pub const AILMENTS: [&str; 7] = ["poison", "scarlet rot", "hemorrhage", "deathblight", "frostbite", "sleep", "madness"];
/// The ailments that hurt over time once they have struck (index into [`AILMENTS`]).
pub const OVER_TIME: [usize; 2] = [0, 1];

/// A KH3 text without its markup (`<br>`, `<color(strong)>`), whitespace collapsed.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The KH3 help text says the item removes (cures) status effects.
pub fn text_clears_status(text: &str) -> bool {
    let t = plain(text).to_lowercase();
    t.contains("status effect") && (t.contains("remove") || t.contains("cure"))
}

/// The text a help id (`kh3_item.battle.detail_elixir`) names in a strings file of the pack (`namespaces` ->
/// `kh3_item.battle` -> `detail_elixir`).
pub fn help_text<'a>(strings: &'a serde_json::Value, id: &str) -> Option<&'a str> {
    let (space, key) = id.rsplit_once('.')?;
    strings["namespaces"][space][key].as_str()
}

/// The battle items whose KH3 help text says they remove status effects, each with the text (plain).
/// `icons` = the pack's `ui/data/item_icons.json`, `strings` = `ui/strings/en/kh3_item.json`.
pub fn clearing_items(icons: &serde_json::Value, strings: &serde_json::Value) -> Vec<(ItemKind, String)> {
    let mut out = Vec::new();
    for item in ItemKind::ALL {
        let key = kh_ui::item_art::game_key(item.name());
        let Some(id) = icons["help"][key].as_str() else { continue };
        let Some(text) = help_text(strings, id) else { continue };
        if text_clears_status(text) {
            out.push((item, plain(text)));
        }
    }
    out
}

/// The tier (0 Cure, 1 Cura, 2 Curaga) of a core effect that is Cure's heal landing on Sora.
pub fn cure_tier(effect: &str) -> Option<usize> {
    (0..3).find(|t| kh_core::host::audio::cure_heal_effect(*t) == effect)
}

/// The items whose effect landed since `before` (the status's `item_effects` counters, `ItemKind::ALL` order). A counter
/// that went DOWN (a new status) is no use.
pub fn items_landed(before: &[u32; 12], now: &[u32; 12]) -> Vec<ItemKind> {
    (0..12).filter(|k| now[*k] > before[*k]).map(|k| ItemKind::ALL[k]).collect()
}

/// What a meter rests at when nothing builds it up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rest {
    Zero,
    Full,
}

/// Learns, per meter, whether it rests at 0 or at its maximum (most of the frames with a maximum).
#[derive(Clone, Copy, Debug, Default)]
pub struct GaugeRest {
    pub zero: [u32; 7],
    pub full: [u32; 7],
}

impl GaugeRest {
    /// Frames a meter must have been seen before its rest value counts as known.
    pub const FRAMES: u32 = 120;

    pub fn observe(&mut self, values: &[u32; 7], max: &[u32; 7]) {
        for i in 0..7 {
            if max[i] == 0 {
                continue;
            }
            if values[i] == 0 {
                self.zero[i] = self.zero[i].saturating_add(1);
            } else if values[i] >= max[i] {
                self.full[i] = self.full[i].saturating_add(1);
            }
        }
    }

    /// The rest value of meter `i` once known (9 in 10 of its seen frames at one end).
    pub fn rest(&self, i: usize) -> Option<Rest> {
        let n = self.zero[i] + self.full[i];
        if n < Self::FRAMES {
            None
        } else if self.zero[i] * 10 >= n * 9 {
            Some(Rest::Zero)
        } else if self.full[i] * 10 >= n * 9 {
            Some(Rest::Full)
        } else {
            None
        }
    }

    /// The value meter `i` is reset to (its rest value), when known and the meter is not at rest now.
    pub fn reset_value(&self, i: usize, value: u32, max: u32) -> Option<u32> {
        let want = match self.rest(i)? {
            Rest::Zero => 0,
            Rest::Full => max,
        };
        (max > 0 && value != want).then_some(want)
    }

    /// How far meter `i` is built up, 0..1 (`None`: its direction is not known yet).
    pub fn built(&self, i: usize, value: u32, max: u32) -> Option<f32> {
        if max == 0 {
            return None;
        }
        let v = value.min(max) as f32 / max as f32;
        Some(match self.rest(i)? {
            Rest::Zero => v,
            Rest::Full => 1.0 - v,
        })
    }
}

/// A damage-over-time special effect on the body: `rate` percent of the maximum and `point` HP per tick (Elden Ring's
/// `changeHpRate` / `changeHpPoint`, positive = damage).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dot {
    pub id: i32,
    pub rate: f32,
    pub point: i32,
}

impl Dot {
    /// The tick at a maximum of `max` HP.
    pub fn tick(&self, max: i32) -> f32 {
        max as f32 * self.rate / 100.0 + self.point as f32
    }

    /// The loss `lost` at the body's maximum `max` is one tick of this effect (within 2 HP or 15 %).
    pub fn matches(&self, lost: i32, max: i32) -> bool {
        let t = self.tick(max);
        t > 0.0 && (lost as f32 - t).abs() <= (0.15 * t).max(2.0)
    }
}

/// What one HP loss of the body is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Loss {
    /// A hit as before (the share decides Small / Blow; `None` kind = below the minimum share).
    Hit,
    /// A status tick of ailment `ailment`: no reaction. `own_loss` = what the same tick would have taken at the
    /// Tarnished's OWN maximum (the body's maximum is raised `damage.in.body_hp` times: a tick in percent of the
    /// maximum is that much larger on it). `dot` = the effect it matched, if one did.
    Tick { ailment: usize, own_loss: i32, dot: Option<Dot> },
    /// (IMPL_ER50, the user: "bleed or frostbite should care about Sora's maximum HP % not Elden Ring's") A PROC
    /// (hemorrhage, frostbite ...: `ailment` when a meter said which) landed with this loss: `pct` percent of the
    /// maximum is taken of SORA'S maximum; `flat` = the rest of the loss (the proc's flat part and the attack that came
    /// with it) goes through the `damage.in` share rule. `dot` = the proc's own effect (`None`: the numbers are the
    /// fallback guess, `guessed`). A hit: it reacts.
    Proc { ailment: Option<usize>, pct: f32, flat: i32, dot: Option<Dot>, guessed: bool },
}

/// What says a proc landed this frame: the damage effects that came onto the body in the last moments (`fresh`), the
/// ailment whose meter just emptied or whose timer just started (`ailment`), and the fallback percent for it when no
/// effect of its own is seen (`guess_pct`, A GUESS from public data, configurable).
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcSign<'a> {
    pub fresh: &'a [Dot],
    pub ailment: Option<usize>,
    pub guess_pct: Option<f32>,
}

/// The ailments whose proc is a burst of damage (hemorrhage, deathblight is a death, frostbite, madness).
pub const BURSTS: [usize; 3] = [2, 4, 6];

/// The proc of effect `dot` is in a loss of `lost` at the body's maximum `body_max`: (percent, the rest of the loss).
/// The loss must hold at least the proc's own damage (within 2 HP / 15 %); the rest is its flat part and whatever
/// came with it.
pub fn split_proc(lost: i32, body_max: i32, dot: &Dot) -> Option<(f32, i32)> {
    let predicted = dot.tick(body_max);
    if predicted <= 0.0 || (lost as f32) < predicted - (0.15 * predicted).max(2.0) {
        return None;
    }
    let pct_part = body_max as f32 * dot.rate.max(0.0) / 100.0;
    Some((dot.rate.max(0.0), (lost as f32 - pct_part).round().max(0.0) as i32))
}

/// What a proc takes off Sora: `pct` percent of HIS maximum, and the flat rest by the `damage.in` share rule.
pub fn proc_amount(params: &crate::ermap::Params, pct: f32, flat: i32, own_max: i32, sora_max: i32) -> f32 {
    sora_max as f32 * pct / 100.0 + if flat > 0 { params.damage_in(flat, own_max, sora_max) } else { 0.0 }
}

/// The same proc at the Tarnished's own maximum (for the hit's kind: Small / Blow by share).
pub fn proc_own_loss(pct: f32, flat: i32, own_max: i32) -> i32 {
    (own_max as f32 * pct / 100.0).round() as i32 + flat
}

/// Classifies a loss of `lost` HP of the body (maximum now `body_max`, the Tarnished's own `own_max`).
/// `active` = the over-time ailment running on the body, if any (its timer); `dots` = the damage-over-time effects on
/// it; `cap_share` = the largest share of his own maximum a tick may be when no effect matches it.
pub fn classify(lost: i32, own_max: i32, body_max: i32, active: Option<usize>, dots: &[Dot], cap_share: f32) -> Loss {
    classify_with(lost, own_max, body_max, active, dots, cap_share, &ProcSign::default())
}

/// As [`classify`], with the signs of a proc. A proc comes first: a fresh damage effect that the loss holds, when a
/// burst ailment's meter says so or no over-time ailment runs (else a fresh effect is an over-time ailment's first
/// tick); then a burst ailment's meter alone, with the fallback percent; then the ticks.
pub fn classify_with(lost: i32, own_max: i32, body_max: i32, active: Option<usize>, dots: &[Dot], cap_share: f32, sign: &ProcSign) -> Loss {
    if lost <= 0 || own_max <= 0 {
        return Loss::Hit;
    }
    let body_max = body_max.max(own_max);
    if sign.ailment.is_some() || active.is_none() {
        // (the largest fresh effect the loss holds)
        let best = sign.fresh.iter().filter_map(|d| split_proc(lost, body_max, d).map(|s| (d, s))).max_by(|a, b| a.0.tick(body_max).total_cmp(&b.0.tick(body_max)));
        if let Some((dot, (pct, flat))) = best {
            return Loss::Proc { ailment: sign.ailment, pct, flat, dot: Some(*dot), guessed: false };
        }
    }
    if let (Some(ailment), Some(guess)) = (sign.ailment, sign.guess_pct) {
        // the guess cannot take more than the loss: then the whole loss is taken as the percent of the body's maximum
        let pct = guess.min(lost as f32 / body_max as f32 * 100.0).max(0.0);
        let flat = (lost as f32 - body_max as f32 * pct / 100.0).round().max(0.0) as i32;
        return Loss::Proc { ailment: Some(ailment), pct, flat, dot: None, guessed: true };
    }
    let Some(ailment) = active else { return Loss::Hit };
    if let Some(dot) = dots.iter().find(|d| d.matches(lost, body_max)) {
        let own_loss = (dot.tick(own_max).round() as i32).max(1).min(lost);
        return Loss::Tick { ailment, own_loss, dot: Some(*dot) };
    }
    if (lost as f32) <= own_max as f32 * cap_share {
        return Loss::Tick { ailment, own_loss: lost, dot: None };
    }
    Loss::Hit
}

/// The over-time ailment running, from the body's ailment timers (seconds left, SDK `proc_status_timers`).
pub fn over_time_active(timers: &[f32; 7]) -> Option<usize> {
    OVER_TIME.into_iter().find(|i| timers[*i] > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ELIXIR: &str = "Fully restores HP/MP/Focus to one<br>teammate and removes all negative<br>status effects. <color(strong)>(Equippable)<color>";
    const PANACEA: &str = "Removes all negative status effects<br>from one teammate and provides<br>temporary immunity. <color(strong)>(Equippable)<color>";
    const TENT: &str = "Fully restores HP/MP/Focus to your<br>whole team. Use this item from the menu.";

    #[test]
    fn the_text_decides() {
        assert!(text_clears_status(ELIXIR) && text_clears_status(PANACEA));
        assert!(!text_clears_status(TENT) && !text_clears_status("Restores 80 HP to one teammate.<br><color(strong)>(Equippable)<color>"));
        assert_eq!(plain(ELIXIR), "Fully restores HP/MP/Focus to one teammate and removes all negative status effects. (Equippable)");
    }

    #[test]
    fn items_from_the_pack_text() {
        let icons = serde_json::json!({ "help": {
            "ITEM_ELIXIR": "kh3_item.battle.detail_elixir", "ITEM_LASTELIXIR": "kh3_item.battle.detail_lastelixir",
            "ITEM_ALLCURE": "kh3_item.battle.detail_allcure", "ITEM_TENT": "kh3_item.battle.detail_tent" } });
        let strings = serde_json::json!({ "namespaces": { "kh3_item.battle": {
            "detail_elixir": ELIXIR, "detail_lastelixir": ELIXIR.replace("one<br>teammate", "your<br>whole team"),
            "detail_allcure": PANACEA, "detail_tent": TENT } } });
        let items: Vec<ItemKind> = clearing_items(&icons, &strings).into_iter().map(|(i, _)| i).collect();
        // (the pouch's ITEM_MEGAELIXIR is the game's ITEM_LASTELIXIR)
        assert_eq!(items, vec![ItemKind::Elixir, ItemKind::Megalixir, ItemKind::Panacea]);
    }

    /// The pack's own texts (skipped when the pack is not built): Elixir, Megalixir and Panacea say so, nothing else.
    #[test]
    fn the_real_pack_decides() {
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
        let read = |rel: &str| std::fs::read_to_string(pack.join(rel)).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        let (Some(icons), Some(strings)) = (read("data/item_icons.json"), read("strings/en/kh3_item.json")) else { return };
        let items: Vec<ItemKind> = clearing_items(&icons, &strings).into_iter().map(|(i, _)| i).collect();
        assert_eq!(items, vec![ItemKind::Elixir, ItemKind::Megalixir, ItemKind::Panacea]);
    }

    #[test]
    fn cure_tiers_and_items() {
        assert_eq!([cure_tier("vm_cur_hed0"), cure_tier("vm_cur_hed1"), cure_tier("vm_cur_hed2"), cure_tier("vs_itm_elx0")], [Some(0), Some(1), Some(2), None]);
        let mut now = [0; 12];
        now[5] = 1; // Elixir
        assert_eq!(items_landed(&[0; 12], &now), vec![ItemKind::Elixir]);
        assert!(items_landed(&now, &[0; 12]).is_empty());
    }

    #[test]
    fn meters_learn_their_rest() {
        let mut g = GaugeRest::default();
        let max = [300; 7];
        let mut v = [0u32; 7];
        v[1] = 300; // rot rests full (resistance left)
        for _ in 0..GaugeRest::FRAMES {
            g.observe(&v, &max);
        }
        assert_eq!((g.rest(0), g.rest(1)), (Some(Rest::Zero), Some(Rest::Full)));
        assert_eq!((g.reset_value(0, 120, 300), g.reset_value(1, 120, 300), g.reset_value(0, 0, 300)), (Some(0), Some(300), None));
        assert_eq!((g.built(0, 150, 300), g.built(1, 75, 300)), (Some(0.5), Some(0.75)));
        assert_eq!(GaugeRest::default().reset_value(0, 120, 300), None);
    }

    #[test]
    fn ticks_and_hits() {
        // poison running; its effect takes 0.1 % + 5 per tick: at the raised maximum 3960 that is 8.96
        let dot = Dot { id: 1, rate: 0.1, point: 5 };
        assert_eq!(classify(9, 396, 3960, Some(0), &[dot], 0.05), Loss::Tick { ailment: 0, own_loss: 5, dot: Some(dot) });
        // no effect matched: a small loss is still a tick, unchanged
        assert_eq!(classify(12, 396, 3960, Some(1), &[], 0.05), Loss::Tick { ailment: 1, own_loss: 12, dot: None });
        // an attack while poisoned is a hit
        assert_eq!(classify(120, 396, 3960, Some(0), &[dot], 0.05), Loss::Hit);
        // nothing running: a hit (burning flinches)
        assert_eq!(classify(3, 396, 3960, None, &[dot], 0.05), Loss::Hit);
        assert_eq!(over_time_active(&[0.0, 2.5, 0.0, 0.0, 0.0, 0.0, 0.0]), Some(1));
        assert_eq!(over_time_active(&[0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0]), None);
    }

    #[test]
    fn a_proc_is_a_percent_of_his_maximum_whatever_the_body_holds() {
        let p = crate::ermap::Params::default();
        // a hemorrhage proc of 15 % + 100 (the effect's own numbers), the Tarnished's own maximum 522, Sora's 300
        let bleed = Dot { id: 6800, rate: 15.0, point: 100 };
        let sign = ProcSign { fresh: &[bleed], ailment: Some(2), guess_pct: Some(15.0) };
        let mut amounts = Vec::new();
        for k in [1, 10, 25] {
            let body = 522 * k;
            let lost = (body as f32 * 0.15).round() as i32 + 100;
            let Loss::Proc { pct, flat, dot, guessed, ailment } = classify_with(lost, 522, body, None, &[bleed], 0.05, &sign) else { panic!("not a proc at x{k}") };
            assert_eq!((pct, dot, guessed, ailment), (15.0, Some(bleed), false, Some(2)));
            assert!((flat - 100).abs() <= 1, "flat {flat} at x{k}");
            amounts.push(proc_amount(&p, pct, flat, 522, 300));
        }
        // 15 % of HIS 300 = 45, plus the flat 100 of 522 as a share of his 300
        let want = 45.0 + p.damage_in(100, 522, 300);
        assert!(amounts.iter().all(|a| (a - want).abs() < 1.0), "{amounts:?} vs {want}");
        // the attack that built it up comes in the same loss: it is in the flat part
        let Loss::Proc { pct, flat, .. } = classify_with(783 + 100 + 60, 522, 5220, None, &[bleed], 0.05, &sign) else { panic!() };
        assert_eq!((pct, flat), (15.0, 160));
        assert_eq!(proc_own_loss(pct, flat, 522), 78 + 160);
    }

    #[test]
    fn proc_without_its_effect_and_ticks_beside_it() {
        // frostbite's meter emptied, no effect of its own seen: the fallback percent (a guess), never more than the loss
        let sign = ProcSign { fresh: &[], ailment: Some(4), guess_pct: Some(10.0) };
        assert_eq!(classify_with(522 + 30, 522, 5220, None, &[], 0.05, &sign), Loss::Proc { ailment: Some(4), pct: 10.0, flat: 30, dot: None, guessed: true });
        let Loss::Proc { pct, flat, .. } = classify_with(200, 522, 5220, None, &[], 0.05, &sign) else { panic!() };
        assert!((pct - 200.0 / 5220.0 * 100.0).abs() < 1e-3 && flat == 0);
        // poison running and a fresh effect without a burst meter = poison's first tick, not a proc
        let poison = Dot { id: 1, rate: 0.1, point: 5 };
        let first = ProcSign { fresh: &[poison], ailment: None, guess_pct: None };
        assert!(matches!(classify_with(9, 396, 3960, Some(0), &[poison], 0.05, &first), Loss::Tick { .. }));
        // a fresh effect the loss does not hold is no proc: the loss is a hit
        let big = Dot { id: 2, rate: 50.0, point: 0 };
        assert_eq!(classify_with(100, 522, 5220, None, &[big], 0.05, &ProcSign { fresh: &[big], ..ProcSign::default() }), Loss::Hit);
    }
}

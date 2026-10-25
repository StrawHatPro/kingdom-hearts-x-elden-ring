//! (party X2) THE FRIEND LEADER'S FRAMEWORK, HOST SIDE (both hosts): Sora's magic and both leaders' Customize blocks
//! fed to the party every step a friend leads (kh-party leader_magic.rs), the camp menu's edits of a leader's block,
//! and the block in save.kh (`party.custom`, additive: a save without it fills from the defaults at the first sync).
//!
//! THE BLOCKS ARE THE RUNTIME'S (`PartyRuntime::customs`): the party (`Party`) is made again at every spawn, the
//! blocks live on. `menu_members` hands each friend's block to the HUD (Customize shows the leader's).

use kh_core::command::leader::{LeaderCustom, LeaderKind, MagicBook};
use kh_host::party::{CustomEdit, PartyChara};
use kh_party::leader_magic::LeaderFeed;
use serde_json::{json, Value};

use crate::runtime::PartyRuntime;

/// Both leaders' blocks (index = `LeaderKind as usize`) and Sora's book of the last feed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Customs {
    pub blocks: [LeaderCustom; 2],
    pub book: MagicBook,
    /// (EQ) The battle items of the shared stock (Sora's pouch stock, count > 0) at the last feed: a leader's shortcut
    /// list offers them as Sora's does (his slots' items, then the stock's: kh-core customize `assignable`).
    pub stock: Vec<kh_core::status::ItemKind>,
    /// (DON-UI) The lock-on target's (weak, resisted) elements for Elemental Insight, when the host knows them (kh-er /
    /// kh-app set it every frame for the leader's lock-on target; None: unknown, nothing is marked).
    pub don_affinity: Option<(kh_party::donald::ElemSet, kh_party::donald::ElemSet)>,
}

pub fn leader_kind(c: PartyChara) -> Option<LeaderKind> {
    match c {
        PartyChara::Donald => Some(LeaderKind::Donald),
        PartyChara::Goofy => Some(LeaderKind::Goofy),
        PartyChara::Sora => None,
    }
}

impl Customs {
    pub fn block(&self, c: PartyChara) -> Option<&LeaderCustom> {
        leader_kind(c).map(|k| &self.blocks[k as usize])
    }

    /// The book of Sora's status now; the blocks synced to it (the first fill, Sora's learn rule).
    pub fn sync(&mut self, book: MagicBook) {
        self.book = book;
        for kind in LeaderKind::ALL {
            self.blocks[kind as usize].sync(kind, &book);
        }
    }

    /// The save shape: `{ "donald": {...}, "goofy": {...} }` (kh-core `LeaderCustom::to_json`).
    pub fn to_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        for kind in LeaderKind::ALL {
            let b = &self.blocks[kind as usize];
            if b.filled {
                map.insert(kind.key().to_string(), serde_json::from_str(&b.to_json()).unwrap_or(Value::Null));
            }
        }
        Value::Object(map)
    }

    /// From the save shape; a missing character (or a missing record) keeps the defaults: filled at the next sync.
    pub fn from_json(v: &Value) -> Customs {
        let mut out = Customs::default();
        for kind in LeaderKind::ALL {
            let r = &v[kind.key()];
            if !r.is_object() {
                continue;
            }
            let nums = |x: &Value| x.as_array().map(|a| a.iter().map(|n| n.as_u64().unwrap_or(0)).collect::<Vec<_>>()).unwrap_or_default();
            let chosen: Vec<bool> = r["chosen"].as_array().map(|a| a.iter().map(|n| n.as_u64().unwrap_or(0) != 0 || n.as_bool().unwrap_or(false)).collect()).unwrap_or_default();
            let pages: Vec<Vec<u64>> = r["shortcuts"].as_array().map(|a| a.iter().map(nums).collect()).unwrap_or_default();
            out.blocks[kind as usize] = LeaderCustom::from_parts(&nums(&r["magic"]), &chosen, &pages, r["filled"].as_bool().unwrap_or(true));
        }
        out
    }
}

impl PartyRuntime {
    /// (route, before the leader's pad frame) Sora's magic and the blocks to the party.
    pub fn feed_leader_magic(&mut self, sora: &kh_core::Sora) {
        let s = sora.status();
        self.customs.blocks[LeaderKind::Donald as usize].don = self.don_book(); // (DON-UI) his block reads his own book
        self.customs.sync(MagicBook::of(s, sora.command().magic_slots));
        self.customs.stock = sora.magic().pouch.stock.iter().filter(|e| e.1 > 0).map(|e| e.0).collect(); // (EQ)
        // a team attack on offer in Sora's queue comes first on Y (the user's rule: the leader starts team attacks)
        // (YHUD) only one WITH THE LEADER (kh_party::yhud): Sora's other rows are never the leading friend's Y
        let rows = self.leader_team_rows(sora);
        let team = !rows.is_empty();
        let feed = LeaderFeed { book: self.customs.book, sora_abilities: s.abilities.clone(), game_level: s.game_level, battle_level: s.battle_level, mp_cost_scale: s.mp_cost_scale, customs: self.customs.blocks, sora_team_offer: team };
        self.party.set_leader_magic(Some(feed));
        if let Some(id) = self.leader {
            self.party.set_leader_team(id, rows); // (YHUD)
        }
        self.party.set_lead_reference(Some((s.max_hp(), s.max_mp()))); // (GOOFY-BAL) a friend leader's HP / MP follow Sora's
        self.feed_don_menu(sora); // (DON-UI) Donald's menu: his block, the ally target list, the lock-on target, Y's team rows
    }

    /// A camp-menu edit of a leader's block (`PartyMenuEdit::Custom`); false = refused (the rules of kh-core
    /// command/leader.rs: a learned tier of the slot's element, a command of his list).
    pub fn apply_custom_edit(&mut self, c: PartyChara, edit: CustomEdit) -> bool {
        let Some(kind) = leader_kind(c) else { return false };
        let items: Vec<kh_core::status::ItemKind> = self.id(c).and_then(|id| self.party.status(id)).map(|s| s.items.iter().take(s.item_slots as usize).filter_map(|i| i.item).collect()).unwrap_or_default(); // (EQ) a used auto-reload slot remembers its item (Sora's rule)
        let items: Vec<kh_core::status::ItemKind> = items.into_iter().chain(self.customs.stock.iter().copied()).collect(); // (EQ) + the shared stock, as Sora's list
        let book = self.customs.book;
        let b = &mut self.customs.blocks[kind as usize];
        let ok = match edit {
            CustomEdit::SetMagicSlot { slot, kind: cmd } => b.set_slot(kind, &book, slot, cmd),
            CustomEdit::SwapMagicSlots(a, x) => b.swap(kind, a, x),
            CustomEdit::SetShortcut { page, slot, kind: cmd } => b.set_shortcut(kind, &book, &items, page, slot, cmd),
        };
        crate::log::line(&format!("party: {} Customize {edit:?}: {}", c.name(), if ok { "set" } else { "REFUSED" }));
        ok
    }

    /// `party.custom` of the save record.
    pub fn custom_record(&self) -> Value {
        self.customs.to_json()
    }

    /// A loaded `party.custom` (null / missing = the defaults, filled at the next feed).
    pub fn load_custom_record(&mut self, v: &Value) {
        self.customs = Customs { book: self.customs.book, stock: std::mem::take(&mut self.customs.stock), ..Customs::from_json(v) }; // (EQ) stock kept
    }
}

/// DEBUG WORDS (`party x2 ...`, the test level's script / console): `status` (the leader's MP, charge, gauge, offers),
/// `gauge` (the leader's formchange gauge full: the next step rolls), `offer KIND` (a grand-magic offer at once,
/// 110 .. 125).
pub fn word(rt: &mut PartyRuntime, w: &[&str]) -> String {
    let Some(id) = rt.leader else { return "party x2: no friend leads".into() };
    let name = rt.leader_chara().name();
    let ensure = |rt: &mut PartyRuntime| {
        if rt.party.caster(id).is_none() {
            rt.party.x2.casters.push((id, kh_core::magic::caster::SpellCaster::new(kh_core::host::PartyMember(id.0))));
        }
    };
    match w {
        ["gauge"] => {
            ensure(rt);
            if let Some(c) = rt.party.caster_mut(id) {
                c.debug_fill_gauge();
            }
            format!("party x2: {name}'s formchange gauge full (the roll at the next step)")
        }
        ["offer", kind] => {
            let Some(k) = kind.parse::<u8>().ok().filter(|k| kh_core::magic::spells::is_grand(*k)) else { return "party x2: offer KIND (110 .. 125)".into() };
            ensure(rt);
            if let Some(c) = rt.party.caster_mut(id) {
                c.debug_offer(k);
            }
            format!("party x2: {name} offered {}", kh_core::magic::spells::command_name(k))
        }
        _ => {
            let s = rt.party.status(id);
            let c = rt.party.caster(id);
            format!(
                "party x2: {name} MP {} / {} charge {:.1} s; gauge {:?}; offers {:?}; actors {}; shots {}",
                s.map_or(0, |s| s.mp),
                s.map_or(0, |s| s.max_mp()),
                s.map_or(0.0, |s| s.mp_charge_left),
                c.map(|c| c.gauge()),
                c.map(|c| c.offers().iter().map(|o| kh_core::magic::spells::command_name(o.cmd)).collect::<Vec<_>>()),
                c.map_or(0, |c| c.alive()),
                c.map_or(0, |c| c.shots)
            )
        }
    }
}

/// The MP costs of a friend's own list entries for the Customize help (Goofy's abilities by tier: their
/// `TresAbilityData` m_UseMP, the same at every tier).
pub fn costs(c: PartyChara) -> Vec<(u8, i32)> {
    use kh_core::command::leader::goofy;
    match c {
        PartyChara::Goofy => (0..3u8).flat_map(|i| (1..=3u8).filter_map(move |t| kh_party::tables::ability_cost(goofy::ABILITY_ROWS[i as usize]).map(|(_, mp)| (goofy::id(i, t), mp)))).collect(),
        _ => Vec::new(),
    }
}

/// The record key in the save's party record.
pub const RECORD_KEY: &str = "custom";

/// An empty record (for hosts that write the field before any feed).
pub fn empty_record() -> Value {
    json!({})
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::magic::spells::cmd;

    /// (DON-UI) A REAL X2-ERA RECORD (Donald's block of Sora's spell ids, filled) loads, keeps the player's choices and
    /// becomes his own ids at the first sync: each spell his own of the same element at the tier he knows (none: emptied),
    /// the items kept, Goofy's block untouched.
    #[test]
    fn an_x2_era_customize_record_loads_and_migrates() {
        use kh_core::command::leader::donald as d;
        let v: Value = serde_json::from_str(r#"{"donald":{"magic":[31,43,51,35,0,0],"chosen":[0,0,0,0,0,0],"shortcuts":[[31,51,43,58],[33,0,0,0],[0,0,0,61]],"filled":true},"goofy":{"magic":[195,198,201,0,0,0],"chosen":[0,0,0,0,0,0],"shortcuts":[[195,198,201,58],[0,0,0,0],[0,0,0,0]],"filled":true}}"#).unwrap();
        let mut c = Customs::from_json(&v);
        assert!(c.blocks[0].filled);
        c.blocks[0].don = d::DonBook { tier: [3, 0, 0, 2, 0, 1], line2: [1, 0, 0, 1, 0, 0], dual: false };
        c.sync(MagicBook { tiers: [3, 0, 0, 2, 0, 1], sora_slots: [cmd::FIRAGA, 0, 0, 0, 0, 0] });
        let b = c.blocks[0];
        assert_eq!(b.shortcuts[0], [d::line1(0, 3), d::line1(5, 1), d::line1(3, 2), cmd::POTION], "Firaga, Curaga -> Cure and Wataga -> Watera (his tiers), the Potion: his own ids");
        assert_eq!(b.shortcuts[1][0], 0, "Blizzard: he has none -> empty");
        assert_eq!(b.shortcuts[2][3], cmd::ETHER, "items kept");
        assert_eq!(b.slots(LeaderKind::Donald), vec![d::line1(0, 1), d::line1(3, 1), d::line1(5, 1)], "his Magic page: his elements");
        assert_eq!(c.blocks[1].shortcuts[0], [195, 198, 201, 58], "Goofy untouched");
    }

    #[test]
    fn the_record_round_trips_and_an_old_save_fills_from_the_defaults() {
        let mut c = Customs::default();
        c.sync(MagicBook { tiers: [3; 6], sora_slots: [cmd::FIRAGA, cmd::WATAGA, cmd::CURAGA, cmd::BLIZZAGA, cmd::THUNDAGA, cmd::AEROGA] });
        c.blocks[0].shortcuts[1][2] = cmd::THUNDARA;
        let v = c.to_json();
        assert_eq!(v["donald"]["shortcuts"][1][2], json!(cmd::THUNDARA));
        let back = Customs::from_json(&v);
        assert_eq!(back.blocks, c.blocks);
        // an old save: no record -> unfilled; the next sync fills the defaults
        let mut old = Customs::from_json(&Value::Null);
        assert!(!old.blocks[0].filled);
        old.sync(c.book);
        // (DON-UI) Donald's defaults are his own (the overhaul's start book: Fire, Flare, Water, Flood)
        assert_eq!(old.blocks[0].shortcuts[0], kh_core::command::leader::donald::default_shortcuts(&kh_core::command::leader::donald::DonBook::START)[0]);
        assert_eq!(old.blocks[1].magic_slots[..3], [0xC3, 0xC6, 0xC9]);
    }
}

// ------------------------------------------------------------------- (party X2-b) the leader's HUD bar, aim, kit rows

/// The kit category of a port ability (`ETresAbilityCategory` 0 Actions, 1 Mobility, 2 Support) and its help line
/// (XD_PLAYABLE_KITS.md 1.7 / 2.6, approved 2026-10-15: our abilities, our text).
fn kit_look(key: &str) -> (u8, &'static str) {
    match key {
        "KIT_SPELL_WEAVER" => (2, "Each different element cast during a combo\nadds 10% damage to its finisher (up to 30%)."),
        "KIT_COUNTER_BLAST" => (0, "Press the attack button right after a\nMagic Block to fire a free Fire bolt."),
        "KIT_QUICK_CHANT" => (2, "Every follow-up spell can be cast\nstraight after the previous one."),
        "KIT_MANA_REBOUND" => (2, "Finisher hits restore MP, or shorten\nthe MP charge."),
        "KIT_METEOR_FINALE" => (0, "With full MP, the ground finisher becomes\nMeteor Drop. Uses all MP."),
        "KIT_BLINK_STEP" => (1, "Chain a second Hop, and Air Blink\none more time per jump."),
        "KIT_AERO_FLOAT" => (1, "Hold the jump button while falling\nto float on the wind for a short time."),
        "KIT_IRON_WALL" => (2, "Widens the timing of a Perfect Guard."),
        "KIT_PAYBACK_BASH" => (0, "Shield bashes after a block deal\nmore damage and knock foes away."),
        "KIT_SHIELD_CHARGE" => (1, "While guarding, push the stick and press\nthe attack button to charge with the shield up."), // (GOOFY-GK) the input is A
        "KIT_STALWART" => (2, "Light hits no longer interrupt\nattacks or guarding."),
        "KIT_RALLY" => (2, "Each Perfect Guard restores MP."),
        "KIT_RICOCHET" => (2, "Goofy Discus bounces to one more target."),
        "KIT_OVER_HERE" => (0, "A shield bash draws the attention of\nnearby enemies for a short time."),
        "KIT_TORNADO_COPTER" => (1, "Hold the jump button while falling\nto glide in a spinning hover."),
        // (GOOFY-GK) the tank rework's combo abilities (build/sora_tmp/party/GK_GOOFY_DESIGN.md section 8)
        "KIT_COMBO_PLUS" | "KIT_COMBO_PLUS_2" => (0, "Adds one more hit to Goofy's\nground combo."),
        "KIT_AIR_COMBO_PLUS" => (0, "Adds a shield bash to the end\nof Goofy's air combo."),
        "KIT_SHIELD_QUAKE" => (0, "Hold the attack button on a combo's\nfinishing blow to slam the ground."),
        "KIT_SHIELD_UPPER" => (0, "Hold the stick away from the target on a\nfinishing blow to launch it skyward."),
        _ => (2, ""),
    }
}

/// (DON-UI) The Abilities page's category of one of Donald's abilities (`ETresAbilityCategory`: 0 Actions, 1 Mobility, 2
/// Support): his combat abilities are Actions, Aero Float Mobility, the magic and party ones Support (KH3's grouping).
pub fn don_category(k: kh_party::donald::DonAbility) -> u8 {
    use kh_party::donald::DonAbility as D;
    match k {
        D::ComboPlus | D::ComboPlus2 | D::AirComboPlus | D::ChargedMissilePlus | D::MissileVolley | D::ArcaneBolt | D::Starfall | D::WandBonkPlus | D::CastGuard | D::SecondChance | D::WithstandCombo => 0,
        D::AeroFloat => 1,
        _ => 2,
    }
}

/// A friend's owned kit abilities as his Abilities page lists them.
pub fn kit_rows(s: &kh_party::FriendStatus) -> Vec<kh_host::party::KitRow> {
    use kh_core::enums::ETresAbilityKind as A;
    use kh_party::pad::donald_kit::KitAbility as D;
    use kh_party::pad::goofy_kit::GoofyAbility as G;
    let row = |key: &str, name: &str, ap: i32, equipped: bool, kh3: Option<A>| {
        let (category, help) = kit_look(key);
        kh_host::party::KitRow { key: key.to_string(), name: name.to_string(), ap, category: if kh3.is_some() { 0 } else { category }, equipped, help: help.to_string(), kh3 }
    };
    let mut out = Vec::new();
    // (DON-UI) DONALD'S OWN ABILITIES (the overhaul, DONALD_OVERHAUL.md 9): his `DonAbility` rows (label, DON-PROG's
    // description and AP) replace the old X3D kit rows on his Abilities page
    if let Some(d) = s.donald.as_ref() {
        for (k, on) in d.abilities.list() {
            out.push(kh_host::party::KitRow { key: k.name(), name: k.label().to_string(), ap: k.ap(), category: don_category(k), equipped: on, help: k.description().to_string(), kh3: None });
        }
    }
    for (k, on) in s.kit.list().into_iter().filter(|_| s.donald.is_none()) {
        let kh3 = match k {
            D::UnisonFire => Some(A::UnisonFire),
            D::UnisonBlizzard => Some(A::UnisonBlizzard),
            D::UnisonThunder => Some(A::UnisonThunder),
            _ => None,
        };
        out.push(row(k.name(), k.label(), k.ap(), on, kh3));
    }
    for (k, on) in s.goofy_kit.list() {
        let kh3 = match k {
            G::FusionSpin => Some(A::FusionSpin),
            G::FusionRocket => Some(A::FusionRocket),
            _ => None,
        };
        out.push(row(k.name(), k.label(), k.ap(), on, kh3));
    }
    out
}

/// THE LEADER'S BAR in the focus-gauge slot (XD 2.1: Goofy's shield gauge on his HUD ring, the focus-bar slot):
/// Goofy's shield gauge, his Discus Barrage focus while he aims; Donald's Spell Volley focus. None for an AI friend.
pub fn leader_focus(party: &kh_party::Party, id: kh_party::MemberId) -> Option<(i32, i32)> {
    let m = party.member(id)?;
    let pad = m.pad.as_ref()?;
    match m.chara {
        // (GOOFY-GG) the user (2026-10-16 23:40): "Instead of using Focus, he should have a new bar above MP called GUARD":
        // Goofy has no bar in the focus slot any more; his GUARD bar is `leader_guard_hud`
        kh_party::Chara::Goofy => {
            let _ = pad;
            None
        }
        // (DON-UI) the overhaul removed Spell Volley (no shotlock, DONALD_OVERHAUL.md 13): the slot is his Resonance ring's,
        // drawn by kh_ui::hud_resonance from `don_hud_view`
        kh_party::Chara::Donald => None,
    }
}

/// (GOOFY-GG) GOOFY'S GUARD BAR (kh_ui::hud_guard, above MP; the user 2026-10-16 23:40): his GUARD meter
/// (`FriendStatus::guard`, kh-party guard.rs) and the look of the sheet `party-guard` while Goofy is the controlled
/// leader; None otherwise (Sora / Donald lead, or Goofy is AI-run: an AI friend's slot shows HP and MP only).
pub fn leader_guard_hud(party: &kh_party::Party, id: kh_party::MemberId) -> Option<(kh_host::party::GuardValues, kh_host::party::GuardLook)> {
    let m = party.member(id)?;
    if m.chara != kh_party::Chara::Goofy || m.pad.is_none() {
        return None;
    }
    let g = party.status(id)?.guard_view()?;
    let p = kh_party::guard::GP;
    let values = kh_host::party::GuardValues { value: g.value, max: g.max, broken: g.broken, regenerating: g.regenerating, rearm_at: g.rearm_at };
    let look = kh_host::party::GuardLook { fill: p.hud_color_fill, shine: p.hud_color_shine, loss: p.hud_color_loss, regen: p.hud_color_regen, broken: p.hud_color_broken, blink_hz: p.hud_broken_blink, offset: (p.hud_offset_x, p.hud_offset_y) };
    Some((values, look))
}

/// (GOOFY-GG) DEBUG words for test runs (kh-app `party-word gg ...`) on Goofy's GUARD: `spend N`, `absorb N`, `drain N`
/// (N points at once through `drain_per_sec`), `break`, `refill`, `show`.
pub fn guard_word(rt: &mut PartyRuntime, words: &[&str]) -> String {
    let Some(id) = rt.id(PartyChara::Goofy) else { return "gg: Goofy is not in the party".to_string() };
    let Some(g) = rt.party.status_mut(id).and_then(|s| s.guard.as_mut()) else { return "gg: Goofy has no GUARD".to_string() };
    let n = |i: usize| words.get(i).and_then(|w| w.parse::<f32>().ok()).unwrap_or(0.0);
    let what = match words {
        ["spend", ..] => format!("spend {} -> {}", n(1), g.spend(n(1))),
        ["absorb", ..] => format!("absorb {:?}", g.absorb(n(1))),
        ["drain", ..] => format!("drain {} -> {}", n(1), g.drain_per_sec(n(1), 1.0)),
        ["break"] => {
            g.break_now();
            "break".to_string()
        }
        ["refill"] => {
            g.refill();
            "refill".to_string()
        }
        ["show"] | [] => "show".to_string(),
        _ => return format!("gg: {}: spend N | absorb N | drain N | break | refill | show", words.join(" ")),
    };
    format!("gg: {what}: GUARD {:.1} / {:.0} (level {}){}{}", g.value, g.max, g.level, if g.broken { " BROKEN" } else { "" }, if g.regenerating() { " regen" } else { "" })
}

/// THE LEADER'S AIM as Sora's shotlock aim shows it (kh-core `AimHud`, UI spec 7.3: the scope, a mark per lock, the
/// lock counter): Goofy's Discus Barrage, Donald's Spell Volley. Mode 1 while aiming, 2 while the volley / the bouncing
/// shield runs (the marks stay until it is over), else None (the HUD closes the scope).
pub fn leader_aim(party: &kh_party::Party, id: kh_party::MemberId) -> Option<kh_core::ui_model::AimHud> {
    use kh_core::ui_model::{AimHud, AimMark};
    let m = party.member(id)?;
    let pad = m.pad.as_ref()?;
    let (aiming, fired, locks, focus, rate, max) = match m.chara {
        kh_party::Chara::Goofy => {
            let g = &pad.goofy;
            let k = kh_party::pad::goofy_kit::K;
            let aiming = party.goofy_locks(id).is_some();
            let fired = !aiming && g.barrage && (m.state == kh_party::NpcStateId::Action || !g.queue.is_empty());
            (aiming, fired, g.locks.clone(), g.focus, k.barrage_focus_rate, k.barrage_max_locks)
        }
        // (DON-UI) no shotlock for Donald (the user, 2026-10-22; DONALD_OVERHAUL.md 13): no aim
        kh_party::Chara::Donald => return None,
    };
    if !aiming && !fired {
        return None;
    }
    let mut marks: Vec<AimMark> = Vec::new();
    for t in &locks {
        match marks.iter_mut().find(|mk| mk.target == *t) {
            Some(mk) => mk.count += 1,
            None => marks.push(AimMark { target: *t, point: 0, count: 1, position: glam::Vec3::ZERO }),
        }
    }
    Some(AimHud { mode: if aiming { 1 } else { 2 }, aim_time: focus * rate, total: locks.len() as i32, max: max as i32, locks: marks, airstep: Vec::new() })
}

impl PartyRuntime {
    /// (GOOFY-GG) The leader's GUARD bar for `Hud::set_guard` (None unless Goofy is the controlled leader).
    pub fn leader_guard_hud(&self) -> Option<(kh_host::party::GuardValues, kh_host::party::GuardLook)> {
        leader_guard_hud(&self.party, self.leader?)
    }

    /// (BN) Takes the kit abilities the level-up of `c` to `level` taught (their names; the window shows each after the
    /// ability sparkle, `kh_ui::hud_side::icon_line`).
    pub fn take_level_abilities(&mut self, c: PartyChara, level: i32) -> Vec<String> {
        match self.level_abilities.iter().position(|a| a.0 == c && a.1 == level) {
            Some(i) => self.level_abilities.remove(i).2,
            None => Vec::new(),
        }
    }

    /// (GOOFY-GG) Takes whether the level-up of `c` to `level` raised GUARD's max (its window gets the GUARD line).
    pub fn take_guard_up(&mut self, c: PartyChara, level: i32) -> bool {
        let n = self.guard_ups.len();
        self.guard_ups.retain(|g| *g != (c, level));
        self.guard_ups.len() != n
    }

    /// (party X2-b) A kit ability worn / taken off from the camp menu (the AP check: his max AP).
    pub fn apply_kit_edit(&mut self, c: PartyChara, key: &str, on: bool) -> bool {
        let Some(s) = self.id(c).and_then(|id| self.party.status_mut(id)) else { return false };
        let ok = if let (Some(k), true) = (kh_party::donald::DonAbility::from_name(key), s.donald.is_some()) {
            // (DON-UI) Donald's own abilities (DON_*): the AP check as KH3's (his AP in use, DON-PROG's `ap`)
            let fits = !on || s.donald.as_ref().is_some_and(|d| d.abilities.equipped(k)) || s.ap_in_use() + k.ap() <= s.max_ap();
            fits && s.donald.as_mut().is_some_and(|d| d.abilities.set_equipped(k, on))
        } else if let Some(k) = kh_party::pad::goofy_kit::GoofyAbility::from_name(key) {
            s.kit_goofy_equip(k, on)
        } else if let Some(k) = kh_party::pad::donald_kit::KitAbility::from_name(key) {
            if on && !s.kit.equipped(k) && s.ap_in_use() + k.ap() > s.max_ap() {
                false
            } else {
                s.kit.set_equipped(k, on)
            }
        } else {
            false
        };
        crate::log::line(&format!("party: {} kit {key} {}: {}", c.name(), if on { "on" } else { "off" }, if ok { "set" } else { "REFUSED" }));
        ok
    }
}

#[cfg(test)]
mod x2b_tests {
    use super::*;
    use crate::{PartyTest, RuntimeConfig};

    #[test]
    fn kit_rows_toggle_with_the_ap_check_and_the_leader_bar() {
        let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        rt.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        rt.spawn_one(PartyChara::Goofy, kh_core::Vec3::new(200.0, 0.0, 0.0), 0.0);
        let d = rt.id(PartyChara::Donald).unwrap();
        let g = rt.id(PartyChara::Goofy).unwrap();
        // the kits as the test loadout gives them (everything owned and worn)
        rt.party.status_mut(d).unwrap().donald.as_mut().unwrap().abilities = kh_party::donald::DonAbilities::test_all(); // (DON-UI) his own abilities
        rt.party.status_mut(g).unwrap().goofy_kit = kh_party::pad::goofy_kit::GoofyKit::test_all();
        // (DON-UI) Donald's page: his DonAbility rows (label, DON-PROG's description and AP), the old X3D kit gone
        let donald = kit_rows(rt.party.status(d).unwrap());
        assert_eq!(donald.len(), kh_party::donald::DonAbility::ALL.len());
        assert!(donald.iter().any(|r| r.key == "DON_DUAL_ATTUNEMENT" && r.name == "Dual Attunement" && r.category == 2));
        assert!(donald.iter().all(|r| r.equipped && !r.help.is_empty() && r.key.starts_with("DON_")));
        let goofy = kit_rows(rt.party.status(g).unwrap());
        assert_eq!(goofy.len(), 15); // (GOOFY-GK) + Combo Plus x2, Air Combo Plus, Shield Quake, Shield Upper
        assert!(goofy.iter().filter(|r| r.kh3.is_none()).all(|r| !r.help.is_empty()), "every Goofy kit ability has its help text (the Fusions use KH3's)");
        assert!(goofy.iter().any(|r| r.name == "Over Here!" && r.category == 0));
        // off, then on again with the AP check (the test loadout is above his AP: on is refused until room is made)
        assert!(rt.apply_kit_edit(PartyChara::Goofy, "KIT_STALWART", false));
        assert!(!rt.party.status(g).unwrap().goofy_kit.equipped(kh_party::pad::goofy_kit::GoofyAbility::Stalwart));
        let room = rt.party.status(g).unwrap().max_ap() - rt.party.status(g).unwrap().ap_in_use();
        let back = rt.apply_kit_edit(PartyChara::Goofy, "KIT_STALWART", true);
        assert_eq!(back, room >= kh_party::pad::goofy_kit::GoofyAbility::Stalwart.ap());
        assert!(rt.apply_kit_edit(PartyChara::Donald, "DON_SPELL_WEAVER", false));
        assert!(!rt.party.status(d).unwrap().donald.as_ref().unwrap().abilities.equipped(kh_party::donald::DonAbility::SpellWeaver));
        assert!(!rt.apply_kit_edit(PartyChara::Donald, "KIT_NOTHING", true));
        // the bar: only for a leader; Goofy's shield gauge out of the aim, no aim marks while not aiming
        assert_eq!(leader_focus(&rt.party, g), None);
        rt.party.set_pad(g, Some(kh_core::Tuning::USER));
        // (GOOFY-GG) Goofy leading: no focus-slot bar, the GUARD bar instead (his meter, full)
        assert_eq!(leader_focus(&rt.party, g), None);
        let (gv, _) = leader_guard_hud(&rt.party, g).expect("Goofy's GUARD bar");
        assert_eq!((gv.value, gv.max, gv.broken), (rt.party.status(g).unwrap().guard.unwrap().max, rt.party.status(g).unwrap().guard.unwrap().max, false));
        assert!(leader_guard_hud(&rt.party, d).is_none(), "Donald has no GUARD");
        assert!(leader_aim(&rt.party, g).is_none());
        rt.party.set_pad(d, Some(kh_core::Tuning::USER));
        assert_eq!(leader_focus(&rt.party, d), None, "(DON-UI) no Spell Volley: his slot is the Resonance ring's");
        assert!(leader_aim(&rt.party, d).is_none(), "(DON-UI) no shotlock for Donald");
    }
}

/// (BN) THE KIT ABILITIES A CHARACTER LEARNS BY REACHING THE LEVELS `from` + 1 ..= `to` (his kit's own level table:
/// Donald's `KitAbility::OWN`, Goofy's `GoofyAbility::OWN`, sheet rows `ability.*.level`): their names, in the order of
/// their levels. What the pad's kit sync (`kit_sync_level` / `kit_goofy_sync_level`) then gives him; KH3 names a level's
/// ability on the LEVEL UP! window (`LevelUpDelta` kind 7).
pub fn level_kit_abilities(c: PartyChara, from: u32, to: u32) -> Vec<String> {
    let reached = |lv: Option<u32>| lv.is_some_and(|lv| lv > from && lv <= to);
    let mut out: Vec<(u32, String)> = Vec::new();
    match c {
        PartyChara::Donald => {
            for k in kh_party::pad::donald_kit::KitAbility::OWN {
                if reached(k.level()) {
                    out.push((k.level().unwrap_or(0), k.label().to_string()));
                }
            }
        }
        PartyChara::Goofy => {
            for k in kh_party::pad::goofy_kit::GoofyAbility::OWN {
                if reached(k.level()) {
                    out.push((k.level().unwrap_or(0), k.label().to_string()));
                }
            }
        }
        PartyChara::Sora => {}
    }
    out.sort_by_key(|o| o.0);
    out.into_iter().map(|o| o.1).collect()
}

// ------------------------------------------------------------------------- (DON-UI) Donald's menu, block and HUD feed

/// The names on the ally target list (KH3's party names).
pub fn ally_name(c: PartyChara) -> &'static str {
    match c {
        PartyChara::Sora => "Sora",
        PartyChara::Donald => "Donald",
        PartyChara::Goofy => "Goofy",
    }
}

impl PartyRuntime {
    /// (DON-UI) Elemental Insight's view of Donald's lock-on target from the host's targets (KH3's attribute resists,
    /// `Targets::defense`; a host whose targets have none leaves it unknown: nothing is marked). Called by `route`.
    pub fn don_affinity_from(&mut self, targets: &dyn kh_core::Targets) {
        let lock = self.id(PartyChara::Donald).and_then(|id| self.party.pad_state(id)).map(|p| p.lock).filter(|t| *t != kh_core::TargetId::NONE);
        self.customs.don_affinity = lock.and_then(|t| targets.element_affinity(t)).map(|r| kh_party::donald::hud::affinity_of_resist(&r)); // (DON-PROG) element_affinity: the host's elements (Elden Ring: NpcParam), not the damage resists
    }

    /// (DON-UI) Donald's learned magic as his Customize block reads it: his own (spawned: his synced copy and his Dual
    /// Attunement), else the roster's.
    pub fn don_book(&self) -> kh_core::command::leader::donald::DonBook {
        match self.id(PartyChara::Donald).and_then(|id| self.party.status(id)).and_then(|s| s.donald.as_ref()) {
            Some(st) => kh_party::pad::menu::book_of(st),
            None => {
                let l = self.don_learned();
                kh_core::command::leader::donald::DonBook { tier: l.tier, line2: l.line2, dual: false }
            }
        }
    }

    /// (DON-UI) THE FEED OF DONALD'S MENU (kh-party pad/menu.rs `DonFeed`), every step before the pad frame: his block,
    /// the ally target list (the active party: Donald, then the friends in formation order, Sora while he is in the
    /// party), his lock-on target's statuses, the host's elements of it (`Customs::don_affinity`), Sora's team attacks
    /// on offer (they stack on Y).
    pub fn feed_don_menu(&mut self, sora: &kh_core::Sora) {
        // the menu's switches (sheet party-don-hud), installed once
        static TUNE: std::sync::Once = std::sync::Once::new();
        TUNE.call_once(|| kh_party::pad::menu::set_don_menu_tune(kh_party::donald::hud::menu_tune_of(&don_hud_params())));
        let Some(id) = self.id(PartyChara::Donald) else { return };
        if self.party.pad_state(id).is_none() {
            return;
        }
        let mut members: Vec<kh_party::pad::menu::AllyRow> = Vec::new();
        let order: Vec<PartyChara> = std::iter::once(PartyChara::Donald).chain(self.lead.current_formation().active().iter().copied().filter(|c| *c != PartyChara::Donald)).collect();
        for c in order {
            match c {
                PartyChara::Sora => {
                    if self.lead.sora_active {
                        let s = sora.status();
                        members.push(kh_party::pad::menu::AllyRow { id: kh_party::donald::cast::SORA_MEMBER, name: ally_name(c), hp: s.hp, hp_max: s.max_hp() });
                    }
                }
                _ => {
                    if let Some((mid, s)) = self.id(c).and_then(|m| self.party.status(m).map(|s| (m, s))) {
                        members.push(kh_party::pad::menu::AllyRow { id: mid, name: ally_name(c), hp: s.hp, hp_max: s.max_hp() });
                    }
                }
            }
        }
        let lock = self.party.pad_state(id).map(|p| p.lock).filter(|t| *t != kh_core::TargetId::NONE);
        let lock = lock.map(|t| (t.0, self.party.don.status.get(t.0).copied().unwrap_or_default()));
        // (YHUD) only Sora's team attacks WITH DONALD (6.6: "Donald's with Sora and with Goofy"; Goofy's Bombardier with Sora
        // was listed here too, and Sora's own rows showed under an empty list: kh_party::yhud)
        let team: Vec<kh_core::command::SituationRow> = self.leader_team_rows(sora);
        let feed = kh_party::pad::menu::DonFeed { on: true, custom: self.customs.blocks[LeaderKind::Donald as usize], members, lock, affinity: self.customs.don_affinity, team, airborne: self.party.member(id).is_some_and(|m| !m.on_ground()), has_target: self.party.pad_state(id).is_some_and(|p| p.lock != kh_core::TargetId::NONE || p.soft != kh_core::TargetId::NONE) };
        if let Some(pad) = self.party.member_mut(id).and_then(|m| m.pad.as_mut()) {
            pad.menu.don.feed = feed;
        }
    }

    /// (DON-UI) Donald's HUD this frame (kh-party `donald::hud::DonHud`): his rings, stars, tracker and his lock-on target's
    /// statuses, the Scan gate; with his menu's side channel (the ally target list, Quick Cast) and each member's worn
    /// Aero Barrier / Ball Lightning (the icon by the portrait). None unless Donald is the controlled leader.
    pub fn don_hud(&self) -> Option<DonHudFeed> {
        let id = self.leader?;
        let m = self.party.member(id)?;
        let st = m.status.donald.as_ref()?;
        let pad = m.pad.as_ref()?;
        let lock = (pad.lock != kh_core::TargetId::NONE).then_some(pad.lock.0);
        let hud = kh_party::donald::hud::DonHud::with(st, &self.party.don.status, lock, self.customs.don_affinity, &kh_party::donald::data());
        let menu = pad.menu.don_view(m.chara, &m.status);
        let mut worn = Vec::new();
        for c in [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy] {
            let member = if c == PartyChara::Sora { Some(kh_party::donald::cast::SORA_MEMBER) } else { self.id(c) };
            if let Some(mid) = member {
                let barrier = kh_party::donald::allyfx::barrier_of(&self.party, mid).is_some();
                let orbs = kh_party::donald::allyfx::orbs_of(&self.party, mid).filter(|o| o.left > 0.0).map_or(0, |o| o.orbs); // active: balls and time left
                if barrier || orbs > 0 {
                    worn.push((c, barrier, orbs));
                }
            }
        }
        Some(DonHudFeed { hud, menu, worn })
    }
}

/// (DON-UI) What the hosts hand the HUD for Donald (`kh_host::hud::Hud::set_don`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonHudFeed {
    pub hud: kh_party::donald::hud::DonHud,
    pub menu: Option<kh_party::pad::menu::DonMenuView>,
    /// Members wearing Aero Barrier / Ball Lightning: (who, barrier, orbs left).
    pub worn: Vec<(PartyChara, bool, u8)>,
}

/// (DON-UI) An element's colour (sheet `color.*`, 0xRRGGBB) as straight RGBA.
pub fn rgba(c: u32) -> [f32; 4] {
    [((c >> 16) & 0xFF) as f32 / 255.0, ((c >> 8) & 0xFF) as f32 / 255.0, (c & 0xFF) as f32 / 255.0, 1.0]
}

/// (DON-UI) kh-party's HUD snapshot as the HUD's view (kh-host `DonHudView`; the worn spells go by character, the HUD
/// seats them).
pub fn don_view_of(feed: &DonHudFeed, team: &[(u32, i64)]) -> kh_host::party::DonHudView {
    use kh_host::party::{ArcaneView, ResonanceView, RingLook, TargetIconsView, UltLook};
    use kh_party::donald::hud::StatusIcon;
    use kh_party::donald::UltMark;
    use kh_party::pad::menu::element_icon;
    let h = &feed.hud;
    let ring = ResonanceView {
        rings: h
            .rings
            .iter()
            .map(|r| RingLook {
                color: rgba(r.color),
                pips: r.pips,
                pips_max: r.pips_max,
                full: r.full,
                ult: match r.ult {
                    UltMark::Hidden => UltLook::Hidden,
                    UltMark::Waiting => UltLook::Waiting,
                    UltMark::Ready => UltLook::Ready,
                    UltMark::Spent => UltLook::Spent,
                },
                icon: element_icon(r.element).to_string(),
            })
            .collect(),
        attuning: h.attuning.map(|(e, c, share)| (element_icon(e).to_string(), rgba(c), share)),
        worn: Vec::new(),
        leader_hp_frames: 0, // (the HUD fills it: its own HP track frame)
        stars: None,         // (the HUD fills it from `arcane`)
    };
    let arcane = ArcaneView {
        stars: h.stars,
        stars_max: h.stars_max,
        star_left: h.star_left,
        zetta: h.zetta.map(|z| z.need.iter().map(|e| (element_icon(e).to_string(), z.got.has(e))).collect()).unwrap_or_default(),
    };
    let target = h.target.as_ref().map(|t| TargetIconsView {
        icons: t
            .icons
            .iter()
            .map(|i| match i {
                StatusIcon::Wet => ("Water".to_string(), 0),
                StatusIcon::Chill(n) => ("Blizzard".to_string(), *n),
                StatusIcon::Frozen => ("MixItem".to_string(), 0),
                StatusIcon::Burn => ("Fire".to_string(), 0),
                StatusIcon::Reveal => ("Ability".to_string(), 0),
                StatusIcon::Exposed => ("StatusDown".to_string(), 0),
                StatusIcon::Blind => ("Dark".to_string(), 0),
            })
            .collect(),
        insight: t.insight.map(|(w, r)| w.iter().map(|e| (element_icon(e).to_string(), true)).chain(r.iter().map(|e| (element_icon(e).to_string(), false))).collect()).unwrap_or_default(),
    });
    let scan = h.target.as_ref().is_some_and(|t| t.scan);
    let menu = feed.menu.as_ref();
    let targets = menu.and_then(|m| m.target.as_ref()).map(|(rows, cursor)| (rows.iter().map(|r| format!("{}  {} / {}", r.1, r.2.max(0), r.3.max(0))).collect(), *cursor));
    let quick_glow = menu.and_then(|m| m.quick).and_then(|q| q.1);
    kh_host::party::DonHudView { ring, arcane, target, scan, targets, quick_glow, y_costs: menu.map(|m| m.y_costs.clone()).unwrap_or_default(), team_serials: team.to_vec(), list: menu.map_or(0, |m| m.list), worn: feed.worn.clone(), look: Some(don_look_of(&don_hud_params())) }
}

impl PartyRuntime {
    /// (YHUD) THE LEADING FRIEND'S TEAM ROWS: the team attacks of Sora's queue that include him (kh_party::yhud). Empty
    /// while Sora leads (his HUD is his own queue), with no leader or while Sora is out of the party.
    pub fn leader_team_rows(&self, sora: &kh_core::Sora) -> Vec<kh_core::command::SituationRow> {
        let Some(chara) = self.leader.and_then(|id| self.party.member(id)).map(|m| m.chara) else { return Vec::new() };
        if !self.lead.sora_active {
            return Vec::new();
        }
        kh_party::yhud::leader_team_rows(&sora.combat().situation, chara)
    }

    /// (YHUD) THE LEADER'S Y TO SORA'S QUEUE: the serial of the team row the leading friend's Y runs now (his menu's
    /// selected Y row, when it is one of [`Self::leader_team_rows`]). None: the press is his own (or nothing) and never
    /// reaches Sora's queue (lead.rs `route`).
    pub fn leader_y_forward(&self, sora: &kh_core::Sora) -> Option<u32> {
        let id = self.leader?;
        let snap = self.party.pad_menu(id)?;
        kh_party::yhud::y_forward(&snap, &self.leader_team_rows(sora))
    }
}

impl PartyRuntime {
    /// (DON-UI) DONALD'S HUD for `Hud::set_don` (None unless he is the controlled leader): his rings and pips, stars,
    /// tracker, lock-on statuses and Scan, the ally target list and Quick Cast's mark, the worn spells by the portraits
    /// (slot 0 = the leader's, then the friends' slots in formation order), Sora's team-attack rows (their names: the
    /// HUD's pack).
    pub fn don_hud_view(&self, sora: &kh_core::Sora) -> Option<kh_host::party::DonHudView> {
        let feed = self.don_hud()?;
        let team: Vec<(u32, i64)> = sora.combat().situation.list.iter().filter(|e| e.class == kh_core::combat::situation::class::TEAM).map(|e| (e.serial, e.kind as i64)).collect();
        Some(don_view_of(&feed, &team))
    }
}

/// (DON-UI) THE SHEET `party-don-hud` AT RUN TIME (read once, then kept): the compiled rows with the repo's sheet file
/// laid on top when it can be read (a changed number needs no rebuild, only a restart).
pub fn don_hud_params() -> kh_party::donald::hud::DonHudParams {
    static P: std::sync::OnceLock<kh_party::donald::hud::DonHudParams> = std::sync::OnceLock::new();
    *P.get_or_init(|| {
        let mut p = kh_party::donald::hud::DON_HUD_PARAMS;
        let path = kh_gendata::project_file("sheets/port/party-don-hud.json").to_string();
        if let Ok(text) = std::fs::read_to_string(&path) {
            for (k, v) in crate::donald::sheet_pairs(&text) {
                kh_party::donald::hud::set_hud_param(&mut p, &k, v);
            }
        }
        p
    })
}

/// (DON-UI) The HUD's look of Donald's pieces from the sheet.
pub fn don_look_of(p: &kh_party::donald::hud::DonHudParams) -> kh_host::party::DonLook {
    use kh_host::party::{ArcaneLayout, DonLook, ResonanceLayout};
    DonLook {
        ring: ResonanceLayout {
            badge_right: p.badge_right,
            badge_y: p.badge_y,
            row_step: p.badge_row_step,
            plate_h: p.badge_plate_h,
            plate: [p.badge_plate, p.badge_plate, p.badge_plate * 1.12, p.badge_plate_alpha],
            plate_pad: p.badge_pad,
            rim: p.badge_rim,
            rim_hz: p.badge_rim_hz,
            pip_size: (p.pip_w, p.pip_h),
            pip_step: p.pip_step,
            pip_off: [p.pip_off, p.pip_off, p.pip_off * 1.12, p.pip_off_alpha],
            icon_size: p.icon_size,
            icon_gap: p.icon_gap,
            ult_size: p.ult_size,
            ult_gap: p.ult_gap,
            ult_dim: p.ult_dim,
            ult_dark: p.ult_dark,
            worn_gap: ResonanceLayout::default().worn_gap,
            hp_bar_x0: ResonanceLayout::default().hp_bar_x0,
            worn_pc_x: p.worn_pc_x,
            worn_pc_y: p.worn_pc_y,
            worn_size_pc: p.worn_size_pc,
            worn_npc: (p.worn_npc_x, p.worn_npc_y),
            worn_size_npc: p.worn_size_npc,
            ult_hot: p.ult_hot,
            ult_glow: p.ult_glow,
            ult_glow_min: p.ult_glow_min,
            ult_pulse_hz: p.ult_pulse_hz,
            ult_ready_scale: p.ult_ready_scale,
            ult_core: p.ult_core,
            ult_glow_h: p.ult_glow_h,
            ult_glow_gain: p.ult_glow_gain,
            ult_tint: p.ult_tint,
            ult_flare_w: p.ult_flare_w,
            ult_flare_h: p.ult_flare_h,
            ult_burst_s: p.ult_burst_s,
            ult_burst_scale: p.ult_burst_scale,
            stars_label: "Triangle".to_string(), // KH3's Y button glyph (the pad set the HUD's Y prompts use)
            worn_disc: ResonanceLayout::default().worn_disc,
            blink_hz: p.blink_hz,
        },
        arcane: ArcaneLayout {
            star_at: ArcaneLayout::default().star_at, // (the stars are in the status HUD's cluster now: no place by the menu)
            star_size: p.star_size,
            star_step: p.star_step,
            star_off: p.star_off,
            star_warn: p.star_warn,
            star_blink_hz: p.star_blink_hz,
            zetta_gap: p.zetta_gap,
            zetta_size: p.zetta_size,
            zetta_step: p.zetta_step,
            zetta_off: p.zetta_off,
            status_at: (p.status_x, p.status_y),
            status_size: p.status_size,
            status_step: p.status_step,
            insight_dy: p.status_insight_dy,
            insight_size: p.status_insight_size,
            y_star_size: p.y_star_size,
            y_star_step: p.y_star_step,
            y_star_at: (p.y_star_x, p.y_star_y),
            y_star_off: p.y_star_off,
        },
    }
}

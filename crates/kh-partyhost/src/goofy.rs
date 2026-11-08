//! (GOOFY) GOOFY IN THE HOST: the test level's fake attacks, so every parry can be tried without Elden Ring. A word announces an
//! attack by a dummy (`dummy-telegraph N CLASS ETA [ACTIVE] [POWER]`): it is on the telegraph list `ETA` seconds ahead, its hit
//! window then opens for `ACTIVE` seconds, and the hit lands as it closes, on Goofy, unless a sweep finds him in the air. Hop and
//! Slip are judged by kh-party from those windows, as in Elden Ring. `dummy-hit N CLASS [POWER]` is the same with no warning.

use crate::runtime::PartyRuntime;
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{AttackClass, TargetId};
use kh_core::Vec3;
use kh_host::party::PartyChara;

/// One fake attack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FakeAttack {
    pub attacker: TargetId,
    pub class: AttackClass,
    pub eta: f32,
    pub active: f32,
    pub power: f32,
}

/// A dummy's attack as `PartyRuntime::hit_friend` delivers it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixtureHit {
    pub kind: Kind,
    pub power: f32,
    pub class: AttackClass,
}

/// A class by its word.
pub fn class_of_word(w: &str) -> Option<AttackClass> {
    Some(match w.to_ascii_lowercase().as_str() {
        "normal" => AttackClass::Normal,
        "thrust" => AttackClass::Thrust,
        "sweep" => AttackClass::Sweep,
        "grab" => AttackClass::Grab,
        "heavy" => AttackClass::Heavy,
        "projectile" | "arrow" => AttackClass::Projectile,
        "magic" => AttackClass::Magic,
        _ => return None,
    })
}

fn perilous(c: AttackClass) -> bool {
    matches!(c, AttackClass::Thrust | AttackClass::Sweep | AttackClass::Grab | AttackClass::Heavy)
}

impl PartyRuntime {
    /// (GF-THREAT) THE TEST LEVEL'S ENEMY CHOICE WITH GOOFY'S TAUNT AND THE THREAT BOOK (`victim` of a fixture dummy; Elden Ring's is
    /// the director, kh-er party/aggro.rs): a taunt that reaches the dummy takes it; with threat on it, a roll over the living
    /// members by their shares, Donald's own weight in it, stable for `aggro.er_period` s as his route is. `None`: no taunt and no
    /// threat on it, or an ally's guard for Taunt Pass to judge: the next rule stands. Whom it attacks next; never a hit moved.
    pub fn gof_victim(&self, dummy: u32, from: Vec3) -> Option<crate::runtime::Victim> {
        use crate::runtime::{party_chara, Victim};
        use kh_party::donald::{ability_info::prog, aggro, cast::SORA_MEMBER};
        use kh_party::goofy::threat;
        if let Some(c) = threat::taunter_of(&self.party, from).and_then(|id| self.current.iter().find(|v| v.id == id && !v.ko)) {
            return Some(Victim::Friend(party_chara(c.chara)));
        }
        let book = &self.party.gof.threat;
        let target = TargetId(dummy);
        if book.top(target).is_none() || self.party.don_prog.taunt_now().is_some() {
            return None;
        }
        let mut cands: Vec<(Victim, f32)> = Vec::new();
        if self.lead.sora_active {
            cands.push((Victim::Sora, 1.0 + threat::weight(book.share(target, SORA_MEMBER))));
        }
        for v in self.current.iter().filter(|v| !v.ko) {
            let don = self.party.status(v.id).and_then(|s| s.donald.as_ref()).map_or(1.0, |d| aggro::pick_weight(&d.abilities, &self.party.don_prog.aggro_of(v.id), from.distance(v.position)));
            cands.push((Victim::Friend(party_chara(v.chara)), (1.0 + threat::weight(book.share(target, v.id))) * don));
        }
        let weights: Vec<f32> = cands.iter().map(|c| c.1).collect();
        let bucket = (self.party.don_prog.clock / prog().value("aggro.er_period").max(0.1)).floor() as u64;
        threat::pick(&weights, aggro::roll(dummy as u64, bucket, 3)).map(|k| cands[k].0)
    }

    /// (GOOFY) The script words `dummy-telegraph N CLASS ETA [ACTIVE] [POWER]` and `dummy-hit N CLASS [POWER]`.
    pub fn gof_fake_word(&mut self, words: &[&str]) -> String {
        let num = |i: usize, d: f32| words.get(i).and_then(|w| w.parse::<f32>().ok()).unwrap_or(d);
        let (Some(n), Some(class)) = (words.get(1).and_then(|w| w.parse::<u32>().ok()), words.get(2).and_then(|w| class_of_word(w))) else {
            return format!("goofy: {}: dummy-telegraph N CLASS ETA [ACTIVE] [POWER] | dummy-hit N CLASS [POWER]", words.join(" "));
        };
        let attack = if words[0] == "dummy-hit" {
            FakeAttack { attacker: TargetId(n), class, eta: 0.0, active: 0.0, power: num(3, 1.0) }
        } else {
            FakeAttack { attacker: TargetId(n), class, eta: num(3, 0.5), active: num(4, 0.3), power: num(5, 1.0) }
        };
        self.gof_fake.push(attack);
        format!("goofy: {attack:?}")
    }

    /// (GOOFY) One step of the fake attacks, before the party's: the list for `Party::set_telegraphs`, the hits that land.
    pub fn gof_fake_step(&mut self, dt: f32, now: f64, position: &dyn Fn(TargetId) -> Option<Vec3>) {
        if self.gof_fake.is_empty() {
            self.party.set_telegraphs(&[]);
            return;
        }
        let mut list = Vec::new();
        let mut land = Vec::new();
        for a in self.gof_fake.iter_mut() {
            if a.eta > 0.0 {
                a.eta -= dt;
            } else if a.active > 0.0 {
                a.active -= dt;
            }
            if a.eta <= 0.0 && a.active <= 0.0 {
                land.push(*a);
            } else {
                list.push(kh_party::Telegraph { attacker: a.attacker, class: a.class, perilous: perilous(a.class), eta: a.eta.max(0.0), active: a.eta <= 0.0 });
            }
        }
        self.gof_fake.retain(|a| a.eta > 0.0 || a.active > 0.0);
        self.party.set_telegraphs(&list);
        for a in land {
            let Some(from) = position(a.attacker) else { continue };
            let Some(id) = self.id(PartyChara::Goofy) else { continue };
            // a sweep passes under him in the air
            if a.class == AttackClass::Sweep && self.party.member(id).is_some_and(|m| !m.on_ground()) {
                continue;
            }
            self.hit_friend(PartyChara::Goofy, a.attacker, from, FixtureHit { kind: Kind::Small, power: a.power, class: a.class }, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FriendRecord, PartyTest, RuntimeConfig};
    use kh_party::goofy::GoofyAbility;

    fn rt() -> PartyRuntime {
        let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        rt.party = kh_party::Party::new(kh_party::PartyConfig { level99: false, total_exp: 0, seed: 1 });
        rt.spawn_one(PartyChara::Goofy, Vec3::ZERO, 0.0);
        rt
    }

    #[test]
    fn an_old_saves_kit_names_and_kh3_seeds_drop_and_free_their_ap() {
        let mut rt = rt();
        let id = rt.id(PartyChara::Goofy).unwrap();
        let s = rt.party.status_mut(id).unwrap();
        s.level = 40;
        s.gof_sync_level();
        let free = s.ap_in_use();
        let mut record = FriendRecord::of(PartyChara::Goofy, s);
        for name in ["KIT_IRON_WALL", "KIT_COMBO_PLUS", "KIT_TORNADO_COPTER", "GOOFY_TURBO", "GOOFY_TORNADE", "DEFENDER"] {
            record.abilities.push((name.to_string(), 1, 1));
        }
        record.apply(s);
        assert_eq!(s.ap_in_use(), free, "the old names take no AP");
        let saved = FriendRecord::of(PartyChara::Goofy, s);
        assert!(saved.abilities.iter().all(|a| !a.0.starts_with("KIT_") && !a.0.starts_with("GOOFY_") && a.0 != "DEFENDER"), "{:?}", saved.abilities);
    }

    #[test]
    fn goofys_abilities_save_and_load_under_their_gof_names() {
        let mut rt = rt();
        let id = rt.id(PartyChara::Goofy).unwrap();
        let s = rt.party.status_mut(id).unwrap();
        s.goofy.as_mut().unwrap().abilities.give(GoofyAbility::LoudMouth);
        s.goofy.as_mut().unwrap().abilities.set_equipped(GoofyAbility::LoudMouth, true);
        let record = FriendRecord::of(PartyChara::Goofy, s);
        assert!(record.abilities.iter().any(|a| a.0 == "GOF_LOUD_MOUTH" && a.2 == 1));
        s.goofy.as_mut().unwrap().abilities = Default::default();
        record.apply(s);
        assert!(s.gof_has(GoofyAbility::LoudMouth));
    }

    #[test]
    fn a_telegraph_word_walks_the_list_then_its_hit_lands_classed_with_its_attacker() {
        let mut rt = rt();
        let id = rt.id(PartyChara::Goofy).unwrap();
        rt.gof_fake_word(&["dummy-telegraph", "2", "thrust", "0.1", "0.1"]);
        let at = |_: TargetId| Some(Vec3::new(200.0, 0.0, 0.0));
        rt.gof_fake_step(0.05, 0.0, &at);
        let t = rt.party.telegraphs()[0];
        assert_eq!((t.attacker, t.class, t.perilous, t.active), (TargetId(2), AttackClass::Thrust, true, false));
        rt.gof_fake_step(0.06, 0.0, &at);
        assert!(rt.party.telegraphs()[0].active, "the window is open");
        let hp = rt.party.status(id).unwrap().hp;
        rt.gof_fake_step(0.11, 0.0, &at);
        assert!(rt.party.telegraphs().is_empty());
        assert!(rt.party.status(id).unwrap().hp < hp, "the hit lands as the window closes");
        rt.gof_fake_step(0.1, 0.0, &at);
    }

    #[test]
    fn a_fake_sweep_passes_under_him_in_the_air() {
        let mut rt = rt();
        let id = rt.id(PartyChara::Goofy).unwrap();
        rt.party.member_mut(id).unwrap().cmc.location.z += 300.0;
        rt.party.member_mut(id).unwrap().cmc.set_falling();
        let hp = rt.party.status(id).unwrap().hp;
        rt.gof_fake_word(&["dummy-hit", "1", "sweep"]);
        rt.gof_fake_step(0.02, 0.0, &|_| Some(Vec3::ZERO));
        assert_eq!(rt.party.status(id).unwrap().hp, hp);
    }

    #[test]
    fn a_bad_word_says_how_it_is_used() {
        let mut rt = rt();
        assert!(rt.gof_fake_word(&["dummy-telegraph", "x"]).contains("dummy-telegraph N CLASS"));
        assert!(rt.gof_fake.is_empty());
    }
}

//! (GOOFY-BAL) THE FRIEND LEADERS' BALANCE (the user, 2026-10-16 23:50, build/sora_tmp/party/ER_PHASE_NOTES.md: "Goofy
//! and Donald need to be balanced like Sora, both of them should be a bit weaker in different aspects, Donald has stronger
//! magic and more MP but weaker Melee, Goofy has no magic and weaker melee than Sora (but stronger than Donald) but higher
//! defense and health"). KH3 has no playable Donald / Goofy: these are THE PORT'S rules for a friend WHILE HE LEADS (an
//! AI friend keeps KH3's own status, unchanged). Design, targets and the measured tables: build/sora_tmp/party/
//! BAL_DESIGN.md; the proof: crates/kh-party/tests/bal_balance.rs.
//!
//! THE STATS OF A LEADER (every number a sheet row):
//! - Strength / Magic / Defense = his row of `sheets/port/party-balance-levels.json` at his level (Sora's own level table
//!   times the per-character ratios of `party-balance.json`, built by tools/party_balance_sheet.js), then his weapon and
//!   armour as before. So his growth is Sora's growth: Goofy's Strength 0.84x, Donald's 0.62x, Donald's Magic 1.15x,
//!   Goofy's Magic 0, Goofy's Defense 1.15x.
//! - Max HP / MP = `hp_ratio` / `mp_ratio` x SORA'S max HP / MP now (the host's feed, [`crate::Party::set_lead_reference`];
//!   without a feed: Sora's base of the difficulty), + his own HP / MP victory bonuses. A
//!   ratio of 0 = his KH3 value (base + bonuses) as before.
//! - In Elden Ring a hit takes a share of the Tarnished's max HP; for a friend leader that share is taken of SORA'S max
//!   HP times the defence factor (Sora's table Defense / his, at his level, ^ `lead.defense_exponent`) instead of his own
//!   max ([`incoming_reference`], kh-er party/leader.rs): his bigger bar and his Defense really make him last longer,
//!   while Sora's own rule is untouched.

use crate::ids::Chara;
use crate::status::FriendStatus;
use kh_core::status::GameLevel;

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_balance.rs"));
include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_balance_levels.rs"));

/// The sheet's numbers.
pub const BP: PartyBalanceParams = PARTY_BALANCE_PARAMS;

/// What a friend's status knows about leading (`FriendStatus::bal`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LeadBalance {
    /// He is the pad-driven leader now (`Party::set_pad`).
    pub leading: bool,
    /// Sora's max HP / MP now (the host's feed; None = no feed: [`sora_reference`]'s fallback).
    pub sora: Option<(i32, i32)>,
}

/// The table row of a character at a level (1..=99, clamped).
pub fn level_row(chara: Chara, level: u32) -> &'static BalanceLevelRow {
    let lv = level.clamp(1, 99);
    let model = chara.model();
    PARTY_BALANCE_LEVELS.iter().find(|r| r.chara == model && r.level == lv).expect("party-balance-levels row")
}

fn ratios(chara: Chara) -> (f32, f32) {
    match chara {
        Chara::Donald => (BP.n_ex001_lead_hp_ratio, BP.n_ex001_lead_mp_ratio),
        Chara::Goofy => (BP.n_ex002_lead_hp_ratio, BP.n_ex002_lead_mp_ratio),
    }
}

/// Sora's max HP / MP the leader's are measured against: the host's feed, or (no feed) Sora's base of the difficulty
/// (Critical halves it, as `GameLevel::row`).
pub fn sora_reference(s: &FriendStatus) -> (i32, i32) {
    if let Some(r) = s.bal.sora {
        return r;
    }
    sora_base(s.game_level)
}

/// The leader's (Strength, Magic, Defense) before weapon and armour.
pub fn base_stats(chara: Chara, level: u32) -> (i32, i32, i32) {
    let r = level_row(chara, level);
    (r.strength, r.magic, r.defense)
}

/// Max HP while leading, before the roster's steps (status.rs adds those: stream IT) (None = the KH3 rule: ratio 0).
pub fn max_hp(s: &FriendStatus) -> Option<i32> {
    let (ratio, _) = ratios(s.chara);
    (ratio > 0.0).then(|| ((ratio * sora_reference(s).0 as f32).round() as i32 + s.bonuses.totals(s.game_level).0).max(1))
}

/// Max MP while leading (None = the KH3 rule).
pub fn max_mp(s: &FriendStatus) -> Option<i32> {
    let (_, ratio) = ratios(s.chara);
    (ratio > 0.0).then(|| ((ratio * sora_reference(s).1 as f32).round() as i32 + s.bonuses.totals(s.game_level).1).max(1))
}

/// The defence factor of an Elden Ring hit on him: (Sora's table Defense / his, at his level) ^ exponent, clamped.
pub fn defense_cut(s: &FriendStatus) -> f32 {
    if BP.lead_defense_exponent == 0.0 {
        return 1.0;
    }
    let sora = kh_core::status::tables::level_row(s.level.clamp(1, 99)).defense.max(1) as f32;
    let own = level_row(s.chara, s.level).defense.max(1) as f32;
    let lo = BP.lead_defense_cut_min.clamp(0.01, 1.0);
    (sora / own).powf(BP.lead_defense_exponent).clamp(lo, 1.0 / lo)
}

/// THE HP AN ELDEN RING HIT'S SHARE IS TAKEN OF for this friend while he leads (kh-er sora.rs `damage_in` takes
/// `share x this`): (Sora's max HP `sora_max_hp`, the host's own, + the friend's own HP bonuses and roster steps, the
/// part of his bar that is not the ratio's) x [`defense_cut`]. So his toughness is exactly hp_ratio / defence factor
/// against Sora's whatever bonuses he has (bonuses lengthen the bar as Sora's do, without changing that ratio). His own
/// max HP when he does not lead, the rule is off (`lead.incoming`), or his HP ratio is 0 (the S2 rule as before).
pub fn incoming_reference(s: &FriendStatus, sora_max_hp: i32) -> f32 {
    let (ratio, _) = ratios(s.chara);
    if !s.bal.leading || !BP.lead_incoming || ratio <= 0.0 || sora_max_hp <= 0 {
        return s.max_hp() as f32;
    }
    let own = s.bonuses.totals(s.game_level).0 + s.roster.hp_mp(s.game_level).0;
    (sora_max_hp + own) as f32 * defense_cut(s)
}

/// How many hits Elden Ring's share `share` (0..1 of the Tarnished's max) takes to knock him out, against Sora's
/// `1 / share` (reports and tests).
pub fn hits_to_ko(s: &FriendStatus, sora_max_hp: i32, share: f32) -> f32 {
    let per_hit = share * incoming_reference(s, sora_max_hp);
    s.max_hp() as f32 / per_hit.max(1e-6)
}

/// The difficulty's Sora base (for reports).
pub fn sora_base(game_level: GameLevel) -> (i32, i32) {
    let r = game_level.row();
    (r.base_hp, r.base_mp)
}

impl crate::party::Party {
    /// (GOOFY-BAL) THE HOST'S FEED: Sora's max HP / MP now (kh-partyhost custom.rs `feed_leader_magic`, every route), the
    /// reference of a friend leader's HP / MP ([`max_hp`] / [`max_mp`]). Kept on every member (only a leader reads it).
    pub fn set_lead_reference(&mut self, sora: Option<(i32, i32)>) {
        let ids: Vec<_> = self.members().collect();
        for id in ids {
            if let Some(s) = self.status_mut(id) {
                s.set_sora_reference(sora);
            }
        }
    }

    /// (GOOFY-BAL) The HP an Elden Ring hit's share is taken of for member `id` ([`incoming_reference`]).
    pub fn incoming_reference(&self, id: crate::MemberId, sora_max_hp: i32) -> Option<f32> {
        self.status(id).map(|s| incoming_reference(s, sora_max_hp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lead(chara: Chara, level: u32) -> FriendStatus {
        let mut s = FriendStatus::new(chara, crate::tables::level_row(chara, level).0);
        s.set_leading(true);
        s
    }

    #[test]
    fn the_table_is_soras_growth_times_the_ratios() {
        for lv in [1, 10, 50, 99] {
            // ratio x Sora's stat with the Kingdom Key (4 / 3 / 0), stored minus the starting weapon (staff 3 / 4, shield 4 / 0)
            let sora = kh_core::status::tables::level_row(lv);
            let (gs, gm, gd) = base_stats(Chara::Goofy, lv);
            let (ds, dm, dd) = base_stats(Chara::Donald, lv);
            assert_eq!(gs + 4, (BP.n_ex002_lead_strength_ratio * (sora.attack + 4) as f32).round() as i32);
            assert_eq!(ds + 3, (BP.n_ex001_lead_strength_ratio * (sora.attack + 4) as f32).round() as i32);
            assert_eq!(dm + 4, (BP.n_ex001_lead_magic_ratio * (sora.magic + 3) as f32).round() as i32);
            assert_eq!(gm, 0, "Goofy has no magic");
            // (DON-KIT) doc 2: Donald's Defense 0.80x Sora's (a glass cannon)
            assert!(gd >= sora.defense && (dd - (BP.n_ex001_lead_defense_ratio * sora.defense as f32).round() as i32).abs() <= 1, "lv {lv}: Donald's defence {dd} of Sora's {}", sora.defense);
        }
    }

    #[test]
    fn the_order_the_user_asked_for_at_every_level() {
        for lv in 1..=99 {
            let sora = kh_core::status::tables::level_row(lv);
            let (s, m) = (sora.attack + 4, sora.magic + 3); // + the Kingdom Key
            let (g, d) = (lead(Chara::Goofy, lv), lead(Chara::Donald, lv));
            assert!(s > g.strength() && g.strength() > d.strength(), "lv {lv}: melee Sora {s} > Goofy {} > Donald {}", g.strength(), d.strength());
            assert!(d.magic() > m, "lv {lv}: Donald's magic {} > Sora's {m}", d.magic());
            assert_eq!(g.magic(), 0, "lv {lv}: Goofy has no magic");
            assert!(g.defense() > sora.defense, "lv {lv}: Goofy's defence");
        }
        let (g, d) = (lead(Chara::Goofy, 30), lead(Chara::Donald, 30));
        let (hp, mp) = sora_reference(&g);
        assert!(g.max_hp() > hp && d.max_mp() > mp, "Goofy HP {} vs Sora {hp}, Donald MP {} vs Sora {mp}", g.max_hp(), d.max_mp());
    }

    #[test]
    fn ai_friends_keep_kh3s_status() {
        for chara in [Chara::Donald, Chara::Goofy] {
            let s = FriendStatus::new(chara, crate::tables::level_row(chara, 40).0);
            let row = crate::tables::level_row(chara, 40);
            assert_eq!(s.strength(), row.1 + crate::tables::weapon(s.weapon).unwrap().3);
            assert_eq!(s.max_hp(), crate::tables::cparam(chara, "base.max_hp") as i32);
            assert_eq!(incoming_reference(&s, 150), s.max_hp() as f32, "an AI friend: the S2 rule");
        }
    }

    #[test]
    fn leading_keeps_the_hp_share_and_full_stays_full() {
        let mut s = FriendStatus::new(Chara::Goofy, 0);
        assert_eq!(s.hp, s.max_hp());
        s.set_leading(true);
        assert_eq!(s.hp, s.max_hp(), "full stays full");
        s.hp = s.max_hp() / 2;
        s.set_leading(false);
        assert!((s.hp as f32 / s.max_hp() as f32 - 0.5).abs() < 0.02, "{} / {}", s.hp, s.max_hp());
    }

    #[test]
    fn goofy_lasts_longer_than_sora_and_donald_as_long() {
        let sora_max = 150;
        for lv in [1, 20, 60, 99] {
            let mut g = lead(Chara::Goofy, lv);
            let mut d = lead(Chara::Donald, lv);
            g.bal.sora = Some((sora_max, 100));
            d.bal.sora = Some((sora_max, 100));
            let (gh, dh) = (hits_to_ko(&g, sora_max, 0.1), hits_to_ko(&d, sora_max, 0.1));
            assert!(gh > 12.5 && gh < 15.5, "lv {lv}: Goofy {gh:.2} hits of 10 % (Sora 10)");
            // (DON-KIT) doc 2: Donald is a glass cannon (HP 0.85x, Defense 0.80x): fewer hits than Sora's 10
            assert!(dh < 9.6 && dh > 5.0, "lv {lv}: Donald {dh:.2}");
            // his own HP bonuses (KH3's HP_UP10 x6) lengthen the bar without changing the ratio to Sora
            for _ in 0..6 {
                g.bonuses.add(kh_core::status::BonusCounts::HP_UP10);
                d.bonuses.add(kh_core::status::BonusCounts::HP_UP10);
            }
            let (gb, db) = (hits_to_ko(&g, sora_max, 0.1), hits_to_ko(&d, sora_max, 0.1));
            assert!(g.max_hp() >= (1.2 * sora_max as f32) as i32 + 60 && gb > 12.0 && gb < 15.5, "lv {lv}: Goofy with bonuses {gb:.2}");
            assert!(db < 9.6 && db > 5.0, "lv {lv}: Donald with bonuses {db:.2}"); // (DON-KIT)
        }
    }
}

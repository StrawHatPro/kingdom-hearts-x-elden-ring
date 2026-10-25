//! (ER party S6) THE PARTY'S PROGRESSION, shared by every host (the test level and the Elden Ring mod): the user's
//! rules of 2026-10-15 (build/sora_tmp/party/ER_PHASE_NOTES.md, "Progression"), the P2 rules of the party's EXP and
//! prizes (moved here from kh-app player.rs / prizes.rs / savepoint.rs so both hosts run one copy), the real
//! progression mode of a host whose friends keep what they earn (Elden Ring), the friends' victory bonuses, and the
//! save's whole party record (format 2 + the v5 friends record).
//!
//! THE USER'S RULES (verbatim: "whoever is the party leader should have the (zero exp) ability, and it remains active
//! on the new party leader if you switch. Unlocking a new friend auto levels them to be where they should at your
//! current level. Who you start playing as always starts at level 1"):
//! - ZERO EXP IS THE LEADER'S (OUR DESIGN: KH3 gives it to Sora only). Whoever leads holds the ability; its on / off
//!   state carries over to the new leader at a Party Formation switch. It gates the party's EXP award through "player
//!   0" (P2's rule: `adjust_exp_for` with the leader). A friend who stops leading loses the ability; Sora keeps owning
//!   his copy (it is his KH3 start ability: `sora-newgame` abilities.owned, rebuilt from the base every level-up) but
//!   it is switched off while he does not lead. [`PartyRuntime::sync_zero_exp`], called at every `route`.
//! - A FRIEND WHO JOINS takes the level KH3's rule gives him for the party's progress: his own level table at the
//!   SHARED EXP total (stats spec 5.2; KH3's friends have no level abilities, their abilities come from the init data,
//!   weapons and victory bonuses), HP / MP full. [`PartyRuntime::join_level`] (`spawn_one` runs it in the real
//!   progression mode). The friend victory bonuses of bosses beaten before he joined are the host's catch-up
//!   (kh-er bossreward.rs).
//! - THE STARTER IS LEVEL 1: a new game's shared total is 0, so whoever starts (Sora, Donald or Goofy) is at level 1 by
//!   his own table; [`starting_formation`] is the new game's formation (the starter alone, leading).
//!
//! THE REAL PROGRESSION MODE ([`PartyRuntime::set_progression`], Elden Ring): the friends start from KH3's init data
//! (`TresChrInitData`, `PartyConfig::level99` false) instead of the test level's level-99 loadout, the sheet
//! `party-test` loadout is not applied, and a friend slotted out comes back with what he had (S0's note: his last
//! record is re-applied at `spawn_one`). The test level never switches it on.

use kh_core::enums::ETresAbilityKind as A;
use kh_core::status::{BonusCounts, ExpPlayer, PrizeKind};
use kh_core::{Sora, Vec3};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;
use kh_party::MemberId;
use serde_json::{json, Value};

use crate::runtime::{chara_of, PartyRuntime};
use crate::save::{FriendRecord, PartySave};
use crate::say;

/// The leader's Zero EXP: who holds it now, and whether it is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ZeroExp {
    /// The member who holds the ability now (`None`: not handed out yet: the first `route` decides).
    pub holder: Option<PartyChara>,
    /// The state that carries over (the holder's equipped bit as the last sync read it).
    pub on: bool,
    /// A loaded save's state, taken at the next sync.
    pub loaded: Option<bool>,
}

/// What [`PartyRuntime::give_exp`] did with an award.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpGiven {
    /// Sora leads: his own `give_exp` (his Zero EXP, his popups).
    Sora,
    /// A friend leads with Sora in the party: `StatusRequest::GiveExpAs` with the leader as player 0 (Sora's popups).
    AsLeader,
    /// A friend leads, Sora is out of the party (not stepped): straight into the shared total, no popups. The EXP
    /// really added.
    Direct(i32),
}

/// A victory bonus or an ability given to a friend (the friends' boss-reward rows, kh-er bossreward.rs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FriendReward {
    /// `ETresVictoryBonusKind` by its `BonusCounts` index (HP_UP10, ITEM_SLOT_UP1 ...).
    Bonus(usize),
    Ability(A),
    /// (party X3D) one of the playable kits' PORT abilities by its save name (`kit:AERO_FLOAT`: Donald's Aero Float,
    /// pad/donald_kit.rs); no AP for Aero Float, the others equipped while his AP allows.
    Kit(String),
    /// (DON-PROG) one of Donald's overhaul abilities (`don:AERO_FLOAT`: the Red Wolf of Radagon's Aero Float; an old
    /// `kit:AERO_FLOAT` / `kit:SPELL_WEAVER` token is read as its overhaul ability): owned, equipped while his AP allows.
    Don(kh_party::donald::DonAbility),
}

impl FriendReward {
    /// `bonus:HP_UP10`, `bonus:ITEM_SLOT_UP1`, `ability:MP_HASTE` (the sheet's tokens).
    pub fn parse(text: &str) -> Option<FriendReward> {
        let (kind, name) = text.trim().split_once(':')?;
        match kind {
            "bonus" => bonus_index(name).map(FriendReward::Bonus),
            "ability" => A::from_game_name(name).or_else(|| A::ALL.iter().copied().find(|k| k.name() == name)).filter(|k| *k != A::None).map(FriendReward::Ability),
            // (party X3D)
            "don" => kh_party::donald::DonAbility::from_name(name).map(FriendReward::Don), // (DON-PROG)
            "kit" if kh_party::pad::donald_kit::KitAbility::from_name(name).and_then(|k| k.to_don()).is_some() => kh_party::pad::donald_kit::KitAbility::from_name(name).and_then(|k| k.to_don()).map(FriendReward::Don), // (DON-PROG) the X3D kit migrated
            "kit" => kh_party::pad::donald_kit::KitAbility::from_name(name).map(|k| FriendReward::Kit(k.name().to_string())).or_else(|| kh_party::pad::goofy_kit::GoofyAbility::from_name(name).map(|k| FriendReward::Kit(k.name().to_string()))), // (party X3G) Goofy's too
            _ => None,
        }
    }

    pub fn label(&self) -> String {
        match self {
            FriendReward::Bonus(i) => BONUS_NAMES.iter().find(|b| b.1 == *i).map_or_else(|| format!("bonus {i}"), |b| b.0.to_string()),
            FriendReward::Ability(k) => k.name().to_string(),
            FriendReward::Don(k) => k.label().to_string(), // (DON-PROG)
            FriendReward::Kit(n) => kh_party::pad::donald_kit::KitAbility::from_name(n).map(|k| k.label().to_string()).or_else(|| kh_party::pad::goofy_kit::GoofyAbility::from_name(n).map(|k| k.label().to_string())).unwrap_or_else(|| n.clone()), // (party X3D) (party X3G)
        }
    }
}

/// The friends' `ETresVictoryBonusKind` names the reward rows use (TresVBonusData: their rows give only these).
const BONUS_NAMES: [(&str, usize); 11] = [
    ("HP_UP3", BonusCounts::HP_UP3),
    ("HP_UP5", BonusCounts::HP_UP5),
    ("HP_UP10", BonusCounts::HP_UP10),
    ("HP_UP15", BonusCounts::HP_UP15),
    ("HP_UP30", BonusCounts::HP_UP30),
    ("MP_UP3", BonusCounts::MP_UP3),
    ("MP_UP5", BonusCounts::MP_UP5),
    ("MP_UP10", BonusCounts::MP_UP10),
    ("DEF_SLOT_UP1", BonusCounts::DEF_SLOT_UP1),
    ("ACC_SLOT_UP1", BonusCounts::ACC_SLOT_UP1),
    ("ITEM_SLOT_UP1", BonusCounts::ITEM_SLOT_UP1),
];

fn bonus_index(name: &str) -> Option<usize> {
    let name = name.trim().trim_start_matches("ETresVictoryBonusKind::");
    BONUS_NAMES.iter().find(|b| b.0.eq_ignore_ascii_case(name)).map(|b| b.1)
}

/// THE NEW GAME'S FORMATION: the chosen starter alone, leading (the others join when they are unlocked: kh-er's
/// unlocks, stream S8). The shared EXP total of a new game is 0: the starter is level 1 by his own table.
pub fn starting_formation(starter: PartyChara) -> Formation {
    Formation::new(&[starter], starter).unwrap_or_default()
}

impl PartyRuntime {
    /// (ER) THE REAL PROGRESSION MODE (module text). Call before the first step; the test level never does.
    pub fn set_progression(&mut self, on: bool) {
        if self.progression != on {
            self.progression = on;
            say!("party: real progression {} (the friends {})", if on { "ON" } else { "off" }, if on { "start from KH3's init data and keep what they earn" } else { "take the party-test loadout at every spawn" });
        }
    }

    /// The level the shared total gives a friend now, applied (HP / MP full): the user's "unlocking a new friend auto
    /// levels them to be where they should at your current level". The level never goes down here (the game's EXP
    /// never does). Returns (from, to).
    pub fn join_level(&mut self, c: PartyChara, total_exp: i32) -> Option<(u32, u32)> {
        let chara = chara_of(c)?;
        let id = self.id(c)?;
        let s = self.party.status_mut(id)?;
        let from = s.level;
        let to = kh_party::tables::level_for_exp(chara, total_exp.max(0)).max(1);
        s.level = to;
        s.total_exp = total_exp.max(0);
        s.hp = s.max_hp();
        s.mp = s.max_mp();
        s.mp_charge_left = 0.0;
        s.refill_guard(); // (GOOFY-GG) GUARD full at his new level
        Some((from, to))
    }

    /// THE LEADER'S ZERO EXP (module text), once per `route`: hands the ability to a new leader with the state the old
    /// one had; reads the holder's live state otherwise (the player may switch it in the leader's Abilities page).
    pub fn sync_zero_exp(&mut self, sora: &mut Sora) {
        let leader = self.lead.applied;
        let held = |rt: &PartyRuntime, sora: &Sora, c: PartyChara| -> Option<bool> {
            match c {
                PartyChara::Sora => {
                    let a = &sora.status().abilities;
                    (a.owned(A::Expzero) > 0).then(|| a.equipped(A::Expzero) > 0)
                }
                friend => {
                    let s = rt.id(friend).and_then(|id| rt.party.status(id))?;
                    (s.abilities.owned(A::Expzero) > 0).then(|| s.abilities.equipped(A::Expzero) > 0)
                }
            }
        };
        match (self.zero_exp.loaded.take(), self.zero_exp.holder) {
            // a loaded save's state is the state from now on (handed to whoever leads)
            (Some(on), _) => self.zero_exp.on = on,
            // the leader holds it: his live state (the player may switch it in his Abilities page)
            (None, Some(h)) if h == leader => {
                if let Some(on) = held(self, sora, leader) {
                    self.zero_exp.on = on;
                    return;
                }
            }
            // a new leader: the old holder's state as it is now
            (None, Some(h)) => {
                if let Some(on) = held(self, sora, h) {
                    self.zero_exp.on = on;
                }
            }
            // the first sync: the state of whoever has it (Sora's copy of KH3's start set, or a loaded friend's)
            (None, None) => {
                let found = [leader, PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy].into_iter().find_map(|c| held(self, sora, c));
                match found {
                    Some(on) => self.zero_exp.on = on,
                    // nobody has it (a host whose Sora lacks it): nothing to hand on
                    None => return,
                }
            }
        }
        let on = self.zero_exp.on;
        // everyone else gives it up: a friend loses the ability, Sora keeps his copy switched off
        for c in [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy] {
            if c == leader {
                continue;
            }
            match c {
                PartyChara::Sora => {
                    let a = &mut sora.status_mut().abilities;
                    if a.equipped(A::Expzero) > 0 {
                        a.set(A::Expzero, a.owned(A::Expzero), 0);
                    }
                }
                friend => {
                    if let Some(s) = self.id(friend).and_then(|id| self.party.status_mut(id)) {
                        if s.abilities.owned(A::Expzero) > 0 {
                            s.abilities.set(A::Expzero, 0, 0);
                        }
                    }
                }
            }
        }
        // the leader holds it, in the carried state
        let given = match leader {
            PartyChara::Sora => {
                let a = &mut sora.status_mut().abilities;
                a.set(A::Expzero, a.owned(A::Expzero).max(1), u8::from(on));
                true
            }
            friend => match self.id(friend).and_then(|id| self.party.status_mut(id)) {
                Some(s) => {
                    s.abilities.set(A::Expzero, 1, u8::from(on));
                    true
                }
                None => false,
            },
        };
        if given {
            if self.zero_exp.holder != Some(leader) {
                say!("party: Zero EXP is the leader's: {} holds it now ({}; the state carries over)", leader.name(), if on { "ON: the party gains no EXP" } else { "off" });
            }
            self.zero_exp.holder = Some(leader);
        }
    }

    /// The party's Zero EXP state (for the save).
    pub fn zero_exp_on(&self) -> bool {
        self.zero_exp.on
    }

    /// A loaded save's Zero EXP state (taken at the next `route`).
    pub fn set_zero_exp_loaded(&mut self, on: bool) {
        self.zero_exp.loaded = Some(on);
    }

    /// "PLAYER 0" OF THE EXP AWARD while a friend leads (P2: his HP, his Zero EXP and EXP Incentive counts, his level);
    /// `None` while Sora leads.
    pub fn exp_player(&self) -> Option<ExpPlayer> {
        let s = self.leader.and_then(|l| self.party.status(l))?;
        Some(ExpPlayer { alive: s.hp > 0, exp_zero: s.count(A::Expzero), exp_bargain: s.count(A::ExpBargain), level: s.level })
    }

    /// THE PARTY'S EXP AWARD (P2, stats spec 4.2; was kh-app player.rs `give-exp`): ONE total (Sora's status: the bank),
    /// gated by "player 0" = the controlled leader; every member then checks the same total with his own table at the
    /// next `step` (`set_total_exp`). Sora out of the party is not stepped (his requests would wait): the award goes
    /// into the total at once, without his popups.
    pub fn give_exp(&mut self, sora: &mut Sora, base_exp: i32, rate: f32, position: Option<Vec3>) -> ExpGiven {
        match self.exp_player() {
            None => {
                sora.give_exp(base_exp, rate, position);
                ExpGiven::Sora
            }
            Some(player) if !self.lead.sora_in => {
                let status = sora.status_mut();
                let reward = kh_core::status::Status::enemy_reward(base_exp, rate);
                let gain = status.adjust_exp_for(reward, player);
                ExpGiven::Direct(status.give_exp_gain(reward, gain).added)
            }
            Some(player) => {
                sora.status_request(kh_core::StatusRequest::GiveExpAs { base_exp, rate, position, player, panel: true });
                ExpGiven::AsLeader
            }
        }
    }

    /// (P2, was kh-app prizes.rs) The drop's modifiers with the friends' abilities: PRIZE_FEEVER on any member, LUCK_UP
    /// summed over the party, a converter when any member has it (prize spec 2; 0x1414241F0).
    pub fn prize_mods(&self, mut mods: kh_prize::SpawnMods) -> kh_prize::SpawnMods {
        for (_, id) in &self.members {
            let Some(s) = self.party.status(*id) else { continue };
            mods.prize_fever |= s.count(A::PrizeFeever) > 0;
            mods.luck_up += s.count(A::LuckUp);
            mods.converters.hp |= s.count(A::HpConverter) > 0;
            mods.converters.mp |= s.count(A::MpConverter) > 0;
            mods.converters.focus |= s.count(A::FpConverter) > 0;
            mods.converters.millionaire |= s.count(A::Millionaire) > 0;
            mods.converters.munny |= s.count(A::MunnyConverter) > 0;
        }
        mods
    }

    /// (P2, was kh-app prizes.rs `prize_members`) THE GETTER SLOTS: Sora (when he is in the party) and every spawned
    /// friend, the leader marked as the player pawn. A friend's point is his capsule centre, his radius by HIS OWN
    /// Treasure Magnets, alive = not KO'd; he refuses prizes while KO'd (Die) or in a team attack's FR state
    /// (0x141259A50). Returns the slots and the friends' member ids by slot.
    pub fn prize_members(&self, sora: &Sora, sora_point: Vec3) -> (Vec<kh_host::prize::PrizeMember>, Vec<Option<MemberId>>) {
        let mut members = Vec::new();
        let mut ids = Vec::new();
        if self.lead.sora_active {
            let s = sora.status();
            members.push(kh_host::prize::PrizeMember { name: "Sora", getter: kh_prize::Getter::sora(sora, sora_point), sora: true, player: self.leader.is_none(), friend: false, share_prize: s.count(A::SharePrize) > 0, refuses: false, hp: s.hp, hp_max: s.max_hp(), mp: s.mp, mp_max: s.max_mp() });
            ids.push(None);
        }
        for (c, id) in &self.members {
            let (Some(v), Some(s)) = (self.current.iter().find(|v| v.id == *id), self.party.status(*id)) else { continue };
            let radius = if s.count(A::MasterDraw) > 0 { None } else { Some([300.0, 600.0, 900.0][(s.count(A::PrizeDraw) as usize).min(2)]) };
            let point = v.position + Vec3::Z * v.capsule.1.max(1.0);
            members.push(kh_host::prize::PrizeMember {
                name: c.name(),
                getter: kh_prize::Getter { point, can_take: !v.ko && v.hp > 0, radius },
                sora: false,
                player: self.leader == Some(*id),
                friend: true,
                share_prize: s.count(A::SharePrize) > 0,
                refuses: v.ko || matches!(v.state, kh_party::NpcStateId::Fr(_)),
                hp: v.hp,
                hp_max: v.hp_max,
                mp: if v.mp_charging { 0 } else { v.mp },
                mp_max: v.mp_max,
            });
            ids.push(Some(*id));
        }
        (members, ids)
    }

    /// (P2, was kh-app prizes.rs `FriendPrizes`) `ApplyPrize` 0x1405AB950 on a friend's pawn: HP +1 / +5 (`Recover(..,
    /// kind 2)`); MP +3 / +5, or while his MP charges the charge time -2 / -6 s instead.
    pub fn apply_prize(&mut self, c: PartyChara, kind: PrizeKind) {
        let Some(id) = self.id(c) else { return };
        match kind {
            PrizeKind::HpSmall | PrizeKind::HpLarge => {
                self.party.heal(id, if kind == PrizeKind::HpSmall { 1 } else { 5 }, 2);
            }
            PrizeKind::MpSmall | PrizeKind::MpLarge => {
                let (seconds, points) = if kind == PrizeKind::MpSmall { (2.0, 3) } else { (6.0, 5) };
                if let Some(s) = self.party.status_mut(id) {
                    if s.mp_charge_left > 0.0 {
                        s.mp_charge_left -= seconds;
                        if s.mp_charge_left <= 0.0 {
                            s.mp_charge_left = 0.0;
                            s.mp = s.max_mp();
                        }
                    } else {
                        s.mp = (s.mp + points).min(s.max_mp());
                    }
                }
            }
            _ => {}
        }
    }

    /// (P2, was kh-app savepoint.rs `party_needs`) The save point's "needs recovery" of the pawn in its capsules (player
    /// 0x141436260): Sora leading = his own test or a friend's; a friend leading = not KO'd, and he, Sora (in the
    /// party) or another friend below max HP / MP.
    pub fn savepoint_needs(&self, sora: &Sora) -> bool {
        let friend_low = |v: &kh_party::MemberView| v.hp < v.hp_max || v.mp < v.mp_max || v.mp_charging;
        let friends = self.members.iter().filter_map(|(_, id)| self.party.view(*id));
        match self.leader.and_then(|l| self.party.view(l)) {
            None => sora.status().hp > 0 && (kh_host::savepoint::SavePoint::needs_recovery(sora) || friends.into_iter().any(|v| friend_low(&v))),
            Some(leader) => {
                let s = sora.status();
                let sora_low = self.lead.sora_active && (s.hp < s.max_hp() || s.mp < s.max_mp());
                !leader.ko && (friend_low(&leader) || sora_low || friends.into_iter().any(|v| v.id != leader.id && friend_low(&v)))
            }
        }
    }

    /// (P2, was kh-app savepoint.rs `recover_party`) `Recover(n, n, n, kind 1)` on the leader and every member of the
    /// active party (tick 0x140F93590): Sora through his core (when in the party); a friend: HP by kh-party's heal (a
    /// KO'd friend's HP above 0 stands him up), MP +n or, while his MP charges, the charge time -n s. Returns the
    /// friends a KO ended for.
    pub fn savepoint_recover(&mut self, sora: &mut Sora, n: i32) -> Vec<PartyChara> {
        if n <= 0 {
            return Vec::new();
        }
        if self.lead.sora_active {
            sora.status_request(kh_core::StatusRequest::Recover { hp: n, mp: n, focus: n, kind: 1 });
        }
        let mut stood = Vec::new();
        for (c, id) in self.members.clone() {
            let was_ko = self.party.view(id).is_some_and(|v| v.ko && v.hp < 1);
            self.party.heal(id, n, 1);
            if let Some(s) = self.party.status_mut(id) {
                if s.mp_charge_left > 0.0 {
                    s.mp_charge_left -= n as f32;
                    if s.mp_charge_left <= 0.0 {
                        s.mp_charge_left = 0.0;
                        s.mp = s.max_mp();
                    }
                } else {
                    s.mp = (s.mp + n).min(s.max_mp());
                }
                if kh_party::guard::GP.guard_save_point_refills {
                    s.refill_guard(); // (GOOFY-GG) the save point / grace refills GUARD and clears a break
                }
                if was_ko && s.hp > 0 {
                    stood.push(c);
                }
            }
        }
        stood
    }

    /// A FRIEND'S VICTORY BONUS OR ABILITY (the friends' boss-reward rows): a bonus counts in `BonusCounts` (the slot
    /// bonuses also open the slot, at most 4, as `test_level99` does); an ability is one more copy (at most 3),
    /// equipped when his AP allows. `None` = he is not in the party (the host keeps the reward for his join).
    pub fn friend_reward(&mut self, c: PartyChara, reward: &FriendReward) -> Option<String> {
        let id = self.id(c)?;
        let s = self.party.status_mut(id)?;
        let text = match reward {
            FriendReward::Bonus(kind) => {
                s.bonuses.add(*kind);
                match *kind {
                    BonusCounts::ITEM_SLOT_UP1 => s.item_slots = (s.item_slots + 1).min(4),
                    BonusCounts::DEF_SLOT_UP1 => s.armour_slots = (s.armour_slots + 1).min(4),
                    BonusCounts::ACC_SLOT_UP1 => s.accessory_slots = (s.accessory_slots + 1).min(4),
                    _ => {}
                }
                let (hp, mp) = (s.max_hp(), s.max_mp());
                s.hp = hp;
                s.mp = mp;
                format!("{} (max HP {hp}, MP {mp}, slots item {} armour {} accessory {})", reward.label(), s.item_slots, s.armour_slots, s.accessory_slots)
            }
            FriendReward::Ability(kind) => {
                let owned = s.abilities.owned(*kind).saturating_add(1).min(3);
                let equipped = s.abilities.equipped(*kind);
                s.abilities.set(*kind, owned, equipped);
                let cost = kh_party::tables::ability_cost(kind.game_name().trim_start_matches("ETresAbilityKind::")).map_or(0, |c| c.0);
                let on = s.ap_in_use() + cost <= s.max_ap();
                if on {
                    s.abilities.set(*kind, owned, equipped + 1);
                }
                format!("{} ({} copy, {})", reward.label(), owned, if on { "equipped" } else { "not equipped: no AP left" })
            }
            // (DON-PROG) one of Donald's overhaul abilities
            FriendReward::Don(k) => match s.don_grant(*k) {
                Some((new, on)) => format!("{} ({}, {})", k.label(), if new { "new" } else { "already owned" }, if on { "equipped" } else { "not equipped: no AP left" }),
                None => format!("{} (not this friend's)", k.label()),
            },
            // (party X3D) a kit ability (Donald's: the Red Wolf's Aero Float)
            FriendReward::Kit(name) => match kh_party::pad::donald_kit::KitAbility::from_name(name) {
                Some(k) if s.chara == kh_party::Chara::Donald => {
                    let (new, on) = s.kit_grant(k);
                    format!("{} ({}, {})", k.label(), if new { "new" } else { "already owned" }, if on { "equipped" } else { "not equipped: no AP left" })
                }
                // (party X3G) one of Goofy's (a reward row may name kit:TORNADO_COPTER ...): owned, worn while the AP allows
                _ => match kh_party::pad::goofy_kit::GoofyAbility::from_name(name) {
                    Some(k) if s.chara == kh_party::Chara::Goofy => {
                        let new = s.goofy_kit.give(k);
                        let on = s.goofy_kit.equipped(k) || s.kit_goofy_equip(k, true);
                        format!("{} ({}, {})", k.label(), if new { "new" } else { "already owned" }, if on { "equipped" } else { "not equipped: no AP left" })
                    }
                    _ => format!("{name} (not this friend's)"),
                },
            },
        };
        say!("party: {} gets {text}", c.name());
        Some(text)
    }

    /// THE SAVE'S PARTY RECORD (save.kh `party`): format 2 (the formation + every character's AI settings,
    /// `kh_host::formation::party_record`) + `friends_version` 5 and `friends` (the v5 friends record, contract ER.6)
    /// + `zero_exp` (the leader's Zero EXP state). A reader of format 2 ignores the rest.
    pub fn party_record(&self) -> Value {
        let save = self.to_save();
        let mut v = kh_host::formation::party_record(&save.formation, &save.ai);
        v["friends_version"] = json!(crate::save::FRIENDS_RECORD_VERSION);
        v["friends"] = Value::Array(save.friends.iter().map(FriendRecord::to_json).collect());
        v["zero_exp"] = json!(self.zero_exp.on);
        v[crate::custom::RECORD_KEY] = self.custom_record(); // (party X2) the leaders' Customize blocks (additive)
        v[kh_party::boosts::RECORD_KEY] = crate::boosts::to_json(&self.roster_boosts); // (IT) the roster's boost total (additive)
        v[crate::donald::RECORD_KEY] = crate::donald::to_json(&self.don_learned); // (DON1) Donald's learned magic (additive)
        v
    }

    /// (ER) A LOAD REPLACES THE FRIENDS' PROGRESSION: in the real progression mode every spawned friend starts again
    /// from KH3's init data at the level `total_exp` gives him (gauges full; his AI settings stay until the record
    /// gives others), the records kept for friends out of the party are dropped, and the Zero EXP state is decided
    /// again (the record's, else whoever has it). Without this a reload after a death would leave what the friends
    /// earned after the save on them. Then [`PartyRuntime::load_party_record`]. Nothing happens outside the mode.
    pub fn reset_friends_for_load(&mut self, total_exp: i32) {
        if !self.progression {
            return;
        }
        for (c, id) in self.members.clone() {
            let Some(chara) = chara_of(c) else { continue };
            if let Some(s) = self.party.status_mut(id) {
                let ai = s.ai;
                *s = kh_party::FriendStatus::new(chara, total_exp.max(0));
                s.ai = ai;
            }
        }
        self.kept_records.clear();
        self.pending_records.clear();
        self.zero_exp = ZeroExp::default();
        self.level_ups.clear();
    }

    /// A LOADED PARTY RECORD (any format: null / format 1 / 2 / with friends): the formation and the AI settings at the
    /// next route, the friends' records onto them (now or at their spawn), the Zero EXP state at the next route.
    pub fn load_party_record(&mut self, v: &Value) {
        let save = read_party_record(v);
        self.from_save(&save);
        if let Some(on) = v["zero_exp"].as_bool() {
            self.set_zero_exp_loaded(on);
        }
        self.load_custom_record(&v[crate::custom::RECORD_KEY]); // (party X2) none in an older save: the defaults
        self.load_roster_record(&v[kh_party::boosts::RECORD_KEY]); // (IT) none in an older save: seeded by the host
        self.load_don_record(&v[crate::donald::RECORD_KEY]); // (DON1) none in an older save: the start, flagged for the catch-up
    }
}

/// The party record of a save as [`PartySave`]: a missing record = the default formation, no AI settings, no friends
/// record (the friends keep KH3's init data); records of unknown characters and odd values are dropped / clamped
/// (`FriendRecord::from_json`).
pub fn read_party_record(v: &Value) -> PartySave {
    PartySave {
        formation: Formation::from_save(v),
        ai: kh_host::formation::ai_of_record(v),
        friends: v["friends"].as_array().map(|a| a.iter().filter_map(FriendRecord::from_json).collect()).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use kh_core::host::TargetId;
    use kh_core::test_world::TestWorld;
    use kh_core::Input;

    struct NoEnemies;
    impl kh_core::Targets for NoEnemies {
        fn enumerate(&self, _out: &mut Vec<kh_core::host::TargetInfo>) {}
        fn info(&self, _id: TargetId) -> Option<kh_core::host::TargetInfo> {
            None
        }
        fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<kh_core::host::HurtShape>) {}
        fn apply_hit(&mut self, _hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
            kh_core::host::HitResult::default()
        }
    }
    impl PartyTargets for NoEnemies {
        fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
        fn attacked_by_enemies(&self) -> bool {
            false
        }
    }

    fn runtime(progression: bool) -> PartyRuntime {
        let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        rt.set_progression(progression);
        rt
    }

    /// A new game's Sora (the true start of sheet sora-newgame: level 1, Zero EXP owned, not equipped).
    fn new_game_sora() -> Sora {
        let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
        *sora.status_mut() = kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None));
        sora
    }

    fn step(rt: &mut PartyRuntime, sora: &mut Sora, world: &TestWorld) {
        let camera = kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let mut targets = NoEnemies;
        let input = Input::default();
        let _ = rt.route(RouteIn { sora, input: &input, dt: 1.0 / 60.0, targets: &targets, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: 0.0 });
        let out = sora.output();
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        rt.step(StepIn { dt: 1.0 / 60.0, sora, out: &out, lock_on: TargetId::NONE, camera: &camera, world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
        let _ = rt.after_step(&NoEnemies, 1.0 / 60.0, 0.0);
    }

    fn zero(rt: &PartyRuntime, c: PartyChara) -> Option<(u8, u8)> {
        let s = rt.party.status(rt.id(c)?)?;
        Some((s.abilities.owned(A::Expzero), s.abilities.equipped(A::Expzero)))
    }

    #[test]
    fn zero_exp_is_the_leaders_and_its_state_carries_over() {
        let world = TestWorld::floor();
        let mut sora = new_game_sora();
        assert_eq!(sora.status().abilities.owned(A::Expzero), 1, "KH3's start set");
        let mut rt = runtime(true);
        step(&mut rt, &mut sora, &world);
        step(&mut rt, &mut sora, &world);
        assert_eq!(rt.zero_exp.holder, Some(PartyChara::Sora));
        assert_eq!((zero(&rt, PartyChara::Donald), zero(&rt, PartyChara::Goofy)), (Some((0, 0)), Some((0, 0))), "the friends do not have it while Sora leads");
        // Sora switches it on, then Donald becomes the leader: Donald has it ON, Sora's copy is off
        sora.status_mut().abilities.set(A::Expzero, 1, 1);
        step(&mut rt, &mut sora, &world);
        assert!(rt.zero_exp_on());
        rt.lead_word(&["leader", "donald"]);
        step(&mut rt, &mut sora, &world);
        assert_eq!(rt.zero_exp.holder, Some(PartyChara::Donald));
        assert_eq!(zero(&rt, PartyChara::Donald), Some((1, 1)));
        assert_eq!(sora.status().abilities.equipped(A::Expzero), 0);
        assert_eq!(sora.status().abilities.owned(A::Expzero), 1, "Sora keeps his own copy");
        // ... and gates the award: player 0 = Donald with Zero EXP
        let before = sora.status().exp;
        assert_eq!(rt.give_exp(&mut sora, 50, 1.0, None), ExpGiven::AsLeader);
        step(&mut rt, &mut sora, &world);
        let _ = sora.step(1.0 / 60.0, &Input::default(), &world);
        assert_eq!(sora.status().exp, before, "Zero EXP on the leader: no EXP");
        // Donald switches it off in his Abilities page; Goofy leads next: off carries over
        rt.apply_edit(&kh_host::party::PartyMenuEdit::Ability { chara: PartyChara::Donald, kind: A::Expzero, copy: 0, on: false });
        step(&mut rt, &mut sora, &world);
        assert!(!rt.zero_exp_on());
        rt.lead_word(&["leader", "goofy"]);
        step(&mut rt, &mut sora, &world);
        assert_eq!((zero(&rt, PartyChara::Donald), zero(&rt, PartyChara::Goofy)), (Some((0, 0)), Some((1, 0))));
        // the award with Goofy (no Zero EXP) as player 0
        assert_eq!(rt.give_exp(&mut sora, 50, 1.0, None), ExpGiven::AsLeader);
        let _ = sora.step(1.0 / 60.0, &Input::default(), &world);
        assert!(sora.status().exp > before, "the leader's Zero EXP is off: the party gains EXP");
        // Sora leads again: his copy takes the state (off)
        rt.lead_word(&["leader", "sora"]);
        step(&mut rt, &mut sora, &world);
        assert_eq!(rt.zero_exp.holder, Some(PartyChara::Sora));
        assert_eq!(zero(&rt, PartyChara::Goofy), Some((0, 0)));
        assert_eq!(sora.status().abilities.equipped(A::Expzero), 0);
    }

    #[test]
    fn a_loaded_zero_exp_state_goes_to_the_saved_leader() {
        let world = TestWorld::floor();
        let mut sora = new_game_sora();
        let mut rt = runtime(true);
        rt.load_party_record(&json!({ "format": 2, "leader": "goofy", "active": ["sora", "donald", "goofy"], "zero_exp": true }));
        for _ in 0..3 {
            step(&mut rt, &mut sora, &world);
        }
        assert_eq!(rt.leader_chara(), PartyChara::Goofy);
        assert_eq!(zero(&rt, PartyChara::Goofy), Some((1, 1)), "Goofy leads with Zero EXP on, as saved");
        assert_eq!(sora.status().abilities.equipped(A::Expzero), 0);
        assert!(rt.party_record()["zero_exp"].as_bool().unwrap());
    }

    #[test]
    fn the_starter_is_level_1_and_a_friend_who_joins_takes_the_partys_level() {
        let world = TestWorld::floor();
        let mut sora = new_game_sora();
        let mut rt = runtime(true);
        rt.set_formation(starting_formation(PartyChara::Donald));
        for _ in 0..3 {
            step(&mut rt, &mut sora, &world);
        }
        assert_eq!(rt.leader_chara(), PartyChara::Donald);
        assert!(!rt.lead.sora_active, "Sora is not in the starter's party");
        assert_eq!(rt.view(PartyChara::Donald).map(|v| v.level), Some(1), "the starter starts at level 1");
        assert!(rt.id(PartyChara::Goofy).is_none());
        // KH3's init data, not the test level's level-99 loadout
        let d = rt.party.status(rt.id(PartyChara::Donald).unwrap()).unwrap();
        assert_eq!((d.weapon, d.bonuses, d.item_slots), ("WEP_DONALD_00", BonusCounts::NONE, 2));
        // the party fights: 3,000 EXP into the shared total (Sora out of the party: straight in, no popups)
        for _ in 0..10 {
            assert!(matches!(rt.give_exp(&mut sora, 300, 1.0, None), ExpGiven::Direct(_)));
        }
        step(&mut rt, &mut sora, &world);
        let total = sora.status().exp;
        assert!(total >= 2_900, "{total}");
        let donald_level = rt.view(PartyChara::Donald).unwrap().level;
        assert_eq!(donald_level, kh_party::tables::level_for_exp(kh_party::Chara::Donald, total));
        assert!(donald_level > 1);
        assert!(!rt.level_ups.is_empty(), "his level-up window");
        // Goofy is unlocked and joins: he is at the level the shared total gives HIM, gauges full
        let mut f = rt.lead.current_formation();
        f.toggle(PartyChara::Goofy).unwrap();
        rt.set_formation(f);
        step(&mut rt, &mut sora, &world);
        let g = rt.view(PartyChara::Goofy).expect("Goofy joined");
        assert_eq!(g.level, kh_party::tables::level_for_exp(kh_party::Chara::Goofy, total));
        assert!(g.level > 1);
        assert_eq!((g.hp, g.mp), (g.hp_max, g.mp_max));
        // Sora joins (the Margit unlock): his level is his own table's at the same total
        let mut f = rt.lead.current_formation();
        f.toggle(PartyChara::Sora).unwrap();
        rt.set_formation(f);
        step(&mut rt, &mut sora, &world);
        assert!(rt.lead.sora_active);
        assert_eq!(sora.status().level, kh_core::status::tables::level_from_exp(total));
    }

    #[test]
    fn a_friend_slotted_out_comes_back_with_what_he_had_in_the_real_progression() {
        let world = TestWorld::floor();
        let mut sora = new_game_sora();
        let mut rt = runtime(true);
        step(&mut rt, &mut sora, &world);
        assert!(rt.friend_reward(PartyChara::Goofy, &FriendReward::parse("bonus:ITEM_SLOT_UP1").unwrap()).is_some());
        assert!(rt.friend_reward(PartyChara::Goofy, &FriendReward::parse("ability:LUCK_UP").unwrap()).is_some());
        let before = FriendRecord::of(PartyChara::Goofy, rt.party.status(rt.id(PartyChara::Goofy).unwrap()).unwrap());
        assert_eq!(before.item_slots, 4);
        rt.lead_word(&["formation", "out", "goofy"]);
        step(&mut rt, &mut sora, &world);
        assert!(rt.id(PartyChara::Goofy).is_none());
        assert!(rt.friend_reward(PartyChara::Goofy, &FriendReward::Ability(A::LuckUp)).is_none(), "not in the party: the host keeps it");
        rt.lead_word(&["formation", "in", "goofy"]);
        step(&mut rt, &mut sora, &world);
        let after = FriendRecord::of(PartyChara::Goofy, rt.party.status(rt.id(PartyChara::Goofy).unwrap()).unwrap());
        assert_eq!(after, before, "his bonus, slots and ability came back with him");
        // the test level (no progression): the loadout again, as before S6
        let mut tl = runtime(false);
        let mut sora2 = new_game_sora();
        step(&mut tl, &mut sora2, &world);
        let g = tl.party.status(tl.id(PartyChara::Goofy).unwrap()).unwrap();
        assert_eq!(g.bonuses.0[BonusCounts::HP_UP10], 6, "the test level keeps its level-99 loadout (its levels come from Sora's test EXP)");
    }

    #[test]
    fn the_party_record_round_trips_and_old_records_load() {
        let world = TestWorld::floor();
        let mut sora = new_game_sora();
        let mut rt = runtime(true);
        step(&mut rt, &mut sora, &world);
        rt.friend_reward(PartyChara::Donald, &FriendReward::Bonus(BonusCounts::HP_UP10));
        rt.set_ai_settings(PartyChara::Donald, kh_host::party::FriendAiSettings { battle: 3, ability: 2, cure: 0, cure_targets: 5 });
        rt.lead_word(&["leader", "donald"]);
        step(&mut rt, &mut sora, &world);
        let record = rt.party_record();
        assert_eq!((record["format"].as_u64(), record["friends_version"].as_u64(), record["leader"].as_str()), (Some(2), Some(5), Some("donald")));
        assert_eq!(record["friends"].as_array().map(|a| a.len()), Some(2));
        // through text, into a fresh runtime
        let text = record.to_string();
        let mut rt2 = runtime(true);
        let mut sora2 = new_game_sora();
        rt2.load_party_record(&serde_json::from_str(&text).unwrap());
        for _ in 0..3 {
            step(&mut rt2, &mut sora2, &world);
        }
        assert_eq!(rt2.leader_chara(), PartyChara::Donald);
        assert_eq!(rt2.ai_settings(PartyChara::Donald).battle, 3);
        let d = rt2.party.status(rt2.id(PartyChara::Donald).unwrap()).unwrap();
        assert_eq!(d.bonuses.0[BonusCounts::HP_UP10], 1);
        // a v4 record (format 1 / 2 without friends) and none at all: KH3's init data
        for old in [json!({ "format": 1, "leader": "sora", "active": ["sora", "goofy"] }), Value::Null] {
            let save = read_party_record(&old);
            assert!(save.friends.is_empty());
            let mut rt3 = runtime(true);
            let mut sora3 = new_game_sora();
            rt3.load_party_record(&old);
            for _ in 0..2 {
                step(&mut rt3, &mut sora3, &world);
            }
            let g = rt3.party.status(rt3.id(PartyChara::Goofy).unwrap()).unwrap();
            assert_eq!((g.weapon, g.bonuses), ("WEP_GOOFY_00", BonusCounts::NONE));
        }
        // an odd record: unknown characters dropped, counts clamped
        let odd = json!({ "format": 2, "leader": "riku", "friends": [ { "chara": "riku" }, { "chara": "donald", "weapon": "WEP_NOPE", "slots": [9, 9, 9], "bonuses": [999], "abilities": [["MP_HASTE", 7, 255]] } ] });
        let save = read_party_record(&odd);
        assert_eq!(save.formation, Formation::default());
        assert_eq!(save.friends.len(), 1);
        assert_eq!((save.friends[0].item_slots, save.friends[0].bonuses[0]), (4, 255));
    }

    #[test]
    fn a_reload_takes_back_what_the_friends_earned_after_the_save() {
        let world = TestWorld::floor();
        let mut sora = new_game_sora();
        let mut rt = runtime(true);
        step(&mut rt, &mut sora, &world);
        rt.friend_reward(PartyChara::Donald, &FriendReward::parse("ability:MP_HASTE").unwrap());
        let saved = rt.party_record();
        // after the save: another bonus and ability, then a death and the reload of that save
        rt.friend_reward(PartyChara::Donald, &FriendReward::parse("bonus:HP_UP10").unwrap());
        rt.friend_reward(PartyChara::Donald, &FriendReward::parse("ability:FIRE_UP").unwrap());
        rt.reset_friends_for_load(sora.status().exp);
        rt.load_party_record(&saved);
        step(&mut rt, &mut sora, &world);
        let d = rt.party.status(rt.id(PartyChara::Donald).unwrap()).unwrap();
        assert_eq!(d.abilities.owned(A::MpHaste), 1, "what the save had");
        assert_eq!((d.abilities.owned(A::FireUp), d.bonuses.0[BonusCounts::HP_UP10]), (0, 0), "what came after it is gone");
        assert_eq!(FriendRecord::of(PartyChara::Donald, d), read_party_record(&saved).friends[0]);
    }

    #[test]
    fn the_prize_and_save_point_rules_reach_the_friends() {
        let world = TestWorld::floor();
        let mut sora = new_game_sora();
        let mut rt = runtime(true);
        step(&mut rt, &mut sora, &world);
        let (members, ids) = rt.prize_members(&sora, Vec3::new(0.0, 0.0, 90.0));
        assert_eq!(members.iter().map(|m| m.name).collect::<Vec<_>>(), vec!["Sora", "Donald", "Goofy"]);
        assert!(members[0].player && !members[1].player);
        assert_eq!(ids[1], rt.id(PartyChara::Donald));
        let g = rt.id(PartyChara::Goofy).unwrap();
        rt.party.status_mut(g).unwrap().hp = 5;
        rt.apply_prize(PartyChara::Goofy, PrizeKind::HpLarge);
        assert_eq!(rt.party.status(g).unwrap().hp, 10);
        step(&mut rt, &mut sora, &world);
        assert!(rt.savepoint_needs(&sora), "a friend below his max HP");
        rt.savepoint_recover(&mut sora, 500);
        step(&mut rt, &mut sora, &world);
        assert_eq!(rt.view(PartyChara::Goofy).map(|v| v.hp == v.hp_max), Some(true));
        let mods = rt.prize_mods(kh_prize::SpawnMods::default());
        assert_eq!(mods.luck_up, 0);
    }
}

//! (DON-PROG) DONALD'S OVERHAUL IN ELDEN RING, the host side of stream DON-PROG (documentation/DONALD_OVERHAUL.md 2, 5.4,
//! 9; build/sora_tmp/party/DON_PLAN.md R21 / R27):
//!
//! 1. TUNING WITHOUT A REBUILD ([`frame`]): at the party's first frame and at every reload of the control file, the
//!    Donald sheets are read again (party-don-core, party-don-spells, party-don-progression) and the `don.<key> = value`
//!    keys laid on top (`kh_partyhost::donald::install_all`); the keys no sheet knows are logged once per reload.
//! 2. HIS BOSS STEPS ([`frame`], twice a second): every boss Sora's records have beaten (`boss:<row>`) whose Donald steps
//!    (`don.<row>` of sheets/port/sora-er-boss-rewards.json) are not recorded (`don:<row>`) gives them to the roster's
//!    Donald, joined or not (he joins with them): a boss beaten now shows his own BONUS! panel (KH3's magic icon on
//!    magic, ER_PHASE_NOTES 2026-10-19) when he is in the party; the first look after a load is THE CATCH-UP of an older
//!    save (every beaten boss once, one panel). Records are only added (`don:<row>`, beside `boss:` / `party:` / `xs:`).
//! 3. HIS AGGRO, THE HATE ROUTE ([`hate`], once a frame after the taunt): Elden Ring's enemies take as their target what
//!    hurt them, so an enemy that would rather not choose Donald (Low Profile / Hide Behind by roll, Distraction, Out of
//!    Sight) is given a minimal hit from the ally's Elden Ring body it should turn to (the route of Goofy's Over Here!,
//!    party/taunt.rs: 1 point, no poise, no reaction); Taunt Pass: an ally guarding (Goofy's GUARD, Sora's block) has
//!    the enemies within 8 m of Donald hit from his body. An enemy "is on Donald" when he is the party member nearest it
//!    within `aggro.er_reach` (Elden Ring has no readable target here). NEVER A HIT MOVED (R21): only whom the enemy
//!    chooses next. One bullet a frame (the bullet slots are shared). `party.donaggro = 0`: off. UNTESTED IN GAME.

use std::sync::Mutex;

use glam::Vec3;
use kh_core::TargetId;
use kh_host::party::PartyChara;
use kh_party::donald::aggro::{self, AggroCtx};
use kh_party::donald::DonAbilities;

use crate::control::Control;
use crate::erlog;

#[derive(Default)]
struct State {
    /// The control file's generation the sheets were last installed for.
    gen: Option<u64>,
    poll_in: f32,
    /// The first look at the records was made (the catch-up).
    looked: bool,
    clock: f32,
    /// (enemy, when) of the hate hits and the last rolls.
    hit: Vec<(TargetId, f32)>,
    rolled: Vec<(TargetId, f32)>,
    told_no_body: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn table() -> &'static kh_host::bossrewards::Table {
    static T: std::sync::OnceLock<kh_host::bossrewards::Table> = std::sync::OnceLock::new();
    T.get_or_init(kh_host::bossrewards::Table::load)
}

/// The BONUS! panel's lines of Donald's grants: magic after KH3's magic icon, Max MP KH3's own stat line.
pub fn panel_lines(grants: &[kh_party::donald::Grant]) -> Vec<String> {
    use kh_ui::hud_side::{icon_line, LineIcon};
    let d = kh_party::donald::data();
    let side = kh_ui::hud_side::SideStrings::default();
    grants.iter().map(|g| if g.is_magic() { icon_line(LineIcon::Magic, &g.label(&d)) } else { side.bonus_lines[1].clone() }).collect()
}

/// ONCE A FRAME with the party's core (party/mod.rs `PartySlot::frame`): the tuning, his boss steps.
#[cfg(windows)]
pub fn frame(core: &mut super::glue::PartyCore, control: &Control, dt: f32) {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let st = guard.get_or_insert_with(State::default);
    st.clock += dt.max(0.0);
    // 1. the sheets and the don.* keys, at the start and at every reload
    if st.gen != Some(control.generation) {
        st.gen = Some(control.generation);
        let done = kh_partyhost::donald::install_all(control.pairs());
        erlog!("don: the Donald sheets installed (party-don-core, party-don-spells, party-don-progression read again){}{}", if done.applied.is_empty() { String::new() } else { format!("; ctl keys applied: {}", done.applied.join(", ")) }, if done.unknown.is_empty() { String::new() } else { format!("; KEYS NO DONALD SHEET KNOWS (ignored): {}", done.unknown.join(", ")) });
    }
    // 2. his boss steps, twice a second
    st.poll_in -= dt;
    if st.poll_in > 0.0 || control.flag("boss.rewards") == Some(false) {
        return;
    }
    st.poll_in = 0.5;
    let t = table();
    let mut records = crate::hud::records();
    let before = records.len();
    let got = core.rt.don_claim_due(&t.don, &mut records);
    let first = !std::mem::replace(&mut st.looked, true);
    if records.len() != before {
        crate::hud::set_records(records);
    }
    if got.is_empty() {
        return;
    }
    let d = kh_party::donald::data();
    for (boss, grants) in &got {
        erlog!("don: {} beaten: Donald's step(s) {} (row don.{boss}; recorded don:{boss}; he has them joined or not)", t.row(boss).map_or(boss.as_str(), |r| r.name.as_str()), grants.iter().map(|g| g.label(&d)).collect::<Vec<_>>().join(", "));
    }
    let present = crate::partyui::present(control).contains(&PartyChara::Donald);
    if !present {
        return;
    }
    if first && got.len() > 1 {
        // the catch-up of an older save: one panel
        let spells = got.iter().flat_map(|g| g.1.iter()).filter(|g| g.is_magic()).count();
        crate::partyui::friend_bonus(PartyChara::Donald, vec![format!("Rewards of {} bosses obtained", got.len()), format!("{spells} magic")]);
    } else {
        for (_, grants) in got {
            crate::partyui::friend_bonus(PartyChara::Donald, panel_lines(&grants));
        }
    }
}

/// One ally an enemy may be turned to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ally {
    pub chara: PartyChara,
    pub pos: Vec3,
}

/// What [`plan`] reads.
pub struct HateIn<'a> {
    pub donald: Vec3,
    pub abilities: &'a DonAbilities,
    pub ctx: AggroCtx,
    /// The other living members with a body (Sora too while he is in the party).
    pub allies: &'a [Ally],
    /// (id, KH3 position) of the hostiles.
    pub enemies: &'a [(TargetId, Vec3)],
    /// Taunt Pass's ally now (an ally guarding).
    pub taunt: Option<PartyChara>,
    pub now: f32,
    /// (ZETTA round 5) Donald is in Zettaflare's sequence: his weight is 0 and every enemy on him is turned (no roll).
    pub zetta: bool,
}

/// THE PURE RULE: the one hate hit due this frame (enemy, the ally whose body hits it). `hit` / `rolled`: (enemy, when)
/// of the hits so far and of the last rolls (kept by the caller).
pub fn plan(i: &HateIn, hit: &mut Vec<(TargetId, f32)>, rolled: &mut Vec<(TargetId, f32)>) -> Option<(TargetId, PartyChara)> {
    // (ZETTA round 5) the user: "enemies don't ever target Donald while he is performing Zettaflare": every enemy on him is
    // turned to the ally nearest it, again each period while the sequence lasts (the same 1-point hate hit; no roll, no
    // party-size rule); with no ally body nothing can be turned (he is invincible meanwhile: kh-party)
    if i.zetta {
        if i.allies.is_empty() {
            return None;
        }
        let p = kh_party::donald::ability_info::prog();
        let (period, reach) = (p.value("aggro.er_period").max(0.1), p.value("aggro.er_reach"));
        let mut on_him: Vec<(TargetId, Vec3, f32)> = i.enemies.iter().map(|(id, pos)| (*id, *pos, pos.distance(i.donald))).filter(|(_, pos, d)| *d <= reach && i.allies.iter().all(|a| a.pos.distance(*pos) > *d)).collect();
        on_him.sort_by(|a, b| a.2.total_cmp(&b.2));
        for (id, pos, _) in on_him {
            if hit.iter().find(|h| h.0 == id).is_some_and(|h| i.now - h.1 < period) {
                continue;
            }
            let Some(a) = i.allies.iter().copied().min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos))) else { break };
            hit.retain(|h| h.0 != id);
            hit.push((id, i.now));
            return Some((id, a.chara));
        }
        return None;
    }
    if !aggro::applies(&i.ctx) || i.allies.is_empty() {
        return None;
    }
    let p = kh_party::donald::ability_info::prog();
    let period = p.value("aggro.er_period").max(0.1);
    let reach = p.value("aggro.er_reach");
    let nearest_ally_to = |at: Vec3| i.allies.iter().copied().min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)));
    let recent = |list: &Vec<(TargetId, f32)>, id: TargetId| list.iter().find(|h| h.0 == id).is_some_and(|h| i.now - h.1 < period);
    let mut on_him: Vec<(TargetId, Vec3, f32)> = i
        .enemies
        .iter()
        .map(|(id, pos)| (*id, *pos, pos.distance(i.donald)))
        .filter(|(_, pos, d)| *d <= reach && i.allies.iter().all(|a| a.pos.distance(*pos) > *d))
        .collect();
    on_him.sort_by(|a, b| a.2.total_cmp(&b.2));
    for (id, pos, d) in on_him {
        if recent(hit, id) {
            continue;
        }
        // Taunt Pass: the guarding ally takes the enemies near Donald
        if let Some(c) = i.taunt.filter(|_| aggro::taunt_pass(i.abilities, &i.ctx, d)) {
            if let Some(a) = i.allies.iter().find(|a| a.chara == c) {
                hit.retain(|h| h.0 != id);
                hit.push((id, i.now));
                return Some((id, a.chara));
            }
        }
        if recent(rolled, id) {
            continue;
        }
        rolled.retain(|r| r.0 != id);
        rolled.push((id, i.now));
        let bucket = (i.now / period).floor() as u64;
        let w = aggro::pick_weight(i.abilities, &i.ctx, d);
        let to = if aggro::roll(id.0 as u64, bucket, 11) >= w {
            nearest_ally_to(pos)
        } else if aggro::switch_on_target(i.abilities, &i.ctx, aggro::roll(id.0 as u64, bucket, 12)) {
            nearest_ally_to(i.donald)
        } else {
            None
        };
        if let Some(a) = to {
            hit.retain(|h| h.0 != id);
            hit.push((id, i.now));
            return Some((id, a.chara));
        }
    }
    None
}

/// ONCE A FRAME (sora.rs pre_physics, after the taunt): the hate route's one hit of this frame. `sora_pos`: Sora's KH3
/// feet when he is in the party.
#[cfg(windows)]
pub fn hate(core: Option<&super::glue::PartyCore>, player: eldenring::cs::FieldInsHandle, targets: &mut crate::targets::ErTargets, control: &Control, sora_pos: Option<Vec3>, _dt: f32) {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let st = guard.get_or_insert_with(State::default);
    let Some(core) = core.filter(|_| control.flag("party.donaggro") != Some(false)) else { return };
    // (AGGRO) the director (party/aggro.rs) applies Donald's rules (kh-party donald::aggro) to every enemy itself;
    // this hate route runs only with `party.aggro = 0`
    if super::aggro::on(control) {
        return;
    }
    let rt = &core.rt;
    let Some(dv) = rt.current.iter().find(|v| v.chara == kh_party::Chara::Donald && !v.ko) else { return };
    let Some(abilities) = rt.party.status(dv.id).and_then(|s| s.donald.as_ref()).map(|d| d.abilities) else { return };
    let ctx = rt.party.don_prog.aggro_of(dv.id);
    let mut allies: Vec<Ally> = rt.current.iter().filter(|v| v.id != dv.id && !v.ko).map(|v| Ally { chara: kh_partyhost::party_chara(v.chara), pos: v.position }).collect();
    if let Some(p) = sora_pos.filter(|_| rt.lead.sora_active) {
        allies.push(Ally { chara: PartyChara::Sora, pos: p });
    }
    let taunt = rt.party.don_prog.taunt_now().and_then(|m| if m == kh_party::donald::cast::SORA_MEMBER { Some(PartyChara::Sora) } else { rt.chara_of_id(m) });
    if !targets.bullets.available() || !targets.bullets.free_slot() {
        return;
    }
    let enemies: Vec<(TargetId, Vec3)> = targets.hostiles_by_distance().into_iter().map(|(_, e)| (e.id, e.pos)).collect();
    let input = HateIn { donald: dv.position, abilities: &abilities, ctx, allies: &allies, enemies: &enemies, taunt, now: st.clock, zetta: rt.party.don_zetta_invincible(dv.id) /* (ZETTA round 5) */ };
    let (mut hit, mut rolled) = (std::mem::take(&mut st.hit), std::mem::take(&mut st.rolled));
    let picked = plan(&input, &mut hit, &mut rolled);
    (st.hit, st.rolled) = (hit, rolled);
    let Some((id, to)) = picked else { return };
    let Some(e) = targets.get(id).cloned() else { return };
    let leader = rt.leader_chara();
    let owner = if to == leader { Some((player, false)) } else { crate::allybody::handle(to).map(|h| (h, true)) };
    let Some((owner, owned)) = owner else {
        if !std::mem::replace(&mut st.told_no_body, true) {
            erlog!("don: aggro: {} has no Elden Ring body to pull the hate to (S3's pool has none for him now)", to.name());
        }
        return;
    };
    let from_pos = allies.iter().find(|a| a.chara == to).map_or(dv.position, |a| a.pos);
    let towards = crate::coords::dir_kh_to_er((e.pos - from_pos) * Vec3::new(1.0, 1.0, 0.0)).normalize_or(Vec3::Z);
    let contact = e.er_pos + Vec3::Y * (e.half_height / crate::coords::CM_PER_M);
    let from = contact - towards * targets.shape.back;
    let attack = crate::bullet::Attack { phys: 1, poise: 0.0, stamina: 0, level: 0, carrier: Default::default(), sp_attribute: None, obj: None };
    let shape = targets.shape;
    let fired = if owned { targets.bullets.fire_owned(owner, Some(e.handle), from, towards, attack, shape) } else { targets.bullets.fire(owner, Some(e.handle), from, towards, attack, shape) };
    match fired {
        Ok(row) => erlog!("don: aggro: target {} c{:04} turned from Donald to {} (a 1-point hate hit from {}, bullet row {row}; never a hit moved)", id.0, e.npc_id, to.name(), if owned { "his ally body" } else { "the leader body" }),
        Err(why) => erlog!("don: aggro: the hate hit on target {} could not be fired ({why})", id.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_party::donald::DonAbility;

    fn with(list: &[DonAbility]) -> DonAbilities {
        let mut a = DonAbilities::default();
        for k in list {
            a.give(*k);
            a.set_equipped(*k, true);
        }
        a
    }

    fn party() -> AggroCtx {
        AggroCtx { others: 2, nearest_ally: 500.0, still: false, casting: false }
    }

    #[test]
    fn don_prog_the_hate_route_turns_enemies_by_his_weight_and_never_alone() {
        let goofy = Ally { chara: PartyChara::Goofy, pos: Vec3::new(-500.0, 0.0, 0.0) };
        let sora = Ally { chara: PartyChara::Sora, pos: Vec3::new(0.0, 900.0, 0.0) };
        let allies = [goofy, sora];
        let enemies: Vec<(TargetId, Vec3)> = (1..=40).map(|n| (TargetId(n), Vec3::new(400.0 + n as f32, 0.0, 0.0))).collect();
        let lp = with(&[DonAbility::LowProfile]);
        // over 200 s of 2-s windows: about 20 % of the enemies on him are turned (Low Profile)
        let (mut hit, mut rolled) = (Vec::new(), Vec::new());
        let mut turned = 0;
        let mut now = 0.0;
        while now < 200.0 {
            let i = HateIn { donald: Vec3::ZERO, abilities: &lp, ctx: party(), allies: &allies, enemies: &enemies, taunt: None, now, zetta: false };
            while let Some((_, to)) = plan(&i, &mut hit, &mut rolled) {
                assert_eq!(to, PartyChara::Goofy, "the nearest ally to the enemy");
                turned += 1;
            }
            now += 2.0;
        }
        let share = turned as f32 / (40.0 * 100.0);
        assert!((share - 0.2).abs() < 0.03, "{share}");
        // alone: never; no ability: never; Out of Sight while still and casting at 15 m+: always
        let i = HateIn { donald: Vec3::ZERO, abilities: &lp, ctx: AggroCtx::ALONE, allies: &allies, enemies: &enemies, taunt: None, now: 500.0, zetta: false };
        assert_eq!(plan(&i, &mut Vec::new(), &mut Vec::new()), None);
        let none = DonAbilities::default();
        let i = HateIn { donald: Vec3::ZERO, abilities: &none, ctx: party(), allies: &allies, enemies: &enemies, taunt: None, now: 500.0, zetta: false };
        let (mut h, mut r) = (Vec::new(), Vec::new());
        assert!(std::iter::from_fn(|| plan(&i, &mut h, &mut r)).next().is_none());
        let oos = with(&[DonAbility::OutOfSight]);
        let far: Vec<(TargetId, Vec3)> = vec![(TargetId(7), Vec3::new(1600.0, 0.0, 0.0))];
        let near_goofy = [Ally { chara: PartyChara::Goofy, pos: Vec3::new(-300.0, 0.0, 0.0) }];
        let quiet = AggroCtx { still: true, casting: true, ..party() };
        let i = HateIn { donald: Vec3::ZERO, abilities: &oos, ctx: quiet, allies: &near_goofy, enemies: &far, taunt: None, now: 1.0, zetta: false };
        assert_eq!(plan(&i, &mut Vec::new(), &mut Vec::new()), Some((TargetId(7), PartyChara::Goofy)));
        // an enemy nearer an ally than Donald is not "on him": left alone
        let by_goofy = [(TargetId(9), Vec3::new(-450.0, 0.0, 0.0))];
        let i = HateIn { donald: Vec3::ZERO, abilities: &oos, ctx: quiet, allies: &near_goofy, enemies: &by_goofy, taunt: None, now: 1.0, zetta: false };
        assert_eq!(plan(&i, &mut Vec::new(), &mut Vec::new()), None);
    }

    #[test]
    fn don_prog_taunt_pass_turns_the_enemies_near_him_to_the_guarding_ally() {
        let tp = with(&[DonAbility::TauntPass]);
        let allies = [Ally { chara: PartyChara::Goofy, pos: Vec3::new(-900.0, 0.0, 0.0) }, Ally { chara: PartyChara::Sora, pos: Vec3::new(0.0, 900.0, 0.0) }];
        let enemies = [(TargetId(1), Vec3::new(300.0, 0.0, 0.0)), (TargetId(2), Vec3::new(850.0, 0.0, 0.0))];
        let i = HateIn { donald: Vec3::ZERO, abilities: &tp, ctx: party(), allies: &allies, enemies: &enemies, taunt: Some(PartyChara::Goofy), now: 1.0, zetta: false };
        let (mut h, mut r) = (Vec::new(), Vec::new());
        assert_eq!(plan(&i, &mut h, &mut r), Some((TargetId(1), PartyChara::Goofy)));
        assert_eq!(plan(&i, &mut h, &mut r), None, "the one at 8.5 m is out of reach; once per interval");
        let p = |n: &str| panel_lines(&[kh_party::donald::Grant::Tier(kh_party::donald::Element::Fire, 2), kh_party::donald::Grant::MaxMp(3)]).iter().any(|l| l.contains(n));
        assert!(p("<icon(Magic)>Fira") && p("Maximum MP increased!"));
    }

    /// (ZETTA round 5) "enemies don't ever target Donald while he is performing Zettaflare": every enemy on him is turned to
    /// the ally nearest it, whatever his abilities or the party's size, again each period; none when no ally has a body.
    #[test]
    fn zetta_every_enemy_on_donald_is_turned_away_from_him() {
        let goofy = Ally { chara: PartyChara::Goofy, pos: Vec3::new(-500.0, 0.0, 0.0) };
        let sora = Ally { chara: PartyChara::Sora, pos: Vec3::new(0.0, 900.0, 0.0) };
        let allies = [goofy, sora];
        let enemies: Vec<(TargetId, Vec3)> = (1..=6).map(|n| (TargetId(n), Vec3::new(300.0 + 20.0 * n as f32, 0.0, 0.0))).collect();
        let none = DonAbilities::default();
        let (mut hit, mut rolled) = (Vec::new(), Vec::new());
        let i = HateIn { donald: Vec3::ZERO, abilities: &none, ctx: AggroCtx::ALONE, allies: &allies, enemies: &enemies, taunt: None, now: 1.0, zetta: true };
        let turned: Vec<(TargetId, PartyChara)> = std::iter::from_fn(|| plan(&i, &mut hit, &mut rolled)).take(20).collect();
        assert_eq!(turned.len(), 6, "every enemy on him, once a period");
        assert!(turned.iter().all(|t| t.1 == PartyChara::Goofy), "to the ally nearest it");
        let later = HateIn { now: 1.0 + kh_party::donald::ability_info::prog().value("aggro.er_period").max(0.1) + 0.01, ..i };
        assert_eq!(std::iter::from_fn(|| plan(&later, &mut hit, &mut rolled)).take(20).count(), 6, "and again the next period");
        let alone = HateIn { allies: &[], ..i };
        assert_eq!(plan(&alone, &mut Vec::new(), &mut Vec::new()), None);
    }
}

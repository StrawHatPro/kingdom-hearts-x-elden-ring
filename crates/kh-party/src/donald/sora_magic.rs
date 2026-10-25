//! (DON-MAGIC part A) THE MAGIC DONALD SHARES WITH SORA (the user, 2026-10-22: "The magic he shares with Sora should look and
//! behave the exact same! INCLUDING his -za magic spells!"): line 1 of every element, Fire / Fira / Firaga / Firaza, Blizzard ..,
//! Thunder .., Water .., Aero .., Cure / Cura / Curaga / Curaza.
//! - THE SAME CODE AS SORA'S: a cast is a shot of Sora's own spell actors, `kh_core::magic::caster::SpellCaster` (the
//!   projectiles, Thunder's strikes, Water's orbit, Aero's barrier and tornado, Cure's heal, their multi-hits, sounds and effects),
//!   with Sora's command (tiers 1-3: Fire 29 ..; the -za tier: Sora's Grand Magic "-gan" command, Firaza 112 .. Curaza 125,
//!   `magic_fx::sora_cmd`) and Donald's own stats (Strength, Magic, level). Its hits go through the reaction rule and
//!   Donald's statuses (`magic_fx::L1Targets`: `Party::don_hit`), his meters and Resonance (the core decides).
//! - AIMED LIKE SORA'S: the shot leaves facing its target (Sora's casts start toward the target; a pose turned by the AI
//!   between the start and the shot made the fireballs miss), from the cast clip's own socket (`cast::muzzle`).
//! - THE CAST POSE: Donald's KH3 magic clips as KH3 picks them for his own spells (`cast::clip_of`: MagicFire 1321 /
//!   MagicAirFire 1331 for the thrown Fire and Blizzard, MagicCast 1341 / MagicAirCast 1351 for the rest and the -za tier);
//!   an AI Donald is held in the play-motion state for the clip ([`hold_pose`]; kh-party pawn.rs refuses his move / turn
//!   commands while a cast runs, marked), so his locomotion no longer runs over the clip.
//! - THE HOSTS DRAW THEM: Donald's casters' spell actors and Blizzard's ice rails are in `Party::caster_projectiles` /
//!   `caster_rails` (leader_magic.rs, marked) like Goofy's and the leader's, so the hosts place their effects.

use super::abilities::DonAbility;
use super::element::Element;
use super::magic_fx::{self, magic, Aim, Ctx, FxLog, HitSpec, L1Targets, MagicFx};
use super::spells::SpellRef;
use super::tune::data;
use crate::ids::MemberId;
use crate::pawn::NpcPawn;
use crate::states::NpcStateId;
use crate::view::SoraView;
use glam::Vec3;
use kh_core::host::targets::Ally;
use kh_core::host::{PartyMember, TargetId};
use kh_core::magic::caster::{CasterStats, Shot, SpellCaster};

/// THE CAST POSE (see the module text): an AI Donald casting a line-1 spell holds the play-motion state for the clip just
/// started, so no move / turn of his runs it over (a pad-driven Donald's own state machine holds it already).
pub fn hold_pose(p: &mut NpcPawn, spell: SpellRef) {
    if !matches!(spell, SpellRef::Line1(..)) || p.pad.is_some() {
        return;
    }
    if p.state != NpcStateId::PlayMotion {
        let (clip, time, speed) = (p.anim.clip, p.anim.time, p.anim.speed);
        p.enter(NpcStateId::PlayMotion);
        // entering may have changed the pose: the cast clip stays
        if p.anim.clip.sequence != clip.sequence {
            p.anim.play(clip, 0.1);
            p.anim.time = time;
        }
        p.anim.speed = speed;
    }
}

/// THE SHOT of a line-1 cast: aimed at its target as Sora's are, then Sora's actors.
#[allow(clippy::too_many_arguments)]
pub fn shoot(fxw: &mut MagicFx, cx: &mut Ctx, caster: MemberId, e: Element, tier: u8, aim: Aim, pos: Vec3, yaw: f32, mz: Vec3) {
    // facing the target at the shot (Sora's start turn is toward it; the clip's pose may have been turned since)
    let yaw = match aim.target().and_then(|t| cx.host.targets.info(t)) {
        Some(t) if crate::pawn::horizontal(t.center - pos).length() > 1.0 => crate::pawn::yaw_of(crate::pawn::horizontal(t.center - pos)),
        _ => yaw,
    };
    if let Some(p) = cx.party.member_mut(caster) {
        p.yaw = yaw;
    }
    sora_line1(fxw, cx, caster, e, tier, aim, pos, yaw, mz);
}

const ALLY_SORA: TargetId = TargetId(0xA11E_00FF);
const fn ally_member(id: MemberId) -> TargetId {
    TargetId(0xA11E_0000 + id.0 as u32)
}

#[allow(clippy::too_many_arguments)]
fn sora_line1(fxw: &mut MagicFx, cx: &mut Ctx, caster: MemberId, e: Element, tier: u8, aim: Aim, pos: Vec3, yaw: f32, mz: Vec3) {
    let cmd = magic_fx::sora_cmd(e, tier);
    let Some((stats, own, full, abilities)) = cx.party.member(caster).and_then(|p| {
        let st = p.status.donald.as_ref()?;
        Some((
            CasterStats { strength: p.status.strength(), magic: p.status.magic(), game_level: p.status.game_level, battle_level: 1, mp_cost_scale: 1.0, mp_charging: p.status.mp_charge_left > 0.0 },
            p.status.abilities.clone(),
            st.full_flags(&data()),
            st.abilities,
        ))
    }) else {
        return;
    };
    let k = match fxw.casters.iter().position(|c| c.0 == caster) {
        Some(k) => k,
        None => {
            fxw.casters.push((caster, SpellCaster::new(PartyMember(caster.0))));
            fxw.casters.len() - 1
        }
    };
    let full_mp = cx.party.status(caster).is_some_and(|s| s.mp >= s.max_mp());
    let target = aim.target().unwrap_or(TargetId::NONE);
    let shot = Shot { cmd, origin: pos, yaw, muzzle: Some(mz), target, finish: false, grand: tier >= 4, full_mp, combo: [1, 0, 0, 0] };
    let spell = SpellRef::Line1(e, tier);
    let c = &mut fxw.casters[k].1;
    c.set_stats(stats, &own, &kh_core::status::AbilitySet::default());
    c.place(pos, yaw);
    let parts = {
        let host = &mut *cx.host;
        let mut t = L1Targets::new(&mut *host.targets, &mut cx.party.don.status, full, abilities, spell);
        c.shoot(host.world, &mut t, &mut *host.audio, &mut *host.effects, &shot);
        t.into_parts()
    };
    match fxw.caster_spell.iter_mut().find(|s| s.0 == caster) {
        Some(s) => s.1 = spell,
        None => fxw.caster_spell.push((caster, spell)),
    }
    cx.log.push(FxLog::Sora { line: 1, cmd });
    finish_l1(cx, caster, spell, parts);
}

/// After Sora's actors hit: the caster's meters for the reactions, Conduct chains / Wildfire, the log.
#[allow(clippy::type_complexity)]
fn finish_l1(cx: &mut Ctx, caster: MemberId, spell: SpellRef, parts: (Vec<(u32, super::react::Reaction)>, Vec<(kh_core::host::TargetInfo, super::react::HitOutcome, f32)>, Vec<FxLog>)) {
    let d = data();
    let (rewards, outcomes, log) = parts;
    if let Some(st) = cx.party.status_mut(caster).and_then(|s| s.donald.as_mut()) {
        let mods = st.abilities.mods();
        for (t, r) in &rewards {
            st.core.on_reaction(*r, *t, spell.elements(), &mods, &d);
        }
    }
    cx.log.extend(log);
    for (t, out, dmg) in outcomes {
        let Some(row) = spell.row(&d).copied() else { continue };
        // the chained hits of a Conduct: a Thunder hit of the line-1 tier's own power
        let spec = HitSpec::of(spell, &row);
        magic_fx::after_reactions(cx, caster, &spec, &t, &out, dmg, &d);
    }
}

/// Every Donald's caster (Sora's actors) one step: the hits through the reaction rule, the Cure heals.
pub fn step_casters(fxw: &mut MagicFx, cx: &mut Ctx, sora: Option<&SoraView>, dt: f32) {
    if fxw.casters.is_empty() {
        return;
    }
    let members: Vec<(MemberId, Vec3, bool, f32)> = cx.party.members().collect::<Vec<_>>().into_iter().filter_map(|i| cx.party.member(i).map(|m| (m.id, m.position(), m.status.ko(), m.yaw))).collect();
    let mut casters = std::mem::take(&mut fxw.casters);
    for (id, c) in casters.iter_mut() {
        let Some(&(_, at, _, yaw)) = members.iter().find(|m| m.0 == *id) else { continue };
        c.place(at, yaw);
        let mut allies: Vec<Ally> = members.iter().filter(|m| m.0 != *id).map(|m| Ally { id: ally_member(m.0), position: m.1, dead: m.2 }).collect();
        if let Some(s) = sora {
            match s.ai_sora {
                Some((_, p)) => allies.push(Ally { id: ALLY_SORA, position: p, dead: false }),
                None => allies.push(Ally { id: ALLY_SORA, position: s.position, dead: s.dead }),
            }
        }
        let Some((full, abilities)) = cx.party.status(*id).and_then(|s| s.donald.as_ref()).map(|st| (st.full_flags(&data()), st.abilities)) else { continue };
        let spell = fxw.caster_spell.iter().find(|s| s.0 == *id).map_or(SpellRef::Line1(Element::Fire, 1), |s| s.1);
        let (step, parts) = {
            let host = &mut *cx.host;
            let mut t = L1Targets::new(&mut *host.targets, &mut cx.party.don.status, full, abilities, spell);
            let s = c.step(host.world, &mut t, &mut *host.audio, &mut *host.effects, &allies, dt);
            (s, t.into_parts())
        };
        let cure = if abilities.has(DonAbility::CureBoost) { magic().boost_cure } else { 1.0 };
        // the Cure line's row power: x on KH3's heal (the sheet's 1 = KH3's own)
        let cure = cure * match spell {
            SpellRef::Line1(Element::Light, _) => spell.row(&data()).map_or(1.0, |r| r.power),
            _ => 1.0,
        };
        for heal in step.heals {
            let amount = ((heal.amount as f32) * cure).round() as i32;
            match heal.ally {
                None => {
                    cx.party.heal(*id, amount, heal.effect);
                    cx.log.push(FxLog::Heal { member: *id, amount });
                }
                Some(ALLY_SORA) => fxw.sora_heals.push((*id, amount, heal.effect)),
                Some(t) if t.0 & 0xFFFF_FF00 == 0xA11E_0000 => {
                    let m = MemberId((t.0 & 0xFF) as u8);
                    cx.party.heal(m, amount, heal.effect);
                    cx.log.push(FxLog::Heal { member: m, amount });
                }
                Some(_) => {}
            }
        }
        finish_l1(cx, *id, spell, parts);
    }
    casters.retain(|(id, c)| cx.party.member(*id).is_some() || c.alive() > 0);
    fxw.casters = casters;
}


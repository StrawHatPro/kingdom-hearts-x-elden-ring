//! Grand magic production (docs/sora-magic-spec.md part I 7): the element tallies that ride on
//! the formchange ("style") gauge, and the roll made when that gauge is full.
//!
//! The gauge itself and the situation-command QUEUE are the combat implementer's
//! (`combat/style.rs`; strand F). What is here, for them to call:
//!
//! - [`on_damage`]: every magic hit that dealt damage adds its row's form points through the
//!   gauge's own `add_hit` and tallies what was really added (after the chain factor) per element,
//!   with the highest command cast of that element (`elementPoints`, `lastCmd`: ctrl +0xB98 / +0xBA8);
//! - [`roll`]: `RollGrandMagic(alt, forced)` 0x1413ED500: called when the gauge reaches its
//!   maximum, BEFORE the formchange offer; returns the `S_*` commands to add to the queue (at most
//!   3). The caller then clears the gauge and calls [`reset`];
//! - [`lifetime`]: the queue lifetime of a grand-magic command (`Param0` 20 s x (1 + 0.2 x
//!   MAGIC_TIME), capped at 99.9 s; J 5.2);
//! - the commands execute through `command::execute(kind)` like any other (`TryMagic` with
//!   `bGrand`: no MP, the `_S` row, invincible until the shot), which also calls [`reset`].

use super::spells::{self, Element};
use super::tables::Sheet;
use crate::enums::ETresAbilityKind;
use crate::pawn::Ctx;

/// ctrl +0xB98 / +0xBA8 (indices = the element's `Attr`), and the pre-roll memory +0xB7C.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Grand {
    pub element_points: [f32; 8],
    pub last_cmd: [u8; 8],
    /// Offers made so far (for tests and the debug text).
    pub offers: u32,
}

/// A hit of the command `cmd` dealt damage: `AddStylePoints(points = m_FormPoint, kind 1, cmd)`
/// (I 7.1) = the gauge's own `style::add` (rates, chain factor, "grand-magic casts add nothing",
/// S_CURAGAN only with Cure Converter); what it really added is tallied per element here (the
/// combat side's own arrays index by its command sheet, which has no rows for the spells).
pub(crate) fn on_damage(cx: &mut Ctx, form_point: i32, cmd: u8) {
    if form_point <= 0 {
        return;
    }
    let before = cx.pawn.combat.style.pending;
    crate::combat::style::add(cx, form_point as f32, 1, cmd as i32);
    let added = cx.pawn.combat.style.pending - before;
    if added > 0.0 {
        if let Some(unit) = spells::unit(cmd) {
            let e = unit.element as usize;
            let grand = &mut cx.pawn.magic.grand;
            grand.element_points[e] += added;
            grand.last_cmd[e] = grand.last_cmd[e].max(cmd);
        }
    }
}

/// Every controller frame, after the gauge's tick: the tallies live exactly as long as the
/// gauge's points ("gauge = 0: elementPoints[*] = 0; lastCmd[*] = 0": the decay reached 0, an
/// offer was made, a grand magic was used, the form ended).
pub(crate) fn sync(cx: &mut Ctx) {
    let style = &cx.pawn.combat.style;
    if style.gauge <= 0.0 && style.pending <= 0.0 {
        reset(&mut cx.pawn.magic.grand);
    }
}

/// The gauge was emptied (it decayed to 0, an offer was made, or a grand magic was used).
pub fn reset(grand: &mut Grand) {
    grand.element_points = [0.0; 8];
    grand.last_cmd = [0; 8];
}

/// The `S_*` command an element offers: one tier above the highest tier cast since the gauge
/// was last empty (I 7.2 table).
pub fn offer_of(element: Element, last_cmd: u8) -> u8 {
    let base = match element {
        Element::Fire => 110,
        Element::Blizzard => 113,
        Element::Thunder => 116,
        Element::Water => 119,
        Element::Aero => 122,
        Element::Cure => return 125,
    };
    let tier1 = element.command(1);
    if last_cmd == tier1 {
        base
    } else if last_cmd == tier1 + 1 {
        base + 1
    } else {
        base + 2
    }
}

fn gan_of(element: Element) -> ETresAbilityKind {
    match element {
        Element::Aero => ETresAbilityKind::Aerogan,
        Element::Water => ETresAbilityKind::Watagan,
        Element::Thunder => ETresAbilityKind::Thundagan,
        Element::Blizzard => ETresAbilityKind::Blizzagan,
        _ => ETresAbilityKind::Firagan,
    }
}

// ---------------------------------------------------------------------------------------------
// (CC) `RollGrandMagic(bFinisher = alt, bForced)` 0x1413ED500 (controller slot 838, vt+0x1A30),
// transcribed in full from re/decomp/1413E/1413ED500.c and its disassembly (the decompiled tail
// after 0x1413EDF88 is garbled; read with build/sora_tmp/dis.js). In order:
//
//  0x1413ED533  pawn = ctrl+0x3E8; not a live ATresPlayerPawnBase -> return false.
//  0x1413ED5C2  ctrl vt+0x1A70() == 0x8B (the anti form) -> return false. The port has no anti
//               form (combat/style.rs header): never true here.
//  0x1413ED5EA  M = alt ? 20 : 10; HasAbility(GRAND_MAGIC 0xB4) -> M = int(M * 1.3f).
//  0x1413ED629  ctrl+0xB7C (the pre-roll's command) = 0.
//  0x1413ED643  !forced && Count(MAGIC_ROULETTE 0xBA) > 0 -> RollGrandMagic(alt, forced = 1): the
//               pre-roll offers its first result itself (AddSituationCommand, vt+0x1578) and
//               writes it to ctrl+0xB7C; its own return value is ignored.
//  per element, in this order: aero (0xB98+0x14, -GAN 0xB9 AEROGAN, top 0x7C), water (+0x10, 0xB8,
//  0x79), thunder (+0x0C, 0xB7, 0x76), blizzard (+0x08, 0xB6, 0x73), fire (+0x04, 0xB5, 0x70):
//    skip the element when ctrl+0xB7C == its top command (0x1413ED66B / 74A / 8A9 / 9D2 / B0F);
//    p = int(points[e]); g = Count(-GAN) + forced; g > 0 -> p += 15 g, lastCmd[e] = top (stored);
//    rand() % 1000 < p * M (0x1413ED6DB ...), and the character allows it (pawn vt+0xD70 =
//    m_ChrUniqueID: aero {1, 3}, water {1, 3, 7}, thunder any, blizzard / fire != 2; Sora is 1);
//    offered = lastCmd == tier 1 ? S_x-RA : lastCmd == tier 2 ? S_x-GA : S_x-GAN;
//    n += 1 ([rbp+40h]); n == 1 -> append, else insert at rand() % n.
//  Curagan 0x7D (0x1413EDC45), skipped when ctrl+0xB7C == 0x7D:
//    forced: max HP >= 1 and HP * 100 / max HP > 50 -> nothing; else !IsPinch (vt+0x2538) -> the
//      roll below with the ability test passed; else !vt+0x27F0() -> the list is CLEARED and holds
//      0x7D alone (n = 1), no roll.
//    the roll: (HasAbility(CURAGAN 0xB1) or the forced low-HP case) and !vt+0x27F0(); v = 25 (50
//      when IsPinch) + 10 per party member (pawn vt+0x2190 party, vt+0xEF8(0..5)) that is dead
//      (vt+0x2540); rand() % 1000 < v * M -> n += 1, inserted like an element.
//  forced (0x1413EDE9C): list non-empty -> ctrl+0xB7C = list[0], AddSituationCommand(list[0], -1, 0);
//    return true.
//  0x1413EDEE6  k = min(len, 3 - (ctrl+0xB7C != 0)); AddSituationCommand(list[i], -1, 0) for i < k;
//               result = k > 0.
//  0x1413EDF4E  !result: pawn vt+0xD80() (pawn+0x15B4, the pawn's mode byte) == 8 -> offer 0x7E
//               S_DARKFIRAGA, == 0x11 -> 0xE7 S_CROSSSHOOT; result = true. Sora's byte is 0 (ctor
//               0x1413E7622): never.
//  0x1413EDF88  int(points[0]) > 0 (the PHYSICAL points, ctrl+0xB98[0]): if (4 - min(n, 3)) * 250 >
//               rand() % 1000 -> result = false (n = every success of this pass, Curagan included).
//  0x1413EDFE5  Count(CRITICAL_CONVERTER 0x6A) > 0 -> result = false.
//  return result. AwardAtMax 0x1413F0ED0 offers the formchange when the result is false: with
//  physical points there is a 75 / 50 / 25 % chance (1 / 2 / 3+ successes) of the formchange
//  next to the grand magic, and with Critical Converter it always comes.
//
// The pawn queries that are host state elsewhere: vt+0x27F0 = pawn+0xE60 bit 20 (0x14110E770),
// one of the `TresRestrictActionVolume` restriction bits, all clear in a test level [I];
// vt+0x2540 "is dead" (state 59) = `Friend::ko` of the party seam.

/// (CC) `characterAllowed(e)`: pawn vt+0xD70 (`m_ChrUniqueID`) against the element.
fn chr_allows(element: Element, chr: u8) -> bool {
    match element {
        Element::Aero => chr == 1 || chr == 3,
        Element::Water => chr == 1 || chr == 3 || chr == 7,
        Element::Blizzard | Element::Fire => chr != 2,
        _ => true,
    }
}

/// (CC) Sora's `m_ChrUniqueID` (pawn+0x5F0, ctor 0x1413E761B).
const SORA_CHR_UID: u8 = 1;
/// (CC) Sora's pawn mode byte (pawn+0x15B4, vt+0xD80; ctor 0x1413E7622): 8 / 0x11 would offer
/// S_DARKFIRAGA 0x7E / S_CROSSSHOOT 0xE7 when nothing was rolled (0x1413EDF5F).
const SORA_PAWN_MODE: u8 = 0;
/// (CC) pawn vt+0x27F0 (pawn+0xE60 bit 20): a restriction bit, clear in a test level.
const CURAGAN_RESTRICTED: bool = false;
/// (CC) 0x1413EDFAA: the formchange chance per missing success, `(4 - min(n, 3)) * 250` of 1000.
const TAIL_STEP: i32 = 250;
const TAIL_CAP: u32 = 3;
/// (CC) 0x1413EDDC5: Curagan's chance value per dead party member; 0x1413EDD52: 50 in a pinch.
const CURAGAN_PER_KO: i32 = 10;
const CURAGAN_CHANCE_PINCH: i32 = 50;

/// (CC) One pass: the list in offer order and `n`, the successes ([rbp+40h]).
struct Pass {
    list: Vec<u8>,
    successes: u32,
}

impl Pass {
    /// "n += 1; n == 1 -> append, else insert at rand() % n" (one rand only from the 2nd on).
    fn add(&mut self, cx: &mut Ctx, cmd: u8) {
        self.successes += 1;
        if self.successes <= 1 {
            self.list.push(cmd);
        } else {
            let at = (cx.pawn.magic.next_random() % self.successes) as usize;
            self.list.insert(at.min(self.list.len()), cmd);
        }
    }
}

/// One pass of the roll (see the transcription above). `forced` = the MAGIC_ROULETTE pre-roll;
/// `skip` = ctrl+0xB7C, the command the pre-roll offered (0 = none).
fn pass(cx: &mut Ctx, m: i32, forced: bool, skip: u8) -> Pass {
    let s = Sheet::Mp;
    let denominator = s.int("grand.roll_denominator").max(1) as u32;
    let per_gan = Sheet::Items.int("menu2.grand.points_per_gan_ability");
    let mut p = Pass { list: Vec::new(), successes: 0 };
    for element in [Element::Aero, Element::Water, Element::Thunder, Element::Blizzard, Element::Fire] {
        let e = element as usize;
        let top = offer_of(element, 0);
        // (CC) the whole element is skipped (no rand) when the pre-roll offered its top command.
        if skip == top {
            continue;
        }
        let mut points = cx.pawn.magic.grand.element_points[e] as i32;
        let g = cx.pawn.status.count(gan_of(element)) as i32 + forced as i32;
        if g > 0 {
            points += per_gan * g;
            // (CC) "lastCmd[e] = the S_x-GAN id" is a store: the pre-roll leaves it for the main pass.
            cx.pawn.magic.grand.last_cmd[e] = top;
        }
        let rolled = cx.pawn.magic.next_random() % denominator;
        if (rolled as i32) < points * m && chr_allows(element, SORA_CHR_UID) {
            let cmd = offer_of(element, cx.pawn.magic.grand.last_cmd[e]);
            p.add(cx, cmd);
        }
    }
    if skip == 125 {
        return p;
    }
    let status = &cx.pawn.status;
    let mut low = false;
    if forced {
        let (hp, max) = (status.hp, status.max_hp());
        if max >= 1 && hp * 100 / max > 50 {
            return p;
        }
        if !status.is_pinch() {
            low = true;
        } else {
            if !CURAGAN_RESTRICTED {
                p.list.clear();
                p.list.push(125);
                p.successes = 1;
            }
            return p;
        }
    }
    if (status.has(ETresAbilityKind::Curagan) || low) && !CURAGAN_RESTRICTED {
        let mut v = if status.is_pinch() { CURAGAN_CHANCE_PINCH } else { CURAGAN_CHANCE };
        let mut friends = Vec::new();
        cx.host.targets.friends(&mut friends);
        v += CURAGAN_PER_KO * friends.iter().take(6).filter(|f| f.ko).count() as i32;
        let rolled = cx.pawn.magic.next_random() % denominator;
        if (rolled as i32) < v * m {
            p.add(cx, 125);
        }
    }
    p
}

/// The Curagan chance value (C 7: "chance value 25"; decompiled C, 0x1413EDD1A).
const CURAGAN_CHANCE: i32 = 25;

/// (CC) What one `RollGrandMagic(alt, 0)` did.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Rolled {
    /// Every command it offered, in the order they were offered (the pre-roll's first).
    pub commands: Vec<u8>,
    /// The return value: false = AwardAtMax also offers the formchange.
    pub result: bool,
}

/// `RollGrandMagic(alt, forced = 0)` 0x1413ED500 (I 7.2): the commands to offer, in offer
/// order. See [`roll_full`] for the return value.
pub(crate) fn roll(cx: &mut Ctx, alt: bool) -> Vec<u8> {
    roll_full(cx, alt).commands
}

/// (CC) `RollGrandMagic(alt, forced = 0)` 0x1413ED500 with its return value (the transcription
/// above): `alt` = the pawn is in MP charge (M = 20 instead of 10; x1.3 truncated with
/// GRAND_MAGIC 180). With MAGIC_ROULETTE (186) a forced pre-roll runs first: only its first
/// result is offered, its element is skipped by the main pass, which then offers at most 2 more.
pub(crate) fn roll_full(cx: &mut Ctx, alt: bool) -> Rolled {
    let s = Sheet::Mp;
    let mut m = if alt { s.int("grand.mult_alt") } else { s.int("grand.mult_normal") };
    if cx.pawn.status.has(ETresAbilityKind::GrandMagic) {
        m = (m as f32 * Sheet::Items.num("menu2.grand.ability_grand_magic_mult")) as i32;
    }
    let max = s.int("grand.max_offers").max(0) as usize;
    let mut commands: Vec<u8> = Vec::new();
    let mut skip = 0u8;
    if cx.pawn.status.count(ETresAbilityKind::MagicRoulette) > 0 {
        if let Some(&first) = pass(cx, m, true, 0).list.first() {
            skip = first;
            commands.push(first);
        }
    }
    let main = pass(cx, m, false, skip);
    let k = main.list.len().min(max.saturating_sub((skip != 0) as usize));
    commands.extend_from_slice(&main.list[..k]);
    let mut result = k > 0;
    if !result {
        let mode_offer = match SORA_PAWN_MODE {
            8 => Some(0x7E),
            0x11 => Some(0xE7),
            _ => None,
        };
        if let Some(cmd) = mode_offer {
            commands.push(cmd);
            result = true;
        }
    }
    let denominator = s.int("grand.roll_denominator").max(1) as u32;
    if cx.pawn.combat.style.points[0] as i32 > 0 {
        let threshold = (4 - main.successes.min(TAIL_CAP) as i32) * TAIL_STEP;
        let rolled = (cx.pawn.magic.next_random() % denominator) as i32;
        if threshold > rolled {
            result = false;
        }
    }
    if cx.pawn.status.count(ETresAbilityKind::CriticalConverter) > 0 {
        result = false;
    }
    cx.pawn.magic.grand.offers += commands.len() as u32;
    Rolled { commands, result }
}

/// The lifetime of a grand-magic situation command (J 5.2): `Param0` (20 s) x (1 + 0.2 x
/// MAGIC_TIME copies), at most 99.9 s; its countdown starts after the queue's 0.5 s delay.
pub fn lifetime(status: &crate::status::Status) -> f32 {
    let s = Sheet::Items;
    let n = status.count(ETresAbilityKind::MagicTime) as f32;
    (20.0 * (1.0 + s.num("menu2.situation.ability_step_magic_time") * n)).min(s.num("menu2.situation.time_cap"))
}

// (CC) RollGrandMagic 0x1413ED500: the override of CRITICAL_CONVERTER, the random tail, the
// pre-roll's store and skip, Curagan's forced / pinch / party terms, the rand draws.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::{AnimId, AnimPlayer};
    use crate::cmc::Cmc;
    use crate::host::targets::Friend;
    use crate::host::{HitRecord, HitResult, Host, HurtShape, NullHost, TargetId, TargetInfo, Targets};
    use crate::params::SORA_PARAMS;
    use crate::pawn::Pawn;
    use crate::test_world::TestWorld;
    use glam::Vec3;
    use ETresAbilityKind as A;

    /// Donald and Goofy; the first `ko` of them dead.
    struct Party {
        ko: usize,
    }

    impl Targets for Party {
        fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
        fn info(&self, _id: TargetId) -> Option<TargetInfo> {
            None
        }
        fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
        fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
            HitResult::default()
        }
        fn friends(&self, out: &mut Vec<Friend>) {
            for i in 0..2 {
                out.push(Friend { uid: 11 + i as u8, ko: i < self.ko, ..Friend::default() });
            }
        }
    }

    fn pawn() -> Pawn {
        let mut p = Pawn::new(SORA_PARAMS, Cmc::new(&SORA_PARAMS, Vec3::ZERO), 0.0, AnimPlayer::new(AnimId::IdleN));
        p.status.placeholder = false;
        p.status.hp_base = 100;
        p.status.hp = 100;
        p
    }

    fn roll_with(pawn: &mut Pawn, ko: usize, alt: bool) -> Rolled {
        let world = TestWorld::floor();
        let mut null = NullHost::default();
        let mut party = Party { ko };
        let mut host = Host { world: &world, targets: &mut party, audio: &mut null.audio, effects: &mut null.effects, ui: &mut null.ui };
        let mut cx = Ctx::new(pawn, &mut host);
        roll_full(&mut cx, alt)
    }

    /// The share of `trials` rolls whose `pick` is true.
    fn share(pawn: &mut Pawn, trials: u32, ko: usize, pick: impl Fn(&Rolled) -> bool) -> f32 {
        (0..trials).filter(|_| pick(&roll_with(pawn, ko, false))).count() as f32 / trials as f32
    }

    fn near(got: f32, want: f32) -> bool {
        (got - want).abs() < 0.04
    }

    /// `PawnMagic::next_random`'s state step (xorshift32).
    fn xs(mut x: u32) -> u32 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    }

    const FIRE: usize = Element::Fire as usize;
    const WATER: usize = Element::Water as usize;

    #[test]
    fn critical_converter_always_leaves_the_formchange() {
        let mut p = pawn();
        // A certain Fire offer (100 points x M 10 = 1000) and physical points.
        p.magic.grand.element_points[FIRE] = 100.0;
        p.combat.style.points[0] = 30.0;
        let free = share(&mut p, 2000, 0, |r| !r.result);
        assert!(near(free, 0.75), "one success: (4 - 1) x 250 / 1000 = 75 % formchange, got {free}");
        p.status.abilities.set(A::CriticalConverter, 1, 1);
        for _ in 0..200 {
            let r = roll_with(&mut p, 0, false);
            assert_eq!(r, Rolled { commands: vec![112], result: false }, "the grand magic still comes, and the formchange with it");
        }
        // Critical Converter without physical points: still false (0x1413EDFED is unconditional).
        p.combat.style.points[0] = 0.0;
        assert!(!roll_with(&mut p, 0, false).result);
    }

    #[test]
    fn the_tail_needs_physical_points_and_scales_with_the_successes() {
        let mut p = pawn();
        p.magic.grand.element_points[FIRE] = 100.0;
        // No physical points: the result stands, and the draws are the five element rolls only
        // (the first success is appended without a rand).
        let before = p.magic.rng;
        let r = roll_with(&mut p, 0, false);
        assert_eq!(r, Rolled { commands: vec![112], result: true });
        assert_eq!(p.magic.rng, (0..5).fold(before, |x, _| xs(x)));
        // Two successes: one insert draw more.
        p.magic.grand.element_points[WATER] = 100.0;
        let before = p.magic.rng;
        let r = roll_with(&mut p, 0, false);
        assert!(r.result && r.commands.len() == 2 && r.commands.contains(&112) && r.commands.contains(&121));
        assert_eq!(p.magic.rng, (0..6).fold(before, |x, _| xs(x)));
        // With physical points: 2 successes 50 %, 3 -> 25 %, 5 -> 25 % (min(n, 3)); 3 offered at most.
        p.combat.style.points[0] = 1.0;
        let two = share(&mut p, 2000, 0, |r| !r.result);
        assert!(near(two, 0.5), "{two}");
        p.magic.grand.element_points[Element::Thunder as usize] = 100.0;
        let three = share(&mut p, 2000, 0, |r| !r.result);
        assert!(near(three, 0.25), "{three}");
        for e in [Element::Blizzard, Element::Aero] {
            p.magic.grand.element_points[e as usize] = 100.0;
        }
        let five = share(&mut p, 2000, 0, |r| !r.result && r.commands.len() == 3);
        assert!(near(five, 0.25), "{five}");
        // Less than one physical point is int 0: no tail.
        p.combat.style.points[0] = 0.9;
        assert!(roll_with(&mut p, 0, false).result);
    }

    #[test]
    fn the_magic_roulette_pre_roll_stores_the_top_tier_and_its_offer_does_not_count() {
        let mut p = pawn();
        p.status.abilities.set(A::MagicRoulette, 1, 1);
        p.magic.grand.element_points[FIRE] = 100.0;
        let (mut alone, mut pair) = (0, 0);
        for _ in 0..400 {
            // Fira was the highest Fire cast: the plain roll would offer S_FIRAGA (111).
            p.magic.grand.last_cmd = [0; 8];
            p.magic.grand.last_cmd[FIRE] = Element::Fire.command(2);
            let r = roll_with(&mut p, 0, false);
            // The pre-roll counted one -GAN copy for every element: lastCmd = the top command.
            for e in [Element::Aero, Element::Water, Element::Thunder, Element::Blizzard, Element::Fire] {
                assert_eq!(p.magic.grand.last_cmd[e as usize], offer_of(e, 0));
            }
            if r.commands == [112] {
                // The pre-roll took Fire: the main pass skips Fire and offers nothing, so the
                // result is false and the formchange comes too.
                assert!(!r.result);
                alone += 1;
            } else {
                // Another element came first; the main pass offers Fire at its stored top tier.
                assert_eq!(r.commands.len(), 2, "{:?}", r.commands);
                assert!([124, 121, 118, 115].contains(&r.commands[0]) && r.commands[1] == 112 && r.result, "{r:?}");
                pair += 1;
            }
        }
        assert!(alone > 0 && pair > 0);
    }

    #[test]
    fn curagan_forced_in_a_pinch_replaces_the_pre_roll() {
        let mut p = pawn();
        p.status.abilities.set(A::MagicRoulette, 1, 1);
        p.magic.grand.element_points[FIRE] = 100.0;
        p.status.hp = 20; // IsPinch: 20 <= 100 x 25 / 100
        for _ in 0..50 {
            // The forced list is cleared and holds Curagan alone; the main pass skips Curagan.
            assert_eq!(roll_with(&mut p, 0, false), Rolled { commands: vec![125, 112], result: true });
        }
    }

    #[test]
    fn curagan_chance_and_its_gates() {
        let mut p = pawn();
        let has = |r: &Rolled| r.commands.contains(&125);
        // Without the ability nothing (main pass); full HP: the forced pre-roll offers no Curagan.
        assert_eq!(share(&mut p, 500, 0, has), 0.0);
        p.status.abilities.set(A::MagicRoulette, 1, 1);
        assert_eq!(share(&mut p, 500, 0, has), 0.0);
        // At 40 % HP (low, no pinch) the pre-roll rolls it without the ability: 25 x 10.
        p.status.hp = 40;
        let low = share(&mut p, 2000, 0, |r| r.commands.first() == Some(&125));
        assert!(low > 0.1, "{low}");
        p.status.abilities.set(A::MagicRoulette, 0, 0);
        p.status.hp = 100;
        // With CURAGAN: 25 x M 10 = 25 %; +10 per dead party member; 50 in a pinch; M 20 in MP charge.
        p.status.abilities.set(A::Curagan, 1, 1);
        let base = share(&mut p, 2000, 0, has);
        assert!(near(base, 0.25), "{base}");
        let one_ko = share(&mut p, 2000, 1, has);
        assert!(near(one_ko, 0.35), "{one_ko}");
        p.status.hp = 20;
        let pinch_two_ko = share(&mut p, 2000, 2, has);
        assert!(near(pinch_two_ko, 0.70), "{pinch_two_ko}");
        p.status.hp = 100;
        let charge = (0..2000).filter(|_| has(&roll_with(&mut p, 0, true))).count() as f32 / 2000.0;
        assert!(near(charge, 0.5), "{charge}");
    }

    #[test]
    fn element_character_gates() {
        // m_ChrUniqueID 1 (Sora) passes every element; 2 has no fire / blizzard, 7 water but no aero.
        for e in [Element::Aero, Element::Water, Element::Thunder, Element::Blizzard, Element::Fire] {
            assert!(chr_allows(e, SORA_CHR_UID));
        }
        assert!(!chr_allows(Element::Fire, 2) && !chr_allows(Element::Blizzard, 2) && chr_allows(Element::Thunder, 2) && !chr_allows(Element::Aero, 2));
        assert!(chr_allows(Element::Water, 7) && !chr_allows(Element::Aero, 7));
    }
}

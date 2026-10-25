//! (DON-MAGIC) THE ELDEN RING SIDE OF DONALD'S STATUSES (documentation/DONALD_OVERHAUL.md 4.2, 4.5, 6.1; DON_PLAN R12).
//! UNTESTED IN GAME (no Elden Ring launches by agents since 2026-10-17): every pure rule below is unit-tested offline.
//!
//! kh-party keeps the enemies' statuses (`Party::don`, party-wide). Once a frame, right after the party's step
//! (party/mod.rs `after_sora_step`, marked), [`publish`] copies what the game side needs into a small snapshot and
//! [`apply_game`] applies it to the Elden Ring characters:
//! - EXPOSED for Sora's own hits (4.5 "+25 % damage from everyone in the party"): `ErTargets::apply_hit` (targets.rs,
//!   marked) multiplies a hit without kh-party's `DON_HIT_FLAG` by [`exposed_mult`] (the party's own hits carry it
//!   already: kh-party `magic_fx::PartyFx`, and Sora's own pass kh-partyhost `SoraTargets`, which runs the reaction rule,
//!   Exposed and the team bonus and flags them: this is the safety net for any other hit);
//! - FROZEN / CHILL / THE LARGE-ENEMY SLOW (6.1, R12) and DON-PROG's BLIZZARD AURA (4.2: -15 % near Donald, `aura.enemy_speed`):
//!   the character's animation speed (`CSChrBehaviorModule::
//!   animation_speed`, chranim.rs `set_speed`, the lab's `dbg.speed` route): Frozen holds it at [`FROZEN_SPEED`] (0),
//!   Chill / the slow at 1 - the slow share; it is given back (1.0) when the status ends or the character leaves the
//!   list. The KH3 hold of a small / medium body comes with the hit itself (FREEZE bad status -> kh-react's Hold, the
//!   puppet, react.chain's limits);
//! - BLIND (the user, 2026-10-22: "blind should make enemies attack the position they last saw you before they were
//!   blinded"; it replaces DON_PLAN's R20): when an enemy becomes Blind, the point its AI aims at that moment
//!   (`ChrIns::lock_on_target_position`: its current target's position as the game keeps it) is recorded
//!   ([`blind_records`]); until Blind ends that field is written back to the recorded point every frame, so it attacks
//!   the empty spot; when Blind ends the writes stop and the game refreshes the field from its real target. Whoever
//!   stands in that spot is still hit; nothing is ever redirected onto someone else. UNTESTED IN GAME: that the AI's
//!   swings and its approach follow the written point (the field is the SDK's documented lock-on target position);
//! - AERO BARRIER and SANCTUARY on Sora ([`sora_damage_mult`], [`sora_invincible`]): Sora's own incoming hits
//!   (sora.rs `damage_in`, marked); a friend leader's and the allies' go through `Party::receive_hit`, which applies them;
//! - LAUNCHES go through the existing juggle route: a launch is the hit's own damage kind (PARABOLA), which the puppet
//!   plays with react.chain's limits; Burn ticks and Conduct / Wildfire are kh-party hits like any other;
//! - GLACIER'S WALLS (6.4: the ice wall blocks projectiles): an Elden Ring bullet cannot be stopped in flight safely (no
//!   tested way to find and end a game bullet), so the hit INTAKE does it, the route the fall guard uses: a hit on Sora
//!   (sora.rs `damage_in`) or on an ally's body (party/allybody_live.rs `deliver`) whose attacker's line passes through a
//!   wall ([`wall_shields`]; the attacker unknown, as a bullet often is: when every hostile's line does) is not taken: no
//!   damage, no reaction, nothing redirected. A melee swing through the ice is stopped the same way. UNTESTED IN GAME.
//! - GRAVITY'S FLATTEN (the user, 2026-10-22; never a boss): a character kh-party flattened (`Party::don_squashes`: width
//!   and height shares) has its MODEL scaled every frame: `ChrCtrl::scale_size_{x,y,z}` (the SDK: the scales the game
//!   builds the character's model matrix from; Elden Ring is Y-up: y = height), its own values saved the first frame and
//!   put back the moment it is no longer flat: restored, dead, gone from the list, a boss (a boss bar), the party off
//!   ([`clear`]) or Sora stopping (sora.rs, [`restore_flattened`]). A character the game has dropped (a load, an area
//!   change, a despawn) is simply forgotten: its object is gone. Collision is untouched. UNTESTED IN GAME.

use std::sync::Mutex;

use kh_core::TargetId;

/// One enemy's statuses as the game side needs them.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Snap {
    pub id: u32,
    /// x on everyone's damage (Exposed), 1 = none.
    pub exposed: f32,
    /// The animation speed it should run at (1 = normal).
    pub speed: f32,
    pub blind: bool,
}

/// The frame's snapshot.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Book {
    pub enemies: Vec<Snap>,
    /// Sora's barrier share taken (1 = none) and Sanctuary's invincibility.
    pub sora_mult: f32,
    pub sora_invincible: bool,
    /// DON-PROG's Blizzard attunement aura (doc 4.2: enemies near Donald -15 %): (Donald's place in KH3 space, reach cm,
    /// the speed share inside it).
    pub aura: Vec<([f32; 3], f32, f32)>,
    /// (DON-MAGIC glacier) Glacier's walls standing (KH3 space).
    #[cfg(feature = "party")]
    pub walls: Vec<kh_party::donald::magic_fx::Wall>,
    /// (DON-MAGIC gravity) Gravity's flattened enemies: (target, width share, height share).
    pub squash: Vec<(u32, f32, f32)>,
}

static BOOK: Mutex<Option<Book>> = Mutex::new(None);
/// kh-party's `magic_fx::DON_HIT_FLAG` (kept here so the hit route needs no party build).
pub const DON_HIT_FLAG: u32 = 0x4000_0000;

/// The Frozen speed (it cannot act).
pub const FROZEN_SPEED: f32 = 0.0;

/// The snapshot of a party's statuses (pure: the tests build one).
#[cfg(feature = "party")]
pub fn book_of(party: &kh_party::Party) -> Book {
    use kh_party::donald::{allyfx, cast::SORA_MEMBER};
    let d = kh_party::donald::data();
    let t = &d.tune;
    let enemies = party
        .don
        .status
        .entries
        .iter()
        .map(|(id, s)| {
            let speed = if s.is_frozen() { FROZEN_SPEED } else { 1.0 - s.slow(t) };
            Snap { id: *id, exposed: s.damage_taken_mult(t), speed, blind: s.is_blind() }
        })
        .collect();
    let aura = party.don_prog.aura.sources.iter().filter(|s| s.aura.enemy_speed_mult < 1.0).map(|s| (s.pos.to_array(), s.aura.radius, s.aura.enemy_speed_mult)).collect();
    Book { enemies, sora_mult: allyfx::damage_mult(party, SORA_MEMBER), sora_invincible: allyfx::invincible(party, SORA_MEMBER), aura, walls: party.don_walls().to_vec(), squash: party.don_squashes() }
}

/// ONCE A FRAME after the party's step: the snapshot the hit routes read.
#[cfg(feature = "party")]
pub fn publish(party: &kh_party::Party) {
    *BOOK.lock().unwrap_or_else(|e| e.into_inner()) = Some(book_of(party));
}

/// The party left (or the feature is off): nothing applies.
pub fn clear() {
    *BOOK.lock().unwrap_or_else(|e| e.into_inner()) = None;
    restore_flattened("the party is off"); // (DON-MAGIC gravity)
}

// ---------------------------------------------------------------------------------------------- Gravity's flatten

/// The model scale of a flattened character (pure): its own (x, y, z) with the width share on x / z and the height share
/// on y (Elden Ring is Y-up).
pub fn flat_scale(own: [f32; 3], wide: f32, tall: f32) -> [f32; 3] {
    [own[0] * wide, own[1] * tall, own[2] * wide]
}

/// THE FLATTEN'S BOOKKEEPING (pure): `held` = the characters flattened last frame (target id); `want` = what kh-party
/// flattens now; `may` = the character is here, alive and no boss. Returns (to scale now, to put back now).
pub fn flatten_plan(held: &[u32], want: &[(u32, f32, f32)], may: impl Fn(u32) -> bool) -> (Vec<(u32, f32, f32)>, Vec<u32>) {
    let now: Vec<(u32, f32, f32)> = want.iter().copied().filter(|w| may(w.0)).collect();
    let back = held.iter().copied().filter(|h| !now.iter().any(|n| n.0 == *h)).collect();
    (now, back)
}

/// (game side) The characters flattened now: (target, handle bits, their own scale).
#[cfg(windows)]
static FLAT: Mutex<Vec<(u32, eldenring::cs::FieldInsHandle, [f32; 3])>> = Mutex::new(Vec::new());

#[cfg(windows)]
fn apply_squash(targets: &crate::targets::ErTargets) {
    let want = with(|b| b.squash.clone()).unwrap_or_default();
    let bosses = crate::erhud::boss_bar_handles();
    let mut flat = FLAT.lock().unwrap_or_else(|e| e.into_inner());
    let held: Vec<u32> = flat.iter().map(|f| f.0).collect();
    // (ZETTA) a target Zettaflare holds is not flattened while it is held (its own scale back; flattened again after)
    let zetta = |e: &crate::targets::Enemy| targets.puppets.zetta_watch.iter().any(|w| w.handle == e.handle);
    let (now, back) = flatten_plan(&held, &want, |id| targets.list.iter().any(|e| e.id.0 == id && e.alive && !bosses.contains(&e.handle) && !zetta(e)));
    for id in back {
        if let Some(k) = flat.iter().position(|f| f.0 == id) {
            let (_, handle, own) = flat.remove(k);
            if let Some(chr) = crate::targets::find_chr(&handle) {
                let ctrl = chr.chr_ctrl.as_mut();
                (ctrl.scale_size_x, ctrl.scale_size_y, ctrl.scale_size_z) = (own[0], own[1], own[2]);
            }
        }
    }
    for (id, wide, tall) in now {
        let Some(e) = targets.list.iter().find(|e| e.id.0 == id) else { continue };
        let Some(chr) = crate::targets::find_chr(&e.handle) else { continue };
        let ctrl = chr.chr_ctrl.as_mut();
        let own = match flat.iter().find(|f| f.0 == id) {
            Some(f) => f.2,
            None => {
                let own = [ctrl.scale_size_x, ctrl.scale_size_y, ctrl.scale_size_z];
                // a scale that is no number or no size is not trusted: that character is left alone
                if own.iter().any(|v| !v.is_finite() || *v <= 0.0 || *v > 100.0) {
                    continue;
                }
                flat.push((id, e.handle, own));
                own
            }
        };
        let s = flat_scale(own, wide, tall);
        (ctrl.scale_size_x, ctrl.scale_size_y, ctrl.scale_size_z) = (s[0], s[1], s[2]);
    }
}

/// Every flattened character back to its own scale (Sora stopping, the party off).
pub fn restore_flattened(why: &str) {
    #[cfg(windows)]
    {
        let mut flat = FLAT.lock().unwrap_or_else(|e| e.into_inner());
        if flat.is_empty() {
            return;
        }
        for (_, handle, own) in flat.drain(..) {
            if let Some(chr) = crate::targets::find_chr(&handle) {
                let ctrl = chr.chr_ctrl.as_mut();
                (ctrl.scale_size_x, ctrl.scale_size_y, ctrl.scale_size_z) = (own[0], own[1], own[2]);
            }
        }
        crate::erlog!("gravity: every flattened character put back to its own size ({why})");
    }
    #[cfg(not(windows))]
    let _ = why;
}

fn with<R>(f: impl FnOnce(&mut Book) -> R) -> Option<R> {
    let mut g = BOOK.lock().unwrap_or_else(|e| e.into_inner());
    g.as_mut().map(f)
}

/// x on a hit's damage for Exposed (1 for kh-party's own hits, which carry it: `flags & DON_HIT_FLAG`).
pub fn exposed_mult(target: TargetId, flags: u32) -> f32 {
    if flags & DON_HIT_FLAG != 0 {
        return 1.0;
    }
    with(|b| b.enemies.iter().find(|e| e.id == target.0).map_or(1.0, |e| e.exposed)).unwrap_or(1.0)
}

/// The enemy is Blind.
pub fn blind(target: TargetId) -> bool {
    with(|b| b.enemies.iter().any(|e| e.id == target.0 && e.blind)).unwrap_or(false)
}

/// THE BLIND AIM (pure): the recorded points of the Blind enemies after this frame. `prev` = last frame's records,
/// `blind` = the enemies Blind now, `aim_now` = where an enemy's AI aims right now (read before this frame's write): a
/// newly Blind enemy records its aim; a still-Blind one keeps its record; one no longer Blind is dropped.
pub fn blind_records(prev: &[(u32, [f32; 4])], blind: &[u32], aim_now: impl Fn(u32) -> Option<[f32; 4]>) -> Vec<(u32, [f32; 4])> {
    blind.iter().filter_map(|id| prev.iter().find(|p| p.0 == *id).map(|p| p.1).or_else(|| aim_now(*id)).map(|a| (*id, a))).collect()
}

/// (DON-MAGIC glacier) Where the hostiles stood at the last [`apply_game`] (feet, KH3 space): the bodies' hit intake
/// reads them for a hit whose attacker is unknown.
static HOSTILES: Mutex<Vec<glam::Vec3>> = Mutex::new(Vec::new());

/// The hostiles' feet as last recorded.
pub fn hostiles() -> Vec<glam::Vec3> {
    HOSTILES.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Records the hostiles' feet (`apply_game`; pure callers: tests).
pub fn set_hostiles(list: Vec<glam::Vec3>) {
    *HOSTILES.lock().unwrap_or_else(|e| e.into_inner()) = list;
}

/// GLACIER'S WALLS SHIELD A PARTY MEMBER (the hit intake; see the module text): a hit on the member at `victim` (feet)
/// from `attacker` (feet; None = unknown: every one of `hostiles` must be walled off) is taken by the ice.
pub fn wall_shields(victim: glam::Vec3, attacker: Option<glam::Vec3>, hostiles: &[glam::Vec3]) -> bool {
    // (ZETTA 2026-10-23) without `party` no wall stands
    #[cfg(feature = "party")]
    return with(|b| kh_party::donald::magic_fx::wall_shields(&b.walls, victim, attacker, hostiles)).unwrap_or(false);
    #[cfg(not(feature = "party"))]
    {
        let _ = (victim, attacker, hostiles);
        false
    }
}

/// The share of an incoming hit Sora takes (Aero Barrier on him).
pub fn sora_damage_mult() -> f32 {
    with(|b| b.sora_mult).unwrap_or(1.0)
}

/// Sanctuary's short invincibility is on Sora.
pub fn sora_invincible() -> bool {
    with(|b| b.sora_invincible).unwrap_or(false)
}

/// The animation speed an enemy at `pos` (KH3 space) should run at: its statuses' (Frozen / Chill / the slow) times the
/// Blizzard aura's when it stands within one (`None`: normal speed).
pub fn speed_of(target: TargetId, pos: glam::Vec3) -> Option<f32> {
    with(|b| speed_in(b, target, pos)).flatten()
}

/// The rule of [`speed_of`] on a book (pure).
pub fn speed_in(b: &Book, target: TargetId, pos: glam::Vec3) -> Option<f32> {
    let own = b.enemies.iter().find(|e| e.id == target.0).map_or(1.0, |e| e.speed);
    let aura = b.aura.iter().filter(|a| glam::Vec3::from_array(a.0).distance(pos) <= a.1).map(|a| a.2).fold(1.0, f32::min);
    let s = own * aura;
    ((s - 1.0).abs() > 1e-4).then_some(s)
}

/// ONCE A FRAME after [`publish`] (game thread): the slowed / Frozen characters' animation speed and the Blind enemies'
/// aim (see the module text).
#[cfg(windows)]
pub fn apply_game(targets: &crate::targets::ErTargets) {
    set_hostiles(targets.list.iter().filter(|e| e.hostile && e.alive && e.active).map(|e| e.pos).collect()); // (DON-MAGIC glacier)
    apply_speeds(targets);
    apply_blind(targets);
    apply_squash(targets); // (DON-MAGIC gravity)
}

#[cfg(windows)]
fn apply_speeds(targets: &crate::targets::ErTargets) {
    static SLOWED: Mutex<Vec<(u32, eldenring::cs::FieldInsHandle)>> = Mutex::new(Vec::new());
    let mut slowed = SLOWED.lock().unwrap_or_else(|e| e.into_inner());
    let mut now = Vec::new();
    for e in targets.list.iter().filter(|e| e.alive) {
        // (ZETTA) Zettaflare's held target keeps its animation running (its slow / Frozen speed waits for the hold's end)
        if targets.puppets.zetta_watch.iter().any(|w| w.handle == e.handle) {
            continue;
        }
        if let Some(s) = speed_of(e.id, e.pos) {
            if let Some(chr) = crate::targets::find_chr(&e.handle) {
                crate::chranim::set_speed(chr, s);
                now.push((e.id.0, e.handle));
            }
        }
    }
    for (id, handle) in slowed.iter() {
        if !now.iter().any(|n| n.0 == *id) {
            if let Some(chr) = crate::targets::find_chr(handle) {
                crate::chranim::set_speed(chr, 1.0);
            }
        }
    }
    *slowed = now;
}

#[cfg(windows)]
fn apply_blind(targets: &crate::targets::ErTargets) {
    static AIMS: Mutex<Vec<(u32, [f32; 4])>> = Mutex::new(Vec::new());
    let mut aims = AIMS.lock().unwrap_or_else(|e| e.into_inner());
    let blind_ids: Vec<u32> = targets.list.iter().filter(|e| e.alive && blind(e.id)).map(|e| e.id.0).collect();
    let read = |id: u32| {
        let e = targets.list.iter().find(|e| e.id.0 == id)?;
        let chr = crate::targets::find_chr(&e.handle)?;
        let v = chr.lock_on_target_position;
        Some([v.0, v.1, v.2, v.3])
    };
    let next = blind_records(&aims, &blind_ids, read);
    for (id, at) in &next {
        if !aims.iter().any(|a| a.0 == *id) {
            crate::erlog!("donald: target {id} is BLIND: it attacks where it last saw its target ({:.2}, {:.2}, {:.2}) until it sees again", at[0], at[1], at[2]);
        }
        if let Some(e) = targets.list.iter().find(|e| e.id.0 == *id) {
            if let Some(chr) = crate::targets::find_chr(&e.handle) {
                chr.lock_on_target_position.0 = at[0];
                chr.lock_on_target_position.1 = at[1];
                chr.lock_on_target_position.2 = at[2];
                chr.lock_on_target_position.3 = at[3];
            }
        }
    }
    for (id, _) in aims.iter().filter(|a| !next.iter().any(|n| n.0 == a.0)) {
        crate::erlog!("donald: target {id} sees again: its own aim comes back");
    }
    *aims = next;
}

#[cfg(all(test, feature = "party"))]
mod tests {
    use super::*;
    use kh_party::donald::status::Size;

    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn party() -> kh_party::Party {
        kh_party::Party::new(kh_party::PartyConfig::test_level99())
    }

    #[test]
    fn gravitys_flatten_scales_the_model_and_puts_back_whoever_is_no_longer_flat() {
        // the model: y is Elden Ring's height
        assert_eq!(flat_scale([1.0, 1.0, 1.0], 1.2, 0.35), [1.2, 0.35, 1.2]);
        assert_eq!(flat_scale([2.0, 2.0, 2.0], 1.2, 0.5), [2.4, 1.0, 2.4]);
        // 1 and 2 flattened; 3 is a boss (never), 4 died: put back
        let here = |id: u32| id != 3 && id != 4;
        let (now, back) = flatten_plan(&[1, 4], &[(1, 1.2, 0.35), (2, 1.2, 0.35), (3, 1.2, 0.35), (4, 1.2, 0.35)], here);
        assert_eq!(now.iter().map(|n| n.0).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(back, vec![4], "the dead one is put back at once");
        // kh-party restored 1 (no longer listed): put back; nothing wanted: everything held goes back
        let (now, back) = flatten_plan(&[1, 2], &[(2, 1.1, 0.6)], here);
        assert_eq!((now.len(), back), (1, vec![1]));
        let (now, back) = flatten_plan(&[2], &[], here);
        assert!(now.is_empty() && back == vec![2], "never left flat");
    }

    #[test]
    fn glaciers_wall_shields_who_stands_behind_it_from_a_shot() {
        use kh_party::donald::magic_fx::Wall;
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let wall = Wall { a: glam::Vec3::new(500.0, -400.0, 0.0), b: glam::Vec3::new(500.0, 400.0, 0.0), half_width: 60.0, height: 300.0, left: 5.0 };
        *BOOK.lock().unwrap_or_else(|e| e.into_inner()) = Some(Book { walls: vec![wall], ..Book::default() });
        let victim = glam::Vec3::ZERO;
        let behind = glam::Vec3::new(1500.0, 0.0, 0.0);
        let beside = glam::Vec3::new(0.0, 1500.0, 0.0);
        // the attacker known: its line through the ice -> no hit; a clear line -> the hit lands
        assert!(wall_shields(victim, Some(behind), &[]));
        assert!(!wall_shields(victim, Some(beside), &[]));
        // unknown (a bullet): shielded only when every hostile is behind the ice
        assert!(wall_shields(victim, None, &[behind, glam::Vec3::new(1200.0, 100.0, 0.0)]));
        assert!(!wall_shields(victim, None, &[behind, beside]));
        assert!(!wall_shields(victim, None, &[]), "nobody known around: the hit lands");
        // no wall: nothing is shielded
        *BOOK.lock().unwrap_or_else(|e| e.into_inner()) = Some(Book::default());
        assert!(!wall_shields(victim, Some(behind), &[]));
        clear();
        assert!(!wall_shields(victim, Some(behind), &[]));
    }

    #[test]
    fn the_snapshot_carries_exposed_slow_frozen_and_blind() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut p = party();
        {
            let s = p.don.status.entry(1, Size::Small);
            s.reveal = 5.0;
            s.exposed = true;
        }
        p.don.status.entry(2, Size::Small).frozen = 2.0;
        {
            let s = p.don.status.entry(3, Size::Small);
            s.chill = 2;
            s.chill_left = 3.0;
        }
        p.don.status.entry(4, Size::Boss).big_slow = 4.0;
        p.don.status.entry(5, Size::Small).blind = 3.0;
        let b = book_of(&p);
        let get = |id: u32| *b.enemies.iter().find(|e| e.id == id).unwrap();
        assert!((get(1).exposed - 1.25).abs() < 1e-5);
        assert_eq!(get(2).speed, FROZEN_SPEED);
        assert!((get(3).speed - 0.88).abs() < 1e-4, "2 Chill stacks: -12 %");
        assert!((get(4).speed - 0.7).abs() < 1e-4, "a boss: -30 %");
        assert!(get(5).blind);
        publish(&p);
        assert!((exposed_mult(TargetId(1), 0) - 1.25).abs() < 1e-5, "Sora's hit on an Exposed enemy");
        assert_eq!(exposed_mult(TargetId(1), DON_HIT_FLAG), 1.0, "a party hit already carries it");
        assert_eq!(exposed_mult(TargetId(9), 0), 1.0);
        assert!(blind(TargetId(5)) && !blind(TargetId(1)));
        assert_eq!(speed_of(TargetId(1), glam::Vec3::ZERO), None, "no speed status");
        assert!(speed_of(TargetId(2), glam::Vec3::ZERO).is_some_and(|s| s == 0.0));
        // the Blizzard aura: -15 % within its reach, on top of a Chill
        let mut b2 = b.clone();
        b2.aura.push(([0.0, 0.0, 0.0], 800.0, 0.85));
        assert!((speed_in(&b2, TargetId(9), glam::Vec3::new(500.0, 0.0, 0.0)).unwrap() - 0.85).abs() < 1e-4, "an enemy without a status, near Donald");
        assert!((speed_in(&b2, TargetId(3), glam::Vec3::new(500.0, 0.0, 0.0)).unwrap() - 0.88 * 0.85).abs() < 1e-4);
        assert_eq!(speed_in(&b2, TargetId(9), glam::Vec3::new(1500.0, 0.0, 0.0)), None, "out of its reach");
        assert_eq!(kh_party::donald::magic_fx::DON_HIT_FLAG, DON_HIT_FLAG, "the same bit as kh-party's");
        clear();
        assert_eq!(sora_damage_mult(), 1.0);
    }

    #[test]
    fn a_blind_enemy_keeps_the_aim_it_had_when_blinded_and_lets_go_when_it_sees() {
        let aim_now = |id: u32| Some([id as f32, 0.0, 0.0, 1.0]);
        // blinded now: its aim of this frame is recorded
        let r = blind_records(&[], &[5], aim_now);
        assert_eq!(r, vec![(5, [5.0, 0.0, 0.0, 1.0])]);
        // later frames keep the recorded point, whatever it would aim at now
        let r2 = blind_records(&r, &[5], |_| Some([99.0, 9.0, 9.0, 1.0]));
        assert_eq!(r2, r, "the spot it last saw its target");
        // Blind ends: dropped (the game's own aim comes back)
        assert!(blind_records(&r2, &[], aim_now).is_empty());
        // a character the game no longer has is not recorded
        assert!(blind_records(&[], &[7], |_| None).is_empty());
    }
}

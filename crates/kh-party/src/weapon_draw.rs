//! (PARTY-WEAPONS) THE FRIENDS' WEAPON IS OUT AS KH3 HAS IT: hidden out of combat, drawn with its appear effect when the
//! battle starts or the friend attacks, casts, guards or joins a Link / team attack, put away with the same effect out of
//! battle after KH3's delay. Every member of the party (an AI friend or the friend who leads) runs this one rule after
//! his step ([`step`], from `Party::step`); the parts every KH character shares (the hero light's hold, the no-combat zone, the
//! appear / disappear sequence) are kh_core::weapon_gate's.
//!
//! THE USER (2026-11-05): "Donald and Goofy's weapons should behave the same was as the keyblades, appearing when in
//! combat or when attacking with their effect"; "will this apply to all of their weapons?" - yes: [`WEAPONS`] is every
//! staff and shield the equipment tables have (14 + 14, `TresEquipItemData` I03021.. / I03041..).
//!
//! KH3's rule (`ATresNpcPawnBase`, every friend; evidence in build/sora_tmp/weapons/PARTY-WEAPONS/STATE.md):
//! - `Draw(1, 1, 1.0)` (pawn vt+0x1340, the NPC override 0x14127A680 -> 0x1405BA440 -> the equipment's `SetVisible`):
//!   at the battle start (slot 1233 0x141274AA0, before `BattleStart`), in battle at the idle's Begin (0x141242140) and
//!   in the land locomotion (0x141219F60), and at the Begin of every attack definition (`m_bWeaponVisible`, true from
//!   0x14121BBF0; none of Donald's 10 or Goofy's 8 definitions change it): 0x1412202E0 base / 0x141220600 jump /
//!   0x141220930 magic.
//! - `Draw(1, 0, 1.0)` (no effect) in the Damage state (0x1412444C0).
//! - `Draw(0, 1, 1.0)` out of battle: the idle's count (0x14124CD40: after two idle loops `IdleBreakW` plays, and when it
//!   ends the weapon is put away) or the run (0x141219F60: battle flag and battle pose off, the weapon out, after
//!   `RunBattleEnd` the loop clip past 80 % of its length). The battle end itself (slot 1234 0x1412749B0, `BattleEnd`)
//!   keeps the weapon out.
//! - The sequence: `w_do_WepBase` / `w_gu_WepBase` `m_AppearEffect` = `m_DisappearEffect` = `vw_don_app00` /
//!   `vw_gof_app00` (attach None: the mesh's origin, the hand socket), no visible delay and no scale (the constructor
//!   0x141777570 zeroes +0x5B8 / +0x5E8 and clears the scale bits; no blueprint sets them), the dither 0.10 s each way
//!   (every `w_do0x0_Wep` / `w_gu0x0_Wep` sets `m_AppearDitherAnimTime` = `m_DisappearDitherAnimTime` = 0.1). The two
//!   effects carry no sound (`ParticleSeAssets` = [None]): KH3 plays none for the friends' weapons.
//!
//! THE CLIPS (the coordinator, 2026-11-05: KH3 puts the weapon away during the friends' own clips): this rule plays them on
//! the member's anim slot from his AnimSet rows ([`PutAway`]): standing after a battle `BattleEnd` (slot 1234), then two
//! idle loops, then `IdleBreakW`, the weapon put away when it ends (the idle tick's `Draw(0, 1, 1.0)` at the next idle end);
//! moving after a battle `RunBattleEnd` (the Land locomotion's battle-pose end; the speed-matched loop yields to it,
//! [`holds_clip`]), then the loop clip past 80 % of its length. Out of battle with the weapon out and no battle pose (an
//! attack's draw) the same counts run without the battle-end clips.

use glam::Vec3;
use kh_core::host::{Attach, EffectId, EffectSpawn, PartyMember};
use kh_core::nocombat::{Act, Fighter};
use kh_core::weapon_gate::{self, Look, Sequence};

use crate::pad::{donald as pd, goofy as pg, goofy_combo as pgc};
use crate::{tables, Chara, NpcStateId};

/// Donald's staves: `w_do_WepBase` (`vw_don_app00`) with each staff's own dither 0.1.
pub const DONALD_LOOK: Look = Look { effect: "vw_don_app00", delay: 0.0, dither: 0.1, hide_delay: 0.0 };
/// Goofy's shields: `w_gu_WepBase` (`vw_gof_app00`) with each shield's own dither 0.1.
pub const GOOFY_LOOK: Look = Look { effect: "vw_gof_app00", delay: 0.0, dither: 0.1, hide_delay: 0.0 };

/// One equippable weapon: its item key (`TresEquipItemData`), its model, the socket it rides on
/// (`TresEquipWeaponData.AttachBoneName` of `<model>_Equip`) and its appear / disappear look (`<model>_Wep`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponRow {
    pub chara: Chara,
    pub key: &'static str,
    pub model: &'static str,
    pub socket: &'static str,
    pub look: Look,
}

const fn don(key: &'static str, model: &'static str) -> WeaponRow {
    WeaponRow { chara: Chara::Donald, key, model, socket: "R_wep_skt", look: DONALD_LOOK }
}

const fn gof(key: &'static str, model: &'static str) -> WeaponRow {
    WeaponRow { chara: Chara::Goofy, key, model, socket: "R_wep", look: GOOFY_LOOK }
}

/// EVERY STAFF AND SHIELD (sheets/port/kh3-party-weapons.json; kh-partyhost's `party_weapons` test walks the sheet, the
/// pack and the blueprints against this list).
pub const WEAPONS: [WeaponRow; 28] = [
    don("WEP_DONALD_00", "w_do010"),
    don("WEP_DONALD_01", "w_do020"),
    don("WEP_DONALD_02", "w_do030"),
    don("WEP_DONALD_03", "w_do040"),
    don("WEP_DONALD_04", "w_do050"),
    don("WEP_DONALD_05", "w_do060"),
    don("WEP_DONALD_06", "w_do070"),
    don("WEP_DONALD_07", "w_do080"),
    don("WEP_DONALD_08", "w_do090"),
    don("WEP_DONALD_09", "w_do100"),
    don("WEP_DONALD_10", "w_do110"),
    don("WEP_DONALD_11", "w_do120"),
    don("WEP_DONALD_12", "w_do130"),
    don("WEP_DONALD_13", "w_do140"),
    gof("WEP_GOOFY_00", "w_gu010"),
    gof("WEP_GOOFY_01", "w_gu020"),
    gof("WEP_GOOFY_02", "w_gu030"),
    gof("WEP_GOOFY_03", "w_gu040"),
    gof("WEP_GOOFY_04", "w_gu050"),
    gof("WEP_GOOFY_05", "w_gu060"),
    gof("WEP_GOOFY_06", "w_gu070"),
    gof("WEP_GOOFY_07", "w_gu080"),
    gof("WEP_GOOFY_08", "w_gu090"),
    gof("WEP_GOOFY_09", "w_gu100"),
    gof("WEP_GOOFY_10", "w_gu110"),
    gof("WEP_GOOFY_11", "w_gu120"),
    gof("WEP_GOOFY_12", "w_gu130"),
    gof("WEP_GOOFY_13", "w_gu140"),
];

fn norm_eq(a: &str, b: &str) -> bool {
    a.chars().filter(|c| *c != '_').map(|c| c.to_ascii_uppercase()).eq(b.chars().filter(|c| *c != '_').map(|c| c.to_ascii_uppercase()))
}

/// The row of an item key ("WEP_DONALD_00", "WEP_DONALD00") or a model ("w_do010").
pub fn weapon_row(weapon: &str) -> Option<&'static WeaponRow> {
    WEAPONS.iter().find(|w| norm_eq(w.key, weapon) || w.model.eq_ignore_ascii_case(weapon))
}

/// The character's default row (his first weapon) when the equipped key is not one of the list.
fn default_row(chara: Chara) -> &'static WeaponRow {
    match chara {
        Chara::Donald => &WEAPONS[0],
        Chara::Goofy => &WEAPONS[14],
    }
}

/// The no-combat rule's name for a friend.
pub const fn fighter(chara: Chara) -> Fighter {
    match chara {
        Chara::Donald => Fighter::Donald,
        Chara::Goofy => Fighter::Goofy,
    }
}

/// Why the weapon changed (the log, the tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The battle flag came on (slot 1233).
    BattleStart,
    /// In battle (the battle idle / locomotion).
    Battle,
    /// An attack, spell, guard, Link / team attack began (an attack definition's Begin).
    Act(Act),
    /// Hurt (the Damage state: no effect).
    Hurt,
    /// The hero light's hold (the leader in the dark).
    Hold,
    /// Put away by the idle's count.
    Idle,
    /// Put away by the run.
    Run,
    /// The hold ended out of battle.
    HoldReleased,
    /// A no-combat place.
    Zone,
}

/// A member's weapon state (equipment +0x2C8 bit 1 and the sequence of its `SetVisible`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeaponDraw {
    pub out: bool,
    pub seq: Sequence,
    /// The host's hold (the hero light's carrier: the leader only), and what the step saw last.
    pub hold: bool,
    hold_applied: bool,
    was_battle: bool,
    /// Seconds standing / moving in the current put-away sequence (the log).
    pub idle_time: f32,
    pub move_time: f32,
    /// The put-away's clip sequence now, and the battle pose (pawn +0x6E4: BATTLE from the battle until a battle-end clip).
    pub put_away: PutAway,
    pub battle_pose: bool,
    /// The idle clip's clock of the last step (its end: the clock wrapped or the clip finished).
    last_time: f32,
    /// The latest change and its cause (the log, the tests); the appear effects spawned so far.
    pub last: Option<(bool, Why)>,
    pub effects: u32,
}

/// THE PUT-AWAY'S CLIPS (KH3's idle tick 0x14124CD40 and the Land locomotion 0x141219F60), out of battle with the weapon out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PutAway {
    #[default]
    None,
    /// `BattleEnd` playing (standing when the battle ended, slot 1234 0x1412749B0).
    BattleEnd,
    /// The idle loops counted (two ends, then the break).
    Idle { ends: u8 },
    /// `IdleBreakW` playing: the weapon goes at its end.
    Break,
    /// `RunBattleEnd` playing (moving when the battle ended).
    RunEnd,
    /// The loop clip: the weapon goes once it is past 80 % of its length.
    RunLoop,
}

/// The rows the sequence plays.
pub const BATTLE_END: &str = "BattleEnd";
pub const IDLE_BREAK: &str = "IdleBreakW";
pub const RUN_BATTLE_END: &str = "RunBattleEnd";
/// KH3's share of the loop clip after `RunBattleEnd` (0x141219F60: `0.8 < time / length`).
pub const RUN_LOOP_SHARE: f32 = 0.8;

fn has_row(chara: Chara, row: &str) -> bool {
    tables::anim_set(chara).get(row).is_some()
}

/// The locomotion yields its own clip to the put-away's (the speed-matched loops of the AI's Move state and the pad's run;
/// `standing`: the stand branch). True while that clip plays.
pub fn holds_clip(m: &crate::pawn::NpcPawn, standing: bool) -> bool {
    let row = match (m.weapon.put_away, standing) {
        (PutAway::BattleEnd, true) => BATTLE_END,
        (PutAway::Break, true) => IDLE_BREAK,
        (PutAway::RunEnd, false) => RUN_BATTLE_END,
        _ => return false,
    };
    m.anim.clip.name == row && !m.anim.finished
}

/// What the drawing reads: out, its `OpacityFade`, the look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponView {
    pub out: bool,
    pub opacity: f32,
    pub look: Look,
}

/// The act a member's state is (None: nothing that draws the weapon). A new pad state is classified here.
pub fn act_of_state(state: NpcStateId) -> Option<Act> {
    match state {
        // an attack definition runs (melee, magic, the jump attack, Goofy's Discus / Tornado / Turbo ...; the friend
        // leader's spells run as definitions too)
        NpcStateId::Action => Some(Act::Attack),
        NpcStateId::Fr(_) => Some(Act::TeamAttack),
        NpcStateId::Pad(crate::pad::AIR_ATTACK) => Some(Act::Attack),
        NpcStateId::Pad(pd::STEP | pd::AIR | pd::CHARGE | pd::BONK) => Some(Act::Attack),
        NpcStateId::Pad(pd::ATTUNE) => Some(Act::Magic),
        // a guard draws the weapon (Sora's guard Begin: Draw(1)); Goofy's IS his shield
        NpcStateId::Pad(pd::BLOCK | pg::GUARD | pg::GUARD_HIT | pg::GUARD_BREAK | pg::BRACE) => Some(Act::Guard),
        NpcStateId::Pad(pg::PARRY | pg::ART | pg::TAUNT) => Some(Act::GuardAttack),
        NpcStateId::Pad(pg::BREAKER) => Some(Act::Breaker),
        NpcStateId::Pad(pgc::COMBO | pgc::AIR_COMBO) => Some(Act::Attack),
        _ => None,
    }
}

/// How long KH3's put-away takes for a friend from a battle's end (his own clip rows): standing (`BattleEnd`, two `IdleN`
/// loops, `IdleBreakW`), moving (`RunBattleEnd` + 0.8 of the run loop).
pub fn put_away_delays(chara: Chara) -> (f32, f32) {
    let len = |name: &str| tables::clip(chara, name).length;
    (len(BATTLE_END) + 2.0 * len("IdleN") + len(IDLE_BREAK), len(RUN_BATTLE_END) + RUN_LOOP_SHARE * len("RunLoopW"))
}

impl WeaponDraw {
    pub fn view(&self, weapon: &str, chara: Chara) -> WeaponView {
        let look = weapon_row(weapon).filter(|w| w.chara == chara).unwrap_or(default_row(chara)).look;
        WeaponView { out: self.out, opacity: self.seq.opacity(&look), look }
    }
}

/// `Draw(out, effect)`: a no-op when already so; else the bit, the sequence and (with the effect) the weapon's own
/// appear / disappear effect at the weapon mesh's origin (the hand socket).
fn draw(m: &mut crate::pawn::NpcPawn, out: bool, effect: bool, why: Why, effects: &mut dyn kh_core::host::Effects) {
    if m.weapon.out == out {
        return;
    }
    m.weapon.out = out;
    m.weapon.seq.set(out, effect);
    m.weapon.last = Some((out, why));
    m.weapon.idle_time = 0.0;
    m.weapon.move_time = 0.0;
    m.weapon.put_away = PutAway::None;
    if !out {
        m.weapon.battle_pose = false;
    }
    if !effect || m.status.weapon.is_empty() {
        return;
    }
    let row = weapon_row(m.status.weapon).filter(|w| w.chara == m.chara).unwrap_or(default_row(m.chara));
    let name = row.look.effect;
    let at = Attach::Party { member: PartyMember(m.id.0), bone: Some(row.socket), offset: Vec3::ZERO };
    m.seam.spawn(effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at, rotation: Vec3::ZERO, scale: 1.0, persistent: false });
    m.weapon.effects += 1;
}

/// ONE MEMBER, after his step: the rule above.
/// `off_hand`: the weapon is out of his hand (Goofy's thrown shield, Donald's staff on the ground): nothing changes until it
/// is back (the hosts draw it where it is).
pub fn step(m: &mut crate::pawn::NpcPawn, dt: f32, off_hand: bool, effects: &mut dyn kh_core::host::Effects) {
    m.weapon.seq.tick(dt);
    if off_hand {
        return;
    }
    let hold = m.weapon.hold;
    let released = m.weapon.hold_applied && !hold;
    m.weapon.hold_applied = hold;
    let battle = m.in_battle;
    let battle_start = battle && !m.weapon.was_battle;
    m.weapon.was_battle = battle;
    let act = act_of_state(m.state);
    if !weapon_gate::may_draw(fighter(m.chara)) {
        // where Elden Ring forbids attacks: put away, and nothing draws it
        draw(m, false, true, Why::Zone, effects);
        return;
    }
    if hold {
        draw(m, true, true, Why::Hold, effects);
    }
    if battle_start {
        draw(m, true, true, Why::BattleStart, effects);
    }
    if let Some(a) = act {
        draw(m, true, true, Why::Act(a), effects);
    }
    if battle {
        draw(m, true, true, Why::Battle, effects);
    }
    if m.state == NpcStateId::Damage {
        draw(m, true, false, Why::Hurt, effects);
    }
    let free = !battle && !hold && act.is_none() && m.state != NpcStateId::Damage;
    if battle {
        m.weapon.battle_pose = true;
    }
    if !(m.weapon.out && free) {
        (m.weapon.idle_time, m.weapon.move_time) = (0.0, 0.0);
        m.weapon.put_away = PutAway::None;
        return;
    }
    if released {
        draw(m, false, true, Why::HoldReleased, effects);
        return;
    }
    put_away(m, dt, effects);
}

/// THE PUT-AWAY'S CLIPS, out of battle with the weapon out and nothing holding it (see [`PutAway`]).
fn put_away(m: &mut crate::pawn::NpcPawn, dt: f32, effects: &mut dyn kh_core::host::Effects) {
    let moving = m.cmc.velocity.truncate().length() > 10.0;
    let (clip, time, length, finished) = (m.anim.clip.name, m.anim.time, m.anim.clip.length.max(1e-4), m.anim.finished);
    // standing: the Idle state, or the AI's Move state on its stand clip (never a run state: its start / stop clips are
    // the move's own, and a stick pushed during the battle end starts the run at once over these clips)
    let standing = match m.state {
        NpcStateId::Idle => true,
        NpcStateId::Move => m.loco.standing,
        _ => false,
    };
    // running: the loop of the AI's Move state or the pad's run (not the start clip: KH3's Land plays RunBattleEnd from the loop)
    let runs = matches!(m.state, NpcStateId::Move | NpcStateId::Pad(crate::pad::RUN)) && !standing && moving && !clip.starts_with("RunStart") && !clip.starts_with("RunEnd") && !clip.starts_with("WalkEnd");
    let ended = finished || time + 1e-4 < m.weapon.last_time;
    m.weapon.last_time = time;
    if standing {
        m.weapon.idle_time += dt;
    } else if runs {
        m.weapon.move_time += dt;
    }
    let chara = m.chara;
    m.weapon.put_away = match m.weapon.put_away {
        PutAway::None if standing && m.weapon.battle_pose && has_row(chara, BATTLE_END) => {
            m.play(BATTLE_END, -1.0);
            m.weapon.last_time = 0.0;
            PutAway::BattleEnd
        }
        PutAway::None if standing => PutAway::Idle { ends: 0 },
        PutAway::None if runs && m.weapon.battle_pose && has_row(chara, RUN_BATTLE_END) => {
            m.play(RUN_BATTLE_END, 0.15);
            m.weapon.last_time = 0.0;
            PutAway::RunEnd
        }
        PutAway::None if runs => PutAway::RunLoop,
        PutAway::None => PutAway::None,
        // the battle end (its TresLastAnimPose writes NORMAL), then the idle
        PutAway::BattleEnd if !standing || clip != BATTLE_END => PutAway::None,
        PutAway::BattleEnd if ended || time >= length - 1e-4 => {
            m.weapon.battle_pose = false;
            m.play("IdleN", -1.0);
            m.weapon.last_time = 0.0;
            PutAway::Idle { ends: 0 }
        }
        PutAway::BattleEnd => PutAway::BattleEnd,
        PutAway::Idle { .. } if !standing => PutAway::None,
        PutAway::Idle { ends } if ended && clip.starts_with("Idle") => {
            let ends = ends + 1;
            if ends >= 2 && has_row(chara, IDLE_BREAK) {
                m.play(IDLE_BREAK, -1.0);
                m.weapon.last_time = 0.0;
                PutAway::Break
            } else if ends >= 2 {
                draw(m, false, true, Why::Idle, effects);
                PutAway::None
            } else {
                PutAway::Idle { ends }
            }
        }
        PutAway::Idle { ends } => PutAway::Idle { ends },
        PutAway::Break if !standing || clip != IDLE_BREAK => PutAway::None,
        // the break's end: KH3's next idle end with the break flag set puts the weapon away (Draw(0, 1, 1.0)); the idle plays
        PutAway::Break if ended || time >= length - 1e-4 => {
            draw(m, false, true, Why::Idle, effects);
            m.play("IdleN", -1.0);
            PutAway::None
        }
        PutAway::Break => PutAway::Break,
        PutAway::RunEnd if standing => PutAway::None,
        PutAway::RunEnd if clip != RUN_BATTLE_END || finished => {
            m.weapon.battle_pose = false;
            PutAway::RunLoop
        }
        PutAway::RunEnd => PutAway::RunEnd,
        PutAway::RunLoop if standing => PutAway::None,
        PutAway::RunLoop if clip != RUN_BATTLE_END && time / length > RUN_LOOP_SHARE => {
            draw(m, false, true, Why::Run, effects);
            PutAway::None
        }
        PutAway::RunLoop => PutAway::RunLoop,
    };
}

#[cfg(test)]
#[path = "weapon_draw_tests.rs"]
mod tests;

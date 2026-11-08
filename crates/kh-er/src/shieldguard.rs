//! (THE USER 2026-11-02: "fix 'Shields don't count as guarding.' and in fact make the character who attacks a shield bounce
//! off playing their animation from KH3. This means Sora, Donald, Goofy and whoever we add in the future") ELDEN RING'S OWN
//! GUARD, READ AT RUN TIME FOR EVERY CHARACTER, AS KH3'S GUARD.
//!
//! THE READ (no table of enemies: the game's own state and its own param row of the character, `read`):
//! - IS IT GUARDING NOW: `CSChrActionFlagModule::action_modifiers_flags` bit 6, which the game's TimeAct sets while an animation
//!   carries TAE event 0 ChrActionFlag action 3 SET_GUARD_TYPE (the vendored SDK's field `guard_type_set`, and with it
//!   `guard_behavior_judge_id`, the guard's behaviour row). The same flag a player's raised shield sets; an NPC's guard
//!   animations carry it as the player's do. Logged once per NpcParam row when first seen set (`targets.rs`, line `guard:`).
//! - FROM WHICH SIDE: the NpcParam row's `guardAngle` (the game's own param name, Paramdex "guard angle"), READ AS THE FULL
//!   ARC (half of it to either side of the body's facing), so no reading of it can ever guard from behind; 0 (no angle in
//!   the row): the sheet's `guard.shield.half_arc` (90 = the front half). The facing: the body's own orientation (the yaw
//!   `Enemy::yaw` already carries).
//! - HOW MUCH IT STOPS: the NpcParam row's `physGuardCutRate`, `magGuardCutRate`, `fireGuardCutRate`, `thunGuardCutRate`,
//!   `darkGuardCutRate` (percent of the damage of that type a guard keeps off), its `staminaGuardDef`, the module's
//!   `guard_level` (the equipped weapon's guard level), its stamina (`CSChrDataModule::stamina` / `max_stamina`): logged.
//!
//! THE GUARDED-HIT RULE (one rule for every KH character's hit, decided by KH3's classification in kh-core):
//! - KH3 decides WHETHER: the host reports the body `guarding` with its `Shield` arc (`guarded_body`); kh-core's
//!   `TargetBody::seen_from` keeps the guard only for a blow from inside the arc, and `classify` (KH3's 33 rows) then says
//!   GUARD unless KH3's own rules let the hit past a guard (a weak point, a grab, a row with m_bInvalidGuard, Zettaflare).
//! - A GUARDED HIT on the KH side: KH3's guard spark `vs_com_haj00` (its clang se00001_037 rides the effect), no hit sound, no
//!   KH3 reaction (no launch, no juggle, no flinch), no style points, and the attacker bounces off (KH3's REFLECT).
//! - THE DAMAGE AND THE GUARD PRESSURE ARE ELDEN RING'S: the hit still goes out as the player's bullet from the attacker's
//!   side with its whole attack value and its stamina damage (atkStam = half the attack value, as every hit of ours), no
//!   dmgLevel and no poise: the game's own guard takes its cut of it (the row's guard cut rate for the bullet's damage
//!   type), takes the stamina, plays its own guard reaction and BREAKS THE GUARD ITSELF when the stamina runs out (its own
//!   guard break, nothing of ours writes stamina). Until the game's HP is read back the row carries the expected chip
//!   (`chip`: the expected HP x (100 - cut) / 100). The HP route (no bullet): the chip written to its HP, no guard pressure.

use kh_core::host::{Shield, TargetBody};

/// Rows `guard.shield.*` of sheet sora-er-host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShieldRules {
    /// `guard.shield.on`: the game's guard counts as KH3's guard (0 = every body open, as before 2026-11-02).
    pub on: bool,
    /// `guard.shield.half_arc` (degrees): the half arc of a guard whose NpcParam row names no guardAngle.
    pub half_arc: f32,
}

impl ShieldRules {
    pub const DEFAULT: ShieldRules = ShieldRules { on: true, half_arc: 90.0 };

    /// One sheet row; false: not one of these (or out of range).
    pub fn apply_row(&mut self, key: &str, v: f32) -> bool {
        match key {
            "guard.shield.on" => self.on = v != 0.0,
            "guard.shield.half_arc" if (1.0..=180.0).contains(&v) => self.half_arc = v,
            _ => return false,
        }
        true
    }
}

/// What the game says of one character's guard (its param row's numbers and, this frame, its flag and stamina).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct GuardRead {
    /// TAE ChrActionFlag 3 SET_GUARD_TYPE is on now (`action_modifiers_flags.guard_type_set`).
    pub up: bool,
    /// `guard_behavior_judge_id` and `guard_level` of the action-flag module.
    pub judge: u32,
    pub level: u32,
    /// NpcParam `guardAngle` (degrees, read as the full arc).
    pub angle: i16,
    /// NpcParam guard cut rates (percent): physical, magic, fire, lightning, holy (`darkGuardCutRate`).
    pub cuts: [f32; 5],
    /// NpcParam `staminaGuardDef`; the body's stamina now and its maximum.
    pub stamina_def: u8,
    pub stamina: i32,
    pub stamina_max: i32,
}

/// The half arc (degrees) of a guard whose row says `angle`: half of the row's full arc, else the sheet's.
pub fn half_arc(rules: &ShieldRules, angle: i16) -> u8 {
    let half = if angle > 0 { angle as f32 * 0.5 } else { rules.half_arc };
    half.round().clamp(1.0, 180.0) as u8
}

/// The shield KH3 is told of: the guard is up (and the rule on), facing the body's yaw (degrees, the core's frame).
pub fn shield_of(rules: &ShieldRules, read: &GuardRead, yaw: f32) -> Option<Shield> {
    if !rules.on || !read.up || !yaw.is_finite() {
        return None;
    }
    let yaw = (yaw.rem_euclid(360.0)).round() as i16;
    Some(Shield { yaw, half_arc: half_arc(rules, read.angle) })
}

/// The body KH3 classifies: its own (DEFAULT / STRONG, kh-react's), guarding behind the shield while one is up.
pub fn guarded_body(base: TargetBody, shield: Option<Shield>) -> TargetBody {
    match shield {
        Some(s) => TargetBody { guarding: true, shield: Some(s), ..base },
        None => base,
    }
}

/// The game takes this hit through its own guard: KH3's rules guarded it (victim result GUARD), the body's own guard is up
/// (its `Shield` was told), the rule is on. (A GUARD of KH3's own, without the game's guard behind it, takes nothing.)
pub fn game_takes(victim: kh_core::host::ReactionResult, shield_up: bool, on: bool) -> bool {
    on && shield_up && victim == kh_core::host::ReactionResult::Guard
}

/// The cut rate (percent) of the damage type that carries a bullet's number (bullet.rs `Carrier` index: 0 physical, 1 magic,
/// 2 fire, 3 lightning, 4 holy; anything else physical).
pub fn cut_of(carrier_index: usize, cuts: &[f32; 5]) -> f32 {
    cuts.get(carrier_index).copied().unwrap_or(cuts[0]).clamp(0.0, 100.0)
}

/// The HP a guard is expected to let through of `expected` (the HP the hit would take open): (100 - cut) %, rounded.
pub fn chip(expected: i32, cut: f32) -> i32 {
    ((expected.max(0) as f32) * (100.0 - cut.clamp(0.0, 100.0)) / 100.0).round() as i32
}

/// THE GAME'S GUARD OF ONE CHARACTER, read from it: its action-flag module and data module (each checked before it is
/// followed) and its NpcParam row (the parameter repository's own lookup). `None`: the action-flag module is not readable.
/// Game thread.
#[cfg(windows)]
pub fn read(chr: &eldenring::cs::ChrIns) -> Option<GuardRead> {
    use fromsoftware_shared::FromStatic;
    if !crate::game::plausible(crate::word!(chr.modules.action_flag)) {
        return None;
    }
    let flags = &chr.modules.action_flag;
    let mut out = GuardRead { up: flags.action_modifiers_flags.guard_type_set(), judge: flags.guard_behavior_judge_id, level: flags.guard_level, ..GuardRead::default() };
    if crate::game::plausible(crate::word!(chr.modules.data)) {
        (out.stamina, out.stamina_max) = (chr.modules.data.stamina, chr.modules.data.max_stamina);
    }
    if chr.npc_param_id >= 0 {
        // SAFETY: game thread; the singleton lives as long as the game; the SDK checks its holder table (poise.rs reads it so).
        if let Some(row) = unsafe { eldenring::cs::SoloParamRepository::instance() }.ok().and_then(|repo| repo.get::<eldenring::cs::NpcParam>(chr.npc_param_id as u32)) {
            out.angle = row.guard_angle();
            out.cuts = [row.phys_guard_cut_rate(), row.mag_guard_cut_rate(), row.fire_guard_cut_rate(), row.thun_guard_cut_rate(), row.dark_guard_cut_rate()];
            out.stamina_def = row.stamina_guard_def();
        }
    }
    Some(out)
}

/// The NpcParam rows whose guard has been told.
#[cfg(windows)]
static TOLD: std::sync::Mutex<Vec<i32>> = std::sync::Mutex::new(Vec::new());

/// THE EVIDENCE FOR THE USER'S TEST: the first time a kind (NpcParam row) is seen guarding, one line with what was read.
#[cfg(windows)]
pub fn tell_first(npc_id: i32, npc_param: i32, anim: i32, read: &GuardRead, shield: &Shield) {
    let Ok(mut told) = TOLD.lock() else { return };
    if told.contains(&npc_param) || told.len() >= 4096 {
        return;
    }
    told.push(npc_param);
    crate::erlog!(
        "guard: c{npc_id:04} NpcParam {npc_param}: the game's GUARD flag (TAE ChrActionFlag 3 SET_GUARD_TYPE) seen set, animation {anim}, guard behaviour judge {}, guard level {}; NpcParam guardAngle {} -> half arc {} deg about its facing {} deg; guard cut physical {:.0} magic {:.0} fire {:.0} lightning {:.0} holy {:.0} %, staminaGuardDef {}, stamina {}/{}: KH hits from inside the arc are GUARDED (the attacker bounces off), the game's guard takes their bullets",
        read.judge, read.level, read.angle, shield.half_arc, shield.yaw, read.cuts[0], read.cuts[1], read.cuts[2], read.cuts[3], read.cuts[4], read.stamina_def, read.stamina, read.stamina_max
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use kh_core::host::BodyReaction;

    const R: ShieldRules = ShieldRules::DEFAULT;

    fn up(angle: i16) -> GuardRead {
        GuardRead { up: true, angle, cuts: [80.0, 40.0, 40.0, 40.0, 40.0], ..GuardRead::default() }
    }

    #[test]
    fn the_games_guard_flag_makes_a_shield_facing_the_body() {
        // not guarding: no shield, the body as kh-react has it
        assert_eq!(shield_of(&R, &GuardRead::default(), 90.0), None);
        let base = TargetBody { reaction: BodyReaction::Strong, ..TargetBody::DEFAULT };
        assert_eq!(guarded_body(base, None), base, "a non-guarding body is unchanged");
        // guarding: the body's yaw, the row's guardAngle as the full arc, else the sheet's half arc
        assert_eq!(shield_of(&R, &up(0), -90.0), Some(Shield { yaw: 270, half_arc: 90 }));
        assert_eq!(shield_of(&R, &up(120), 10.4), Some(Shield { yaw: 10, half_arc: 60 }));
        assert_eq!(half_arc(&R, 400), 180);
        assert_eq!(shield_of(&ShieldRules { on: false, ..R }, &up(0), 0.0), None, "guard.shield.on = 0: as before");
        let b = guarded_body(base, shield_of(&R, &up(0), 180.0));
        assert!(b.guarding && b.reaction == BodyReaction::Strong && b.shield.is_some());
    }

    #[test]
    fn the_shield_guards_its_front_only() {
        // a body at the origin facing -X (yaw 180), the party member in front of it at -X, another behind it at +X
        let b = guarded_body(TargetBody::DEFAULT, shield_of(&R, &up(0), 180.0));
        let front = b.seen_from(Vec3::new(-150.0, 20.0, 0.0), Vec3::ZERO);
        assert!(front.guarding && front.shield.is_some(), "from the front: guarding");
        let back = b.seen_from(Vec3::new(150.0, 0.0, 0.0), Vec3::ZERO);
        assert!(!back.guarding && back.shield.is_none(), "from behind: open");
    }

    #[test]
    fn the_chip_is_what_the_guard_cut_lets_through() {
        let cuts = [80.0, 40.0, 100.0, 0.0, 55.0];
        assert_eq!(chip(100, cut_of(0, &cuts)), 20);
        assert_eq!(chip(100, cut_of(2, &cuts)), 0, "a full fire guard");
        assert_eq!(chip(100, cut_of(3, &cuts)), 100, "no lightning guard");
        assert_eq!(chip(100, cut_of(9, &cuts)), 20, "an unknown carrier: physical");
        assert_eq!(chip(-5, 50.0), 0);
        assert_eq!(chip(100, 150.0), 0);
    }

    #[test]
    fn only_a_guard_kh3_gave_with_the_games_guard_up_goes_to_the_game() {
        use kh_core::host::ReactionResult as R;
        assert!(game_takes(R::Guard, true, true));
        assert!(!game_takes(R::Guard, false, true), "KH3's own guard, no game guard: nothing taken");
        assert!(!game_takes(R::Guard, true, false), "guard.shield.on = 0");
        assert!(!game_takes(R::Damage, true, true), "an open hit takes the ordinary route");
    }

    #[test]
    fn the_rows_are_read() {
        let mut r = R;
        assert!(r.apply_row("guard.shield.half_arc", 70.0) && r.half_arc == 70.0);
        assert!(!r.apply_row("guard.shield.half_arc", 0.0));
        assert!(r.apply_row("guard.shield.on", 0.0) && !r.on);
        assert!(!r.apply_row("guard.other", 1.0));
    }
}

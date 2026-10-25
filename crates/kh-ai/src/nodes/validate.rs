//! The action definitions' validation (C3, 2026-10-15): what `TresBTDecorator_ValidateAction`
//! (Calc 0x14047FB20) asks the action definition's class default object.
//!
//! The decorator reads the definition class (`bUseBlackboardQuery` (+0xB8 bit 0, default on) ?
//! the BB class key `ActionDefinition` : `ActionDefinitionOverride` +0x88) and, for an actor
//! target, calls on the CDO (each wrapper returns false without a pawn):
//! - `bValidateLocation` (+0xE8 bit 0) -> 0x140520A40 -> CDO vt+0x3E8 = slot 125
//!   `UTresAttackDefinitionBase` 0x140432AD0,
//! - `bValidateOrientation` (bit 1) -> 0x140520A80 -> vt+0x3F8 = slot 127 0x140432D20,
//! - `bValidateExecution` (bit 2) -> 0x140518050 -> vt+0x408 = slot 129 (`TresNpcAttackDefinitionBase`
//!   0x14121E9F0, `…Magic` 0x14121EBF0, `…JumpBase` 0x14121EB60),
//! and passes when every enabled check passes (asm 0x14047FCF1..0x14047FE1E). A vector key
//! (any value, valid or not) uses the location variants vt+0x3E0 / 0x3F0 / 0x400 (0x140520A20 /
//! 0x140520A60 / 0x140518030 -> CDO slots 124 / 126 / 128): every friend definition's vtable
//! holds 0x14041F1E0 there (`mov al,1; ret`, C4): a vector target passes all three checks.
//!
//! The class defaults are the native constructors' (the friends' blueprints have no bytecode and
//! only serialise what differs, so the merged pack properties override these):
//! `UTresAttackDefinitionBase` 0x14042FD30 (m_bTargetRequired on, m_bDirectPathRequired and
//! m_bLineOfSightRequired on, m_YawTolerance 90, m_bValidatePitch off, m_EQSRangeRefinement 50),
//! `UTresNpcAttackDefinitionBase` 0x14121BBF0 (m_bMinDistance / m_bMaxDistance on, m_MaxDistance 500,
//! m_bValidateYaw on, m_YawTolerance 90), `…Melee` 0x14123F050 (m_bMinHeight / m_bMaxHeight on,
//! 0 / 100), `…Ranged` 0x14123F0D0 (m_bDirectPathRequired off, m_bValidateYaw on, m_bPitch on,
//! m_YawTolerance 30, m_PitchToleranceUp 45, m_PitchToleranceDown 0), `…Magic` 0x14121C1B0,
//! `…JumpBase` 0x14121BFA0 (no validation fields), Goofy's specials 0x1411BB870..0x1411BBA30
//! (ability kinds GOOFY_TORNADE / GOOFY_TURBO / GOOFY_FLIP; Discus m_bNpcValidatePitch off).
//!
//! Positions follow UE: an actor's location is its root (capsule centre) = `ActorView::center`.

use crate::api::ActorView;
use crate::pack::{pb, pf, pobj, ps, Props};
use glam::Vec3;

/// The validation fields of one action definition (native defaults + the class's merged props).
#[derive(Clone, Debug, PartialEq)]
pub struct ActionCheck {
    pub native: String,
    pub target_required: bool,
    pub self_in_viewport: bool,
    pub use_min_dist: bool,
    pub min_dist: f32,
    pub use_max_dist: bool,
    pub max_dist: f32,
    pub use_min_height: bool,
    pub min_height: f32,
    pub use_max_height: bool,
    pub max_height: f32,
    pub direct_path: bool,
    pub line_of_sight: bool,
    pub validate_yaw: bool,
    pub yaw_offset: f32,
    pub yaw_tolerance: f32,
    pub validate_pitch: bool,
    pub pitch_offset: f32,
    pub pitch_tolerance: f32,
    /// `UTresNpcAttackDefinitionRanged` m_bPitch / Up / Down (slot 150 override 0x141248060).
    pub ranged: bool,
    pub ranged_pitch: bool,
    pub pitch_up: f32,
    pub pitch_down: f32,
    /// `UTresAttack8_n_ex002_Discus` (slot 150 override 0x1411CA530) m_bNpcValidatePitch.
    pub discus: bool,
    pub npc_pitch: bool,
    pub npc_pitch_offset: f32,
    pub npc_pitch_tolerance: f32,
    /// Execution (slot 129): `m_bSwimAttack`, m_NpcAIInfo (ETresFNpcAIAttackDefType value,
    /// ETresAbilityKind name), whether the class is a `TresNpcAttackDefinitionBase`.
    pub npc: bool,
    /// A `UTresAttackDefinitionBase` (the FindAttackLocation nodes need one).
    pub attack: bool,
    /// `m_EQSRangeRefinement` (+0x100; ctor 0x14042FD30: 50).
    pub eqs_refine: f32,
    pub magic: bool,
    pub swim_attack: bool,
    pub def_type: u8,
    pub ability: Option<String>,
}

fn is_npc_attack(native: &str) -> bool {
    native.starts_with("TresNpcAttackDefinition") || (native.starts_with("TresAttack") && native.contains("_n_ex00"))
}

/// The native class derives from `UTresAttackDefinitionBase` (the friends' attack classes, not
/// the plain action definitions such as PlayCameraAnim / RideMarshmallow).
pub fn is_attack_definition(native: &str) -> bool {
    is_npc_attack(native) || native.starts_with("TresAttackDefinition")
}

impl ActionCheck {
    pub fn from_class(native: &str, p: &Props) -> ActionCheck {
        let npc = is_npc_attack(native);
        let melee = native == "TresNpcAttackDefinitionMelee";
        let ranged = matches!(native, "TresNpcAttackDefinitionRanged" | "TresNpcAttackDefinitionMagic" | "TresAttack8_n_ex002_Discus");
        let magic = native == "TresNpcAttackDefinitionMagic";
        let attack = npc || native.starts_with("TresAttackDefinition");
        // ctor ability kinds of Goofy's specials (0x1411BB8E0 / 0x1411BB980 / 0x1411BBA30)
        let ctor_ability = match native {
            "TresAttack6_n_ex002_Tornado" => Some("GOOFY_TORNADE"),
            "TresAttack7_n_ex002_Turbo" => Some("GOOFY_TURBO"),
            "TresAttack8_n_ex002_Discus" => Some("GOOFY_FLIP"),
            _ => None,
        };
        // ctor m_bSwimAttack: WaterRush 0x1411BB870, WaterSwing 0x1411BBAD0
        let ctor_swim = matches!(native, "TresAttack5_n_ex002_WaterRush" | "TresAttack9_n_ex001_WaterSwing");
        let info = pobj(p, "m_NpcAIInfo");
        let def_type = info
            .and_then(|o| o.get("m_AttackDefType"))
            .and_then(|v| v.as_str())
            .map(|s| match crate::pack::enum_value(s) {
                "TRES_FNPC_AI_ATK_DEF_NORMAL_ATTACK" => 1,
                "TRES_FNPC_AI_ATK_DEF_ABILITY_ATTACK" => 2,
                "TRES_FNPC_AI_ATK_DEF_ABILITY_CURE" => 3,
                "TRES_FNPC_AI_ATK_DEF_ABILITY_SUPPORT" => 4,
                _ => 0,
            })
            .unwrap_or(0);
        let ability = info
            .and_then(|o| o.get("m_AttackAbilityKind"))
            .and_then(|v| v.as_str())
            .map(|s| crate::pack::enum_value(s).to_string())
            .or_else(|| ctor_ability.map(String::from));
        let _ = ps;
        ActionCheck {
            native: native.to_string(),
            target_required: pb(p, "m_bTargetRequired", attack),
            self_in_viewport: pb(p, "m_bSelfInViewport", false),
            use_min_dist: pb(p, "m_bMinDistance", npc),
            min_dist: pf(p, "m_MinDistance", 0.0),
            use_max_dist: pb(p, "m_bMaxDistance", npc),
            max_dist: pf(p, "m_MaxDistance", if npc { 500.0 } else { 0.0 }),
            use_min_height: pb(p, "m_bMinHeight", melee),
            min_height: pf(p, "m_MinHeight", 0.0),
            use_max_height: pb(p, "m_bMaxHeight", melee),
            max_height: pf(p, "m_MaxHeight", if melee { 100.0 } else { 0.0 }),
            direct_path: pb(p, "m_bDirectPathRequired", attack && !ranged),
            line_of_sight: pb(p, "m_bLineOfSightRequired", attack),
            validate_yaw: pb(p, "m_bValidateYaw", npc),
            yaw_offset: pf(p, "m_YawOffset", 0.0),
            yaw_tolerance: pf(p, "m_YawTolerance", if ranged { 30.0 } else { 90.0 }),
            validate_pitch: pb(p, "m_bValidatePitch", false),
            pitch_offset: pf(p, "m_PitchOffset", 0.0),
            pitch_tolerance: pf(p, "m_PitchTolerance", 0.0),
            ranged,
            ranged_pitch: pb(p, "m_bPitch", ranged),
            pitch_up: pf(p, "m_PitchToleranceUp", 45.0),
            pitch_down: pf(p, "m_PitchToleranceDown", 0.0),
            discus: native == "TresAttack8_n_ex002_Discus",
            npc_pitch: pb(p, "m_bNpcValidatePitch", false),
            npc_pitch_offset: pf(p, "m_NpcPitchOffset", 0.0),
            npc_pitch_tolerance: pf(p, "m_NpcPitchTolerance", 0.0),
            npc,
            attack,
            eqs_refine: pf(p, "m_EQSRangeRefinement", 50.0),
            magic,
            swim_attack: pb(p, "m_bSwimAttack", ctor_swim),
            def_type,
            ability,
        }
    }
}

impl ActionCheck {
    /// The query parameters an attack definition's CDO hands its `m_ActionEQSQuery` (vt+0x480
    /// = `UTresAttackDefinitionBase` 0x140438C90, not overridden by the friends' classes) [C4]:
    /// distances dmin = (bMinDistance and MinDistance > -10000) ? MinDistance : 0, dmax =
    /// (bMaxDistance and MaxDistance < 10000) ? MaxDistance : dmin, "Min Distance" = min(dmin +
    /// refinement, mid), "Max Distance" = max(dmax - refinement, mid); heights (relative to the
    /// target: the ring's height range) "Min Height" = (bMaxHeight and MaxHeight < 10000) ?
    /// -MaxHeight : 0, "Max Height" = (bMinHeight and MinHeight > -10000) ? -MinHeight : Min
    /// Height; "Min / Max Filter Height" -100000 / 100000, or with bTargetRequired and a target:
    /// (bMinHeight ? MinHeight : -10000) + f - 2 x its bounds' z extent / (bMaxHeight ? MaxHeight
    /// : 10000) - f, f = min((Max Height - Min Height) / 2, refinement); "Min / Max Item Spacing" =
    /// min(distance / 2 + 100, 600); k = max(Max Height - Min Height, 3000) x 0.001 + 1;
    /// "Min / MaxXYItemSpacing" = spacing x k; "ZItemSpacing" = 100 k; "Item / Context Height
    /// Offset" 0.
    pub fn attack_query_params(&self, target: Option<&ActorView>) -> Vec<(String, f32)> {
        let dmin = if self.use_min_dist && self.min_dist > -10000.0 { self.min_dist } else { 0.0 };
        let dmax = if self.use_max_dist && self.max_dist < 10000.0 { self.max_dist } else { dmin };
        let mid = (dmax + dmin) * 0.5;
        let min_d = (dmin + self.eqs_refine).min(mid);
        let max_d = (dmax - self.eqs_refine).max(mid);
        let min_h = if self.use_max_height && self.max_height < 10000.0 { -self.max_height } else { 0.0 };
        let max_h = if self.use_min_height && self.min_height > -10000.0 { -self.min_height } else { min_h };
        let (mut min_f, mut max_f) = (-100000.0, 100000.0);
        if let (true, Some(t)) = (self.target_required, target) {
            let f = ((max_h - min_h) * 0.5).min(self.eqs_refine);
            min_f = (if self.use_min_height { self.min_height } else { -10000.0 }) + f - t.half_height * 2.0;
            max_f = (if self.use_max_height { self.max_height } else { 10000.0 }) - f;
        }
        let min_is = (min_d * 0.5 + 100.0).min(600.0);
        let max_is = (max_d * 0.5 + 100.0).min(600.0);
        let k = (max_h - min_h).max(3000.0) * 0.001 + 1.0;
        [
            ("Min Distance", min_d),
            ("Max Distance", max_d),
            ("Min Height", min_h),
            ("Max Height", max_h),
            ("Min Filter Height", min_f),
            ("Max Filter Height", max_f),
            ("Min Item Spacing", min_is),
            ("Max Item Spacing", max_is),
            ("MinXYItemSpacing", min_is * k),
            ("MaxXYItemSpacing", max_is * k),
            ("ZItemSpacing", k * 100.0),
            ("Item Height Offset", 0.0),
            ("Context Height Offset", 0.0),
        ]
        .iter()
        .map(|(n, v)| (n.to_string(), *v))
        .collect()
    }
}

/// What the checks need to know about the asking pawn.
pub struct Me {
    pub center: Vec3,
    pub half_height: f32,
    pub yaw: f32,
    /// FUN_1405ad820: the movement component says swimming.
    pub swimming: bool,
    /// FUN_141267540: bound by a bad status (adhere actor vt+0x7C0, or SNEEZE state with bad
    /// status 0x0D) [port: `AiBody::function("IsBadStatusBound")`, fallback false].
    pub bad_status_bound: bool,
}

/// The world queries the checks make (the decorator's Ctx implements it).
pub trait CheckWorld {
    /// FUN_14443b030 + FUN_140e5af80: the point projects inside the player's viewport.
    fn in_viewport(&self, p: Vec3) -> bool;
    /// FUN_140545a90: a straight navigation path (path found, < 3 points or < 5 with a clear
    /// nav raycast; `UTresAllLinksDisabledNavigationQueryFilter`).
    fn direct_path(&self, from: Vec3, to: Vec3) -> bool;
    /// One of the two "AttackLOS" traces (bounds top -> bounds top, eyes -> eyes) is clear.
    fn line_of_sight(&self, from: Vec3, to: Vec3) -> bool;
    /// FUN_1405a4ea0 -> pawn vt+0x14F0 (ability kind, equipped = 1).
    fn has_ability(&self, kind: &str) -> bool;
}

/// Degrees normalised to (-180, 180] (FRotator::NormalizeAxis).
pub fn norm(a: f32) -> f32 {
    let mut d = a % 360.0;
    if d < 0.0 {
        d += 360.0;
    }
    if d > 180.0 {
        d -= 360.0;
    }
    d
}

/// FVector::Rotation pitch / yaw in degrees.
pub fn rotation(v: Vec3) -> (f32, f32) {
    let yaw = v.y.atan2(v.x).to_degrees();
    let pitch = v.z.atan2(v.truncate().length()).to_degrees();
    (pitch, yaw)
}

impl ActionCheck {
    /// Slot 125 0x140432AD0 (+ 147 0x14043F040, 148 0x14043F140, 151 0x14043E970, 152 0x14043EAE0).
    pub fn location(&self, me: &Me, t: Option<&ActorView>, w: &dyn CheckWorld) -> bool {
        if !self.target_required {
            return true;
        }
        let Some(t) = t else { return false };
        if self.self_in_viewport && !w.in_viewport(me.center) {
            return false;
        }
        // 0x14043F040: 2D distance root -> target location, minus the target's collision radius
        // (FUN_140540e20: m_CollisionRadius override +0xA20, else the capsule's larger extent)
        let d = (t.center - me.center).truncate().length() - t.radius;
        let min_ok = !self.use_min_dist || self.min_dist <= d;
        let max_ok = !self.use_max_dist || d <= self.max_dist;
        if !(min_ok && max_ok) {
            return false;
        }
        // 0x14043F140: the target's capsule bottom / top relative to the pawn's root height;
        // limits -10000 / 10000 when off
        let bottom = t.center.z - t.half_height - me.center.z;
        let top = bottom + 2.0 * t.half_height;
        let lo = if self.use_min_height { self.min_height } else { -10000.0 };
        let hi = if self.use_max_height { self.max_height } else { 10000.0 };
        if !(lo <= top && bottom <= hi) {
            return false;
        }
        // 0x14043E970: a direct navigation path to the target
        if self.direct_path && !w.direct_path(me.center - Vec3::Z * me.half_height, t.position) {
            return false;
        }
        // 0x14043EAE0: "AttackLOS" traces
        if self.line_of_sight && !w.line_of_sight(me.center + Vec3::Z * me.half_height, t.center + Vec3::Z * t.half_height) {
            return false;
        }
        true
    }

    /// Slot 127 0x140432D20 (+ 149 0x14043FC00 yaw, 150 0x14043F4F0 / Ranged 0x141248060 pitch).
    pub fn orientation(&self, me: &Me, t: Option<&ActorView>) -> bool {
        if !self.target_required {
            return true;
        }
        let Some(t) = t else { return false };
        let (pitch, yaw) = rotation(t.center - me.center);
        if self.validate_yaw && norm(yaw - (me.yaw + self.yaw_offset)).abs() > self.yaw_tolerance {
            return false;
        }
        // the pawn stands upright: its pitch is 0
        if self.discus {
            // slot 150 0x1411CA530 [C4]: m_bNpcValidatePitch off -> true; else the pitch of
            // unit(target root - own root) against normalise(own pitch (0: upright) +
            // m_NpcPitchOffset): passes when |normalise(difference)| <= m_NpcPitchTolerance
            if self.npc_pitch && norm(pitch - norm(self.npc_pitch_offset)).abs() > self.npc_pitch_tolerance {
                return false;
            }
        } else if self.ranged {
            // 0x141248060: target location vt+0x6E0; fails above Up or below Down (asm 0x141248395)
            if self.ranged_pitch {
                let d = norm(pitch);
                if d > self.pitch_up || d < self.pitch_down {
                    return false;
                }
            }
        } else if self.validate_pitch && norm(pitch - self.pitch_offset).abs() > self.pitch_tolerance {
            return false;
        }
        true
    }

    /// Slot 129: NpcBase 0x14121E9F0 / Magic 0x14121EBF0 / JumpBase 0x14121EB60.
    pub fn execution(&self, me: &Me, t: Option<&ActorView>, w: &dyn CheckWorld) -> bool {
        if !self.npc {
            // TresActionDefinitionBase's own slot 129 [D: not read; only the world-mode
            // RideMarshmallow uses it]
            return true;
        }
        if t.is_none() {
            return false;
        }
        let ability_ok = || self.ability.as_deref().map(|k| w.has_ability(k)).unwrap_or(true);
        if self.magic && self.def_type >= 2 && !ability_ok() {
            return false;
        }
        if self.swim_attack && !me.swimming {
            return false;
        }
        if me.bad_status_bound {
            return false;
        }
        if self.def_type > 1 && !ability_ok() {
            return false;
        }
        true
    }
}

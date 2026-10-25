//! The numbers of the Elden Ring host (pure; no game access): how Sora's damage maps to Elden
//! Ring hit points and back, who counts as a target, the weight classes. The values are the rows
//! of `sheets/port/sora-er-host.json` (read at Sora's start; the defaults here are the same
//! numbers, used when the sheet is not found). HOST CHOICES, not data of either game.

use kh_core::enums::ETresDamageKind;

/// A weight class of a target, from the size of its Elden Ring capsule (a first cut).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Class {
    pub max_radius_cm: f32,
    pub max_height_cm: f32,
    /// `ETresBodyPushPowerLevel` 0..9 of its push body (Sora is 4).
    pub push_level: u8,
    /// A body of this class can be launched, juggled and blown away (the enemy puppet). False =
    /// KH3's armoured giant: damage and poise only, it is never moved by us.
    pub puppet: bool,
}


/// How one `ETresDamageKind` of a hit of Sora's is asked of an Elden Ring enemy (rows
/// `react.kind.<Name>` of the sheet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reaction {
    /// `dmgLevel` of the attack row for a target standing on the ground.
    pub level: u8,
    /// `dmgLevel` for a target that is our puppet in the air (a light airborne reaction).
    pub air_level: u8,
    /// `atkSuperArmor`: the poise damage of the hit.
    pub poise: f32,
}

pub const KINDS: usize = 24;

/// An explicit class of one kind of character (rows `target.npc.<cNNNN | NpcParam id>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Override {
    /// True: `id` is a character id (cNNNN); false: an NpcParam row id.
    pub by_chr: bool,
    pub id: i32,
    pub puppet: bool,
}

pub const OVERRIDES: usize = 32;

/// THE ANIMATION OF A REACTION (IMPL_ER6_STATE.md; rows `react.anim.*`): Elden Ring plays a damage
/// animation for the `dmgLevel` of the bullet that delivers the hit; the puppet holds it at its
/// airborne frame while the body flies and lets the rest (lying, getting up) run when it lands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimRules {
    /// `dmgLevel`s for a FLINCH (a basic hit on a standing body), tried in this order per kind of
    /// character until one is seen to start a damage animation.
    pub flinch_levels: [u8; 4],
    /// `dmgLevel`s for a THROW (launcher, blow-away, any hit on a body in the air).
    pub blow_levels: [u8; 4],
    /// Seconds into the throw animation at which the body is in the air (held there while it
    /// flies), and from which it lies on the ground.
    pub air_at: f32,
    pub down_at: f32,
    /// Per-character timings (`react.anim.cNNNN`).
    pub timings: [Option<AnimTiming>; ANIM_TIMINGS],
    /// `react.slide.<class>`: the share of KH3's flinch slide (175 cm a hit) a body of the size
    /// class is moved (small, medium, large).
    pub slide: [f32; 3],
    /// `target.flying_height`: a body first seen with its feet more than this many cm above the
    /// ground is a flyer (kh-react `Body::flying`: it is not dropped when a reaction lets go).
    pub flying_height: f32,
}

pub const ANIM_TIMINGS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimTiming {
    pub chr: i32,
    pub air_at: f32,
    pub down_at: f32,
}

impl Default for AnimRules {
    fn default() -> AnimRules {
        AnimRules::DEFAULT
    }
}

impl AnimRules {
    pub const DEFAULT: AnimRules = AnimRules { flinch_levels: [2, 1, 8, 3], blow_levels: [10, 4, 7, 6], air_at: 0.3, down_at: 0.5, timings: [None; ANIM_TIMINGS], slide: [1.0, 1.0, 1.0], flying_height: 120.0 };

    /// (air_at, down_at) for a character id.
    pub fn timing(&self, chr: i32) -> (f32, f32) {
        self.timings.iter().flatten().find(|t| t.chr == chr).map_or((self.air_at, self.down_at), |t| (t.air_at, t.down_at))
    }
}

/// The per-character rows of the sheet (`react.anim.cNNNN`).
fn default_timings() -> [Option<AnimTiming>; ANIM_TIMINGS] {
    let mut t = [None; ANIM_TIMINGS];
    t[0] = Some(AnimTiming { chr: 4311, air_at: 0.3, down_at: 0.5 });
    t
}

fn levels(text: &str) -> Option<[u8; 4]> {
    let v: Vec<u8> = text.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
}

const fn reaction(level: u8, air_level: u8, poise: f32) -> Reaction {
    Reaction { level, air_level, poise }
}

/// THE DEFAULT TABLE: (ETresDamageKind value, reaction); the same numbers as the sheet's rows. What
/// each `dmgLevel` does was measured (IMPL_ER5_STATE.md). Kinds without a row react as SMALL.
const REACT_ROWS: &[(u8, Reaction)] = &[
    (0, reaction(0, 0, 0.0)),
    (1, reaction(1, 1, 10.0)),
    (2, reaction(10, 10, 60.0)),
    (3, reaction(10, 10, 60.0)),
    (4, reaction(6, 1, 60.0)),
    (7, reaction(6, 1, 60.0)),
    (8, reaction(6, 1, 60.0)),
    (9, reaction(7, 7, 60.0)),
    (10, reaction(6, 1, 60.0)),
    (11, reaction(6, 1, 60.0)),
    (16, reaction(0, 0, 0.0)),
];

/// The explicit rows of the sheet (`target.npc.*`).
fn default_overrides() -> [Option<Override>; OVERRIDES] {
    let mut o = [None; OVERRIDES];
    o[0] = Some(Override { by_chr: true, id: 4170, puppet: false });
    o
}

fn default_reactions() -> [Reaction; KINDS] {
    let mut t = [REACT_ROWS[1].1; KINDS];
    for (kind, r) in REACT_ROWS {
        t[*kind as usize] = *r;
    }
    t
}

const KIND_NAMES: &[(&str, u8)] = &[("None", 0), ("Small", 1), ("Blow", 2), ("Sidespin", 3), ("Verticalroll", 4), ("Billiard", 5), ("Float", 6), ("Toss", 7), ("Launch", 8), ("Beat", 9), ("Parabola", 10), ("ParabolaNoRecovery", 11), ("Magnet", 16)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// `damage.out.scale`: Elden Ring HP per KH3 damage point. THE ONE CONVERSION.
    pub out_scale: f32,
    /// `damage.in.scale`: share of Sora's HP per share of the Tarnished's HP.
    pub in_scale: f32,
    /// `damage.in.blow_share`: from this share of the Tarnished's max HP a hit throws Sora.
    pub blow_share: f32,
    /// `damage.in.min_share`: below this share a loss is damage over time (no hit reaction).
    pub min_share: f32,
    /// `exp.per_max_hp`: KH3 EXP per hit point of a defeated enemy's maximum.
    pub exp_per_max_hp: f32,
    /// `target.range`, cm.
    pub range: f32,
    /// `target.engage_range`, cm.
    pub engage_range: f32,
    pub small: Class,
    pub medium: Class,
    pub large: Class,
    /// `target.poise_armoured`: a body whose poise maximum (the larger of super armour and
    /// toughness) is above this is armoured whatever its size.
    pub poise_armoured: f32,
    /// `target.hp_armoured`: so is a body of more than this many hit points (bosses).
    pub hp_armoured: f32,
    pub overrides: [Option<Override>; OVERRIDES],
    /// By `ETresDamageKind` value.
    pub reactions: [Reaction; KINDS],
    /// `damage.out.poise_scale`: multiplies every row's poise number.
    pub poise_scale: f32,
    /// `react.landing_level`: the `dmgLevel` asked of a puppet when it lands (0 = none).
    pub landing_level: u8,
    /// `damage.in.body_hp`: the hidden Tarnished's maximum HP is held at this multiple of his own.
    pub body_hp: f32,
    /// `death.scripted.chr / .area / .block`: where Sora's death goes straight to Elden Ring's own (the tutorial boss).
    pub scripted_death: [i32; 3],
    /// `target.point.max_height` (cm; IMPL_ER13): the point of an enemy that Sora's attacks aim at lies at most this far
    /// above its feet (KH3's enemies have their lock point about 90 cm up; an Elden Ring capsule's centre can be metres up).
    pub point_max_height: f32,
    /// `target.point.terrain_slope` (degrees; IMPL_ER14): while Sora and a target both stand on the ground, the height
    /// difference of their feet up to tan(this) x their horizontal distance is the hillside, not "the target is high"
    /// (KH3's attack choice: lock point more than 160 above him = the rising attack and air combos). 0 = KH3's own measure.
    pub terrain_slope: f32,
    /// `hud.enemy_gauge.hp_scale` (IMPL_ER13): Elden Ring HP x this = the HP Sora's enemy gauge shows (KH3: one bar per 200 HP,
    /// 20 stock squares, shown HP capped at 24,000).
    pub gauge_hp_scale: f32,
    /// `grace.flame.sfx.undiscovered` / `.discovered` (IMPL_ER17): the sfx ids of a site of grace's own light (the effect of
    /// its map object AEG099_060), not drawn while KH3's save points stand for the graces.
    pub grace_flame_sfx: [i32; 2],
    /// `own.anim.<what>` (IMPL_ER17): the animation ids (first, last) of the hidden body's own actions that Sora performs
    /// (retargeted): each MEASURED in the game by doing the action; (0, -1) = unused.
    pub own_anims: [(i32, i32); OWN_ANIMS],
    /// `own.move.<what>` (IMPL_ER17): the same for actions in which THE GAME MOVES its body (a ladder, a fog wall): Sora
    /// follows the body through them (the passive switches are relaxed, the pad is the game's) instead of holding it in place.
    pub own_moves: [(i32, i32); OWN_ANIMS],
    /// collision.radius (cm; IMPL_ER18): NO CAST OF SORA'S AGAINST ELDEN RING'S WORLD IS WIDER THAN THIS. KH3's capsule is 60 cm
    /// in radius; Elden Ring's doorways are made for its own player's 40 cm (MEASURED: an opened double door leaves 0.80 m).
    pub collision_radius: f32,
    /// collision.gap (cm; IMPL_ER23A): a crack between floor boards narrower than twice this is closed for the lines and thin
    /// probes of KH3's floor finding (sweep.rs `Options::gap`; 0 = off).
    pub collision_gap: f32,
    /// collision.floor_layer (a layer number; IMPL_ER24): the FLOORS of the map's other collision mesh (the one characters
    /// stand on) are taken from a second cast with this layer (sweep.rs `Extra`; 0 = none). (WALLS) 0 since the KH characters
    /// cast with the Tarnished's own layer (`collision_layer`), which sees that mesh itself.
    pub collision_floor_layer: u32,
    /// (WALLS) collision.layer (a layer number): THE LAYER OF EVERY CAST OF THE KH CHARACTERS (leader and friends) against Elden
    /// Ring's world = the Tarnished's own character proxy's (2, read in the game's code: havok.rs `FILTER_PLAYER`), so they are
    /// blocked by exactly what blocks him, the invisible walls included. Control key `w.filter` (a whole word) overrides it.
    pub collision_layer: u32,
    /// collision.foot (cm; IMPL_ER24): the foot rule's radius = Elden Ring's own player capsule (sweep.rs `foot_support`; 0 = off).
    pub collision_foot: f32,
    /// target.point.hover_min (cm; IMPL_ER23A): a character whose model's bones (their lower tenth) are more than this above its
    /// position node (and more than half the model's own height) HOVERS above its capsule: the capsule is taken up to its body.
    pub hover_min: f32,
    /// (IMPL_ER19) rows `prize.*`: the host's choices about the prize orbs in Elden Ring (prize.rs).
    pub prize: PrizeRules,
    /// (IMPL_ER19) row `savepoint.sfx`: 0 = the save point is silent, 1 = KH3 (its sound with the heal burst),
    /// 2 = also at every entry with nothing to heal (savepoint.rs).
    pub savepoint_sfx: u8,
    /// (IMPL_ER52) rows `savepoint.ground_*`: the save point's effects on the floor under its root (savepoint_rules.rs).
    pub ground: crate::savepoint_rules::Ground,
    /// (IMPL_ER21) rows `react.poise.*`, `react.revenge.*`, `react.endurance.*`, `react.chain.*`: what keeps an armoured
    /// body from being staggered again and again (bossguard.rs).
    pub guard: crate::bossguard::GuardRules,
    /// (IMPL_ER45) rows `poise.*`: Elden Ring's poise decides how hard an ordinary (not armoured) enemy is to stagger (poise.rs).
    pub poise: crate::poise::PoiseRules,
    /// (IMPL_ER22) `finish.*`: KH3's boss finish (finish.rs).
    pub finish: crate::finish::FinishRules,
    pub anim: AnimRules,
    /// (IMPL_ER32) `break.*`, `damage.element.*`: breakable map objects and the elements of Sora's hits (breakables.rs).
    pub breaking: crate::breakables::BreakRules,
    /// (IMPL_ER61) `damage.shotlock.*`: what a shotlock's hits ask of the game (`shotlock_hit`).
    pub shotlock: ShotlockRules,
    /// (IMPL_ER61) `magic.mp_cost_scale`: the factor on every spell's MP cost in this host (`Status::mp_cost_scale`;
    /// the test app keeps KH3's 1).
    pub mp_cost_scale: f32,
    /// (DF, 2026-10-15) THE DIFFICULTY ON ELDEN RING'S HITS: a factor on every hit's share rule (`damage_in`), set from Sora's
    /// game level at run time (sora.rs): KH3's factor on damage by the other side (0x140642820: Critical x2) times the
    /// base-HP ratio Standard / this level (90 / 45 = 2 on Critical, 0x140575D73): with the share rule a halved maximum alone
    /// would change nothing, so a hit takes the share of the STANDARD maximum x2 off the halved bar = 4x the share. 1 = Standard.
    /// Not applied to status ticks (KH3's max-HP-proportional damage skips the difficulty factor: 0x140632240 step 2).
    pub diff_in: f32,
    /// (DF) row `magic.mp_cost_scale.critical`: the MP cost factor on Critical in place of `mp_cost_scale` (KH3 Critical's
    /// halved MP already makes magic scarcer than the 1.5 nerf did; both together would count it twice).
    pub mp_cost_scale_critical: f32,
    /// (DF) row `difficulty.new_game`: the `ETresGameLevelID` of a NEW GAME when the control file has no `difficulty` key
    /// (0 Beginner, 1 Standard, 2 Proud, 3 Critical). The user, 2026-10-16: "keep everything as it was on 'normal' mode" = 1
    /// (decision dec032); Critical by the control key `difficulty = critical`.
    pub new_game_level: u8,
}

/// (IMPL_ER61; the user, 2026-10-14: "seriously nerf shotlock, it is one shotting enemies with four health bars")
/// The rows `damage.shotlock.*`. MEASURED in er\logs\kh-er.prev.log: one Ragnarok fired 24 bullets of 26 KH each on
/// c4630 (max HP 2949), every one asked 26 x 7 = 182 of the game (4368 in all) and the game took 2038 HP (69 % of its
/// maximum) in half a second; a long ground combo took 709 - 1137 HP of enemies of 2896 - 3128 HP. KH3's shotlock
/// bullets each carry an ordinary hit's damage and a full lock fires two dozen of them: nothing in the one conversion
/// `damage.out.scale` knows they come as a volley. HOST CHOICES, not data of either game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotlockRules {
    /// `damage.shotlock.scale`: a factor on the KH3 damage of every shotlock hit (before `damage.out.scale`).
    pub scale: f32,
    /// `damage.shotlock.cap_share`: one shotlock asks at most this share of a big target's maximum HP of the game
    /// (summed over its hits on that target; 0 = no cap). Asked, not taken: the target's defences come after.
    pub cap_share: f32,
    /// `damage.shotlock.cap_min_hp`: the cap is for targets of at least this many hit points (big enemies, bosses).
    pub cap_min_hp: i32,
    /// `damage.shotlock.gap`: seconds without a shotlock hit on a target after which the next hit opens a new shotlock.
    pub gap: f32,
}

impl ShotlockRules {
    pub const DEFAULT: ShotlockRules = ShotlockRules { scale: 0.5, cap_share: 0.4, cap_min_hp: 1500, gap: 3.0 };
    /// The key `balance.shotlock = 0`: shotlock hits as every other hit (no scale, no cap).
    pub const OFF: ShotlockRules = ShotlockRules { scale: 1.0, cap_share: 0.0, cap_min_hp: 0, gap: 3.0 };

    fn apply_row(&mut self, key: &str, v: f32) -> bool {
        match key {
            "damage.shotlock.scale" if (0.0..=10.0).contains(&v) => self.scale = v,
            "damage.shotlock.cap_share" if (0.0..=10.0).contains(&v) => self.cap_share = v,
            "damage.shotlock.cap_min_hp" if v >= 0.0 => self.cap_min_hp = v as i32,
            "damage.shotlock.gap" if (0.1..=60.0).contains(&v) => self.gap = v,
            _ => return false,
        }
        true
    }
}

/// `m_CommandKind` of a shotlock's projectiles (kh-core projectile.rs: `HitRecord::flags` low byte = the cast command).
pub const SHOTLOCK_COMMAND: u32 = 0xFD;

/// A hit of a shotlock: its projectile's command is the shotlock's (or its attack row is a ShootFlow row `SF_*`).
/// (XSI, 2026-10-16) Not Light Wave's row `SF_LFSHOOT` (an XS ability cast during MP Charge, not a shotlock: XS 2.9).
pub fn is_shotlock(hit: &kh_core::HitRecord) -> bool {
    hit.attack_name != LIGHT_WAVE_ROW && (hit.flags & 0xFF == SHOTLOCK_COMMAND || hit.attack_name.starts_with("SF_"))
}

/// (XSI) Light Wave's attack row (kh-core abilities/xs): a ShootFlow row name, but no shotlock.
pub const LIGHT_WAVE_ROW: &str = "SF_LFSHOOT";

/// What one target has been asked by the shotlock that is hitting it now.
#[derive(Clone, Copy, Debug)]
struct Volley {
    target: u32,
    last_step: u64,
    asked_kh: f32,
    capped: bool,
}

/// The shotlocks under way, per target (`shotlock_hit`).
#[derive(Debug, Default)]
pub struct ShotlockLedger {
    volleys: Vec<Volley>,
}

/// Sora's fixed step (kh-core: 60 Hz).
const STEPS_PER_SECOND: f32 = 60.0;

impl ShotlockLedger {
    pub const fn new() -> ShotlockLedger {
        ShotlockLedger { volleys: Vec::new() }
    }

    /// The KH3 damage a shotlock hit of `kh` on `target` (maximum HP `max_hp`) asks for in Sora's step `step`, and
    /// whether the cap cut it. A hit the cap has emptied still lands (its reaction, sound and effect) with the least
    /// damage there is (1 HP asked).
    pub fn adjust(&mut self, rules: &ShotlockRules, out_scale: f32, target: u32, max_hp: i32, step: u64, kh: f32) -> (f32, bool) {
        if !(kh > 0.0) {
            return (kh, false);
        }
        let gap = (rules.gap * STEPS_PER_SECOND) as u64;
        self.volleys.retain(|v| step.saturating_sub(v.last_step) <= gap);
        let scaled = kh * rules.scale;
        let cap = (rules.cap_share > 0.0 && max_hp > 0 && max_hp >= rules.cap_min_hp).then(|| rules.cap_share * max_hp as f32 / out_scale.max(1e-6));
        let at = match self.volleys.iter().position(|v| v.target == target) {
            Some(at) => at,
            None => {
                self.volleys.push(Volley { target, last_step: step, asked_kh: 0.0, capped: false });
                self.volleys.len() - 1
            }
        };
        let v = &mut self.volleys[at];
        v.last_step = step;
        let give = match cap {
            Some(cap) => scaled.min((cap - v.asked_kh).max(0.0)),
            None => scaled,
        };
        v.asked_kh += give;
        let cut = give < scaled;
        let first_cut = cut && !v.capped;
        v.capped |= cut;
        (give.max(MIN_KH), first_cut)
    }
}

/// The least KH3 damage a hit that does damage asks for (`damage_out` makes 1 HP of it).
const MIN_KH: f32 = 0.01;

/// THE ONE LEDGER of the game thread's hits (targets.rs `apply_hit`).
pub static SHOTLOCK_LEDGER: std::sync::Mutex<ShotlockLedger> = std::sync::Mutex::new(ShotlockLedger::new());

/// (IMPL_ER61) A hit as Elden Ring is to be asked for it: a shotlock's hit scaled by `damage.shotlock.scale` and
/// capped per target per shotlock (`damage.shotlock.cap_*`); every other hit unchanged. The second value: a line for
/// the log when this hit is the first the cap cut on its target.
pub fn shotlock_hit(params: &Params, hit: &kh_core::HitRecord, max_hp: i32, step: u64) -> (kh_core::HitRecord, Option<String>) {
    if !is_shotlock(hit) || params.shotlock == ShotlockRules::OFF {
        return (*hit, None);
    }
    let Ok(mut ledger) = SHOTLOCK_LEDGER.lock() else { return (*hit, None) };
    let rules = params.shotlock;
    let (damage, first_cut) = ledger.adjust(&rules, params.out_scale, hit.target.0, max_hp, step, hit.damage);
    let note = first_cut.then(|| {
        format!(
            "(IMPL_ER61) shotlock '{}' on target {} (max HP {max_hp}): this shotlock has asked {:.0} % of its maximum (damage.shotlock.cap_share): its further hits ask 1 HP",
            hit.attack_name, hit.target.0, rules.cap_share * 100.0
        )
    });
    (kh_core::HitRecord { damage, ..*hit }, note)
}

/// (IMPL_ER19) The host's choices about KH3's prize orbs in Elden Ring (sheet rows `prize.*` of sora-er-host;
/// the control keys of prize.rs override them). None of these is data of either game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrizeRules {
    /// `prize.spawn_height`: cm above the dead enemy's CENTRE where its orbs burst out (KH3: the pawn's location
    /// after a per-pawn offset that was not read; 0 = the centre).
    pub spawn_height: f32,
    /// `prize.glow`: a factor on the brightness of the orbs' effects (1 = as authored under the effects' exposure rule).
    pub glow: f32,
    /// `prize.size`: a factor on the size of the orbs' effects (the picture only; 1 = KH3's component scales).
    pub size: f32,
    /// `prize.award_wait`: seconds a kill whose row names a rune value waits for the game's award before that value is paid.
    pub award_wait: f32,
    /// `prize.award_wait_unpriced`: seconds a kill WITHOUT a rune value (a boss, a rune-less enemy) waits for the
    /// game's award before it is closed without munny.
    pub award_wait_unpriced: f32,
    /// `prize.munny_expire_pays`: 1 = the munny of a munny orb that vanishes uncollected is given all the same
    /// (0 = KH3: lost).
    pub munny_expire_pays: bool,
    /// `prize.log_flight`: 1 = each orb's flight is told in the log when it ends.
    pub log_flight: bool,
    /// (IMPL_ER20) `prize.hp.small` / `prize.hp.large`: the hit points an HP orb gives in this host (KH3: 1 / 5;
    /// the user, 2026-10-13: "Scale up the HP orbs a tiny bit, maybe to +20 max": 20 for the large one, KH3's 1 : 5 kept).
    pub hp_small: i32,
    pub hp_large: i32,
}

impl PrizeRules {
    pub const DEFAULT: PrizeRules = PrizeRules { spawn_height: 80.0, glow: 3.27, size: 1.0, award_wait: 5.0, award_wait_unpriced: 20.0, munny_expire_pays: false, log_flight: true, hp_small: 4, hp_large: 20 };
}

/// How many id ranges `own.anim.*` can hold.
pub const OWN_ANIMS: usize = 32;

/// `a`, `a-b`, several separated by commas: the id ranges of a row (or of the key `own.anims`).
pub fn own_ranges(text: &str) -> Vec<(i32, i32)> {
    text.split(',')
        .filter_map(|part| {
            let part = part.trim();
            let (a, b) = part.split_once('-').unwrap_or((part, part));
            let (a, b) = (a.trim().parse::<i32>().ok()?, b.trim().parse::<i32>().ok()?);
            (a >= 0 && b >= a).then_some((a, b))
        })
        .collect()
}

/// The sheet's `own.anim.*` rows (kept equal by the test below).
fn default_own_anims() -> [(i32, i32); OWN_ANIMS] {
    let mut o = [(0, -1); OWN_ANIMS];
    o[..OWN_ANIM_DEFAULTS.len()].copy_from_slice(&OWN_ANIM_DEFAULTS);
    o
}
fn default_own_moves() -> [(i32, i32); OWN_ANIMS] {
    let mut o = [(0, -1); OWN_ANIMS];
    o[..OWN_MOVE_DEFAULTS.len()].copy_from_slice(&OWN_MOVE_DEFAULTS);
    o
}
const OWN_MOVE_DEFAULTS: [(i32, i32); 2] = [(60060, 60060), (60030, 60030)];
const OWN_ANIM_DEFAULTS: [(i32, i32); 7] = [(60071, 60071), (26011, 26011), (68000, 68000), (26031, 26031), (68010, 68012), (68050, 68050), (68211, 68212)];

impl Default for Params {
    fn default() -> Params {
        Params {
            out_scale: 7.0,
            in_scale: 1.0,
            blow_share: 0.25,
            min_share: 0.004,
            exp_per_max_hp: 0.03,
            range: 4000.0,
            engage_range: 800.0,
            small: Class { max_radius_cm: 35.0, max_height_cm: 110.0, push_level: 2, puppet: true },
            medium: Class { max_radius_cm: 80.0, max_height_cm: 260.0, push_level: 5, puppet: true },
            large: Class { max_radius_cm: 100_000.0, max_height_cm: 100_000.0, push_level: 8, puppet: false },
            poise_armoured: 120.0,
            hp_armoured: 1500.0,
            overrides: default_overrides(),
            reactions: default_reactions(),
            poise_scale: 1.0,
            landing_level: 7,
            body_hp: 10.0,
            scripted_death: [4690, 10, 1],
            point_max_height: 100.0,
            terrain_slope: 30.0,
            gauge_hp_scale: 1.0,
            grace_flame_sfx: [1010, 1011],
            own_anims: default_own_anims(),
            own_moves: default_own_moves(),
            collision_radius: 32.0,
            collision_gap: 10.0,
            collision_floor_layer: 0x5c, // (WALLS load fix) the coordinator's live sheet value: floors also from 0x5c (union with layer 2)
            collision_layer: 2,       // (WALLS)
            collision_foot: 40.0,
            hover_min: 60.0,
            prize: PrizeRules::DEFAULT,
            savepoint_sfx: 1,
            ground: crate::savepoint_rules::Ground::DEFAULT,
            guard: crate::bossguard::GuardRules::DEFAULT,
            poise: crate::poise::PoiseRules::DEFAULT,
            finish: crate::finish::FinishRules::DEFAULT,
            anim: AnimRules { timings: default_timings(), ..AnimRules::DEFAULT },
            breaking: crate::breakables::BreakRules::DEFAULT,
            shotlock: ShotlockRules::DEFAULT,
            mp_cost_scale: 1.5, // (user 2026-10-22) "use the 1.5 multiplier as the base cost, since there is no difficulty select"
            diff_in: 1.0,                // (DF)
            mp_cost_scale_critical: 1.0, // (DF)
            new_game_level: 1,           // (DF, dec032) Standard
        }
    }
}

impl Params {
    /// The sheet's rows over the defaults; the second value lists rows that were not understood.
    pub fn from_sheet(text: &str) -> Result<(Params, Vec<String>), String> {
        let json: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let rows = json.get("rows").and_then(|r| r.as_array()).ok_or("no rows")?;
        let mut p = Params { own_anims: [(0, -1); OWN_ANIMS], own_moves: [(0, -1); OWN_ANIMS], overrides: [None; OVERRIDES], anim: AnimRules { timings: [None; ANIM_TIMINGS], ..AnimRules::DEFAULT }, ..Params::default() };
        let mut problems = Vec::new();
        for row in rows {
            let (Some(id), Some(value)) = (row.get(0).and_then(|v| v.as_str()), row.get(1)) else { continue };
            let number = value.as_f64().map(|v| v as f32).filter(|v| v.is_finite());
            let class = |c: &mut Class| -> bool {
                let get = |k: &str| value.get(k).and_then(|v| v.as_f64()).map(|v| v as f32);
                match (get("max_radius_cm"), get("max_height_cm"), get("push_level")) {
                    (Some(r), Some(h), Some(l)) => {
                        *c = Class { max_radius_cm: r, max_height_cm: h, push_level: l.clamp(0.0, 9.0) as u8, puppet: value.get("puppet").and_then(|v| v.as_bool()).unwrap_or(c.puppet) };
                        true
                    }
                    _ => false,
                }
            };
            if let Some(name) = id.strip_prefix("react.kind.") {
                let get = |k: &str| value.get(k).and_then(|v| v.as_f64()).map(|v| v as f32);
                match (KIND_NAMES.iter().find(|(n, _)| *n == name), get("level"), get("air_level"), get("poise")) {
                    (Some((_, kind)), Some(level), Some(air), Some(poise)) => p.reactions[*kind as usize] = Reaction { level: level as u8, air_level: air as u8, poise },
                    _ => problems.push(id.to_string()),
                }
                continue;
            }
            if let Some(what) = id.strip_prefix("react.anim.") {
                let get = |k: &str| value.get(k).and_then(|v| v.as_f64()).map(|v| v as f32);
                let ok = match (what, value.as_str(), number) {
                    ("flinch_levels", Some(text), _) => levels(text).map(|l| p.anim.flinch_levels = l).is_some(),
                    ("blow_levels", Some(text), _) => levels(text).map(|l| p.anim.blow_levels = l).is_some(),
                    ("air_at", _, Some(v)) => { p.anim.air_at = v; true }
                    ("down_at", _, Some(v)) => { p.anim.down_at = v; true }
                    (who, _, _) => match (who.strip_prefix('c').and_then(|n| n.parse::<i32>().ok()), get("air_at"), get("down_at"), p.anim.timings.iter().position(|t| t.is_none())) {
                        (Some(chr), Some(air_at), Some(down_at), Some(free)) => { p.anim.timings[free] = Some(AnimTiming { chr, air_at, down_at }); true }
                        _ => false,
                    },
                };
                if !ok {
                    problems.push(id.to_string());
                }
                continue;
            }
            // (DF) the difficulty of a new game, by name
            if id == "difficulty.new_game" {
                match value.as_str().map(|t| t.trim().to_ascii_uppercase()).and_then(|t| ["BEGINNER", "STANDARD", "PROUD", "CRITICAL"].iter().position(|n| *n == t)) {
                    Some(level) => p.new_game_level = level as u8,
                    None => problems.push(id.to_string()),
                }
                continue;
            }
            if id.starts_with("own.anim.") || id.starts_with("own.move.") {
                // (a number, or text: `a`, `a-b`, several separated by commas)
                let text = value.as_str().map(str::to_string).or(number.map(|n| format!("{}", n as i64))).unwrap_or_default();
                let ranges = own_ranges(&text);
                let mut ok = !ranges.is_empty();
                for r in ranges {
                    let list = if id.starts_with("own.move.") { &mut p.own_moves } else { &mut p.own_anims };
                    match list.iter().position(|o| o.1 < o.0) {
                        Some(free) => list[free] = r,
                        None => ok = false,
                    }
                }
                if !ok {
                    problems.push(id.to_string());
                }
                continue;
            }
            if let Some(who) = id.strip_prefix("target.npc.") {
                let class = value.as_str().and_then(|v| match v {
                    "puppet" => Some(true),
                    "armoured" => Some(false),
                    _ => None,
                });
                let key = match who.strip_prefix('c') {
                    Some(n) => n.parse::<i32>().ok().map(|n| (true, n)),
                    None => who.parse::<i32>().ok().map(|n| (false, n)),
                };
                match (class, key, p.overrides.iter().position(|o| o.is_none())) {
                    (Some(puppet), Some((by_chr, id)), Some(free)) => p.overrides[free] = Some(Override { by_chr, id, puppet }),
                    _ => problems.push(id.to_string()),
                }
                continue;
            }
            let ok = match (id, number) {
                ("damage.out.poise_scale", Some(v)) if v >= 0.0 => { p.poise_scale = v; true }
                ("damage.in.body_hp", Some(v)) if v >= 1.0 => { p.body_hp = v; true }
                ("death.scripted.chr", Some(v)) => { p.scripted_death[0] = v as i32; true }
                ("death.scripted.area", Some(v)) => { p.scripted_death[1] = v as i32; true }
                ("death.scripted.block", Some(v)) => { p.scripted_death[2] = v as i32; true }
                ("collision.radius", Some(v)) if (5.0..=100.0).contains(&v) => { p.collision_radius = v; true }
                ("collision.gap", Some(v)) if (0.0..=30.0).contains(&v) => { p.collision_gap = v; true }
                ("collision.floor_layer", Some(v)) if (0.0..=127.0).contains(&v) && v.fract() == 0.0 => { p.collision_floor_layer = v as u32; true }
                ("collision.layer", Some(v)) if (0.0..=127.0).contains(&v) && v.fract() == 0.0 => { p.collision_layer = v as u32; true } // (WALLS)
                ("collision.foot", Some(v)) if (0.0..=60.0).contains(&v) => { p.collision_foot = v; true }
                ("target.point.hover_min", Some(v)) if (0.0..=1000.0).contains(&v) => { p.hover_min = v; true }
                ("prize.spawn_height", Some(v)) if (-200.0..=500.0).contains(&v) => { p.prize.spawn_height = v; true }
                ("prize.glow", Some(v)) if (0.0..=100.0).contains(&v) => { p.prize.glow = v; true }
                ("prize.size", Some(v)) if (0.05..=10.0).contains(&v) => { p.prize.size = v; true }
                ("prize.award_wait", Some(v)) if (0.5..=60.0).contains(&v) => { p.prize.award_wait = v; true }
                ("prize.award_wait_unpriced", Some(v)) if (0.5..=120.0).contains(&v) => { p.prize.award_wait_unpriced = v; true }
                ("prize.munny_expire_pays", Some(v)) => { p.prize.munny_expire_pays = v != 0.0; true }
                ("prize.log_flight", Some(v)) => { p.prize.log_flight = v != 0.0; true }
                ("prize.hp.small", Some(v)) if (1.0..=9999.0).contains(&v) => { p.prize.hp_small = v as i32; true }
                ("prize.hp.large", Some(v)) if (1.0..=9999.0).contains(&v) => { p.prize.hp_large = v as i32; true }
                ("savepoint.sfx", Some(v)) if (0.0..=2.0).contains(&v) => { p.savepoint_sfx = v as u8; true }
                ("react.poise.basic", Some(v)) if (0.0..=1.0).contains(&v) => { p.guard.poise_basic = v; true }
                ("react.poise.max_share", Some(v)) if v > 0.0 => { p.guard.poise_max_share = v; true }
                ("react.poise.lockout", Some(v)) if (0.0..=600.0).contains(&v) => { p.guard.poise_lockout = v; true }
                ("react.revenge.giant", Some(v)) if (0.0..=100.0).contains(&v) => { p.guard.revenge_giant = v; true }
                ("react.revenge.heavy", Some(v)) if (0.0..=100.0).contains(&v) => { p.guard.revenge_heavy = v; true }
                ("react.revenge.armour", Some(v)) if (0.0..=60.0).contains(&v) => { p.guard.revenge_armour = v; true }
                ("react.endurance.lockout", Some(v)) if (0.0..=600.0).contains(&v) => { p.guard.endurance_lockout = v; true }
                ("react.chain.max", Some(v)) if (0.0..=100.0).contains(&v) => { p.guard.chain_max = v as u32; true }
                ("react.chain.window", Some(v)) if (0.0..=600.0).contains(&v) => { p.guard.chain_window = v; true }
                ("react.chain.cooldown", Some(v)) if (0.0..=600.0).contains(&v) => { p.guard.chain_cooldown = v; true }
                ("target.point.max_height", Some(v)) if v > 0.0 => { p.point_max_height = v; true }
                ("target.point.terrain_slope", Some(v)) if (0.0..=60.0).contains(&v) => { p.terrain_slope = v; true }
                ("hud.enemy_gauge.hp_scale", Some(v)) if v > 0.0 => { p.gauge_hp_scale = v; true }
                ("grace.flame.sfx.undiscovered", Some(v)) if v > 0.0 => { p.grace_flame_sfx[0] = v as i32; true }
                ("grace.flame.sfx.discovered", Some(v)) if v > 0.0 => { p.grace_flame_sfx[1] = v as i32; true }
                ("react.landing_level", Some(v)) if v >= 0.0 => { p.landing_level = v as u8; true }
                ("target.poise_armoured", Some(v)) => { p.poise_armoured = v; true }
                ("target.hp_armoured", Some(v)) => { p.hp_armoured = v; true }
                ("damage.out.scale", Some(v)) if v > 0.0 => { p.out_scale = v; true }
                ("magic.mp_cost_scale", Some(v)) if (0.1..=10.0).contains(&v) => { p.mp_cost_scale = v; true } // (IMPL_ER61)
                ("magic.mp_cost_scale.critical", Some(v)) if (0.1..=10.0).contains(&v) => { p.mp_cost_scale_critical = v; true } // (DF)
                (key, Some(v)) if key.starts_with("damage.shotlock.") => p.shotlock.apply_row(key, v), // (IMPL_ER61)
                ("damage.in.scale", Some(v)) if v >= 0.0 => { p.in_scale = v; true }
                ("damage.in.blow_share", Some(v)) => { p.blow_share = v; true }
                ("damage.in.min_share", Some(v)) => { p.min_share = v; true }
                ("exp.per_max_hp", Some(v)) if v >= 0.0 => { p.exp_per_max_hp = v; true }
                ("target.range", Some(v)) if v > 0.0 => { p.range = v; true }
                ("target.engage_range", Some(v)) => { p.engage_range = v; true }
                ("react.slide.small", Some(v)) if v >= 0.0 => { p.anim.slide[0] = v; true }
                ("react.slide.medium", Some(v)) if v >= 0.0 => { p.anim.slide[1] = v; true }
                ("react.slide.large", Some(v)) if v >= 0.0 => { p.anim.slide[2] = v; true }
                ("target.flying_height", Some(v)) => { p.anim.flying_height = v; true }
                ("target.class.small", _) => class(&mut p.small),
                ("target.class.medium", _) => class(&mut p.medium),
                ("target.class.large", _) => class(&mut p.large),
                (key, Some(v)) if key.starts_with("finish.") => p.finish.apply_row(key, v),
                (key, Some(v)) if key.starts_with("savepoint.ground_") => p.ground.apply_row(key, v), // (IMPL_ER52)
                (key, Some(v)) if key.starts_with("poise.") => p.poise.apply_row(key, v),
                (key, Some(v)) if key.starts_with("break.") || key.starts_with("damage.element.") => p.breaking.apply_row(key, v),
                (key, Some(_)) if key.starts_with("blood.") => true, // (IMPL_ER60) read by blood.rs
                _ => false,
            };
            if !ok {
                problems.push(id.to_string());
            }
        }
        Ok((p, problems))
    }

    /// Elden Ring hit points a hit of `kh_damage` takes off a target (at least 1 for a hit that
    /// does any damage).
    pub fn damage_out(&self, kh_damage: f32) -> i32 {
        if !(kh_damage > 0.0) {
            return 0;
        }
        ((kh_damage * self.out_scale).round() as i32).max(1)
    }

    /// The KH3 damage number that corresponds to `er_hp` Elden Ring hit points (the inverse, for
    /// reporting what a hit dealt in the core's units).
    pub fn damage_out_inverse(&self, er_hp: i32) -> f32 {
        er_hp as f32 / self.out_scale.max(1e-6)
    }

    /// What a hit that took `er_damage` of the Tarnished's `er_max_hp` takes off Sora (max `sora_max_hp`).
    pub fn damage_in(&self, er_damage: i32, er_max_hp: i32, sora_max_hp: i32) -> f32 {
        if er_damage <= 0 || er_max_hp <= 0 {
            return 0.0;
        }
        // (DF) x the difficulty (`diff_in`; 1 on Standard)
        (er_damage as f32 / er_max_hp as f32 * sora_max_hp as f32 * self.in_scale * self.diff_in.max(0.0)).max(if self.in_scale > 0.0 { 1.0 } else { 0.0 })
    }

    /// (DF) A STATUS TICK (poison, rot ...): the share rule WITHOUT the difficulty (a max-HP-proportional effect; KH3's
    /// such damage skips the difficulty factor, 0x140632240 step 2).
    pub fn damage_in_tick(&self, er_damage: i32, er_max_hp: i32, sora_max_hp: i32) -> f32 {
        Params { diff_in: 1.0, ..*self }.damage_in(er_damage, er_max_hp, sora_max_hp)
    }

    /// What kind of hit an Elden Ring loss is: `None` = damage over time (no reaction).
    pub fn kind_in(&self, er_damage: i32, er_max_hp: i32) -> Option<ETresDamageKind> {
        let share = er_damage as f32 / er_max_hp.max(1) as f32;
        if share < self.min_share {
            None
        } else if share >= self.blow_share {
            Some(ETresDamageKind::Blow)
        } else {
            Some(ETresDamageKind::Small)
        }
    }

    /// The base EXP of a defeated enemy of `max_hp` Elden Ring hit points.
    pub fn exp_of(&self, max_hp: i32) -> i32 {
        if self.exp_per_max_hp <= 0.0 || max_hp <= 0 {
            return 0;
        }
        (max_hp as f32 * self.exp_per_max_hp).ceil() as i32
    }

    pub fn weight(&self, radius_cm: f32, height_cm: f32) -> Weight {
        if radius_cm <= self.small.max_radius_cm && height_cm <= self.small.max_height_cm {
            Weight::Small
        } else if radius_cm <= self.medium.max_radius_cm && height_cm <= self.medium.max_height_cm {
            Weight::Medium
        } else {
            Weight::Large
        }
    }

    /// The reaction asked for a hit of `kind` (an `ETresDamageKind` value).
    pub fn reaction(&self, kind: i32) -> Reaction {
        self.reactions.get(kind.max(0) as usize).copied().unwrap_or(self.reactions[1])
    }

    /// THE CLASS TABLE: may this body be launched and juggled? An explicit row of its character id
    /// or NpcParam id decides first; then its size class, its poise and its hit points.
    pub fn puppetable(&self, weight: Weight, poise_max: f32, hp_max: i32, chr_id: i32, npc_param: i32) -> bool {
        if let Some(o) = self.overrides.iter().flatten().find(|o| if o.by_chr { o.id == chr_id } else { o.id == npc_param }) {
            return o.puppet;
        }
        self.class(weight).puppet && poise_max <= self.poise_armoured && hp_max as f32 <= self.hp_armoured
    }

    pub fn class(&self, weight: Weight) -> Class {
        match weight {
            Weight::Small => self.small,
            Weight::Medium => self.medium,
            Weight::Large => self.large,
        }
    }
}

/// THE FALLBACK HOSTILITY RULE by Elden Ring team type, used only when the game's own "can
/// target" function was not located (targets.rs): the enemy teams. Team numbers as the community
/// tables name them (6 Enemy, 7 StrongEnemy / boss, 9 AngryFriend = an NPC turned hostile,
/// 13 / 16 - 18 invaders, 21 hostile to all). NOT verified against the game's team table.
pub fn hostile_team(team: u8) -> bool {
    matches!(team, 6 | 7 | 9 | 13 | 16 | 17 | 18 | 21)
}

/// (DF, 2026-10-15) THE DIFFICULTY FACTOR ON ELDEN RING'S HITS (`Params::diff_in`) for a game level, from the sheet
/// `sora-difficulty`: `enemy_damage` (0x140642820: x0.5 / 1 / 1.5 / 2) x `base_hp` of Standard / `base_hp` of the level
/// (0x140575D73: 90 / 45 on Critical). WHY THE RATIO: in KH3 a hit takes a NUMBER of HP, so a halved maximum makes every hit
/// twice the share; the Elden Ring route takes a SHARE of the maximum (`damage_in`), where a halved maximum alone would
/// change nothing. Beginner 0.5, Standard 1, Proud 1.5, Critical 4.
pub fn difficulty_in(level: kh_core::status::GameLevel) -> f32 {
    let (row, standard) = (level.row(), kh_core::status::GameLevel::Standard.row());
    row.enemy_damage * standard.base_hp.max(1) as f32 / row.base_hp.max(1) as f32
}

/// The mirror of Sora's HP in the Tarnished's HP bar: the same share, never 0 while he lives.
pub fn mirror_hp(sora_hp: i32, sora_max: i32, er_max: i32) -> i32 {
    if sora_hp <= 0 {
        return 0;
    }
    ((sora_hp as f64 / sora_max.max(1) as f64 * er_max as f64).ceil() as i32).clamp(1, er_max.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_is_the_defaults() {
        let text = include_str!("../../../sheets/port/sora-er-host.json");
        let (p, problems) = Params::from_sheet(text).unwrap();
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(p, Params::default());
    }

    #[test]
    fn damage_out_is_one_scale() {
        let p = Params { out_scale: 2.5, ..Params::default() };
        assert_eq!(Params::default().out_scale, 7.0);
        assert_eq!(p.damage_out(100.0), 250);
        assert_eq!(p.damage_out(0.1), 1);
        assert_eq!(p.damage_out(0.0), 0);
        assert!((p.damage_out_inverse(250) - 100.0).abs() < 1e-4);
    }

    /// (IMPL_ER61) The Ragnarok of the user's log (er\logs\kh-er.prev.log 2724.6 s): 24 bullets of 26 KH on c4630,
    /// max HP 2949 (it had 2246 left). Before: 182 asked a bullet, 4368 in all (the game took 2038).
    #[test]
    fn a_shotlock_is_scaled_and_capped_per_target() {
        let p = Params::default();
        assert_eq!(p.shotlock, ShotlockRules::DEFAULT);
        let mut ledger = ShotlockLedger::new();
        let mut asked = Vec::new();
        for k in 0..24u64 {
            let (kh, _) = ledger.adjust(&p.shotlock, p.out_scale, 130, 2949, 1000 + k, 26.0);
            asked.push(p.damage_out(kh));
        }
        // 13 KH -> 91 asked a bullet until 40 % of 2949 = 1180 asked; then 1 HP a bullet
        assert_eq!(asked[0], 91);
        let total: i32 = asked.iter().sum();
        assert!((1180..=1180 + 24).contains(&total), "{total} {asked:?}");
        assert_eq!(asked.iter().filter(|a| **a == 91).count(), 12);
        assert_eq!(*asked.last().unwrap(), 1);
        // the next shotlock (after the gap) starts afresh; another target has its own share
        let (kh, cut) = ledger.adjust(&p.shotlock, p.out_scale, 130, 2949, 1000 + 24 + 181, 26.0);
        assert_eq!((p.damage_out(kh), cut), (91, false));
        // a small enemy (under cap_min_hp) is scaled, never capped: 24 bullets x 91 = 2184 asked of a 1467 HP body
        let small: i32 = (0..24u64).map(|k| p.damage_out(ledger.adjust(&p.shotlock, p.out_scale, 60, 1467, 5000 + k, 26.0).0)).sum();
        assert_eq!(small, 24 * 91);
    }

    #[test]
    fn only_shotlock_hits_are_touched() {
        let p = Params::default();
        let mut hit = kh_core::HitRecord::empty(kh_core::TargetId(7));
        hit.damage = 20.0;
        hit.attack_name = "KB_STAB";
        assert_eq!(shotlock_hit(&p, &hit, 3000, 1).0.damage, 20.0);
        hit.attack_name = "SF_KCSHOT";
        assert_eq!(shotlock_hit(&p, &hit, 3000, 1).0.damage, 10.0);
        hit.attack_name = "";
        hit.flags = SHOTLOCK_COMMAND | 2 << 8;
        assert!(is_shotlock(&hit));
        let off = Params { shotlock: ShotlockRules::OFF, ..Params::default() };
        assert_eq!(shotlock_hit(&off, &hit, 3000, 1).0.damage, 20.0);
        assert_eq!(p.mp_cost_scale, 1.5); // (user 2026-10-22) the 1.5 is the base cost (no difficulty select)
    }

    /// (XSI) Light Wave's waves (row SF_LFSHOOT) are not shotlock hits: no nerf, no cap; Spectral Rays' rows are.
    #[test]
    fn xsi_light_wave_is_not_a_shotlock_but_spectral_rays_is() {
        let p = Params::default();
        let mut hit = kh_core::HitRecord::empty(kh_core::TargetId(7));
        hit.damage = 20.0;
        hit.attack_name = "SF_LFSHOOT";
        assert!(!is_shotlock(&hit));
        assert_eq!(shotlock_hit(&p, &hit, 3000, 1).0.damage, 20.0);
        for row in ["SF_REFLECTIVELASER", "SF_REFLECTIVELASER_RANGE"] {
            hit.attack_name = row;
            assert!(is_shotlock(&hit), "{row}");
        }
    }

    #[test]
    fn damage_in_is_the_same_share() {
        let p = Params::default();
        // 30 % of a Tarnished of 500 HP = 30 % of Sora's 265.
        assert!((p.damage_in(150, 500, 265) - 79.5).abs() < 1e-3);
        assert_eq!(p.kind_in(150, 500), Some(ETresDamageKind::Blow));
        assert_eq!(p.kind_in(60, 500), Some(ETresDamageKind::Small));
        assert_eq!(p.kind_in(1, 500), None);
    }

    #[test]
    fn mirror_keeps_one_point_while_alive() {
        assert_eq!(mirror_hp(265, 265, 500), 500);
        assert_eq!(mirror_hp(1, 265, 500), 2);
        assert_eq!(mirror_hp(0, 265, 500), 0);
        assert_eq!(mirror_hp(133, 265, 500), 251);
    }

    #[test]
    fn the_class_table() {
        let mut p = Params { overrides: [None; OVERRIDES], ..Params::default() };
        // A soldier flies; a troll (large) and a mounted boss (hit points) do not.
        assert!(p.puppetable(Weight::Medium, 51.0, 192, 4310, 43100000));
        assert!(!p.puppetable(Weight::Large, 80.0, 1100, 4600, 46000000));
        assert!(!p.puppetable(Weight::Medium, 80.0, 2889, 3251, 32510000));
        assert!(!p.puppetable(Weight::Medium, 300.0, 400, 1, 1));
        p.overrides[0] = Some(Override { by_chr: true, id: 3251, puppet: true });
        p.overrides[1] = Some(Override { by_chr: false, id: 43100000, puppet: false });
        assert!(p.puppetable(Weight::Large, 80.0, 2889, 3251, 32510000));
        assert!(!p.puppetable(Weight::Medium, 51.0, 192, 4310, 43100000));
        assert_eq!(p.reaction(99), p.reaction(1));
        assert_eq!(p.reaction(0).level, 0);
    }

    #[test]
    fn the_animation_rules() {
        let p = Params::default();
        assert_eq!(p.anim.flinch_levels, [2, 1, 8, 3]);
        assert_eq!(p.anim.blow_levels, [10, 4, 7, 6]);
        assert_eq!(p.anim.timing(4311), (0.3, 0.5));
        assert_eq!(p.anim.timing(9999), (p.anim.air_at, p.anim.down_at));
        assert_eq!(levels("2, 1,8,3"), Some([2, 1, 8, 3]));
        assert_eq!(levels("2,1"), None);
    }

    #[test]
    fn weights_by_size() {
        let p = Params::default();
        assert_eq!((p.exp_of(55), p.exp_of(192), p.exp_of(2889)), (2, 6, 87));
        assert_eq!(p.weight(25.0, 60.0), Weight::Small);
        assert_eq!(p.weight(40.0, 150.0), Weight::Medium);
        assert_eq!(p.weight(150.0, 600.0), Weight::Large);
    }
}

// (DF, 2026-10-15) the difficulty on the Elden Ring route: the factor, the host rows, the tables of IMPL_DF_STATE.md
#[cfg(test)]
#[path = "difficulty_tests.rs"]
mod difficulty_tests;

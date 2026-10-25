//! (GOOFY-HR, 2026-10-17) HIT REACTIONS FOR EVERY PLAYABLE PARTY CHARACTER: the shared hurt / blow / down / get-up /
//! air-recovery path of the party pawns (Donald, Goofy and any character added later: nothing here knows a name; a
//! character's reactions are its own KH3 anim set's damage rows, read by their KH3 row names).
//!
//! THE USER (2026-10-16 23:40, ER_PHASE_NOTES): "When Goofy takes a heavy hit he just is frozen in his hurt animation
//! for a second and then reverts to normal it sucks."
//!
//! THE CAUSE (evidence: build/sora_tmp/er/fix3/ref/kh-er_goofy_session.log, t = 102.407 s: `Goofy Damage M_0211`,
//! +0.117 s `M_0212`, +1.20 s `M_0213`, +1.80 s `Idle IdleB`): the friends' reaction is kh-react's `UTresAICharState_Damage`
//! (the enemies' model) and the pawn showed it with two mistakes:
//! 1. the reaction ran on the ENEMY MEDIAN clip lengths (`ClipTimes::MEDIAN`: the down clip 1.667 s, the ground
//!    flinch 1.0 s), not on the character's own clips (Goofy's M_0213 is 2.0 s, his flinches 1.17 - 1.33 s);
//! 2. the clips were mapped by PHASE with KH3's Blow(index, phase) misread as "0 thrown, 1 down, 2 get up": the
//!    lying phase played M_0212 - the 0.27 s FLIGHT LOOP, as a one-shot, so it stood on its last frame for ~0.8 s (the
//!    "frozen hurt animation") - and the real landing + get-up clip M_0213 (2.0 s) got only the last 0.6 s of the
//!    median's time, from its start, before Idle cut in (the "reverts to normal").
//!
//! THE RULE NOW (KH3's own functions):
//! - `Blow(index, phase)` = `ATresNpcPawnBase` 0x14125FC00: M_0211 + 3 x index + phase, phase 0 START, 1 the flight
//!   LOOP, 2 landing + lying + GET-UP in one clip (as Sora's 0x1414333A0, combat spec 9.4); the loop plays looped (Sora's
//!   handler plays it with mode 3);
//! - the friends' launch / slam clips = `UTresNpcState_Damage` 0x141246960: 0 the fall (M_0230 / M_0237 / M_0212),
//!   1 M_0227, 2 M_0228, 3 the down (M_0234 / M_0238 / M_0213); its Toss handler 0x141251570 plays M_0227 (one-shot) ->
//!   M_0228 (loop) -> M_0237 (loop) -> M_0238 else M_0213 at the landing, and ends with that clip;
//! - `Small(index, variant)` = 0x1412646C0 (M_0201 + index, M_0205 / M_0206 for every second front / back hit; air
//!   M_0207 + index); `Wall` = 0x141262420 (M_0223 / M_0224, then the fall loop, then Land);
//! - every phase ends WHEN ITS OWN CLIP ENDS: the reaction's clip times are the character's clip lengths (per hit, for
//!   the direction index and variant of that hit);
//! - Aerial Recovery from the hit + 0.2 s while thrown (Sora's `RECOVER_DELAY`, Damage Setup 0x141365930);
//! - the leader (pad) may leave a damage clip at its EnableChange (KH3's exits rule 0x14135E188) - else at its
//!   CheckBadStat, where KH3 lets a status take over the get-up - with the stick or B: the KH3 "quick get-up";
//! - a lethal heavy hit plays the blow and the KO starts at the down clip's CheckBadStat (port design, the brief);
//! - KH3's system hit effect `VS_non_HIT0` at the contact of a hit that took HP (sora-combat-hit-effects effect_02).
//!
//! Numbers: sheet `sheets/port/party-hurt.json` (generated `party_hurt.rs`). Physics: kh-react's handlers on the fixed
//! step (every brake `factor^(dt x 60)`; tests below show 60 Hz and 120 Hz give the same throw).

use glam::Vec3;
use kh_react::{ClipTimes, Phase, ReactClip};

use crate::anim::{AnimSetTable, ClipDef};
use crate::pawn::NpcPawn;
use crate::states::NpcStateId;

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_hurt.rs"));

/// The sheet's numbers.
pub const HP: PartyHurtParams = PARTY_HURT_PARAMS;

/// KH3's system hit effect (`/Game/Effects/system/Hit/par/VS_non_HIT0`): the chooser's effect for a character hit of no
/// attribute (sora-combat-hit-effects effect_02, `HitEffects` 0x1406294F0 / chooser 0x140628960).
pub const IMPACT_EFFECT: &str = "VS_non_HIT0";

/// What the shared path keeps on a pawn (`NpcPawn::hurt`).
#[derive(Clone, Debug, PartialEq)]
pub struct HurtRun {
    /// A lethal heavy hit's blow is playing: the KO comes at its down clip's mark; no hit lands meanwhile.
    pub dying: bool,
    /// A hit took HP this step: KH3's hit effect at this point (spawned at the next step: a hit has no host).
    pub impact: Option<Vec3>,
    /// Sora owns AIR_RECOVERY (the leader feed, `Party::step`; true without a feed, as the kits' own flags).
    pub air_recovery: bool,
    /// What the path did, newest last (tests, the trace): "start", "clip", "leader_exit", "air_recovery", "ko".
    pub log: Vec<(&'static str, &'static str)>,
}

impl Default for HurtRun {
    fn default() -> HurtRun {
        HurtRun { dying: false, impact: None, air_recovery: true, log: Vec::new() }
    }
}

impl HurtRun {
    fn note(&mut self, what: &'static str, clip: &'static str) {
        if self.log.len() >= 64 {
            self.log.remove(0);
        }
        self.log.push((what, clip));
    }
}

// ------------------------------------------------------------------------------------------- the character's clips

/// A character's anim set as the reaction needs it (the generated tables; a mock in the tests).
pub trait ClipSet {
    fn clip(&self, name: &str) -> Option<&'static ClipDef>;
}

impl ClipSet for AnimSetTable {
    fn clip(&self, name: &str) -> Option<&'static ClipDef> {
        self.get(name)
    }
}

/// A plain list of clips (tests: a future character's set).
pub struct Clips(pub &'static [ClipDef]);

impl ClipSet for Clips {
    fn clip(&self, name: &str) -> Option<&'static ClipDef> {
        self.0.iter().find(|c| c.name == name)
    }
}

/// KH3's damage rows by number (M_0201 .. M_0240).
fn m(n: u32) -> &'static str {
    const NAMES: [&str; 40] = [
        "M_0201", "M_0202", "M_0203", "M_0204", "M_0205", "M_0206", "M_0207", "M_0208", "M_0209", "M_0210", "M_0211", "M_0212", "M_0213", "M_0214", "M_0215", "M_0216", "M_0217", "M_0218", "M_0219", "M_0220", "M_0221", "M_0222",
        "M_0223", "M_0224", "M_0225", "M_0226", "M_0227", "M_0228", "M_0229", "M_0230", "M_0231", "M_0232", "M_0233", "M_0234", "M_0235", "M_0236", "M_0237", "M_0238", "M_0239", "M_0240",
    ];
    NAMES.get(n.wrapping_sub(201) as usize).copied().unwrap_or("")
}

fn row(set: &dyn ClipSet, n: u32) -> Option<&'static ClipDef> {
    set.clip(m(n))
}

/// `Small(index, variant)` on the ground (0x1412646C0), falling back to the index-0 row (FUN_1405ACE80).
fn small_ground(set: &dyn ClipSet, index: u8, variant: bool) -> Option<&'static ClipDef> {
    let i = index.min(3) as u32;
    let n = match (i, variant) {
        (0, true) => 205,
        (1, true) => 206,
        _ => 201 + i,
    };
    row(set, n).or_else(|| row(set, 201 + i)).or_else(|| row(set, 201))
}

fn small_air(set: &dyn ClipSet, index: u8) -> Option<&'static ClipDef> {
    row(set, 207 + index.min(3) as u32).or_else(|| row(set, 207))
}

/// `Blow(index, phase)` (0x14125FC00): phase 0 start, 1 the flight loop, 2 the down + get-up.
fn blow(set: &dyn ClipSet, index: u8, phase: u32) -> Option<&'static ClipDef> {
    row(set, 211 + 3 * index.min(3) as u32 + phase).or_else(|| row(set, 211 + phase))
}

/// 0x141246960 phase 3: the down of a launch / slam (M_0234, else M_0238, else M_0213).
fn launch_down(set: &dyn ClipSet) -> Option<&'static ClipDef> {
    row(set, 234).or_else(|| row(set, 238)).or_else(|| blow(set, 0, 2))
}

/// 0x141246960 phase 0 as the Toss handler uses it after the hover: M_0237, else the flight loop.
fn launch_fall(set: &dyn ClipSet) -> Option<&'static ClipDef> {
    row(set, 237).or_else(|| blow(set, 0, 1))
}

/// The slam's fall (0x141246960 phase 0: M_0230, else M_0237, else M_0212).
fn beat_fall(set: &dyn ClipSet) -> Option<&'static ClipDef> {
    row(set, 230).or_else(|| launch_fall(set))
}

/// `Wall(index)` phase 0 (0x141262420): M_0224 for index 1, else M_0223.
fn wall(set: &dyn ClipSet, index: u8) -> Option<&'static ClipDef> {
    row(set, if index == 1 { 224 } else { 223 }).or_else(|| row(set, 223))
}

fn named(set: &dyn ClipSet, name: &str) -> Option<&'static ClipDef> {
    set.clip(name)
}

/// The clip a reaction clip family plays, and whether it loops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    pub clip: &'static ClipDef,
    pub looped: bool,
}

fn once(c: Option<&'static ClipDef>) -> Option<Pick> {
    c.map(|clip| Pick { clip, looped: false })
}

fn looped(c: Option<&'static ClipDef>) -> Option<Pick> {
    c.map(|clip| Pick { clip, looped: true })
}

/// THE CLIP of what the reaction is doing (`clip`: kh-react's family; `phase` for the step before the handler's
/// first play). None: keep what plays (the bad-status hold).
pub fn pick(set: &dyn ClipSet, clip: ReactClip, phase: Phase, index: u8, variant: bool) -> Option<Pick> {
    use ReactClip as C;
    match clip {
        C::SmallGround => once(small_ground(set, index, variant)),
        C::SmallAir => once(small_air(set, index)),
        C::Land | C::WallDown => once(named(set, "Land")),
        C::FallLoop | C::WallFall => looped(named(set, "JumpNormalDownLoop")),
        C::BlowStart => once(blow(set, index, 0)),
        C::BlowLoop | C::BlowFallLoop | C::Spin => looped(blow(set, index, 1)),
        C::BlowDown | C::SpinDown => once(blow(set, index, 2)),
        C::TossStart => once(row(set, 227).or_else(|| blow(set, 0, 0))),
        C::TossRiseLoop => looped(row(set, 228).or_else(|| blow(set, 0, 1))),
        C::TossFallStart | C::TossFallLoop => looped(launch_fall(set)),
        C::TossDown | C::BeatDown => once(launch_down(set)),
        C::BeatFall => looped(beat_fall(set)),
        C::WallHit => once(wall(set, index)),
        C::Held => None,
        C::None => match phase {
            Phase::None | Phase::FlinchGround => once(small_ground(set, index, variant)),
            Phase::FlinchAir => once(small_air(set, index)),
            Phase::LaunchedRising | Phase::LaunchedHover => once(row(set, 227).or_else(|| blow(set, 0, 0))),
            Phase::SlammedDown => looped(beat_fall(set)),
            Phase::Frozen | Phase::Stunned => None,
            _ => once(blow(set, index, 0)),
        },
    }
}

fn length(c: Option<&'static ClipDef>, default: f32) -> f32 {
    c.map_or(default, |c| c.length / c.rate.max(1.0e-3))
}

/// The time of the first notify of `class` in a clip.
fn notify_time(c: &ClipDef, class: &str) -> Option<f32> {
    c.notifies.iter().filter(|n| n.class == class).map(|n| n.start).reduce(f32::min)
}

/// Where a damage clip lets a leader out: its EnableChange (KH3's exits rule), else its CheckBadStat.
pub fn stand_time(c: &ClipDef) -> Option<f32> {
    notify_time(c, "TresAnimNotify_EnableChange").or_else(|| notify_time(c, "TresAnimNotify_CheckBadStat"))
}

/// Where a down clip starts rising: its IK state (the feet planted), else 0.6 s before its end (kh-react's default).
fn rise_time(c: &ClipDef) -> f32 {
    notify_time(c, "TresAnimNotifyState_IK").unwrap_or(c.length - ClipTimes::MEDIAN.getup_tail)
}

/// THE REACTION'S CLIP TIMES for one hit: the character's own clip lengths for the hit's direction index and variant,
/// the enemies' medians only for a row the character does not have.
pub fn clip_times(set: &dyn ClipSet, index: u8, variant: bool) -> ClipTimes {
    let med = ClipTimes::MEDIAN;
    let down = blow(set, index, 2);
    let launch_down = launch_down(set);
    ClipTimes {
        small_ground: length(small_ground(set, index, variant), med.small_ground),
        small_air: length(small_air(set, index), med.small_air),
        land: length(named(set, "Land"), med.land),
        blow_start: length(blow(set, index, 0), med.blow_start),
        blow_down: length(down, med.blow_down),
        toss_start: length(row(set, 227).or_else(|| blow(set, 0, 0)), med.toss_start),
        toss_fall_start: length(launch_fall(set), med.toss_fall_start),
        toss_down: length(launch_down, med.toss_down),
        // the slam: kh-react's Beat has no rebound (the friends' 0x141250B30 bounces on M_0227 / M_0228): the launch's
        // down clip instead (knocked down + get up), [port design] so the slam never ends in a held pose
        beat_down: Some(length(launch_down, med.blow_down)),
        // no spin clip in a friend's set: a SIDESPIN / VERTICALROLL is a BLOW (Setup 0x140457380)
        spin: None,
        spin_distance: med.spin_distance,
        spin_down: med.spin_down,
        wall: length(wall(set, index), med.wall),
        wall_down: length(named(set, "Land"), med.wall_down),
        getup_tail: down.map_or(med.getup_tail, |d| (d.length - rise_time(d)).clamp(0.0, d.length)),
    }
}

/// The families in which KH3 lets him recover in the air (thrown, launched, dropping out of a reaction).
pub fn in_flight(clip: ReactClip) -> bool {
    use ReactClip as C;
    matches!(clip, C::BlowStart | C::BlowLoop | C::BlowFallLoop | C::Spin | C::TossStart | C::TossRiseLoop | C::TossFallStart | C::TossFallLoop | C::WallFall | C::FallLoop)
}

/// The down / get-up families (where a lethal blow turns into the KO).
fn is_down(clip: ReactClip) -> bool {
    use ReactClip as C;
    matches!(clip, C::BlowDown | C::SpinDown | C::TossDown | C::BeatDown | C::WallDown)
}

/// The families he stands or lies on the ground in (the leader's exit).
fn on_floor(clip: ReactClip) -> bool {
    use ReactClip as C;
    matches!(clip, C::SmallGround | C::Land) || is_down(clip)
}

/// A heavy damage kind: one whose handler throws (kh-react Setup: BLOW 2 / SIDESPIN 3 / VERTICALROLL 4 / 5 / TOSS 6, 7 /
/// LAUNCH 8 / BEAT 9 / PARABOLA 10, 11 / 21 / 22).
pub fn heavy(kind: kh_core::enums::ETresDamageKind) -> bool {
    matches!(kind.value(), 2..=11 | 21 | 22)
}

// ------------------------------------------------------------------------------------------- on the pawn

/// The member is thrown through the air by a reaction (the kits' aerial recovery reads this too).
pub fn thrown(p: &NpcPawn) -> bool {
    p.state == NpcStateId::Damage && in_flight(p.react.clip())
}

/// KH3's recoverable window: thrown, in the air, from the hit + `hurt.recover_delay`, not dying.
pub fn recoverable(p: &NpcPawn) -> bool {
    thrown(p) && !p.on_ground() && !p.hurt.dying && p.state_time >= HP.hurt_recover_delay
}

/// Before the hit reaches kh-react (`NpcPawn::begin_reaction`): the character's clip times, so that its Setup chooses by
/// HIS set (no spin clip: a SIDESPIN is a BLOW) - refined for the hit's own index / variant in `started`.
pub(crate) fn prepare(p: &mut NpcPawn, hit: &kh_core::HitRecord) {
    let set = crate::tables::anim_set(p.chara);
    p.react.body.clips = aura_recovery(p.don_aura.recovery_mult, don_longer_down(p.chara, clip_times(&set, crate::pawn::damage_dir_index(hit.direction, p.yaw), false))); // (DON-KIT)
}

/// (DON-KIT) DONALD'S LONGER KNOCKDOWNS (documentation/DONALD_OVERHAUL.md 5.3: "Heavy hits knock him down for longer than
/// Sora, using the shared HR hit-reaction rules with his own timings"): his down / get-up clips play at
/// `hurt.n_ex001.down_rate` (sheet party-hurt), so the reaction's down phases last his clip length / that rate.
pub fn down_rate(chara: crate::ids::Chara) -> f32 {
    if chara == crate::ids::Chara::Donald && HP.hurt_n_ex001_down_rate > 0.05 {
        HP.hurt_n_ex001_down_rate
    } else {
        1.0
    }
}

/// (DON-KIT) THE AERO AURA'S RECOVERY (DONALD_OVERHAUL.md 4.2 "Party evasion and recovery speed up"; DON-PROG's
/// `aura.aero_recovery`, x1.2): the reaction's ground phases (the flinches, the landings, the downs and the get-up) last
/// their time / `mult`; the flights stay the physics'.
pub fn aura_recovery(mult: f32, mut c: ClipTimes) -> ClipTimes {
    if mult > 0.0 && mult != 1.0 {
        c.small_ground /= mult;
        c.small_air /= mult;
        c.land /= mult;
        c.blow_down /= mult;
        c.toss_down /= mult;
        c.beat_down = c.beat_down.map(|v| v / mult);
        c.wall_down /= mult;
        c.getup_tail /= mult;
    }
    c
}

/// (DON-KIT) The play rate of a reaction clip of `clip`'s family on this member: Donald's down rate on the downs, x the Aero
/// aura's recovery on the ground families.
pub fn reaction_rate(p: &NpcPawn, clip: ReactClip) -> f32 {
    let down = if is_down(clip) { down_rate(p.chara) } else { 1.0 };
    let ground = on_floor(clip) || matches!(clip, ReactClip::SmallAir);
    down * if ground && p.don_aura.recovery_mult > 0.0 { p.don_aura.recovery_mult } else { 1.0 }
}

/// (DON-KIT) The reaction's clip times with his down phases lengthened by [`down_rate`].
pub fn don_longer_down(chara: crate::ids::Chara, mut c: ClipTimes) -> ClipTimes {
    let r = down_rate(chara);
    if r != 1.0 {
        c.blow_down /= r;
        c.toss_down /= r;
        c.beat_down = c.beat_down.map(|v| v / r);
        c.wall_down /= r;
        c.getup_tail /= r;
    }
    c
}

/// A reaction started (`NpcPawn::begin_reaction`): the character's own clip times for this hit and its first clip,
/// with the clip's own KH3 blend (none on the hit clips: the hit lands at once, as in KH3).
pub(crate) fn started(p: &mut NpcPawn) {
    let set = crate::tables::anim_set(p.chara);
    let (clip, index, variant) = (p.react.clip(), p.react.index(), kh3_variant(&p.react));
    p.react.body.clips = aura_recovery(p.don_aura.recovery_mult, don_longer_down(p.chara, clip_times(&set, index, variant))); // (DON-KIT)
    if let Some(k) = pick(&set, clip, p.react.phase(), index, variant) {
        play(p, k, -1.0);
        p.hurt.note("start", k.clip.name);
    }
    let r = reaction_rate(p, clip); // (DON-KIT) his down rate x the Aero aura's recovery
    if r != 1.0 || down_rate(p.chara) != 1.0 {
        p.anim.speed = r;
    }
}

/// A hit took HP: KH3's hit effect at the contact (the side of his capsule toward the attacker).
pub(crate) fn impact(p: &mut NpcPawn, hit: &kh_core::HitRecord) {
    if !HP.hurt_impact_effect {
        return;
    }
    let d = Vec3::new(hit.direction.x, hit.direction.y, 0.0).normalize_or_zero();
    p.hurt.impact = Some(p.capsule_centre() - d * p.params.capsule_radius);
}

/// Spawns what the hits of the last step asked for (pawn `step`, with the host).
pub(crate) fn flush(p: &mut NpcPawn, host: &mut crate::PartyHost) {
    let Some(at) = p.hurt.impact.take() else { return };
    let id = kh_core::host::EffectId(kh_core::notify::notify_asset_id(IMPACT_EFFECT));
    p.seam.spawn(host.effects, &kh_core::host::EffectSpawn { id, name: IMPACT_EFFECT, at: kh_core::host::Attach::World(at), rotation: Vec3::new(0.0, p.yaw + 180.0, 0.0), scale: 1.0, persistent: false });
}

fn play(p: &mut NpcPawn, k: Pick, blend: f32) {
    if k.looped {
        p.anim.play_loop(k.clip, blend);
    } else {
        p.anim.play(k.clip, blend);
    }
    p.hit_list.clear();
}

/// KH3's variant of the ground flinch: every second consecutive hit of one reaction chain (Sora: `variant = carried & 1`,
/// damage.rs Setup; the friends' Small(index, variant) 0x1412646C0). kh-react's own flag flips on every hit since the
/// pawn was made (its first hit would show the variant).
pub fn kh3_variant(r: &kh_react::EnemyReaction) -> bool {
    r.hits() % 2 == 0
}

/// The movement component follows the reaction's body: walking when it is on a floor, falling in the air (the pawn's
/// `on_ground`, the kits' recovery gates, the host's airborne flag and the state after the reaction read it).
fn sync_body(p: &mut NpcPawn, out: &kh_react::ReactOutput) {
    if !(out.suspended || out.finished) {
        return;
    }
    if out.grounded {
        if p.cmc.mode != kh_core::cmc::MovementMode::Walking {
            p.cmc.mode = kh_core::cmc::MovementMode::Walking;
            p.cmc.velocity.z = 0.0;
        }
    } else if p.cmc.on_ground() {
        p.cmc.set_falling();
    }
}

/// What the step of a running reaction asks of the pawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    Go,
    /// The reaction is over: idle (the status's state follows).
    Done,
    /// A lethal blow reached its KO point.
    Ko,
}

/// One step of a running reaction after kh-react's (pawn `step`): the clip of the family now playing, the end, the
/// lethal blow's KO point.
pub(crate) fn tick(p: &mut NpcPawn, out: &kh_react::ReactOutput) -> Tick {
    sync_body(p, out);
    if out.finished {
        return if p.hurt.dying { Tick::Ko } else { Tick::Done };
    }
    let set = crate::tables::anim_set(p.chara);
    let clip = p.react.clip();
    if let Some(k) = pick(&set, clip, out.phase, p.react.index(), kh3_variant(&p.react)) {
        if !std::ptr::eq(k.clip, p.anim.clip) && k.clip.name != p.anim.clip.name {
            play(p, k, k.clip.blend_in.max(HP.hurt_phase_blend));
            p.hurt.note("clip", k.clip.name);
        } else if k.looped && !p.anim.force_loop {
            p.anim.force_loop = true;
        }
        let r = reaction_rate(p, clip); // (DON-KIT) his down clips play slower; the Aero aura speeds recovery
        if r != 1.0 || down_rate(p.chara) != 1.0 || p.don_aura.recovery_mult != 1.0 {
            p.anim.speed = r;
        }
    }
    if p.hurt.dying && is_down(clip) && p.anim.time + 1.0e-4 >= stand_time(p.anim.clip).unwrap_or(p.anim.clip.length) {
        return Tick::Ko;
    }
    Tick::Go
}

/// THE LEADER'S QUICK GET-UP (pad/mod.rs, after the kit): on the floor, past the clip's EnableChange / CheckBadStat,
/// the stick or B ends the reaction (KH3's exits: "m_bEnableChange && stick: StopMovement; finish"). True = he is free
/// (Idle) now; the pad's own state switch takes the input in the same step.
pub(crate) fn pad_exit(p: &mut NpcPawn, stick: bool, jump: bool) -> bool {
    if !HP.hurt_leader_exit || p.state != NpcStateId::Damage || p.hurt.dying || p.bad.0 != 0 || !p.on_ground() || !(stick || jump) {
        return false;
    }
    if !on_floor(p.react.clip()) {
        return false;
    }
    let Some(t) = stand_time(p.anim.clip) else { return false };
    // (DON-KIT) Donald's quick get-up opens later (5.3), real seconds at his down clips' rate
    let late = if p.chara == crate::ids::Chara::Donald && is_down(p.react.clip()) { HP.hurt_n_ex001_exit_late * down_rate(p.chara) } else { 0.0 };
    if p.anim.time + 1.0e-4 < t + late {
        return false;
    }
    let name = p.anim.clip.name;
    p.release_reaction();
    p.cmc.stop_movement();
    p.to_idle();
    p.hurt.note("leader_exit", name);
    true
}

/// THE SHARED AERIAL RECOVERY (pad/mod.rs, after the kit: a kit with its own recovery move - Donald's Flutter, Goofy's
/// recovery - took the press already under the same gate): B in the recoverable window, with Sora's AIR_RECOVERY. True =
/// the reaction is released, his speed stopped: the caller puts him in the pad's fall.
pub(crate) fn pad_air_recovery(p: &mut NpcPawn, jump: bool) -> bool {
    if !HP.hurt_air_recovery || !p.hurt.air_recovery || !jump || !recoverable(p) {
        return false;
    }
    let name = p.anim.clip.name;
    p.release_reaction();
    p.cmc.velocity = Vec3::ZERO;
    p.cmc.gravity_off = false;
    p.cmc.set_falling();
    p.hurt.note("air_recovery", name);
    true
}

/// `Party::step`: Sora's AIR_RECOVERY on every member (the leader feed), as the kits read it.
pub(crate) fn feed(party: &mut crate::Party) {
    let Some(owned) = party.x2.feed.as_ref().map(|f| {
        let k = kh_core::enums::ETresAbilityKind::from_game_name("AIR_RECOVERY");
        k.is_none_or(|k| f.sora_abilities.owned(k) > 0)
    }) else {
        return;
    };
    let ids: Vec<crate::MemberId> = party.members().collect();
    for id in ids {
        if let Some(m) = party.member_mut(id) {
            m.hurt.air_recovery = owned;
        }
    }
}

#[cfg(test)]
#[path = "hurt_tests.rs"]
mod tests;

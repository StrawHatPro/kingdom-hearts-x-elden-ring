//! (GOOFY) THE PARRY RULES (documentation/GOOFY_OVERHAUL.md 4, 6, 7; PLAN.md R1 - R3), engine-free: which technique answers
//! an attack, each technique's window at its rank, and the one judgement of a hit that arrives. The mod learns of a hit after
//! the game applied it, so nothing here predicts: the verdict is read from the tap clock set earlier (R3).
//!
//! (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique, Goofy's big damage should solely come from his
//! guard break attacks from 6.3. The parries are there to make the enemy's stagger go up fast while taking 0 damage yourself")
//! A parry opens no counter: each technique takes 0 damage and fills the enemy's Stagger by its own row ([`parry_stagger`]);
//! the full gauge's BROKEN is cashed by Goofy Breaker (pad/goofy_breaker.rs). A perfect parry gives MP back ([`gives_mp`]).

use super::learn::{self, GoofyMove, RankBank};
use super::tune::K;
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{AttackClass, HitInfo, HitRecord};

/// A parry technique (11.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tech {
    Deflect,
    PinningGuard,
    HopCounter,
    SlipCounter,
    ShieldReflect,
    SpellWard,
    BullBrace,
    RhythmDeflect,
}

impl Tech {
    /// What carries its rank.
    pub const fn mv(self) -> GoofyMove {
        match self {
            Tech::Deflect => GoofyMove::Deflect,
            Tech::PinningGuard => GoofyMove::PinningGuard,
            Tech::HopCounter => GoofyMove::HopCounter,
            Tech::SlipCounter => GoofyMove::SlipCounter,
            Tech::ShieldReflect => GoofyMove::ShieldReflect,
            Tech::SpellWard => GoofyMove::SpellWard,
            Tech::BullBrace => GoofyMove::BullBrace,
            Tech::RhythmDeflect => GoofyMove::RhythmDeflect,
        }
    }

    /// (THE USER 2026-11-02: "The parries are there to make the enemy's stagger go up fast while taking 0 damage yourself") ITS
    /// STAGGER at rank 1, its own row (`stagger.*`): the job of every technique now, with no counter after it. Rhythm Deflect's
    /// is the second deflect of a string ([`rhythm_stagger`] has the rest).
    pub fn stagger(self) -> f32 {
        match self {
            Tech::Deflect => K.stagger_deflect,
            Tech::PinningGuard => K.stagger_pin,
            Tech::HopCounter => K.stagger_hop,
            Tech::SlipCounter => K.stagger_slip,
            Tech::ShieldReflect => K.stagger_reflect,
            Tech::SpellWard => K.stagger_ward,
            Tech::BullBrace => K.stagger_brace,
            Tech::RhythmDeflect => K.rhythm_stagger,
        }
    }

    /// The answer to an attack class (4.3).
    pub const fn for_class(c: AttackClass) -> Tech {
        match c {
            AttackClass::Normal => Tech::Deflect,
            AttackClass::Thrust => Tech::PinningGuard,
            AttackClass::Sweep => Tech::HopCounter,
            AttackClass::Grab => Tech::SlipCounter,
            AttackClass::Heavy => Tech::BullBrace,
            AttackClass::Projectile => Tech::ShieldReflect,
            AttackClass::Magic => Tech::SpellWard,
        }
    }
}

/// The class of a hit. A host that says nothing leaves `Normal`; KH3's own grab kinds and the ignore-guard flag then still
/// read as what they are, so the test level's fixtures keep their meaning.
/// (MIMIC-BOSS, THE USER'S DECISION 2026-11-06: "stay unparryable") Sora's attacks that carry KH3's ignore-guard flag
/// (`invalid_guard`: 349 of the 482 rows of sora-combat-attack-data) stay Heavy, unparryable, for Goofy and for a Goofy copy of
/// the Mimic Tear bosses alike (only Bull Brace or a dodge answers them). No behaviour change.
pub fn class_of(hit: &HitRecord) -> AttackClass {
    match hit.attack_info.class {
        AttackClass::Normal if matches!(hit.damage_kind, Kind::Catch | Kind::Bind) => AttackClass::Grab,
        AttackClass::Normal if hit.info.flags50 & HitInfo::FLAG50_IGNORE_GUARD != 0 => AttackClass::Heavy,
        c => c,
    }
}

/// The hit is perilous (the attack's own mark, or an unblockable class).
pub fn perilous_of(hit: &HitRecord) -> bool {
    hit.attack_info.perilous || matches!(class_of(hit), AttackClass::Grab | AttackClass::Heavy)
}

/// The stick against the enemy (R1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StickRel {
    #[default]
    Neutral,
    Toward,
    Away,
    Side,
}

/// Reads the stick (world yaw, pushed or not) against the direction to the enemy (world yaw).
pub fn stick_rel(stick_yaw: f32, active: bool, enemy_yaw: Option<f32>) -> StickRel {
    if !active {
        return StickRel::Neutral;
    }
    let Some(e) = enemy_yaw else { return StickRel::Side };
    let off = crate::pawn::wrap180(stick_yaw - e).abs();
    if off <= K.input_toward_arc {
        StickRel::Toward
    } else if off >= K.input_away_arc {
        StickRel::Away
    } else {
        StickRel::Side
    }
}

/// The tap clock as the hit finds it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct TapFacts {
    /// Seconds since the last accepted X press.
    pub age: f32,
    /// Mashes behind it (each shrinks the window).
    pub mash: u8,
    /// The hold before that press and the gap from its release to the press (Spell Ward).
    pub prev_hold: f32,
    pub prev_gap: f32,
    /// A deflect string is running.
    pub in_string: bool,
}

/// The hit as he stands.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct HitFacts {
    pub class: AttackClass,
    /// The shield is up (guard, a tap's stance, a parry's recoil).
    pub raised: bool,
    /// Within the block's arc / the deflect's arc.
    pub front_block: bool,
    pub front_deflect: bool,
    /// The stick at the tap against the attacker.
    pub stick: StickRel,
    /// Bull Brace is set, for this long.
    pub braced: Option<f32>,
}

/// What of his kit the judgement reads.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Mods {
    pub ranks: RankBank,
    pub level: u32,
    pub pips: u8,
    pub in_zone: bool,
    pub forward_plus: bool,
    /// Second Deflect is worn and unused this string.
    pub second_deflect: bool,
    /// Revenge Bash stands ready.
    pub revenge: bool,
    /// Stagger Boost is worn.
    pub stagger_boost: bool,
}

impl Mods {
    fn rank(&self, t: Tech) -> u8 {
        learn::rank(&self.ranks, t.mv(), self.level)
    }
}

/// A parry that worked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parry {
    pub tech: Tech,
    /// The right answer to this attack (a plain deflect of an attack with its own technique is not).
    pub right: bool,
    /// The stick pushed toward the enemy on a plain deflect.
    pub aggressive: bool,
    /// Second Deflect forgave the tap.
    pub forgiven: bool,
    /// Revenge Bash turned a block into this.
    pub revenge: bool,
}

/// What a hit does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Parry(Parry),
    /// The shield takes it: GUARD pays.
    Block,
    /// It lands. `failed`: he tried to parry an unblockable with the wrong answer.
    Through { failed: bool },
}

/// The mash penalty on a window.
fn mashed(w: f32, mash: u8) -> f32 {
    w * K.deflect_mash_shrink.powi(mash as i32)
}

/// A tap technique's window at `rank`, before the mash.
fn base_window(t: Tech, rank: u8) -> f32 {
    let top = rank >= 2;
    match t {
        Tech::PinningGuard => if top { K.pin_window_r2 } else { K.pin_window },
        Tech::ShieldReflect => if top { K.reflect_window_r2 } else { K.reflect_window },
        Tech::SpellWard => if top { K.ward_window_r2 } else { K.ward_window },
        _ => if top { K.deflect_window_r2 } else { K.deflect_window },
    }
}

/// A tap's window as the hit judges it: the technique's at its rank, Resolve, an aggressive deflect's cut, Rhythm Deflect's
/// rank 2 inside a string, the mash.
pub fn window(t: Tech, tap: &TapFacts, aggressive: bool, m: &Mods) -> f32 {
    let mut w = base_window(t, m.rank(t).max(1));
    if m.pips >= 1 {
        w += K.resolve_pip1_window;
    }
    if aggressive {
        w -= if m.forward_plus { K.deflect_aggressive_cut_plus } else { K.deflect_aggressive_cut };
    }
    if tap.in_string && m.rank(Tech::RhythmDeflect) >= 2 {
        w += K.rhythm_window_add;
    }
    mashed(w.max(0.0), tap.mash)
}

/// How long Bull Brace must be set before impact.
pub fn brace_set(rank: u8) -> f32 {
    if rank >= 2 { K.brace_set_min_r2 } else { K.brace_set_min }
}

/// The Spell Ward input: X held through the cast, released and pressed again.
fn ward_input(t: &TapFacts) -> bool {
    t.prev_hold >= K.ward_hold_min && t.prev_gap <= K.ward_release_max
}

/// THE ONE JUDGEMENT of a hit that takes HP.
pub fn judge(h: &HitFacts, t: &TapFacts, m: &Mods) -> Verdict {
    let right = Tech::for_class(h.class);
    let plain = Parry { tech: right, right: true, aggressive: false, forgiven: false, revenge: false };
    if h.class == AttackClass::Heavy && h.front_deflect && m.rank(Tech::BullBrace) > 0 && h.braced.is_some_and(|s| s >= brace_set(m.rank(Tech::BullBrace))) {
        return Verdict::Parry(plain);
    }
    let unblockable = matches!(h.class, AttackClass::Grab | AttackClass::Heavy);
    let toward = h.stick == StickRel::Toward;
    if h.raised && h.front_deflect {
        if m.in_zone && t.age <= window(Tech::Deflect, t, false, m) {
            return Verdict::Parry(plain);
        }
        let own_input = match h.class {
            AttackClass::Thrust | AttackClass::Projectile => toward,
            AttackClass::Magic => ward_input(t),
            _ => false,
        };
        if own_input && m.rank(right) > 0 && t.age <= window(right, t, false, m) {
            return Verdict::Parry(plain);
        }
        if !unblockable {
            let w = window(Tech::Deflect, t, toward, m);
            let answer = Parry { tech: Tech::Deflect, right: h.class == AttackClass::Normal, aggressive: toward, forgiven: false, revenge: false };
            if t.age <= w {
                return Verdict::Parry(answer);
            }
            if m.second_deflect && t.age <= w + K.second_deflect_late {
                return Verdict::Parry(Parry { forgiven: true, ..answer });
            }
            if m.revenge {
                return Verdict::Parry(Parry { revenge: true, ..answer });
            }
        }
    }
    if unblockable {
        return Verdict::Through { failed: h.raised && t.age <= window(Tech::Deflect, t, false, m) };
    }
    if h.raised && h.front_block {
        Verdict::Block
    } else {
        Verdict::Through { failed: false }
    }
}

/// (THE USER 2026-11-02: "...when Goofy successfully performs a perfect deflect") A PERFECT DEFLECT: the right answer to the
/// attack, timed. A plain deflect of an attack that has its own technique is not (`right` false: half Stagger, 4.3), nor a late
/// tap Second Deflect forgave (11.3: "one mistimed tap per string is forgiven"); Revenge Bash's is (11.3: "his next deflect
/// counts as perfect").
pub fn perfect(p: &Parry) -> bool {
    p.right && !p.forgiven
}

/// (THE USER 2026-11-02: "make non-boss enemies bounce off of Goofy's shield if they're not doing a long combo string when Goofy
/// successfully performs a perfect deflect") THE SWING BOUNCES OFF HIS SHIELD: a [`perfect`] parry on the shield (Deflect, Pinning
/// Guard, In the Zone's deflect of a sweep) of a melee blow a shield can turn (Normal, Thrust, Sweep), its attacker not in the
/// middle of a string (Rhythm Deflect's own reading, `GoofyCore::mid_string`). Not a heavy (Bull Brace: an unblockable no guard
/// turns), a grab, a projectile or magic (nothing swung at the shield, the shooter is out of reach); Hop and Slip Counter are
/// evasions (nothing touches the shield), Ally Guard / Interpose parry for a friend on their own. Whether the attacker is a boss is
/// the seam's (`seams::on_bounce`). Row `deflect.bounce` switches it.
pub fn bounces(p: &Parry, class: AttackClass, mid_string: bool) -> bool {
    K.deflect_bounce && perfect(p) && matches!(class, AttackClass::Normal | AttackClass::Thrust | AttackClass::Sweep) && !mid_string
}

/// A technique's rank scale of its Stagger: rank 2 `stagger.rank2_mult`, rank 3 `stagger.rank3_mult` (THE USER 2026-11-02: the
/// counters' rank 3 follow-ups went with the counters; rank 3 now fills more).
pub fn rank_mult(rank: u8) -> f32 {
    match rank {
        0..=1 => 1.0,
        2 => K.stagger_rank2_mult,
        _ => K.stagger_rank3_mult,
    }
}

/// RHYTHM DEFLECT'S STAGGER (THE USER 2026-11-02, Crescendo's payoff moved into the string itself): the `hits`-th deflect in a
/// row of a string (the first is a plain Deflect): 28 at the second, 6 more each after, at most 46 (rows `rhythm.*`), rank 1.
pub fn rhythm_stagger(hits: u8) -> f32 {
    (K.rhythm_stagger + K.rhythm_stagger_per * hits.saturating_sub(2) as f32).min(K.rhythm_stagger_max)
}

/// THE STAGGER A PARRY ADDS (THE USER 2026-11-02: "The parries are there to make the enemy's stagger go up fast while taking 0
/// damage yourself"): the technique's own row ([`Tech::stagger`]); a plain deflect of an attack with its own technique half of
/// Deflect's; a deflect in time inside a string, with Rhythm Deflect learned, [`rhythm_stagger`] at Rhythm Deflect's rank. Then
/// its rank ([`rank_mult`]), the aggressive deflect, Resolve's first pip and Stagger Boost. `hits`: the deflects in a row with
/// this one (`GoofyCore::string_hit`).
pub fn parry_stagger(p: &Parry, hits: u8, m: &Mods) -> f32 {
    let rhythm = hits >= 2 && p.tech == Tech::Deflect && p.right && m.rank(Tech::RhythmDeflect) > 0;
    let (base, ranked_by) = if rhythm {
        (rhythm_stagger(hits).max(K.stagger_deflect), Tech::RhythmDeflect)
    } else if p.tech == Tech::Deflect && !p.right {
        (K.stagger_deflect * K.stagger_plain_mult, Tech::Deflect)
    } else {
        (p.tech.stagger(), p.tech)
    };
    let mut v = base * rank_mult(m.rank(ranked_by));
    if p.aggressive {
        v *= K.deflect_aggressive_stagger;
    }
    if m.pips >= 1 {
        v *= K.resolve_pip1_stagger;
    }
    if m.stagger_boost {
        v *= K.stagger_boost_mult;
    }
    v
}

/// (THE USER 2026-11-02: "MP back on a perfect parry is good") A PERFECT PARRY GIVES MP BACK, the replacement of the counters'
/// MP: [`perfect`] parries only (row `parry.mp`, or `parry.mp_charge` off a running MP Charge).
pub fn gives_mp(p: &Parry) -> bool {
    perfect(p)
}

/// What the camp page shows of a technique at `rank` (0: not learned, shown at rank 1): the plain number of [`parry_stagger`],
/// with no Resolve, aggression or ability (Rhythm Deflect: its second deflect and its top, a range).
pub fn shown_stagger(t: Tech, rank: u8) -> (f32, f32) {
    let r = rank_mult(rank.max(1));
    match t {
        Tech::RhythmDeflect => (rhythm_stagger(2) * r, K.rhythm_stagger_max * r),
        _ => (t.stagger() * r, t.stagger() * r),
    }
}

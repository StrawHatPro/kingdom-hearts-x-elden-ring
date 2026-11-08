//! (GOOFY) THE SHIELD ARTS (documentation/GOOFY_OVERHAUL.md 6.2, 8, 9, 11.2; PLAN.md R1, R14, R16, R19): two equipped Arts on
//! LB + A and LB + B, Taunt fixed on LB + X. Perfect parries give MP back (pad/goofy.rs `parry_mp`; THE USER 2026-11-02: "MP
//! back on a perfect parry is good"); the counters' MP and their free Art are retired with the counters.
//! Everything reads `buttons` / `stick` of pad/goofy.rs and his state on `FriendStatus`, so a filled `AiDrive` plays them all.
//!
//! COST AND COOLDOWN (THE USER 2026-11-02: "Goofy's abilities (except taunt) should cost more MP, and introduce a cooldown with
//! a visible timer, they are way too spammable"): every Art but Taunt costs several times its old MP (a full bar buys a few,
//! not a stream) and, once used, waits its row `art.*.cooldown` before it can be used again, whoever plays him. A press while
//! it waits is refused with the panel's refused sound and spends nothing. Taunt has neither.
//! The panel's entry shows the wait ([`panel_row`]'s `cooldown`, drawn by kh-ui's command display).
//!
//! - Goofy Charge: a rush that shoves what it meets and ends in a guard stance.
//! - Goofy Tornado: a whirl that hits around him and deflects bullets while it spins.
//! - Goofy Bash: the overhead slam, big Stagger.
//! - Goofy Turbo: the shield as a sled, steered by the stick; X cancels it into the guard, B jumps off.
//! - Taunt: `GoofyRun::taunt`, read by the hosts' enemy targeting.
//! - Interpose: a dash to the nearest threatened friend, then a cover that deflects the next hit aimed at them (R14: Goofy
//!   absorbs it, no hit is moved). Bodyguard waives the MP while a friend is low.
//! - Shield Toss: the shield flies out and back on its own clock (`step`), in any state he is in. While it is out he can only guard,
//!   bare-handed: no parry, half GUARD, no shove, no Breaker, no other Art that needs the shield. Its 3 s cap is the
//!   longest this lasts.
//!
//! Every number is a row `art.*` of `sheets/port/party-kit-goofy.json`; rank 2 and 3 of each are rows too.

use super::goofy::{self, g, gp, has, state_n, stick, stm, BRACE, GUARD, GUARD_HIT, HOP, PARRY};
use super::goofy::{ART, TAUNT};
use super::goofy_combo::{self as combo, COMBO};
use super::goofy_counters as counters;
use super::goofy_shots::{self as shots, Area, Push, SoloRow};
use super::{JUMP, RUN};
use crate::goofy::parry::StickRel;
use crate::goofy::{GoofyAbility as GA, GoofyEvent, GoofyMove, StaggerWhy, ThreatWhy, Verdict, K};
use crate::host::PartyHost;
use crate::ids::MemberId;
use crate::party::Party;
use crate::pawn::{forward, horizontal, wrap180, yaw_of, NpcPawn, StepCtx};
use crate::projectile::Projectile;
use crate::states::NpcStateId;
use crate::status::FriendStatus;
use glam::{Vec2, Vec3};
use kh_core::command::leader::goofy as ids;
use kh_core::command::{CommandRow, MenuEvent};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{AttackClass, HitRecord, HitResult, TargetId};

/// The areas of the Arts (`Area::tag`).
const TAG: &str = "art";

/// One Art. `Art::id` is kh-core's id of his Customize block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Art {
    Charge,
    Tornado,
    Bash,
    Turbo,
    Interpose,
    ShieldToss,
    Taunt,
}

impl Art {
    pub const ALL: [Art; 7] = [Art::Charge, Art::Tornado, Art::Bash, Art::Turbo, Art::Interpose, Art::ShieldToss, Art::Taunt];

    pub const fn id(self) -> u8 {
        match self {
            Art::Charge => ids::CHARGE,
            Art::Tornado => ids::TORNADO,
            Art::Bash => ids::BASH,
            Art::Turbo => ids::TURBO,
            Art::Interpose => ids::INTERPOSE,
            Art::ShieldToss => ids::SHIELD_TOSS,
            Art::Taunt => ids::TAUNT,
        }
    }

    pub fn from_id(id: u8) -> Option<Art> {
        Art::ALL.into_iter().find(|a| a.id() == id)
    }

    /// What carries its rank.
    pub const fn mv(self) -> GoofyMove {
        match self {
            Art::Charge => GoofyMove::GoofyCharge,
            Art::Tornado => GoofyMove::GoofyTornado,
            Art::Bash => GoofyMove::GoofyBash,
            Art::Turbo => GoofyMove::GoofyTurbo,
            Art::Interpose => GoofyMove::Interpose,
            Art::ShieldToss => GoofyMove::ShieldToss,
            Art::Taunt => GoofyMove::Taunt,
        }
    }

    /// The hit record's name (the hosts' logs).
    pub const fn row(self) -> &'static str {
        match self {
            Art::Charge => "GOOFY_ART_CHARGE",
            Art::Tornado => "GOOFY_ART_TORNADO",
            Art::Bash => "GOOFY_ART_BASH",
            Art::Turbo => "GOOFY_ART_TURBO",
            Art::Interpose => "GOOFY_ART_INTERPOSE",
            Art::ShieldToss => "GOOFY_ART_TOSS",
            Art::Taunt => "GOOFY_ART_TAUNT",
        }
    }

    /// Its place in `Art::ALL` (`ArtsRun::cooldown`).
    const fn index(self) -> usize {
        match self {
            Art::Charge => 0,
            Art::Tornado => 1,
            Art::Bash => 2,
            Art::Turbo => 3,
            Art::Interpose => 4,
            Art::ShieldToss => 5,
            Art::Taunt => 6,
        }
    }

    /// The kit log's word.
    const fn word(self) -> &'static str {
        match self {
            Art::Charge => "art_charge",
            Art::Tornado => "art_tornado",
            Art::Bash => "art_bash",
            Art::Turbo => "art_turbo",
            Art::Interpose => "art_interpose",
            Art::ShieldToss => "art_toss",
            Art::Taunt => "art_taunt",
        }
    }
}

fn ranked(rank: u8, v: [f32; 3]) -> f32 {
    v[rank.clamp(1, 3) as usize - 1]
}

/// The MP of `art` at `rank`.
pub fn mp(art: Art, rank: u8) -> i32 {
    ranked(
        rank,
        match art {
            Art::Charge => [K.art_charge_mp, K.art_charge_mp_r2, K.art_charge_mp_r3],
            Art::Tornado => [K.art_tornado_mp, K.art_tornado_mp_r2, K.art_tornado_mp_r3],
            Art::Bash => [K.art_bash_mp, K.art_bash_mp_r2, K.art_bash_mp_r3],
            Art::Turbo => [K.art_turbo_mp, K.art_turbo_mp_r2, K.art_turbo_mp_r3],
            Art::Interpose => [K.art_interpose_mp, K.art_interpose_mp_r2, K.art_interpose_mp_r3],
            Art::ShieldToss => [K.art_toss_mp, K.art_toss_mp_r2, K.art_toss_mp_r3],
            Art::Taunt => [K.art_taunt_mp, K.art_taunt_mp_r2, K.art_taunt_mp_r3],
        },
    ) as i32
}

/// (THE USER 2026-11-02: "...introduce a cooldown with a visible timer, they are way too spammable") How long `art` waits from
/// the moment it is used before it can be used again (rows `art.*.cooldown`). Taunt has none: the user left it out.
pub fn cooldown(art: Art) -> f32 {
    match art {
        Art::Charge => K.art_charge_cooldown,
        Art::Tornado => K.art_tornado_cooldown,
        Art::Bash => K.art_bash_cooldown,
        Art::Turbo => K.art_turbo_cooldown,
        Art::Interpose => K.art_interpose_cooldown,
        Art::ShieldToss => K.art_toss_cooldown,
        Art::Taunt => 0.0,
    }
}

/// The seconds `art` still waits on this Goofy (0: ready; not Goofy: 0).
pub fn cooldown_left(s: &FriendStatus, art: Art) -> f32 {
    s.goofy.as_ref().map_or(0.0, |g| g.run.arts.cooldown[art.index()].max(0.0))
}

/// Its power at `rank` (0: it does not hit).
pub fn power(art: Art, rank: u8) -> f32 {
    ranked(
        rank,
        match art {
            Art::Charge => [K.art_charge_power, K.art_charge_power_r2, K.art_charge_power_r3],
            Art::Tornado => [K.art_tornado_power, K.art_tornado_power_r2, K.art_tornado_power_r3],
            Art::Bash => [K.art_bash_power, K.art_bash_power_r2, K.art_bash_power_r3],
            Art::Turbo => [K.art_turbo_power, K.art_turbo_power_r2, K.art_turbo_power_r3],
            Art::ShieldToss => [K.art_toss_power, K.art_toss_power_r2, K.art_toss_power_r3],
            Art::Interpose | Art::Taunt => [0.0; 3],
        },
    )
}

/// The Stagger of one of its hits at `rank`.
pub fn stagger(art: Art, rank: u8) -> f32 {
    ranked(
        rank,
        match art {
            Art::Charge => [K.art_charge_stagger, K.art_charge_stagger_r2, K.art_charge_stagger_r3],
            Art::Tornado => [K.art_tornado_stagger, K.art_tornado_stagger_r2, K.art_tornado_stagger_r3],
            Art::Bash => [K.art_bash_stagger, K.art_bash_stagger_r2, K.art_bash_stagger_r3],
            Art::Turbo => [K.art_turbo_stagger, K.art_turbo_stagger_r2, K.art_turbo_stagger_r3],
            Art::ShieldToss => [K.art_toss_stagger, K.art_toss_stagger_r2, K.art_toss_stagger_r3],
            Art::Interpose | Art::Taunt => [0.0; 3],
        },
    )
}

fn kind_of(name: &str) -> Kind {
    Kind::from_game_name(name).unwrap_or(Kind::Small)
}

/// Who a cover watches over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Friend {
    Member(MemberId),
    Sora,
}

/// Interpose's cover (seconds left, hits left).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cover {
    pub friend: Friend,
    pub left: f32,
    pub hits: u8,
}

/// The shield in flight (`step`).
#[derive(Clone, Debug, PartialEq)]
pub struct Toss {
    /// What the hosts draw (`Party::thrown_shield`).
    pub body: Projectile,
    pub rank: u8,
    pub age: f32,
    /// On its way back to him.
    pub back: bool,
    /// What it flies at (None: the spot `goal`).
    pub target: Option<TargetId>,
    pub goal: Vec3,
    pub hit: Vec<TargetId>,
    pub bounced: bool,
}

/// The running Art (`ArtsRun::running`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArtRun {
    pub art: Art,
    pub rank: u8,
    /// 0 the wind-up, 1 the move, 2 its end.
    pub phase: u8,
    /// Seconds in this phase.
    pub t: f32,
    /// Seconds to the next hit of a move that hits again.
    pub next: f32,
    /// The clip's rate now.
    pub rate: f32,
    /// Its one hit or throw is out.
    pub done: bool,
    pub landed: bool,
    /// Turbo's heading.
    pub dir: Vec3,
    /// Interpose: whom, what threatens them, where he goes.
    pub friend: Option<Friend>,
    pub danger: Vec3,
    pub goal: Vec3,
}

/// The Arts' side of `GoofyRun`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ArtsRun {
    pub running: Option<ArtRun>,
    /// [LB + A, LB + B], ids of his Customize block, once the host fed them (`Party::set_goofy_arts`).
    pub equipped: Option<[u8; 2]>,
    /// The guard stance holds this long with X let go.
    pub stance: f32,
    pub cover: Option<Cover>,
    pub toss: Option<Toss>,
    /// The living friends in the party and their HP shares (noted each step: a pawn's step cannot read the others).
    pub friends: Vec<(MemberId, f32)>,
    /// A member leads: `SoraView`'s own place and HP are that member's, Sora is `ai_sora` or out of the party.
    pub friend_leads: bool,
    /// Shots the spin batted back: (shooter, damage), sent at the next step.
    pub batted: Vec<(TargetId, f32)>,
    /// (THE USER 2026-11-02: "...introduce a cooldown with a visible timer") Seconds each Art (index `Art::ALL`) still waits
    /// before it can be used again; set when it starts, run down by `clock`.
    pub cooldown: [f32; 7],
}

// ---------------------------------------------------------------------------------------------- reading

fn run(p: &NpcPawn) -> Option<ArtRun> {
    g(p).arts.running.filter(|_| matches!(state_n(p), Some(ART | TAUNT)))
}

/// The shield is out of his hand: he guards bare-handed.
pub fn bare_handed(p: &NpcPawn) -> bool {
    g(p).arts.toss.is_some()
}

/// What a bare hand makes of a verdict: no parry. What he could block he blocks; an unblockable lands.
pub fn bare_verdict(v: Verdict, class: AttackClass, front_block: bool) -> Verdict {
    match v {
        Verdict::Parry(_) if front_block && !matches!(class, AttackClass::Grab | AttackClass::Heavy) => Verdict::Block,
        Verdict::Parry(_) => Verdict::Through { failed: false },
        other => other,
    }
}

/// What his GUARD pays now against a full hand.
pub fn guard_mult(p: &NpcPawn) -> f32 {
    if bare_handed(p) {
        K.art_toss_bare_drain
    } else {
        1.0
    }
}

/// The equipped Art of `slot` (0 LB + A, 1 LB + B). Before the host's first feed, the first two he knows.
pub fn equipped(s: &FriendStatus, slot: u8) -> Option<Art> {
    let arts = s.goofy.as_ref()?.run.arts.equipped.unwrap_or_else(|| ids::default_arts(&s.gof_art_book()));
    Art::from_id(*arts.get(slot as usize)?)
}

/// The two equipped Arts as their rank lines (GF-AI's reading).
pub fn art_moves(s: &FriendStatus) -> [Option<GoofyMove>; 2] {
    [equipped(s, 0).map(Art::mv), equipped(s, 1).map(Art::mv)]
}

/// (GF-AI) The two equipped Arts AI Goofy's brain may ask for now: one that is still cooling down is left out, so his brain
/// never stalls on a press the kit refuses (THE USER 2026-11-02: the cooldown holds for whoever plays him).
pub fn ready_art_moves(s: &FriendStatus) -> [Option<GoofyMove>; 2] {
    let ready = |slot: u8| equipped(s, slot).filter(|a| cooldown_left(s, *a) <= 0.0).map(Art::mv);
    [ready(0), ready(1)]
}

/// (THE USER 2026-11-02: "let Goofy use his last MP and go into MP reserve") The Arts AI Goofy's brain may ask for now: the
/// ready ones ([`ready_art_moves`]) the party's MP rule lets him pay (crate::mp_rule: any MP above 0, none in MP Charge), so
/// he may spend his last MP into MP Charge as the player may. Not wastefully: while `keep` (a friend is low enough for the
/// bodyguard, `ai.interpose_low`) and his Interpose is learned and ready, an Art that would take the last of his MP is left
/// out, since that MP buys Interpose (which may itself take the last of it).
pub fn ai_art_moves(s: &FriendStatus, keep: bool) -> [Option<GoofyMove>; 2] {
    if !crate::mp_rule::can_spend(s) {
        return [None, None];
    }
    let guard_ready = keep && s.gof_rank(GoofyMove::Interpose) > 0 && cooldown_left(s, Art::Interpose) <= 0.0;
    let ok = |m: Option<GoofyMove>| {
        m.filter(|mv| {
            let Some(art) = Art::ALL.into_iter().find(|a| a.mv() == *mv) else { return false };
            *mv == GoofyMove::Interpose || !guard_ready || !crate::mp_rule::drains(s.mp, mp(art, s.gof_rank(*mv)))
        })
    };
    let [a, b] = ready_art_moves(s);
    [ok(a), ok(b)]
}

/// (GF-AI, THE USER 2026-11-02: "Make sure Goofy as an AI party member uses his new tools properly") The MP price of the Art of
/// a rank line at his rank now, for AI Goofy's "Watch MP" reading (0: not an Art he has).
pub fn move_mp(s: &FriendStatus, mv: GoofyMove) -> i32 {
    Art::ALL.into_iter().find(|a| a.mv() == mv).map_or(0, |art| mp(art, s.gof_rank(mv).max(1)))
}

/// (GF-AI) The cooldown left of the Art of a rank line (0: ready, or not an Art).
pub fn move_cooldown_left(s: &FriendStatus, mv: GoofyMove) -> f32 {
    Art::ALL.into_iter().find(|a| a.mv() == mv).map_or(0.0, |art| cooldown_left(s, art))
}

/// The panel's row of an Art (`pad/menu.rs`). While it cools down the row carries its wait (THE USER 2026-11-02:
/// "...introduce a cooldown with a visible timer"), which the HUD draws on the entry; `enabled` leaves the wait out (kh-core
/// `CommandRow::cooldown`), so a waiting entry stays lit over its gauge and MP Charge still greys it.
pub fn panel_row(s: &FriendStatus, id: u8) -> CommandRow {
    let Some(art) = Art::from_id(id) else { return CommandRow::default() };
    let rank = s.gof_rank(art.mv());
    let left = if rank > 0 { cooldown_left(s, art) } else { 0.0 };
    let wait = if left > 0.0 { (left, cooldown(art).max(left)) } else { (0.0, 0.0) };
    CommandRow { kind: 28, name: ids::name(id), enabled: rank > 0 && s.mp_charge_left <= 0.0, mp_short: rank > 0 && !crate::mp_rule::may_pay(s, mp(art, rank)) /* (THE USER 2026-11-02) short only at 0 MP / in MP Charge */, quantity: 0, needs_target: false, cooldown: wait }
}

impl FriendStatus {
    /// (GOOFY) His Arts' ranks as his Customize block reads them.
    pub fn gof_art_book(&self) -> ids::ArtBook {
        let mut ranks = [0; 7];
        for art in Art::ALL {
            ranks[(art.id() - ids::FIRST) as usize] = self.gof_rank(art.mv());
        }
        ids::ArtBook { ranks, fed: self.goofy.is_some() }
    }
}

// ---------------------------------------------------------------------------------------------- starting one

/// LB + A (`slot` 0) / LB + B (`slot` 1): the equipped Art. True = he started it (the kit took the step).
pub fn try_art(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost, slot: u8) -> bool {
    equipped(&p.status, slot).is_some_and(|art| start(p, cx, host, art))
}

/// (GF-AI) The Art of a rank line, for AI Goofy's brain: the same start, price and rules. True = he started it.
pub fn try_move(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost, mv: GoofyMove) -> bool {
    Art::ALL.into_iter().find(|a| a.mv() == mv).is_some_and(|art| start(p, cx, host, art))
}

/// LB + X: Taunt. True = he started it.
pub fn try_taunt(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) -> bool {
    start(p, cx, host, Art::Taunt)
}

/// He may start `art` now: on free ground, out of the guard or a parry's recoil, from a shove's hit, a hop's end.
/// In the air only the throw.
fn open(p: &NpcPawn, art: Art) -> bool {
    let n = state_n(p);
    if !p.on_ground() {
        return art == Art::ShieldToss && n == Some(JUMP);
    }
    let landing = n == Some(JUMP) && p.pad.as_ref().is_some_and(|d| d.jump.sub >= 10);
    matches!(p.state, NpcStateId::Idle | NpcStateId::Move)
        || n == Some(RUN)
        || landing
        || matches!(n, Some(GUARD | BRACE | PARRY | GUARD_HIT))
        || (n == Some(COMBO) && combo::confirm_open(p))
        || (n == Some(HOP) && p.anim.time >= K.hop_cancel)
}

/// The panel's feedback (a host's refused or decide sound; an AI Goofy has none).
fn panel(p: &mut NpcPawn, e: MenuEvent) {
    if let Some(d) = p.pad.as_mut() {
        d.menu.events.push(e);
    }
}

/// The price: MP by the party's MP rule (crate::mp_rule: never while charging, any MP above 0, reaching 0 starts the charge).
/// (THE USER 2026-11-02: the free Art after a counter went with the counters.)
fn pay(p: &mut NpcPawn, cost: i32) -> bool {
    // (THE USER 2026-11-02: "let Goofy use his last MP and go into MP reserve") the party's MP rule (crate::mp_rule): any MP
    // above 0 buys the Art (Taunt's small price too); one dearer than what he has left takes the last of it and starts MP
    // Charge at once. Refused (the refused sound) only in MP Charge or at 0 MP
    crate::mp_rule::spend(&mut p.status, cost).is_some()
}

fn friend_spots(p: &NpcPawn, cx: &StepCtx) -> Vec<(Friend, Vec3)> {
    let mut out: Vec<(Friend, Vec3)> = g(p).arts.friends.iter().filter_map(|(id, _)| cx.members.iter().find(|m| m.0 == *id).map(|m| (Friend::Member(*id), m.1))).collect();
    match cx.sora.ai_sora {
        Some((_, at)) => out.push((Friend::Sora, at)),
        None if !g(p).arts.friend_leads && !cx.sora.dead => out.push((Friend::Sora, cx.sora.position)),
        None => {}
    }
    out
}

/// The nearest friend in reach with a danger on them: the attacker of a live telegraph close by, else any hostile right next to
/// them. (friend, their feet, the danger's feet)
fn threatened(p: &NpcPawn, cx: &StepCtx, host: &PartyHost) -> Option<(Friend, Vec3, Vec3)> {
    let me = p.position();
    let live: Vec<Vec3> = cx.telegraphs.iter().filter_map(|t| host.targets.info(t.attacker)).filter(shots::lockable).map(|t| t.position).collect();
    let enemies: Vec<Vec3> = shots::enemies(host).iter().map(|t| t.position).collect();
    let mut best: Option<(f32, (Friend, Vec3, Vec3))> = None;
    for (f, at) in friend_spots(p, cx) {
        let d = horizontal(at - me).length();
        if d > K.art_interpose_range {
            continue;
        }
        let gap = |e: &&Vec3| horizontal(**e - at).length();
        let danger = live.iter().filter(|e| gap(e) <= K.art_interpose_threat_near).min_by(|a, b| gap(a).total_cmp(&gap(b))).or_else(|| enemies.iter().filter(|e| gap(e) <= K.art_interpose_threat_close).min_by(|a, b| gap(a).total_cmp(&gap(b))));
        if let Some(e) = danger {
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, (f, at, *e)));
            }
        }
    }
    best.map(|b| b.1)
}

/// BODYGUARD: a friend under the row's share of HP, alive.
fn friend_low(p: &NpcPawn, cx: &StepCtx) -> bool {
    let line = K.art_interpose_bodyguard_below;
    let sora = match cx.sora.ai_sora {
        Some(_) => cx.sora.ai_sora_hp,
        None if !g(p).arts.friend_leads && cx.sora.hp_max > 0 => cx.sora.hp as f32 / cx.sora.hp_max as f32,
        None => 1.0,
    };
    (sora > 0.0 && sora < line) || g(p).arts.friends.iter().any(|f| f.1 < line)
}

fn spot(at: Vec3, danger: Vec3) -> Vec3 {
    at + horizontal(danger - at).normalize_or_zero() * K.art_interpose_gap
}

/// Plays a copy of a clip without its Attack window and root motion: the hits and the move are ours.
fn play(p: &mut NpcPawn, clip: &'static str, rate: f32, looped: bool) {
    let c = shots::clip_copy(clip, true, false, true);
    if looped {
        p.anim.play_loop(c, K.combo_blend);
    } else {
        p.anim.play(c, K.combo_blend);
    }
    p.anim.speed = rate;
    p.hit_list.clear();
}

fn start(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost, art: Art) -> bool {
    let rank = p.status.gof_rank(art.mv());
    if rank == 0 || !open(p, art) || (bare_handed(p) && art != Art::Taunt) {
        return false;
    }
    // THE COOLDOWN (THE USER 2026-11-02: "...introduce a cooldown with a visible timer, they are way too spammable"): refused
    // with the refused sound, as short MP is, and nothing is spent (the MP stays)
    if cooldown_left(&p.status, art) > 0.0 {
        panel(p, MenuEvent::Refused);
        goofy::log(p, "art_cooling");
        return false;
    }
    let cover = if art == Art::Interpose { threatened(p, cx, host) } else { None };
    if art == Art::Interpose && cover.is_none() {
        return false;
    }
    let bodyguard = art == Art::Interpose && has(p, GA::Bodyguard) && friend_low(p, cx);
    if !bodyguard && !pay(p, mp(art, rank)) {
        panel(p, MenuEvent::Refused);
        return false;
    }
    goofy::cancel_action(p);
    p.cmc.speed_limit_override = None;
    p.cmc.walk_mode = false;
    p.anim.speed = 1.0;
    p.enter(NpcStateId::Pad(if art == Art::Taunt { TAUNT } else { ART }));
    if p.on_ground() {
        p.cmc.stop_movement();
    }
    let me = p.position();
    let want = goofy::target_info(p, host).map(|t| horizontal(t.position - me)).filter(|v| v.length() > 1.0).map(yaw_of).or_else(|| {
        let (y, _, a) = stick(p);
        a.then_some(y)
    });
    if art != Art::Interpose {
        goofy::face(p, want, K.combo_turn);
    }
    let mut r = ArtRun { art, rank, phase: 0, t: 0.0, next: 0.0, rate: 1.0, done: false, landed: false, dir: forward(p.yaw), friend: None, danger: me, goal: me };
    let at = p.capsule_centre();
    match art {
        Art::Charge => {
            r.phase = 1;
            r.rate = K.art_charge_rate;
            p.play_looped(K.art_charge_clip, K.guard_blend);
            // (COMBAT-FIX) the shield's own sphere on SGuardWalk's shield track (charge_tick keeps it there)
            let off = charge_shield(p);
            let mut a = area(art, rank, shots::local(p, off), K.art_charge_time, K.art_charge_radius, K.art_charge_kind);
            a.follow = Some(off);
            a.push = Push::Forward;
            a.hit_effect = K.art_charge_hit_effect;
            a.hit_se = K.art_charge_hit_se;
            shots::add(p, a);
            shots::sound(p, host, K.art_charge_se, at);
            shots::effect(p, host, K.art_charge_trail, at, 1.0);
        }
        Art::Tornado => {
            r.rate = K.art_tornado_rate;
            play(p, K.art_tornado_start_clip, r.rate, false);
        }
        Art::Bash => {
            r.rate = K.art_bash_rate;
            play(p, K.art_bash_clip, r.rate, false);
        }
        Art::Turbo => {
            r.rate = K.art_turbo_rate;
            play(p, K.art_turbo_start_clip, r.rate, false);
        }
        Art::Taunt => {
            r.rate = K.art_taunt_rate;
            p.play(K.art_taunt_clip, K.guard_blend);
            p.anim.speed = r.rate;
            let left = ranked(rank, [K.art_taunt_time, K.art_taunt_time_r2, K.art_taunt_time_r3]);
            gp(p).taunt = Some(goofy::Taunt { centre: me, radius: K.art_taunt_radius, left });
            if rank >= 3 {
                if let Some(gd) = p.status.guard.as_mut() {
                    gd.value = (gd.value + K.art_taunt_r3_guard).min(gd.max);
                }
            }
            // (THE USER 2026-11-02: "taunt shouldn't have the honeycomb shield effect") the bang is heard; the clip's own
            // glint is all that shows (the bash's hexagon VN_slg_bsh0 is no longer spawned here)
            shots::sound(p, host, K.art_taunt_se, at);
        }
        Art::Interpose => {
            let (f, feet, danger) = cover.unwrap_or((Friend::Sora, me, me));
            r.phase = 1;
            r.rate = K.art_interpose_rate;
            r.friend = Some(f);
            r.danger = danger;
            r.goal = spot(feet, danger);
            p.play_looped(K.art_interpose_clip, K.guard_blend);
            gp(p).hold_invincible = true;
            shots::sound(p, host, K.art_interpose_se, at);
            shots::effect(p, host, K.art_interpose_trail, at, 1.0);
        }
        Art::ShieldToss => {
            r.rate = K.art_toss_rate;
            play(p, if p.on_ground() { K.art_toss_clip } else { K.art_toss_air_clip }, r.rate, false);
        }
    }
    p.anim.speed = r.rate;
    let k = gp(p);
    k.hitstop = 0.0;
    k.arts.running = Some(r);
    k.arts.cooldown[art.index()] = cooldown(art); // the wait starts when it is used (THE USER 2026-11-02)
    goofy::log(p, art.word());
    panel(p, MenuEvent::ShortcutExecuted);
    true
}

/// A hit sphere of `art`.
fn area(art: Art, rank: u8, centre: Vec3, life: f32, radius: f32, kind: &str) -> Area {
    let row = SoloRow { power: power(art, rank), ..SoloRow::plain(art.row(), kind_of(kind)) };
    let mut a = Area::new(row, centre, life, radius, TAG);
    a.stagger = stagger(art, rank);
    a
}

// ---------------------------------------------------------------------------------------------- the moves

/// The Art's end, whatever ended it: the rate, the move, its hit spheres.
pub fn end(p: &mut NpcPawn) {
    p.anim.speed = 1.0;
    p.cmc.direct_xy = None;
    tidy(p);
}

/// The running Art is gone (something else took him out of its state): its book and its spheres. The new state's rate and hit-stop stay.
fn tidy(p: &mut NpcPawn) {
    let k = gp(p);
    k.arts.running = None;
    k.hold_invincible = false;
    k.shots.retain(|a| a.tag != TAG);
}

/// Pre-move: a parry for a friend or a hit's reaction took him out of the state.
pub fn tidy_step(p: &mut NpcPawn) {
    if g(p).arts.running.is_some() && !matches!(state_n(p), Some(ART | TAUNT)) {
        tidy(p);
    }
}

fn leave(p: &mut NpcPawn) {
    end(p);
    goofy::leave_to_ground(p);
}

/// Out of an Art into a guard stance held `stance` seconds with X let go.
fn into_stance(p: &mut NpcPawn, stance: f32) {
    end(p);
    gp(p).arts.stance = stance;
    goofy::start_guard(p);
}

fn drift(p: &mut NpcPawn, v: Vec3) {
    p.cmc.direct_xy = Some(Vec2::new(v.x, v.y));
}

/// (COMBAT-FIX) The shield now in SGuardWalk (`art.charge.shield_track`, KH3's ATK_00 per frame), his frame.
fn charge_shield(p: &NpcPawn) -> Vec3 {
    crate::actions::track_at(&K.art_charge_shield_track, p.anim.time)
}

/// (COMBAT-FIX) An area of his weapon's KH3 collision group `group` (`actions::attack_shape`), rebuilt on him each step.
fn group_area(p: &NpcPawn, art: Art, rank: u8, group: &'static str, life: f32, kind: &str) -> Area {
    let s = crate::actions::attack_shape(p, group);
    let mut a = area(art, rank, (s.a + s.b) * 0.5, life, s.radius, kind);
    a.group = Some(group);
    a
}

fn charge_tick(p: &mut NpcPawn, r: &ArtRun) -> bool {
    if r.t < K.art_charge_time && p.on_ground() {
        drift(p, forward(p.yaw) * K.art_charge_speed);
        // (COMBAT-FIX) the sphere rides the shield (THE USER 2026-11-06: "Goofy's ground combo hit box also extends out way
        // too far"; the coordinator: KH3 1:1)
        let off = charge_shield(p);
        if let Some(a) = gp(p).shots.iter_mut().rev().find(|a| a.tag == TAG) {
            a.follow = Some(off);
        }
        return true;
    }
    drift(p, Vec3::ZERO);
    let rank = r.rank;
    into_stance(p, K.art_charge_stance);
    // RANK 3: the stance opens as a fresh tap, so what lands on it right away is a Deflect
    if rank >= 3 {
        let (yaw, _, active) = stick(p);
        stm(p).core.press(yaw, active, StickRel::Neutral);
    }
    false
}

fn tornado_tick(p: &mut NpcPawn, r: &mut ArtRun, dt: f32) -> bool {
    match r.phase {
        0 => {
            if p.anim.finished {
                r.phase = 1;
                r.t = 0.0;
                r.rate = 1.0;
                play(p, K.art_tornado_loop_clip, 1.0, true);
            }
        }
        1 => {
            let (yaw, _, active) = stick(p);
            drift(p, if active { forward(yaw) * K.art_tornado_move_speed } else { Vec3::ZERO });
            r.next -= dt;
            if r.next <= 0.0 {
                r.next = K.art_tornado_tick;
                // (COMBAT-FIX) KH3's Attack_Tornado (M_1322's window): the prism round him, circumradius 130, not a 280 sphere
                let mut a = group_area(p, Art::Tornado, r.rank, "Attack_Tornado", (K.art_tornado_tick * 0.8).max(0.05), K.art_tornado_kind);
                a.hit_effect = K.art_tornado_hit_effect;
                a.hit_se = K.art_tornado_hit_se;
                shots::add(p, a);
            }
            if r.t >= K.art_tornado_time {
                r.phase = 2;
                r.t = 0.0;
                r.rate = K.art_tornado_rate;
                drift(p, Vec3::ZERO);
                play(p, K.art_tornado_end_clip, r.rate, false);
            }
        }
        _ => {
            if p.anim.finished {
                leave(p);
                return false;
            }
        }
    }
    true
}

fn bash_tick(p: &mut NpcPawn, r: &mut ArtRun) -> bool {
    if !r.done && p.anim.time >= K.art_bash_hit {
        r.done = true;
        let round = r.rank >= 3;
        // (COMBAT-FIX) KH3's FR_SHIELDGUARDIAN_HEAVYBASH (the Trinity Guard pawn's SGuardHeavyBash): a 150 sphere 80 ahead and
        // 150 up, through the clip's Attack window (art.bash.hit + art.bash.active); rank 3's ring is the approved design's
        let off = if round { Vec3::Z * K.combo_up } else { Vec3::new(K.art_bash_forward, 0.0, K.art_bash_up) };
        let radius = if round { K.art_bash_r3_radius } else { K.art_bash_radius };
        let mut a = area(Art::Bash, r.rank, shots::local(p, off), (K.art_bash_active / r.rate.max(0.01)).max(0.05), radius, K.art_bash_kind);
        a.follow = Some(off);
        a.hit_effect = K.art_bash_hit_effect;
        a.hit_se = K.art_bash_hit_se;
        shots::add(p, a);
    }
    let (_, _, active) = stick(p);
    if p.anim.finished || (active && p.anim.time >= K.art_bash_move) {
        leave(p);
        return false;
    }
    true
}

fn turbo_tick(p: &mut NpcPawn, r: &mut ArtRun, dt: f32) -> bool {
    match r.phase {
        0 => {
            if p.anim.finished {
                r.phase = 1;
                r.t = 0.0;
                r.rate = 1.0;
                r.dir = forward(p.yaw);
                play(p, K.art_turbo_loop_clip, 1.0, true);
                if r.rank >= 3 {
                    gp(p).invincible = K.art_turbo_r3_invincible;
                }
            }
        }
        1 => {
            // he leaves the ground: the ride is over, the fall is his
            if !p.on_ground() {
                leave(p);
                return false;
            }
            let (yaw, _, active) = stick(p);
            if active {
                let turn = wrap180(yaw - p.yaw).clamp(-K.art_turbo_turn * dt, K.art_turbo_turn * dt);
                p.yaw = wrap180(p.yaw + turn);
            }
            r.dir = forward(p.yaw);
            drift(p, r.dir * K.art_turbo_speed);
            r.next -= dt;
            if r.next <= 0.0 {
                r.next = K.art_turbo_tick;
                // (COMBAT-FIX) KH3's Attack_Body (M_1325's window): the upright capsule of radius 80 on his centre, not a 190
                // sphere 60 ahead
                let mut a = group_area(p, Art::Turbo, r.rank, "Attack_Body", (K.art_turbo_tick * 0.8).max(0.05), K.art_turbo_kind);
                a.push = Push::Forward;
                a.hit_effect = K.art_turbo_hit_effect;
                a.hit_se = K.art_turbo_hit_se;
                shots::add(p, a);
            }
            if r.t >= ranked(r.rank, [K.art_turbo_time, K.art_turbo_time_r2, K.art_turbo_time_r3]) {
                r.phase = 2;
                r.t = 0.0;
                r.rate = K.art_turbo_rate;
                play(p, K.art_turbo_end_clip, r.rate, false);
            }
        }
        _ => {
            let left = (1.0 - r.t / K.art_turbo_brake.max(0.01)).max(0.0);
            drift(p, r.dir * K.art_turbo_speed * left);
            if p.anim.finished {
                leave(p);
                return false;
            }
        }
    }
    true
}

fn taunt_tick(p: &mut NpcPawn) -> bool {
    let (_, _, active) = stick(p);
    if p.anim.finished || (active && p.anim.time >= K.art_taunt_move) {
        leave(p);
        return false;
    }
    true
}

fn interpose_tick(p: &mut NpcPawn, r: &mut ArtRun, cx: &StepCtx) -> bool {
    let friend = r.friend.unwrap_or(Friend::Sora);
    if let Some((_, at)) = friend_spots(p, cx).into_iter().find(|f| f.0 == friend) {
        r.goal = spot(at, r.danger);
    }
    let to = horizontal(r.goal - p.position());
    let step = K.art_interpose_speed * cx.dt;
    if to.length() > K.art_interpose_arrive.max(step) && r.t < K.art_interpose_dash_max {
        p.yaw = yaw_of(to);
        drift(p, to.normalize_or_zero() * K.art_interpose_speed);
        return true;
    }
    drift(p, Vec3::ZERO);
    let (left, hits) = (ranked(r.rank, [K.art_interpose_cover, K.art_interpose_cover_r2, K.art_interpose_cover_r3]), ranked(r.rank, [K.art_interpose_hits, K.art_interpose_hits_r2, K.art_interpose_hits_r3]));
    let danger = horizontal(r.danger - p.position());
    if danger.length() > 1.0 {
        p.yaw = yaw_of(danger);
    }
    into_stance(p, K.art_interpose_stance);
    gp(p).arts.cover = Some(Cover { friend, left, hits: hits as u8 });
    goofy::log(p, "cover");
    false
}

fn toss_tick(p: &mut NpcPawn, r: &mut ArtRun, host: &mut PartyHost) -> bool {
    if !r.done && p.anim.time >= K.art_toss_throw {
        r.done = true;
        throw(p, r.rank, host);
    }
    let (_, _, active) = stick(p);
    if p.anim.finished || (r.done && active && p.anim.time >= K.art_toss_move) {
        leave(p);
        return false;
    }
    true
}

/// The shield leaves his hand: at the target, else ahead of him.
fn throw(p: &mut NpcPawn, rank: u8, host: &mut PartyHost) {
    let from = p.capsule_centre() + forward(p.yaw) * p.params.capsule_radius;
    let target = goofy::target_info(p, host).filter(|t| t.center.distance(from) <= K.art_toss_range + t.radius);
    let goal = target.map_or(from + forward(p.yaw) * K.art_toss_range, |t| t.center);
    let dir = (goal - from).normalize_or_zero();
    let id = target.map_or(TargetId::NONE, |t| t.id);
    let mut body = Projectile::discus(p.id, from, dir, false, id, None);
    body.velocity = dir * K.art_toss_speed;
    gp(p).arts.toss = Some(Toss { body, rank, age: 0.0, back: false, target: target.map(|t| t.id), goal, hit: Vec::new(), bounced: false });
    goofy::log(p, "toss_out");
}

/// The MainProc of the states `goofy::ART`, `TAUNT`.
pub fn tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    let Some(mut r) = run(p) else {
        leave(p);
        return;
    };
    if p.state_time > K.art_max_time {
        leave(p);
        return;
    }
    if g(p).hitstop > 0.0 {
        gp(p).hitstop -= dt;
        p.anim.speed = 0.0;
        p.cmc.direct_xy = Some(Vec2::ZERO);
        if g(p).hitstop > 0.0 {
            return;
        }
        gp(p).hitstop = 0.0;
    }
    p.anim.speed = r.rate;
    r.t += dt;
    let keep = match r.art {
        Art::Charge => charge_tick(p, &r),
        Art::Tornado => tornado_tick(p, &mut r, dt),
        Art::Bash => bash_tick(p, &mut r),
        Art::Turbo => turbo_tick(p, &mut r, dt),
        Art::Taunt => taunt_tick(p),
        Art::Interpose => interpose_tick(p, &mut r, cx),
        Art::ShieldToss => toss_tick(p, &mut r, host),
    };
    if keep {
        gp(p).arts.running = Some(r);
    }
}

/// A hit of an Art landed: Stagger, threat, a slam's hit-stop.
pub fn on_landed(p: &mut NpcPawn, a: &Area, target: TargetId) {
    landed(p, target, a.stagger);
    let k = gp(p);
    if let Some(r) = k.arts.running.as_mut().filter(|r| r.art == Art::Bash && !r.landed) {
        r.landed = true;
        k.hitstop = K.art_bash_hitstop;
    }
}

fn landed(p: &mut NpcPawn, target: TargetId, amount: f32) {
    let core = &mut stm(p).core;
    core.events.push(GoofyEvent::Stagger { target, amount, why: StaggerWhy::Art });
    core.events.push(GoofyEvent::Threat { target, amount: K.threat_basic, why: ThreatWhy::Basic });
}

// ---------------------------------------------------------------------------------------------- what the rest of the kit asks

/// X ends it into the guard: Turbo, any time.
pub fn guard_cancel(p: &NpcPawn) -> bool {
    run(p).is_some_and(|r| r.art == Art::Turbo)
}

/// B jumps off the sled.
pub fn jump_open(p: &NpcPawn) -> bool {
    run(p).is_some_and(|r| r.art == Art::Turbo && r.phase == 1)
}

/// The spin deflects this hit: any bullet, from any side.
pub fn spin_deflects(p: &NpcPawn, class: AttackClass) -> bool {
    matches!(class, AttackClass::Projectile | AttackClass::Magic) && run(p).is_some_and(|r| r.art == Art::Tornado && r.phase == 1)
}

/// A bullet met the spin: no HP goes (R3, `guarded`), Stagger and threat on the shooter. Rank 3 bats it back (R18: a KH hit for the
/// look of it, the bullet itself stays cancelled).
pub fn spin_deflect(p: &mut NpcPawn, hit: &HitRecord, attacker: Option<TargetId>) -> HitResult {
    let rank = run(p).map_or(1, |r| r.rank);
    let at = p.capsule_centre();
    if let Some(target) = attacker.or(goofy::target_id(p)) {
        let core = &mut stm(p).core;
        core.events.push(GoofyEvent::Stagger { target, amount: K.art_tornado_deflect_stagger, why: StaggerWhy::Art });
        core.events.push(GoofyEvent::Threat { target, amount: K.threat_deflect, why: ThreatWhy::Deflect });
        if rank >= 3 {
            gp(p).arts.batted.push((target, hit.damage * K.art_tornado_r3_back));
        }
    }
    let k = gp(p);
    k.fx.push((K.deflect_spark, at));
    k.sfx.push((K.guard_block_se, at));
    goofy::log(p, "spin_deflect");
    HitResult { accepted: true, damage_dealt: 0.0, hp_after: p.status.hp as f32, killed: false, guarded: true }
}

// ---------------------------------------------------------------------------------------------- the party's step

/// The stance, the cover, the Arts' cooldowns, the taunt that goes where he goes (`hooks::clocks`).
pub fn clock(m: &mut NpcPawn, dt: f32) {
    let (at, ko) = (m.position(), m.status.hp < 1);
    let Some(s) = m.status.goofy.as_mut() else { return };
    let a = &mut s.run.arts;
    a.stance = (a.stance - dt).max(0.0);
    for c in a.cooldown.iter_mut() {
        *c = (*c - dt).max(0.0);
    }
    if let Some(c) = a.cover.as_mut() {
        c.left -= dt;
    }
    if ko || a.cover.is_some_and(|c| c.left <= 0.0) {
        a.cover = None;
    }
    if let Some(t) = s.run.taunt.as_mut() {
        t.centre = at;
    }
}

/// Notes the living friends for every Goofy (`ids`: the members).
pub fn note_friends(party: &mut Party, ids: &[MemberId]) {
    let list: Vec<(MemberId, f32)> = ids.iter().filter_map(|i| party.member(*i)).filter(|m| m.status.hp > 0).map(|m| (m.id, m.status.hp as f32 / m.status.max_hp().max(1) as f32)).collect();
    let friend_leads = ids.iter().filter_map(|i| party.member(*i)).any(|m| m.pad.is_some());
    for id in ids {
        if let Some(s) = party.member_mut(*id).and_then(|m| m.status.goofy.as_mut()) {
            s.run.arts.friends = list.iter().copied().filter(|f| f.0 != *id).collect();
            s.run.arts.friend_leads = friend_leads;
        }
    }
}

/// The party's step of the Arts for one Goofy, whatever state he is in: the batted shots and the shield's flight.
pub fn step(p: &mut NpcPawn, dt: f32, host: &mut PartyHost) {
    for (shooter, damage) in std::mem::take(&mut gp(p).arts.batted) {
        let Some(t) = host.targets.info(shooter) else { continue };
        let row = SoloRow { flat: Some(damage.max(1.0)), ..SoloRow::plain(Art::Tornado.row(), kind_of(K.batted_kind)) };
        counters::bat_back(p, host, &t, crate::projectile::Batted { row, stagger: K.art_tornado_deflect_stagger });
    }
    toss_step(p, dt, host);
}

/// The shield is back in his hand.
fn caught(p: &mut NpcPawn, host: &mut PartyHost) {
    gp(p).arts.toss = None;
    let at = p.capsule_centre();
    shots::sound(p, host, K.art_toss_catch_se, at);
    goofy::log(p, "toss_back");
}

/// One step of the flight. It runs on its own clock, in any state he is in, and always ends: a target that vanished turns it back,
/// and at `art.toss.max_time`, or when he goes down, it is simply in his hand again.
fn toss_step(p: &mut NpcPawn, dt: f32, host: &mut PartyHost) {
    let Some(mut t) = g(p).arts.toss.clone() else { return };
    t.age += dt;
    if t.age >= K.art_toss_max_time || p.state == NpcStateId::Die || p.hurt.dying || p.status.hp < 1 {
        caught(p, host);
        return;
    }
    let live = t.target.and_then(|id| host.targets.info(id)).filter(shots::lockable);
    if t.target.is_some() && live.is_none() {
        t.target = None;
        t.back = true;
    }
    let aim = if t.back { p.capsule_centre() } else { live.map_or(t.goal, |i| i.center) };
    let to = aim - t.body.position;
    let step = K.art_toss_speed * dt;
    if t.back && to.length() <= step.max(K.art_toss_catch) {
        caught(p, host);
        return;
    }
    if to.length() <= step {
        t.body.position = aim;
        t.back = true;
    } else {
        t.body.velocity = to.normalize_or_zero() * K.art_toss_speed;
        t.body.position += t.body.velocity * dt;
    }
    let row = SoloRow { power: power(Art::ShieldToss, t.rank), ..SoloRow::plain(Art::ShieldToss.row(), kind_of(K.art_toss_kind)) };
    let pos = t.body.position;
    for info in shots::hittables(host) {
        if t.hit.contains(&info.id) {
            continue;
        }
        let Some(s) = shots::touch(host, &info, pos, K.art_toss_radius) else { continue };
        t.hit.push(info.id);
        let at = (s.a + s.b) * 0.5;
        if shots::deliver(p, &row, info.id, s.part, at, t.body.velocity, host).is_none() {
            continue;
        }
        shots::effect(p, host, K.art_toss_hit_effect, at, 1.0);
        shots::sound(p, host, K.art_toss_hit_se, at);
        landed(p, info.id, stagger(Art::ShieldToss, t.rank));
        if t.back {
            continue;
        }
        // RANK 3: on to the next enemy, once; else home
        let near = |i: &kh_core::host::TargetInfo| i.center.distance(pos);
        let next = shots::enemies(host).into_iter().filter(|i| t.rank >= 3 && !t.bounced && !t.hit.contains(&i.id) && near(i) <= K.art_toss_r3_bounce_range).min_by(|a, b| near(a).total_cmp(&near(b)));
        match next {
            Some(i) => {
                t.bounced = true;
                t.target = Some(i.id);
                goofy::log(p, "toss_bounce");
            }
            None => {
                t.back = true;
                t.target = None;
            }
        }
    }
    gp(p).arts.toss = Some(t);
}

// ---------------------------------------------------------------------------------------------- the cover (R14)

fn covers(m: &NpcPawn, friend: Friend, at: Vec3) -> bool {
    m.status.goofy.as_ref().is_some_and(|s| s.run.arts.cover.is_some_and(|c| c.friend == friend)) && horizontal(m.position() - at).length() <= K.art_interpose_leash
}

/// R14, INTERPOSE: a blow aimed at the covered friend standing at `at`: Goofy parries it for them. True: it is absorbed.
fn cover_hit(party: &mut Party, friend: Friend, at: Vec3, hit: &HitRecord) -> bool {
    if !hit.takes_hp() || hit.damage < 1.0 {
        return false;
    }
    let Some(id) = party.members().find(|g| party.member(*g).is_some_and(|m| covers(m, friend, at))) else { return false };
    let Some(m) = party.member_mut(id) else { return false };
    if !goofy::parry_for_friend(m, hit) {
        return false;
    }
    goofy::log(m, "interpose");
    let a = &mut gp(m).arts;
    if let Some(c) = a.cover.as_mut() {
        c.hits = c.hits.saturating_sub(1);
    }
    if a.cover.is_some_and(|c| c.hits == 0) {
        a.cover = None;
    }
    true
}

/// A hit on the member `id` (`Party::receive_hit`): Some = Interpose absorbed it (the friend's side, as a Ward absorbs).
pub fn interpose(party: &mut Party, id: MemberId, hit: &HitRecord) -> Option<HitResult> {
    let at = party.member(id)?.position();
    if !cover_hit(party, Friend::Member(id), at, hit) {
        return None;
    }
    let hp = party.member(id)?.status.hp as f32;
    Some(HitResult { accepted: false, damage_dealt: 0.0, hp_after: hp, killed: false, guarded: true })
}

impl Party {
    /// (GF-ARTS) The Customize block's [LB + A, LB + B] for every Goofy, leader or AI (kh-partyhost feeds it each step).
    pub fn set_goofy_arts(&mut self, arts: [u8; 2]) {
        for id in self.members().collect::<Vec<_>>() {
            if let Some(s) = self.member_mut(id).and_then(|m| m.status.goofy.as_mut()) {
                s.run.arts.equipped = Some(arts);
            }
        }
    }

    /// (GF-ARTS) What the Customize block of the Goofy in the party reads (None: he is not in it).
    pub fn goofy_art_book(&self) -> Option<ids::ArtBook> {
        self.members().filter_map(|i| self.member(i)).find(|m| m.status.goofy.is_some()).map(|m| m.status.gof_art_book())
    }

    /// (GF-ARTS, R14) A hit on Sora, who is no member (the hosts ask before `Sora::take_hit`): true = Interpose absorbed it and he
    /// takes nothing. `at`: his feet; `from`: where the blow came from; `attacker`: who threw it, when known.
    pub fn cover_sora(&mut self, at: Vec3, damage: f32, from: Vec3, attacker: Option<TargetId>) -> bool {
        let mut hit = HitRecord::empty(TargetId::NONE);
        hit.damage = damage;
        hit.attacker = kh_core::host::HitAttacker { position: from, yaw: 0.0, radius: 0.0, valid: true };
        hit.attack_info.attacker = attacker;
        cover_hit(self, Friend::Sora, at, &hit)
    }
}

#[cfg(test)]
#[path = "goofy_arts_tests.rs"]
mod tests;

//! THE AI-WORLD INPUTS (contract ER.3; was kh-app party_glue.rs `AppAiWorld` and party_play.rs `LeaderAiWorld`).
//!
//! The host gives the runtime its enemies through [`PartyTargets`]: the same object is the friends' hit targets
//! (`kh_core::Targets`, contract B.6) and the AI's enemy list. The test level implements it over its training
//! dummies; the Elden Ring host over ErTargets (stream S1). Everything else the trees see (the player, the allies,
//! the camera, member MP, the game level) the runtime builds itself from Sora and the party.

use kh_ai::{ActorRef, ActorView};
use kh_core::{Output, TargetId, Vec3};
use kh_party::MemberView;

/// The host's targets as the party sees them: what the friends hit (`kh_core::Targets`) and what the AI calls
/// enemies.
pub trait PartyTargets: kh_core::Targets {
    /// Every actor of the enemy side as kh-ai sees it, in the host's order: `actor: ActorRef::Enemy(id)` with the
    /// target id, `center` = the look-at / lock-on point (the test dummies: position + half height). Bystanders are
    /// listed too with `hostile: false` (the trees' enemy list leaves them out; an actor look-up still finds them).
    fn ai_actors(&self, out: &mut Vec<ActorView>);
    /// An enemy attacked the player lately (`PlayerView::attacked_by_enemies`; the test dummies: within 60 steps).
    fn attacked_by_enemies(&self) -> bool;
    /// (FX5) THE HOST'S MEMORY OF A FIGHT WITH THE PARTY: enemy `id` hit a member (the leader, AI Sora, a friend's body)
    /// or was hit by one lately (Elden Ring: ErTargets' hit / attack memory, 5 s). Such an enemy stays in the trees'
    /// enemy list wherever it stands (`EnemyFeed::keep_threats`). The default: no memory (only the range counts).
    fn fighting_party(&self, _id: u32) -> bool {
        false
    }
}

/// (FX5, OUR DESIGN: KH3's enemies exist only inside a battle area, Elden Ring's whole map is full of them) THE FRIENDS
/// FIGHT WHAT THREATENS THE PARTY. KH3's own target choice (EQ_NpcTarget_LockOnTarget) has NO distance filter: it only
/// SCORES the player's distance against a 30 m border (TresNpcEnvQueryTest_BorderDistance m_Border 3000), because every
/// enemy it lists is already in the battle area. Fed Elden Ring's characters within `target.range` (40 m), Donald cast at
/// a camp 30 m away while Goofy led (CV3 run 2, during Melina's talk). So the trees' enemy list holds an enemy only while
/// it threatens the party: within this range (cm, 3-D, to the enemy's edge) of the leader or of any member who is up
/// (AI Sora too), or fighting the party by the host's memory (`PartyTargets::fighting_party`), or the leader's hard
/// lock-on target (the player chose it). Every other enemy is a bystander for the trees (an actor look-up still finds
/// it). The same rule whoever leads. 1200 cm: the party is put in battle at 800 cm (`PARTY_AGGRO_RANGE`, Elden Ring's
/// `target.engage_range`) or by a blow; once it is, the friends (Donald's spells, AI Sora's ranged tools) also take on
/// an enemy closing in a few metres beyond that, never a camp 20 - 40 m off unless it fights the party (an archer that
/// hits a member is in by the memory). Elden Ring host key `party.threat_range` (cm; 0 = off: every hostile, as before).
pub const THREAT_RANGE: f32 = 1200.0;

static THREAT_RANGE_NOW: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(THREAT_RANGE.to_bits());

/// (FX5) The threat range in use (cm; `THREAT_RANGE` until a host sets another). 0 or less = the filter is off (every
/// hostile listed, the behaviour before FX5).
pub fn threat_range() -> f32 {
    f32::from_bits(THREAT_RANGE_NOW.load(std::sync::atomic::Ordering::Relaxed))
}

static LEFT_OUT_TOLD: std::sync::Mutex<(usize, f64)> = std::sync::Mutex::new((0, f64::MIN));

/// (FX5) For the log: true when the count of hostiles left out of the trees differs from the last one told, at most
/// once a second (`now`: the host clock, s).
pub fn left_out_changed(n: usize, now: f64) -> bool {
    let Ok(mut told) = LEFT_OUT_TOLD.lock() else { return false };
    if told.0 == n || (now - told.1).abs() < 1.0 {
        return false;
    }
    *told = (n, now);
    true
}

/// (FX5) A host's key for the threat range (Elden Ring: `party.threat_range`); `None` = the default.
pub fn set_threat_range(cm: Option<f32>) {
    let v = cm.filter(|v| v.is_finite()).unwrap_or(THREAT_RANGE);
    THREAT_RANGE_NOW.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// (FX5) Does enemy `a` threaten the party? `party`: the feet of the leader and of every member who is up (AI Sora too);
/// `lock_on`: the leader's hard lock-on target; `fighting`: the host's memory (`PartyTargets::fighting_party`).
pub fn threatens(a: &ActorView, party: &[Vec3], lock_on: TargetId, range: f32, fighting: &dyn Fn(u32) -> bool) -> bool {
    let ActorRef::Enemy(id) = a.actor else { return true };
    if range <= 0.0 || (lock_on != TargetId::NONE && lock_on.0 == id) || fighting(id) {
        return true;
    }
    party.iter().any(|p| (a.position - *p).length() - a.radius <= range)
}

/// The enemies of one moment (taken before the AI ticks: the trees do not change them).
#[derive(Clone, Debug, Default)]
pub struct EnemyFeed {
    pub actors: Vec<ActorView>,
    pub attacked: bool,
    /// (FX5) Hostiles left out of the trees by `keep_threats` (not threatening the party), for the log.
    pub left_out: usize,
}

impl EnemyFeed {
    pub fn of(t: &dyn PartyTargets) -> EnemyFeed {
        let mut actors = Vec::new();
        t.ai_actors(&mut actors);
        EnemyFeed { actors, attacked: t.attacked_by_enemies(), left_out: 0 }
    }

    /// (FX5) The feed of the friends' trees: `of`, then only the enemies that threaten the party stay hostile
    /// (`threatens`); the others become bystanders.
    pub fn of_party(t: &dyn PartyTargets, party: &[Vec3], lock_on: TargetId) -> EnemyFeed {
        let mut feed = EnemyFeed::of(t);
        feed.left_out = feed.keep_threats(party, lock_on, threat_range(), &|id| t.fighting_party(id));
        feed
    }

    /// (FX5) Every hostile that does not threaten the party becomes a bystander; returns how many did.
    pub fn keep_threats(&mut self, party: &[Vec3], lock_on: TargetId, range: f32, fighting: &dyn Fn(u32) -> bool) -> usize {
        let mut n = 0;
        for a in self.actors.iter_mut().filter(|a| a.hostile) {
            if !threatens(a, party, lock_on, range, fighting) {
                a.hostile = false;
                n += 1;
            }
        }
        n
    }

    /// (ZETTA) ZETTAFLARE'S TARGET IS LEFT ALONE (the user: "the allies you have target everybody else except for the
    /// zettaflare's target"): from the channel's start to the hit it is a bystander of the friends' trees (AI Sora, Goofy,
    /// any AI member): they pick and fight the others, and with no other enemy they stay by the leader. Returns how many.
    pub fn reserve(&mut self, ids: &[TargetId]) -> usize {
        let mut n = 0;
        for a in self.actors.iter_mut().filter(|a| a.hostile) {
            if let ActorRef::Enemy(id) = a.actor {
                if ids.contains(&TargetId(id)) {
                    a.hostile = false;
                    n += 1;
                }
            }
        }
        n
    }

    /// The trees' enemy list (bystanders left out).
    pub fn enemies(&self, out: &mut Vec<ActorView>) {
        out.extend(self.actors.iter().filter(|a| a.hostile).cloned());
    }

    pub fn actor(&self, id: u32) -> Option<ActorView> {
        self.actors.iter().find(|a| a.actor == ActorRef::Enemy(id)).cloned()
    }

    /// The look-at / lock-on point of an enemy.
    pub fn centre(&self, id: TargetId) -> Option<Vec3> {
        self.actor(id.0).map(|a| a.center)
    }
}

/// A member as an AI actor.
pub fn member_actor(v: &MemberView) -> ActorView {
    let (radius, half) = if v.capsule.0 > 0.0 { v.capsule } else { (34.0, 70.0) };
    ActorView {
        actor: ActorRef::Member(v.id.0),
        uid: v.chara.uid() as i32,
        position: v.position,
        center: v.position + Vec3::Z * half,
        yaw: v.yaw,
        velocity: v.velocity,
        radius,
        half_height: half,
        alive: !v.ko,
        hostile: false,
        lockable: false,
        airborne: !v.on_ground,
        hp: v.hp as f32,
        hp_max: v.hp_max as f32,
        state: v.state_name,
        ..ActorView::default()
    }
}

/// The straight-line navigation stand-in (open note N1) when the host gives none.
pub static STRAIGHT_NAV: kh_ai::StraightNav = kh_ai::StraightNav { floor_z: None };

/// THE AI'S WORLD WHILE SORA LEADS (contract C.3): Sora as the player, the host's enemies, the friends as allies,
/// the drawn camera, the host's navigation.
pub struct SoraAiWorld<'a> {
    pub sora: &'a Output,
    pub status: &'a kh_core::Status,
    pub in_battle: bool,
    pub attacking: bool,
    pub lock_on: TargetId,
    pub enemies: &'a EnemyFeed,
    pub members: &'a [MemberView],
    pub camera: kh_ai::CameraView,
    pub nav: &'a dyn kh_ai::Nav,
    pub game_level: u8,
    /// (party I2) `PartyRuntime::member_mps`: the members' MP for `member_mp`.
    pub mps: &'a [(u8, (i32, i32, bool, f32))],
}

impl SoraAiWorld<'_> {
    fn player_actor(&self) -> ActorView {
        let o = self.sora;
        ActorView { actor: ActorRef::Player, uid: 0, position: o.position, center: o.capsule_center, yaw: o.yaw, velocity: o.velocity, alive: self.status.hp > 0, airborne: !o.on_floor, hp: self.status.hp as f32, hp_max: self.status.max_hp() as f32, state: o.state.name(), ..ActorView::default() }
    }
}

impl kh_ai::AiWorld for SoraAiWorld<'_> {
    fn player(&self) -> kh_ai::PlayerView {
        let lock = if self.lock_on == TargetId::NONE { ActorRef::None } else { ActorRef::Enemy(self.lock_on.0) };
        kh_ai::PlayerView {
            actor: self.player_actor(),
            floor_position: self.sora.position,
            on_ground: self.sora.on_floor,
            hp: self.status.hp,
            hp_max: self.status.max_hp(),
            mp: self.status.mp,
            mp_max: self.status.max_mp(),
            dead: self.status.hp < 1,
            in_battle: self.in_battle,
            attacking: self.attacking,
            attacked_by_enemies: self.enemies.attacked,
            lock_on: lock,
            ..kh_ai::PlayerView::default()
        }
    }
    fn camera(&self) -> kh_ai::CameraView {
        self.camera
    }
    // (party P, C4 relay) Sora's focus gauge for the cure decision's focus need (FUN_141269f60)
    fn player_focus(&self) -> Option<(i32, i32)> {
        Some((self.status.focus, self.status.focus_max))
    }
    // (party I2, C3 relay) another member's MP for the cure decision's MP need (FUN_14126af20 / FUN_14126b550)
    fn member_mp(&self, member: u8) -> Option<(i32, i32, bool, f32)> {
        self.mps.iter().find(|m| m.0 == member).map(|m| m.1)
    }
    fn enemies(&self, out: &mut Vec<ActorView>) {
        self.enemies.enemies(out)
    }
    fn allies(&self, out: &mut Vec<ActorView>) {
        out.extend(self.members.iter().map(member_actor));
    }
    fn actor(&self, actor: ActorRef) -> Option<ActorView> {
        match actor {
            ActorRef::None => None,
            ActorRef::Player => Some(self.player_actor()),
            ActorRef::Member(m) => self.members.iter().find(|v| v.id.0 == m).map(member_actor),
            ActorRef::Enemy(id) => self.enemies.actor(id),
        }
    }
    fn game_level(&self) -> u8 {
        self.game_level
    }
    fn nav(&self) -> &dyn kh_ai::Nav {
        self.nav
    }
}

/// Sora's actor id among the allies while a friend leads (not a party member: kh-party answers nothing for it).
pub const SORA_ACTOR: u8 = 7;

/// THE AI'S WORLD WHILE A FRIEND LEADS (contract C.3, our design): the leader is "the player" of every tree (the
/// friends follow and guard him); Sora, when he is in the party, is an ally (actor `Member(SORA_ACTOR)`).
pub struct LeaderAiWorld<'a> {
    pub leader: &'a MemberView,
    pub lock_on: TargetId,
    pub in_battle: bool,
    pub members: &'a [MemberView],
    pub sora: Option<(&'a Output, &'a kh_core::Status)>,
    pub enemies: &'a EnemyFeed,
    pub camera: kh_ai::CameraView,
    pub nav: &'a dyn kh_ai::Nav,
    pub game_level: u8,
    /// (party I2) `PartyRuntime::member_mps` (the friends' MP; Sora's from `sora`) for `member_mp`.
    pub mps: &'a [(u8, (i32, i32, bool, f32))],
}

impl LeaderAiWorld<'_> {
    fn player_actor(&self) -> ActorView {
        ActorView { actor: ActorRef::Player, ..member_actor(self.leader) }
    }
    fn sora_actor(&self) -> Option<ActorView> {
        let (o, s) = self.sora?;
        Some(ActorView { actor: ActorRef::Member(SORA_ACTOR), uid: 0, position: o.position, center: o.capsule_center, yaw: o.yaw, velocity: o.velocity, radius: 34.0, half_height: 89.0, alive: s.hp > 0, airborne: !o.on_floor, hp: s.hp as f32, hp_max: s.max_hp() as f32, state: crate::lead::sora_state_name(o.state), ..ActorView::default() }) // (party AIX) an AI ally's state names (his own tree's): the friends' support / cure branches check TargetActor in IDLE / TURN / AI_MOVE, which "TSID_STATE_RUN" never was
    }
}

impl kh_ai::AiWorld for LeaderAiWorld<'_> {
    /// (C4 relay) a friend leading: Donald and Goofy have no focus gauge (no focus need)
    fn player_focus(&self) -> Option<(i32, i32)> {
        None
    }
    // (party I2, C3 relay) another member's MP for the cure decision: the friends' from the runtime, Sora's (an ally
    // while a friend leads) from his status (pawn+0x7C0 bit 1 / +0x7C4)
    fn member_mp(&self, member: u8) -> Option<(i32, i32, bool, f32)> {
        if member == SORA_ACTOR {
            return self.sora.map(|(_, s)| (s.mp, s.max_mp(), s.mp_charging, s.mp_charge_timer.max(0.0)));
        }
        self.mps.iter().find(|m| m.0 == member).map(|m| m.1)
    }
    fn player(&self) -> kh_ai::PlayerView {
        let l = self.leader;
        let lock = if self.lock_on == TargetId::NONE { ActorRef::None } else { ActorRef::Enemy(self.lock_on.0) };
        kh_ai::PlayerView {
            actor: self.player_actor(),
            floor_position: l.position,
            on_ground: l.on_ground,
            hp: l.hp,
            hp_max: l.hp_max,
            mp: l.mp,
            mp_max: l.mp_max,
            dead: l.ko,
            in_battle: self.in_battle,
            attacking: l.state == kh_party::NpcStateId::Action || l.state == kh_party::NpcStateId::Pad(kh_party::pad::AIR_ATTACK),
            attacked_by_enemies: self.enemies.attacked,
            lock_on: lock,
            ..kh_ai::PlayerView::default()
        }
    }
    fn camera(&self) -> kh_ai::CameraView {
        self.camera
    }
    fn enemies(&self, out: &mut Vec<ActorView>) {
        self.enemies.enemies(out)
    }
    fn allies(&self, out: &mut Vec<ActorView>) {
        out.extend(self.members.iter().filter(|v| v.id != self.leader.id).map(member_actor));
        out.extend(self.sora_actor());
    }
    fn actor(&self, actor: ActorRef) -> Option<ActorView> {
        match actor {
            ActorRef::None => None,
            ActorRef::Player => Some(self.player_actor()),
            ActorRef::Member(SORA_ACTOR) => self.sora_actor(),
            ActorRef::Member(m) if m == self.leader.id.0 => Some(self.player_actor()),
            ActorRef::Member(m) => self.members.iter().find(|v| v.id.0 == m).map(member_actor),
            ActorRef::Enemy(id) => self.enemies.actor(id),
        }
    }
    fn game_level(&self) -> u8 {
        self.game_level
    }
    fn nav(&self) -> &dyn kh_ai::Nav {
        self.nav
    }
}

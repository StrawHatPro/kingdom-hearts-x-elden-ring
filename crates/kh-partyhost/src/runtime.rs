//! THE PARTY RUNTIME (contract ER.1; was kh-app party_glue.rs, stream F, + P / P2 / B4 / B5 / C4 / D3 / D4 / D5 / I1 / I2
//! additions): the one place a host touches the party streams' APIs (docs/party-contracts.md).
//!
//! - kh-party (stream B): the party, stepped right after Sora in the same fixed step (contract B.3), with the host's
//!   collision and its enemies as the friends' targets (Sora and the friends are never in that list); the shared EXP
//!   total handed over after every step (stats spec 5.2: each friend levels by its own table from Sora's total);
//!   Sora's heals from the friends applied to him.
//! - kh-ai (stream C): one controller per member, ticked before the party's step (contract C.1).
//! - kh-party team (stream D): the team attacks' friend side after the party, their state-event cameras.
//! - The HUD feed (kh_host::party, contract F.1 / F.2): the friends' gauges and faces, their level-up windows, the
//!   camp menu's party pages and the edits it makes.
//! - The test loadout of sheet `party-test` (level 99 by the shared EXP, weapons, items, AI settings).
//! - The leader (stream P, lead.rs): the formation, the pad's frame to a leading friend, AI Sora.
//!
//! The friends' armour / accessory slots, their counts and the AI bytes are kh-party's FriendStatus (contract B.8).

use std::sync::Arc;

use kh_core::{Output, Sora, TargetId};
use kh_host::party::{FriendAiSettings, PartyChara, PartyHudSlot, PartyMenuEdit, PartyMenuMember};
use kh_party::{Chara, MemberId, MemberView, Party, PartyConfig, PartyEventKind, PartyHost, SoraView};

use crate::aiworld::{EnemyFeed, LeaderAiWorld, PartyTargets, SoraAiWorld, STRAIGHT_NAV};
use crate::lead::Leadership;
use crate::loadout::{PartyTest, PartyWeapon};
use crate::say;

/// Whom an enemy's fixture attack hits (the test level's `dummy-target N ...`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Victim {
    /// The nearest living of Sora and the friends (default).
    Near,
    Sora,
    Friend(PartyChara),
}

/// A friend's level-up for the HUD: (who, new level, Strength, Magic, Defense, AP gained).
pub type LevelUp = (PartyChara, i32, [i32; 4]);

/// Where the AI pack comes from when the party is spawned without one (the host's loader; files are the host's).
pub type AiPackSource = Box<dyn Fn() -> Option<Arc<kh_ai::AiPack>> + Send + Sync>;

/// What the runtime is made from (the host reads the files).
pub struct RuntimeConfig {
    /// `--party` (default) / `--no-party` / the words `party on|off`.
    pub wanted: bool,
    /// Sheet `party-test` (`PartyTest::from_rows`).
    pub test: PartyTest,
    /// Sheet `party-weapons` (`PartyWeapon::from_rows`).
    pub weapons: Vec<PartyWeapon>,
    /// The AI pack (build/sora_pack/party/ai), when the host has it now.
    pub ai_pack: Option<Arc<kh_ai::AiPack>>,
    /// Asked again when the party spawns without a pack.
    pub ai_pack_source: Option<AiPackSource>,
}

/// One party step's inputs (contract ER.1 `step(StepIn) -> StepOut`).
pub struct StepIn<'a> {
    pub dt: f32,
    pub sora: &'a mut Sora,
    /// Sora's output of this step (his step ran just before).
    pub out: &'a Output,
    /// The HUD's lock-on target (Sora's).
    pub lock_on: TargetId,
    /// The drawn camera of the last step (the AI's camera, the Warp state's view test).
    pub camera: &'a kh_camera::CameraView,
    pub world: &'a dyn kh_core::World,
    /// The enemies: the friends' hit targets and the AI's enemy list.
    pub targets: &'a mut dyn PartyTargets,
    /// The host's sound / effect seams: every party sound and effect of this step goes through them.
    pub audio: &'a mut dyn kh_core::Audio,
    pub effects: &'a mut dyn kh_core::Effects,
    /// The host's navigation (None = the straight-line stand-in).
    pub nav: Option<&'a dyn kh_ai::Nav>,
    /// Seconds of play (log lines only).
    pub now: f64,
}

/// A party spell actor or team projectile for the effect side: the seam handle of its body effect, where it is
/// and where it flies (kh-app party_fx `ProjectileTap`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileTap {
    pub effect: u32,
    pub position: kh_core::Vec3,
    pub forward: kh_core::Vec3,
}

/// What one party step leaves for the host.
#[derive(Clone, Debug, Default)]
pub struct StepOut {
    /// The party's live spell actors and the team link pawns' meteors / missiles (their body effects follow them).
    pub projectiles: Vec<ProjectileTap>,
    /// The party stepped (members present).
    pub stepped: bool,
    /// (ER party S1) the party's events of this step (kh-party `PartyOutput::events`: items used, KOs, hits ...), for the
    /// host's own rules (the Elden Ring host: an item's status clearing, S2's `status::on_party_item`).
    pub events: Vec<kh_party::PartyEvent>,
}

pub struct PartyRuntime {
    pub party: Party,
    /// Spawned members (HUD slot order).
    pub members: Vec<(PartyChara, MemberId)>,
    /// `--party` (default) / `--no-party` / the words `party on|off`.
    pub wanted: bool,
    pub test: PartyTest,
    pub weapons: Vec<PartyWeapon>,
    /// The views of the two latest steps (drawing interpolates them).
    pub previous: Vec<MemberView>,
    pub current: Vec<MemberView>,
    /// Level-ups not yet shown by the HUD.
    pub level_ups: Vec<LevelUp>,
    /// (GOOFY-GG) The level-ups (character, new level) that raised GUARD's max, not yet shown: the host adds the line
    /// `kh_party::guard::GP.hud_level_up_line` to that level-up's window.
    pub guard_ups: Vec<(PartyChara, i32)>,
    /// (BN) The kit abilities a level-up taught (character, new level, the abilities' names), not yet shown: the host adds
    /// them to that level-up's LEVEL UP! window with the ability sparkle ([`PartyRuntime::take_level_abilities`]).
    pub level_abilities: Vec<(PartyChara, i32, Vec<String>)>,
    victims: Vec<(u32, Victim)>,
    pub trace: bool,
    last_exp: i32,
    /// The friends' AI settings changed (Customize, words): the AI (stream C) is told.
    pub ai_dirty: bool,
    /// (stream D) the team attacks' friend side, stepped after the party (contract D.3).
    pub team: kh_party::team::Team,
    /// Sora's heals on friends during his step (Cure, items, Assist Friends), applied before the party's step.
    pub pending_heals: Vec<(MemberId, i32, u8)>,
    /// AI settings given before the friends were spawned (applied at spawn), and those of a friend slotted out
    /// (party D5: kept for him, given back when he joins again).
    pending_ai: Vec<(PartyChara, FriendAiSettings)>,
    /// (stream C) the AI pack and one controller per member (ticked before the party's step, contract C.1), the
    /// party's shared AI state, the last trace line shown per member (`party-trace`).
    pub ai_pack: Option<Arc<kh_ai::AiPack>>,
    pub ai_pack_source: Option<AiPackSource>,
    pub ai: Vec<(PartyChara, kh_ai::AiController)>,
    pub ai_shared: kh_ai::AiShared,
    last_trace: [String; 2],
    /// (stream D) the team attacks' state-event cameras (contract D.4) and the actor actions of the last team step.
    pub cams: kh_camera::camera_anim::StateEventCameras,
    actor_actions: Vec<u8>,
    /// (stream D) what the team cameras add to the drawn camera this step: their FOV (None = the field camera's
    /// own) and the blueprints' shakes (the host's camera applies both).
    pub team_fov: Option<f32>,
    pub team_shake: kh_camera::shake::ShakeOffset,
    /// (ZETTA round 5) The Zettaflare event's white over the picture this step (0..1; kh_party ZettaView::white): the hosts
    /// draw it (the test level: kh-app finish.rs' white plate).
    pub zetta_white: f32,
    /// (ZETTA round 5) Zettaflare's camera began handing back this step: the host puts its gameplay camera behind the leader
    /// (Donald) now (`team_camera` returns true for it too; the test level reads and clears this).
    pub zetta_behind: bool,
    zetta_handing: bool,
    /// (ZETTA 2026-10-23) Zettaflare's camera against the level: the way the present shot is kept clear of walls
    zetta_solve: ZettaSolve,
    /// (ZETTA 2026-10-23) the AI friends' loop guard (hopguard.rs) and Zettaflare's span as of the last step
    hop_guard: crate::hopguard::HopGuard,
    zetta_span_was: bool,
    /// (ZETTA 2026-10-23) friends caught in a move loop this step, for the host's put-back (the ER glue's warp)
    pub hop_loops: Vec<kh_party::MemberId>,
    /// (ZFIN round 7) Zettaflare's cinematic has the view this step (any share, from the cast's take-over to the end of the
    /// hand-back): the hosts hide KH3's HUD and the lock-on reticle meanwhile.
    pub zetta_cinematic: bool,
    /// (party I2) Sora's look-at target point (his lock-on component's look-at bone, `MyLookAtTargetBoneName` "t_camera"
    /// [A p_ex001_PawnBase], vt+0x6F0 0x1405BAAB0) as the last drawn pose has it: set by the host before `pose`.
    pub sora_look_point: Option<kh_core::Vec3>,
    /// (party I2) The enemies' centres after the last step (an enemy focus's look-at point).
    enemy_centres: Vec<(TargetId, kh_core::Vec3)>,
    /// Sora's team abilities were equipped (sheet party-test sora.team_abilities), once.
    sora_team_done: bool,
    /// (party P) the member the pad controls (None = Sora leads); which friends the party formation has in the
    /// active party (Donald, Goofy); the speed-matched loop's last run time per friend (lead.rs).
    pub leader: Option<MemberId>,
    pub active_friends: [bool; 2],
    pub loco_time: [f32; 3],
    /// (DON-MAGIC part A) TEST / DEBUG `party don-warp`: Donald held on a spot (cm) facing a yaw, every step, until `party don-free`.
    pub don_hold: Option<(kh_core::Vec3, f32)>,
    /// (party P) the formation, the leader routing and AI Sora (lead.rs; was kh-app party_play.rs `PartyPlay`).
    pub lead: Leadership,
    /// (ER S0, contract ER.6) the v5 friends record of a friend not in the party (his last status, or a loaded one),
    /// for `to_save`; and loaded records waiting for their friend's spawn. The test level never loads one.
    pub(crate) kept_records: Vec<crate::save::FriendRecord>,
    pub(crate) pending_records: Vec<crate::save::FriendRecord>,
    /// (ER party S6) the real progression mode (progression.rs: KH3's init data, no test loadout, a friend slotted out
    /// comes back with what he had); off in the test level.
    pub progression: bool,
    /// (ER party S6) the leader's Zero EXP (progression.rs; the user's rule, both hosts).
    pub zero_exp: crate::progression::ZeroExp,
    /// (IT) THE ROSTER'S PERMANENT STAT BOOSTS (crate::boosts): one total for every character, unlocked or not, copied
    /// onto each friend's status; saved once in the party record.
    pub roster_boosts: kh_party::boosts::RosterBoosts,
    /// (IT) the loaded party record had no roster total (a save older than it): the host seeds it once from what Sora
    /// received (`PartyRuntime::migrate_roster`).
    pub roster_unrecorded: bool,
    /// (DON1) THE ROSTER'S LEARNED MAGIC of Donald (crate::donald): one for the roster, joined or not; copied onto his status;
    /// saved once in the party record (`donald_magic`).
    pub don_learned: kh_party::donald::DonLearned,
    /// (DON1) the loaded party record had no `donald_magic` (a save older than the overhaul): the host rebuilds the steps once.
    pub don_unrecorded: bool,
    /// (party X2) both friend leaders' Customize blocks (they outlive the party's respawns) and Sora's book (custom.rs).
    pub customs: crate::custom::Customs,
}

pub fn ai_settings_of(ai: FriendAiSettings) -> kh_ai::AiSettings {
    use kh_ai::{AbilityStyle, BattleStyle, CureStyle};
    kh_ai::AiSettings {
        battle: [BattleStyle::Together, BattleStyle::Free, BattleStyle::Support, BattleStyle::Focus][ai.battle.min(3) as usize],
        ability: [AbilityStyle::Free, AbilityStyle::Balance, AbilityStyle::Less][ai.ability.min(2) as usize],
        cure: [CureStyle::Often, CureStyle::Pinch, CureStyle::Always][ai.cure.min(2) as usize],
        cure_targets: ai.cure_targets & 7,
    }
}

pub fn chara_of(c: PartyChara) -> Option<Chara> {
    match c {
        PartyChara::Donald => Some(Chara::Donald),
        PartyChara::Goofy => Some(Chara::Goofy),
        PartyChara::Sora => None,
    }
}

/// The HUD / host character of a kh-party member.
pub fn party_chara(c: Chara) -> PartyChara {
    if c == Chara::Donald {
        PartyChara::Donald
    } else {
        PartyChara::Goofy
    }
}

impl PartyRuntime {
    pub fn new(config: RuntimeConfig) -> PartyRuntime {
        let RuntimeConfig { wanted, test, weapons, ai_pack, ai_pack_source } = config;
        PartyRuntime {
            party: Party::new(PartyConfig::test_level99()),
            members: Vec::new(),
            wanted,
            test,
            weapons,
            previous: Vec::new(),
            current: Vec::new(),
            level_ups: Vec::new(),
            guard_ups: Vec::new(), // (GOOFY-GG)
            level_abilities: Vec::new(), // (BN)
            victims: Vec::new(),
            trace: false,
            last_exp: -1,
            ai_dirty: true,
            team: kh_party::team::Team::new(),
            pending_heals: Vec::new(),
            pending_ai: Vec::new(),
            ai_pack,
            ai_pack_source,
            ai: Vec::new(),
            ai_shared: kh_ai::AiShared::new(),
            last_trace: Default::default(),
            cams: kh_camera::camera_anim::StateEventCameras::new(),
            actor_actions: Vec::new(),
            team_fov: None,
            zetta_white: 0.0,
            zetta_behind: false,
            zetta_handing: false,
            zetta_solve: ZettaSolve::default(),
            hop_guard: Default::default(),
            zetta_span_was: false,
            hop_loops: Vec::new(),
            zetta_cinematic: false, // (ZFIN round 7)
            team_shake: Default::default(),
            sora_look_point: None,
            enemy_centres: Vec::new(),
            sora_team_done: false,
            leader: None,
            active_friends: [true; 2],
            loco_time: [0.0; 3],
            don_hold: None, // (DON-MAGIC part A)
            lead: Leadership::default(),
            kept_records: Vec::new(),
            pending_records: Vec::new(),
            progression: false, // (ER party S6)
            zero_exp: Default::default(), // (ER party S6)
            roster_boosts: kh_party::boosts::RosterBoosts::NONE, // (IT)
            roster_unrecorded: false, // (IT)
            don_learned: kh_party::donald::DonLearned::start(), // (DON1)
            don_unrecorded: false, // (DON1)
            customs: Default::default(), // (party X2)
        }
    }

    pub fn id(&self, c: PartyChara) -> Option<MemberId> {
        self.members.iter().find(|m| m.0 == c).map(|m| m.1)
    }

    /// The character of a member id.
    pub fn chara_of_id(&self, id: MemberId) -> Option<PartyChara> {
        self.members.iter().find(|m| m.1 == id).map(|m| m.0)
    }

    /// A member's view after the last step.
    pub fn view(&self, c: PartyChara) -> Option<MemberView> {
        self.id(c).and_then(|id| self.party.view(id))
    }

    fn new_controller(&self, c: PartyChara, id: MemberId) -> Option<kh_ai::AiController> {
        let pack = self.ai_pack.as_ref()?;
        let pawn = if c == PartyChara::Donald { "n_ex001" } else { "n_ex002" };
        Some(kh_ai::AiController::new(pack.clone(), pawn, id.0, 0x5EED_A100 ^ id.0 as u64))
    }

    /// Spawns both friends at their home positions round Sora (sheet `party-test` spawn.offsets: FriendHomePos rows
    /// 0 / 1, 180 cm at +70 / -70 degrees), with the test loadout.
    pub fn spawn(&mut self, sora: &Output, total_exp: i32) {
        if !self.members.is_empty() {
            return;
        }
        self.party = Party::new(PartyConfig { total_exp, ..PartyConfig::test_level99() });
        // (ER party S6) the real progression: KH3's init data at the shared total's level
        if self.progression {
            self.party = Party::new(PartyConfig { level99: false, total_exp, ..PartyConfig::test_level99() });
        }
        for (i, c) in PartyChara::FRIENDS.into_iter().enumerate() {
            // (party P) only the friends the party formation has in the active party
            if !self.active_friends[i] {
                continue;
            }
            let (length, angle) = self.test.offsets[i];
            let a = (sora.yaw + angle).to_radians();
            let position = sora.position + kh_core::Vec3::new(a.cos(), a.sin(), 0.0) * length;
            let id = self.party.spawn_as(chara_of(c).unwrap(), MemberId(i as u8), position, sora.yaw); // (party P) fixed ids: Donald 0, Goofy 1
            self.members.push((c, id));
            self.apply_test_loadout(c);
            if self.ai_pack.is_none() {
                self.ai_pack = self.ai_pack_source.as_ref().and_then(|f| f());
            }
            if let Some(ctl) = self.new_controller(c, id) {
                self.ai.push((c, ctl));
            }
        }
        self.rebase_levels(total_exp);
        for (c, ai) in std::mem::take(&mut self.pending_ai) {
            self.set_ai_settings(c, ai);
        }
        self.current = self.views();
        self.previous = self.current.clone();
        self.last_exp = total_exp;
        self.ai_dirty = true;
        say!("party: spawned {}", self.members.iter().map(|(c, id)| self.party.view(*id).map_or(String::new(), |v| format!("{} (Lv {} HP {}/{} MP {}/{})", c.name(), v.level, v.hp, v.hp_max, v.mp, v.mp_max))).collect::<Vec<_>>().join(", "));
    }

    /// TEST-LEVEL FIXTURE (plan N3): the friends take the level their own tables give for Sora's total, also
    /// DOWNWARD (the test-level toggle lowers Sora's EXP; the game's EXP never goes down), gauges full. The level-99
    /// loadout's equipped abilities are kept (a lower level's AP may be exceeded: a test-level choice).
    fn rebase_levels(&mut self, total: i32) {
        for (c, id) in self.members.clone() {
            let Some(chara) = chara_of(c) else { continue };
            if let Some(s) = self.party.status_mut(id) {
                s.level = kh_party::tables::level_for_exp(chara, total);
                s.total_exp = total;
                s.hp = s.max_hp();
                s.mp = s.max_mp();
            }
        }
    }

    /// TEST-LEVEL CHOICE (sheet party-test sora.team_abilities): Sora's team abilities (the Unison / Fusion
    /// finishers: owned by the level-99 loadout but left unequipped while the level had no party) are equipped; the
    /// listed copies are taken off first to make the AP (180 of 180 are in use).
    fn equip_sora_team_abilities(&mut self, sora: &mut Sora) {
        let names = |list: &[String]| list.iter().filter_map(|n| kh_core::enums::ETresAbilityKind::from_game_name(n)).collect::<Vec<_>>();
        let (equip, room) = (names(&self.test.sora_team.0), names(&self.test.sora_team.1));
        if equip.is_empty() || sora.status().placeholder {
            return;
        }
        let status = sora.status_mut();
        // Only when the AP is there after making room (a lower test level has less: nothing is changed then).
        let cost = |k: &kh_core::enums::ETresAbilityKind| kh_core::status::tables::ability_ap(*k);
        let freed: i32 = room.iter().filter(|k| status.abilities.equipped(**k) > 0).map(cost).sum();
        let needed: i32 = equip.iter().filter(|k| status.abilities.equipped(**k) < status.abilities.owned(**k)).map(cost).sum();
        if status.max_ap() - status.ap_in_use() + freed < needed {
            say!("party: Sora's team abilities NOT equipped: {needed} AP needed, {} free after making room (level {})", status.max_ap() - status.ap_in_use() + freed, status.level);
            return;
        }
        for kind in room {
            status.unequip(kind);
        }
        let mut done = Vec::new();
        for kind in equip {
            match status.equip(kind) {
                Ok(()) => done.push(kind.name()),
                Err(e) => say!("party: Sora's {}: not equipped ({e})", kind.name()),
            }
        }
        say!("party: Sora's team abilities equipped (sheet party-test): {} (AP {} / {})", done.join(", "), status.ap_in_use(), status.max_ap());
    }

    /// (party P) THE PARTY FORMATION adds one friend at the leader's side (the home position of its slot, as spawn).
    pub fn spawn_one(&mut self, c: PartyChara, at: kh_core::Vec3, yaw: f32) {
        if self.id(c).is_some() {
            return;
        }
        let Some(chara) = chara_of(c) else { return };
        let i = if c == PartyChara::Donald { 0 } else { 1 };
        let (length, angle) = self.test.offsets[i];
        let a = (yaw + angle).to_radians();
        let position = at + kh_core::Vec3::new(a.cos(), a.sin(), 0.0) * length;
        let id = self.party.spawn_as(chara, MemberId(i as u8), position, yaw);
        self.members.push((c, id));
        self.members.sort_by_key(|m| m.0 as u8); // the HUD / camp order: Donald, Goofy
        self.apply_test_loadout(c);
        self.sync_roster(); // (IT) he joins with every permanent boost the roster received (before his gauges are filled)
        self.sync_donald(); // (DON1) Donald joins with every magic step the roster earned
        // (ER party S6) the real progression: he joins at the level the shared total gives him (the user's rule)
        if self.progression {
            let _ = chara;
            let total = self.last_exp.max(0);
            if let Some((from, to)) = self.join_level(c, total) {
                say!("party: {} joins at level {to} (the shared EXP {total}; was {from})", c.name());
            }
        } else if let Some(s) = self.party.status_mut(id) {
            s.level = kh_party::tables::level_for_exp(chara, self.last_exp.max(0));
            s.total_exp = self.last_exp.max(0);
            s.hp = s.max_hp();
            s.mp = s.max_mp();
        }
        if let Some(ctl) = self.new_controller(c, id) {
            self.ai.push((c, ctl));
        }
        // (party D5) the settings kept for him while he was out (or loaded from a save meanwhile)
        if let Some(k) = self.pending_ai.iter().position(|p| p.0 == c) {
            let (_, ai) = self.pending_ai.remove(k);
            self.set_ai_settings(c, ai);
        }
        self.current = self.views();
        self.previous = self.current.clone();
        self.ai_dirty = true;
    }

    /// (party P) THE PARTY FORMATION takes one friend out (no model, no AI, no HUD gauge).
    pub fn remove_one(&mut self, c: PartyChara) {
        let Some(id) = self.id(c) else { return };
        if self.leader == Some(id) {
            self.party.set_pad(id, None);
            self.leader = None;
        }
        // (party D5) his AI settings outlive his place in the party (the save's party record keeps every friend's;
        // they come back with him: spawn_one)
        let kept = self.ai_settings(c);
        self.pending_ai.retain(|p| p.0 != c);
        self.pending_ai.push((c, kept));
        // (ER S0, contract ER.6) his progression for the save while he is out
        if let Some(s) = self.party.status(id) {
            let r = crate::save::FriendRecord::of(c, s);
            self.kept_records.retain(|k| k.chara != c);
            // (ER party S6, S0's note) the real progression: his record comes back at his next spawn_one
            if self.progression {
                self.pending_records.retain(|k| k.chara != c);
                self.pending_records.push(r.clone());
            }
            self.kept_records.push(r);
        }
        self.party.remove(id);
        self.members.retain(|m| m.0 != c);
        self.ai.retain(|a| a.0 != c);
        self.current = self.views();
        self.previous = self.current.clone();
    }

    /// (party P) a member's AI controller.
    pub fn ai_of(&mut self, id: MemberId) -> Option<&mut kh_ai::AiController> {
        let c = self.members.iter().find(|m| m.1 == id).map(|m| m.0)?;
        self.ai.iter_mut().find(|a| a.0 == c).map(|a| &mut a.1)
    }

    pub fn despawn(&mut self) {
        self.ai.clear();
        self.ai_shared = kh_ai::AiShared::new();
        for (_, id) in self.members.drain(..) {
            self.party.remove(id);
        }
        self.current.clear();
        self.previous.clear();
        say!("party: removed");
    }

    /// Sheet `party-test` <chara>.loadout over kh-party's level-99 status: weapon, items, AI bytes.
    fn apply_test_loadout(&mut self, c: PartyChara) {
        let loadout = if c == PartyChara::Donald { self.test.donald.clone() } else { self.test.goofy.clone() };
        let Some(id) = self.id(c) else { return };
        // (ER party S6) the real progression: KH3's init data stands (the game's Restore Defaults for the AI bytes, F.2),
        // only a record of his own (loaded, or kept while he was out) replaces it
        if self.progression {
            if let Some(s) = self.party.status_mut(id) {
                (s.ai.battle, s.ai.ability, s.ai.cure, s.ai.cure_targets) = (0, 0, 1, 7);
            }
            if let Some(k) = self.pending_records.iter().position(|r| r.chara == c) {
                let r = self.pending_records.remove(k);
                if let Some(s) = self.party.status_mut(id) {
                    r.apply(s);
                }
            }
            return;
        }
        let Some(s) = self.party.status_mut(id) else { return };
        if let Some(w) = kh_party::tables::weapon(&loadout.weapon) {
            s.weapon = w.1;
        }
        for (slot, name) in loadout.items.iter().enumerate().take(s.items.len()) {
            if let Some(item) = kh_core::status::ItemKind::from_name(name) {
                s.items[slot] = kh_party::ItemSlot { item: Some(item), present: true };
            }
        }
        let ai = FriendAiSettings::from_record(loadout.ai);
        s.ai.battle = ai.battle;
        s.ai.ability = ai.ability;
        s.ai.cure = ai.cure;
        self.set_targets(c, ai.cure_targets);
        // (ER S0, contract ER.6) a loaded v5 record waiting for him replaces the loadout's equipment (never in the test level)
        if let Some(k) = self.pending_records.iter().position(|r| r.chara == c) {
            let r = self.pending_records.remove(k);
            if let Some(s) = self.party.status_mut(id) {
                r.apply(s);
            }
        }
    }

    /// The Recovery targets (kept in kh-party's AiSettings, given to the AI).
    fn set_targets(&mut self, c: PartyChara, targets: u8) {
        if let Some(s) = self.id(c).and_then(|id| self.party.status_mut(id)) {
            s.ai.cure_targets = targets & 7;
        }
    }

    fn targets(&self, c: PartyChara) -> u8 {
        self.id(c).and_then(|id| self.party.status(id)).map_or(7, |s| s.ai.cure_targets & 7)
    }

    pub fn ai_settings(&self, c: PartyChara) -> FriendAiSettings {
        // (party D5) a friend out of the party: the settings kept for him (pending_ai), not the defaults, so the save's
        // party record keeps them
        if self.id(c).is_none() {
            if let Some(p) = self.pending_ai.iter().find(|p| p.0 == c) {
                return p.1;
            }
        }
        let s = self.id(c).and_then(|id| self.party.status(id));
        FriendAiSettings { battle: s.map_or(0, |s| s.ai.battle), ability: s.map_or(0, |s| s.ai.ability), cure: s.map_or(1, |s| s.ai.cure), cure_targets: self.targets(c) }
    }

    pub fn set_ai_settings(&mut self, c: PartyChara, ai: FriendAiSettings) {
        if self.id(c).is_none() {
            // not spawned yet (a word of the first step): applied at spawn
            self.pending_ai.retain(|p| p.0 != c);
            self.pending_ai.push((c, ai));
            return;
        }
        if let Some(s) = self.id(c).and_then(|id| self.party.status_mut(id)) {
            s.ai.battle = ai.battle;
            s.ai.ability = ai.ability;
            s.ai.cure = ai.cure;
        }
        self.set_targets(c, ai.cure_targets);
        self.ai_dirty = true;
        say!("party: {} AI settings: {}", c.name(), ai.describe());
    }

    fn views(&self) -> Vec<MemberView> {
        self.members.iter().filter_map(|(_, id)| self.party.view(*id)).collect()
    }

    /// (FX5) Where the threat rule measures from (aiworld.rs `threatens`): the leader's feet (`leader`: Sora's or the
    /// leading friend's), every member who is up, and AI Sora's while a friend leads with him in the party.
    pub fn threat_points(&self, leader: kh_core::Vec3, sora: &Output) -> Vec<kh_core::Vec3> {
        let mut at = vec![leader];
        at.extend(self.current.iter().filter(|v| !v.ko).map(|v| v.position));
        if self.leader.is_some() && self.lead.sora_active {
            at.push(sora.position);
        }
        at
    }

    /// (party I2) THE AI'S CAMERA = THE DRAWN VIEW (C3 -> F request; contract C.3): `camera` is the view the last
    /// step left for drawing (field / lock-on / aim / shotlock / finish / team cameras), i.e. the PlayerCameraManager
    /// POV cache the game's frustum test FUN_140e5ac70 reads (it is filled after the pawns tick, so the AI sees the
    /// previous frame's view as in KH3). Its lens is the one the host draws with: the field camera's horizontal FOV
    /// (kh_camera::projection, CAMERA_PARAMS) or the team CameraAnim's FOV + its shake, and the fixed 16:9 viewport.
    /// The far plane stays `camera_far` None = 2097152 cm (C4: UCameraComponent ctor 0x143FA3B70).
    pub fn ai_camera(&self, camera: &kh_camera::CameraView) -> kh_ai::CameraView {
        let (fov_field, aspect, _) = kh_camera::projection(&kh_camera::CAMERA_PARAMS);
        let fov = self.team_fov.map_or(fov_field, |f| f + self.team_shake.fov);
        kh_ai::CameraView { position: camera.position, pitch: camera.rotation.pitch, yaw: camera.rotation.yaw, fov, aspect }
    }

    /// (party I2) Every member's MP for `AiWorld::member_mp` (C3 -> F request): (member id, (MP, max MP, MP
    /// charge running, seconds of charge left)) = what FUN_14126af20 / FUN_14126b550 read of another member
    /// (pawn MP, max, +0x7C0 bit 1, +0x7C4).
    pub fn member_mps(&self) -> Vec<(u8, (i32, i32, bool, f32))> {
        self.members.iter().filter_map(|(_, id)| self.party.status(*id).map(|s| (id.0, (s.mp, s.max_mp(), s.mp_charge_left > 0.0, s.mp_charge_left.max(0.0))))).collect()
    }

    /// (party AIX, OUR DESIGN: KH3 has no friend leader) THE ENEMIES' OWN BATTLE STATE while a friend leads. KH3's battle
    /// flag counts the enemy pawns whose own battle flag is set (0x140EA0B20): an enemy is in battle once its AI has the
    /// party in sight, whether or not anyone was hit. The test level's engaged rule is a stand-in for the HUD / menu
    /// (a passive dummy only for 3 s after a blow: kh-app fixtures.rs), which left a friend-led party in the field among
    /// dummies (AI Sora stood beside them); the Elden Ring host counts real hostiles round the leader. Stand-in for that
    /// sight here: a living hostile enemy within `PARTY_AGGRO_RANGE` (800 cm, sheet-equal to ER target.engage_range) of
    /// the leader, an AI friend or AI Sora counts as engaged.
    fn party_aggro(&self, targets: &dyn PartyTargets, sora: &Output) -> u32 {
        let mut actors = Vec::new();
        targets.ai_actors(&mut actors);
        let mut at: Vec<kh_core::Vec3> = self.current.iter().filter(|v| !v.ko).map(|v| v.position).collect();
        if self.lead.sora_active {
            at.push(sora.position);
        }
        actors.iter().filter(|a| a.alive && a.hostile && at.iter().any(|p| (a.position - *p).truncate().length() - a.radius <= crate::lead::PARTY_AGGRO_RANGE)).count() as u32
    }

    /// (ZETTA round 5c) THE ALLIES WAIT OUT ZETTAFLARE (the user: "If there is only one target (and they're being targeted by
    /// or are in the zettaflare) then the allies move away and wait for when the player regains control of Donald"): with no
    /// enemy left to them (every one Zettaflare's or near its zones) through the span, the waiting spots
    /// (`Party::don_zetta_wait_spots`: beside Donald past his zone, off the beam, out of the shot behind him) on the host's
    /// ground that the leader can reach; empty = not waiting. The friends' trees rest and kh-party walks each to its place
    /// (zettaflare::step_out); AI Sora steers to his (lead.rs); at the hand-back all is as before.
    pub(crate) fn zetta_wait_spots(&self, lead_at: kh_core::Vec3, enemies: &EnemyFeed, nav: &dyn kh_ai::Nav) -> Vec<kh_core::Vec3> {
        if !self.party.don_zetta_span() || enemies.actors.iter().any(|a| a.hostile) {
            return Vec::new();
        }
        self.party.don_zetta_wait_spots().into_iter().filter_map(|p| {
            let g = nav.project(p, 300.0, 600.0)?;
            nav.path_exists(lead_at, g).then_some(g)
        }).collect()
    }

    /// THE PARTY'S STEP (contract B.3), right after Sora's in the same fixed step.
    pub fn step(&mut self, input: StepIn) -> StepOut {
        let StepIn { dt, sora, out, lock_on, camera, world, targets, audio, effects, nav, now } = input;
        let nav: &dyn kh_ai::Nav = nav.unwrap_or(&STRAIGHT_NAV);
        if !self.sora_team_done && self.wanted {
            self.sora_team_done = true;
            self.equip_sora_team_abilities(sora);
        }
        if self.wanted && self.members.is_empty() && self.active_friends.iter().any(|a| *a) { // (party P) the formation
            self.spawn(out, sora.status().exp);
        } else if !self.wanted && !self.members.is_empty() {
            self.despawn();
        }
        if self.members.is_empty() {
            return StepOut::default();
        }
        self.sync_roster(); // (IT) the roster's boosts on every friend (a load or a reset may have rebuilt a status)
        self.sync_donald(); // (DON1) the roster's learned magic onto Donald (the same reasons)
        let status = sora.status();
        // THE SHARED EXP (stats spec 5.2): each friend checks its own table against Sora's total.
        if status.exp < self.last_exp {
            self.last_exp = status.exp;
            self.rebase_levels(status.exp);
            say!("party: t={now:.3}s Sora's EXP went down (test level): the friends follow (Lv {})", self.members.iter().filter_map(|(_, id)| self.party.status(*id).map(|s| s.level.to_string())).collect::<Vec<_>>().join(" / "));
        }
        if status.exp != self.last_exp {
            self.last_exp = status.exp;
            let before: Vec<(MemberId, [i32; 4])> = self.members.iter().filter_map(|(_, id)| self.party.status(*id).map(|s| (*id, [s.strength(), s.magic(), s.defense(), s.max_ap()]))).collect();
            let guard_before: Vec<(MemberId, f32)> = self.members.iter().filter_map(|(_, id)| self.party.status(*id).and_then(|s| s.guard.map(|g| (*id, g.max)))).collect(); // (GOOFY-GG)
            for (id, from, to) in self.party.set_total_exp(status.exp) {
                let Some(c) = self.members.iter().find(|m| m.1 == id).map(|m| m.0) else { continue };
                let old = before.iter().find(|b| b.0 == id).map_or([0; 4], |b| b.1);
                let new = self.party.status(id).map_or(old, |s| [s.strength(), s.magic(), s.defense(), s.max_ap()]);
                let delta = [new[0] - old[0], new[1] - old[1], new[2] - old[2], new[3] - old[3]];
                say!("party: t={now:.3}s {} level {from} -> {to} (Strength {:+} Magic {:+} Defense {:+} AP {:+})", c.name(), delta[0], delta[1], delta[2], delta[3]);
                self.level_ups.push((c, to as i32, delta));
                // (BN) the kit abilities this level taught (his kit's own level table): on the same LEVEL UP! window
                let mut learned = crate::custom::level_kit_abilities(c, from, to);
                learned.extend(crate::donald::level_abilities(c, from, to)); // (DON-PROG) Donald's level tree (his overhaul abilities)
                if !learned.is_empty() {
                    say!("party: t={now:.3}s {} learned by level: {}", c.name(), learned.join(", "));
                    self.level_abilities.push((c, to as i32, learned));
                    if self.level_abilities.len() > 16 {
                        self.level_abilities.remove(0);
                    }
                }
                // (GOOFY-GG) GUARD's max grows with his level (sheet party-guard-levels)
                if let (Some(old), Some(g)) = (guard_before.iter().find(|b| b.0 == id).map(|b| b.1), self.party.status(id).and_then(|s| s.guard)) {
                    if g.max > old {
                        say!("party: t={now:.3}s {} GUARD max {old:.0} -> {:.0}", c.name(), g.max);
                        self.guard_ups.push((c, to as i32));
                        if self.guard_ups.len() > 16 {
                            self.guard_ups.remove(0);
                        }
                    }
                }
            }
        }
        let view = SoraView {
            position: out.position,
            yaw: out.yaw,
            velocity: out.velocity,
            on_ground: out.on_floor,
            hp: status.hp,
            hp_max: status.max_hp(),
            mp: status.mp,
            mp_max: status.max_mp(),
            dead: status.hp < 1,
            // (party AIX) A FRIEND LEADING: the party's battle flag is our own rule (KH3 has no friend leader): the host's
            // engaged count, which the host measures round the leader (test level: the dummies' engage point is the
            // leader; Elden Ring: kh-er party/aiworld.rs ErPartyTargets), with KH3's 1.0 s linger and forced flag (the
            // same rule as Sora's own flag: kh-core actions.rs pawn_tick, 0x140EA0B20). Sora's own flag is frozen while he
            // is out of the party (not stepped: the friends never entered battle) and is measured round AI Sora otherwise.
            in_battle: if self.leader.is_some() { let n = kh_core::Targets::engaged_count(&*targets) + self.party_aggro(&*targets, out); self.lead.battle_step(n, sora.actions().battle_forced, dt) } else { sora.actions().battle },
            lock_on,
            total_exp: status.exp,
            game_level: status.game_level,
            // (stream B3, minimal edit) the drawn camera for the Warp state's out-of-view test (as the AI's below)
            camera: Some({ let c = self.ai_camera(camera); kh_party::PartyCamera { position: c.position, pitch: c.pitch, yaw: c.yaw, fov: c.fov, aspect: c.aspect } }), // (party I2) the drawn lens
            // (stream B4, minimal edit) Sora's ETresStateID for the RailSlide state's off-rail warp
            state: out.state as i32,
            // (party K1) his MaxWalkSpeed (MySprintSpeed while sprinting, kh-core run.rs) and AnalogInputModifier (the
            // walk rate in walk mode, kh-core cmc.rs): the friends' FollowPlayer speed rule
            max_speed: if out.movement_state == kh_core::MovementState::Sprint { sora.params().speed_sprint } else { sora.params().speed_run },
            analog: if out.movement_state == kh_core::MovementState::Walk { sora.params().speed_walk_rate } else { 1.0 },
            // (party AIX) AI Sora as the friends' ally (a friend leading, Sora in the party): their Cures / items reach him
            ai_sora: (self.leader.is_some() && self.lead.sora_active).then_some((MemberId(crate::aiworld::SORA_ACTOR), out.position)),
        };
        // (party P) a friend leading: the party's "player" is the leader (lead.rs leader_view)
        let view = self.leader_view(view);
        self.party.set_in_battle(view.in_battle);
        for (id, amount, effect) in std::mem::take(&mut self.pending_heals) {
            let healed = self.party.heal(id, amount, effect);
            say!("party: t={now:.3}s Sora heals member {}: +{healed} HP", id.0);
        }
        // (stream C) THE AI: each member's controller ticks before the party's step (contract C.1)
        if self.ai_dirty {
            self.ai_dirty = false;
            for (c, ctl) in self.ai.iter_mut() {
                let s = self.party.status(self.members.iter().find(|m| m.0 == *c).map_or(MemberId(0), |m| m.1)).map(|s| (s.ai.battle, s.ai.ability, s.ai.cure));
                if let Some((battle, ability, cure)) = s {
                    let cure_targets = self.members.iter().find(|m| m.0 == *c).and_then(|m| self.party.status(m.1)).map_or(7, |s| s.ai.cure_targets);
                    ctl.set_settings(ai_settings_of(FriendAiSettings { battle, ability, cure, cure_targets }));
                }
            }
        }
        // (FX5) the trees' enemies: only those that threaten the party (aiworld.rs `threatens`), whoever leads
        let mut enemies = EnemyFeed::of_party(&*targets, &self.threat_points(view.position, out), view.lock_on);
        enemies.reserve(&self.party.don_zetta_off_limit_targets(&*targets)); // (ZETTA) the allies leave Zettaflare's target alone; (round 5c) and every enemy near its zones until the player has Donald back
        if crate::aiworld::left_out_changed(enemies.left_out, now) {
            say!("party: t={now:.3}s (FX5) the friends' trees: {} hostile(s) of the host left out (none within {:.0} cm of the party, none fighting it, not the leader's lock: they do not threaten the party), {} in", enemies.left_out, crate::aiworld::threat_range(), enemies.actors.iter().filter(|a| a.hostile).count());
        }
        // (party P) a friend leading: the trees see him as the player; his own tree is not ticked (lead.rs)
        if let Some(leader) = self.leader {
            let views = self.views();
            if let Some(lv) = views.iter().find(|v| v.id == leader).copied() {
                // (ZETTA round 5c) no enemy left to them: the friends wait at spots off Zettaflare's zones
                self.party.don_zetta.wait_spots = self.zetta_wait_spots(lv.position, &enemies, nav);
                let mps = self.member_mps();
                let world_view = LeaderAiWorld { leader: &lv, lock_on: view.lock_on, in_battle: view.in_battle, members: &views, sora: self.lead.sora_in.then_some((out, sora.status())), enemies: &enemies, camera: self.ai_camera(camera), nav, game_level: status.game_level as u8, mps: &mps }; // (party I2) drawn view, member MP
                for (c, ctl) in self.ai.iter_mut() {
                    let Some(member) = self.members.iter().find(|m| m.0 == *c).map(|m| m.1) else { continue };
                    if member == leader {
                        continue;
                    }
                    let fr = self.party.view(member).is_some_and(|v| matches!(v.state, kh_party::NpcStateId::Fr(_)));
                    // (ZETTA round 5c) waiting out Zettaflare: the tree rests, the party walks him to his place (zettaflare::step_out)
                    ctl.set_stop_ai(fr || !self.party.don_zetta.wait_spots.is_empty());
                    ctl.tick(dt, &mut kh_party::ai_body::PartyBody { party: &mut self.party, member }, &world_view, &mut self.ai_shared);
                }
            }
        } else if !self.ai.is_empty() {
            let views = self.views();
            let mps = self.member_mps();
            let world_view = SoraAiWorld {
                sora: out,
                status: sora.status(),
                in_battle: view.in_battle,
                attacking: matches!(out.state, kh_core::StateId::Attack),
                lock_on,
                enemies: &enemies,
                members: &views,
                camera: self.ai_camera(camera), // (party I2) the drawn view
                nav,
                game_level: status.game_level as u8,
                mps: &mps, // (party I2)
            };
            for (i, (c, ctl)) in self.ai.iter_mut().enumerate() {
                let Some(member) = self.members.iter().find(|m| m.0 == *c).map(|m| m.1) else { continue };
                // D: the tree pauses while a team attack owns the pawn
                let fr = self.party.view(member).is_some_and(|v| matches!(v.state, kh_party::NpcStateId::Fr(_)));
                ctl.set_stop_ai(fr);
                ctl.tick(dt, &mut kh_party::ai_body::PartyBody { party: &mut self.party, member }, &world_view, &mut self.ai_shared);
                if self.trace {
                    let t = ctl.trace();
                    // the trees on the active path (n_ex00x_BT and every RunBehavior / dynamic subtree by name).
                    // (party I2) Each `AiTrace::path` entry is one running tree instance, "TREE: node > node"
                    // (kh-ai bt.rs active_path): its name is the text before ": ".
                    let mut subtrees: Vec<&str> = Vec::new();
                    for entry in &t.path {
                        let name = entry.split_once(": ").map_or(entry.as_str(), |(tree, _)| tree).trim();
                        if !name.is_empty() && !subtrees.contains(&name) {
                            subtrees.push(name);
                        }
                    }
                    let line = format!("{} [{:?}] {} (target {:?})", c.name(), t.style, subtrees.join(" > "), t.target);
                    if line != self.last_trace[i.min(1)] {
                        say!("party-trace: t={now:.3}s {line}");
                        self.last_trace[i.min(1)] = line;
                    }
                }
            }
        }
        // (party B5) Sora's Blizzard ice rails: the friends' rail probe rides them (kh-party Party::set_ice_rails)
        // (party X3D) + a leading Donald's own Blizzard rails (his X2 caster), their ids moved to their own range so the
        // two sets never link into each other
        let mut rails = sora.magic().rails.clone();
        let off = |id: u32| if id == 0 { 0 } else { id | 0x4000_0000 };
        rails.extend(self.party.caster_rails().into_iter().map(|n| kh_core::magic::RailNode { id: off(n.id), previous: off(n.previous), ..n }));
        self.party.set_ice_rails(&rails);
        let mut aura_targets = crate::donald::AuraTargets::new(&mut *targets, &self.party); // (DON-PROG) the Fire aura on the party's hits
        let mut host = PartyHost { world, targets: &mut aura_targets, audio, effects };
        let output = self.party.step(dt, &view, &mut host);
        self.zetta_allies_back(now); // (ZETTA 2026-10-23) the clean slate at Zettaflare's hand-back; the move-loop guard
        // (DON-MAGIC part A) TEST / DEBUG: Donald held on his spot (`party don-warp`)
        if let (Some((at, yaw)), Some(id)) = (self.don_hold, self.first_donald()) {
            if let Some(p) = self.party.member_mut(id) {
                p.cmc.location = kh_core::Vec3::new(at.x, at.y, p.cmc.location.z);
                p.cmc.stop_movement();
                if !kh_party::donald::cast::casting(p) {
                    p.yaw = yaw;
                }
            }
        }
        self.don_after_step(sora, &out.events, dt); // (DON-PROG) the auras on Sora, his Thunder hits and guards (AI Donald, Taunt Pass)
        // (stream D) the team attacks' side after the party (contract D.3)
        let team = self.team.step(dt, &mut self.party, &mut host);
        for e in &team.events {
            match e {
                kh_party::team::TeamEvent::ActorSpecificAction { action } => self.actor_actions.push(*action),
                other => say!("party: t={now:.3}s team: {other:?}"),
            }
        }
        for heal in &output.sora_heals {
            // (party P) a friend leading: the heals on "the player" are the leader's
            if let Some(leader) = self.leader {
                self.party.heal(leader, heal.amount, heal.effect);
                // (party AIX) an Ether / Elixir on the leader: its MP too (the members' MP rule, kh-party Request::Mp: none
                // while the MP charge runs); before, only the HP part reached a friend leader
                if heal.mp > 0 {
                    if let Some(s) = self.party.status_mut(leader) {
                        if s.mp_charge_left <= 0.0 {
                            s.mp = (s.mp + heal.mp).min(s.max_mp());
                        }
                    }
                }
                // (party D5) the heal's effect on the pawn healed (kh_party::actions::recover_effect)
                self.party.heal_effect_on(leader, heal.effect, host.effects);
                continue;
            }
            sora.status_request(kh_core::StatusRequest::Recover { hp: heal.amount, mp: heal.mp, focus: 0, kind: heal.effect });
            // (party D5) Recover 0x1405B57D0 on Sora plays the kind's effect on him (Donald's Cure: vm_cur_bds1 + se00301_040)
            self.party.heal_effect_on_sora(heal.from, heal.effect, host.effects);
        }
        // (party AIX) the friends' Cures / items / Ethers on AI Sora (a friend leading): Recover on Sora, as on the player
        for heal in &output.ai_sora_heals {
            sora.status_request(kh_core::StatusRequest::Recover { hp: heal.amount, mp: heal.mp, focus: 0, kind: heal.effect });
            self.party.heal_effect_on_sora(heal.from, heal.effect, host.effects);
            say!("party: t={now:.3}s member {} heals AI Sora: +{} HP +{} MP", heal.from.0, heal.amount, heal.mp);
        }
        // (party X3D) the kit's log (what started / released / was dodged or blocked) for the trace
        for (id, _, e) in kh_party::pad::donald::take_log(&mut self.party) {
            let who = self.members.iter().find(|m| m.1 == id).map_or("?", |m| m.0.name());
            say!("party: t={now:.3}s {who}: KIT {e:?}");
        }
        for e in &output.events {
            let who = self.members.iter().find(|m| m.1 == e.member).map_or("?", |m| m.0.name());
            match &e.kind {
                PartyEventKind::StateChanged { .. } | PartyEventKind::Notify { .. } => {}
                other => say!("party: t={now:.3}s {who}: {other:?}"),
            }
        }
        let views = self.views();
        self.previous = std::mem::replace(&mut self.current, views);
        // (party I2) the enemies' look-at points for the head look-at (pose)
        let mut actors = Vec::new();
        targets.ai_actors(&mut actors);
        self.enemy_centres = actors.iter().filter_map(|a| match a.actor { kh_ai::ActorRef::Enemy(id) => Some((TargetId(id), a.center)), _ => None }).collect();
        // (stream E) the spell actors for the effect / sound side: their body effect follows them
        // (+ stream D) the team link pawns' meteors / missiles.
        let team = self.team.projectile_taps().map(|(effect, position, forward)| ProjectileTap { effect, position, forward });
        // (party X2) + the friend leader's Sora spells (kh-party leader_magic.rs casters)
        let casters: Vec<ProjectileTap> = self.party.caster_projectiles().map(|(effect, position, forward)| ProjectileTap { effect, position, forward }).collect();
        // (party X3D) Donald's kit: its bolts, meteors and missiles
        let kit: Vec<ProjectileTap> = kh_party::pad::donald::party_taps(&self.party).into_iter().map(|(effect, position, forward)| ProjectileTap { effect, position, forward }).collect();
        let projectiles = self.party.projectiles.iter().filter(|p| p.alive).map(|p| ProjectileTap { effect: p.effect, position: p.position, forward: p.velocity.normalize_or_zero() }).chain(team).chain(casters).chain(kit).collect();
        StepOut { projectiles, stepped: true, events: output.events.clone() } // (ER party S1) + events
    }

    /// The friends' slots of the status HUD (contract F.1).
    pub fn hud_slots(&self) -> Vec<PartyHudSlot> {
        self.members
            .iter()
            .filter_map(|(c, id)| self.party.view(*id).map(|v| PartyHudSlot { chara: *c, hp: v.hp, hp_max: v.hp_max, mp: v.mp, mp_max: v.mp_max, mp_charging: v.mp_charging, mp_charge_value: v.mp_charge, in_battle: v.in_battle, focus: if self.leader == Some(*id) { crate::custom::leader_focus(&self.party, *id) } else { None } /* (party X2-b) */ }))
            .collect()
    }

    /// The friends as the camp menu shows them (contract F.2).
    pub fn menu_members(&self) -> Vec<PartyMenuMember> {
        self.members
            .iter()
            .filter_map(|(c, id)| {
                let s = self.party.status(*id)?;
                let chara = chara_of(*c)?;
                let level = s.level;
                let exp_level = if level > 1 { kh_party::tables::level_row(chara, level).0 } else { 0 };
                let exp_next = if level < 99 { kh_party::tables::level_row(chara, level + 1).0 } else { 0 };
                let abilities = kh_core::enums::ETresAbilityKind::ALL.iter().filter(|k| s.abilities.owned(**k) > 0).map(|k| (*k, s.abilities.owned(*k), s.abilities.equipped_mask(*k))).collect();
                Some(PartyMenuMember {
                    chara: *c,
                    level: level as i32,
                    exp: s.total_exp,
                    exp_level,
                    exp_next,
                    hp: s.hp,
                    hp_max: s.max_hp(),
                    mp: s.mp,
                    mp_max: s.max_mp(),
                    ap_max: s.max_ap(),
                    ap_in_use: s.ap_in_use(),
                    strength: s.strength(),
                    magic: s.magic(),
                    defense: s.defense(),
                    // ETresDamageAttribute 1 fire, 2 blizzard, 3 thunder, 4 water, 5 aero, 6 dark (B's worn-equipment sums)
                    resists: { let r = s.resists(); [r[1], r[2], r[3], r[4], r[5], r[6]] },
                    weapon: s.weapon.to_string(),
                    armour: s.armour.iter().take(s.armour_slots as usize).map(|a| a.map(str::to_string)).collect(),
                    accessories: s.accessories.iter().take(s.accessory_slots as usize).map(|a| a.map(str::to_string)).collect(),
                    items: s.items.iter().take(s.item_slots as usize).map(|i| i.item.filter(|_| i.present)).collect(),
                    abilities,
                    ai: self.ai_settings(*c),
                    custom: self.customs.block(*c).copied(), // (party X2)
                    custom_costs: crate::custom::costs(*c), // (party X2)
                    kit: crate::custom::kit_rows(s), // (party X2-b)
                    guard: s.guard_view().map(|g| (g.value.round() as i32, g.max.round() as i32)), // (GOOFY-GG)
                    // (EQ) the slots' Auto-Reload and the used ones waiting for their refill (Sora's "used" rows)
                    item_reload: (0..s.item_slots as usize).map(|k| s.item_reload.get(k).copied().unwrap_or(false)).collect(),
                    item_spent: s.items.iter().zip(s.item_reload).take(s.item_slots as usize).map(|(i, r)| i.item.filter(|_| !i.present && r)).collect(),
                })
            })
            .collect()
    }

    /// One edit of the camp menu (contract F.2) on the friend's status. (party P) An AI edit of Sora's entry is AI
    /// Sora's settings ([`PartyRuntime::set_sora_ai_settings`]); the host routes it there.
    pub fn apply_edit(&mut self, edit: &PartyMenuEdit) {
        let c = edit.chara();
        let Some(id) = self.id(c) else { return };
        match edit {
            PartyMenuEdit::Weapon { id: key, .. } => {
                if let Some(s) = self.party.status_mut(id) {
                    match s.equip_weapon(key) {
                        Ok(()) => say!("party: {} weapon {key}: Strength {} Magic {}", c.name(), s.strength(), s.magic()),
                        Err(e) => say!("party: {} weapon {key}: refused ({e})", c.name()),
                    }
                }
            }
            PartyMenuEdit::Armour { slot, id: piece, .. } => {
                if let Some(s) = self.party.status_mut(id) {
                    let ok = s.set_armour(*slot, piece.as_deref());
                    say!("party: {} armour slot {slot} = {piece:?}{} (Defense {})", c.name(), if ok { "" } else { ": REFUSED" }, s.defense());
                }
            }
            PartyMenuEdit::Accessory { slot, id: piece, .. } => {
                if let Some(s) = self.party.status_mut(id) {
                    let ok = s.set_accessory(*slot, piece.as_deref());
                    say!("party: {} accessory slot {slot} = {piece:?}{}", c.name(), if ok { "" } else { ": REFUSED" });
                }
            }
            PartyMenuEdit::ItemSlot { slot, item, .. } => {
                // (EQ) Sora's item-slot rule (the auto-reload carried over / inherited, kh-core customize `equip_item`)
                if let Some(s) = self.party.status_mut(id) {
                    s.set_item_slot_kh3(*slot, *item);
                }
            }
            PartyMenuEdit::Ability { kind, copy, on, .. } => {
                // (EQ) THE COPY UNDER THE CURSOR, as Sora's `set_copy` (was: a count through `set`, which lit the FIRST
                // copies: equipping copy 2 showed copy 1 equipped after the host's answer, the row "jumped"), with the
                // AP budget the screen tested
                if let Some(s) = self.party.status_mut(id) {
                    if !s.set_ability(*kind, *copy, *on) {
                        say!("party: {} ability {} copy {} {}: REFUSED (not owned or over his AP)", c.name(), kind.name(), copy + 1, if *on { "on" } else { "off" });
                    }
                }
            }
            // (EQ) Equipment's Triangle on one of his item rows
            PartyMenuEdit::AutoReload { item, on, .. } => {
                if let Some(s) = self.party.status_mut(id) {
                    let ok = s.set_auto_reload(*item, *on);
                    say!("party: {} auto-reload of {} {}{}", c.name(), item.name(), if *on { "on" } else { "off" }, if ok { "" } else { ": no slot holds it" });
                }
            }
            PartyMenuEdit::UseItem { item, .. } => {
                // (party I2) ItemEffect 0x1405BE820 is ATresCharPawnBase's, the SAME function for Sora and the friends
                // (Potion 80, Hi-/Mega-Potion 160, Ether 100 MP or half the charge, Hi-/Mega-Ether all, the elixirs all;
                // x 0.5 when the target is in Die, state 0x3B; x (1 + 0.5 x the user's ITEM_UP count)): kh-party's
                // FriendStatus::item_effect is its transcription (the MP-charge rules included). The camp menu's caller
                // (which pawn counts as the user for ITEM_UP) was not read [D]: no boost.
                if let Some(s) = self.party.status_mut(id) {
                    let mult = if s.ko() { 0.5 } else { 1.0 };
                    let healed = s.item_effect(*item, mult);
                    say!("party: {} used {} from the menu: HP +{healed}", c.name(), item.name());
                }
            }
            PartyMenuEdit::Ai { settings, .. } => self.set_ai_settings(c, *settings),
            PartyMenuEdit::Custom { edit, .. } => drop(self.apply_custom_edit(c, *edit)), // (party X2)
            PartyMenuEdit::Kit { key, on, .. } => drop(self.apply_kit_edit(c, key, *on)), // (party X2-b)
        }
    }

    /// Whom enemy `n`'s fixture attack hits now (`dummy-target`).
    pub fn victim(&self, dummy: u32, from: kh_core::Vec3, sora: kh_core::Vec3) -> Victim {
        let v = self.victim_rule(dummy, from, sora);
        // (ZETTA round 5) the user: "enemies don't ever target Donald while he is performing Zettaflare": an enemy that would
        // choose him takes the nearest other member (Sora too while he is in the party); none: Sora's (no one when he is out:
        // the attack is not made)
        match v {
            Victim::Friend(c) if self.id(c).is_some_and(|id| self.party.don_zetta_invincible(id)) => {
                let mut best = (f32::INFINITY, Victim::Sora);
                if self.lead.sora_active {
                    best.0 = from.distance(sora);
                }
                for m in &self.current {
                    let d = from.distance(m.position);
                    if !m.ko && !self.party.don_zetta_invincible(m.id) && d < best.0 {
                        best = (d, Victim::Friend(party_chara(m.chara)));
                    }
                }
                best.1
            }
            other => other,
        }
    }

    /// The victim by the rules before (ZETTA round 5) Zettaflare's shield ([`PartyRuntime::victim`]).
    fn victim_rule(&self, dummy: u32, from: kh_core::Vec3, sora: kh_core::Vec3) -> Victim {
        let rule = self.victims.iter().find(|v| v.0 == dummy).map_or(Victim::Near, |v| v.1);
        // (DON-PROG) Donald's aggro (5.4): whom the enemy chooses (Taunt Pass, his weight, Distraction); never a hit moved
        if rule == Victim::Near {
            if let Some(v) = self.don_victim(dummy, from, sora) {
                return v;
            }
        }
        match rule {
            Victim::Near => {
                let mut best = (from.distance(sora), Victim::Sora);
                for v in &self.current {
                    let c = party_chara(v.chara);
                    let d = from.distance(v.position);
                    if !v.ko && d < best.0 {
                        best = (d, Victim::Friend(c));
                    }
                }
                best.1
            }
            Victim::Friend(c) if self.id(c).is_none() => Victim::Sora,
            other => other,
        }
    }

    /// (DON-MAGIC) Where a fixture attack's victim stands (Donald's Blind keeps the spot: `Party::don_blind_track`).
    pub fn victim_position(&self, victim: Victim, sora: kh_core::Vec3) -> Option<kh_core::Vec3> {
        match victim {
            Victim::Sora => Some(sora),
            Victim::Friend(c) => self.current.iter().find(|v| party_chara(v.chara) == c && !v.ko).map(|v| v.position),
            Victim::Near => None,
        }
    }

    /// (DON-MAGIC part A) The first Donald of the party (the debug words).
    fn first_donald(&self) -> Option<kh_party::MemberId> {
        let ids: Vec<kh_party::MemberId> = self.party.members().collect();
        ids.into_iter().find(|i| self.party.status(*i).is_some_and(|s| s.donald.is_some()))
    }

    pub fn set_victim(&mut self, dummy: u32, victim: Victim) {
        self.victims.retain(|v| v.0 != dummy);
        self.victims.push((dummy, victim));
    }

    /// A hit on a friend whose damage is final (contract B.3 `receive_hit`: the host's damage-in rule already ran):
    /// the friend's own reaction, KO and revive rules. `None` = not in the party.
    pub fn receive_hit(&mut self, c: PartyChara, hit: &kh_core::HitRecord) -> Option<kh_core::host::HitResult> {
        let id = self.id(c)?;
        Some(self.party.receive_hit(id, hit))
    }

    /// HP given to a friend (an item, a cure, a prize): what was healed. `None` = not in the party.
    pub fn heal(&mut self, c: PartyChara, hp: i32, effect: u8) -> Option<i32> {
        let id = self.id(c)?;
        Some(self.party.heal(id, hp, effect))
    }

    /// A test dummy's fixture attack on a friend: the fixture's plain amount (as on Sora: ceil(max(60 - Defense, 1) x
    /// power) with a dummy attack of 60), at most the friend's per-hit cap (`m_DamageMax` 24, stats spec 7).
    pub fn hit_friend(&mut self, c: PartyChara, dummy: TargetId, from: kh_core::Vec3, kind: kh_core::enums::ETresDamageKind, power: f32, now: f64) {
        let Some(id) = self.id(c) else { return };
        let Some(defender) = self.party.defender(id) else { return };
        let amount = (((60 - defender.defense).max(1) as f32 * power).ceil()).min(if defender.damage_max > 0 { defender.damage_max as f32 } else { f32::MAX });
        let at = self.party.view(id).map_or(from, |v| v.position);
        let direction = (at - from).normalize_or_zero();
        let record = kh_core::HitRecord { position: at, direction, damage: amount, damage_kind: kind, attack_name: "fixture.dummy", ..kh_core::HitRecord::empty(TargetId::NONE) };
        let result = self.party.receive_hit(id, &record);
        say!("fixture: t={now:.3}s dummy {} attacks {}: {} for {amount}: {} HP left{}", dummy.0, c.name(), kind.name(), result.hp_after, if result.killed { " (KO)" } else { "" });
    }

    /// View (reset) and `recover`: the friends' HP / MP full, KO ended.
    pub fn recover(&mut self) {
        for (_, id) in self.members.clone() {
            if let Some(s) = self.party.status_mut(id) {
                let (hp, mp) = (s.max_hp(), s.max_mp());
                s.hp = hp;
                s.mp = mp;
                s.mp_charge_left = 0.0;
            }
        }
    }

    pub fn status_text(&self) -> String {
        let mut lines = vec![format!("PARTY ({}): {}", if self.wanted { "on" } else { "off" }, if self.members.is_empty() { "none".to_string() } else { String::new() })];
        for v in &self.current {
            let c = party_chara(v.chara);
            lines.push(format!("  {} Lv {} HP {}/{} MP {}/{}{} {} at ({:.0}, {:.0}) {}", c.name(), v.level, v.hp, v.hp_max, v.mp, v.mp_max, if v.mp_charging { " CHARGE" } else { "" }, v.state_name, v.position.x, v.position.y, if v.ko { format!("KO {:.1}s", v.revive_left) } else { String::new() }));
            lines.push(format!("    AI: {}", self.ai_settings(c).describe()));
        }
        lines.join("\n")
    }

    /// (party I2) The enemy centres of the last step (the look-at points of an enemy focus).
    pub fn enemy_centre(&self, id: TargetId) -> Option<kh_core::Vec3> {
        self.enemy_centres.iter().find(|d| d.0 == id).map(|d| d.1)
    }
}

// ------------------------------------------------------------------------------------------- Sora's targets

/// The friends' ids in Sora's `Targets::allies` (a namespace of their own, far from the enemies' numbers).
pub const ALLY_BASE: u32 = 0x4000_0000;

/// SORA'S TARGETS WITH THE PARTY (contracts D.2, E): the host's targets for everything a target is, plus the
/// friends as his allies (Cure / items on friends, Assist Friends: `allies` / `heal_ally`) and the team-attack seam
/// (`friends` from D's `Team::friend_rows`, `friend_link` to `Team::request`). A heal on a friend is applied before
/// the party's next step (`PartyRuntime::step`).
pub struct SoraTargets<'a> {
    pub targets: &'a mut dyn kh_core::Targets,
    pub runtime: &'a mut PartyRuntime,
}

impl kh_core::Targets for SoraTargets<'_> {
    fn enumerate(&self, out: &mut Vec<kh_core::host::TargetInfo>) {
        let from = out.len();
        self.targets.enumerate(out);
        // (ZETTA) AI Sora (a friend leads) leaves Zettaflare's target alone until its hit (the user: "the allies you have
        // target everybody else except for the zettaflare's target"): his own targeting does not see it
        if self.runtime.leader.is_some() {
            let reserved = self.runtime.party.don_zetta_off_limit_targets(&*self.targets); // (round 5c) and every enemy near its zones
            if !reserved.is_empty() {
                let mut i = from;
                while i < out.len() {
                    if reserved.contains(&out[i].id) {
                        out.remove(i);
                    } else {
                        i += 1;
                    }
                }
            }
        }
    }
    fn script_hold(&mut self, id: kh_core::TargetId, hold: Option<kh_core::host::ScriptHold>) {
        self.targets.script_hold(id, hold) // (ZETTA)
    }
    fn held_animated(&self, id: kh_core::TargetId) -> bool {
        self.targets.held_animated(id) // (ZETTA)
    }
    fn script_finale(&mut self, id: kh_core::TargetId, finale: Option<kh_core::host::ScriptFinale>) {
        self.targets.script_finale(id, finale) // (ZETTA)
    }
    fn drawn_extent(&self, id: kh_core::TargetId) -> Option<(f32, f32)> {
        self.targets.drawn_extent(id) // (ZETTA 2026-10-23) what the host draws of it (the framing and the beam's cover)
    }
    fn info(&self, id: TargetId) -> Option<kh_core::host::TargetInfo> {
        self.targets.info(id)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<kh_core::host::HurtShape>) {
        self.targets.hurt_shapes(id, out)
    }
    fn apply_hit(&mut self, hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
        // (ZETTA) AI Sora never hits Zettaflare's target before its hit (a sweep that reaches it is not taken)
        if self.runtime.leader.is_some() && (self.runtime.party.don_zetta_reserved().contains(&hit.target) || self.targets.info(hit.target).is_some_and(|i| self.runtime.party.don_zetta_off_limits(i.position, i.radius))) /* (ZETTA round 5c) */ {
            return kh_core::host::HitResult::default();
        }
        let hit = crate::donald::aura_hit(&self.runtime.party.don_prog.aura, hit); // (DON-PROG) the Fire aura on Sora's hits
        // (DON-MAGIC) SORA'S HITS THROUGH DONALD'S REACTION RULE (documentation/DONALD_OVERHAUL.md 4.5): on an enemy with a
        // status, the reaction (his Thunder on a Wet enemy Conducts, Frozen + Thunder / Aero Shatters, a hit Exposes a
        // Revealed one) and its damage factor, Exposed's x1.25 (everyone's hits), and DON-KIT's x1.2 on Donald's team rows at
        // Resonance 3 (team::link::don_team_hit_mult); flagged so the hosts' own routes do not apply them again
        let mut hit = hit;
        if hit.takes_hp() && hit.flags & kh_party::donald::magic_fx::DON_HIT_FLAG == 0 {
            let size = self.targets.info(hit.target).map_or(kh_party::donald::status::Size::Small, |i| kh_party::donald::magic_fx::size_of(&*self.targets, &i));
            let mult = self.runtime.party.don_other_hit(hit.target.0, size, hit.attribute) * kh_party::team::link::don_team_hit_mult(&self.runtime.party, hit.attack_name);
            hit.damage *= mult;
            hit.flags |= kh_party::donald::magic_fx::DON_HIT_FLAG;
        }
        self.targets.apply_hit(&hit)
    }
    fn defense(&self, id: TargetId) -> Option<kh_core::host::TargetDefense> {
        self.targets.defense(id)
    }
    fn allies(&self, out: &mut Vec<kh_core::host::Ally>) {
        self.targets.allies(out);
        out.extend(self.runtime.current.iter().map(|v| kh_core::host::Ally { id: TargetId(ALLY_BASE + v.id.0 as u32), position: v.position, dead: v.ko }));
    }
    fn heal_ally(&mut self, id: TargetId, amount: i32, effect: u8) {
        if id.0 >= ALLY_BASE {
            self.runtime.pending_heals.push((MemberId((id.0 - ALLY_BASE) as u8), amount, effect));
        } else {
            self.targets.heal_ally(id, amount, effect)
        }
    }
    fn lock_on_range(&self, id: TargetId) -> f32 {
        self.targets.lock_on_range(id)
    }
    fn engaged_count(&self) -> u32 {
        self.targets.engaged_count()
    }
    fn body(&self, id: TargetId, part: u32) -> kh_core::host::TargetBody {
        self.targets.body(id, part)
    }
    fn take_armor(&mut self, id: TargetId, power: i32, weak: bool) -> i32 {
        self.targets.take_armor(id, power, weak)
    }
    fn take_reaction_power(&mut self, id: TargetId, power: i32) -> i32 {
        self.targets.take_reaction_power(id, power)
    }
    fn rush_suppression(&self, id: TargetId) -> bool {
        self.targets.rush_suppression(id)
    }
    fn terrain_dz(&self, id: TargetId) -> f32 {
        self.targets.terrain_dz(id)
    }
    fn push_body(&self, id: TargetId) -> Option<kh_core::host::PushBody> {
        self.targets.push_body(id)
    }
    fn reaction_state(&self, id: TargetId) -> u8 {
        self.targets.reaction_state(id)
    }
    fn attack_shapes(&self, out: &mut Vec<kh_core::host::AttackShape>) {
        self.targets.attack_shapes(out)
    }
    fn touched(&mut self, hit: &kh_core::HitRecord) {
        self.targets.touched(hit)
    }
    fn finish_kind(&self, id: TargetId) -> kh_core::host::FinishKind {
        self.targets.finish_kind(id)
    }
    // (XSI) the XS abilities' questions go to the wrapped targets (Hidden Potential, the spectacle cap, Rage Raid)
    fn foe_level(&self, id: TargetId) -> Option<u8> {
        self.targets.foe_level(id)
    }
    fn damage_to_hp(&self) -> f32 {
        self.targets.damage_to_hp()
    }
    fn in_revenge(&self, id: TargetId) -> bool {
        self.targets.in_revenge(id)
    }
    fn end_revenge(&mut self, id: TargetId) {
        self.targets.end_revenge(id)
    }
    fn friends(&self, out: &mut Vec<kh_core::host::Friend>) {
        if !self.runtime.members.is_empty() {
            self.runtime.team.friend_rows(&self.runtime.party, out);
        }
    }
    fn friend_link(&mut self, req: kh_core::host::FriendLinkRequest) {
        self.runtime.team.request(req);
    }
    // (DON-PROG) forwarded: DON-MAGIC's sizes and Insight's elements of the host's targets
    fn size_class(&self, id: TargetId) -> u8 {
        self.targets.size_class(id)
    }
    fn element_affinity(&self, id: TargetId) -> Option<[i32; 8]> {
        self.targets.element_affinity(id)
    }
}

// ------------------------------------------------------------------------------------------- the debug words

fn chara_word(w: &str) -> Option<PartyChara> {
    PartyChara::from_name(w).filter(|c| *c != PartyChara::Sora)
}

impl PartyRuntime {
    /// THE PARTY WORDS (contract F.3). Returns the line to print.
    pub fn word(&mut self, text: &str) -> String {
        let w: Vec<&str> = text.split_whitespace().collect();
        let number = |i: usize| w.get(i).and_then(|v| v.parse::<f32>().ok());
        match w.as_slice() {
            ["party", "on"] => {
                self.wanted = true;
                "party: on (spawned in the next step)".into()
            }
            ["party", "off"] => {
                self.wanted = false;
                "party: off".into()
            }
            ["party-recover"] => {
                self.recover();
                "party: HP / MP full".into()
            }
            ["party-hp", who, _] | ["party-ko", who] => {
                let Some(c) = chara_word(who) else { return format!("party: {text}: donald | goofy") };
                let Some(id) = self.id(c) else { return format!("party: {} is not in the party", c.name()) };
                let hp = if w[0] == "party-ko" { 0 } else { number(2).unwrap_or(0.0) as i32 };
                if hp <= 0 {
                    // A KO goes through the friend's own damage path (Die state, revive timer).
                    let now = self.party.view(id).map_or(0, |v| v.hp);
                    let record = kh_core::HitRecord { damage: now as f32 + 1.0, attack_name: "debug.party-ko", ..kh_core::HitRecord::empty(TargetId::NONE) };
                    let r = self.party.receive_hit(id, &record);
                    return format!("party: {} KO (killed {})", c.name(), r.killed);
                }
                if let Some(s) = self.party.status_mut(id) {
                    s.hp = hp.min(s.max_hp());
                }
                format!("party: {} HP {hp}", c.name())
            }
            ["party-style", who, style] | ["party-ability", who, style] | ["party-cure", who, style, ..] => {
                let Some(c) = chara_word(who) else { return format!("party: {text}: donald | goofy") };
                let mut ai = self.ai_settings(c);
                let value = match (w[0], *style) {
                    ("party-style", "together") => Some((0, 0)),
                    ("party-style", "free") => Some((0, 1)),
                    ("party-style", "support") => Some((0, 2)),
                    ("party-style", "focus") => Some((0, 3)),
                    ("party-ability", "free") => Some((1, 0)),
                    ("party-ability", "balance") => Some((1, 1)),
                    ("party-ability", "less") => Some((1, 2)),
                    ("party-cure", "often") => Some((2, 0)),
                    ("party-cure", "pinch") => Some((2, 1)),
                    ("party-cure", "always") => Some((2, 2)),
                    _ => None,
                };
                let Some((row, v)) = value else { return format!("party: {text}: party-style together|free|support|focus, party-ability free|balance|less, party-cure often|pinch|always [targets BITS]") };
                ai.set_style(row, v);
                if let (Some(&"targets"), Some(bits)) = (w.get(3), number(4)) {
                    ai.cure_targets = bits as u8 & 7;
                }
                self.set_ai_settings(c, ai);
                format!("party: {} AI {}", c.name(), ai.describe())
            }
            ["party-trace", on] => {
                self.trace = *on == "on";
                format!("party: trace {}", if self.trace { "on" } else { "off" })
            }
            ["dummy-target", n, who] => {
                let Some(n) = n.parse::<u32>().ok() else { return format!("party: {text}: dummy-target N sora|donald|goofy|near") };
                let victim = match *who {
                    "near" => Victim::Near,
                    "sora" => Victim::Sora,
                    other => match chara_word(other) {
                        Some(c) => Victim::Friend(c),
                        None => return format!("party: {text}: dummy-target N sora|donald|goofy|near"),
                    },
                };
                self.set_victim(n, victim);
                format!("fixture: dummy {n} attacks {victim:?}")
            }
            ["team-gauge", ..] => "team-gauge: stream D has not named its kh-core call yet (contract D.5)".to_string(),
            ["party", "x2", rest @ ..] => crate::custom::word(self, rest),
            // (DON-SPELLB) TEST / DEBUG: `party don-status N KIND SECONDS`: puts one of Donald's statuses on target N (the test
            // level's dummy N) to look at its effect on the body: wet | chill1..chill4 | frozen | burn | reveal | exposed | blind |
            // slow | clear (several on one target stack)
            ["party", "don-status", n, kind, secs] => {
                let (Ok(n), Ok(t)) = (n.parse::<u32>(), secs.parse::<f32>()) else { return format!("party: {text}: don-status N KIND SECONDS") };
                let s = self.party.don.status.entry(n, kh_party::donald::Size::Small);
                match *kind {
                    "wet" => s.wet = t,
                    "chill1" | "chill2" | "chill3" | "chill4" => {
                        s.chill = kind[5..].parse().unwrap_or(1);
                        s.chill_left = t;
                    }
                    "frozen" => s.frozen = t,
                    "burn" => s.burn = t,
                    "reveal" | "exposed" => {
                        s.reveal = t;
                        s.exposed = *kind == "exposed";
                    }
                    "blind" => s.blind = t,
                    "slow" => s.big_slow = t,
                    "clear" => *s = kh_party::donald::TargetStatus::new(s.size),
                    _ => return format!("party: don-status {kind}: wet | chill1..chill4 | frozen | burn | reveal | exposed | blind | slow | clear"),
                }
                format!("party: target {n} {kind} for {t} s")
            }
            // (DON-MAGIC) TEST / DEBUG: `party don-cast ID` (a row id of party-don-spells: l2.fire.1, hy.fire_thunder ...): the
            // first Donald casts it at the nearest foe at the next step (kh_party::donald::cast party_step), free when the
            // rules refuse it (to look at every spell's effects in the test level)
            ["party", "don-cast", id] => match kh_party::donald::SpellRef::from_id(id) {
                Some(s) => {
                    self.party.don_fx.debug_casts.push(s);
                    format!("party: Donald casts {id} at the next step")
                }
                None => format!("party: don-cast {id}: no such spell row"),
            },
            // (ZETTA) TEST / DEBUG: `party don-zetta-ready`: every Donald meets Zettaflare's condition now (it learned, every learned
            // element at Resonance 3 in this fight, a full Arcane Flow, his MP up to its price): `party don-cast y.zettaflare.1`
            // then casts it by the rules (kh_party::donald::zettaflare::debug_make_ready)
            // (ZETTA) TEST / DEBUG: `party don-ult ELEMENT` (fire, blizzard, thunder, water, aero, light): the first Donald plays
            // that element's ultimate presentation (Zettaflare's circle and casting animation in its colour, 3 s to the release;
            // the payload is DON-SPELLB's); `party zetta-cinematic on|off|sheet`: the cinematic camera's key
            ["party", "don-ult", e] => match kh_party::donald::Element::ALL.into_iter().find(|x| format!("{x:?}").eq_ignore_ascii_case(e)) {
                Some(x) => {
                    self.party.don_zetta.debug_ults.push(x);
                    format!("party: Donald plays the {x:?} ultimate's cast at the next step")
                }
                None => format!("party: don-ult {e}: fire | blizzard | thunder | water | aero | light"),
            },
            // (AUD) TEST / DEBUG: `party don-attune ELEMENT [RES]` (fire ... light | none; RES 0..3, default 3): every Donald is
            // attuned to it at once, at that Resonance (no cast, no Release), to look at the attuned missiles, slams, dodges,
            // glide and auras in the test level
            ["party", "don-attune", e, rest @ ..] => {
                let res = rest.first().and_then(|r| r.parse::<f32>().ok()).unwrap_or(3.0).clamp(0.0, 3.0);
                let el = kh_party::donald::Element::ALL.into_iter().find(|x| format!("{x:?}").eq_ignore_ascii_case(e));
                if el.is_none() && *e != "none" {
                    return format!("party: don-attune {e}: fire | blizzard | thunder | water | aero | light | none [RES]");
                }
                let ids: Vec<_> = self.party.members().collect();
                let mut n = 0;
                for id in ids {
                    let Some(st) = self.party.status_mut(id).and_then(|s| s.donald.as_mut()) else { continue };
                    st.core.attune.clear();
                    if let Some(x) = el {
                        let _ = st.core.attune.attune(x, false, 3.0, 0.0);
                        if let Some(s) = st.core.attune.slot_mut(x) {
                            s.res = res;
                        }
                    }
                    n += 1;
                }
                format!("party: {n} Donald(s) attuned to {e} at Resonance {res}")
            }
            ["party", "zetta-cinematic", v] => {
                let on = match *v {
                    "on" | "1" => Some(true),
                    "off" | "0" => Some(false),
                    _ => None,
                };
                kh_party::donald::zettaflare::set_cinematic(on);
                format!("party: Zettaflare's cinematic camera {}", on.map_or("as the sheet (zetta.cinematic)", |b| if b { "on" } else { "off" }))
            }
            // (ZETTA round 5) `party zetta-ending on|off|sheet`: the ending (kg702c: the wide shot of the beam, the shot behind him)
            ["party", "zetta-ending", v] => {
                let on = match *v {
                    "on" | "1" => Some(true),
                    "off" | "0" => Some(false),
                    _ => None,
                };
                kh_party::donald::zettaflare::set_ending(on);
                format!("party: Zettaflare's ending {}", on.map_or("as the sheet (zetta.ending)", |b| if b { "on" } else { "off" }))
            }
            // (DON-MAGIC part A) TEST / DEBUG: `party don-warp X Y YAW` (m, deg): the first Donald is held there every step (his
            // AI still runs) until `party don-free`; for the line-1 side-by-side with Sora
            ["party", "don-warp", x, y, yaw] => match (x.parse::<f32>(), y.parse::<f32>(), yaw.parse::<f32>(), self.first_donald()) {
                (Ok(x), Ok(y), Ok(yaw), Some(id)) => {
                    self.don_hold = Some((kh_core::Vec3::new(x * 100.0, y * 100.0, 0.0), yaw));
                    if let Some(p) = self.party.member_mut(id) {
                        p.cmc.location = kh_core::Vec3::new(x * 100.0, y * 100.0, p.cmc.location.z);
                        p.cmc.stop_movement();
                        p.yaw = yaw;
                    }
                    format!("party: Donald held at ({x}, {y}) m facing {yaw}")
                }
                _ => "party: don-warp X Y YAW (m, deg)".to_string(),
            },
            ["party", "don-free"] => match self.first_donald() {
                Some(_) => {
                    self.don_hold = None;
                    "party: Donald free (his AI again)".to_string()
                }
                _ => "party: no Donald to free".to_string(),
            },
            // (FACE) TEST / DEBUG: `party don-talk [MOOD] [SECONDS]` / `party goofy-talk ...`: a talk line's face (KH3's field
            // voice face: TA_<mood>_E_00 + its blink timer, TA_<mood>_M_01; MOOD default | joy | angry | sorrow | surprise,
            // 3 s by default) on the member, as the conversations play it
            ["party", who @ ("don-talk" | "goofy-talk"), rest @ ..] => {
                let chara = if *who == "don-talk" { kh_host::party::PartyChara::Donald } else { kh_host::party::PartyChara::Goofy };
                let mood = rest.first().and_then(|m| kh_party::face::Mood::from_name(m)).unwrap_or_default();
                let secs = rest.iter().find_map(|s| s.parse::<f32>().ok()).unwrap_or(3.0);
                match self.members.iter().find(|m| m.0 == chara).map(|m| m.1) {
                    Some(id) if kh_party::face::talk(&mut self.party, id, mood, secs) => format!("party: {} talks for {secs} s ({mood:?})", chara.name()),
                    _ => format!("party: {} is not in the party", chara.name()),
                }
            }
            // (FACE) TEST / DEBUG: `party don-voice GROUP [N]` / `party goofy-voice ...`: the member says a voice-group line
            // the way the state code does (`NpcPawn::queue_voice`: QuarterHP / RecoveryHP / BattleStart / FriendLink /
            // MagicCast / GiveRecovery / Damage, unit N, Replace): a line no body clip authored lips for (the face layer's
            // lip-sync)
            ["party", who @ ("don-voice" | "goofy-voice"), group, rest @ ..] => {
                use kh_core::host::party::VoiceGroup as G;
                let chara = if *who == "don-voice" { kh_host::party::PartyChara::Donald } else { kh_host::party::PartyChara::Goofy };
                let g = match group.to_ascii_lowercase().as_str() {
                    "quarterhp" => G::QuarterHp,
                    "recoveryhp" => G::RecoveryHp,
                    "friendlink" => G::FriendLink,
                    "battlestart" => G::BattleStart,
                    "magiccast" => G::MagicCast,
                    "giverecovery" => G::GiveRecovery,
                    "damage" => G::Damage,
                    _ => return format!("party: {who} {group}: QuarterHP | RecoveryHP | FriendLink | BattleStart | MagicCast | GiveRecovery | Damage"),
                };
                let n = rest.first().and_then(|n| n.parse::<i32>().ok()).unwrap_or(1);
                match self.members.iter().find(|m| m.0 == chara).map(|m| m.1).and_then(|id| self.party.member_mut(id)) {
                    Some(p) => {
                        p.queue_voice(g, n, kh_core::host::party::VoiceMode::Replace);
                        format!("party: {} says {group} {n}", chara.name())
                    }
                    None => format!("party: {} is not in the party", chara.name()),
                }
            }
            // (FACE) TEST / DEBUG: `party look-head-up donald|goofy DEG` (0 = KH3's own limit; `default` = 30): the head's
            // look-up cap (kh-er keys party.look_head_up.donald / .goofy)
            // (FACE) TEST / DEBUG: `party look-at donald|goofy X Y Z` (m, relative to the member's place now; Z up) / `party
            // look-at WHO off`: a look focus forced over the controller's (a target high above him)
            ["party", "look-at", who, rest @ ..] => {
                let (id, chara) = if *who == "goofy" { (crate::poser::GOOFY, kh_host::party::PartyChara::Goofy) } else { (crate::poser::DONALD, kh_host::party::PartyChara::Donald) };
                // (FACE) X ahead of him, Y to his left (his facing), Z up
                let at = self.members.iter().find(|m| m.0 == chara).and_then(|m| self.party.view(m.1)).map(|v| (v.position, v.yaw));
                match (rest, at) {
                    (["off"], _) => {
                        crate::poser::set_forced_look(id, None);
                        format!("party: {who} looks as his controller says")
                    }
                    ([x, y, z], Some((p, yaw))) => match (x.parse::<f32>(), y.parse::<f32>(), z.parse::<f32>()) {
                        (Ok(x), Ok(y), Ok(z)) => {
                            let (s, c) = yaw.to_radians().sin_cos();
                            let point = p + kh_core::Vec3::new(x * c - y * s, x * s + y * c, z) * 100.0;
                            crate::poser::set_forced_look(id, Some(point));
                            format!("party: {who} looks at {point:?}")
                        }
                        _ => "party: look-at WHO X Y Z (m) | off".to_string(),
                    },
                    _ => "party: look-at WHO X Y Z (m) | off (the member must be in the party)".to_string(),
                }
            }
            ["party", "look-head-up", who, deg] => {
                let id = if *who == "goofy" { crate::poser::GOOFY } else { crate::poser::DONALD };
                let v = if *deg == "default" { None } else { deg.parse::<f32>().ok() };
                crate::poser::set_look_head_up_max(id, v);
                format!("party: {who}'s head look-up cap {}", v.map_or(format!("{} deg (default)", crate::poser::LOOK_HEAD_UP_MAX_DEG), |d| if d <= 0.0 { "KH3's own".to_string() } else { format!("{d} deg") }))
            }
            ["party", "don-zetta-ready"] => {
                let n = kh_party::donald::zettaflare::debug_make_ready(&mut self.party);
                // (ZETTA) Zettaflare is the player-led Donald's only (the user: "make sure AI donald cannot use Zettaflare")
                let led = self.party.members().any(|id| self.party.member(id).is_some_and(|m| m.pad.is_some() && m.status.donald.is_some()));
                format!("party: {n} Donald(s) meet Zettaflare's condition{}", if led { "" } else { " (NO DONALD LEADS: Zettaflare is the player-led Donald's only; `leader donald` first, or a cast is refused)" })
            }
            // (DON-MAGIC) `party don-cast ID sora`: an ally spell (Aero Barrier, Ball Lightning) cast on Sora instead of Donald
            ["party", "don-cast", id, "sora"] => match kh_party::donald::SpellRef::from_id(id) {
                Some(s) => {
                    self.party.don_fx.debug_casts.push(s);
                    self.party.don_fx.debug_on_sora = true;
                    format!("party: Donald casts {id} on Sora at the next step")
                }
                None => format!("party: don-cast {id}: no such spell row"),
            }, // (party X2) the leader's magic: debug words
            _ => format!("party: {text}: not a party word (party on|off, party-hp, party-ko, party-style, party-ability, party-cure, party-trace, party-recover, dummy-target; party-word and team-gauge are answered by the streams' own debug words)"),
        }
    }
}

// ------------------------------------------------------------------------------------------- the team cameras (stream D)

impl PartyRuntime {
    /// THE TEAM ATTACKS' CAMERAS (contract D.4), after the field camera in the same step: the step's events go to
    /// the state-event cameras; while they return a view it replaces the drawn one. Returns true when the field
    /// camera is to be put behind Sora again.
    pub fn team_camera(&mut self, dt: f32, events: &[kh_core::Event], sora: &Output, field: &mut kh_camera::CameraView, world: &dyn kh_core::World, stick: kh_camera::CameraInput) -> bool {
        self.zetta_behind = false; // (ZETTA round 5) set again below for the one step the hand-back begins
        for e in events {
            self.cams.on_event(&e.kind);
        }
        for a in std::mem::take(&mut self.actor_actions) {
            self.cams.on_actor_action(a);
        }
        let field_fov = kh_camera::projection(&kh_camera::CAMERA_PARAMS).0;
        // (party X3G-b) a leading Goofy's solo Shield Guardian mode: shield_01 on him (fed as the accompany pawn while no
        // team attack has one); its end hands back to the field camera
        let pad_shield = if self.team.accompany().is_none() { self.leader.and_then(|id| self.party.goofy_mode_actor(id)) } else { None };
        self.cams.pad_shield(pad_shield.is_some());
        let targets = kh_camera::camera_anim::CameraTargets { sora: (sora.position, sora.yaw), accompany: self.team.accompany().or(pad_shield), friend_camera_target: self.team.camera_target(), field: kh_camera::camera_anim::AnimView { location: field.position, rotation: field.rotation, fov: field_fov }, stick }; // (D3) + stick: shield_01 is steered; (party D4) sora = his actor origin (feet), as KH3's camera base
        // (D3) with the level: the collision pass of the event cameras and shield_01's own step.
        let out = self.cams.update_in(dt, &targets, Some(world));
        self.team_fov = None;
        if let Some(view) = out.view {
            field.position = view.location;
            field.rotation = view.rotation;
            self.team_fov = Some(view.fov);
        }
        self.team_shake = out.shake;
        self.zetta_camera(dt, field, field_fov, world, stick); // (ZETTA) Zettaflare's cinematic camera on top
        out.reset_field_camera || self.zetta_behind /* (ZETTA round 5) behind Donald as the hand-back begins */
    }

    /// (ZETTA) ZETTAFLARE'S CINEMATIC CAMERA (kh_party::donald::zettaflare, both hosts: the test level's player.rs and Elden
    /// Ring's KhCam call `team_camera`): the KH3 event's shots of Donald and the target laid over the drawn view by their
    /// share (the blend in, the smooth hand-back at the end; none at once after an interrupt or his KO), pulled out of the
    /// level by the camera's collision pass from the shot's subject; the camera stick held hands back early.
    fn zetta_camera(&mut self, dt: f32, field: &mut kh_camera::CameraView, field_fov: f32, world: &dyn kh_core::World, stick: kh_camera::CameraInput) {
        self.zetta_white = 0.0;
        self.zetta_cinematic = false; // (ZFIN round 7)
        let Some(v) = self.party.don_zetta_view(dt, stick.right_stick.length().min(1.0)) else {
            self.zetta_handing = false;
            return;
        };
        self.zetta_cinematic = true; // (ZFIN round 7)
        self.zetta_white = v.white.clamp(0.0, 1.0);
        // (round 5) the hand-back begins: the gameplay camera is put behind Donald (the shot behind him eases into it)
        if v.handing_back && !self.zetta_handing {
            self.zetta_behind = true;
        }
        self.zetta_handing = v.handing_back;
        // (ZETTA 2026-10-23, the user: "the camera was also not very zoomed out during the thinning of the beam shot, it was very
        // close to Donald", then "Also Zettaflare should obey wall collision again, now you don't see anything cause it's gone
        // through a wall") THE SHOT KEPT OUT OF THE LEVEL, ITS FRAMING KEPT: swung round its subject at the same distance and
        // lens, else mirrored across the beam, else pulled in with the lens widened (`ZettaSolve`); chosen at each cut and held
        let (at, rotation, fov) = self.zetta_solve.view(&v, world, dt);
        let mut v = v;
        v.rotation = rotation;
        v.fov = fov;
        let w = v.weight.clamp(0.0, 1.0);
        let wrap = |d: f32| ((d + 540.0) % 360.0) - 180.0;
        let r = field.rotation;
        field.position = field.position.lerp(at, w);
        field.rotation = kh_camera::Rot { pitch: r.pitch + (v.rotation.x - r.pitch) * w, yaw: r.yaw + wrap(v.rotation.y - r.yaw) * w, roll: r.roll + (v.rotation.z - r.roll) * w };
        let under = self.team_fov.unwrap_or(field_fov);
        self.team_fov = Some(under + (v.fov - under) * w);
        // (ZETTA round 5c) the shake of the target in the beam (already x the view's share): its turn on the drawn view, its sway
        // in the camera's own frame
        if v.shake_rot != kh_core::Vec3::ZERO || v.shake_loc != kh_core::Vec3::ZERO {
            let r = field.rotation;
            field.rotation = kh_camera::Rot { pitch: r.pitch + v.shake_rot.x, yaw: r.yaw + v.shake_rot.y, roll: r.roll + v.shake_rot.z };
            let (sy, cy) = field.rotation.yaw.to_radians().sin_cos();
            field.position += kh_core::Vec3::new(-sy, cy, 0.0) * v.shake_loc.y + kh_core::Vec3::Z * v.shake_loc.z;
        }
    }
}

impl PartyRuntime {
    /// (ZETTA 2026-10-23, the user: "Goofy also randomly starts hopping after you do Zettaflare and never stops, fix this
    /// behaviour and make him act perfectly") After the party's step: (1) the step Zettaflare's span ends (its hand-back),
    /// every AI friend's running move is stopped and the waiting spots cleared: each is back on his own tree's follow / combat
    /// at once (nothing of Zettaflare's carries on); (2) the loop guard (hopguard.rs): a friend who takes off four times in six
    /// seconds is stopped and handed to the host (`hop_loops`) to be put back by the leader, the line saying what he did.
    pub fn zetta_allies_back(&mut self, now: f64) {
        use kh_party::NpcStateId as S;
        let span = self.party.don_zetta_span();
        let ai: Vec<kh_party::MemberId> = self.party.members().filter(|id| self.party.member(*id).is_some_and(|p| p.pad.is_none())).collect();
        if self.zetta_span_was && !span {
            self.party.don_zetta.wait_spots.clear();
            let mut stopped = Vec::new();
            for id in &ai {
                let Some(p) = self.party.member(*id) else { continue };
                if matches!(p.command_running(), Some(kh_party::MemberCommand::MoveTo { .. })) {
                    stopped.push(format!("{:?}", p.chara));
                    self.party.command(*id, kh_party::MemberCommand::Stop);
                }
            }
            say!("party: t={now:.3}s ZETTAFLARE'S HAND-BACK: the friends are back on their own AI (waiting spots cleared; moves stopped: {})", if stopped.is_empty() { "none".to_string() } else { stopped.join(", ") });
        }
        self.zetta_span_was = span;
        self.hop_loops.clear();
        for id in ai {
            let Some(p) = self.party.member(id) else { continue };
            let air = matches!(p.state, S::Jump | S::NavLinkJump | S::Hopping);
            if self.hop_guard.observe(id, air, now) {
                let leader = self.leader.and_then(|l| self.party.view(l)).map(|v| v.position);
                say!("party: t={now:.3}s {:?} LOOPS A MOVE: {} take-offs in {} s (state {:?}, command {:?}, at ({:.0}, {:.0}, {:.0}), the leader at {:?}, Zettaflare's span {span}): stopped, put back by the leader", p.chara, crate::hopguard::MAX_HOPS, crate::hopguard::WINDOW_S, p.state, p.command_running(), p.position().x, p.position().y, p.position().z, leader.map(|l| (l.x as i32, l.y as i32, l.z as i32)));
                self.party.command(id, kh_party::MemberCommand::Stop);
                self.hop_loops.push(id);
            }
        }
    }
}

/// (ZETTA 2026-10-23) A TARGET'S LOW SHOT OVER RISING GROUND (after [`ZettaSolve`] kept the shot out of the level), for
/// the target's low shots (920 / 930 / 940) only: the ground under the camera (traced at its spot from the target's centre height down)
/// rising above `ZETTA_CAM_GROUND_CLEAR` under it (a slope uphill of the target) lifts it out, its pitch turned by as much
/// as it takes to keep the target where it was in the frame. Pure but for the world's trace.
pub fn zetta_cinematic_view(v: &kh_party::donald::zettaflare::ZettaView, world: &dyn kh_core::World) -> (kh_core::Vec3, kh_core::Vec3) {
    use kh_core::Vec3;
    let (mut at, mut rot) = (v.position, v.rotation);
    if kh_party::donald::zettaflare::SUBJECT_SHOTS.contains(&v.shot) {
        let start = Vec3::new(at.x, at.y, at.z.max(v.pivot.z) + 50.0);
        let end = at - Vec3::Z * ZETTA_CAM_GROUND_CLEAR;
        if let Some(hit) = world.line_trace(start, end) {
            let ground = hit.position.z;
            if !hit.start_penetrating && hit.impact_normal.z > 0.3 && ground + ZETTA_CAM_GROUND_CLEAR > at.z {
                let pitch_to = |p: Vec3| { let d = v.pivot - p; d.z.atan2(Vec3::new(d.x, d.y, 0.0).length().max(1e-3)).to_degrees() };
                let lifted = Vec3::new(at.x, at.y, ground + ZETTA_CAM_GROUND_CLEAR);
                rot.x += pitch_to(lifted) - pitch_to(at);
                at = lifted;
            }
        }
    }
    (at, rot)
}

/// (ZETTA 2026-10-23, the user: "Also Zettaflare should obey wall collision again, now you don't see anything cause it's gone
/// through a wall") ZETTAFLARE'S CAMERA AGAINST THE LEVEL, every shot, in both hosts (the world they pass is the camera's own
/// collision layer: Elden Ring's camera filter, the test level's world). A shot is BLOCKED when the camera's sphere, swept
/// from its subject (`pivot`) to it, meets the level first: it would be inside or behind geometry. A blocked shot is
/// resolved keeping its framing, in this order: (1) SWUNG round its subject at the same distance and lens (yaw by 5 degree
/// steps out to 90 either side, then 8 / 16 degrees higher): the view turned with it, so the subject keeps its place on the
/// screen and the beam reads the same way; (2) the MIRRORED shot (the other side of the beam's vertical plane) and its
/// swings; (3) only then PULLED IN along its line to where the level allows, the lens widened by as much (the subject's
/// size on the screen kept: no crop; at most +30 degrees, 110 at most). The way is chosen at each cut (a new shot) and held
/// for the shot; if it becomes blocked mid-shot it is chosen again and blended to over [`ZETTA_RESOLVE_BLEND_S`]. A
/// target's low shot is then lifted out of rising ground (`zetta_cinematic_view`).
#[derive(Clone, Debug, Default)]
pub struct ZettaSolve {
    shot: &'static str,
    way: ZettaWay,
    blend: Option<(kh_core::Vec3, kh_core::Vec3, f32, f32)>,
    last: Option<(kh_core::Vec3, kh_core::Vec3, f32)>,
}

/// (ZETTA 2026-10-23) The way a shot is kept clear (see [`ZettaSolve`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ZettaWay {
    /// Where it is authored.
    #[default]
    Authored,
    /// Swung round its subject (yaw, pitch degrees), mirrored across the beam or not.
    Swung { yaw: f32, pitch: f32, mirror: bool },
    /// Pulled in (mirrored or not), the lens widened.
    Pulled { mirror: bool },
}

/// (ZETTA 2026-10-23) A mid-shot re-choice blends over this long (s).
pub const ZETTA_RESOLVE_BLEND_S: f32 = 0.35;

fn zetta_forward(rot: kh_core::Vec3) -> kh_core::Vec3 {
    let (p, y) = (rot.x.to_radians(), rot.y.to_radians());
    kh_core::Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), p.sin())
}

fn zetta_rot_of(f: kh_core::Vec3, roll: f32) -> kh_core::Vec3 {
    kh_core::Vec3::new(f.z.clamp(-1.0, 1.0).asin().to_degrees(), f.y.atan2(f.x).to_degrees(), roll)
}

/// (ZETTA 2026-10-23) The shot's camera kept clear by `way` (position, rotation, FOV), before the pull-in's own trace.
fn zetta_way_pose(v: &kh_party::donald::zettaflare::ZettaView, yaw: f32, pitch: f32, mirror: bool) -> (kh_core::Vec3, kh_core::Vec3) {
    use glam::Quat;
    use kh_core::Vec3;
    let mut d = v.position - v.pivot;
    let mut f = zetta_forward(v.rotation);
    let mut roll = v.rotation.z;
    if mirror {
        let b = Vec3::new(v.beam_dir.x, v.beam_dir.y, 0.0).normalize_or(Vec3::X);
        let n = Vec3::new(-b.y, b.x, 0.0);
        d -= 2.0 * d.dot(n) * n;
        f -= 2.0 * f.dot(n) * n;
        roll = -roll;
    }
    let len = d.length();
    if len < 1e-3 {
        return (v.position, v.rotation);
    }
    // the swing: a rigid turn about the subject (yaw about the vertical, then the elevation), the view turned with it
    let q_yaw = Quat::from_rotation_z(yaw.to_radians());
    let d1 = q_yaw * d;
    let h = Vec3::new(d1.x, d1.y, 0.0);
    let el = d1.z.atan2(h.length()) + pitch.to_radians();
    let el = el.clamp(-1.4, 1.4);
    let d2 = h.normalize_or(Vec3::X) * (el.cos() * len) + Vec3::Z * (el.sin() * len);
    let q_el = Quat::from_rotation_arc(d1.normalize(), d2.normalize());
    let f2 = q_el * (q_yaw * f);
    (v.pivot + d2, zetta_rot_of(f2, roll))
}

/// (ZETTA 2026-10-23) Is the camera at `at` clear of the level, seen from its subject? (the camera's sphere swept from the
/// subject meets nothing before it; a subject already in contact is not counted)
pub fn zetta_clear(pivot: kh_core::Vec3, at: kh_core::Vec3, world: &dyn kh_core::World) -> bool {
    let r = kh_camera::CAMERA_PARAMS.collision_radius;
    !matches!(world.sweep_capsule(pivot, at, r, r), Some(hit) if !hit.start_penetrating && hit.time < 0.999)
}

/// (ZETTA 2026-10-23) The ways tried, in order: authored, its swings (yaw first, then a little pitch), the mirrored shot
/// and its swings.
fn zetta_ways() -> Vec<ZettaWay> {
    let mut out = vec![ZettaWay::Authored];
    for mirror in [false, true] {
        for pitch in [0.0f32, 8.0, 16.0] {
            for k in 0..=18 {
                for sign in [1.0f32, -1.0] {
                    let yaw = 5.0 * k as f32 * sign;
                    if k == 0 && (sign < 0.0 || (pitch == 0.0 && !mirror)) {
                        continue;
                    }
                    out.push(ZettaWay::Swung { yaw, pitch, mirror });
                }
            }
        }
    }
    out
}

impl ZettaSolve {
    /// (ZETTA 2026-10-23) The camera of `way` for this frame of the shot (position, rotation, FOV).
    pub fn pose_of(v: &kh_party::donald::zettaflare::ZettaView, way: ZettaWay, world: &dyn kh_core::World) -> (kh_core::Vec3, kh_core::Vec3, f32) {
        match way {
            ZettaWay::Authored => (v.position, v.rotation, v.fov),
            ZettaWay::Swung { yaw, pitch, mirror } => {
                let (at, rot) = zetta_way_pose(v, yaw, pitch, mirror);
                (at, rot, v.fov)
            }
            ZettaWay::Pulled { mirror } => {
                let (far, rot) = zetta_way_pose(v, 0.0, 0.0, mirror);
                let at = kh_camera::camera_anim::collide(v.pivot, far, world);
                let (l0, l1) = ((far - v.pivot).length(), (at - v.pivot).length().max(1.0));
                let wide = (2.0 * ((v.fov * 0.5).to_radians().tan() * (l0 / l1)).atan().to_degrees()).min(v.fov + 30.0).min(110.0).max(v.fov);
                (at, rot, wide)
            }
        }
    }

    /// (ZETTA 2026-10-23) The first way that keeps this frame of the shot clear: a swing, the mirrored shot, else the pull-in
    /// (of the two lines, the one the level lets come further out).
    pub fn choose(v: &kh_party::donald::zettaflare::ZettaView, world: &dyn kh_core::World) -> ZettaWay {
        for way in zetta_ways() {
            let (at, _, _) = Self::pose_of(v, way, world);
            if zetta_clear(v.pivot, at, world) {
                return way;
            }
        }
        let out = |mirror: bool| (Self::pose_of(v, ZettaWay::Pulled { mirror }, world).0 - v.pivot).length();
        ZettaWay::Pulled { mirror: out(true) > out(false) + 1.0 }
    }

    /// (ZETTA 2026-10-23) One frame of Zettaflare's camera: the way chosen at the cut and held, chosen again (and blended to)
    /// only when it becomes blocked; then a target's low shot lifted out of the ground.
    pub fn view(&mut self, v: &kh_party::donald::zettaflare::ZettaView, world: &dyn kh_core::World, dt: f32) -> (kh_core::Vec3, kh_core::Vec3, f32) {
        if v.shot != self.shot {
            // a cut: chosen for this shot, no blend
            self.shot = v.shot;
            self.way = Self::choose(v, world);
            self.blend = None;
        } else if !matches!(self.way, ZettaWay::Pulled { .. }) && !zetta_clear(v.pivot, Self::pose_of(v, self.way, world).0, world) {
            // something new blocks it mid-shot: chosen again, blended to
            self.way = Self::choose(v, world);
            if let Some((p, r, f)) = self.last {
                self.blend = Some((p, r, f, 0.0));
            }
        }
        let (at, rot, fov) = Self::pose_of(v, self.way, world);
        let mut lifted = v.clone();
        (lifted.position, lifted.rotation) = (at, rot);
        let (at, rot) = zetta_cinematic_view(&lifted, world);
        let out = match self.blend.as_mut() {
            Some((p, r, f, t)) => {
                *t += dt;
                let w = (*t / ZETTA_RESOLVE_BLEND_S).clamp(0.0, 1.0);
                let w = w * w * (3.0 - 2.0 * w);
                let wrap = |d: f32| ((d + 540.0) % 360.0) - 180.0;
                let o = (p.lerp(at, w), kh_core::Vec3::new(r.x + (rot.x - r.x) * w, r.y + wrap(rot.y - r.y) * w, r.z + (rot.z - r.z) * w), *f + (fov - *f) * w);
                if w >= 1.0 {
                    self.blend = None;
                }
                o
            }
            None => (at, rot, fov),
        };
        self.last = Some(out);
        out
    }

    /// (ZETTA 2026-10-23) The way the present shot is kept clear.
    pub fn way(&self) -> ZettaWay {
        self.way
    }
}

/// (ZETTA 2026-10-23) How far over the ground a target's low cinematic shot is kept (cm).
pub const ZETTA_CAM_GROUND_CLEAR: f32 = 30.0;

// (XSI, 2026-10-16) SoraTargets passes the XS abilities' questions to the wrapped targets: without it Hidden Potential,
// the spectacle cap (`damage_to_hp`) and Rage Raid saw the trait's defaults whenever the party wraps Sora's targets.
#[cfg(test)]
mod xsi_tests {
    use super::*;
    use kh_core::host::{HitResult, HurtShape, TargetInfo};
    use kh_core::{HitRecord, Targets};

    struct Foe {
        ended: bool,
    }

    impl Targets for Foe {
        fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
        fn info(&self, _id: TargetId) -> Option<TargetInfo> {
            None
        }
        fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
        fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
            HitResult::default()
        }
        fn foe_level(&self, _id: TargetId) -> Option<u8> {
            Some(42)
        }
        fn damage_to_hp(&self) -> f32 {
            0.5
        }
        fn in_revenge(&self, _id: TargetId) -> bool {
            !self.ended
        }
        fn end_revenge(&mut self, _id: TargetId) {
            self.ended = true;
        }
    }

    #[test]
    fn xsi_sora_targets_pass_the_xs_questions_on() {
        let mut rt = PartyRuntime::new(RuntimeConfig { wanted: false, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        let mut foe = Foe { ended: false };
        let mut t = SoraTargets { targets: &mut foe, runtime: &mut rt };
        assert_eq!((t.foe_level(TargetId(1)), t.damage_to_hp(), t.in_revenge(TargetId(1))), (Some(42), 0.5, true));
        t.end_revenge(TargetId(1));
        assert!(!t.in_revenge(TargetId(1)));
    }
}

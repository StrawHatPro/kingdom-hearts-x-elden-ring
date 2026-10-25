//! The party (contract B.3): its members, their projectiles, the per-step order and the
//! cross-member requests (heals, MP).

use crate::command::{CommandStatus, CommandTicket, MemberCommand, TargetRef};
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::pawn::{friend_damage, NpcPawn, Request, StepCtx};
use crate::projectile::{Projectile, ProjectileKind};
use crate::status::FriendStatus;
use crate::tables;
use crate::view::{MemberView, PartyEvent, PartyEventKind, PartyOutput, SoraHeal, SoraView};
use glam::Vec3;
use kh_core::enums::{ETresDamageAttribute, ETresDamageKind};
use kh_core::host::{HitAttacker, HitInfo, HitRecord, HitResult, HurtShape, TargetFlags};
use kh_core::status::Defender;

/// How a party starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartyConfig {
    /// Members start with `FriendStatus::test_level99` (plan N2) instead of `TresChrInitData`.
    pub level99: bool,
    /// The shared EXP total at spawn (members not in the level-99 loadout take their level from it).
    pub total_exp: i32,
    pub seed: u64,
}

impl PartyConfig {
    /// The test level's default (user decision 2): the party at level 99.
    pub fn test_level99() -> PartyConfig {
        PartyConfig { level99: true, total_exp: 1_527_640, seed: 0x5EED }
    }
}

impl Default for PartyConfig {
    fn default() -> PartyConfig {
        PartyConfig { level99: false, total_exp: 0, seed: 0x5EED }
    }
}

/// The party.
#[derive(Clone, Debug)]
pub struct Party {
    pub config: PartyConfig,
    members: Vec<Option<NpcPawn>>,
    pub projectiles: Vec<Projectile>,
    /// (B5) Sora's Blizzard ice-rail nodes as the host last gave them (`set_ice_rails`).
    ice_rails: Vec<kh_core::magic::RailNode>,
    /// (party P3) The notify handlers of removed members whose notify states still had something running (a looping
    /// sound, a persistent effect): stopped at the next step (`stop_removed`), when the host's seams are at hand.
    removed_loops: Vec<kh_core::host::party::PartyNotify>,
    /// (party X2) A friend leader's side of Sora's magic: the host's feed and the leaders' casters (leader_magic.rs).
    pub x2: crate::leader_magic::PartyMagic,
    /// (DON1) the Donald overhaul's party-wide state: the enemies' statuses and the step's events (crate::donald::hooks).
    pub don: crate::donald::hooks::PartyDon,
    /// (DON-MAGIC) Donald's spells in the world: the actors, Sora's actors of his line 1, the worn barriers / orbs
    /// (crate::donald::magic_fx, allyfx, cast).
    pub don_fx: crate::donald::magic_fx::MagicFx,
    /// (DON-PROG) the auras, Out of Sight's clocks, Taunt Pass and AI Donald's memory (crate::donald::ai::party_pass).
    pub don_prog: crate::donald::ai::DonProgState,
    /// (ZETTA) the Zettaflares in progress (crate::donald::zettaflare: the KH3 cutscene's presentation, aimed at the target).
    pub don_zetta: crate::donald::zettaflare::ZettaWorld,
}

impl Party {
    pub fn new(config: PartyConfig) -> Party {
        Party { config, members: Vec::new(), projectiles: Vec::new(), ice_rails: Vec::new(), removed_loops: Vec::new(), x2: Default::default(), don: Default::default(), don_fx: Default::default() /* (DON-MAGIC) */, don_prog: Default::default() /* (DON-PROG) */, don_zetta: Default::default() /* (ZETTA) */ } // (party X2) x2; (DON1) don
    }

    /// (party P3) Stops what removed members' notify states left running (also called by `step`). Hosts that drop the
    /// whole party call it once with their seams.
    pub fn stop_removed(&mut self, audio: &mut dyn kh_core::Audio, effects: &mut dyn kh_core::Effects) {
        for mut n in self.removed_loops.drain(..) {
            n.stop_all(audio, effects);
        }
    }

    pub fn spawn(&mut self, chara: Chara, position: Vec3, yaw: f32) -> MemberId {
        let id = MemberId(self.members.len() as u8);
        let status = if self.config.level99 { FriendStatus::test_level99(chara) } else { FriendStatus::new(chara, self.config.total_exp) };
        let seed = self.config.seed ^ ((id.0 as u64 + 1) << 32) ^ chara.uid() as u64;
        self.members.push(Some(NpcPawn::new(id, chara, status, position, yaw, seed)));
        id
    }

    /// (stream P relay, Party Formation) Puts the member at index `id.0` whatever the spawn order: the
    /// Vec is padded with empty slots up to it and an empty or occupied slot there is replaced; the
    /// RNG seed is the one `spawn` gives that id. Donald stays MemberId(0), Goofy MemberId(1).
    pub fn spawn_as(&mut self, chara: Chara, id: MemberId, position: Vec3, yaw: f32) -> MemberId {
        let i = id.0 as usize;
        while self.members.len() <= i {
            self.members.push(None);
        }
        let status = if self.config.level99 { FriendStatus::test_level99(chara) } else { FriendStatus::new(chara, self.config.total_exp) };
        let seed = self.config.seed ^ ((id.0 as u64 + 1) << 32) ^ chara.uid() as u64;
        self.members[i] = Some(NpcPawn::new(id, chara, status, position, yaw, seed));
        id
    }

    /// The same as `remove`: the slot is left empty (ids of the others do not move).
    pub fn despawn(&mut self, id: MemberId) {
        self.remove(id);
    }

    /// Leaves the member's slot empty (`None`); the other ids stay.
    pub fn remove(&mut self, id: MemberId) {
        if let Some(m) = self.members.get_mut(id.0 as usize) {
            // (party P3) his running notify-state loops are stopped at the next step
            if let Some(p) = m.as_ref().filter(|p| p.notify_seam.open() > 0) {
                self.removed_loops.push(p.notify_seam.clone());
            }
            *m = None;
        }
    }

    pub fn members(&self) -> impl Iterator<Item = MemberId> + '_ {
        self.members.iter().flatten().map(|m| m.id)
    }

    pub fn member(&self, id: MemberId) -> Option<&NpcPawn> {
        self.members.get(id.0 as usize).and_then(|m| m.as_ref())
    }

    /// (party K1) An AI member's speed-matched loop pose in the Move state (crate::locomotion), for the host's drawing
    /// as the pad friends' (pad_state(..).loco).
    pub fn ai_loco(&self, id: MemberId) -> Option<crate::pad::LocoPose> {
        self.member(id).filter(|m| m.pad.is_none() && m.state == crate::NpcStateId::Move).and_then(|m| m.loco.pose)
    }

    pub fn member_mut(&mut self, id: MemberId) -> Option<&mut NpcPawn> {
        self.members.get_mut(id.0 as usize).and_then(|m| m.as_mut())
    }

    pub fn find(&self, chara: Chara) -> Option<MemberId> {
        self.members.iter().flatten().find(|m| m.chara == chara).map(|m| m.id)
    }

    pub fn command(&mut self, id: MemberId, cmd: MemberCommand) -> CommandTicket {
        match self.member_mut(id) {
            Some(m) => m.command(cmd),
            None => CommandTicket(0),
        }
    }

    pub fn command_status(&self, id: MemberId, ticket: CommandTicket) -> CommandStatus {
        self.member(id).map(|m| m.command_status(ticket)).unwrap_or(CommandStatus::Unknown)
    }

    pub fn view(&self, id: MemberId) -> Option<MemberView> {
        self.member(id).map(|m| m.view())
    }

    pub fn status(&self, id: MemberId) -> Option<&FriendStatus> {
        self.member(id).map(|m| &m.status)
    }

    pub fn status_mut(&mut self, id: MemberId) -> Option<&mut FriendStatus> {
        self.member_mut(id).map(|m| &mut m.status)
    }

    /// The AI settings bytes (F writes them through `status_mut`).
    pub fn ai_settings(&self, id: MemberId) -> Option<crate::status::AiSettings> {
        self.status(id).map(|s| s.ai)
    }

    /// Sets the battle flag of every member (the host's engaged count, as Sora's).
    pub fn set_in_battle(&mut self, on: bool) {
        // (ZETTA round 5c, the user: "Keep Donald in combat until the animation fully ends if the target dies") a Donald in
        // the middle of a cast (its clip and recovery: `cast_holds_still`; Zettaflare / an ultimate to its hand-off) stays
        // in battle until it ends; from then the host's state (and its own out-of-combat timing) applies
        let casting: Vec<MemberId> = self.don_zetta.runs.iter().filter(|r| !r.done).map(|r| r.caster).collect();
        for (i, m) in self.members.iter_mut().enumerate() {
            let Some(m) = m.as_mut() else { continue };
            let busy = m.in_battle && (crate::pad::donald::cast_holds_still(m) || casting.contains(&MemberId(i as u8)) || m.don_zetta.down.is_some() /* (ZETTA 2026-10-23) down after it */);
            m.in_battle = on || busy;
        }
    }

    /// Blueprint function-by-name answers for C's decorators (`ArithmeticComparison`,
    /// `BoolPropertyCheck`). `None` = a name this crate does not know.
    pub fn function(&self, id: MemberId, name: &str) -> Option<f32> {
        let m = self.member(id)?;
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        Some(match name {
            // 0x141257340 (Party side answers it; see pawn.rs `can_battle`).
            "CanBattle" => b(m.can_battle()),
            "IsRideOn" => 0.0,
            // no water in the test level: every swim / water query is false (the pawn has no swim state yet)
            "IsSwimming" | "IsUnderWater" | "IsUnderwater" | "IsInWater" | "IsDiving" | "IsSwimSurface" | "IsWaterSurface" => 0.0,
            "IsDead" | "IsDie" => b(m.state == crate::NpcStateId::Die),
            "GetMagicPoint" | "GetMP" => m.status.mp as f32,
            "GetMaxMagicPoint" => m.status.max_mp() as f32,
            "GetHitPoint" | "GetHP" => m.status.hp as f32,
            "GetMaxHitPoint" => m.status.max_hp() as f32,
            "GetHPRate" => m.status.hp as f32 / m.status.max_hp().max(1) as f32,
            "IsMPCharge" | "IsMpCharge" => b(m.status.mp_charge_left > 0.0),
            "IsInBattle" | "IsBattle" => b(m.in_battle),
            "IsGround" | "IsOnGround" => b(m.on_ground()),
            "GetLevel" => m.status.level as f32,
            // C2 (contract C.2): the Magic stat (FUN_1405836E0's MagicPower; Cure = 120 + max(1, Magic), 0x1414742A0)
            "GetMagic" => m.status.magic() as f32,
            // vt+0x17F0: seconds of MP recharge left (0 when not recharging)
            "GetMPChargeTimeLeft" => m.status.mp_charge_left.max(0.0),
            // Donald vt+0x2918 = 0x1411CA900: not restricted (FUN_1405AD320(5): a system restriction flag of
            // FUN_140E8E870, never set in the port) and the ability gate FUN_1405A4EA0(0xCD DONALD_CURE, 1).
            "CanCastCure" => b(m.chara == crate::Chara::Donald && m.status.don_tree_gate("DONALD_CURE").unwrap_or_else(|| m.status.can_pay("DONALD_CURE"))), // (DON-PROG) without the KH3 ability: his state
            // SetEnableWarp (+0xB96): nothing in the test level turns the warp off [inf: the default is on]
            "IsEnableWarp" => 1.0,
            // C3 (contract C.2): pawn vt+0x14F0 with equipped = 1 (ValidateAction 0x14121E9F0 / 0x14121EBF0)
            n if n.starts_with("HasAbility:") => {
                let k = kh_core::enums::ETresAbilityKind::from_game_name(&n["HasAbility:".len()..])?;
                b(m.status.don_has_ability(k).unwrap_or(m.status.abilities.equipped(k) > 0)) // (DON-PROG) DONALD_*: his overhaul magic answers
            }
            // vt+0x1500: the count of an equipped ability (ITEM_UP: item amounts x (1 + 0.5 n), FUN_141263950)
            n if n.starts_with("GetAbilityNum:") => {
                let k = kh_core::enums::ETresAbilityKind::from_game_name(&n["GetAbilityNum:".len()..])?;
                m.status.abilities.equipped(k) as f32
            }
            // FUN_1405AD320(n): the system restriction flags of FUN_140E8E870; none is set in the port
            n if n.starts_with("IsCommandRestricted:") => 0.0,
            // FUN_141267540: bound by a bad status (FUN_1405ACFE0, read here as being in a bad-status state
            // [inf]) or SNEEZE on him while the state component is in 0x97
            "IsBadStatusBound" => b(matches!(m.state, crate::NpcStateId::BadStatStun | crate::NpcStateId::BadStatFreeze | crate::NpcStateId::BadStatSneeze | crate::NpcStateId::BadStatBurn) || m.bad.0 == kh_core::combat::badstat::kind::SNEEZE),
            // ATresAIPawnBase::GetTimeMotionless 0x141927B00 = movement +0xC68 (seconds under 10 cm/s [inf: the
            // movement's own rule is not read])
            "GetTimeMotionless" => m.motionless,
            // (B4, C4 relay) accepted damage reactions (the Interrupt decorator restarts on each change)
            "GetDamageReactionCount" => m.damage_reactions as f32,
            // (party I1) the warp check FUN_141258790 reads the NAVLINK_JUMP state's byte +0xA9 (set with +0xA8 only
            // by the rail leave 0x141252210, word 0x101): a rail-leave jump never warps; any other jump / fall may
            "IsRailLeaveJump" => b(m.state == crate::NpcStateId::NavLinkJump && m.trav.jump.steer),
            _ => return None,
        })
    }

    /// The defender for a hit on a member (kind Friend, damage_max 24).
    /// (party X2) DECISION 1 (the user, 2026-10-15): a member the PAD drives (the friend leader) has no `m_DamageMax`:
    /// KH3's friend rule (a base above 24 becomes floor((24 + S - D) / 2): kh-core damage.rs step 5) is the AI friends'
    /// only; the leader takes damage by Sora's rules. AI members keep 24.
    pub fn defender(&self, id: MemberId) -> Option<Defender> {
        self.member(id).map(|m| {
            let mut d = m.status.as_defender();
            if m.pad.is_some() {
                d.damage_max = 0; // (party X2)
            }
            d
        })
    }

    pub fn receive_hit(&mut self, id: MemberId, hit: &HitRecord) -> HitResult {
        // (ZETTA round 5) the user: "Donald is invincible during Zettaflare's cutscene": from the channel to the hand-off no hit
        // lands on him (no damage, no reaction, nothing that would stop the cast)
        if crate::donald::zettaflare::invincible(self, id) {
            return HitResult::default();
        }
        // (DON-MAGIC) Sanctuary's short invincibility, Aero Barrier's damage cut (the cast's interruption is DON-KIT's hit hook)
        if crate::donald::allyfx::invincible(self, id) {
            return HitResult::default();
        }
        let mut h = *hit;
        h.damage *= crate::donald::allyfx::damage_mult(self, id);
        let r = match self.member_mut(id) {
            Some(m) => m.receive_hit(&h),
            None => HitResult::default(),
        };
        r
    }

    /// The shared total changed (an EXP award): every member checks its own table
    /// (`CheckLevelUp` 0x140EBE400 per party slot, stats spec 5.2). Returns (member, from, to) per level-up.
    pub fn set_total_exp(&mut self, total: i32) -> Vec<(MemberId, u32, u32)> {
        let mut out = Vec::new();
        for m in self.members.iter_mut().flatten() {
            let from = m.status.level;
            if let Some(to) = m.status.set_total_exp(total) {
                m.events.push(PartyEventKind::LevelUp { level: to });
                out.push((m.id, from, to));
            }
        }
        out
    }

    /// (B4) `RequestSuperSlide` (pawn interface vt+0x240 0x141271170, the gate vt+0x238 0x141259FC0): the
    /// member super-slides (SonicSlide) while the AI move feeds a direction; false when refused.
    pub fn request_super_slide(&mut self, id: MemberId) -> bool {
        self.member_mut(id).is_some_and(crate::traverse::request_super_slide)
    }

    /// (B4) `SetSituation(kind, value)` (pawn vt+0x2888 0x141277550): 6 talk point, 7 Goofy's coop, 8 gimmick
    /// wait start the SituationAction state; 0 ends it (the member idles).
    pub fn set_situation(&mut self, id: MemberId, kind: u8, value: f32) -> bool {
        self.member_mut(id).is_some_and(|m| crate::traverse::set_situation(m, kind, value))
    }

    /// (B4) `RequestNpcMotionPlay` (0x141270750, situation 9): start / loop / end motions by anim-set row,
    /// the loop played `count` times (< 0: until `finish_npc_motion`).
    pub fn request_npc_motion(&mut self, id: MemberId, count: i32, anims: [Option<&'static str>; 3]) -> bool {
        self.member_mut(id).is_some_and(|m| crate::traverse::request_npc_motion(m, count, anims))
    }

    /// (B4) The NPC-motion finish request (0x141270970 -> FUN_141245900): the end motion next.
    pub fn finish_npc_motion(&mut self, id: MemberId) -> bool {
        self.member_mut(id).is_some_and(crate::traverse::finish_npc_motion)
    }

    /// (B4) Goofy's coop actor (pawn +0xBE8) location for his situation 7 (None = no actor).
    pub fn set_coop_target(&mut self, id: MemberId, at: Option<Vec3>) {
        if let Some(m) = self.member_mut(id) {
            m.trav.coop_target = at;
        }
    }

    /// (B5) Sora's Blizzard ice rails (`Sora::magic().rails`, kh-core `magic::RailNode`): the host hands them in before
    /// each `step`; the friends' rail probe (the shared 0x1405A7740, which finds every `BlizzardRail` actor) rides them as
    /// it rides the level's rails. An empty slice = none.
    pub fn set_ice_rails(&mut self, nodes: &[kh_core::magic::RailNode]) {
        self.ice_rails.clear();
        self.ice_rails.extend_from_slice(nodes);
    }

    /// (B5) The member's RailSlide state waits for the friend data's warp query (`TresFriendData.WarpQuery`, the
    /// off-rail warp of 0x14124E890): the AI host runs it (kh-ai does it for `PartyBody`) and answers with
    /// `answer_warp_query`.
    pub fn warp_query_wanted(&self, id: MemberId) -> bool {
        self.member(id).is_some_and(crate::traverse::warp_query_wanted)
    }

    /// (B5) The warp query's best item (None: no item; the state asks again on a later tick).
    pub fn answer_warp_query(&mut self, id: MemberId, best: Option<Vec3>) {
        if let Some(m) = self.member_mut(id) {
            crate::traverse::warp_query_answer(m, best);
        }
    }

    /// (B5) `JumpCodeDriven` (iface vt+0x3A8 0x14054C6A0): the member jumps to `target` (a host's nav link; the
    /// rail end uses it inside the crate) with the state's own h0 100 / h_min 0; a target more than 500 above him
    /// looks for a ledge to hang from on the way (the Dangle state). False for a member in a state that refuses it.
    pub fn request_jump_to(&mut self, id: MemberId, target: Vec3, world: &dyn kh_core::World) -> bool {
        let Some(m) = self.member_mut(id) else { return false };
        if m.state.refuses_commands() || crate::traverse::refuses_now(m) {
            return false;
        }
        crate::jump::request_code_driven(m, target, 0.0, false, world);
        true
    }

    /// (party ER S1) The Elden Ring host's waypoint for the member's running MoveTo (`NpcPawn::steer`: (the goal it was
    /// made for, the point to head for); None = the straight line). Port design: KH3 follows its navmesh path.
    pub fn set_steer(&mut self, id: MemberId, steer: Option<(Vec3, Vec3)>) {
        if let Some(m) = self.member_mut(id) {
            m.steer = steer;
        }
    }

    /// (party ER S1) The goal of the member's running MoveTo (None: no MoveTo runs), for the host's waypoint.
    pub fn move_goal(&self, id: MemberId) -> Option<crate::command::MoveGoal> {
        match self.member(id)?.command_now() {
            Some(MemberCommand::MoveTo { goal, .. }) => Some(goal),
            _ => None,
        }
    }

    /// (LADDER2) The member's ladder jump while he is in one (port design, crate::ladder).
    pub fn ladder(&self, id: MemberId) -> Option<crate::ladder::Leap> {
        self.member(id).filter(|m| m.state == crate::states::NpcStateId::Ladder).map(|m| m.ladder)
    }

    /// (LADDER2) The members who left the floor on a ladder jump since the last call, and where (the host plays the
    /// jump's sound there).
    pub fn take_ladder_takeoffs(&mut self) -> Vec<(MemberId, glam::Vec3)> {
        let ids: Vec<MemberId> = self.members().collect();
        let mut out = Vec::new();
        for id in ids {
            if let Some(m) = self.member_mut(id) {
                if std::mem::take(&mut m.ladder.took_off) {
                    out.push((id, m.position()));
                }
            }
        }
        out
    }

    /// A heal from Sora (Cure, an item, Assist Friends) on a member.
    pub fn heal(&mut self, id: MemberId, hp: i32, _effect: u8) -> i32 {
        self.member_mut(id).map(|m| m.status.heal(hp)).unwrap_or(0)
    }

    /// (party D5) The effect of a heal of `kind` on member `id` (`actions::recover_effect`: Donald's Cure ->
    /// vm_cur_bds1 at vfx_abn_center), with the member's own seam handles; its effect-attached SE follows through the
    /// effect seam. Hosts call it for a heal they redirect (a friend leading takes "the player's" heals).
    pub fn heal_effect_on(&mut self, id: MemberId, kind: u8, effects: &mut dyn kh_core::host::Effects) {
        let Some((name, socket, group)) = crate::actions::recover_effect(kind) else { return };
        if let Some(m) = self.member_mut(id) {
            let at = kh_core::host::Attach::party(kh_core::host::PartyMember(id.0), Some(socket));
            m.seam.spawn(effects, &kh_core::host::EffectSpawn { id: kh_core::host::EffectId(group), name, at, rotation: Vec3::ZERO, scale: 1.0, persistent: false });
        }
    }

    /// (party D5) As [`Party::heal_effect_on`] for a heal on Sora (`SoraHeal`), with the healer's seam handles.
    pub fn heal_effect_on_sora(&mut self, from: MemberId, kind: u8, effects: &mut dyn kh_core::host::Effects) {
        let Some((name, socket, group)) = crate::actions::recover_effect(kind) else { return };
        if let Some(m) = self.member_mut(from) {
            let at = kh_core::host::Attach::Sora { bone: Some(socket), offset: Vec3::ZERO };
            m.seam.spawn(effects, &kh_core::host::EffectSpawn { id: kh_core::host::EffectId(group), name, at, rotation: Vec3::ZERO, scale: 1.0, persistent: false });
        }
    }

    /// One fixed step (contract B.3).
    pub fn step(&mut self, dt: f32, sora: &SoraView, host: &mut PartyHost) -> PartyOutput {
        // (DON-MAGIC) EVERYONE'S hits on an Exposed enemy deal x1.25 (documentation/DONALD_OVERHAUL.md 4.5): the party's own
        // hits pass this wrapper of the host's targets (Donald's spells carry their own, magic_fx::DON_HIT_FLAG); after the
        // step the non-Donald hits go through the reaction rule (a friend's hit Exposes a Revealed enemy)
        let exposed = self.don_exposed();
        let team = crate::team::link::don_team_hit_mult(self, "FR_FLAREFORCE"); // DON-KIT's x1.2 at Resonance 3 (Donald's rows)
        let mut wrapped = crate::donald::magic_fx::PartyFx { inner: &mut *host.targets, mults: &exposed, seen: Vec::new(), team };
        crate::donald::zettaflare::hands_off_begin(self); // (ZETTA round 5c) the allies never hit what Zettaflare keeps them off
        let out = {
            let mut h = PartyHost { world: host.world, targets: &mut wrapped, audio: &mut *host.audio, effects: &mut *host.effects };
            self.step_inner(dt, sora, &mut h)
        };
        crate::donald::zettaflare::hands_off_end(); // (ZETTA round 5c)
        let seen = std::mem::take(&mut wrapped.seen);
        crate::donald::magic_fx::after_party_hits(self, host, &seen);
        out
    }

    /// The step itself ((DON-MAGIC) split off so the host's targets can be wrapped).
    fn step_inner(&mut self, dt: f32, sora: &SoraView, host: &mut PartyHost) -> PartyOutput {
        self.stop_removed(host.audio, host.effects); // (party P3)
        crate::donald::hooks::step(self, dt); // (DON1) every Donald's clock, the refunds, the enemies' statuses
        crate::donald::ai::party_pass(self, sora, host, dt); // (DON-PROG) the auras, the aggro clocks, AI Donald's choices
        crate::pad::donald::aura_to_members(self); // (DON-KIT) the Thunder / Aero auras onto each member's motions
        let positions: Vec<(MemberId, Vec3, u8)> = self.members.iter().flatten().map(|m| (m.id, m.position(), m.chara.uid())).collect();
        let zones = self.don_zetta_zones(); // (ZETTA) the allies keep out of Zettaflare's target and beam
        crate::donald::zettaflare::step_out(self, &zones);
        let cx = StepCtx { sora, members: &positions, dt, ice_rails: &self.ice_rails, zones: &zones };
        let mut out = PartyOutput::default();
        let mut requests = Vec::new();
        for slot in self.members.iter_mut() {
            let Some(m) = slot.as_mut() else { continue };
            if m.status.game_level != sora.game_level {
                // (DF) a change of difficulty (Critical halves the bases): the gauges are kept within the new maxima
                m.status.game_level = sora.game_level;
                m.status.hp = m.status.hp.min(m.status.max_hp());
                m.status.mp = m.status.mp.min(m.status.max_mp());
            }
            m.step(&cx, host);
            for e in m.events.drain(..) {
                out.events.push(PartyEvent { member: m.id, kind: e });
            }
            for r in m.requests.drain(..) {
                requests.push((m.id, r));
            }
        }
        for (from, r) in requests {
            match r {
                Request::Projectile(p) => {
                    // (ZETTA round 5c) no ally shot at or through Zettaflare's zones until the player has Donald back
                    let aim = p.target_point.or_else(|| host.targets.info(p.target).map(|i| i.center));
                    let off = host.targets.info(p.target).is_some_and(|i| self.don_zetta_off_limits(i.position, i.radius));
                    if !off && aim.map_or(true, |to| self.don_zetta_path_clear(p.position, to, 50.0)) {
                        self.projectiles.push(p);
                    }
                }
                Request::Heal { target, amount, effect } => match target {
                    TargetRef::Sora => out.sora_heals.push(SoraHeal { amount, effect, mp: 0, from }),
                    // (party AIX) AI Sora (a friend leading): not a member; the host heals him
                    TargetRef::Member(id) if self.member(id).is_none() && sora.ai_sora.is_some_and(|a| a.0 == id) => out.ai_sora_heals.push(SoraHeal { amount, effect, mp: 0, from }),
                    TargetRef::Member(id) => {
                        self.heal(id, amount, effect);
                    }
                    _ => {}
                },
                // (party D5) the heal's effect on the healed member (actions::recover_effect)
                Request::HealEffect { member, kind } => {
                    // (GOOFY-IT) on AI Sora (a friend leading): not a member; the host plays it on Sora (no HP / MP here,
                    // the item's own row carries those)
                    // (a Cure on him already plays its kind through his Heal row: item kinds only here)
                    if self.member(member).is_none() && sora.ai_sora.is_some_and(|a| a.0 == member) && kh_core::host::audio::recover_kind_effect(kind).is_some() {
                        out.ai_sora_heals.push(SoraHeal { amount: 0, effect: kind, mp: 0, from });
                    } else {
                        self.heal_effect_on(member, kind, host.effects);
                    }
                }
                // (GOOFY-IT) an item's effect on the player: the host plays the kind on him (SoraHeal with no HP / MP)
                Request::SoraEffect { kind } => out.sora_heals.push(SoraHeal { amount: 0, effect: kind, mp: 0, from }),
                Request::Item { target, item, boost } => {
                    // (party AIX) an item on AI Sora: his own ItemEffect amounts (as on the player)
                    if let TargetRef::Member(id) = target {
                        if self.member(id).is_none() && sora.ai_sora.is_some_and(|a| a.0 == id) {
                            let (hp, mp) = crate::actions::sora_item_amounts(item, boost);
                            out.ai_sora_heals.push(SoraHeal { amount: hp, effect: 1, mp, from });
                        }
                    }
                    if let TargetRef::Member(id) = target {
                        if let Some(m) = self.member_mut(id) {
                            // ItemEffect 0x1405BE820: x 0.5 when the target is in Die (state 0x3B)
                            let mult = if m.state == crate::NpcStateId::Die { 0.5 * boost } else { boost };
                            m.status.item_effect(item, mult);
                        }
                    }
                }
                Request::Mp { target, amount } => match target {
                    TargetRef::Sora => out.sora_heals.push(SoraHeal { amount: 0, effect: 1, mp: amount, from }),
                    TargetRef::Member(id) if self.member(id).is_none() && sora.ai_sora.is_some_and(|a| a.0 == id) => out.ai_sora_heals.push(SoraHeal { amount: 0, effect: 1, mp: amount, from }), // (party AIX)
                    TargetRef::Member(id) => {
                        if let Some(s) = self.status_mut(id) {
                            if s.mp_charge_left <= 0.0 {
                                s.mp = (s.mp + amount).min(s.max_mp());
                            }
                        }
                    }
                    _ => {}
                },
            }
        }
        crate::leader_magic::step(self, dt, sora, host, &mut out); // (party X2) the leaders' Sora spells
        crate::donald::cast::party_step(self, dt, sora, host, &mut out); // (DON-MAGIC) Donald's casts, Releases, spell actors
        crate::face::party_step(self, dt); // (ZETTA) the members' face-clip slots age (a play this step shows at time 0)
        crate::donald::zettaflare::party_step(self, dt, host); // (ZETTA) Zettaflare's cutscene sequence (after the casts' step)
        crate::pad::donald::party_step(self); // (party X3D) the kit's gauge points to the X2 caster
        crate::pad::donald::party_host_step(self, dt, sora, host); // (DON-KIT) Donald's missiles / areas: hits through Party::don_hit
        self.goofy_party_step(dt); // (party X3G) Goofy's kit: Sora's Fusions / Glide, his own abilities by level
        crate::hurt::feed(self); // (GOOFY-HR) Sora's AIR_RECOVERY for the shared aerial recovery
        self.step_projectiles(dt, host, &mut out);
        out
    }

    /// `UTresAttack8_n_ex002_Discus` hit handler 0x1411CC550 (the +0x48 interface, after the base handler
    /// 0x14122DAE0): a wall hit (`wall`) stops the shield (its +0x440 vt+0x208) and zeroes the fade +0x218;
    /// otherwise the first hit sets the fade to m_fAttackFadeTime. On the first hit: sub-step 2 -> 1, else
    /// 3, and +0x20E = 1.
    fn discus_hit(&mut self, owner: MemberId, wall: bool) {
        let Some(m) = self.member_mut(owner) else { return };
        let Some(mut run) = m.action else { return };
        if run.def != Some(crate::ActionDef::GoofyDiscus) {
            return;
        }
        let fade = tables::aparam(m.chara, "Attack8_Discus", "fAttackFadeTime");
        if !run.caught {
            run.timer2 = if wall { 0.0 } else { fade };
            run.sub = if run.sub == 2 { 1 } else { 3 };
            run.caught = true;
        } else if wall {
            run.timer2 = 0.0;
        }
        m.action = Some(run);
    }

    /// (ZETTA 2026-10-23, the user: "Donald's wand should stay where it falls") Where member `id`'s weapon lies on the
    /// ground (world place, cm, and turn): Donald's staff dropped in Zettaflare's down (zettaflare::StaffAway). The hosts
    /// draw it there instead of in his hand.
    pub fn weapon_ground(&self, id: MemberId) -> Option<(Vec3, glam::Quat)> {
        self.member(id)?.don_zetta.staff?.ground
    }

    /// (party P3) The thrown shield of member `id` (Goofy's Discus) while it is out of his hand.
    pub fn thrown_shield(&self, id: MemberId) -> Option<&Projectile> {
        self.projectiles.iter().find(|p| p.owner == id && p.kind == ProjectileKind::Discus && p.alive)
    }

    /// (party P3) Whether member `id`'s weapon is out of his hand: Discus thrown (ThrowWeapon) until the catch clip's
    /// TresAnimNotify_SetWeaponVisible (M_1329 / M_1332 at 0.13 s), the game's weapon hide / show around the catch.
    pub fn weapon_hidden(&self, id: MemberId) -> bool {
        let Some(m) = self.member(id) else { return false };
        // (ZETTA 2026-10-23) Donald's staff on its way back to his hand after Zettaflare's down (zettaflare::StaffAway)
        if m.don_zetta.staff.is_some_and(|s| s.ground.is_none()) {
            return true;
        }
        let Some(run) = m.action.filter(|r| r.def == Some(crate::ActionDef::GoofyDiscus) && m.state == crate::NpcStateId::Action) else { return false };
        if !run.shot {
            return false;
        }
        if run.sub < 4 {
            return true;
        }
        let show = m.anim.clip.notifies.iter().find(|n| n.class == "TresAnimNotify_SetWeaponVisible").map(|n| n.start);
        show.is_some_and(|t| m.anim.time < t)
    }

    fn step_projectiles(&mut self, dt: f32, host: &mut PartyHost, out: &mut PartyOutput) {
        let mut shapes: Vec<HurtShape> = Vec::new();
        let mut own: Vec<crate::projectile::HitShape> = Vec::new();
        let mut list = Vec::new();
        let mut i = 0;
        while i < self.projectiles.len() {
            let mut p = self.projectiles[i];
            let owner_pos = self.member(p.owner).map(|m| m.capsule_centre()).unwrap_or(p.position);
            let aim = host.targets.info(p.target).map(|t| t.center);
            let prev = p.position;
            p.advance(dt, aim, owner_pos);
            if p.kind == ProjectileKind::Discus {
                // the shield is the weapon: the catch clip of the definition (sub-step 4, 0x1411C7670) or the
                // definition's end takes it back (FUN_1405BCAF0, the weapon in hand again)
                let back = self.member(p.owner).map_or(true, |m| !(m.state == crate::NpcStateId::Action && m.action.is_some_and(|r| r.def == Some(crate::ActionDef::GoofyDiscus) && r.sub < 4)));
                // (party P3) a pad-driven Goofy's targeted shield whose target is gone in flight returns (free throw)
                let pad_owned = self.member(p.owner).is_some_and(|m| m.pad.is_some());
                if !back && pad_owned && p.free.is_none() && !p.caught && aim.is_none() {
                    p.free = Some(0.0);
                    p.returning = true;
                    p.homing_on = false;
                    if let Some(m) = self.member_mut(p.owner) {
                        crate::actions::discus_make_free(m);
                    }
                }
                if back {
                    p.alive = false;
                } else if p.free.is_some() && p.returning && p.position.distance(owner_pos) <= tables::param("proj.discus.HomingMinDist").max(1.0) {
                    // (party P3) the free throw is back: he catches it (the definition's catch, as on a wall hit)
                    p.velocity = Vec3::ZERO;
                    self.discus_hit(p.owner, true);
                } else if p.free.is_some() && !p.returning && p.speed > 0.0 {
                    // (party P3) the free throw's out leg ends on the map: it turns back
                    let r = tables::param("proj.discus.radius");
                    if let Some(h) = host.world.sweep_capsule(prev, p.position, r, r).filter(|h| !h.start_penetrating) {
                        p.position = prev + (p.position - prev) * h.time;
                        p.returning = true;
                    }
                } else if p.free.is_some() {
                    // returning: it flies back to him over whatever is in the way (it is his shield coming home)
                } else if p.speed > 0.0 {
                    // Attack_Discus is HITMAP: a map hit goes to the same handler 0x1411CC550 with its normal; the
                    // shield stops on it [inf: the projectile movement's blocking-hit stop].
                    let r = tables::param("proj.discus.radius");
                    if let Some(h) = host.world.sweep_capsule(prev, p.position, r, r).filter(|h| !h.start_penetrating) {
                        p.position = prev + (p.position - prev) * h.time;
                        p.velocity = Vec3::ZERO;
                        p.speed = 0.0;
                        p.homing_on = false;
                        // FUN_14126C8E0(normal, 45): a wall = the normal more than 45 deg from up
                        let n = h.impact_normal;
                        let up_angle = 90.0 - n.z.atan2(Vec3::new(n.x, n.y, 0.0).length()).to_degrees();
                        self.discus_hit(p.owner, up_angle > 45.0);
                    }
                }
            }
            if p.alive && p.hitting() {
                p.shapes(&mut own);
                list.clear();
                host.targets.enumerate(&mut list);
                for info in &list {
                    if !info.flags.has(TargetFlags::HITTABLE) || p.already_hit(info.id) {
                        continue;
                    }
                    shapes.clear();
                    host.targets.hurt_shapes(info.id, &mut shapes);
                    let hit_shape = shapes.iter().find(|s| own.iter().any(|o| crate::actions::seg_seg_dist(o.a, o.b, s.a, s.b) <= s.radius + o.radius));
                    let Some(shape) = hit_shape.copied() else { continue };
                    p.mark_hit(info.id);
                    let Some(m) = self.member(p.owner) else { continue };
                    let Some(row) = tables::attack_row(m.chara, p.kind.attack()) else { continue };
                    let defender = Defender::of_target(host.targets.defense(info.id).as_ref());
                    let mut hit = HitRecord::empty(info.id);
                    hit.part = shape.part;
                    hit.attack_name = row.name;
                    hit.position = p.position;
                    hit.direction = p.velocity.normalize_or_zero();
                    if hit.direction == Vec3::ZERO {
                        hit.direction = (info.center - owner_pos).normalize_or_zero();
                    }
                    hit.damage = friend_damage(&m.status, row, &defender);
                    if p.kind == ProjectileKind::Discus {
                        hit.damage *= crate::pad::goofy::discus_scale(m); // (party X3G) the ricochets / Discus Barrage bounces
                    }
                    hit.damage_kind = ETresDamageKind::from_game_name(row.damage_kind).unwrap_or(ETresDamageKind::Small);
                    hit.attribute = ETresDamageAttribute::from_game_name(row.attribute).unwrap_or(ETresDamageAttribute::Physical);
                    hit.attacker = HitAttacker { position: owner_pos, yaw: m.yaw, radius: m.params.capsule_radius, valid: true };
                    hit.info = HitInfo { power: row.power, power_scale: row.power_scale, parabola_angle: row.parabola_angle, effect_time: row.effect_time, ..HitInfo::DEFAULT };
                    if p.kind != ProjectileKind::Discus {
                        hit.info.flags50 |= HitInfo::FLAG50_MAGIC;
                    }
                    let res = host.targets.apply_hit(&hit);
                    if res.accepted {
                        out.events.push(PartyEvent { member: p.owner, kind: PartyEventKind::Hit { target: info.id, damage: res.damage_dealt, killed: res.killed, attack: row.name } });
                    }
                    // (party P3) the spell actor's pawn-hit effect (its TresAtkCollComponent m_HitEffects) at the hit
                    if let (Some(effect), Some(m)) = (p.kind.hit_effect(), self.member_mut(p.owner)) {
                        m.seam.spawn(host.effects, &kh_core::host::EffectSpawn { id: kh_core::host::EffectId(kh_core::notify::notify_asset_id(effect)), name: effect, at: kh_core::host::Attach::World(p.position), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
                    }
                    // (ER party S5) and its pawn-hit SE (m_PawnHitSEAsset, contract E.5: SoundPlay { id: ACTOR_PAWN_HIT, name: class }; the
                    // hosts play the class's SE only: its hit effect's own sounds come with the effect above)
                    if let (Some(class), Some(m)) = (p.kind.class(), self.member_mut(p.owner)) {
                        m.seam.play(host.audio, &kh_core::host::SoundPlay { id: kh_core::host::audio::ACTOR_PAWN_HIT, name: class, at: kh_core::host::Attach::World(p.position), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
                    }
                    // (party X3G) Goofy's ricochets / Discus Barrage: the shield is sent on to the next target
                    let bounced = p.kind == ProjectileKind::Discus && res.accepted && self.member_mut(p.owner).is_some_and(|m| crate::pad::goofy::discus_bounce(m, &mut p, info.id, host));
                    if bounced {
                    } else if p.kind == ProjectileKind::Discus && p.free.is_some() {
                        // (party P3) the free throw's first hit turns it back to him (it keeps hitting on its way)
                        p.returning = true;
                    } else if p.kind == ProjectileKind::Discus {
                        // a pawn hit (0x1411CC550): the first one turns the shield's homing off (MyMovement
                        // vt+0x580 = SetHoming(false)) and starts the fade
                        if !p.caught {
                            p.caught = true;
                            p.homing_on = false;
                        }
                        self.discus_hit(p.owner, false);
                    }
                    // Fire / Blizzard end on their first hit (m_RespawnCheckType HIT_ANY / HIT_PAWN; m_bNoDestroyOnHit off).
                    if p.ends_on_hit() {
                        p.alive = false;
                        break;
                    }
                }
            }
            if !p.alive {
                // (party P3) the actor's body effect runs out where it ended (fading as authored)
                if p.effect != 0 {
                    host.effects.stop(kh_core::host::EffectHandle(p.effect), false);
                }
                out.events.push(PartyEvent { member: p.owner, kind: PartyEventKind::ProjectileEnded { kind: p.kind.name(), at: p.position } });
                self.projectiles.remove(i);
            } else {
                self.projectiles[i] = p;
                i += 1;
            }
        }
    }
}

/// Stream D's view of the party (`team::TeamParty`, contract D.3), implemented by B.
impl crate::team::TeamParty for Party {
    fn team_members(&self, out: &mut Vec<crate::team::TeamMember>) {
        use kh_core::enums::ETresAbilityKind as K;
        for m in self.members.iter().flatten() {
            out.push(crate::team::TeamMember {
                id: m.id,
                chara: m.chara,
                position: m.position(),
                yaw: m.yaw,
                on_ground: m.on_ground(),
                ko: m.state == crate::NpcStateId::Die,
                state: m.state,
                in_battle: m.in_battle,
                combi_time: m.status.abilities.equipped(K::CombiTime),
                combi_up: m.status.abilities.equipped(K::CombiUp),
                clip: m.anim.clip.name,
                clip_time: m.anim.time,
                clip_ended: m.anim.finished,
            });
        }
    }

    fn team_command(&mut self, id: MemberId, cmd: MemberCommand) -> CommandTicket {
        self.command(id, cmd)
    }

    /// (B4, D3 relay D.6) The member's own attack row by name (`n_ex00x_AttackDataTable`, e.g. Goofy's
    /// FR_GOOFYSHOOT_RUSH) delivered by `actions::deliver` as his attack; false for an unknown row / member.
    fn fr_hit(&mut self, id: MemberId, row: &'static str, target: kh_core::host::TargetId, at: Vec3, host: &mut crate::host::PartyHost) -> bool {
        let Some(m) = self.member_mut(id) else { return false };
        let Some(r) = crate::tables::attack_row(m.chara, row) else { return false };
        let sora = crate::view::SoraView::default();
        let cx = crate::pawn::StepCtx { sora: &sora, members: &[], dt: 0.0, ice_rails: &[], zones: &[] };
        crate::actions::deliver(m, r, target, 0, at, &cx, host)
    }

    fn fr_drive(&mut self, id: MemberId, drive: crate::team::FrDrive) {
        if let Some(m) = self.member_mut(id) {
            m.fr_drive = Some(drive);
        }
    }
}

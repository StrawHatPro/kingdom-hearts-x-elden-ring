//! (ER party S2) THE LEADER BODY: Elden Ring's hidden player body follows whoever leads (Sora, Donald or Goofy).
//!
//! Plan: build/sora_tmp/er/party/ER_PARTY_PLAN.md S2; research ER_PARTY_RESEARCH.md part 2; contract docs/party-contracts.md
//! ER.2 (`LeaderView`, `LeaderSink`). State note: build/sora_tmp/er/party/IMPL_S2_STATE.md.
//!
//! THE SEAM. `SoraHost` (sora.rs) keeps one [`LeaderBody`] (`SoraHost::lead`). The party host (S1, party/glue.rs) feeds it
//! every party step ([`LeaderBody::update`]: the runtime's `leader_view_of`, Sora's dodge frames ORed into his
//! `invulnerable` as S0 asked) and carries out what the body code ordered ([`LeaderBody::apply`]: kh-partyhost's
//! `RuntimeLeaderSink`). The body code reads it through `SoraHost::friend_lead`:
//! - SORA LEADS (or the party is off, or the feature is not compiled): `friend_lead` is None and every body rule runs on
//!   Sora's own `Output` exactly as before (`write`, the follow rule, the net, water, grabs, damage-in, the fall guard,
//!   prompts, the camera's seat). The only party addition then: the seat's rebase of the KH3 frame (a cutscene, an
//!   action, a grab moves the frame under Sora) is handed to the allies as a counter-move ([`LeaderBody::seated`]), so
//!   that the frame's move does not drag Donald and Goofy with him.
//! - A FRIEND LEADS: the body is written to HIS feet and yaw; the body's HP losses are hits on him (share of HIS maximum,
//!   the same status classes: `guards::friend_hit`), its deaths (deep water at his height, the net's second fall) his KO;
//!   when the body leads (a cutscene, a grab, an action) he is SEATED ON IT (`LeadOrder::Follow`, held every party step;
//!   the KH3 frame is not moved, so Sora and the other friend stay where they are), and his figure is the body's pose
//!   retargeted onto HIS skeleton ([`PoseOver`]: figure.rs rows he has, his hands with his own limb lengths, limbs.rs) for
//!   whoever draws him (S4: [`LeaderBody::overlay`]).
//! - (LADDER2) THE LADDER JUMP of a friend leader: `LeadOrder::LadderJump` (kh-party's ladder jump started on his pawn).

use std::path::PathBuf;
use std::sync::Arc;

use glam::{Quat, Vec3};
use kh_anim::retarget::Retarget;
use kh_anim::{Skeleton, Trs};
use kh_core::{Output, Sora, Status};
use kh_host::party::PartyChara;
use kh_partyhost::{LeaderHit, LeaderSink, LeaderView, PartyRuntime, PosedChar, RuntimeLeaderSink};

use crate::guards::{FriendLead, LeadOrder};

/// (S0's note) SORA'S DODGE FRAMES (the reaction types 6 - 9 and 11 of his actions: invincible / ghost): the ER host's
/// rule, ORed into his `LeaderView.invulnerable` / an AI Sora body's `invulnerable` (sora.rs `after_steps` uses the same).
pub fn sora_dodge_frames(reaction: u8) -> bool {
    matches!(reaction, 6 | 7 | 8 | 9 | 11)
}

/// (U5, the plan's default) WHO CARRIES THE HERO LIGHT (the Keyblade sparkle in the dark): Sora while he is in the party
/// (AI Sora draws it when a friend leads), otherwise the leader's weapon tip (S4 draws it there).
pub fn light_carrier(sora_in_party: bool, leader: PartyChara) -> PartyChara {
    if sora_in_party {
        PartyChara::Sora
    } else {
        leader
    }
}

/// The body rules' view of a friend leader (None for Sora: the old paths).
pub fn friend_lead_of(view: &LeaderView, hp: (i32, i32)) -> Option<FriendLead> {
    if view.chara == PartyChara::Sora || view.member.is_none() {
        return None;
    }
    Some(FriendLead { position: view.position, yaw: view.yaw, velocity: view.velocity, on_floor: view.on_floor, capsule: view.capsule, alive: !view.ko, hp: hp.0, max_hp: hp.1.max(1), share_max: hp.1.max(1) as f32 /* (GOOFY-BAL) set by friend_lead */, invulnerable: view.invulnerable })
}

/// THE BODY'S POSE ON A PARTY MEMBER'S SKELETON: the body's (or the cutscene figure's) rows retargeted, his hands (and on
/// he planted them his feet) brought to the body's as far as his limbs reach.
#[derive(Clone, Debug)]
pub struct PoseOver {
    pub chara: PartyChara,
    pub retarget: Retarget,
    /// The `ROWS` the retarget wants (indices into `source`).
    pub keep: Vec<usize>,
    /// Every `ROWS` row's bone in the KH3 world.
    pub source: Vec<(Vec3, Quat)>,
    /// Where his pelvis goes.
    pub pelvis: Vec3,
    /// 0..1 (eased in and out by the body code).
    pub weight: f32,
    /// The component's place (the cutscene figure's); None = where the drawing puts him.
    pub place: Option<Trs>,
    /// The limbs brought to the body's: 2 = the hands, 0 = none.
    pub limbs: u8,
    pub reach: f32,
    /// His weapon is not drawn meanwhile (hands on a chest, an item).
    pub hide_weapon: bool,
    /// (ANIMS) The fit at his proportions (propfit.rs): the pelvis is its, the limbs go to its places, his posture is kept
    /// by its share. None: the rule before (`own.fit = 0`, no measures).
    pub plan: Option<crate::propfit::Plan>,
}

impl PoseOver {
    /// Lays the pose over a drawn character (`posed` from kh-partyhost's poser, skeleton = his). False when it does not fit.
    pub fn apply(&self, skeleton: &Skeleton, posed: &mut PosedChar) -> bool {
        if posed.local.len() != skeleton.len() || self.weight <= 0.0 || self.source.len() != crate::figure::ROWS.len() {
            return false;
        }
        let to_world = self.place.unwrap_or(posed.component_to_world);
        let rows: Vec<(Vec3, Quat)> = self.keep.iter().filter_map(|k| self.source.get(*k).copied()).collect();
        if rows.len() != self.retarget.rows() {
            return false;
        }
        if let Some(plan) = self.plan.as_ref() {
            // (ANIMS) at his proportions: the plan's pelvis, his posture kept by its share, each planned limb to its place
            let own = posed.local.clone();
            crate::propfit::apply(&self.retarget, skeleton, &rows, plan, &to_world, self.weight, self.reach, &mut posed.local);
            // (ANIMS) THE DRAWN POSE'S LIMITS (propfit.rs `limits`): a head tucked, a spine folded, a knee bent backwards or
            // shut, a hand over his head: his own pose is drawn instead
            {
                let (before, after) = (kh_anim::pose::component_space(skeleton, &own), kh_anim::pose::component_space(skeleton, &posed.local));
                if let Err(why) = crate::propfit::limits(skeleton, &before, &after) {
                    static TOLD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                    if TOLD.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 600 == 0 {
                        crate::erlog!("!!! own: the borrowed pose on {} breaks a limit ({why}): his own pose is drawn", self.chara.name());
                    }
                    posed.local = own;
                    posed.component = before;
                    posed.component_to_world = to_world;
                    return false;
                }
            }
        } else {
            self.retarget.apply(skeleton, &rows, self.pelvis, &to_world, self.weight, &mut posed.local);
        }
        if self.plan.is_none() && self.reach > 0.0 && self.limbs > 0 {
            let all = [crate::figure::ARMS[0], crate::figure::ARMS[1], crate::figure::LEGS[0], crate::figure::LEGS[1]];
            for (k, rows) in all.iter().take(self.limbs.min(4) as usize).enumerate() {
                let to = to_world.rotation.conjugate() * (self.source[rows[2]].0 - to_world.translation);
                crate::limbs::reach_sora(skeleton, &mut posed.local, k, to, self.weight * self.reach);
            }
        }
        posed.component = kh_anim::pose::component_space(skeleton, &posed.local);
        posed.component_to_world = to_world;
        if self.hide_weapon && self.weight > 0.5 {
            if let Some(w) = posed.weapon.as_mut() {
                w.2 = false;
            }
        }
        true
    }
}

/// (ANIMS) A member's own walk (`WalkRunN` blend space at `speed` cm/s, at least its walk) as a local pose, its phase by
/// `distance` walked over the heaviest sample's stride (its length at the row's speed): no foot slides at any speed.
pub fn walk_local(m: &kh_partyhost::poser::PosedMember, distance: f32, speed: f32) -> Option<Vec<Trs>> {
    let v = speed.clamp(140.0, 680.0);
    let samples = kh_partyhost::poser::row_samples(m, "WalkRunN", 0.0, v).ok()?;
    let lengths: Vec<f32> = samples.iter().map(|(s, _)| m.animator.clip_info(s).map_or(1.0, |i| i.length.max(0.05))).collect();
    let phase = (distance.max(0.0) / (v * lengths.first().copied()?)).fract();
    let mut acc: Option<Vec<Trs>> = None;
    let mut total = 0.0f32;
    for ((s, w), len) in samples.iter().zip(&lengths) {
        if *w <= 0.0 {
            continue;
        }
        let p = m.animator.sample(s, phase * len);
        let f = w / (total + w);
        acc = Some(match acc {
            None => p,
            Some(a) => a.iter().zip(&p).map(|(x, y)| Trs { translation: x.translation.lerp(y.translation, f), rotation: x.rotation.slerp(y.rotation, f).normalize(), scale: x.scale.lerp(y.scale, f) }).collect(),
        });
        total += w;
    }
    acc
}

/// (ANIMS) `pose` laid over `local` by `weight`, the root and the face (the subtree of `facial`) left as they are.
pub fn blend_body(skeleton: &Skeleton, pose: &[Trs], weight: f32, local: &mut [Trs]) {
    let face = skeleton.index_of("facial");
    let under_face = |mut b: usize| loop {
        if Some(b) == face {
            return true;
        }
        match skeleton.bones[b].parent {
            Some(p) => b = p,
            None => return false,
        }
    };
    for i in 1..local.len().min(pose.len()) {
        if under_face(i) {
            continue;
        }
        let (a, b) = (local[i], pose[i]);
        local[i] = Trs { translation: a.translation.lerp(b.translation, weight), rotation: a.rotation.slerp(b.rotation, weight).normalize(), scale: a.scale.lerp(b.scale, weight) };
    }
}

/// The leader-body state (sora.rs `SoraHost::lead`).
#[derive(Default)]
pub struct LeaderBody {
    view: Option<LeaderView>,
    /// The friend leader's HP and maximum (his `MemberView`).
    friend_hp: (i32, i32),
    /// (GOOFY-BAL) the HP an Elden Ring hit's share is taken of for the friend leader (0 = his max HP).
    friend_share: f32,
    orders: Vec<LeadOrder>,
    /// The seat while the body leads a friend (re-applied every party step).
    hold: Option<(Vec3, f32)>,
    /// KH3 cm to add to every member but the leader: the seat's frame rebase while Sora leads.
    seat: Vec3,
    /// (a lift) Per member id: what cancels `seat` for those riding the moving body with Sora.
    riding: Vec<(u8, Vec3)>,
    /// The body's pose for the friend leader this frame (None: his own).
    pub over: Option<PoseOver>,
    /// The ease of `over` (0..1), as Sora's `own_weight`.
    pub over_weight: f32,
    /// The friends' skeletons (the pack's manifests), read once.
    skeletons: Vec<(PartyChara, Arc<Skeleton>)>,
    pack: Option<PathBuf>,
    /// The body's rig for the friend leader's skeleton (FLVER, chara).
    pub(crate) rig: Option<(usize, PartyChara, crate::figure::Rig)>,
    pub(crate) rig_refused: Option<(usize, PartyChara)>,
    /// A friend leader's KO was handed to the body (the log's edge).
    pub ko_seen: bool,
    pub orders_given: u64,
    /// A real-time cutscene whose cut shows no figure of the player: the leader is not drawn (U6). For S4.
    pub hidden: bool,
    /// The cutscene figure is shown on the friend leader (sora.rs `cut_frame`).
    pub cut_on: bool,
    /// (U5) Who carries the hero light this step (`light_carrier`; None = not decided yet: Sora). For S4: the leader's
    /// weapon tip sparkles while `keyblade_wanted()` and this is not Sora.
    pub light: Option<PartyChara>,
    /// (ANIMS) The friend leader's fit of the borrowed actions (propfit.rs).
    pub fit: crate::propfit::Fit,
    /// (ANIMS) The friend leader walked through a borrowed action with his own walk: the state, and what to draw (who,
    /// distance walked, speed cm/s, weight).
    pub walk: crate::propfit::WalkThrough,
    pub walk_draw: Option<(PartyChara, f32, f32, f32)>,
}

impl LeaderBody {
    /// Where the party pack is (`PACK/party`): the friends' skeletons are read from it.
    pub fn set_pack(&mut self, pack: &std::path::Path) {
        self.pack = Some(pack.join("party"));
    }

    // ------------------------------------------------------------------------------------- S1's side

    /// EVERY PARTY STEP (S1, after `rt.step`): the controlled pawn as the body must follow it. `dodge`: Sora's dodge frames
    /// (`sora_dodge_frames(sora.actions().reaction)`) are ORed into his `invulnerable` when he leads (S0's note).
    pub fn update(&mut self, rt: &PartyRuntime, out: &Output, status: &Status, dodge: bool) {
        let mut view = rt.leader_view_of(out, status);
        if view.member.is_none() {
            view.invulnerable |= dodge;
            self.friend_hp = (status.hp, status.max_hp());
            self.friend_share = 0.0; // (GOOFY-BAL)
        } else if let Some(v) = view.member.and_then(|id| rt.party.view(id)) {
            self.friend_hp = (v.hp, v.hp_max);
            // (GOOFY-BAL) balanced like Sora: a hit's share of SORA'S max HP x his defence factor (kh-party balance.rs)
            self.friend_share = view.member.and_then(|id| rt.party.incoming_reference(id, status.max_hp())).unwrap_or(0.0);
        }
        self.view = Some(view);
    }

    /// The party is off (`party.er = 0`, no runtime): the body is Sora's alone again.
    pub fn clear(&mut self) {
        if self.view.take().is_some() {
            crate::erlog!("party: the leader body is SORA'S again (the party host is off)");
        }
        self.orders.clear();
        self.hold = None;
        self.seat = Vec3::ZERO;
        self.over = None;
        self.over_weight = 0.0;
    }

    /// (ER party I, X3G's request) A HIT ON THE FRIEND LEADER, AT ONCE (sora.rs `damage_in`): capped by his defender's
    /// `damage_max` (S2's rule), handed to kh-party's `receive_hit` (his kit answers first: Goofy's shield guard / Perfect
    /// Guard, Donald's Magic Block, their invincible moves). The result tells the body code what became of it: `guarded`
    /// (0 HP, his guard reaction, not a hit reaction) or not `accepted` (it did not land). None = no friend leads.
    pub fn hit_now(rt: &mut PartyRuntime, mut record: kh_core::HitRecord) -> Option<kh_core::host::HitResult> {
        let id = rt.leader?;
        let cap = rt.party.defender(id).map_or(0, |d| d.damage_max);
        record.damage = crate::guards::capped(record.damage, cap);
        let result = rt.party.receive_hit(id, &record);
        // (FINAL4) a hit his guard took (or that did not land) bleeds nothing
        if result.guarded || !result.accepted {
            crate::blood::note_guarded(rt.leader_chara());
        }
        let what =if result.guarded { " GUARDED: 0 HP, his guard's reaction (no hit reaction)".to_string() } else if !result.accepted { " NOT TAKEN (invincible)".to_string() } else { format!(": {} HP left{}", result.hp_after, if result.killed { " (KO)" } else { "" }) };
        crate::erlog!("party: the leader's body hit -> {} for {:.0} ({}){what}", rt.leader_chara().name(), record.damage, record.attack_name);
        Some(result)
    }

    /// EVERY PARTY STEP (S1, right after `rt.step` and `update`): what the body code ordered for a friend leader (hits,
    /// the seat, a death) through kh-partyhost's `RuntimeLeaderSink`, the seat held, and the frame's counter-move to the
    /// members who do not lead. Returns that counter-move (KH3 cm) for the host's own KH3 data (S1's breadcrumb trail).
    pub fn apply(&mut self, rt: &mut PartyRuntime, sora: &mut Sora) -> Vec3 {
        let orders = std::mem::take(&mut self.orders);
        let leader = rt.leader;
        for order in orders {
            self.orders_given += 1;
            match order {
                LeadOrder::Hit(record) => {
                    // (ER party I) the same rule as `hit_now` (one place)
                    let _ = Self::hit_now(rt, record);
                }
                LeadOrder::Follow(at, yaw) => self.hold = Some((at, yaw)),
                LeadOrder::Release => self.hold = None,
                LeadOrder::LadderJump { to, facing } => {
                    // (LADDER2) his leap up the ladder (kh-party ladder.rs; the pad moves nothing meanwhile)
                    self.hold = None;
                    let jump = kh_party::LadderJump { to, facing, delay: 0.0 };
                    let leaped = match leader.and_then(|id| rt.party.member_mut(id)) {
                        Some(m) => {
                            kh_party::ladder::begin(m, jump);
                            true
                        }
                        None => false,
                    };
                    crate::erlog!("ladder: {} LEAPS UP THE LADDER to ({:.0}, {:.0}, {:.0}) facing {:.0}{}", rt.leader_chara().name(), jump.to.x, jump.to.y, jump.to.z, jump.facing, if leaped { "" } else { ": NOT STARTED (no leader pawn)" });
                }
                LeadOrder::Nudge(by) => {
                    if let Some(id) = leader {
                        if let Some(m) = rt.party.member_mut(id) {
                            m.cmc.location += by;
                        }
                    }
                }
                LeadOrder::Die => {
                    RuntimeLeaderSink { runtime: rt, sora }.die();
                    crate::erlog!("party: the leader's body died: {} is knocked out", rt.leader_chara().name());
                }
            }
        }
        if leader.is_none() {
            self.hold = None;
        }
        if let Some((at, yaw)) = self.hold {
            RuntimeLeaderSink { runtime: rt, sora }.follow_body(at, yaw);
        }
        let seat = std::mem::take(&mut self.seat);
        let riding = std::mem::take(&mut self.riding);
        if seat != Vec3::ZERO || !riding.is_empty() {
            let members: Vec<_> = rt.members.iter().map(|(_, id)| *id).filter(|id| Some(*id) != leader).collect();
            for id in members {
                // (a lift: the members on it ride it with Sora, their counter-move cancelled)
                let delta = seat + riding.iter().filter(|(m, _)| *m == id.0).map(|(_, d)| *d).sum::<Vec3>();
                if delta == Vec3::ZERO {
                    continue;
                }
                if let Some(m) = rt.party.member_mut(id) {
                    m.cmc.location += delta;
                    // (LADDER2) a ladder jump of his own and a waypoint are places in the world too
                    m.ladder.from += delta;
                    m.ladder.to += delta;
                    if let Some((goal, via)) = m.steer.as_mut() {
                        *goal += delta;
                        *via += delta;
                    }
                }
            }
        }
        seat
    }

    /// A hit for the leader as S0's `LeaderHit` (for hosts that route through `LeaderSink` themselves).
    pub fn take_hits(&mut self) -> Vec<LeaderHit> {
        let mut out = Vec::new();
        self.orders.retain(|o| match o {
            LeadOrder::Hit(record) => {
                out.push(LeaderHit { sora: None, record: *record });
                false
            }
            _ => true,
        });
        out
    }

    /// The controlled pawn as last given (None: the party is off).
    pub fn view(&self) -> Option<&LeaderView> {
        self.view.as_ref()
    }

    /// FOR WHOEVER DRAWS THE LEADER (S4 / S1): the body's pose laid over his poser's pose this frame (a ladder, a grab, a
    /// cutscene, a chest, an item). False = draw his own.
    /// (ANIMS) FOR WHOEVER DRAWS THE LEADER: while the game walks the friend leader through a borrowed action (the fog, a
    /// door's steps: `walk_draw`), HIS OWN KH3 WALK (`WalkRunN` at the move's speed, its phase by the distance walked, its own
    /// stride), upright, his weapon in his hand, eased over his pose; his face stays his. False: not walking now.
    pub fn walk_pose(&self, chara: PartyChara, member: Option<&kh_partyhost::poser::PosedMember>, posed: &mut PosedChar) -> bool {
        let Some((who, distance, speed, weight)) = self.walk_draw else { return false };
        let Some(m) = member.filter(|_| who == chara && weight > 0.0) else { return false };
        match walk_local(m, distance, speed) {
            Some(walk) if walk.len() == posed.local.len() => {
                let skeleton = m.animator.skeleton();
                blend_body(skeleton, &walk, weight.min(1.0), &mut posed.local);
                posed.component = kh_anim::pose::component_space(skeleton, &posed.local);
                if let (Some(w), Some(mount)) = (posed.weapon.as_mut(), m.weapon.as_ref()) {
                    if let Some(c) = posed.component.get(mount.socket.bone) {
                        w.1 = mount.socket.local.then(c);
                        w.2 = w.2 || weight > 0.5;
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub fn overlay(&mut self, chara: PartyChara, posed: &mut PosedChar) -> bool {
        let Some(over) = self.over.as_ref().filter(|o| o.chara == chara) else { return false };
        let over = over.clone();
        let Some(skeleton) = self.skeleton(chara) else { return false };
        over.apply(&skeleton, posed)
    }

    // ------------------------------------------------------------------------------------- the body code's side (sora.rs)

    /// A friend leads (None: Sora does, or the party is off).
    pub fn friend_lead(&self) -> Option<FriendLead> {
        self.view.as_ref().and_then(|v| friend_lead_of(v, self.friend_hp)).map(|mut f| {
            if self.friend_share > 0.0 {
                f.share_max = self.friend_share; // (GOOFY-BAL)
            }
            f
        })
    }

    /// The leader's chara (Sora when the party is off).
    pub fn chara(&self) -> PartyChara {
        self.view.map_or(PartyChara::Sora, |v| v.chara)
    }

    /// The party host runs (S1 feeds `update`).
    pub fn active(&self) -> bool {
        self.view.is_some()
    }

    pub fn order(&mut self, order: LeadOrder) {
        if let LeadOrder::Follow(at, yaw) = order {
            // (the seat is held every step: only the latest counts)
            self.orders.retain(|o| !matches!(o, LeadOrder::Follow(..) | LeadOrder::Release));
            self.hold = Some((at, yaw));
        }
        if order == LeadOrder::Release {
            self.hold = None;
            self.orders.retain(|o| !matches!(o, LeadOrder::Follow(..)));
        }
        self.orders.push(order);
    }

    /// The body leads no more (sora.rs, every frame without a follow): the seat's hold is let go (once).
    pub fn release(&mut self) {
        if self.hold.is_some() || self.orders.iter().any(|o| matches!(o, LeadOrder::Follow(..))) {
            self.order(LeadOrder::Release);
        }
    }

    /// FOR S4: the leader is not drawn now (a cutscene's cut without a figure of the player).
    pub fn leader_hidden(&self) -> bool {
        self.hidden
    }

    /// The seat moved the KH3 frame under Sora by `shift_er` (Elden Ring m): the members who do not lead stay where they
    /// are in the world (only while the party runs and Sora leads: a friend's seat never moves the frame).
    pub fn seated(&mut self, shift_er: Vec3) {
        if self.view.is_some_and(|v| v.member.is_none()) {
            self.seat -= crate::coords::dir_er_to_kh(shift_er);
        }
    }

    /// (a lift, Sora leading) As `seated`, except for the members `riders` (MemberId numbers) who stand on the same moving
    /// body: they ride it with him.
    pub fn seated_riding(&mut self, shift_er: Vec3, riders: &[u8]) {
        if self.view.is_some_and(|v| v.member.is_none()) {
            let kh = crate::coords::dir_er_to_kh(shift_er);
            self.seat -= kh;
            for r in riders {
                self.riding.push((*r, kh));
            }
        }
    }

    /// A friend's skeleton (Sora's comes from his animator: not here).
    pub fn skeleton(&mut self, chara: PartyChara) -> Option<Arc<Skeleton>> {
        if let Some((_, s)) = self.skeletons.iter().find(|(c, _)| *c == chara) {
            return Some(s.clone());
        }
        let dir = match chara {
            PartyChara::Donald => "n_ex001",
            PartyChara::Goofy => "n_ex002",
            PartyChara::Sora => return None,
        };
        let path = self.pack.as_ref()?.join(dir).join("manifest.json");
        let read = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| kh_anim::json::Json::parse(&t)).and_then(|j| Skeleton::from_manifest(&j));
        match read {
            Ok(s) => {
                crate::erlog!("party: {}'s skeleton read for the leader body ({} bones, {})", chara.name(), s.len(), path.display());
                let s = Arc::new(s);
                self.skeletons.push((chara, s.clone()));
                Some(s)
            }
            Err(e) => {
                crate::erlog!("party: {}'s skeleton NOT read ({}: {e}): the body's pose is not shown on him", chara.name(), path.display());
                // (said once: an empty skeleton marks it)
                self.skeletons.push((chara, Arc::new(Skeleton::default())));
                None
            }
        }
        .filter(|s| !s.bones.is_empty())
    }

    /// Adds a skeleton (tests; a host that has one already).
    pub fn put_skeleton(&mut self, chara: PartyChara, skeleton: Arc<Skeleton>) {
        self.skeletons.retain(|(c, _)| *c != chara);
        self.skeletons.push((chara, skeleton));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::{Pose, SORA_PARAMS};

    fn sora_out(x: f32) -> (Sora, Output) {
        let sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(x, 20.0, 0.15), yaw: 30.0 });
        let out = sora.output();
        (sora, out)
    }

    /// "SORA LEADS = BYTE-IDENTICAL DECISIONS": for every recorded Sora-led view (Sora's own outputs, alive / hit / dodging)
    /// there is no friend lead, so every body rule takes Sora's old path; the dodge frames are ORed into his view.
    #[test]
    fn sora_leading_takes_the_old_paths() {
        for (x, dodge) in [(0.0, false), (150.0, true), (-3000.0, false)] {
            let (sora, out) = sora_out(x);
            let view = LeaderView { chara: PartyChara::Sora, position: out.position, yaw: out.yaw, velocity: out.velocity, on_floor: out.on_floor, capsule: kh_partyhost::SORA_CAPSULE, invulnerable: dodge, in_fr: false, ko: false, airborne: !out.on_floor, state: out.state as i32, pitch: 0.0, member: None };
            assert!(friend_lead_of(&view, (sora.status().hp, sora.status().max_hp())).is_none());
            let mut body = LeaderBody { view: Some(view), ..LeaderBody::default() };
            assert!(body.friend_lead().is_none() && body.active());
            assert_eq!(body.chara(), PartyChara::Sora);
            // Sora's seat counters the frame for the others, a friend's never
            body.seated(Vec3::new(0.0, 1.0, 0.0));
            assert!((body.seat - Vec3::new(0.0, 0.0, -100.0)).length() < 1e-3);
        }
        assert!(LeaderBody::default().friend_lead().is_none(), "the party off: Sora");
        for r in [6u8, 7, 8, 9, 11] {
            assert!(sora_dodge_frames(r));
        }
        for r in [0u8, 1, 2, 5, 10, 12] {
            assert!(!sora_dodge_frames(r));
        }
        assert_eq!(light_carrier(true, PartyChara::Donald), PartyChara::Sora);
        assert_eq!(light_carrier(false, PartyChara::Goofy), PartyChara::Goofy);
    }

    /// A friend leading: his feet, yaw, capsule, HP; KO = not alive; his seat is held, the frame never counter-moved; a
    /// release ends the hold.
    #[test]
    fn a_friend_leads_the_body() {
        let view = LeaderView { chara: PartyChara::Donald, position: Vec3::new(10.0, 20.0, 30.0), yaw: 45.0, velocity: Vec3::ZERO, on_floor: true, capsule: (30.0, 49.0, 51.0), invulnerable: false, in_fr: false, ko: true, airborne: false, state: 0, pitch: 0.0, member: Some(kh_party::MemberId(0)) };
        let f = friend_lead_of(&view, (0, 120)).unwrap();
        assert!(!f.alive && f.position == view.position && f.capsule.2 == 51.0 && f.max_hp == 120);
        let mut body = LeaderBody { view: Some(view), friend_hp: (0, 120), ..LeaderBody::default() };
        assert_eq!(body.chara(), PartyChara::Donald);
        body.seated(Vec3::ONE);
        assert_eq!(body.seat, Vec3::ZERO);
        body.order(LeadOrder::Follow(Vec3::X, 10.0));
        body.order(LeadOrder::Follow(Vec3::Y, 20.0));
        assert_eq!(body.hold, Some((Vec3::Y, 20.0)));
        assert_eq!(body.orders.iter().filter(|o| matches!(o, LeadOrder::Follow(..))).count(), 1);
        body.order(LeadOrder::Hit(kh_core::HitRecord::empty(kh_core::TargetId::NONE)));
        assert_eq!(body.take_hits().len(), 1);
        body.order(LeadOrder::Release);
        assert!(body.orders.iter().all(|o| !matches!(o, LeadOrder::Follow(..))));
    }

    fn views(rt: &mut PartyRuntime) {
        rt.current = rt.members.iter().filter_map(|(_, id)| rt.party.view(*id)).collect();
    }

    /// `apply` with a real runtime: Sora leading, the seat's counter-move moves the friends (not Sora's place in KH3);
    /// Donald leading, the body's hit lands on HIM (his HP), his seat is held where the body is, a release lets him go,
    /// the body's death knocks him out.
    #[test]
    fn apply_carries_out_the_orders() {
        let mut rt = PartyRuntime::new(kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        let (mut sora, out) = sora_out(0.0);
        rt.spawn_one(PartyChara::Donald, Vec3::new(100.0, 0.0, 0.0), 0.0);
        rt.spawn_one(PartyChara::Goofy, Vec3::new(0.0, 100.0, 0.0), 0.0);
        views(&mut rt);
        let mut body = LeaderBody::default();
        body.update(&rt, &out, sora.status(), true);
        assert!(body.friend_lead().is_none() && body.view().unwrap().invulnerable, "Sora leads; his dodge ORed in");
        let before = rt.view(PartyChara::Donald).unwrap().position;
        body.seated(Vec3::new(1.0, 0.0, 0.0)); // the frame moved 1 m along Elden Ring's x = KH3 y
        let seat = body.apply(&mut rt, &mut sora);
        assert!((seat - Vec3::new(0.0, -100.0, 0.0)).length() < 1e-3);
        assert!((rt.view(PartyChara::Donald).unwrap().position - (before + seat)).length() < 1e-3);
        assert_eq!(sora.output().position, out.position, "Sora is not moved in KH3");
        // Donald leads
        rt.leader = rt.id(PartyChara::Donald);
        views(&mut rt);
        body.update(&rt, &out, sora.status(), false);
        let f = body.friend_lead().expect("Donald leads");
        let hp = f.hp;
        body.order(LeadOrder::Hit(crate::guards::friend_hit(&f, Some(kh_core::enums::ETresDamageKind::Small), 5.0, kh_core::TargetId::NONE, f.position - Vec3::X * 100.0, 0.0, 40.0)));
        body.order(LeadOrder::Follow(Vec3::new(500.0, 0.0, 0.0), 90.0));
        body.apply(&mut rt, &mut sora);
        let v = rt.view(PartyChara::Donald).unwrap();
        assert_eq!(v.hp, hp - 5, "the body's hit is his");
        assert!((v.position - Vec3::new(500.0, 0.0, 0.0)).length() < 1e-3 && (v.yaw - 90.0).abs() < 1e-3, "seated: {:?}", v.position);
        body.apply(&mut rt, &mut sora);
        assert!((rt.view(PartyChara::Donald).unwrap().position - Vec3::new(500.0, 0.0, 0.0)).length() < 1e-3, "held");
        body.release();
        body.apply(&mut rt, &mut sora);
        assert!(body.hold.is_none());
        body.order(LeadOrder::Die);
        body.apply(&mut rt, &mut sora);
        assert_eq!(rt.view(PartyChara::Donald).unwrap().hp, 0, "the body's death is his KO");
        assert_eq!(sora.status().hp, sora_out(0.0).0.status().hp, "Sora untouched");
    }

    fn party_skeleton(npc: &str) -> Option<Skeleton> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party").join(npc).join("manifest.json");
        let text = std::fs::read_to_string(path).ok()?;
        Skeleton::from_manifest(&kh_anim::json::Json::parse(&text).ok()?).ok()
    }

    /// THE RIG WITH A PARTY SKELETON (from the pack's manifests): Donald lacks the little fingers, Goofy also `kubi`; the
    /// rows they have make a retarget, Donald's legs shorter than Goofy's.
    #[test]
    fn the_rig_on_the_friends_skeletons() {
        let (Some(donald), Some(goofy)) = (party_skeleton("n_ex001"), party_skeleton("n_ex002")) else {
            eprintln!("no party pack: skipped");
            return;
        };
        let (rows_d, keep_d) = crate::figure::rows_for(&donald);
        let (rows_g, keep_g) = crate::figure::rows_for(&goofy);
        assert_eq!(rows_d.len(), crate::figure::ROWS.len() - 12, "Donald: no yubiE, the fingers B - D two joints");
        assert_eq!(rows_g.len(), crate::figure::ROWS.len() - 6, "Goofy: no yubiE; his neck kubi1");
        assert!(rows_g.iter().any(|r| r.target == "kubi1") && rows_g.iter().find(|r| r.target == "mune").unwrap().aim.map(|a| a.1) == Some("kubi1"));
        assert_eq!(keep_d.len(), rows_d.len());
        let _ = keep_g;
        // a source bind: a c0000-like standing body (the rows' bones placed on a 180 cm figure facing +X, no turn)
        let binds = fake_binds();
        let (rd, kd) = crate::figure::retarget_from_binds(&binds, &donald).expect("Donald's retarget");
        let (rg, _) = crate::figure::retarget_from_binds(&binds, &goofy).expect("Goofy's retarget");
        assert!(rd.ratio < rg.ratio, "Donald's legs are shorter: {} vs {}", rd.ratio, rg.ratio);
        let _ = kd;
    }

    /// A standing c0000-like figure for the rows (KH3 axes, cm): pelvis 100, head 170, arms down, legs to the floor.
    fn fake_binds() -> Vec<(Vec3, Quat)> {
        crate::figure::ROWS
            .iter()
            .map(|r| {
                let s = r.source;
                let side = if s.starts_with("L_") { -1.0 } else if s.starts_with("R_") { 1.0 } else { 0.0 };
                let p = match s.trim_start_matches("L_").trim_start_matches("R_") {
                    "Pelvis" => Vec3::new(0.0, 0.0, 100.0),
                    "Spine" => Vec3::new(0.0, 0.0, 110.0),
                    "Spine1" => Vec3::new(0.0, 0.0, 122.0),
                    "Spine2" => Vec3::new(0.0, 0.0, 135.0),
                    "Neck" => Vec3::new(0.0, 0.0, 150.0),
                    "Head" => Vec3::new(0.0, 0.0, 165.0),
                    "Clavicle" => Vec3::new(0.0, side * 8.0, 145.0),
                    "UpperArm" => Vec3::new(0.0, side * 20.0, 145.0),
                    "Forearm" => Vec3::new(0.0, side * 22.0, 118.0),
                    "Hand" => Vec3::new(0.0, side * 23.0, 92.0),
                    "Thigh" => Vec3::new(0.0, side * 10.0, 95.0),
                    "Calf" => Vec3::new(0.0, side * 11.0, 50.0),
                    "Foot" => Vec3::new(0.0, side * 11.0, 8.0),
                    "Toe0" => Vec3::new(12.0, side * 11.0, 2.0),
                    f => {
                        // fingers: below the hand, a little apart by finger and joint
                        let n: f32 = f.chars().nth(6).and_then(|c| c.to_digit(10)).unwrap_or(0) as f32;
                        let j: f32 = f.chars().nth(7).and_then(|c| c.to_digit(10)).unwrap_or(0) as f32;
                        Vec3::new(n - 2.0, side * 23.0, 88.0 - j * 2.0)
                    }
                };
                (p, Quat::IDENTITY)
            })
            .collect()
    }
}

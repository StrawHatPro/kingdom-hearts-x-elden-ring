//! (MIMIC-ASH, THE USER 2026-11-05: "multi-ashes should work as if you summoned them") THE MIMIC TEAR BOSS'S SPIRIT ASH,
//! WHOLE: every BuddyParam row of the ash's trigger is one spirit, placed, faced, animated and doped as Elden Ring's own
//! summon routine does it, all of them hostile the way the game makes an opponent's spirit hostile in the Colosseum, all
//! dying with the boss. The player's own summon is never touched (the manager, its request, its group, its HUD).
//! UNTESTED IN THE GAME (the user tests it). Static RE: build/sora_tmp/party/SUMMON/IMPL_MIMIC_ASH_RE.md.
//!
//! WHY NOT THE GAME'S OWN ROUTINE FOR AN ENEMY OWNER (2.7.2.0; 2.7.1.0 in the RE note):
//! - the generator (0x4bbeb0) is hard-wired to the main player (WorldChrMan +0x1e508: its spawn point, its key, its
//!   steam id) and to the manager's one request (+0x20 / +0x24 / +0x3c / +0xe4): running it would replace the player's
//!   own summon;
//! - CreateBuddy (0x4baf80) can take another owner (the Colosseum's remote spirits are made through it by 0x4b7d80 with
//!   the opponent as owner), but files the spirit in the group keyed by the owner's player key (0x6576e0 reads
//!   `owner + 0x580`, a PlayerIns field: ChrIns ends at 0x580), and the disappear task (0x4b9390, its loop at
//!   0x4b94b0..0x4b9549) despawns at once every spirit of a group whose key no character of the PLAYER set
//!   (WorldChrMan +0x10EE0, 0x509480) has: a group owned by the boss (an EnemyIns) would be gone the next frame. It also
//!   takes one of the six buddy character slots the player's own summon uses (0x4bb580: slot < 6).
//!
//! So the closest faithful route: the generator's rows, places, facings, arrival animations and level doping; the
//! Colosseum's opponent setup (team from the owner through the game's own two team functions, the hostile marks through
//! the game's own relation test, every frame); the characters made by the party's debug creator (allybody `ask_foe`),
//! one after the other (the creator makes one at a time: the game makes them in one frame).
//!
//! NOT REPRODUCED (each needs a player owner): the buddy AI team that makes spirits follow their owner and share its
//! target (0x4bd4c0 -> 0x37d2c0; the rows' `disablePCTargetShare` / `pcFollowType`): the boss's spirits pick their
//! targets by their own AI and the team relation, as any enemy; the owner's steam id at `chr + 0x218`
//! (`character_creator_steam_id`), the buddy sync flag (`chr + 0x1c9 |= 4`), the wall sweep and ground lift of the
//! placement (0xc3bc30, 0x4b9830: the spirit is made at the computed place, the game's physics settles it), the DLC
//! blessing of the player's buddies (0x4b8f90).
//!
//! API (mimic_boss.rs): `summon_hostile` (reads the rows, places them; nothing is made yet), `poll` every step (makes the
//! next spirit, adopts the new ones, keeps the hostile marks; gives the living spirits), `kill_all` (HP 0: the game's own
//! death and spirit fade), `is_over`. Logs: `mimicash:`.

pub mod rules;

use std::collections::VecDeque;
use std::sync::OnceLock;

use eldenring::cs::{BuddyParam, ChrIns, ChrInsExt, EquipParamGoods, FieldInsHandle, SoloParamRepository, WorldChrMan};
use eldenring::param::BUDDY_PARAM_ST;
use fromsoftware_shared::FromStatic;
use glam::{Quat, Vec3};

use crate::erlog;
use crate::game;
use crate::havok::{locate, Located};
use crate::word;
use rules::{AshRow, Placed};

/// The owner's effective team (`chr + 0x6c`, replaced while SpEffect state 132 or the chr slot overrides it): CreateBuddy
/// reads the owner's through it (0x4bb6ba).
pub(crate) const TEAM_OF_FN: Located = Located {
    name: "effective team of a character",
    pattern: "48 89 5c 24 10 57 48 83 ec 20 0f b6 41 6c 48 8b f9 88 02 48 8b da 48 8b 89 78 01 00 00 48 85 c9 74 1d ba 84 00 00 00 e8 ? ? ? ?",
    capture: 0,
    rva_2_7_1_0: 0x003f_1c90,
    rva_2_7_2_0: 0x003f_1c90,
};

/// The spirit adopts that team unless its own is 5 (CreateBuddy 0x4bb6c6).
pub(crate) const ADOPT_TEAM_FN: Located = Located {
    name: "adopt a team (unless team 5)",
    pattern: "48 89 5c 24 10 57 48 83 ec 20 48 8b d9 48 8b fa 48 8d 4c 24 30 e8 ? ? ? ? 0f b6 00 38 43 6c 74 06 0f b6 07 88 43 6c",
    capture: 0,
    rva_2_7_1_0: 0x003f_7770,
    rva_2_7_2_0: 0x003f_7800,
};

/// The team relation "hostile" (79 x 79 table at 0x3b2b3f0, query word 1): the Update's test for the arena hostile marks
/// (0x4b8d64).
pub(crate) const HOSTILE_FN: Located = Located {
    name: "team relation: hostile",
    pattern: "48 83 ec 28 0f b6 01 45 33 c0 48 6b c8 4f 0f b6 02 48 8d 54 24 30 48 03 c8 66 c7 44 24 30 01 00 48 8d 05 ? ? ? ? c6 44 24 32 00 48 8b 0c c8 48 8b 01 ff 10 0f b6 c0",
    capture: 0,
    rva_2_7_1_0: 0x0051_b5d0,
    rva_2_7_2_0: 0x0051_b870,
};

/// The team relation "friendly" (query word 0x100): the Update's test for the arena friendly marks (0x4b8de0).
pub(crate) const FRIENDLY_FN: Located = Located {
    name: "team relation: friendly",
    pattern: "48 83 ec 28 0f b6 01 45 33 c0 48 6b c8 4f 0f b6 02 48 8d 54 24 30 48 03 c8 66 c7 44 24 30 00 01 48 8d 05 ? ? ? ? c6 44 24 32 00 48 8b 0c c8 48 8b 01 ff 10 0f b6 c0",
    capture: 0,
    rva_2_7_1_0: 0x0051_b740,
    rva_2_7_2_0: 0x0051_b9e0,
};

/// Every address of this file (gamever.rs's offline check runs them against both executables).
#[cfg(test)]
pub(crate) const LOCATED: [Located; 4] = [TEAM_OF_FN, ADOPT_TEAM_FN, HOSTILE_FN, FRIENDLY_FN];

/// A spirit that has not appeared is given up after this long (s).
const GIVE_UP_AFTER: f32 = 5.0;

type TeamOfFn = unsafe extern "C" fn(*mut ChrIns, *mut u8) -> *mut u8;
type AdoptTeamFn = unsafe extern "C" fn(*mut ChrIns, *const u8);
type RelationFn = unsafe extern "C" fn(*const u8, *const u8) -> u8;

#[derive(Clone, Copy)]
struct Fns {
    team_of: Option<usize>,
    adopt: Option<usize>,
    hostile: Option<usize>,
    friendly: Option<usize>,
}

static FNS: OnceLock<Fns> = OnceLock::new();

fn fns() -> Fns {
    *FNS.get_or_init(|| {
        let f = Fns { team_of: locate(&TEAM_OF_FN), adopt: locate(&ADOPT_TEAM_FN), hostile: locate(&HOSTILE_FN), friendly: locate(&FRIENDLY_FN) };
        erlog!("mimicash: the game's team functions (PORT, by pattern): effective team {}, adopt {}, hostile {}, friendly {}", f.team_of.is_some(), f.adopt.is_some(), f.hostile.is_some(), f.friendly.is_some());
        f
    })
}

/// One spirit of the ash in the field.
struct Spirit {
    handle: FieldInsHandle,
    row: AshRow,
}

/// The creator was asked for a spirit (its last character before the ask; seconds waited).
struct Asking {
    placed: Placed,
    before: usize,
    waited: f32,
}

/// THE BOSS'S ASH (a "ticket"): its rows still to make, the one asked, the spirits in the field.
pub(crate) struct Ash {
    what: String,
    owner: FieldInsHandle,
    level: u8,
    queue: VecDeque<Placed>,
    asking: Option<Asking>,
    spirits: Vec<Spirit>,
    ended: bool,
    /// The creator's "busy" was logged for the row waiting now.
    told_wait: bool,
}

impl Ash {
    /// What the log calls it.
    pub(crate) fn what(&self) -> &str {
        &self.what
    }

    /// Nothing left to make and no spirit living.
    pub(crate) fn is_over(&self) -> bool {
        self.queue.is_empty() && self.asking.is_none() && self.spirits.is_empty()
    }
}

fn repo() -> Option<&'static SoloParamRepository> {
    // SAFETY: game thread; the singleton lives as long as the game.
    unsafe { SoloParamRepository::instance() }.ok()
}

fn row_of(id: u32, b: &BUDDY_PARAM_ST) -> AshRow {
    AshRow {
        id,
        npc_param: b.npc_param_id(),
        think: b.npc_think_param_id(),
        init: b.npc_player_init_param_id(),
        x_offset: b.x_offset(),
        z_offset: b.z_offset(),
        y_angle: b.y_angle(),
        generate_anim: b.generate_anim_id(),
        doping: [b.doping_sp_effect_lv0(), b.doping_sp_effect_lv1(), b.doping_sp_effect_lv2(), b.doping_sp_effect_lv3(), b.doping_sp_effect_lv4(), b.doping_sp_effect_lv5(), b.doping_sp_effect_lv6(), b.doping_sp_effect_lv7(), b.doping_sp_effect_lv8(), b.doping_sp_effect_lv9(), b.doping_sp_effect_lv10()],
        arena_doping: [b.arena_doping_sp_effect_1(), b.arena_doping_sp_effect_2(), b.arena_doping_sp_effect_3(), b.arena_doping_sp_effect_4(), b.arena_doping_sp_effect_5()],
        arena_friendly: [b.arena_friendly_sp_effect_1(), b.arena_friendly_sp_effect_2(), b.arena_friendly_sp_effect_3(), b.arena_friendly_sp_effect_4(), b.arena_friendly_sp_effect_5()],
        // (the paramdef types the first one u32; the game reads all five as the same i32 slots, 0xd29370)
        arena_hostile: [i32::try_from(b.arena_hostile_sp_effect_1()).unwrap_or(-1), b.arena_hostile_sp_effect_2(), b.arena_hostile_sp_effect_3(), b.arena_hostile_sp_effect_4(), b.arena_hostile_sp_effect_5()],
    }
}

/// THE ASH, READ AND PLACED (nothing made yet): the goods (its level in it, the game's `% 100`), the boss's body as the
/// owner, the summoner's place and facing (the game's physics space; `yaw` = the rotation about +Y of its orientation).
/// The spirits come at `origin_ahead` of the summoner, each at the generator's place of its row.
pub(crate) fn summon_hostile(goods: u32, owner: &FieldInsHandle, at_er: Vec3, yaw: f32) -> Result<Ash, String> {
    let repo = repo().ok_or("no param repository")?;
    let g = repo.get::<EquipParamGoods>(goods).ok_or_else(|| format!("no goods row {goods}"))?;
    let r = crate::spirit::request_of(goods, g.ref_id_default(), g.ref_category())?;
    let all: Vec<(i32, AshRow)> = repo.rows::<BuddyParam>().map(|(id, b)| (b.trigger_sp_effect_id(), row_of(id, b))).collect();
    let rows = rules::rows_of_trigger(&all, r.trigger);
    if rows.is_empty() {
        return Err(format!("no BuddyParam row has triggerSpEffectId {}", r.trigger));
    }
    if !at_er.is_finite() || !yaw.is_finite() {
        return Err("no place for the summoner".to_string());
    }
    let origin = rules::origin_ahead(at_er, yaw);
    let placed = rules::place_all(origin, yaw, &rows);
    let what = format!("goods {goods} +{} (trigger {}, {} spirit(s))", r.level, r.trigger, rows.len());
    erlog!("mimicash: THE BOSS'S ASH {what}{}: origin ({:.2}, {:.2}, {:.2}) yaw {:.1} deg: {}", if rules::is_mimic(&rows) { " (THE MIMIC TEAR's rows)" } else { "" }, origin.x, origin.y, origin.z, yaw.to_degrees(), rules::describe(&placed));
    Ok(Ash { what, owner: *owner, level: r.level, queue: placed.into(), asking: None, spirits: Vec::new(), ended: false, told_wait: false })
}

/// The character the creator made after `before`, complete (this frame).
fn made_after(before: usize) -> Option<&'static mut ChrIns> {
    let last = crate::allybody::last_created();
    if last == before || !game::plausible(last) {
        return None;
    }
    // SAFETY: the creator's own last character, this frame; completeness checked before any module.
    let chr = unsafe { &mut *(last as *mut ChrIns) };
    game::chr_complete(chr).then_some(chr)
}

/// The effective team of a character (the game's own function; its plain team when not located).
fn team_of(chr: &mut ChrIns, f: &Fns) -> u8 {
    let Some(at) = f.team_of else { return chr.team_type };
    let mut out = chr.team_type;
    // SAFETY: the game's own function on a live complete character, an out byte on our stack; game thread.
    unsafe {
        let team_of: TeamOfFn = std::mem::transmute(at);
        let _ = team_of(chr as *mut ChrIns, &mut out);
    }
    out
}

/// The local player's effective team.
fn player_team(f: &Fns) -> Option<u8> {
    // SAFETY: game thread; the singleton lives as long as the game.
    let player = unsafe { WorldChrMan::instance_mut() }.ok()?.main_player.as_deref_mut()?;
    let chr = &mut player.chr_ins;
    game::chr_complete(chr).then(|| team_of(chr, f))
}

/// The relation test (`Some(true)` = it holds; `None` = the function was not located).
fn relation(at: Option<usize>, a: u8, b: u8) -> Option<bool> {
    let at = at?;
    // SAFETY: the game's own pure table lookup on two team bytes (each < 79: the table is 79 x 79); game thread.
    Some(a < 79 && b < 79 && unsafe { std::mem::transmute::<usize, RelationFn>(at)(&a, &b) } != 0)
}

/// THE COLOSSEUM'S OPPONENT SETUP, ONCE THE SPIRIT IS THERE: the owner's team, the facing, the arrival, the level doping.
fn adopt(chr: &mut ChrIns, p: &Placed, owner: &FieldInsHandle, level: u8, f: &Fns) {
    let before = chr.team_type;
    let owner_team = crate::targets::find_chr(owner).map(|o| team_of(o, f));
    match (owner_team, f.adopt) {
        (Some(t), Some(at)) => {
            // SAFETY: the game's own function on a live complete character and a team byte on our stack; game thread.
            unsafe { std::mem::transmute::<usize, AdoptTeamFn>(at)(chr as *mut ChrIns, &t) };
        }
        (Some(t), None) => chr.team_type = rules::adopt_team(chr.team_type, t),
        (None, _) => erlog!("mimicash: !!! the boss's body is not in the field: row {} keeps its own team {before}", p.row.id),
    }
    let q = game::quaternion(Quat::from_rotation_y(p.yaw));
    let physics = chr.modules.physics.as_mut();
    physics.orientation = q;
    physics.interpolated_orientation = q;
    if p.row.generate_anim >= 0 && game::plausible(word!(chr.modules.event)) {
        // (the arrival the game plays when its spirit is ready, 0x4be170: the event module's "play this id next")
        chr.modules.event.as_mut().request_animation_id = p.row.generate_anim;
    }
    let doping = rules::setup_effects(&p.row, level, rules::APPLY_ARENA_DOPING);
    for id in &doping {
        // (the game passes 0 for the dopings, 0x4be179)
        chr.apply_speffect(*id, false);
    }
    erlog!("mimicash: spirit of row {} IN THE FIELD (NpcParam {}): team {before} -> {} (the boss's {owner_team:?}), arrival anim {}, doping {doping:?}, HP {}/{}", p.row.id, p.row.npc_param, chr.team_type, p.row.generate_anim, chr.modules.data.hp, chr.modules.data.max_hp);
}

/// THE ARENA MARKS (the Update's rule, every frame): hostile / friendly to the local player by the game's relation.
fn mark(chr: &mut ChrIns, row: &AshRow, player: Option<u8>, f: &Fns) {
    let me = team_of(chr, f);
    let (hostile, friendly) = match player {
        // (relation functions not located: the boss's spirits are the boss's team, hostile to the player by construction)
        Some(p) => (relation(f.hostile, p, me).unwrap_or(true), relation(f.friendly, p, me).unwrap_or(false)),
        None => return,
    };
    let has: Vec<i32> = chr.special_effect.entries().take(128).map(|e| e.param_id).collect();
    for id in rules::arena_marks(row, hostile, friendly, &|id| has.contains(&id)) {
        // (the game passes 1 for these, 0x4b8d91)
        chr.apply_speffect(id, true);
        erlog!("mimicash: row {}: arena {} mark SpEffect {id} (player team {player:?}, spirit team {me})", row.id, if hostile { "hostile" } else { "friendly" });
    }
}

fn kill(handle: &FieldInsHandle) {
    if let Some(chr) = crate::targets::find_chr(handle) {
        if chr.modules.data.hp > 0 {
            chr.modules.data.hp = 0;
        }
    }
}

/// EVERY STEP: the next spirit asked of the creator, a new one adopted, the hostile marks kept, the dead let go. Gives
/// the living spirits.
pub(crate) fn poll(ash: &mut Ash, dt: f32) -> Vec<FieldInsHandle> {
    let f = fns();
    if let Some(mut a) = ash.asking.take() {
        match made_after(a.before) {
            Some(chr) if ash.ended => {
                erlog!("mimicash: row {} came after the boss's end: it dies at once", a.placed.row.id);
                chr.modules.data.hp = 0;
            }
            Some(chr) => {
                adopt(chr, &a.placed, &ash.owner, ash.level, &f);
                ash.spirits.push(Spirit { handle: chr.field_ins_handle, row: a.placed.row });
            }
            None if a.waited > GIVE_UP_AFTER => erlog!("mimicash: !!! row {} did not appear in {GIVE_UP_AFTER} s: given up", a.placed.row.id),
            None => {
                a.waited += dt;
                ash.asking = Some(a);
            }
        }
    }
    if ash.asking.is_none() && !ash.ended {
        if let Some(p) = ash.queue.pop_front() {
            match crate::allybody::ask_foe(rules::chr_of(p.row.npc_param, p.row.init), p.row.npc_param, p.row.think, p.row.init, p.at) {
                Ok(before) => {
                    ash.asking = Some(Asking { placed: p, before, waited: 0.0 });
                    ash.told_wait = false;
                }
                Err(e) => {
                    if !ash.told_wait {
                        erlog!("mimicash: row {} waits for the creator ({e})", p.row.id);
                        ash.told_wait = true;
                    }
                    ash.queue.push_front(p);
                }
            }
        }
    }
    let player = player_team(&f);
    let mut out = Vec::new();
    ash.spirits.retain(|s| match crate::targets::find_chr(&s.handle) {
        Some(chr) if chr.modules.data.hp > 0 => {
            mark(chr, &s.row, player, &f);
            out.push(s.handle);
            true
        }
        _ => {
            erlog!("mimicash: the spirit of row {} is gone", s.row.id);
            false
        }
    });
    out
}

/// THE BOSS IS GONE: every spirit of its ash dies (HP 0: the game's own death and the spirit's fade); nothing more is made.
pub(crate) fn kill_all(ash: &mut Ash) {
    if !ash.ended {
        erlog!("mimicash: {}: the boss is gone: its {} spirit(s) die with it ({} not made)", ash.what, ash.spirits.len(), ash.queue.len());
    }
    ash.ended = true;
    ash.queue.clear();
    for s in &ash.spirits {
        kill(&s.handle);
    }
}

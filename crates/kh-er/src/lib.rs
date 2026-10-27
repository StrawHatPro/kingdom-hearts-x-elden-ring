//! Sora hosted inside Elden Ring. `kh_er.dll` is the payload `kh_er_loader.dll` (what me3
//! injects) loads and reloads; it does nothing on any executable other than 2.7.1.0 or 2.7.2.0 (PORT). Everything
//! Elden Ring specific lives in this crate and the loader; `kh-core` / `kh-anim` know nothing of it.
//!
//! State note: `build/sora_tmp/er/IMPL_ER1_STATE.md`.

pub mod coords;
pub mod ermap;
// (fx) the numbers of Sora's sound and effects in this host (sheet sora-er-fx)
pub mod fxmap;
// (prize) the one drop rule of this host and the munny mirror (pure; prize.rs is the game half)
pub mod prizemap;
// (loot) Elden Ring item -> KH3 item rules and the lot roll (pure; sheet sora-er-items)
pub mod lootmap;
pub mod sweep;
// (IMPL_ER15) the void of the game-over screen (the camera maths is pure; the rest is the game half)
pub mod void;
// (IMPL_ER48) Elden Ring's status ailments and Sora: ticks without a flinch, what clears them (pure; status.rs is the game half)
pub mod status_rules;
// (DTH) one death for whoever leads: the scripted death (KO, fade, the game's own scene), the game over, the party after it
pub mod death;
// (DON-MAGIC) the Elden Ring side of Donald's statuses: Exposed on Sora's hits, Frozen / Chill / the slow on the characters'
// animation speed, Blind's aim, Aero Barrier and Sanctuary on Sora (donstatus.rs; UNTESTED IN GAME)
pub mod donstatus;
// (ER party S1) Donald, Goofy and AI Sora: the party host (party/mod.rs; cargo feature `party`, key `party.er`)
#[cfg(feature = "party")]
pub mod party;
// (DON-PROG) Elemental Insight in Elden Ring: the enemies' NpcParam elements as Donald's (insight.rs)
#[cfg(feature = "party")]
pub mod insight;
// (ER party S6) the party's screens, save and progression: the mailbox between the party's step and the HUD / save /
// save point / prizes / rewards (partyui.rs; cargo feature `party`, key `party.er`)
#[cfg(all(windows, feature = "party"))]
pub mod partyui;
// (PFY) Y on the Party Formation screen through the Elden Ring pad path (tests only)
#[cfg(all(test, windows, feature = "party"))]
mod pfy_tests;
#[cfg(all(test, feature = "party"))] // (ZETTA 2026-10-23) its tests drive kh-party
mod zetta_tests; // (ZETTA) Zettaflare forced through any enemy's reaction rules
// (EQ) a friend's camp-menu edits in force at once while the world is paused, through the Elden Ring path (tests only)
#[cfg(all(test, windows, feature = "party"))]
mod eq_tests;
// (ER party S8) unlocking Donald, Goofy and Sora: the character select, the staged fights, Donald at the Finger Reader,
// the unlock moment, the unlock record (unlock/; cargo feature `party`, key `party.unlocks`)
#[cfg(feature = "party")]
pub mod unlock;
// (ER boss) SDK extension: Elden Ring fields the pinned SDK leaves unnamed (the AI switch of EMEVD 2004[1], the update
// rate of 2004[34]); found with Ghidra, build/sora_tmp/er/party/IMPL_BOSS_STATE.md. Used by unlock/ (feature `party`).
#[cfg(all(windows, feature = "party"))]
pub mod sdkx;
// (ER party S3) the AI allies' invisible Elden Ring bodies, F1 redirection, the go/no-go test (allybody.rs; cargo feature
// `party`, keys `party.er` / `party.bodies` / `party.body_test` / `party.bullet_owner`)
#[cfg(feature = "party")]
pub mod allybody;
// (ER party S4) Donald, Goofy and the team props handed to the renderer; their camp pictures and the void's fallen friend
// (partydraw.rs; cargo feature `party`, keys `party.er` / `r.party`)
#[cfg(all(windows, feature = "party"))]
pub mod partydraw;

#[cfg(windows)]
mod bossguard;
#[cfg(windows)]
mod poise; // (IMPL_ER45) Elden Ring's poise decides how hard an ordinary enemy is to stagger (`poise.kh`)
mod travel; // (IMPL_ER23A) fast travel only from a save point
mod ladder; // (LADDER2) ladders: Y at the foot of one = the party's jump up onto the game's own top exit
mod exitnet; // (ANIMS) every way off a ladder / out of a body action ends the interaction (no softlock)
mod propfit; // (ANIMS) the borrowed Elden Ring actions at each character's own proportions (standoff, rungs, planted feet, style)
#[cfg(test)]
mod propfit_sheet; // (ANIMS) the offline proof sheets (build/sora_tmp/party/ANIMS/proof)
mod limbs; // (ER70) hands and feet on a ladder's rungs, for the worn outfit's own skeleton
mod chest; // (IMPL_ER58) chests: Sora follows the body and plays KH3's own treasure-box opening (`chest.kh`)
mod mapwatch; // (IMPL_ER38) the map's requests and outcomes, the game's window focus, the View guard
pub mod loadin; // (LDI) everything in place on the first visible frame after a load (the party held under the loading screen)
mod guards; // (IMPL_ER23A) press ownership, the fall guard, the stuck detector
mod erconfig;
mod portrait;
mod bullet;
// (IMPL_ER32) Sora's hits break Elden Ring's breakable map objects; fire is fire (`break.kh`)
#[cfg(windows)]
mod breakables;
#[cfg(windows)]
mod cam;
// (CV1) the conversation system in Elden Ring: one talk at a time, the bubble, the locks, the camera (conv.rs)
#[cfg(windows)]
pub mod conv;
#[cfg(windows)]
mod chranim;
#[cfg(windows)]
mod control;
#[cfg(windows)]
mod damage;
#[cfg(windows)]
mod inputs;
// (prize) KH3's prize orbs from killed enemies, the munny mirror; OFF unless `prize.kh` / `munny.mirror`
#[cfg(windows)]
mod prize;
// (loot) the kill drop as KH3 prize boxes and the gate on the game's item-give function (OFF by default)
#[cfg(windows)]
mod loot;
mod erkeyui; // (HUD2) Elden Ring's own key-item UI (MAP FOUND, pickup window, item note) off while KH3's OBTAINED popup is on
#[cfg(windows)]
mod savepoint;
#[cfg(windows)]
mod status; // (IMPL_ER48) Cure, status-clearing items and save points clear Elden Ring's ailments on the body
mod savepoint_rules; // (IMPL_ER26) the save points' pure rules: unlock, Y, the rest for story events
// (savegate) saving only at save points, reloading the last save (IMPL_ER10)
#[cfg(windows)]
mod savegate;
// (SAVEPATH) the save file the game really uses, whatever its name (me3 `savefile`, Me3 Manager); ER0000.sl2 warned about
#[cfg(windows)]
mod savepath;
#[cfg(windows)]
mod puppet;
#[cfg(windows)]
mod targets;
mod groundwait; // (ZETTA 2026-10-23) the wait for the ground after the game moves its player
mod zdefer; // (ZETTA 2026-10-23) Zettaflare owns the presentation: its damage owed until the hand-back
mod zpose; // (ZETTA 2026-10-23) Zettaflare's target in its heavy hurt, frozen at its middle
mod zphase; // (ZETTA 2026-10-24) a two-phase boss killed whole the game's way: its change blocked, its defeat the game's
mod zboss; // (ZETTA 2026-10-23) a boss Zettaflare kills goes quietly: its last words and its cutscene skipped the game's way
mod zdraw; // (ZDRAW) Zettaflare's target drawn from its own model (its FLVER from the game's files, its bones from memory)
mod zfinale; // (ZFIN) Zettaflare's finale on an Elden Ring character: the renderer's draw route fed, the crumbled body hidden
// (IMPL_ER8) Elden Ring's own HUD hidden part by part
#[cfg(windows)]
mod erhud;
#[cfg(windows)]
mod entry;
// (PORT) which eldenring.exe runs (2.7.1.0 / 2.7.2.0) and the per-version address tables
pub mod gamever;
#[cfg(windows)]
mod frame;
// (INT) the whole KH3 side of the frame drawn between two fixed steps (`r.interp`, default on)
#[cfg(windows)]
pub mod interp;
// (INT) the measurement: Sora, the camera and the effects drawn between the steps under Elden Ring-like frame times
#[cfg(all(test, windows))]
mod int_tests;
// (BM) passive wildlife starts no battle mode (Sora, a friend leader, the friends): tests only
#[cfg(all(test, windows, feature = "party"))]
mod bm_tests;
#[cfg(windows)]
mod fx;
#[cfg(windows)]
mod audio_listener; // (ER62) one listener for every sound: the camera on screen (`audio.listener_fix`)
#[cfg(windows)]
mod game;
#[cfg(windows)]
mod havok;
#[cfg(windows)]
mod ersound; // (LADDER2) Elden Ring's own sounds played by the game at run time (the ladder jump's Torrent wind)
// (hud) Sora's HUD, camp menu and game-over menu in this host
#[cfg(windows)]
mod hud;
#[cfg(windows)]
mod log;
mod redact;
#[cfg(windows)]
mod pad;
#[cfg(windows)]
mod passive;
// (IMPL_ER11) the game's own pause while a Kingdom Hearts menu is open
#[cfg(windows)]
mod newgame;
#[cfg(windows)]
mod pause;
// (IMPL_ER11) the cutscene's actors
#[cfg(windows)]
mod remo;
// (IMPL_ER16) memory probes (testing aids)
#[cfg(windows)]
mod probe;
// (IMPL_ER16) the figure a real-time cutscene draws in the player's place
#[cfg(windows)]
mod figure;
/// (IMPL_ER22) KH3's boss finish: the slow motion, the cuts, the white-out, the sound (`finish.kh`).
mod finish;
/// (IMPL_ER31) Bosses give magic, abilities, slots, max HP / MP (`boss.rewards`; the table is kh_host::bossrewards).
mod bossreward;
/// (IMPL_ER17) The sites of grace's own flame, not drawn.
mod graceflame;
mod gracehide; // (SV) the graces' mound models, found by a memory search, never drawn
mod gracelock; // (SV) the game's grace actions (Touch grace / Rest) switched off in ActionButtonParam
mod telescope; // (TELESCOPE) the game's Birdseye Telescopes switched off in ActionButtonParam (`telescope.er = 1`: the game's own)
mod erprompt; // (RC) every Y interaction (Elden Ring's prompts, party talks, Save) as KH3's green reaction command
mod talkdrive; // (IMPL_ER36) a story rest driven to its end: the gates, her entry's place, the step machine
mod talkkh; // (CV2) Elden Ring's character conversations as KH bubbles through crate::conv (`talk.kh`)
mod shopkh; // (SHOP) the merchants' buy / sell windows -> the KH shop; the smiths' talk refused; held online items converted
mod story; // (IMPL_ER33) the story events Elden Ring ties to resting: the table, once-only, deferral, the cover
mod cutgrace; // (IMPL_ER33) a cutscene's own site of grace, hidden
mod tarnfx; // (LANT) nothing of the hidden Tarnished drawn: his Lantern's model (its light kept), effects proven his
/// (IMPL_ER17) Elden Ring's empty bloodstain, put out of the way.
mod stain;
/// (IMPL_ER60) Blood on Sora and the Keyblade by Elden Ring's rules; a save point washes it off (`blood.kh`).
#[cfg(windows)]
mod blood;
// (IMPL_ER59) Sora's outfits, opened by talking to Boc: the camp menu's Outfits entry, the save record, the drawn model
mod boc;
// (ENG) Elden Ring's own group limiter: up to enemy.max_attackers (5) enemies of a group attack Sora at once (attackers.rs)
pub mod attackers;
// (save) Sora's save data per Elden Ring character
#[cfg(windows)]
mod save;
#[cfg(windows)]
mod slots;
#[cfg(windows)]
mod script;
// (IMPL_ER12) the title's flow (new game)
#[cfg(windows)]
mod title;
// (IMPL_ER13) Elden Ring's tutorials are not shown
#[cfg(windows)]
mod tutorial;
// (IMPL_ER15) KH3's Load screen on the title
#[cfg(windows)]
mod titleload;
#[cfg(windows)]
mod sora;
#[cfg(windows)]
mod falllog; // (IMPL_ER24) why he left the floor / what he pushes against: the log's reports
#[cfg(windows)]
mod wallscan;
#[cfg(windows)]
mod world;

//! Sora hosted inside Elden Ring. `kh_er.dll` is the payload `kh_er_loader.dll` (what me3
//! injects) loads and reloads; it does nothing on any executable other than 2.7.1.0 or 2.7.2.0 (PORT). Everything
//! Elden Ring specific lives in this crate and the loader; `kh-core` / `kh-anim` know nothing of it.
//!
//! State note: `build/sora_tmp/er/IMPL_ER1_STATE.md`.

pub mod coords;
pub mod ermap;
// (THE USER 2026-11-02: "Shields don't count as guarding") Elden Ring's own guard of every character as KH3's guard
pub mod shieldguard;
// (THE IN-GAME TESTER 2026-11-03: a Breaker killed Margit the KH side expected to live) a KH hit never takes more than expected
pub mod hitcap;
// (fx) the numbers of Sora's sound and effects in this host (sheet sora-er-fx)
pub mod fxmap;
// (prize) the one drop rule of this host and the munny mirror (pure; prize.rs is the game half)
pub mod prizemap;
// (loot) Elden Ring item -> KH3 item rules and the lot roll (pure; sheet sora-er-items)
pub mod lootmap;
pub mod sweep;
// (NOKRON-WALLS) a boss arena's invisible walls for every KH character while its fight runs (sheet arena-walls; pure + the game half)
pub mod arena_walls;
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
// (P8 MOOGLE NPC) KH3's Synthesis Moogle at Roundtable Hold: drawn by the party path, his Talk opens the Workshop (moogle.rs)
#[cfg(all(windows, feature = "party"))]
pub mod moogle;

#[cfg(windows)]
mod bossguard;
#[cfg(windows)]
mod bossrevenge; // (REVENGE) KH3's revenge for Elden Ring's bosses: the limit, the stun gate (`react.revenge.boss*`, `react.boss_stun*`)
#[cfg(windows)]
mod aireplan; // (REVENGE) a boss's AI asked to plan its next action now (Lua's ai:Replaning)
// (REVENGE) the Mimic Tear boss copies' revenge (the meter, the gate, the counter by form), driven by mimic_boss.rs
#[cfg(all(windows, feature = "party"))]
mod mimicrevenge;
#[cfg(windows)]
mod poise; // (IMPL_ER45) Elden Ring's poise decides how hard an ordinary enemy is to stagger (`poise.kh`)
#[cfg(all(test, windows))]
mod freezefx_tests; // (FREEZE-FX) every frozen Elden Ring body wears KH3's frozen look for exactly its freeze
#[cfg(all(test, windows))]
mod stagger_tests; // (GF-BREAK) Goofy's Stagger on Elden Ring's bodies
mod travel; // (IMPL_ER23A) fast travel only from a save point
mod nocombat; // (NOCOMBAT) where Elden Ring forbids the Tarnished to attack (its state type 457: the Roundtable Hold ...), no KH character may
mod ladder; // (LADDER2) ladders: Y at the foot of one = the party's jump up onto the game's own top exit
mod exitnet; // (ANIMS) every way off a ladder / out of a body action ends the interaction (no softlock)
mod propfit; // (ANIMS) the borrowed Elden Ring actions at each character's own proportions (standoff, rungs, planted feet, style)
#[cfg(test)]
mod propfit_sheet; // (ANIMS) the offline proof sheets (build/sora_tmp/party/ANIMS/proof)
mod limbs; // (ER70) hands and feet on a ladder's rungs, for the worn outfit's own skeleton
mod cutfit; // (CUTSCENE-ANIM) the leader in a real-time cutscene at his own proportions, on the scene's floor, hands and feet on what the figure's touch
#[cfg(all(test, windows))]
mod cutfit_sheet; // (CUTSCENE-ANIM) the offline proof (build/sora_tmp/weapons/CUTSCENE-ANIM/shots)
mod chest; // (IMPL_ER58) chests: Sora follows the body and plays KH3's own treasure-box opening (`chest.kh`)
mod doorfree; // (ER-TWEAKS) every Elden Ring door / lever opens without locking the leader into an animation (`door.free = 0`: as before)
mod fogwalk; // (ER-TWEAKS) the leader walks through a fog gate with his own walk, not the Tarnished's pose (`fog.walk = 0`: as before)
#[cfg(windows)]
mod freezefx; // (FREEZE-FX) KH3's frozen look on every frozen Elden Ring enemy (the shared route's host half)
mod herolight; // (HEROLIGHT-KB) the hero light's glow and sparkles on the party leader's weapon, whatever it is
mod mapwatch; // (IMPL_ER38) the map's requests and outcomes, the game's window focus, the View guard
mod nopause; // (NO-ER-PAUSE) Elden Ring's in-game top menu refused where the game opens it (the user 2026-11-09)
#[cfg(windows)]
mod crashrec; // (CONT-CRASH) an exception nobody takes: its place in the log and a minidump beside it, then on to the game / Windows as before
mod mapgate; // (MAP-GATE) Elden Ring's own gate on its map read from the game, and its own map request made when that gate keeps refusing
pub mod loadin; // (LDI) everything in place on the first visible frame after a load (the party held under the loading screen)
mod guards; // (IMPL_ER23A) press ownership, the fall guard, the stuck detector
mod grabs; // (GRABS) Elden Ring's grabs on KH characters: KH3's caught sequence, the throw census, the Raya Lucaria Abductor, the lock-on camera's release
mod deathplane; // (DEATHPLANE) Elden Ring's death barriers (its contact tracker's kill / lethal-fall / fall-camera kinds) for every KH character
mod erconfig;
mod dinput_mask; // (CAMP-OPTIONS) the game's DirectInput keyboard and mouse masked as its pad is
mod remap_er; // (CAMP-OPTIONS) the button tables at the input layer (controller, keyboard + mouse)
mod button_icons_er; // (BUTTON-ICONS) the KH UI's button icons kept live: the setting, the tables, the device used last
mod titlemusic; // (TITLE-AUDIO) KH3's title music and menu sounds on Elden Ring's title screen
mod mimic_music; // (MIMIC-MUSIC) KH3's Replicas in place of Elden Ring's boss music in the two Mimic Tear fights
mod eroptions; // (CAMP-OPTIONS) Elden Ring's options screen in KH3's look: the game side (kh-host er_options.rs, kh-ui camp_options.rs)
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
pub mod attackread; // (GF-SENSOR) the class of an attack row (R2)
#[cfg(windows)]
mod chranim;
#[cfg(windows)]
mod taebook; // (GF-SENSOR) every character's attack windows, read from the game's files at run time
#[cfg(windows)]
mod autopad; // (GF-INGAME) the test pilot: `test.auto`
#[cfg(windows)]
mod sensorlab; // (GF-SENSOR) the attack sensor's test bench (`atk.*` keys)
#[cfg(windows)]
pub mod stance; // (GF-SENSOR) R6: an enemy's stance forced to break, by ladder
#[cfg(windows)]
mod aipause; // (GF-POLISH) Pin Down: an enemy's AI off for a moment, always given back
#[cfg(windows)]
mod rebound; // (THE USER 2026-11-02) an enemy's swing knocked back off Goofy's shield on a perfect deflect, by ladder
#[cfg(windows)]
pub mod telegraph; // (GF-SENSOR) what the enemies are doing, each frame: the telegraph list, who threw a hit
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
mod erdialog; // (ER-POPUPS) Elden Ring's event-script pop-up boxes (EMEVD 2007[1] messages, 2007[10] questions) as KH3's Dialog window (`er.dialog_kh = 0`: the game's)
#[cfg(windows)]
mod dragonmsg; // (ER-TWEAKS) Elden Ring's "A new draconic power is available" popup after a dragon's defeat is never shown (`er.dragon_popup = 1`: shown)
#[cfg(windows)]
mod savepoint;
#[cfg(windows)]
mod status; // (IMPL_ER48) Cure, status-clearing items and save points clear Elden Ring's ailments on the body
mod savepoint_rules; // (IMPL_ER26) the save points' pure rules: unlock, Y, the rest for story events
#[cfg(windows)]
mod savepoint_area; // (INTERACT-MARK) the save points whose area is the user's choice (the Roundtable Hold's, at table level)
mod gestureprompt; // (EMOTE-PROMPTS) Elden Ring's gesture-gated events as a KH3 Y command once the gesture is known (sheets/port/kh-er-gesture-events.json)
mod spiritspring; // (SPIRITSPRING) the leader's normal jump at a spiritspring rises to Torrent's height (sheets/port/kh-er-spiritsprings.json)
// (savegate) saving only at save points, reloading the last save (IMPL_ER10)
#[cfg(windows)]
mod savegate;
// (SAVEPATH) the save file the game really uses, whatever its name (me3 `savefile`, Me3 Manager); ER0000.sl2 warned about
#[cfg(windows)]
mod savepath;
#[cfg(windows)]
mod puppet;
// (THE USER 2026-11-02: "an enemy clipped into the ground fully") every place written for an enemy we move is never below
// the ground under it, and what we switched off on it is always given back (pure; puppet.rs is the game half)
mod groundclamp;
#[cfg(windows)]
mod targets;
// (COMBAT-FIX) Elden Ring's own "cannot be damaged" state of a character: no KH hit, no lock while it holds (Rennala's barrier)
#[cfg(windows)]
mod invuln;
#[cfg(all(test, windows))]
mod invuln_tests; // (COMBAT-FIX) no KH hit / lock on what Elden Ring holds invulnerable
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
// (INT, SM) the smoothness metrics the measurements below share (kh-partyhost's file)
#[cfg(test)]
#[path = "../../kh-partyhost/src/sm_metrics.rs"]
mod sm_metrics;
// (INT) the measurement: Sora, the camera and the effects drawn between the steps under Elden Ring-like frame times
#[cfg(all(test, windows))]
mod int_tests;
// (BM) passive wildlife starts no battle mode (Sora, a friend leader, the friends): tests only
#[cfg(all(test, windows, feature = "party"))]
mod bm_tests;
// (HUD-REVIVE-COMBAT) one party-wide combat state (any enemy targeting / fighting any member), whoever leads: tests only
#[cfg(all(test, windows, feature = "party"))]
mod combat_tests;
#[cfg(windows)]
mod fx;
#[cfg(windows)]
mod fxrays; // (AUDIT-B W-HANDOVER) the effects' line trace (KH3's SQEX_Raycast) through Elden Ring's Havok world
#[cfg(windows)]
mod audio_listener; // (ER62) one listener for every sound: the camera on screen (`audio.listener_fix`)
#[cfg(windows)]
mod game;
#[cfg(windows)]
mod havok;
#[cfg(windows)]
mod standin; // (ER-HITSFX round 2) who stands in for a KH character: the hidden Tarnished, the ally-pool bodies, the Mimic boss bodies (and those that left)
#[cfg(windows)]
mod ersound; // (LADDER2) Elden Ring's own sounds played by the game at run time (the ladder jump's Torrent wind)
#[cfg(windows)]
mod gosound; // (GOS, THE USER 2026-11-03) the game over's silence: Elden Ring's master volume and our KH sounds, never the menus' sounds
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
mod ngquiet; // (CC-MUTE, THE USER 2026-11-07) Elden Ring's character creation silent and taking no input until our first KH new-game pop-up
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
mod worldtime; // (AUDIT-A WORLDTIME) KH3's world slow (WorldSlow 0x14140E500) and the forms' hit-stops (SetGlobalGameSpeed)
#[cfg(windows)]
mod chrtime; // (WORLDTIME-2) the world slow on Elden Ring's own clocks: each enemy's delta, its bullets, the effects, the map clock
#[cfg(windows)]
mod fxtime; // (WORLDTIME-2 fx) Elden Ring's effects by their owner's KH3 type, and its physics step at PHYSX's
/// (IMPL_ER31) Bosses give magic, abilities, slots, max HP / MP (`boss.rewards`; the table is kh_host::bossrewards).
mod bossreward;
/// (P9b ACQUISITION) KH3's weapons from Elden Ring's bosses, chests and rare drops (the rules: kh_host::bossrewards).
mod weaponsrc;
/// (IMPL_ER17) The sites of grace's own flame, not drawn.
mod graceflame;
mod gracehide; // (SV) the graces' mound models, found by a memory search, never drawn
mod gracelock; // (SV) the game's grace actions (Touch grace / Rest) switched off in ActionButtonParam
mod ertext; // (MUNNY-TEXT) Elden Ring's own text changed in its loaded message tables: the currency runes say munny
mod telescope; // (TELESCOPE) the game's Birdseye Telescopes switched off in ActionButtonParam (`telescope.er = 1`: the game's own)
mod summonpool; // (SUMMON-POOL) Elden Ring's summoning pools never activate: their action button's region is nothing (the user 2026-11-10)
mod erprompt; // (RC) every Y interaction (Elden Ring's prompts, party talks, Save) as KH3's green reaction command
mod reactmark; // (INTERACT-MARK) KH3's green reaction-target marker on what the first Y interaction is about
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
/// (THE USER 2026-11-02) A blow a KH character's guard took: his guard sound (his sheet row), no blood, the hidden body silent.
#[cfg(windows)]
mod guardblow;
// (IMPL_ER59) Sora's outfits: the save record, the drawn model; (THE USER 2026-11-02) the Outfits screen only through
// Boc's "Alter garments" (talkkh/alter.rs), no camp menu entry
mod boc;
// (THE USER 2026-11-02) Roderika's spirit tuning: the KH Spirit Tuning screen spends the KH3 stock's gloveworts and raises the
// ash's level in the game's inventory (talkkh/tuning.rs swallows her window)
#[cfg(windows)]
mod spirittune;
// (P7 SMITHS, the user 2026-11-03) KH3's Keyblade Forge from every smith (talkkh/smith.rs swallows command 24; sora.rs calls
// `talk_frame`)
#[cfg(windows)]
mod forgekh;
// (THE USER 2026-11-03) the shared Elden Ring icon route: any goods id -> its icon on a KH screen (Summon's ashes too)
#[cfg(windows)]
pub mod ericon;
// (THE USER 2026-11-02: "Give Sora the ability to 'Summon' from the command menu, but he uses Elden Ring's spirit ashes") the
// Summon command's Elden Ring side: the owned ashes, the bell, the spirit through the game's own buddy system, its timer and
// leaving, Elden Ring's spirit HUD hidden; (THE USER: "Mimic Tear needs to mimic Sora") the Mimic Tear calls Mimic Sora
#[cfg(windows)]
mod spirit;
#[cfg(windows)]
mod spirit_mimic;
#[cfg(all(windows, feature = "party"))]
mod mimic_boss_ash; // (MIMIC-ASH) the Mimic Tear boss's whole spirit ash: every BuddyParam row, as the game's summon makes it
#[cfg(windows)]
mod mimic_boss; // (MIMIC-BOSS) the two Mimic Tear bosses: the copy of the leader, the solo fight, its summon (kh-partyhost mimicboss)
#[cfg(all(windows, feature = "party"))]
mod mimic_rise; // (MIMIC-BOSS hotfix 3, MB3-FOGRISE) the Mimic Tear's rise at the fog gate: Elden Ring's own pool wake (kh-partyhost mimicboss PoolWake)
// (ENG) Elden Ring's own group limiter: up to enemy.max_attackers (5) enemies of a group attack Sora at once (attackers.rs)
pub mod attackers;
// (save) Sora's save data per Elden Ring character
#[cfg(windows)]
mod save;
#[cfg(windows)]
mod slots;
// (CONT-LADDER) Continue and Load put the leader at the save point of the save they load (our own record of each KH save)
#[cfg(windows)]
mod saveplace;
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

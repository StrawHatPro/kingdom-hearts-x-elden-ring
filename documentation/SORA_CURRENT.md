# Sora, as he is now (FINAL10 build)

This page describes Sora in the KH3 Sora → Elden Ring mod **as deployed now**: the FINAL10 build (`build/sora_tmp/er/final10/kh_er_af5718317bd7.dll`, deployed 2026-10-18 22:39). It was built from `build/sora_tmp/er/final10/ws`, and that folder's sheets and code are the source for everything below.

This page is documentation only. No code, sheet, build or game file was changed, and nothing was run to write it.

**Labels used on this page**

| Label | Meaning |
|---|---|
| **Implemented** | In FINAL10's sheets and code. Many things are still **untested in game**; section 9 lists them. |
| **Planned** | Decided (by you, or approved in a design note) but not in the build. |
| **Potential (suggestion)** | Not decided anywhere. It is my suggestion, tied to Elden Ring progression, for you to accept or change. |
| *[user]* | A reward you picked yourself. |
| *[XS]* | One of the 16 new Sora abilities (XS design, approved 2026-10-16). |
| *proposal* | Built and active, but marked in the sheet as "a proposal, not confirmed by the user". |

Short source names: `sheets/` = `final10/ws/sheets/port/`, `notes/` = `build/sora_tmp/party/` or `build/sora_tmp/er/`.

---

## 1. Overview

**Who he is.** Sora is the KH3 Sora, his combat ported from the game's own code and data. He runs inside Elden Ring in place of the Tarnished:
- He is drawn by the mod's own renderer.
- He hits Elden Ring enemies with KH3 combos, magic and juggles.
- He saves at KH3 save points, which replace the sites of grace.

He is one of three playable characters, with Donald and Goofy (section 8).

**Choosing who to start as (Implemented).** The new game asks "Whose journey will you begin?" (Sora / Donald / Goofy; `sheets/party-er-unlocks.json`).
- If you pick Sora, he starts at level 1.
- If you pick someone else, Sora is found fighting Margit and joins when Margit is beaten (`ER_PHASE_NOTES.md` U2).

**The opening's two KH3 questions (Implemented).** These are KH3's own (`kh-core/src/status/newgame.rs`). The default pair, for a host that does not ask, is Balance + Warrior.

| Question | Choices | What it changes |
|---|---|---|
| "What do you desire?" | Vitality / Balance / Wisdom | Start HP / MP (Standard): 120/100, 105/110, 90/120 |
| "What power do you seek?" | Warrior / Mystic / Guardian | The level-up ability path: A / B / C (section 4.2) |

**Controls (Implemented).** These are KH3's layout on an Xbox pad (`sheets/sora-input-layout.json`); the keyboard is an extra path.

| Pad | Action |
|---|---|
| Left stick / right stick | Move / camera (R3 resets the camera) |
| **A** | Attack; confirm in menus; advances dialogue while talking, except in combat |
| **B** | Jump (hold for Glide once he has it); cancel |
| **X** | Block; with the stick, Dodge Roll; in the air, Aerial Dodge / Air Slide |
| **Y** | Situation command: Second Form, Grand Magic, team attacks, finishers, reprisals |
| **LB + A/B/X/Y** | Shortcuts: spells and items, set in Customize (3 sets of 4) |
| **RB** | Tap: lock on. Hold: Shotlock aim |
| **RT** | Tap: change target |
| **LT** | Next situation command |
| **D-pad** | Command menu: Attack / Magic / Items. Link is disabled: Sora has no Links in the port. |
| **Menu** | Pause (the KH3 menu) |

**The HUD in combat (Implemented 2026-11-02 at your request, untested in game).** KH3's own rule (`0x141631D90`, `kh-host/src/hud.rs` `Hud::battle_mode`):
- The command menu is **blue** outside combat and turns to KH3's **orange-gold battle look** while the leader is in combat: the Attack / Magic / Items bars, the selected bar, the "COMMANDS" title and the item / shortcut numbers (the movie's `cmdYellow` state). A special battle (KH3's forced battle flag) gives the red look.
- "In combat" is the leader's battle flag: an engaged enemy, kept **1.0 s** after the last one (KH3's linger). Sora's own flag when he leads; the party's flag when Donald, Goofy or a later character leads.
- **Sora's portrait** shows KH3's **battle face** (the `battle` look of his face movie `FacePcExSora01`, already in the pack) while his battle flag is on, and his smiling face otherwise. The low-HP (red) face still wins over it. Donald's and Goofy's portraits do the same from their own battle flag, as before.
- `hud-cmd battle 0|1|2` fixes the colour for a test; `hud-cmd battle live` gives it back.

**Sora's HUD face follows his outfit (Implemented 2026-11-03 at your request, untested in game).** The outfit he wears (Boc's Outfits screen, or the loaded save's) picks the face movie of his HUD portrait. These are KH3's own: each outfit's pawn names one in KH3's data (`UI/Data/Character/<pawn>_StatusData`). Each movie has KH3's normal, battle and low-HP looks (KH1's is one picture for all of them, as in KH3), so the battle-face rule above and the red low-HP face work for every outfit. The face also changes in his small slot when a friend leads, and on the Party Formation screen's card.

| Outfit | HUD face | Battle face |
|---|---|---|
| KINGDOM HEARTS III | KH3's `FacePcExSora01` | KH3's |
| KINGDOM HEARTS II | KH3's `FacePcEx2Sora01` | KH3's |
| KINGDOM HEARTS | KH3's `FacePcEx1Sora01` (its KH1 illustration) | none: KH3 shows the same picture for every look (normal, battle, hurt, low HP), and so does the mod (your decision 2026-11-03: "just use KH3's KH1 sora face") |
| Toy Box, Monstropolis, The Caribbean, San Fransokyo | KH3's `FacePcTsSora01`, `FacePcMiSora01`, `FacePcCaSora01`, `FacePcBxSora01` (packed 2026-11-03) | KH3's |
| Quadratum (Verum Rex) | KH3's `FacePcExSora01`: KH3's own data gives this outfit his usual face (its colours are the same black and red) | KH3's |

Pack: the four world-costume faces are in `tools/sora_ui_pack.js`'s movie list, so every player's setup packs them from their own KH3 install (this PC's pack got them with the targeted `node tools/sora_ui_pack.js --only=FacePcTsSora01,FacePcMiSora01,FacePcCaSora01,FacePcBxSora01`; `FacePcEx1Sora01` was already in the list). Pictures: `build/sora_tmp/hud/of_sheet.png` (every outfit: normal, battle, low HP, friend slot).

**Outfits: only through Boc (Implemented 2026-11-02 at your request, untested in game).** You asked: "Move the Outfits selection to Boc, replacing his armor alteration menu with it completely (this means it is only accessible by speaking with Boc)".
- **Boc's "Alter garments" opens the KH3 Outfits screen.** Elden Ring's alteration shop never opens. The mod recognises the talk command that opens the shop (command 142, `c1_142(110000, 110399)`), not Boc's identity, so this works in all five of his talk scripts, wherever he stands. Source: the game's own talk scripts (`kh-er/src/talkkh/alter.rs`).
- **His other options and his quest are unchanged.** While the Outfits screen is up, his script sees the shop as open (menu type 26). When you close it, his talk list comes back.
- **On the pad:** up / down picks an outfit, **A** wears it, and **B** (or Menu) closes the screen. It uses KH3's menu sounds.
- **Outfits is no longer in the camp menu**, and no other menu leads to it. The site of grace's own "Alter garments" (after Boc's quest) is left alone, and the KH save point replaces that menu anyway.
- **The outfit you have on stays on**, even while Boc can't be reached (it is the `outfit:` record in Sora's save).
- `boc.outfits = 0` gives Boc's own shop back, with KH3's default outfit. `boc.gate` no longer does anything.

**Quadratum (Verum Rex): his look (fixed 2026-11-03 at your request "verum rex Sora is still very broken looking", tested in game with RT MAX and RT OFF; staged, not deployed).** He is KH3's realistic Sora (pawn `p_dl001`, Re Mind). His materials are KH3's own DLC shaders, translated from the game's compiled shaders: skin `SQEX_dl_skin_Basemat` (subsurface profile), mouth `SQEX_DLC_Mouth`, eyes `SQEX_DLC_EyeBaseMat` + `SQEX_EyeLens`, lashes / brows `SQEX_DLC_MatsugeBaseMat` / `SQEX_DLCMayugeMaseMat`, clothes `SQEX_CHRSPTexBaseMat` (5 slots), hair `SQEX_KHHairBaseMat` (core / alpha / ksk), Keyblade `w_allBaseMat_UseReal`. His world's own post-process material composes him: `KHS_CG` (the Quadratum's "CG World", controller of `ss_01`), not the KH World's toon colour table.
- **What was broken (seen in game):** his irises near black; his hair, skin and cloth flattened into toon bands; a washed, bluish brightness above everyone else's; orange hair. Causes: (1) values chosen by hand in 2026-10 against a studio render, not KH3's (`own_look.json`: iris scatter `EyelsisSSCMulti` 25 instead of the game's 45, hair colour x2.8, a brightness lift up to x1.9); (2) the hybrid light (Elden Ring's light on KH characters) treated him as a toon character: its soft band staircase and a studio key of ours, while KHS_CG has no colour table and takes the light as received; (3) the hybrid gave his Subsurface materials (hair `HairSSColor`, irises, cloth) their scatter term for the sun alone, so under the sky, in shade or by a torch it was missing.
- **Now:** no value is written over KH3's (`own_look.json`: `materials` empty, `field` off); under the hybrid his outfit is "the realistic family": the light as received (staircase 0), no rig light of ours, and KH3's own scatter term on the game's light less the sun's (`er_char.wgsl` `kh_real_sss`, as KH3's light shader gives it). Result: blue irises, a black jacket with its weave, navy trousers, natural skin, dark-brown hair with strand light (KH3's own hair colour), modelled by the world's light like KH3 Sora beside him.
- **Remaining differences:** he is lit by Elden Ring's world, not the Quadratum's night rig (sun 0.8, sky 1.3, eight warm spots) or a studio key, so his hair reads darker than in the official render; there is no screen-space subsurface blur for his skin; `r.hybrid = 0` (the old path) gets the world's light unshaped.
- **Keys:** `boc.verumrex.own_look = 0` = no look file (the hybrid's KH World defaults again, game values); `boc.verumrex.own_shaders = 0` = the KH World set.
- **Pictures:** `build/sora_tmp/er/verumrex/game/` (`F_rtmax.png`, `H_rtoff.png`, `AB_sun.png`, `AB_face.png`: before / after in game), offline `build/sora_tmp/er/verumrex/offline/`.
- **Deploy (together):** the 25 files of `build/sora_tmp/er/verumrex/pack/` onto `build/sora_pack/` (its `own_look.json` replaces ER85's; 24 rebuilt `outfits/verumrex/materials/er/*_cg2_hy3*.ps.dxil`; hashes in `pack_files.sha256`) and a DLL built from the tree (kh-er-render `cgfield.rs` `RealLook`, `gfx.rs` hooks). Old DLL + new files: the look file's new block is ignored (he is drawn with KH3's values and the KH World defaults). New DLL + old files: as before. Release: `own_look.json` is replayed as raw text by packgen (`tools/packgen/replay/files_outfits_verumrex.json` holds ER85's sha256 5637722e...): regenerate that replay and bump the pack epoch. The DXIL is not in the replay (the setup's shader build makes it from the harness, which now carries `kh_real_sss`), so a release DLL + setup needs nothing else.

**The camp menu's button guide (fixed 2026-11-02).** After Party Formation (or any screen with a guide) was closed, the next camp menu still showed that screen's guide on its top level, for example "(A) Add / Remove (Y) Make Leader (B) Back". Now the guide is taken down when the menu closes and when it opens. Each screen shows only its own guide, and the top level shows none (`kh-ui/src/camp_menu.rs` `drop_guide`). Stats now shows KH3's "Select Friend" guide when friends are in the party.

**The game over: silence and Continue (Implemented 2026-11-03 at your request).** You asked: "silence all music and other SFX when on death screen, make 'continue' simply load the latest save game as if it was selected from load game", then "(except for menu SFX, dont silence that)".
- **What goes silent.** When the leader dies, all of Elden Ring's sound fades out over 1 s, as KH3's black screen rises. That covers its music, ambience, sound effects and voices. Our own KH sounds fade out too: Sora's and the party's voices, battle and world sounds, and the low-HP alarm.
- **What still plays.** KH3's game-over loop (static SE 21, `se00000_046`), which KH3 itself plays on its game-over screen, and every menu sound: the menu opening, the cursor, confirm, cancel, a refused choice, and the Load screen's sounds.
  - Menu sounds started on the death screen are marked one by one, so a sound category they share with the world can't catch them.
  - Our sounds come out of the mod's own audio output, which Elden Ring's volume never reaches.
- **How Elden Ring is silenced.** Through the game's own master volume, the one it uses to go quiet when its window closes. It is not your Options volume, and nothing is saved. `kh-er/src/gosound.rs` explains it in full.
- **When the sound comes back.** It fades back in over 0.5 s as soon as the game over leaves the screen. That happens when:
  - the game's loading screen starts (Continue, Load);
  - the scene ends;
  - you go to the title, or the mod's screens stop for more than 1 s;
  - the mod is switched off.
  It never stays muted.
- **Continue loads your latest save.** It loads it exactly as if you picked it in Load Game, by the same route and behind the same black screen, and the loaded character starts fresh.
  - "Latest" means, of the slots the Load screen lists, the one saved most recently. That is the slot whose Sora save file was written last. A save point's Save and the Save screen both write it.
  - The game's play time isn't used: two characters' play times say nothing about which one was saved last.
  - If no listed slot has a dated save, Continue loads your current slot, as it always did before. If even that is unknown, Elden Ring's own death and respawn follow (also as before).
- **A load after any death starts the loaded leader on the ground.** The in-game test found this case: Goofy was leading, died in the middle of a hop, and you picked Continue. Donald loaded at the grace, but the game's body was then moved to where Goofy had been before the load, 20 m up, and fell forever. The leader is now put on the loaded body at once, with his motion stopped, and no stale place from before the load is ever written.
- Keys: `gameover.silence = 0` keeps the old behaviour (no silence). `gameover.mute_fade` (default 1.0 s) and `gameover.unmute_fade` (default 0.5 s) set the fade times.

**Difficulty (Implemented)** (`IMPL_DF_STATE.md` revision of 2026-10-16; `sheets/sora-er-host.json` row `difficulty.new_game` = `STANDARD`).
- **Standard is the default** for new games. On Standard, everything is as before the difficulty work:
  - spell MP costs ×1.5 (`magic.mp_cost_scale`);
  - MP Charge 50 s;
  - no Critical abilities at the start.
- **Critical is an option.** Set `difficulty = critical` in the ctl file before starting a new game. Critical gives:
  - halved base HP / MP (Balance: HP 52 / MP 55);
  - AP +30 (58 at level 1);
  - Elden Ring hits on him ×4 of the Standard share;
  - EXP ×0.75;
  - MP Charge 70 s;
  - KH3's normal spell MP cost (×1.0).
- **No Critical Counter at the start**, even on Critical (your decision dec032). He starts on Critical with Critical Recharge and Critical Converter, plus KH3's 13 Critical start abilities. 54 of his 58 AP are in use.
- Existing characters keep their difficulty. A switch exists only as a ctl key, off by default (`difficulty.existing = switch`).
- **Up to 5 enemies may attack at once.** This applies on every difficulty, through Elden Ring's own group-attack limiter (`enemy.max_attackers` = 5; `IMPL_ENG_STATE.md`, `kh-er/src/attackers.rs`).

---

## 2. Stats

**Where his stats come from** (`sheets/sora-levels.json`, `sora-newgame.json`, `sora-equipment.json`):
- **Strength, Magic, Defense, AP** come from the level table, plus the Keyblade and gear. The Kingdom Key adds Strength +4 and Magic +3.
- **Max HP and MP do NOT grow with levels** (as in KH3). They grow only from boss rewards, some converted Elden Ring items and roster boosts (section 7).

**Key levels, Standard** (AP on Critical = +30). Strength and Magic include the Kingdom Key's +4 / +3; Defense is with no armour.

| Level | Total EXP | Strength | Magic | Defense | AP |
|---|---|---|---|---|---|
| 1 | 0 | 15 | 15 | 8 | 28 |
| 10 | 5,410 | 19 | 20 | 12 | 40 |
| 20 | 29,440 | 24 | 25 | 18 | 62 |
| 30 | 87,850 | 29 | 31 | 23 | 76 |
| 40 | 197,560 | 35 | 36 | 28 | 90 |
| 50 | 376,030 | 40 | 41 | 34 | 118 |
| 60 | 599,450 | 45 | 47 | 39 | 134 |
| 70 | 828,820 | 50 | 52 | 44 | 148 |
| 80 | 1,064,140 | 56 | 57 | 49 | 162 |
| 90 | 1,305,410 | 61 | 62 | 55 | 174 |
| 99 | 1,527,640 | 66 | 67 | 59 | 180 |

**HP / MP by bosses beaten** (Balance start; Standard / Critical). These are the DF note's offline numbers, from the boss table's HP / MP steps (`IMPL_DF_STATE.md` table 4).

| Bosses beaten | Max HP | Max MP |
|---|---|---|
| 0 | 105 / 52 | 110 / 55 |
| 9 | 135 / 67 | 120 / 59 |
| 24 | 185 / 91 | 135 / 65 |
| all rows, DLC included | 385 / 191 | 186 / 83 |

The other fixed numbers: Focus 100 on every difficulty. At the start: 3 item slots, 1 armour slot, 1 accessory slot.

**How EXP and levels work here (Implemented).**
- A defeated Elden Ring enemy gives `ceil(its max HP × 0.03)` KH3 EXP (`exp.per_max_hp`). On Critical it is ×0.75. KH3's cap of 999 per kill applies (`IMPL_DF_STATE.md`).
- EXP goes into **one shared party total**. Each character reads his level from that total with his own level table (`kh-partyhost/src/progression.rs`).
- **Zero EXP belongs to the party leader**, whoever leads. Its on/off state carries over when you change leader (your rule).
- Level-ups give Strength / Magic / Defense / AP and the path's ability (section 4.2).
- Elden Ring's own rune levelling is not part of Sora's progression. Runes become munny (section 7).

---

## 3. Combat kit

| Part | What is in FINAL10 | Status |
|---|---|---|
| **Ground combo** | KH3's Keyblade combo: normal, approach (far) and crowd hits, then the finisher. Combo Plus copies lengthen it. | Implemented (`sheets/sora-combat-attacks.json` kinds 1-4) |
| **Air combo** | KH3's air combo: normal, approach, crowd, falling-attack and finisher hits. Air Combo Plus lengthens it. | Implemented (kinds 0x0F-0x13) |
| **Finishers** (ground and air) | Basic finisher. With the abilities: Speed Slash, Triple Rush, Magic Flash, Last Charge, Falling Slash. XS finishers: Spiral Rush, Zantetsuken, Event Horizon (replaces Magic Flash), Giant Stamp, Crimson Blast (Riskcharge). | Implemented |
| **Re Mind combo swaps** | Quick Slash, Flash Step, Radial Blaster, Aerial Sweep and Aerial Dive replace combo steps when equipped | Implemented (DLC bosses give them) |
| **Launchers** | Rising Spiral (ground), Groundbreaker (air). Air juggles on Elden Ring enemies are limited by the "not infinitely stunnable" rule: 3 reactions in 10 s, then 8 s with none (`sora-er-host.json` `react.chain.*`). | Implemented |
| **Guard** | Block (X). Reprisals after a block: Counter Slash, Counter Impact, Counter Kick, Final Blow. Payback Strike after Aerial Recovery. An Elden Ring blow on his guard (leader or AI Sora) plays KH3's guard sound `se00001_037` (the sound of KH3's common guard effect `vs_com_haj00`), never a hit sound, and leaves no blood. While his guard is up the hidden Elden Ring body's own sounds are held off (2026-11-02; `kh-er/src/guardblow.rs`, sheet rows `guard_sound.*`; the hold is untested in game). | Implemented |
| **Enemy shields** | (2026-11-02, your request "Shields don't count as guarding ... bounce off playing their animation from KH3") An Elden Ring enemy whose own guard is up (its TAE guard flag, read every frame for every enemy; its NpcParam guard angle and cut rates) is a GUARDING body to KH3's rules, from the front only (the row's guardAngle read as the full arc, else the front half). A swing into it is GUARDED: KH3's guard spark `vs_com_haj00` (clang `se00001_037`), no hit sound, no KH3 reaction, and Sora bounces off with KH3's own recoil (`TresPlayerState_Reflect`, `AttackReflect` / `AttackAirReflect`), on ANY swing of a combo (KH3's own guards repel only the first; the combo ends). Damage and guard pressure are Elden Ring's: the hit still goes to the game as his bullet without stagger or poise, so the enemy's own guard takes its cut, its stamina and breaks itself. From behind: a plain hit. Spells are guarded too but never bounce him. Sheet rows `guard.shield.*` (`kh-er/src/shieldguard.rs`, `kh-core` `TargetBody::shield` / `seen_from`). | Implemented, untested in game |
| **Blood** | Blood builds up slowly (2026-11-02, a quarter of the old rate): about 67 landed hits for a fully bloody Keyblade; on him about 200 landed hits, or 50 blows that each take a tenth of his HP. It dries, fades over an hour, washes off in rain or water, and a save point clears it (`sora-er-host.json` `blood.*`). | Implemented |
| **Critical Counter** | A block in the last 0.1333 s before a hit makes the reprisal deal ×2. Not given at the start (section 4). | Implemented |
| **Dodge** | Dodge Roll, Aerial Dodge, Air Slide, Superslide; Risk Dodge with its four counters | Implemented |
| **Movement** | High Jump, Doubleflight, Superjump and Glide, all boss rewards. Ladders (LADDER2, 2026-10-25): press Y at a ladder's Climb prompt and the party jumps up the whole ladder in one leap, with Torrent's wind sound, and lands where the Tarnished would step off at the top, facing the same way. There is no climbing down: just drop off the ladder (the Descend prompt is disabled). No fall damage (XS notes: "already the mod's rule"). | Implemented |
| **Death barriers** | (2026-11-03, your request "KH characters should die from Elden Ring's death barriers (like the ocean)") Elden Ring's barriers are special collision its own contact query finds (the physics module's tracker, filter 0x66): kind 15 kills at once (the sea's kill plane, a pit's floor), kind 14 makes the next landing lethal, kind 13 is the fall-death camera over a pit. The hidden body never died of them because the mod's no-death switch makes the game drop those contacts. Now the same query runs at every KH character's place each frame: the leader dies his normal death (Sora: KH3's knockdown and the game over; a friend leader: his KO and the game over), Donald / Goofy / AI Sora are knocked out, Mimic Sora goes down and leaves. Spirit ashes are the game's own and die by its rule. Ordinary falls stay harmless. The old drowning guess (dying after 0.6 s 150 cm under any water surface) is retired: `water.death` is 0 by default (1 brings it back for debugging), so a KH character dies only where Elden Ring itself kills. Key `barrier.death` (1), log `barrier:` (`kh-er/src/deathplane.rs`). | Implemented; seen in game 2026-11-03 (Sora and Goofy leading died in the sea with KH3's game over and Continue; AI Donald KO'd) |
| **Flowmotion / Athletic Flow** | The states are in the shared core: airstep travel, flow attacks Sonic Slash / Sonic Cleave / Buzz Saw / Somersault, Wall Kick, wall run, rail slide, pole spin and pole swing. Elden Ring's world has no KH3 flow markers (`SQEX_bEnableWallRun` etc.), so wall run, rail and pole moves have not occurred in Elden Ring (`IMPL_ER7`, `IMPL_ER58`). Whether airsteps to enemies trigger in Elden Ring: **unknown**. | Ported in core; in Elden Ring effectively absent / unknown |
| **Shotlocks** (hold RB) | Kingdom Key's two (sheet names: "King of Hearts", "Ragnarok"). Spectral Rays *[XS]* is picked in Customize "Shotlocks". Your nerf applies: damage ×0.5, at most 40 % of the target's max HP per shotlock, 3 s gap. **Aim sound (2026-11-02, your report "startup sfx continues ... after it's cancelled" / "way too loud"):** the aim's startup sound (KH3's static SE 18 `se00000_039`, 7 s) now stops with KH3's own 0.6 s fade the moment the aim ends, whatever ends it (R1 let go, a hit, death, the menu's slow option, a warp or load, the shot, the airstep, a menu / game over / cutscene taking the screen: one rule, `kh-host/src/hud.rs` `scope_sound_rule`). It plays at KH3's own level: before it got the menus' +12 dB (3.36 of full scale before the output level, clipped), now ×1.00 (peak 0.84, like Sora's combat sounds). A warp during the aim also ends the slow-motion sound state now (footsteps no longer stay at low pitch). | Implemented, untested in game |
| **Mod sound safety** | (2026-11-02, your report "sometimes Audio from specifically our mod will become very loud") Causes found and fixed: (1) every HUD sound got the menus' +12 dB, so loud ones (the game-over loop, the shotlock aim, the lock marks) overdrove the mix: your log clipped 625 samples at a peak of 1.457 at both game overs; the gain now stops at each sound's own peak 0.8 (sheet row `sound.ui_peak_ceiling`, key `snd.ui_peak`); (2) after a load or respawn the audio engine restarted its numbering but the menus' sound list was kept, so some of Sora's first sounds got the +12 dB; now cleared; (3) the title ignored KH3's Master Volume row. Added: a limiter on our output (-0.5 dBFS, nothing clips), KH3's per-asset instance limits (`ConcurrencyOverrides`, 16 by default), and a log line each second `sound: mix this second: peak ... limiter cut ... dB ... HUD sounds raised up to x...` plus `!!! sound: THE MIX RAN OVER FULL SCALE` naming the loudest sounds when it happens. | Implemented, untested in game |
| **Situation commands** (Y) | Second Form, Grand Magic, team attacks, form finishers, Riskcharge, Ancient Light | Implemented |
| **Second Form** (formchange) | The Kingdom Key's only formchange. The situation gauge fills from hits; when it fills, KH3's `RollGrandMagic` rule decides whether Second Form is offered beside Grand Magic. Critical Converter makes it always offered. Form finishers: Stun Impact / Sonic Blade / Ars Arcanum; form combo finishers: Explosion, Stride Break, Ripple Drive, Hurricane Period, Aerial Finish, Magnet Splash. Formchange Extender lengthens it. Rage Form (anti form) is **not** in the port. | Implemented |
| **Grand Magic** | Offered on Y when the gauge fills from magic hits. The "-za" tiers need the matching ability (Firaza, Blizzaza, Thundaza, Waterza, Aeroza from the Cufflink accessories). No MP cost. | Implemented |
| **Team attacks: Donald** | Unison Fire (level 8). Unison Blizzard and Unison Thunder: **no source**, see section 9. Donald Meteor, Flare Force. | Partly implemented |
| **Team attacks: Goofy** | Fusion Spin (level 10), Fusion Rocket (Leonine Misbegotten), Goofy Bombardier, Trinity Guard (the group Shield Guardian; you kept it) | Implemented |
| **Links** (summons) | None. The Link entry of the command menu is disabled (`kh-core/src/command/mod.rs`). | Not in the port |
| **Attraction Flow** | **Excluded by your decision.** Attraction Extender and Attraction Enhancer were removed and their level slots refilled (X1; `IMPL_X1_STATE.md`). | Excluded |

---

## 4. Abilities: when and how he gets each

AP is the sheet's cost to equip. A boss reward or a level gives one **owned** copy; you equip it on the Abilities screen while your AP allows.

Sources: `sheets/sora-abilities.json`, `sora-levels.json`, `sora-newgame.json`, `sora-er-boss-rewards.json`, `sora-equipment.json`, `sora-er-items.json`. Boss rewards are once per character.

### 4.1 At the start (Implemented)

| Ability | AP | Notes |
|---|---|---|
| Scan, Dodge Roll, Air Slide, Aerial Dodge, Block, Superslide, Assist Friends | 1, 3, 1, 3, 2, 1, 1 | Equipped |
| Sonic Slash, Sonic Cleave, Buzz Saw, Somersault (flow attacks) | 2 each | Equipped |
| Pole Spin, Pole Swing, Wall Kick | 1 each | Equipped |
| Damage Control, Magic Lock-On | 4, 1 | Equipped |
| Zero EXP | 0 | Owned. It belongs to whoever leads. |
| Treasure Magnet | 0 | From the Kingdom Key itself |
| Critical only: Critical Recharge (3) equipped; Critical Converter (1) owned; KH3's 13 Critical start abilities (Aerial Recovery, Counter Slash, Rising Spiral, Counter Impact, Combo Master, Groundbreaker, Risk Dodge, Air Slide ×2, Superslide ×2, Counter Kick, Final Blow) | - | **No Critical Counter** (dec032) |

### 4.2 Level-ups, by path (Implemented)

A = Warrior, B = Mystic, C = Guardian (`sheets/sora-levels.json`). The ★ rows are the X1 slots refilled with XS abilities.

| Level | A (Warrior) | B (Mystic) | C (Guardian) |
|---|---|---|---|
| 2 | Combo Plus | Magic Combo Thrift | Damage Control |
| 3 | Treasure Magnet | Treasure Magnet | Treasure Magnet |
| 4 | Air Combo Plus | MP Haste | Withstand Combo |
| 5 | Auto-Finish | Auto-Finish | Auto-Finish |
| 6 | MP Haste | Combo Plus | Combo Plus |
| 8 | Unison Fire | Unison Fire | Unison Fire |
| 9 | Combo Plus | Fire Boost | Block Replenisher |
| 10 | Fusion Spin | Fusion Spin | Fusion Spin |
| 12 ★ | **Spiral Rush** *[XS]* | Water Boost | MP Haste |
| 14 | Fire Boost | Leaf Bracer | Air Combo Plus |
| 16 | Air Combo Plus | Air Combo Plus | Fire Boost |
| 18 ★ | Water Boost | Blizzard Boost | **Critical Counter** *[XS]* |
| 20 | Combo Boost | Grand Magic Extender | Reprisal Boost |
| 22 ★ | **Diving Strike** *[XS]* | **Diving Strike** *[XS]* | **Diving Strike** *[XS]* |
| 24 | Air Combo Boost | Reprisal Boost | Combo Plus |
| 26 | Withstand Combo | Magic Galvanizer | Leaf Bracer |
| 28 | Leaf Bracer | Withstand Combo | Water Boost |
| 30 | Blizzard Boost | Combo Plus | Second Chance |
| 32 | Link Extender | Damage Control | Air Combo Plus |
| 34 | Thunder Boost | Thunder Boost | Link Extender |
| 36 | Block Replenisher | Block Replenisher | Combo Boost |
| 38 | Aero Boost | Aero Boost | Blizzard Boost |
| 40 | Reprisal Boost | Air Combo Plus | Thunder Boost |
| 42 | Grand Magic Extender | Second Chance | Air Combo Boost |
| 44 ★ | Magic Combo Thrift | **Light Wave** *[XS]* | Grand Magic Extender |
| 46 | Magic Galvanizer | Combo Boost | Magic Combo Thrift |
| 48 | Second Chance | Link Extender | Aero Boost |
| 50 | Damage Control | Air Combo Boost | Magic Galvanizer |

Nothing is learned after level 50. Each path holds 28 level abilities.

### 4.3 Boss rewards: abilities, magic, HP / MP, slots (Implemented)

**The rules** (`kh-host/src/bossrewards.rs`):
- Each row gives once per character when the boss's defeat flag is seen. Only you fixed the *[user]* items; every other item is a *proposal* (built and active, not confirmed by you).
- **Magic gives "one tier more of that element"**, whatever tier the row names. Beat the bosses in any order and you still go up one tier at a time.
- A *[XS]* ability gives nothing if he already owns it from his level path ("skip if owned").
- The "Great Rune order" slots go: item, accessory, armour, item, accessory, armour, item. The Nth such boss gives the Nth slot.
- **The flag ids come from the game's event scripts** (section 9). `boss.grant = <row>` in the ctl gives a row by hand.

| # | Boss | Region | Path | Sora's reward |
|---|---|---|---|---|
| 1 | Tree Sentinel | Limgrave | optional | Max HP +10; Spiral Rush (3 AP) *[XS]* |
| 2 | Flying Dragon Agheel | Limgrave | optional | Max HP +10; Sonic Dash (3 AP) *[XS]* |
| 3 | Margit, the Fell Omen | Limgrave (Stormveil gate) | main | **Thunder** *[user]*; MP Safety (0 AP) |
| 4 | Leonine Misbegotten | Weeping Peninsula (Castle Morne) | optional | Speed Slash (3 AP); Max HP +10; Fusion Rocket (3 AP) |
| 5 | Godrick the Grafted | Limgrave (Stormveil Castle) | main | **Cure** *[user]*; Counter Slash (4 AP); slot (Great Rune order) |
| 6 | Royal Knight Loretta | Liurnia (Caria Manor) | optional | **Blizzard**; Rising Spiral (3 AP) |
| 7 | Glintstone Dragon Smarag | Liurnia | optional | Grand Magic Extender (3 AP); Max MP +5; Spectral Rays (3 AP) *[XS]* |
| 8 | Red Wolf of Radagon | Liurnia (Raya Lucaria) | main | Aerial Recovery (2 AP); Max MP +5 |
| 9 | Rennala, Queen of the Full Moon | Liurnia (Raya Lucaria) | main | **Blizzara**; High Jump (2 AP); slot (Great Rune order) |
| 10 | Magma Wyrm Makar | Liurnia (Ruin-Strewn Precipice) | optional | Falling Slash (2 AP); Max HP +10 |
| 11 | Ancestor Spirit | Siofra River | optional | **Watera**; Treasure Magnet (1 AP) |
| 12 | Dragonkin Soldier of Nokstella | Ainsel River | optional | Groundbreaker (3 AP); Max MP +5; Break Boost (4 AP) *[XS]* |
| 13 | Starscourge Radahn | Caelid | main | **Aero**; Superjump (2 AP); Max MP +5; slot (Great Rune order) |
| 14 | Mimic Tear | Nokron | optional | Formchange Extender (3 AP); Max HP +5; Hidden Potential (5 AP) *[XS]* |
| 15 | Regal Ancestor Spirit | Nokron | optional | Focus Syphon (3 AP); Max HP +5 |
| 16 | Valiant Gargoyles | Siofra Aqueduct | optional | **Waterga**; Air Slide (1 AP) |
| 17 | Astel, Naturalborn of the Void | Lake of Rot (Grand Cloister) | optional | Magic Flash (5 AP); Max MP +5; Light Wave (4 AP) *[XS]* |
| 18 | Lichdragon Fortissax | Deeproot Depths | optional | **Thundaga**; Max HP +10 |
| 19 | Elemer of the Briar | Altus Plateau (Shaded Castle) | optional | Master Treasure Magnet (5 AP); Giant Stamp (5 AP) *[XS]* |
| 20 | Godskin Noble | Mt. Gelmir (Volcano Manor) | optional | Risk Dodge (3 AP); Max HP +10; Critical Counter (5 AP) *[XS]* |
| 21 | Rykard, Lord of Blasphemy | Mt. Gelmir (Volcano Manor) | optional | **Fira**; Triple Rush (4 AP); slot (Great Rune order) |
| 22 | Draconic Tree Sentinel | Capital Outskirts | main | **Thundara**; Counter Impact (4 AP) |
| 23 | Godfrey (golden shade) | Leyndell | required | **Cura**; Doubleflight (2 AP) |
| 24 | Morgott, the Omen King | Leyndell | required | Final Blow (5 AP); Max HP +10; slot (Great Rune order); Ancient Light (6 AP) *[XS]* |
| 25 | Mohg, the Omen | Leyndell (Shunning-Grounds) | optional | Combo Master (4 AP); Max HP +10; Rage Raid (4 AP) *[XS]* |
| 26 | Commander Niall | Mountaintops (Castle Sol) | optional | **Blizzaga**; Superslide (1 AP) |
| 27 | Fire Giant | Mountaintops | required | **Firaga**; Counter Kick (4 AP); Max HP +10 |
| 28 | Mohg, Lord of Blood | Mohgwyn Palace | optional | Max HP +10; slot (Great Rune order); Riskcharge (5 AP) *[XS]* (replaced his Focus Syphon) |
| 29 | Loretta, Knight of the Haligtree | Haligtree | optional | Superslide (1 AP); Max MP +5; Chandelier (3 AP) *[XS]* |
| 30 | Malenia, Blade of Miquella | Haligtree (Elphael) | optional | Focus Syphon (3 AP); Air Slide (1 AP); Max HP +10; slot (Great Rune order) |
| 31 | Godskin Duo | Farum Azula | required | **Aerora**; Payback Strike (5 AP) |
| 32 | Dragonlord Placidusax | Farum Azula | optional | **Aeroga**; Grand Magic Extender (3 AP); Max HP +10 |
| 33 | Maliketh, the Black Blade | Farum Azula | required | **Glide** *[user]*; **Curaga** |
| 34 | Sir Gideon Ofnir | Leyndell, Ashen Capital | required | Max HP +10; Max MP +5 |
| 35 | Godfrey / Hoarah Loux | Leyndell, Ashen Capital | required | Max HP +10; Max MP +5 |
| 36 | Radagon / Elden Beast | Erdtree | required | Max HP +10 |
| 37 | Divine Beast Dancing Lion | DLC (Belurat) | main | Flash Step (3 AP); Max MP +3; Max HP +10 |
| 38 | Rellana, Twin Moon Knight | DLC (Castle Ensis) | main | Quick Slash (3 AP); Max MP +3; Zantetsuken (4 AP) *[XS]*; Max HP +10 |
| 39 | Golden Hippopotamus | DLC (Shadow Keep) | main | Max MP +3; Max HP +10 |
| 40 | Messmer the Impaler | DLC (Shadow Keep) | required | Last Charge (7 AP); item slot +1; Max HP +10; Max MP +3 |
| 41 | Commander Gaius | DLC | optional | Radial Blaster (3 AP); Max HP +10; Max MP +3 |
| 42 | Putrescent Knight | DLC (Stone Coffin Fissure) | optional | Max MP +3; Max HP +10 |
| 43 | Scadutree Avatar | DLC | optional | Max MP +3; Max HP +10 |
| 44 | Metyr, Mother of Fingers | DLC | optional | Max MP +3; Event Horizon (4 AP) *[XS]*; Max HP +10 |
| 45 | Midra, Lord of Frenzied Flame | DLC (Midra's Manse) | optional | Max MP +3; Max HP +10 |
| 46 | Romina, Saint of the Bud | DLC | required | Aerial Sweep (2 AP); Max MP +3; Max HP +10 |
| 47 | Bayle the Dread | DLC (Jagged Peak) | optional | Aerial Dive (2 AP); Max HP +10; Max MP +3 |
| 48 | Promised Consort Radahn | DLC (Enir-Ilim) | required | item slot +1; Myriad (7 AP) *[XS]*; Max HP +10; Max MP +3 |

On Critical, a boss ability that Critical already gave at the start is skipped; the boss's other rewards still come.

**Small bosses** (any boss-bar enemy not in the table above) each give one capped reward, once (`small.*` rows, proposal):
- every 3rd: a permanent +1 to Strength, Magic, Defense and AP in turn, for the whole roster;
- the others alternate between a recovery item (Hi-Potion up to Megalixir by tier) and an armour piece or accessory;
- the tier is capped by region.

### 4.4 The 16 new abilities (XSI, Implemented)

Design: `XS_SORA_ABILITIES.md`. Build state: `IMPL_XSI_STATE.md`, Elden Ring hookup `IMPL_ER90_STATE.md`. All 16 are in FINAL10; the form weapons and effects are drawn in Elden Ring.

| Ability | AP | How he gets it | What it is (pad) |
|---|---|---|---|
| Diving Strike | 2 | Level 22, every path | KH3 plunge: A while falling, with ≥ 6 m of drop and an enemy below. Damage grows with the fall. |
| Sonic Dash | 3 | Flying Dragon Agheel | In the air, locked on 4-15 m: X + stick at the target. Dashes through; A after the 2nd dash launches. |
| Spiral Rush | 3 | Level A 12, or Tree Sentinel | Light Form drill charge finisher, when the target is 5-12 m away |
| Zantetsuken | 4 | Rellana (DLC) | Hold A on the finisher step: a charged cut (up to ×1.6) |
| Event Horizon | 4 | Metyr (DLC) | Light-ring magic finisher (replaces Magic Flash, scales with Magic) |
| Giant Stamp | 5 | Elemer of the Briar | Hyper Hammer slam finisher against big foes. The hammer is shown for the move. |
| Chandelier | 3 | Loretta, Knight of the Haligtree | Blizzard finish becomes Blizzard Blades' ice chandelier |
| Spectral Rays | 3 | Glintstone Dragon Smarag | A second shotlock: refracting lasers. Pick it in Customize. |
| Light Wave | 4 | Level B 44, or Astel | During MP Charge: tap for a piercing wave, hold for a 6-wave volley |
| Critical Counter | 5 | Level C 18, or Godskin Noble | The just-block (×2 reprisal). **Not** at the start on any difficulty. |
| Rage Raid | 4 | Mohg, the Omen | Counter when a boss breaks out of your juggle (its revenge) |
| Riskcharge | 5 | Mohg, Lord of Blood | Situation command: pay HP, then Crimson Blast |
| Ancient Light | 6 | Morgott | Second Form finisher outside the form (capped at 25 % of the target's max HP) |
| Myriad | 7 | Promised Consort Radahn (DLC) | Second Form ends in Ultimate Form's finale (25 % cap) |
| Hidden Potential | 5 | Mimic Tear | Passive: more Strength / Magic against stronger foes (by region band) |
| Break Boost | 4 | Dragonkin Soldier of Nokstella | Passive: breaks a boss's guard faster and keeps it open longer |

### 4.5 Abilities that come only from gear (Implemented, through loot)

Armour and accessories drop from enemies, corpses / world pickups, chests and merchants, by tier and region (`sheets/sora-er-items.json`). The 60 variant necklaces and medals (`ACC_ITEM51`-`110`) never drop.

| Ability | Worn on (droppable pieces) |
|---|---|
| Fire / Blizzard / Thunder / Water / Aero Boost (also level-ups) | Cufflinks, Celestriad, Yin-Yang Cufflink, Master / Star Medal |
| Firaza, Blizzaza, Thundaza, Waterza, Aeroza (the Grand Magic -za tiers) | The matching Cufflink |
| MP Haste / Hastera / Hastega | Sorcerer's Ring, Necklaces / Flanniversary Badge / Mickey Clasp, Crystal Regalia |
| MP Thrift, Extra Cast, Full MP Blast, Endless Magic | Flanniversary Badge, Rune Ring, Force Ring, Mickey Clasp |
| Damage Syphon | Chains, Aqua Chaplet, Aero Armlet, Moon Amulet, Star Charm, Cosmic Arts |
| Element Syphons (Fire, Blizzard, Thunder, Water, Aero, Dark) | Bangles, Chokers, Trinkets, Bandannas, Fans, Anklets |
| Burn / Cloud / Sneeze / Freeze / Electric Protection | The Rosettes |
| Defender | Soldier's / Fencer's Earring |
| Lucky Strike, Wizard's Ruse, Item Boost | Lucky Ring, Wisdom Ring, Hero's Glove |
| Magic Treasure Magnet, Master Treasure Magnet (also a boss) | Mage's / Slayer's Earring, Draw Ring |
| MP Walker, HP Walker | Laughter Pin, Forest Clasp |
| HP / MP / Munny Converter | Bronze / Silver / Gold Amulet |
| Cure Converter | Buster, Valor, Phantom, Orichalcum Ring, Breakthrough |
| Harvester, Chef Extraordinaire | Gourmand's Ring |
| Frontline Photographer | Hero's Belt |

### 4.6 Abilities with no source in the mod now

| Ability | Why there is no source |
|---|---|
| Unison Blizzard, Unison Thunder | Decided (XD decision 3) for Royal Knight Loretta and the Draconic Tree Sentinel, but **missing from the boss sheet** (section 9) |
| Focus Converter, Stun Protection, Situation Boost, Magic Roulette | Only on Keyblades the mod cannot give (section 6) |
| Critical Recharge, Critical Converter | Critical start only; nothing gives them on Standard |
| Berserk Charge, Curaza and the KH3 cuisine abilities (Hearty Meal etc.) | Cooking is not in the mod; their accessory rows are excluded variants |
| Attraction Extender, Attraction Enhancer | **Excluded** (your decision; X1) |

---

## 5. Magic

**The spells.** Sora has six elements, each with three tiers (`kh-core/src/magic/spells.rs`; names from KH3's strings): Fire / Fira / Firaga, Blizzard / Blizzara / Blizzaga, Thunder / Thundara / Thundaga, Water / Watera / Waterga, Aero / Aerora / Aeroga, Cure / Cura / Curaga.
- Grand Magic adds the -za versions (Firaza ... Aeroza) as situation commands.
- MP costs are KH3's: Fire costs 10, per the DF note. On Standard they are ×1.5. MP Charge after running dry lasts 50 s (Critical: 70 s, ×1.0 costs).

**When each tier unlocks (Implemented).**
- Each boss below gives **one tier more** of its element, whichever of that element's bosses you beat first (`kh-host/src/bossrewards.rs`).
- KH3's prologue gives Fire and Water at tier 1 (`sheets/sora-newgame.json` `magic.known`).

| Element | Tier 1 | Tier 2 | Tier 3 | The element's bosses | Extra boss (Planned, shared with Donald) |
|---|---|---|---|---|---|
| Fire | start | 1st boss | 2nd boss | Rykard, Fire Giant | Magma Wyrm Makar |
| Water | start | 1st boss | 2nd boss | Ancestor Spirit, Valiant Gargoyles | Dragonkin Soldier of Nokstella |
| Thunder | 1st boss | 2nd boss | 3rd boss | Margit *[user]*, Draconic Tree Sentinel, Lichdragon Fortissax | Ancient Dragon Lansseax |
| Cure | 1st boss | 2nd boss | 3rd boss | Godrick *[user]*, Godfrey (golden shade), Maliketh | Morgott |
| Blizzard | 1st boss | 2nd boss | 3rd boss | Royal Knight Loretta, Rennala, Commander Niall | Borealis the Freezing Fog |
| Aero | 1st boss | 2nd boss | 3rd boss | Starscourge Radahn, Godskin Duo, Dragonlord Placidusax | Red Wolf of Radagon |

The table's own order gives Fira at Rykard, Waterga at the Gargoyles and so on, but beating them out of order never skips a tier.

**Planned (user decision, with the Donald overhaul; not implemented yet):** each element gets the **extra boss** in the last column, shared with Donald (`documentation/DONALD_OVERHAUL.md`, section 9). Sora takes his next tier from **any** of that element's bosses. If he **already has the -ga tier** of that element, the boss gives him **Max MP +3** instead. Sora **only** gets something from a boss of his own magic: bosses that give Donald something Sora doesn't have (Flare and the other line-2 spells, the ultimates, the Arcane Y upgrades, Zettaflare) give Sora nothing.

**Grand Magic (Implemented).** Offered when the situation gauge fills from magic hits (`kh-core/src/magic/grand.rs`, KH3's `RollGrandMagic`). Two of its tiers need something extra:
- **-za tiers**: they need the element's Cufflink ability (section 4.5).
- **Curaza**: it needs Cure Converter, and Curaza has no source (section 4.6).

**Donald (Implemented).** Donald uses the same spells and tiers as Sora while you play as Donald (your decision; stream X2).

---

## 6. Keyblades

### 6.1 What is implemented now

**Only the Kingdom Key can be held** (`sheets/sora-newgame.json`, `sora-equipment.json`).

| Keyblade | Strength / Magic | Ability | Formchange | Shotlocks | How he gets it |
|---|---|---|---|---|---|
| **Kingdom Key** | +4 / +3 | Treasure Magnet | **Second Form** | "King of Hearts", "Ragnarok" | Start weapon (keyblade slot 0; slots 1 and 2 empty) |

What else exists:
- **Kingdom Key +1 to +10** exist as upgrade rows (+10: Strength 9 / Magic 8, adds Formchange Extender from +4). The mod has **no upgrade or synthesis system**: materials are "kept, nothing uses them" (`kh-host/src/items.rs`). So the upgrades cannot be reached.
- **Form weapons shown briefly (XS moves, not equippable):**
  - Hyper Hammer (Favorite Deputy's form) during Giant Stamp;
  - Blizzard Blades (Crystal Snow's form) during Chandelier;
  - the Ultimate Form weapon (Ultima Weapon) during Myriad.

  The Kingdom Key is hidden meanwhile (`IMPL_ER90_STATE.md`).
- **Elden Ring weapons never become Keyblades.** A picked-up weapon becomes munny, a synthesis material or an accessory (`sora-er-items.json` rules `r2.7xx`). By contrast, staves / seals and shields can become Donald's and Goofy's weapons (`party.weapons`).

### 6.2 What is planned or decided

**Nothing is decided for Keyblades:**
- `docs/sora-er-boss-rewards.md` open question 4: "Keyblades are still not assigned to any boss."
- The XS design dropped Double Form (two Keyblades at once) as "a later proposal if wanted".

### 6.3 The other Keyblades in the data, with a potential source

These 16 have full stat rows in `sora-equipment.json` and models / form variants in `sora-weapon.json`, but **no way to get them**.
- Formchange names are given only where this project's notes name them; "unknown" means the data holds the form but its name is not mapped here.
- The KH3 Formchange Extender upgrades and the keyblade-only abilities above would come with these.

| Keyblade | Str / Mag | Ability | Formchange | **Potential (suggestion):** where to get it |
|---|---|---|---|---|
| Hero's Origin | 5 / 2 | Defender | unknown | Godrick the Grafted: the first demigod, the hero's first great wall |
| Shooting Star | 3 / 5 | Magic Treasure Magnet | unknown | Rennala, Queen of the Full Moon: stars and sorcery, early magic Keyblade |
| Starlight | 4 / 4 | MP Haste | unknown (has a form-05 variant) | Early and cheap: sold by Merchant Kalé (Church of Elleh) |
| Favorite Deputy | 5 / 3 | Lucky Strike | Hyper Hammer / Drill Punch | Elemer of the Briar, so it matches Giant Stamp's hammer from the same boss |
| Ever After | 2 / 6 | Leaf Bracer | unknown | Glintstone Dragon Smarag, so it matches Spectral Rays ("Ever After's lasers") |
| Happy Gear | 6 / 2 | Focus Converter | unknown | Royal Knight Loretta (Caria Manor) |
| Hunny Spout | 4 / 4 | Harvester | unknown | Ancestor Spirit (Siofra River) |
| Nano Gear | 5 / 4 | Stun Protection | unknown | Valiant Gargoyles (Siofra Aqueduct): constructs for a tech Keyblade |
| Wheel of Fate | 6 / 4 | Waterza | unknown | Dragonkin Soldier of Nokstella (Ainsel River, water) |
| Crystal Snow | 3 / 6 | Freeze Protection | Blizzard Blades / Blizzard Claws | Commander Niall (snowy Mountaintops) |
| Grand Chef | 4 / 6 | Wizard's Ruse | unknown | A merchant: the Twin Maiden Husks (Roundtable Hold) |
| Classic Tone | 6 / 9 | MP Haste | unknown | Astel, Naturalborn of the Void: a late magic Keyblade |
| Oathkeeper | 7 / 7 | MP Converter | Light Form (Re Mind); Double Form with Oblivion | Morgott, the Omen King, beside Ancient Light |
| Oblivion | 8 / 6 | HP Converter | Dark Form (Re Mind); Double Form with Oathkeeper | Maliketh, the Black Blade |
| Ultima Weapon | 8 / 3 | Situation Boost, Combo Boost, Air Combo Boost | Ultimate Form | Radagon / Elden Beast (the last boss), or Malenia as the hardest optional fight |
| Dead of Night | 4 / 4 | Lucky Strike, Magic Roulette | unknown (has a form-05 variant) | DLC: Midra, Lord of Frenzied Flame |

**Potential (suggestion), building on these:**
- **Equipping:**
  - only Second Form is ported, so start each new Keyblade with its stats and ability only, without its formchange;
  - add each formchange later as its own job;
  - turn on keyblade slots 1-2 so you can switch Keyblades with the D-pad.
- **Kingdom Key upgrades:** let a smith take munny plus the materials the mod already converts Smithing Stones into: Fluorite, Damascus, Adamantite, Electrum, Orichalcum, Orichalcum+ (`sora-er-items.json` `r2.30x`). Smithing Master Hewg at the Roundtable fits.
- **Elden Ring weapons:** a rare chance for some Elden Ring weapons to give a Keyblade, the way staves and shields already give the friends' weapons.

---

## 7. Items, prizes and progression

| Thing | How it works now | Status / source |
|---|---|---|
| **Munny** | Runes become munny, 1 munny per rune. Enemies drop munny orbs (≈ 0.4 munny per enemy max HP, a guess). Merchants are priced in munny. | Implemented (`sora-prizes.json` `er.*`, `sora-er-items.json` `r2.100`, `shop.kh`) |
| **HP / MP / Focus orbs** | KH3 prizes drop from enemies. HP orbs are raised for Elden Ring: +4 small, +20 large. MP +3 / +5, Focus +3 / +5. Treasure Magnet widens the pull (300-900 cm). | Implemented (`sora-prizes.json`, `sora-er-host.json` `prize.*`) |
| **Usable items** (Potion … Megalixir) | Only from enemies, corpses / world pickups, chests, NPCs, bosses and shops. Plants and animals give **materials only**. | Implemented (your 2026-10-18 rules; `source.usable`) |
| **Flasks** | Crimson / Cerulean Flasks give **nothing** | Implemented (your decision) |
| **Golden Seed / Sacred Tear** | Megalixir | Implemented (your decision) |
| **Synthesis materials** | Elden Ring crafting and gathering items become KH3 materials, Smithing Stones become ores. They are **kept but unused**: no synthesis in the mod. | Materials Implemented; synthesis not in the mod |
| **Gloveworts** (your 2026-11-02 request) | Grave Glovewort [1]-[9], Great Grave Glovewort, Ghost Glovewort [1]-[9], Great Ghost Glovewort: each its **own KH3 upgrade material**, from wherever Elden Ring puts them. Roderika powers spirit ashes up with them on a **KH3-style Spirit Tuning screen**. | Implemented, **untested in game** (section 7.1) |
| **Stat boosts** | Permanent boosts go to **the whole roster**, unlocked or not, and a late joiner arrives with them. Sources: every 3rd small boss (+1 Str / Mag / Def / AP in turn), Whetblades (AP +1 each), Memory Stones (Max MP +5 each). | Implemented (your 2026-10-17 rule; `kh-partyhost/src/boosts.rs`; the item conversions are *proposals*) |
| **Slots** | Boss slots (Great Rune order, Messmer, Promised Consort). Talisman Pouch → accessory slot, then armour slot, then Megalixir. Larval Tear → armour slot (while under three), then Orichalcum+. | Implemented (Talisman / Larval rules are *proposals*) |
| **Save points** | A KH3 save point stands at every site of grace; the grace itself is removed and can never be used. Touching one unlocks it for fast travel. Menu: Save, **Load**, Select an Area, Pass Time, Respawn Enemies, Party Formation, Title Screen. **Delete** a slot from the save list, with a Yes/No confirm. Saving heals the party and cures Elden Ring statuses on every member. Disabled in combat. | Implemented (`kh-host/src/savepoint_menu.rs`, your decisions) |

### 7.1 Gloveworts and Roderika's spirit tuning (your 2026-11-02 request; untested in game)

**Why they were missing.** The loot sheet's rule `r2.309.reinforce_other` ("any other reinforcement material (gloveworts ...)") turned every glovewort into an ore (Fluorite ... Orichalcum+) at every pickup, and the KH shop never sold Elden Ring's own lines, so the Twin Maiden Husks' gloveworts were gone too. Roderika's spirit tuning ran Elden Ring's window, which reads gloveworts from the game's inventory, where none were.

**Where they come from now** (Elden Ring's own placement, nothing listed by us):
- **Map pickups, corpses, chests, boss rewards**: the game's own lots, through the loot gate, which now hands each glovewort to the KH3 inventory as itself (regulation 2.7.2.0 has 91 lot slots with a glovewort in `ItemLotParam_map`, 107 in `ItemLotParam_enemy`).
- **Enemy drops**: an enemy type taken over by `loot.kh` rolls its own lot and throws a KH3 material box; any other enemy's drop goes through the gate.
- **Shops**: the KH shop also sells the merchant's own glovewort lines, read at run time from the `ShopLineupParam` rows the merchant's script names, once their release flag is set, at Elden Ring's price (runes = munny). This is the Twin Maiden Husks after the Glovewort Picker's / Ghost-Glovewort Picker's Bell Bearings (rows 101829-101846: 800 ... 8000 Grave, 1200 ... 12000 Ghost). A line with a limited stock would be left out and logged; none exists in 2.7.2.0.

**The KH3 items** (sheet `sora-item-table`, COMPOSED rows; mapping rows `id.goods.10900`-`10919` of `sora-er-items`):

| Elden Ring goods | KH3 item key | Item id | Grade |
|---|---|---|---|
| 10900-10908 Grave Glovewort [1]-[9] | `MAT_GLOVE_GRAVE_01`-`09` | 0x781-0x789 | 1-9 |
| 10909 Great Grave Glovewort | `MAT_GLOVE_GRAVE_GREAT` | 0x78A | 10 |
| 10910-10918 Ghost Glovewort [1]-[9] | `MAT_GLOVE_GHOST_01`-`09` | 0x791-0x799 | 1-9 |
| 10919 Great Ghost Glovewort | `MAT_GLOVE_GHOST_GREAT` | 0x79A | 10 |

- Names are Elden Ring's. Descriptions are **composed** in KH3's material style ("A white flower that blooms in catacombs. Tunes spirit ashes to +1. (Upgrade material)").
- **Pictures** (your 2026-11-03 request): **Elden Ring's own icon** of each glovewort, everywhere a glovewort shows: the camp Items / Materials lists and pane, the "Obtained" popup, the KH shop, and any other screen using the item art.
  - The icon number is the goods' `EquipParamGoods.iconId` (2100-2109 Grave, 2110-2119 Ghost), in the item table's `er_icon` column; spirit tuning checks it against the live params at run time and logs any mismatch.
  - The picture comes from the UI pack's `er_icons`, the same route Elden Ring's key items use: `MENU_ItemIcon_<n>`, 160 x 160, scaled into the KH3 icon clip.
  - Those pictures are cut from the player's own install by `tools/er_icons.js`, already a step of `tools/packgen.js`. No setup change and no pack epoch bump are needed.
- **The shared route** for any Elden Ring goods icon on a KH screen:
  - kh-er `crates/kh-er/src/ericon.rs`: `goods_icon_id(goods)` gives the icon number, `goods_icon_key(goods)` gives the picture id.
  - kh-ui `item_art::er_icon_key(n)` (`ERICON_<n>`) is resolved by any `ItemArt::icon`.
  - `item_art::set_icon_aliases` / `add_alias_icons` give an item key an Elden Ring icon.
- Each item is category 7 (a material prize box), with KH3's stock maximum of 99, saved with the KH save like every material.
- A pickup shows KH3's "Obtained" item popup with its picture.

**The camp menu.** The Items screen lists them on the **Materials** tab (and in All). Materials now follow the item table's order: KH3's materials, then the ores (Fluorite, Damascus, Adamantite, Orichalcum, Orichalcum+, Electrum), **then the gloveworts** (Grave [1]-[9], Great Grave, Ghost [1]-[9], Great Ghost), then Evanescent and Illusory Crystal. Other stock keeps its pickup order; key items stay last. Pictures:
- `build/sora_tmp/er/glovewort/items_02_materials_tab.png` (the list, Grave Glovewort [1] focused with its picture and text)
- `items_01_obtained_popup.png` (the popup)
- `items_03_all_tab.png` (the All tab)

**Roderika** (your decision: a KH3-style screen; `roderika.kh = 0` gives her own menu back).
- Her "Spirit Tuning" option runs talk command 136 (`c1_136()`, read in her scripts t320001110 / t320006000; no other script uses it). The command is not run.
- The **Spirit Tuning** screen opens instead. It uses the camp menu's Items look with KH3's Keyblade Forge texts:
  - "Power up the ...?" with Yes / No
  - "You lack the needed materials."
  - "This spirit ash cannot be made any stronger." (KH3's Keyblade line)
- The screen lists every spirit ash held, with its level, the next level's glovewort with your stock of it, the munny, and what that level gives.
- Pad: A powers up, B or Menu backs out, LB / RB page. When it closes, her talk list comes back.
- **All numbers are read from the game at run time:**
  - the ash's level: its place in the `reinforceGoodsId` chain
  - the cost: `reinforceMaterialId` (Grave for ordinary ashes, Ghost for renowned ones, the grade Elden Ring asks) and `reinforcePrice` in munny
  - what it gives: the HP / Attack / Defense rates of the spirit's per-level doping SpEffect
  - the summon time per level: the Summon timer's own rule (`spirittune.rs summon_time_per_level`, from `kh_core::command::summon::duration`: +6 s per level for an ordinary ash, +4.5 s for a renowned one; section 7.2).
- **An upgrade, step by step:**
  1. The host checks again: the ash is held at that level, the KH3 stock holds the glovewort, Sora has the munny.
  2. In the game's inventory, the ash's entry becomes the next level's row: its item id and goods handle, the inventory's lookup, and any quick-item / pouch slot that held it. The Summon command reads that level. A handle of another form is refused, with nothing changed.
  3. Only then are the glovewort and the munny taken from the KH3 stock and Sora's munny.
  - No Elden Ring materials are involved anywhere. The KH3 stock is the one truth.
- Pictures: `build/sora_tmp/er/glovewort/tuning_01_list.png`, `tuning_02_question.png`, `tuning_03_after_upgrade.png`, `tuning_04_refused.png`.
- Code: `kh-er/src/spirittune.rs`, `kh-er/src/talkkh/tuning.rs`, `kh-host/src/hud_tuning.rs`, `kh-ui/src/tuning.rs`.

**Not seen in the game yet:**
- the gate handing a real glovewort over
- the Twin Maiden Husks' lines
- Roderika's command 136 being swallowed
- above all, the ash's inventory rewrite (written from the SDK's layout)
- spirit ashes now stay in the game's inventory (the Summon work changed `r2.510.spirit_ash` to pass; section 7.2), but no real pickup has been seen yet

### 7.2 Summon: Elden Ring's spirit ashes (your 2026-11-02 / 11-03 requests; untested in game)

**What it is.** Sora's fourth command, KH3's Link slot, now reads **Summon**. It calls an Elden Ring spirit ash he has equipped. While the spirit is out, the row reads **Unsummon**. Both words are composed: KH3's slot says "Link".

**Greyed when** (Xbox pad, KH3's layout; KH3's refused sound if you press it anyway):
- he doesn't own the **Spirit Calling Bell** (key item 8158, checked by its name at run time);
- **nothing is equipped**;
- **MP Charge** is running;
- the chant is still playing.

Only the player's Sora has the row. AI Sora and a host without spirits keep KH3's three rows. Donald and Goofy don't have it.

**Equipping** (camp menu → Equipment, Sora only): a new **Spirit Ashes** group of **4 slots** after the item slots. It uses KH3's ability line art and the Link icon; the title and help line are composed.
- A slot lists the ashes he owns, with "+N" and the MP cost.
- Square removes one. An ash already in another slot isn't offered.
- The slot keeps the ash's level-0 id, so an upgrade at Roderika keeps it equipped.
- The four slots are saved with Sora's KH save. This is additive: an older save just has four empty slots.

**The Summon list.** The equipped ashes, each with its MP cost in KH3's number field. A cost above his MP is recoloured (KH3's "short of MP" colour), but he can still call it.

**MP cost** (sheet `sora-magic-mp` rows `summon.mp.*`, your choice of MP converted from FP):
- an FP ash costs **ceil(FP × 0.5)**;
- an HP ash costs **ceil(HP × 0.15)**;
- the minimum is 1.

Costs from the 2.7.2.0 regulation (read offline):

| Ash (goods row) | Elden Ring cost | MP |
|---|---|---|
| 241000 (cheapest) | 11 FP | 6 |
| 213000 | 22 FP | 11 |
| 210000 | 37 FP | 19 |
| 203000 | 50 FP | 25 |
| 222000 | 77 FP | 39 |
| 201000 / 202000 | 100 FP | 50 |
| 243000 | 110 FP | 55 |
| 200000 Black Knife Tiche | 132 FP | 66 |
| 246000 / 245000 / 244000 | 135 / 185 / 240 HP | 21 / 28 / 36 |
| **207000 Mimic Tear** | **660 HP** | **All MP** (your 2026-11-03 rule) |

The cost is paid at the press by KH3's own rule: MP Thrift lowers it, and an overspend empties the bar and starts **MP Charge**, as a spell does. MP Safety refuses an overspend.

**The Mimic Tear takes all MP** (your 2026-11-03 request: "mimic tear should make Sora instantly go into MP recharge"). Its summon empties Sora's MP and starts MP Charge at once, whatever he had, as Donald's Zettaflare does. It shows "All" where other ashes show a cost.
- It needs MP above 0 and no MP Charge running. MP Safety never refuses it.
- **Its Unsummon refund** (my decision): the share of the time left of **the MP he had when he called it**, given as charge progress, since he is always in MP Charge then. Calling it and sending it straight back costs nothing but the chant, as with every other ash. A refund of none would make an early Unsummon pointless; a share of the 99-MP conversion would not match what it actually took.

**The call.** Sora plays **KH3's Link call `M_1571`** (3.3 s, his random "Link" voice, the light of every Link's start). **KH3's Link chant camera `LINK_START`** plays over it: KH3's own 0.8 s blend in, and a 0.5 s blend back at the chant's end (KH3 cut here into the Link's own cameras). At 3.09 s, when the light ends, the spirit appears.
- A hit that cuts the chant short still calls it, because the MP is already paid.
- It works on the ground and in the air.

**The spirit comes through Elden Ring's own spirit summon**, with the ash's own row and level, anywhere. There is no Rebirth Monument; a small hook keeps the game from dismissing it for "no monument in range".
- It is an **extra ally** beside the full party. One at a time.
- Elden Ring's own spirit HUD (the spirit HP bars and the monument icon) is hidden.
- KH targets never include it. Enemies and the aggro director treat it as a friend.
- **Ashes that only work beside a monument** are greyed. So is everything while online or in the Colosseum.

**The timer** (your request: rows `summon.time.*`):

| Tier | At +0 | Per level | At +10 |
|---|---|---|---|
| Ordinary ash (goods type 7, Grave Glovewort) | 60 s | +6 s | 120 s |
| Renowned ash (goods type 8, Ghost Glovewort) | 45 s | +4.5 s | 90 s |

The ash's other upgrade gains are Elden Ring's own per-level stats.

**Its gauge.** A KH3-style gauge in KH3's fourth party slot (above Donald's and Goofy's slots) shows the spirit's **name**, its **HP** with KH3's red follow-down, and the **time left** draining in a pale blue ring. The ring and the time pulse red under 10 s. It shows the ash's **Elden Ring icon** (your 2026-11-03 requests), drawn **under** the HP and time rings and cut round with a soft edge, so no corners show. KH3's Link icon is the fallback when the icon isn't known. The Mimic Tear shows its own ash icon.
- The Equipment screen's item panes show the same icon. Their art already has transparent corners, so it is not cut round there.
- KH3's command list rows have no picture slot, so the Summon list shows names and costs only.
- It appears when the spirit appears.
- **When the spirit leaves** (your 2026-11-03 request: "have the spirit ashes HUD element fade away after they either die or time out"), the gauge shows its final values: **empty HP** if it died, **empty time** if it timed out. It then plays **KH3's own guest-slot `out` animation**: a 25-frame hold, then 0.6 s of rolling right while fading out, 1.13 s in all.
  - Unsummon and the save point fade the same way, with the values they had (my decision: one look for a spirit leaving).
  - A load or the mode's end removes it at once.
  - A new summon during the fade replaces it cleanly.
  - kh-er holds the last spirit for 1.5 s of real time (`SummonView::leaving`), so the fade also plays while a save point's menu pauses the world.
  - Pictures: `pics/12_fade_died_*.png`, `pics/13_fade_timeout_*.png`.

**How it ends.** After any end the row reads "Summon" again.

| How | Refund |
|---|---|
| **Unsummon** (Decide on the row) | **MP cost × time left ÷ full time**, rounded down (100 % left = the full cost back, 0 % = nothing) |
| Its **time runs out** | none |
| It **dies** | none |
| The **KH save point's menu opens**, or Elden Ring's own rest at a grace | none (the save point refills MP anyway) |
| A load, a map change, or the game's own end of it | none |

- Unsummon and the timeout play a KH3 vanish: Anti Form's warp-out effect `vs_ant_wrp_out03` with its sound. Elden Ring's own fade follows.
- **Unsummon during MP Charge** (my decision): MP can't be written while charging, so the refund shortens the charge. The charge timer loses refund ÷ max MP of its length. The MP you get back is worth the same, and a large refund ends the charge at once with full MP.
- If the game refuses or fails a summon, the full cost is given back.

**Out of combat** it follows the leader. Elden Ring's own spirit follow does this, because the hidden Tarnished body stands at the KH leader. The game teleports it at 33 m or when it is stuck; KH3's friends warp at 30 m. In combat it fights with its own Elden Ring AI.

**Spirit ashes stay spirit ashes now.** The loot rule that turned every spirit ash into a KH accessory (`r2.510.spirit_ash`) passes them through, so Roderika can tune them and Summon can use them. Accessories already made from ashes in an older save stay accessories: the accessory doesn't record which ash it came from.

**Mimic Tear → Mimic Sora** (your 2026-11-02 / 11-03 requests: "Mimic Tear needs to mimic Sora, the mimic Sora can just use Sora's party member AI and uses his current equipment (mimic sora is unable to use Summon)", "build mimic Sora", "mimic Sora also needs to use the same outfit that Sora is currently using"). **Built; the Mimic Tear is no longer greyed** once his pose data has loaded (a second or so after the game starts) and the party host runs. Elden Ring's own Tarnished copy is never asked for: the Mimic Tear never goes through the game's spirit summon.

What happens when you call it:
- **Mimic Sora appears 1.5 m to Sora's right**, with a copy of Sora's kit as it is at that moment: level, stats, Keyblade, armour, accessories, abilities, magic and slots, and his own copy of the pouch. He arrives with full HP, MP and focus, in his normal form, with no meal or buff. The Mimic Tear's upgrade bonus is added (Elden Ring's own +0 .. +10 rows: max HP up to ×3.3, damage dealt up to ×2.47, damage taken ÷ up to 1.133). Later changes to Sora's equipment don't reach him, as with Elden Ring's Mimic Tear.
- **He never has Summon.** His Summon row is shut and re-shut every step.
- **His AI is AI Sora's own**: Goofy's tree on its own controller, AI Sora's magic, Cure / Potion, shotlock and Second Form rules, and AI Sora's Customize settings. He follows whoever leads: your Sora, or Donald / Goofy when one of them leads. Out of combat he follows like the friends and warps behind the leader when he is 25 m away and off camera. He takes no part in team attacks.
- **If AI Sora is in the party too, both exist:** two AI Soras plus the player.
- **Enemies and the lock-on treat him as an ally.** He has his own hidden Elden Ring body, a fourth slot in the ally-body pool beside Sora's, Donald's and Goofy's (sheet `party-er-bodies` `body.max` is now 4). It is on the spirit team, so Elden Ring's lock-on never takes him and its enemies attack him. A hit on that body becomes a hit on his core. His own hits are fired by that body, so the enemy's hate goes to him; a hit that may kill stays yours, for the runes. KH targets never include his body.
- **The spirit rules apply as to any ash:** the timer of its level, the KH gauge with the Mimic icon and his HP, Unsummon with the refund rule above, the save point, a rest. He also leaves when his HP reaches 0.
- **His arrival** (your 2026-11-03 request: "mimic sora, upon first being summoned, needs to play the proper mimic tear animation transforming into Sora"). It runs on Elden Ring's own clock for the Mimic Tear, read from its data (the arrival animation 60500 of its copy of the Tarnished):
  - at 0 s the game's own "a spirit appears" sound (c999997800) plays at his feet;
  - a pool of liquid silver (the Mimic Tear's own surface) spreads at his feet until 0.67 s, where the game starts its arrival effect;
  - he rises out of it from the feet up, with a liquid edge, until 3.1 s, where the game's copy has finished fading in;
  - the pool drains by 3.6 s. Only then does his AI take over.
  - While he forms he stands still and cannot be hit. It plays at every summon of the Mimic, never at a follow warp.
  - The game's arrival effect itself (FFX 640) can't be started at a point: no SpEffect carries it. The pool and the rising are drawn by us, with the game's surface and the game's light.
  - Pose: Sora's own idle (KH3 has no rise clip that fits).
- **His end** (your 2026-11-03 decision: "KH3 knockdown + silver dissolve"):
  - **His HP reaches 0:** KH3's knockdown (his own KO, as AI Sora's) with the game's spirit-death sound (s999997700). After 2 s he melts down into the silver over the game's 2 s spirit fade, with its sound (s999997730). Then he is gone.
  - **Time out, Unsummon, the save point or a rest:** no knockdown. The game's "spirit leaves" sound (c999997801), and he melts into the pool over the game's own fade-out (0.17 to 1.67 s). The pool drains, and he is gone.
  - **A load or the mode's end:** he goes at once.
  - From the moment he is dismissed he is no longer a spirit: the gauge goes, nobody hits him, and his body is released.
- **His look:** he is drawn with the Mimic Tear's own surface from the game's data (WetAspectParam 18: liquid silver). It is written into Elden Ring's G-buffer, and his KH colour pass is skipped, so **Elden Ring's own light passes shade him** as they shade the Mimic Tear. Your Sora and AI Sora look the same as before.
  - PhantomParam 200's sheen (the spirits' edge glow) is in the pictures but **not drawn in the game yet**: it needs its own set of additive shaders.
  - If the G-buffer route can't run in a frame (`r.hybrid 0`, no early depth, the shaders still being made), he is not drawn that frame. He is never drawn in Sora's own colours.
- **The outfit:** he wears the outfit Sora wears (KH1, KH2, the world costumes, Verum Rex), with the Mimic Tear surface over it. The outfit at the summon is written in his log line.
  - **My decision:** if you change outfit at Boc while he is out, he is redrawn in the new one. Your words were "the same outfit that Sora is *currently* using". An outfit is a look, not equipment: everything that affects play stays the summon's copy. And the renderer holds one outfit at a time; freezing the old one would mean loading a second outfit set.
  - The outfit's cloth simulation is the player's, so it doesn't run on him: his pose is the outfit's retargeted pose without its cloth.
- **His Keyblade** is drawn with the Keyblade model the renderer has loaded, which is your current one. His keychain is his own.
- **His effects and sounds** play at his own bones. An effect held on him over time (a trail, an aura) is moved to its bone after every step of his (position only, not its turn), and stopped when he goes.
- **The pool's look** can be tuned while the game runs: `r.mimic_pool_albedo` (0.2), `r.mimic_pool_gloss` (0.8), `r.mimic_pool_ripple` (0.35), `r.mimic_pool_ripple_cm` (14), `r.mimic_pool_ripple_speed` (16 cm/s), `r.mimic_pool_edge` (0.3), `r.mimic_pool_meniscus` (0.8). It is a dark rippled chrome puddle with a broken soft rim, shaded by the game's own light and reflections. His pipeline states are built as soon as his pose data has loaded, so the first summon of a session plays the whole arrival.
- **The aggro director** (enemies steered to party members) counts him as one of the AI allies, like Donald and Goofy, under his own key: the same threat model, holds and hate hits, the hate hits fired from his own body. An enemy an ability holds (Over Here!, Taunt Pass) still goes to that member. He has no borrowed target-priority row of his own.

**Test aids** (test profile only): `test.mimic = N` calls him as the Mimic Tear +N would, with no ash, bell or MP, under every spirit rule. `test.mimic_down` gives him a lethal blow (his knockdown and the dissolve).

**Seen in game** (2026-11-03, test profile, ER 2.7.2.0, with `test.mimic`): he forms out of the pool beside Sora and his AI takes over; the game's own sounds (c999997800 / c999997801 / s999997700 / s999997730) were played; he follows you; his Elden Ring body is made on the spirit team; a lethal blow plays the knockdown, then the dissolve; a timeout plays the dissolve. Your Sora stays normal with `r.hybrid 1`, with and without his body. Screens and logs: `build/sora_tmp/party/SUMMON/mimic_test/` (`r3_arrival_ingame_strip.png`, `r2_death_ingame_strip.png`, `r3_timeout_ingame_strip.png`, `r2_body.png`). Seen in game too (polish round, run 4): his states built before any summon, so the first summon of the session plays the whole arrival; the new dark rippled pool (`r4_arrival_ingame_strip.png`, before: `r3_arrival_ingame_strip.png`); enemies turned to him by the director ("aggro: target … -> Mimic Sora", "REALLY aims at Mimic Sora", hate hits from his body), their blows reached his core ("Mimic Sora's body is hit for 15.9"), and the director gave them back to Sora when he left; a Keyblade trail held on his weapon socket followed his bones. Not tested in game: a real Mimic Tear summon from the menu (the test save has no ash or bell), AI Sora and Mimic Sora together, outfits.

Pictures (offline, the game's surface rows over Sora's model; the light is a stand-in for the game's):
- beside Sora: `build/sora_tmp/party/SUMMON/pics/mimic_beside_sora.png`
- in outfits: `mimic_outfit_kh2.png`, `mimic_outfit_kh1.png`, `mimic_outfit_sanfransokyo.png` in the same folder
- the older look study: `mimic_sora.png`
- the arrival: `mimic_arrival_strip.png` (and `mimic_arrival_0..5.png`); the dissolve: `mimic_dissolve_strip.png` (and `_0..5`)

Code:
- kh-partyhost: `mimic.rs` (the kit), `lead.rs` (`MimicAi`; AI Sora's step now takes any leader)
- kh-er: `spirit_mimic.rs` (his core, AI input, body want, hits, pose, drawing), `spirit.rs` (the Mimic Tear branch, `test.mimic`), `allybody.rs` / `party/allybody_live.rs` (the fourth key)
- kh-er-render: `mimic.rs` (`submit_mimic`), `gfx/mimic_draw.rs`, `shaders/er_char.wgsl` (`gb_main`'s `KH_MIMIC` path)
- kh-shaderc: `--only mimic`. It writes `<variant>[_S]_gb3_mm.ps.dxil` for the main pack and every outfit and own set: 140 files. `--only mimicform` writes the forming writers `_gb3_mf` (130 files); `--only mimicpool` writes the pool (`mmpool.vs` / `.ps`).

**Deploy note** (these pack files must ship with a DLL that has Mimic Sora; without them he is not drawn, and the log says why):
- `build/sora_pack/materials/er/` and each `build/sora_pack/outfits/<id>/materials/er/` (caribbean, kh1, kh2, monstropolis, sanfransokyo, toybox, verumrex): every `*_gb3_mm.ps.dxil` (whole body, 130 files), every `*_gb3_mf.ps.dxil` (forming, 130 files), and the indexes `shaders_mimic*.json` / `shaders_mimicform*.json` (28 files);
- `build/sora_pack/materials/er/mmpool.vs.dxil` and `mmpool.ps.dxil` (the pool);
- the sheet `sheets/port/party-er-bodies.json` (`body.max` 4).
Nothing existing in the pack was changed.

**Code:**
- kh-core: `command/summon.rs` (the rules), `command/mod.rs` (the row), `states/summon_chant.rs`
- kh-er: `spirit.rs`, `spirit_mimic.rs`
- kh-ui: `camp_equip.rs`, `hud_spirit.rs`
- kh-host: `hud.rs`, `save.rs`
- kh-camera: `camera_anim.rs`

**Pictures** in `build/sora_tmp/party/SUMMON/pics/`:
- the Equipment area: `1_equipment_spirit_area.png`, `2_spirit_stock_list.png`
- the Summon list: `4_summon_command_list.png`
- greyed: `5_summon_greyed_nothing_equipped.png`
- the gauge: `01_full_time.png`, `03_half_time_damaged.png`, `04_warning_under_10s_peak.png`

**Not seen in game:** everything above.

---

## 8. Party (brief)

- **Starting characters:** Sora, Donald or Goofy, chosen at the new game; the starter is level 1.
- **Recruiting** (Implemented; `sheets/party-er-unlocks.json`):

  | Friend | Where he is | How he joins |
  |---|---|---|
  | Goofy | Fighting Leonine Misbegotten | Beat the boss |
  | Donald | Angry at the Finger Reader crone in Limgrave | Talk to him; six lines you approved |
  | Sora (if not the starter) | Fighting Margit | Beat Margit |

  Until you enter the arena, the staged fight does no damage on either side. A new friend joins at the level the shared EXP gives him.
- **Leader swap:** Party Formation at a save point picks who is in the party (up to 3) and who leads.
  - The leader holds Zero EXP.
  - Customize edits the leader's own shortcuts.
  - Menus list only the active party.
  - AI allies are targetable by enemies everywhere, and anyone can pick up items into the shared inventory.
- **Friends' boss rewards:** Donald and Goofy have their own boss lists (`don.*` and `gof.*` rows, DONALD_OVERHAUL.md section 9, GOOFY_OVERHAUL.md section 11.4), with their own HP, MP, slots, abilities, spells and ranks. A friend who joins later is caught up. Red Wolf also gives Donald's Aero Float.

---

## 9. Known gaps, untested items and open questions

**Surprises found while writing this:**
1. **Unison Blizzard and Unison Thunder have no source** (Planned, not built).
   - XD decision 3 (approved 2026-10-15) gives them to Royal Knight Loretta and the Draconic Tree Sentinel (`XD_PLAYABLE_KITS.md` lines 114-115, 357).
   - Only Fusion Rocket at Leonine was added (`IMPL_X3G_STATE.md`).
   - Donald's kit uses only the Unisons Sora owns, so Donald can't use them either.
2. **Link Extender is still on all three level paths** (A 32, B 48, C 34), but the port has no Links. It is useless, like the Attraction abilities you had removed.
3. **No Keyblade but the Kingdom Key can be obtained**, and the Kingdom Key cannot be upgraded. The friends get weapons from Elden Ring loot; Sora doesn't.

**Not verified:**
- **Boss defeat flags** were checked against the game's own event scripts on 2026-10-30 (all 54 rows; full list in `ER_BOSS_FLAGS.md`): 51 were right, Onyx Lord and Astel, Stars of Darkness were corrected, and Alabaster Lord, who has no boss defeat in the game, was replaced by the second Onyx Lord. Ten more rows are reserved (no reward yet): Crucible Knights, Erdtree Avatars, Ancient Dragon Lansseax and Borealis. None has been seen set by a real kill yet. A wrong flag means a boss gives a small-boss item instead of its row, until `boss.grant` is used.
- Small bosses count only when killed by **Sora's own hit**, per the code's header comment (`kh-er/src/bossreward.rs`). Whether a Donald or Goofy kill counts: **unknown**.
- **Untested in game**, by the stream notes: Critical difficulty, the 5-attacker limiter, most XS moves in Elden Ring, the item conversions, the roster boosts, the HUD's battle colour and Sora's battle portrait (2026-11-02; seen only in test pictures, `build/sora_tmp/hud/bc_*.png`), his per-outfit HUD faces (2026-11-03; `build/sora_tmp/hud/of_*.png`), Boc's "Alter garments" opening the Outfits screen and the camp menu's guide fix (2026-11-02; seen only in test pictures, `build/sora_tmp/er/boc_outfits/*.png`), and the leader reset (2026-11-02, your "Magicunement" picture). Since that fix, every load and every change of leader rebuilds the command menu and the leader's HUD pieces from scratch (kh-host `Hud::new_character`, docs/party-contracts.md P.3). Tests cover every Sora / Donald / Goofy pair through death and game-over Load, through the title's Load, and through a plain swap (pictures in `build/sora_tmp/party/LEADER-RESET/shots/`). It has not been seen in Elden Ring.

**Built but not confirmed by you:** most boss rewards (everything not marked *[user]*), the small-boss rules, the Great Rune slot order, and the Memory Stone / Whetblade / Talisman Pouch / Larval Tear conversions.

**Open questions:**
- Where should the Keyblades come from (section 6.3)?
- Should the DLC keep KH3's six Re Mind abilities, or should they move to base-game bosses (`docs/sora-er-boss-rewards.md` Q2)?
- Is one stat boost per three small bosses the right amount (Q1)?
- Flowmotion in Elden Ring: there are no KH3 flow markers, so wall run, rail and pole moves don't come up. Should they get Elden Ring triggers?

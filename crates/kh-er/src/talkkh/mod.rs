//! (CV2, the user 2026-10-16: "convert ALL of Elden Ring's real-time conversations (with characters like Varre and
//! stuff) to this new system. Use subtitle information to fill in the speech bubbles for Elden Ring character
//! conversations. Elden Ring character's audio should still play, and is skipped if you press A before the voice line
//! finishes. This conversation system needs to allow for choices, so that choices from Elden Ring conversations still
//! work in this new system." + "for elden ring characters, one line of subtitle would be one speech bubble")
//! THE ELDEN RING ADAPTER of the KH conversation system: it drives CV1's conversation host (`crate::conv`,
//! docs/party-contracts.md "Conversation" C.4: the bubble, the runner, the lock, the conversation camera). State note:
//! build/sora_tmp/er/IMPL_CV2_STATE.md. EVERYTHING GAME-SIDE IS UNTESTED IN THE GAME.
//!
//! HOW ELDEN RING TALKS (eldenring.exe 2.7.1.0, read in a copy of the exe and in the game's own talk scripts; the
//! addresses are RVAs):
//! - A talk script (talk/*.talkesdbnd, 365 scripts, 359 of them characters') runs its commands through
//!   `CSEzStateTalkEvent` slot 1 (0xea7100) and its queries through `CSEzStateTalkEnv` slot 1 (0xea24a0).
//! - TalkToPlayer (command 1) starts a BLOCK on the character's `CSNpcTalkIns`: TalkParam rows id, id + 1, ... one
//!   LINE each (0xe9c0e0 looks up block + index + 1; the row's voice plays on the character, 0xe9bf10). The talk update
//!   (0xe9ba20, every frame) posts the line's subtitle (0xe9c780, TalkMsg id = the row's msgId) and goes to the next
//!   line when the voice has ended or the line's time is up, or AT ONCE when the game's skip byte is set
//!   (`[RVA 0x458fc08] + 0x92c0`, written every frame from input action 6001 by 0xb04140) unless the line refuses it
//!   (TalkToPlayer's flag 1, the timeout). The next line's start stops the voice still playing.
//! - Menus: ClearTalkListData (20), AddTalkListData (19 / 149: index, EventTextForTalk id, flag), then
//!   ShowShopMessage(1) (10: the talk list) or OpenConversationChoicesMenu(0) (76: the conversation's choices); the
//!   script waits while CheckSpecificPersonMenuIsOpen (59) says 1 and reads GetTalkListEntryResult (23 = the popup
//!   menu's + 0x178). Yes / No: OpenGenericDialog (17, buttons 3), CheckSpecificPersonGenericDialogIsOpen (58), the
//!   button (22: 1 Yes, 2 No).
//!
//! WHAT THE ADAPTER DOES (`talk.kh`, default 1; `talk.kh = 0` = Elden Ring's own subtitles, menus and dialogs, every
//! hook a plain pass-through):
//! - EVERY SUBTITLE LINE OF A CHARACTER'S TALK IS ONE KH BUBBLE (the row the game plays; a new row = a new bubble; the
//!   game's own line breaks are spaces inside it): the speaker's name from the character's NpcParam nameId (NpcName),
//!   the text from TalkMsg in the game's language (the game's message repository holds the current language), the
//!   tail at the character's head. The game's own subtitle is not posted while the bubble shows it.
//! - THE VOICE IS THE GAME'S (nothing of it is touched): the bubble goes on when the game's line does. A while the
//!   voice plays sets the game's own skip byte for that line, on the game's thread, right before the talk update reads
//!   it: the game skips exactly as for its own button (the next line's start stops the voice).
//! - CHOICES: the talk list (Talk / Purchase / Sell / Leave ...), the conversation choices and the Yes / No dialogs
//!   are KH options in the bubble; Elden Ring's menu or dialog never opens (its command is not run), the script's
//!   "is it open" queries read 1 until the player chooses, then the result query reads the option's own index (the
//!   value the game's menu would have left). B = "Leave" in a talk list, "No" in a Yes / No. What the option opens
//!   (a shop, the level up, a full-screen menu) is the game's as before. A menu whose option has no text, or one
//!   asked while the bubble cannot show, stays Elden Ring's. A choice the bubble can no longer show is GIVEN BACK:
//!   the command is run for the game then.
//! - ENGAGED (the player in the conversation: the talk camera command 138 is on, or a choice is up; never by an NPC's
//!   line during a battle): ONE CV1 CONVERSATION (open-ended, owner "talk", the leader and the NPC by handle) for it:
//!   CV1 holds the leader's pad and the party, frames the conversation camera, shows the bubble; the adapter pushes
//!   one `Line` per game line and one external `Choice` per game menu, reports the game's voice, turns the runner's
//!   `StopVoice` into the game's skip and its `Chosen` into the script's answer, and ends the conversation with the
//!   talk. NOT ENGAGED (proximity lines, a hostile NPC's taunts, lines in a battle): Elden Ring's own subtitle (CV1's
//!   conversations always hold the leader's pad: a bubble-only kind is CV2's request to CV1, IMPL_CV2_STATE.md).
//! - Another owner's conversation running (Donald's), `conv.on = 0`, no bubble in the HUD, a cutscene: Elden Ring's
//!   own subtitles, menus and dialogs.
//! - Left alone: the grace's menu and the other system scripts (talk ids below 100000, Melina's 3000 excepted:
//!   savepoint.rs drives the grace), cutscenes (no bubble while one plays: their subtitles are their own), menus of
//!   other kinds (shops, level up ...).
//!
//! KEYS: `talk.kh` (1; 0 = Elden Ring's own UI), `talk.kh.skip` (talk ids, comma separated, left to Elden Ring),
//! `talk.kh.log` (0; 1 = a `talk:` line per line and choice).

pub mod rules;
// (SHOP) the merchants' buy / sell windows -> the KH shop; the smiths' talk refused (pure rules of the hooks)
pub mod shop;
pub mod steps;

#[cfg(windows)]
pub(crate) mod game; // (SHOP) pub(crate): npc_of for the KH shop
#[cfg(windows)]
pub(crate) mod hooks; // (PORT) pub(crate): the offline address check reads its constants
#[cfg(all(test, windows))]
mod mock_tests;

#[cfg(windows)]
pub use game::{frame, masks_game_pad, TalkCtx};
#[cfg(windows)]
pub use hooks::{install, uninstalled};

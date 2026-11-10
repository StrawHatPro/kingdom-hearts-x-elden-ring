//! (MELINA-SUMMON, the user 2026-11-08: "Talk to Melina needs to just straight up make Melina appear and talk, using the
//! same logic for her dialogue but skipping having to access a grace menu behind the scenes all together") MELINA'S
//! TALK FROM THE SAVE-POINT MENU: the pure rules. savepoint.rs gives the machines what it sees each frame and does what
//! they say (the game's own talk commands on the grace's own talk object, the flags); nothing here touches the game.
//! State notes: build/sora_tmp/er/IMPL_ER33_STATE.md (addenda IMPL_ER36, MELINA-SUMMON).
//!
//! WHAT THE GRACE DOES WHEN HER ENTRY IS CHOSEN (PARSED, the grace's talk script t000001000; groups are named as the
//! listings in build/sora_tmp/er/er33/txt name them, a call `c6_N` = group x(2147483647 - N)):
//! - x36(4655): if flag 4651 (she is summoned) is off, x28 picks her appearing animation (x28 state 3: 4698 -> 20006
//!   and 0.5 s; 108 -> 20000; 400001 off -> 20000; else 20001; 3.5 s) and x35 summons her: it waits until the player
//!   carries neither special effect 9600 nor 9603, runs COMMAND 128 (2180, 21800000, 1, 3000, anim, 1, 2, 60), sets
//!   4651 and waits the 3.5 / 0.5 s. Then 4652 (she talks) and 4655 (asked from the menu) are set, and it waits until
//!   her script clears 4652, or 5 s have passed with neither 9600 nor 9603 on the player (no Melina there). Then, with
//!   4680 off, x29(20010) sends her away: COMMAND 129 (20010, 1.4) and flags 4651 .. 4657 cleared.
//! - With 4680 on she stays until the rest ends: x23 states 37 / 38 call x67 = x29(20011) when 400001 is on, 108 off
//!   and 11002745 off, else x29(20010).
//! - WHAT SHE TALKS ABOUT is not in x36: building the menu, x43 clears 24 TOPIC FLAGS and x57 sets the one of the
//!   entry it adds (its conditions are tried in order, the first that holds wins); her script (t000003000 x45 -> x43)
//!   reads it. Before the menu the rest's x30 decides whether SHE SPEAKS BY HERSELF (x59 .. x62: flag 4653), and x50
//!   then summons her the same way (x28, 4652 on, the same wait; sent away with 4680 off, 108 on or 11009260 on
//!   without 400001; else at the rest's end).
//!
//! WHAT HER SCRIPT NEEDS (PARSED, t000003000): its main machine waits for 4652 (x46), then x45 waits for 9001 off and
//! for special effect 9606 off or 10 s, and picks the talk by 4653 / 4654 / 4655 / 4656 / 4657 / 1054539200; x46's exit
//! clears 4652 .. 4657 when the talk is over. NO STATE REACHABLE FROM ITS MAIN MACHINE TESTS FLAG 9000 (the player
//! sits): only the generic groups x8 / x24 do, and nothing calls them. Her Accept / Refuse, the invitation (x48 sets
//! 10009655, "Go to the Roundtable Hold" x80 sets 104, common.emevd event 945 warps on 104 alone) are her own commands
//! on her own talk object.
//!
//! HOW SHE IS SUMMONED HERE (no rest, no grace menu, Sora never sits): command 128 writes a request into the
//! `CS::CSTalkDynamicChrCtrl` of the talk object it runs on (eldenring.exe 2.7.1.0, read in the copy: the command at
//! 0x140ea7100 case 0x80 -> 0x140e9d990, which accepts it only in the controller's state 0 "_Update_None"); the
//! controller's "_Update_Create" (0x140e9eca0) places her from THE PLAYER'S OWN position and facing (WorldChrMan's
//! main player), not from the grace, and starts her animation. An SDK-made talk object has no controller (its pointer
//! is null), so the commands are run on THE GRACE'S OWN TALK OBJECT (talkkh/hooks.rs finds it by the grace
//! character's handle): the same controller, the same summon, the same leaving as at a rest. The flags are written as
//! the grace's script writes them. `Summon` is that sequence, a step a frame.
//!
//! THE ACCORD STILL NEEDS THE GAME'S REST: its cutscene is common.emevd event 950, which waits for flag 9000, and only
//! the grace's rest sets 9000 (PARSED). So the automatic row (and an accord whose rest was tried and brought nothing)
//! keeps `Drive`: the black cover, the game's real rest, the scene and her talk after it, B until he stands if the
//! grace menu comes. Nothing of `Drive` ever selects anything in the grace menu.

/// The talk id Elden Ring's popup menu carries while the grace's own menu is up (the grace's talk script t000001000).
pub const TALK_GRACE: i32 = 1000;
/// ... and while Melina's talk has it (t000003000).
pub const TALK_MELINA: i32 = 3000;
/// Flags of the talk scripts: she is summoned (x35); she talks (set by the grace, cleared by her script at the end:
/// PARSED x36 / x46); she speaks by herself (x59 .. x62); asked from the menu (x36's argument); the agreed warp to the
/// Roundtable Hold (104, until 105).
pub const FLAG_SUMMONED: u32 = 4651;
pub const FLAG_TALKING: u32 = 4652;
pub const FLAG_OWN: u32 = 4653;
pub const FLAG_ASKED: u32 = 4655;
pub const FLAG_WARP: u32 = 104;
pub const FLAG_ROUNDTABLE: u32 = 105;
/// Her invitation to the Roundtable Hold was made (x48 state 9 sets it; the grace's x57 offers "Tell her you're off to
/// Roundtable Hold" while it is on and 105 is off).
pub const FLAG_INVITED: u32 = 10009655;
/// (MELINA-RT) THE TOUCH OF HER HAND: the special effect her script waits for on the MAIN PLAYER before it sets 104
/// (t000003000 x80 state 3 `IF [61 9601 == 1]`, query 61 = the main player's effects, CSEzStateTalkEnv::invoke 0x140ea3b90
/// case 0x3d; x77 state 2 also moves on with it). Her own script cannot give it: its only giver `c5_62(GetWorkValue(0) > 1,
/// 9601)` needs a work slot above 1, and her script writes 0 and 1 only (command 100 stores an int, 0x140eaa38e; query 100
/// reads an int, 0x140ea5ee2). In the game it comes with the body resting at the grace, which Sora never does here.
/// Nothing else reads it on the player: of the 365 talk scripts only hers asks query 61 for it (the five others use it on
/// their own character, query 60 / command 121), no event script tests it on the player (m10_00 10003741 and m60 events
/// watch NPCs), no param, the player's HKS and Melina's TAE do not name it. SpEffectParam 9601: 0.05 s, state info 0.
pub const HAND: i32 = 9601;

/// (MELINA-RT2, the user 2026-11-10: "make the wait to teleport shorter, just a couple seconds") HER X80'S WAIT. Once she
/// has the touch of her hand, x80 state 4 waits `IF [103 > 11 abort-if-false 57 0 == 1]`: query 103 = the state's own
/// time in seconds (CSEzStateTalkEnv::invoke 0x140ea3b90 case 0x67: `*(float *)(*(env + 8) + 0x48)`, answered as a
/// float), query 57 = the talk is over (case 0x39: `!FUN_140ea7b60(env, 0)`, which is 1 while the talk object's
/// character, `[env + 0x18] + 0x40`, is the talk partner in `[0x144592c28] + 0x92d0 / + 0x92d8`). MEASURED (the user's
/// log 2026-11-10): 9601 at 49.888, her line 10010600 at 50.173 (state 4 begins), the talk over at 52.522, flag 104 at
/// 61.156 = 50.16 + 11: 8.6 s of nothing after the talk. Then state 2 -> state 1 sets 11109786 and 104, and the calls
/// return (x77, x78, x61, x45; x46's exit clears 4652 .. 4657).
/// THE WAIT TAKEN OFF: her script reads its state time `X80_WAIT` seconds ahead while it is in x77 / x80 (from the frame
/// it asks query 61 for 9601 and gets 1: of her whole script only x77 state 2, after her line 10010600, and x80 state 3
/// ask it; in that call chain only x80 state 4 asks query 103). The game still answers query 57 itself: her script
/// moves on, and does everything after the wait itself, on the frame the game's own talk ends.
pub const QUERY_PLAYER_EFFECT: i32 = 61;
pub const QUERY_STATE_TIME: i32 = 103;
pub const X80_WAIT: f32 = 11.0;

/// (MELINA-RT2) What her talk's queries are answered (talkkh/hooks.rs asks it in the query detour, for talk id 3000,
/// after the game's own answer). `armed`: the summon's Roundtable window (`SummonStep.hand`: her summoned talk, 10009655
/// on, 105 off, 104 off).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hurry {
    /// Her talk object, once her script asked for the touch of her hand and the game said yes (0: not yet).
    pub ins: usize,
    /// Her state-time reads answered ahead so far.
    pub raised: u32,
}

impl Hurry {
    /// One of her queries as the game answered it (`arg`: its first argument). `Some(v)`: her script reads `v` instead.
    pub fn on_query(&mut self, armed: bool, ins: usize, id: i32, arg: Option<f32>, answer: f32) -> Option<f32> {
        if !armed || ins == 0 {
            *self = Hurry::default();
            return None;
        }
        if id == QUERY_PLAYER_EFFECT && arg == Some(HAND as f32) && answer == 1.0 {
            if self.ins != ins {
                *self = Hurry { ins, raised: 0 };
            }
            return None;
        }
        if id == QUERY_STATE_TIME && self.ins == ins {
            self.raised = self.raised.saturating_add(1);
            return Some(answer + X80_WAIT);
        }
        None
    }

    /// The summon's window is over (or never was): nothing is answered ahead any more.
    pub fn disarm(&mut self) {
        *self = Hurry::default();
    }
}
/// What x29 clears when she is sent away (4651 .. 4657).
pub const MELINA_FLAGS: [u32; 7] = [4651, 4652, 4653, 4654, 4655, 4656, 4657];
/// The special effects her script keeps on the player while she is there (t000003000 main state 3: 9600 or 9603); the
/// grace's x35 / x36 test them.
pub const MARKERS: [i32; 2] = [9600, 9603];
/// The player's special effects the grace's x57 / x60 and her x72 test before the invitation to the Roundtable Hold (she
/// warps him there). 4270 = TRAVEL CLOSED HERE (eldenring.exe, read in the copy: put on the player while the area's
/// fast-travel flag is off; travel.rs). 4271 / 4272: put on by the same update in other states, NOT identified; 4280 /
/// 4282 / 4286 NOT identified (the old note "multiplayer states" was never verified).
pub const NO_TRAVEL: i32 = 4270;
pub const MP_EFFECTS: [i32; 6] = [4270, 4271, 4272, 4280, 4282, 4286];
/// The 24 topic flags the grace's x43 clears before x57 sets one (states 2 and 4).
pub const TOPIC_FLAGS: [u32; 24] = [
    1042379200, 1042379202, 1042379206, 1046389201, 1043349250, 1038509250, 1043539250, 11009255, 1043509200, 1054559200, 1036489250, 1039519250, 1037519200, 10009656, 11009270, 11009275, 1054539210, 35009350, 35009352,
    1054539215, 1042379208, 11009265, 35009355, 35009358,
];
/// Command 128 as x35 runs it, before the animation: her character, her NpcParam row, 1, her talk id; after the
/// animation: 1, 2, 60.
pub const SUMMON_COMMAND: i32 = 128;
pub const SUMMON_HEAD: [i32; 4] = [2180, 21800000, 1, TALK_MELINA];
pub const SUMMON_TAIL: [i32; 3] = [1, 2, 60];
/// Command 129 as x29 runs it: the leaving animation and 1.4.
pub const DISMISS_COMMAND: i32 = 129;
pub const DISMISS_SECONDS: f32 = 1.4;

/// x84: the expansion's maps (20000000 .. 28999999, 40000000 .. 43999999 and the expansion's open world): no talk of
/// hers there. `area`: the map area of the player's block (60 = the open world, 61 = the expansion's).
pub fn expansion_map(area: u32) -> bool {
    (20..=28).contains(&area) || (40..=43).contains(&area) || area == 61
}

/// CAN THE GAME'S GRACE MENU HOLD AN ENTRY OF HERS HERE AT ALL (PARSED: x84, x52 state 0, x43 state 4)? `Err`: why not.
pub fn menu_gate(flag: &dyn Fn(u32) -> Option<bool>, area: u32) -> Result<(), &'static str> {
    let on = |f: u32| flag(f) == Some(true);
    if expansion_map(area) {
        return Err("the talk script adds no entry of Melina's in this map (the expansion's areas)");
    }
    // x52 state 0: Ranni's entries take the place
    if on(400394) && ((!on(12019280) && (on(12012710) || on(12012711) || on(12012712))) || (on(1034509406) && on(12012713))) {
        return Err("the talk script shows Ranni's entry in the place of Melina's (flag 400394)");
    }
    // x43 state 4
    if !on(953) {
        return Err("flag 953 is off (the accord's scene has not played)");
    }
    if on(11102790) {
        return Err("flag 11102790 is on");
    }
    if on(11009260) && !on(400001) {
        return Err("flag 11009260 is on (she has left)");
    }
    if on(108) {
        return Err("flag 108 is on");
    }
    if on(110) {
        return Err("flag 110 is on (the Erdtree burns: she is gone)");
    }
    Ok(())
}

/// What the grace's script would have her talk about here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Talk {
    /// She speaks by herself (flag 4653: x30 -> x59 .. x62; her x40 picks the talk).
    Own,
    /// Asked from the menu (flag 4655), about the topic flag x57 sets (None: x57's developer entry under flag 4699,
    /// which sets no topic).
    Topic(Option<u32>),
}

impl Talk {
    /// For the log.
    pub fn describe(&self) -> String {
        match self {
            Talk::Own => "she has something to say (flag 4653: the grace's x30)".to_string(),
            Talk::Topic(Some(f)) => format!("asked from the menu (flag 4655), topic flag {f} (the grace's x57)"),
            Talk::Topic(None) => "asked from the menu (flag 4655), the developer entry of flag 4699 (no topic)".to_string(),
        }
    }
}

/// DOES SHE SPEAK BY HERSELF AT A REST HERE (PARSED: x23 states 41 / 29 -> x84, then x30 -> x59 / x60 / x61 / x62, the
/// ones that set 4653)? `has`: the player carries this special effect.
pub fn own_talk(flag: &dyn Fn(u32) -> Option<bool>, has: &dyn Fn(i32) -> bool, area: u32) -> bool {
    let on = |f: u32| flag(f) == Some(true);
    if expansion_map(area) || on(11102790) || on(110) {
        return false;
    }
    if !on(953) || on(4698) {
        // x59
        return on(953);
    }
    if !on(4680) {
        return false;
    }
    if on(108) {
        // x62
        return !on(35009360);
    }
    if on(11009260) && !on(400001) {
        // x61
        return on(9104);
    }
    // x60 (its registers: r0 = 10009655, r1 .. r6 = the six effects, r7 = 9104)
    let alone = !MP_EFFECTS.iter().any(|e| has(*e));
    let invited = on(10009655);
    (on(10000851) && !invited && alone)
        || ([3062, 3063, 3064, 3065].iter().any(|f| on(*f)) && !invited && alone)
        || (!on(11009260) && !on(400001) && !on(9104) && [11002741, 11002742, 11002743, 11002744].iter().any(|f| on(*f)))
        || (on(9104) && !on(400001))
}

/// THE ENTRY THE GRACE'S MENU WOULD ADD FOR HER (PARSED: x57; the caller has checked `menu_gate`): its topic flag, the
/// first condition that holds in x57's order. None: x57 adds no entry. `Some(None)`: the developer entry (4699).
pub fn topic(flag: &dyn Fn(u32) -> Option<bool>, has: &dyn Fn(i32) -> bool) -> Option<Option<u32>> {
    let on = |f: u32| flag(f) == Some(true);
    let any = |fs: &[u32]| fs.iter().any(|f| on(*f));
    if !on(4680) {
        // x57 state 4: the accord refused, asked again
        return if !on(1042372700) {
            Some(Some(1042379200))
        } else if on(4699) {
            Some(None)
        } else {
            None
        };
    }
    // x57 state 6 (its registers as the script sets them: r0 = 10009655, r1 = 1042379203, r2 = 3940, r3 = 35002731, r4 = 35002733)
    let (r0, r1, r2, r3, r4) = (on(10009655), on(1042379203), on(3940), on(35002731), on(35002733));
    let alone = !MP_EFFECTS.iter().any(|e| has(*e));
    let rows: [(bool, u32); 23] = [
        (r0 && !on(105) && alone, 10009656),
        (on(1042372701) && !r1 && !r0, 1042379202),
        (on(1042372703), 1042379208),
        (!on(1042372702) && !on(1042379207) && r1 && !r0, 1042379206),
        (on(1046382701) && !on(1046389202), 1046389201),
        (on(1043342700) && !on(1043349251), 1043349250),
        (on(1038502710) && !on(1038509251), 1038509250),
        (on(1043532710) && !on(1043539251), 1043539250),
        (on(11002740) && !on(11009256), 11009255),
        (on(1043502700) && !on(1043509201), 1043509200),
        (on(1054552700) && !on(1054559201), 1054559200),
        (!on(1037519201) && !on(11009260) && !on(400001) && any(&[1037512700, 1038512700, 1038502711, 1039512711, 1037522700]), 1037519200),
        (on(1036482710) && on(1036489213) && r2 && !on(1036489251), 1036489250),
        (on(1039512710) && on(1039519209) && on(1036489210) && r2 && !on(1039519251), 1039519250),
        (on(11002745), 11009265),
        (any(&[1054552701, 1052562700, 1052542700, 1051532700, 1052532710]) && !on(1054539211), 1054539210),
        (on(1054532702), 1054539215),
        ((on(35002730) || r3) && !on(35009351), 35009350),
        (r3 && !on(35009353), 35009352),
        (on(35002732) && !r4, 35009355),
        (r4, 35009358),
        (on(11109387) && !on(11009271), 11009270),
        (on(1049539207) && !on(11009276), 11009275),
    ];
    if let Some((_, f)) = rows.iter().find(|(holds, _)| *holds) {
        return Some(Some(*f));
    }
    on(4699).then_some(None)
}

/// WHAT "Talk to Melina" RUNS HERE, as the grace would at a rest: her own talk first (x30 runs before the menu is
/// built), else the menu's entry (where the menu can hold one). None: the game has no talk of hers here now.
pub fn talk_for(flag: &dyn Fn(u32) -> Option<bool>, has: &dyn Fn(i32) -> bool, area: u32) -> Option<Talk> {
    if own_talk(flag, has, area) {
        return Some(Talk::Own);
    }
    menu_gate(flag, area).ok()?;
    topic(flag, has).map(Talk::Topic)
}

/// x28: her appearing animation and the seconds x35 waits after the summon. None: 4651 is on, she is already there
/// (x28 state 2: no summon).
pub fn appear(flag: &dyn Fn(u32) -> Option<bool>) -> Option<(i32, f32)> {
    let on = |f: u32| flag(f) == Some(true);
    if on(FLAG_SUMMONED) {
        None
    } else if on(4698) {
        Some((20006, 0.5))
    } else if on(108) || !on(400001) {
        Some((20000, 3.5))
    } else {
        Some((20001, 3.5))
    }
}

/// Her leaving animation once her talk is over (read then): x36 / x50's own sending away (20010), else the one at the
/// rest's end (x67).
pub fn leave_anim(talk: Talk, flag: &dyn Fn(u32) -> Option<bool>) -> i32 {
    let on = |f: u32| flag(f) == Some(true);
    let at_once = match talk {
        Talk::Topic(_) => !on(4680),
        Talk::Own => !on(4680) || on(108) || (on(11009260) && !on(400001)),
    };
    if !at_once && on(400001) && !on(108) && !on(11002745) {
        20011
    } else {
        20010
    }
}

/// What savepoint.rs sees in a frame of a summon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Near {
    /// The grace's own talk object was seen in the last moments (talkkh/hooks.rs).
    pub grace_talk: bool,
    /// The player carries 9600 or 9603 (a Melina's script runs).
    pub marker: bool,
    /// Flag 4652.
    pub talking: bool,
    /// Flag 104 on, 105 off: the game's warp to the Roundtable Hold follows.
    pub warp: bool,
    /// Flag 10009655 on, 105 off: she has invited him to the Roundtable Hold and he has not been there.
    pub invited: bool,
}

/// What savepoint.rs is to do in a frame of a summon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    None,
    /// The topic flags as the menu would leave them (x43, x57; 4653 for her own talk), command 128 with this
    /// animation, flag 4651.
    Summon(i32),
    /// Flag 4652 (and 4655 for a menu topic).
    Start,
    /// Command 129 with the leaving animation `leave_anim` gives now, and 4651 .. 4657 cleared (x29).
    Dismiss,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Waiting for the grace's talk object and for no other Melina (x35 state 4).
    Clear { since: f32 },
    /// Summoned: her appearing (x35 state 3).
    Appear { since: f32, wait: f32 },
    /// Her talk (x36 / x50's wait).
    Talk { since: f32 },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SummonStep {
    pub order: Order,
    /// A line for the log (a change of phase).
    pub log: Option<&'static str>,
    pub end: Option<End>,
    /// (MELINA-RT) Put `HAND` (9601) on the main player this frame, through the game's own apply (the function talk command
    /// 62 calls, 0x1403e8dc0, as `c1_62(9601)` would): her talk runs in the Roundtable context and 104 is not on yet.
    pub hand: bool,
}

/// THE SUMMON, as the grace's x36 / x50 run it. Every path ends in `Done` within the limits below, and every path that
/// summoned her ends with `Order::Dismiss` (or the game's own warp), so nothing of hers is left set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summon {
    pub talk: Talk,
    /// `appear` as read when it was asked for (None: she was there already).
    pub appear: Option<(i32, f32)>,
    pub phase: Phase,
    /// (MELINA-RT) The touch of her hand was given last frame (its first frame is logged).
    pub handing: bool,
}

impl Summon {
    /// Seconds to wait for the grace's talk object and for no other Melina before nothing is done.
    pub const CLEAR_LIMIT: f32 = 10.0;
    /// x36 / x50: after this long without 9600 / 9603 on the player, no Melina is there: the talk is over.
    pub const ABSENT_AFTER: f32 = 5.0;
    /// The longest talk before she is sent away all the same.
    pub const TALK_LIMIT: f32 = 600.0;

    pub fn new(talk: Talk, appear: Option<(i32, f32)>) -> Summon {
        Summon { talk, appear, phase: Phase::Clear { since: 0.0 }, handing: false }
    }

    /// Her talk has the pad (her lines, her choices): Sora gets none.
    pub fn holds_pad(&self) -> bool {
        matches!(self.phase, Phase::Appear { .. } | Phase::Talk { .. })
    }

    /// Something savepoint.rs could not do (the command did not run): ended; `dismiss`: she may be there, send her away.
    pub fn abort(&mut self, dismiss: bool, why: &'static str) -> SummonStep {
        self.phase = Phase::Done;
        SummonStep { order: if dismiss { Order::Dismiss } else { Order::None }, log: None, end: Some(End::Failed(why)), hand: false }
    }

    /// ONE FRAME.
    pub fn step(&mut self, dt: f32, seen: Near) -> SummonStep {
        let quiet = SummonStep { order: Order::None, log: None, end: None, hand: false };
        match self.phase {
            Phase::Done => quiet,
            Phase::Clear { since } => {
                let since = since + dt;
                self.phase = Phase::Clear { since };
                if seen.grace_talk && (self.appear.is_none() || !seen.marker) {
                    return match self.appear {
                        Some((anim, wait)) => {
                            self.phase = Phase::Appear { since: 0.0, wait };
                            SummonStep { order: Order::Summon(anim), log: Some("she is summoned (command 128 on the grace's own talk object): she appears beside him"), end: None, hand: false }
                        }
                        None => {
                            self.phase = Phase::Talk { since: 0.0 };
                            SummonStep { order: Order::Start, log: Some("she is there already (flag 4651): her talk is started"), end: None, hand: false }
                        }
                    };
                }
                if since > Self::CLEAR_LIMIT {
                    self.phase = Phase::Done;
                    return SummonStep { end: Some(End::Failed(if seen.grace_talk { "the summon: another Melina is still there (9600 / 9603 on the player)" } else { "the summon: the grace's talk object was not seen" })), ..quiet };
                }
                quiet
            }
            Phase::Appear { since, wait } => {
                let since = since + dt;
                self.phase = Phase::Appear { since, wait };
                if since >= wait {
                    self.phase = Phase::Talk { since: 0.0 };
                    return SummonStep { order: Order::Start, log: Some("she has appeared: her talk is started (flag 4652)"), end: None, hand: false };
                }
                quiet
            }
            Phase::Talk { since } => {
                let since = since + dt;
                self.phase = Phase::Talk { since };
                // (MELINA-RT) her script waits for the touch of her hand (x80 state 3) once it reaches the Roundtable's
                // "Very well. Let my hand rest upon you" (x77): given while her talk runs in that context, until 104
                let hand = seen.talking && seen.invited && !seen.warp;
                let first = hand && !self.handing;
                self.handing = hand;
                let quiet = SummonStep { hand, log: first.then_some("her talk is the Roundtable Hold's (10009655 on, 105 off): the touch of her hand her script waits for (x80: effect 9601 on the player; the game's resting body gives it, Sora never rests) is given by the game's own apply each frame until her script sets 104"), ..quiet };
                if seen.warp {
                    self.phase = Phase::Done;
                    return SummonStep { end: Some(End::Talked("she takes him to the Roundtable Hold: the game's own warp follows")), ..quiet };
                }
                if !seen.talking {
                    self.phase = Phase::Done;
                    return SummonStep { order: Order::Dismiss, log: None, end: Some(End::Talked("her talk is over (her script cleared flag 4652): she leaves")), hand: false };
                }
                if since > Self::ABSENT_AFTER && !seen.marker {
                    self.phase = Phase::Done;
                    return SummonStep { order: Order::Dismiss, log: None, end: Some(End::Nothing("no Melina came (no 9600 / 9603 on the player 5 s into her talk): the flags are cleared")), hand: false };
                }
                if since > Self::TALK_LIMIT {
                    self.phase = Phase::Done;
                    return SummonStep { order: Order::Dismiss, log: None, end: Some(End::Failed("her talk did not end in time: she is sent away")), hand: false };
                }
                quiet
            }
        }
    }
}

// ---------------------------------------------------------------- the accord's rest (it needs the game's rest)

/// What savepoint.rs sees in a frame of the accord's rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Seen {
    /// The game's rest state (the body sits).
    pub resting: bool,
    pub cutscene: bool,
    /// The grace's own menu is up (top job, talk id 1000) / another window of the game's is (a top job otherwise).
    pub grace_menu: bool,
    pub other_window: bool,
    /// Her talk runs: flag 4652 is on, or the popup's talk id became 3000 during this rest.
    pub talking: bool,
    /// She takes him to the Roundtable Hold: flag 104 on, 105 off (the game's warp and load follow).
    pub warp: bool,
    /// savepoint.rs is still pressing for the rest.
    pub pressing: bool,
}

/// What savepoint.rs is to do in a frame of the accord's rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    None,
    /// Start asking the game for its rest (the body's action button, repeated by savepoint.rs's own flow).
    Rest,
    Back,
}

/// How it ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Her talk (or the scene and her talk) was reached; the text says how it ended.
    Talked(&'static str),
    /// Clean, but nothing of hers came.
    Nothing(&'static str),
    /// A step failed: the step, for `!!! story:`.
    Failed(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The black cover is up this frame.
    pub cover: bool,
    /// The player's own pad is kept from the game this frame (only our B reaches it).
    pub mask: bool,
    pub act: Act,
    /// A line for the log (a change of stage).
    pub log: Option<&'static str>,
    pub end: Option<End>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    /// The cover goes up; the press follows `COVER_LEAD` later.
    Up { since: f32 },
    /// The game is asked to rest.
    Pressing { since: f32 },
    /// The body rests, covered: the scene (or her own talk) is awaited.
    Resting { since: f32 },
    /// The event's cutscene plays (uncovered).
    Scene { since: f32 },
    /// Her talk (uncovered; the pad is the game's): until the rest ends, or the grace menu is back.
    Talk { since: f32, seen: bool },
    /// Covered: B until the body stands.
    Leave { since: f32, presses: u8, next_at: f32, talked: bool, why: &'static str },
    Done,
}

/// THE ACCORD'S REST, DRIVEN: cover, the game's rest, the scene, her talk, the rest ending by itself. The grace menu is
/// never used: if it comes, it is covered at once and left with B. Every path ends in `Done` with the cover down
/// within the limits below, whatever the game does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    pub stage: Stage,
}

impl Default for Drive {
    fn default() -> Drive {
        Drive::new()
    }
}

impl Drive {
    /// Seconds the cover is up before the game's button is pressed.
    pub const COVER_LEAD: f32 = 0.2;
    /// Seconds the game has to begin its rest (MEASURED: its own "touch" of 4.7 s comes first at a save point unlocked
    /// by its flag alone).
    pub const PRESS_LIMIT: f32 = 14.0;
    /// Seconds a rest waits, covered, for the scene / her own talk / the grace menu (MEASURED: the menu 1.8 - 2.2 s, the
    /// accord's scene 1.5 s after the rest began).
    pub const REST_LIMIT: f32 = 12.0;
    /// The longest talk / scene before the rest is left for the player.
    pub const TALK_LIMIT: f32 = 600.0;
    pub const SCENE_LIMIT: f32 = 600.0;
    /// Leaving: B every `BACK_EVERY` seconds, at most `BACKS` times, `LEAVE_LIMIT` seconds in all.
    pub const BACK_EVERY: f32 = 0.9;
    pub const BACKS: u8 = 7;
    pub const LEAVE_LIMIT: f32 = 10.0;

    pub fn new() -> Drive {
        Drive { stage: Stage::Up { since: 0.0 } }
    }

    /// The player's stick and buttons are not Sora's: the body rests and the game has the pad (her talk, her choices).
    pub fn holds_pad(&self) -> bool {
        !matches!(self.stage, Stage::Up { .. } | Stage::Pressing { .. } | Stage::Done)
    }

    fn leave(&mut self, talked: bool, why: &'static str) -> Step {
        self.stage = Stage::Leave { since: 0.0, presses: 0, next_at: 0.15, talked, why };
        Step { cover: true, mask: true, act: Act::None, log: Some(why), end: None }
    }

    fn done(&mut self, end: End) -> Step {
        self.stage = Stage::Done;
        Step { cover: false, mask: false, act: Act::None, log: None, end: Some(end) }
    }

    /// ONE FRAME.
    pub fn step(&mut self, dt: f32, seen: Seen) -> Step {
        let hold = |cover: bool, mask: bool| Step { cover, mask, act: Act::None, log: None, end: None };
        match self.stage {
            Stage::Done => hold(false, false),
            Stage::Up { since } => {
                let since = since + dt;
                if since >= Self::COVER_LEAD {
                    self.stage = Stage::Pressing { since: 0.0 };
                    return Step { act: Act::Rest, log: Some("the black cover is up: the game's real rest is asked for"), ..hold(true, false) };
                }
                self.stage = Stage::Up { since };
                hold(true, false)
            }
            Stage::Pressing { since } => {
                let since = since + dt;
                self.stage = Stage::Pressing { since };
                if seen.cutscene {
                    self.stage = Stage::Scene { since: 0.0 };
                    return Step { log: Some("a cutscene began: the cover is lifted"), ..hold(false, false) };
                }
                if seen.resting {
                    self.stage = Stage::Resting { since: 0.0 };
                    return hold(true, true);
                }
                if !seen.pressing || since > Self::PRESS_LIMIT {
                    // (he stands: nothing to leave)
                    return self.done(End::Failed("the rest: the game did not rest (its prompt went, or the presses did not reach it)"));
                }
                hold(true, false)
            }
            Stage::Resting { since } => {
                let since = since + dt;
                self.stage = Stage::Resting { since };
                if seen.cutscene {
                    self.stage = Stage::Scene { since: 0.0 };
                    return Step { log: Some("the scene's first frame: the cover is lifted"), ..hold(false, false) };
                }
                if seen.talking {
                    self.stage = Stage::Talk { since: 0.0, seen: true };
                    return Step { log: Some("MELINA SPEAKS BY HERSELF: the cover is lifted"), ..hold(false, false) };
                }
                if !seen.resting {
                    return self.done(End::Nothing("the rest ended by itself before anything came"));
                }
                if seen.grace_menu {
                    return self.leave(false, "the game's grace menu came up and no scene began: the rest is left");
                }
                if since > Self::REST_LIMIT {
                    return self.leave(false, "nothing came of the rest in time (no scene, no talk, no menu): the rest is left");
                }
                hold(true, true)
            }
            Stage::Scene { since } => {
                let since = since + dt;
                self.stage = Stage::Scene { since };
                if !seen.cutscene {
                    if !seen.resting {
                        return self.done(End::Talked("the scene is over and the body stands"));
                    }
                    self.stage = Stage::Talk { since: 0.0, seen: false };
                    return Step { log: Some("the scene is over: her talk is the game's (its choices take the pad)"), ..hold(false, false) };
                }
                if since > Self::SCENE_LIMIT {
                    return self.done(End::Failed("the scene: it did not end"));
                }
                hold(false, false)
            }
            Stage::Talk { since, seen: was } => {
                let since = since + dt;
                let now = was || seen.talking;
                self.stage = Stage::Talk { since, seen: now };
                if seen.warp {
                    return self.done(End::Talked("she takes him to the Roundtable Hold: the game's own warp follows (nothing is pressed)"));
                }
                if since > Self::TALK_LIMIT {
                    return self.leave(now, "her talk: it did not end in time: the rest is left");
                }
                if seen.cutscene {
                    return hold(false, false);
                }
                if !seen.resting {
                    return self.done(End::Talked("the rest ended by itself (her script ends it)"));
                }
                if seen.grace_menu {
                    return self.leave(true, "her talk is over and the game's grace menu is back: covered at once, the rest is left for the player");
                }
                hold(false, false)
            }
            Stage::Leave { since, presses, next_at, talked, why } => {
                let since = since + dt;
                let end = |talked: bool| if talked { End::Talked("the rest was left for the player after her talk") } else { End::Nothing(why) };
                if !seen.resting {
                    return self.done(end(talked));
                }
                if seen.warp {
                    return self.done(End::Talked("she takes him to the Roundtable Hold: the game's own warp follows"));
                }
                if seen.cutscene {
                    self.stage = Stage::Scene { since: 0.0 };
                    return hold(false, false);
                }
                if presses >= Self::BACKS || since > Self::LEAVE_LIMIT {
                    // (the last resort: the cover never stays; what is then on screen is the game's)
                    return self.done(End::Failed("leaving the rest: the body still sits after every B (the cover is lifted all the same; B leaves the game's menu)"));
                }
                if since >= next_at {
                    self.stage = Stage::Leave { since, presses: presses + 1, next_at: since + Self::BACK_EVERY, talked, why };
                    return Step { act: Act::Back, ..hold(true, true) };
                }
                self.stage = Stage::Leave { since, presses, next_at, talked, why };
                hold(true, true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    const REST: Seen = Seen { resting: true, cutscene: false, grace_menu: false, other_window: false, talking: false, warp: false, pressing: false };
    const MENU: Seen = Seen { grace_menu: true, ..REST };
    const STAND: Seen = Seen { resting: false, ..REST };

    fn flags(on: &[u32]) -> impl Fn(u32) -> Option<bool> + '_ {
        move |f| Some(on.contains(&f))
    }

    fn clean(_: i32) -> bool {
        false
    }

    /// A step that was not a plain hold: (time, act, end).
    type Mark = (f32, Act, Option<End>);

    /// Runs the machine against a script of (what is seen, for how many seconds); returns every step that was not a
    /// plain hold, with its time, and whether the cover was up in each frame.
    fn run(drive: &mut Drive, script: &[(Seen, f32)]) -> (Vec<Mark>, Vec<bool>) {
        let (mut out, mut covers, mut t) = (Vec::new(), Vec::new(), 0.0f32);
        for (seen, seconds) in script {
            for _ in 0..(*seconds / DT).round() as usize {
                let s = drive.step(DT, *seen);
                t += DT;
                covers.push(s.cover);
                if s.act != Act::None || s.end.is_some() {
                    out.push((t, s.act, s.end));
                }
            }
        }
        (out, covers)
    }

    /// Runs a summon against a script; returns (time, order, end) of every step that did something.
    fn call(s: &mut Summon, script: &[(Near, f32)]) -> Vec<(f32, Order, Option<End>)> {
        let (mut out, mut t) = (Vec::new(), 0.0f32);
        for (seen, seconds) in script {
            for _ in 0..(*seconds / DT).round() as usize {
                let st = s.step(DT, *seen);
                t += DT;
                if st.order != Order::None || st.end.is_some() {
                    out.push((t, st.order, st.end));
                }
            }
        }
        out
    }

    #[test]
    fn the_gates_are_the_scripts() {
        let base = [953, 4680];
        assert_eq!(menu_gate(&flags(&base), 60), Ok(()));
        assert_eq!(menu_gate(&flags(&base), 10), Ok(()), "Stormveil");
        assert!(menu_gate(&flags(&base), 61).is_err() && menu_gate(&flags(&base), 21).is_err() && menu_gate(&flags(&base), 41).is_err(), "the expansion's maps");
        assert!(menu_gate(&flags(&[4680]), 60).is_err(), "before the accord's scene");
        assert!(menu_gate(&flags(&[953, 110]), 60).is_err() && menu_gate(&flags(&[953, 108]), 60).is_err() && menu_gate(&flags(&[953, 11102790]), 60).is_err());
        assert!(menu_gate(&flags(&[953, 11009260]), 60).is_err());
        assert!(menu_gate(&flags(&[953, 11009260, 400001]), 60).is_ok());
        assert!(menu_gate(&flags(&[953, 400394, 12012711]), 60).is_err(), "Ranni's entry");
        assert!(menu_gate(&flags(&[953, 400394, 12012711, 12019280]), 60).is_ok());
        // an unreadable flag counts as off for a veto and as off for the need: no entry
        assert!(menu_gate(&|_| None, 60).is_err());
    }

    /// THE ROUNDTABLE INVITATION (x60 conditions a / b): after the accord, with Margit's introduction (10000851) or a
    /// grace outside Limgrave (3062 .. 3065), until she was heard (10009655); never with one of the six effects on him (4270: travel closed here).
    #[test]
    fn she_speaks_by_herself_as_the_graces_x30_says() {
        let accord = [951, 953, 4680];
        assert!(!own_talk(&flags(&accord), &clean, 60), "nothing to say yet");
        assert!(own_talk(&flags(&[951, 953, 4680, 10000851]), &clean, 60), "Margit was met");
        assert!(own_talk(&flags(&[951, 953, 4680, 3063]), &clean, 10), "a grace outside Limgrave, at Stormveil");
        assert!(!own_talk(&flags(&[951, 953, 4680, 10000851, 10009655]), &clean, 60), "heard: the menu's entry takes over");
        assert!(!own_talk(&flags(&[951, 953, 4680, 10000851]), &|e| e == 4270, 60), "travel closed here (4270): not by the script");
        assert!(!own_talk(&flags(&[951, 953, 4680, 10000851]), &clean, 61), "the expansion's maps: no x30");
        assert!(!own_talk(&flags(&[951, 953, 10000851]), &clean, 60), "the accord refused (4680 off)");
        assert!(!own_talk(&flags(&[953, 4680, 10000851, 110]), &clean, 60) && !own_talk(&flags(&[953, 4680, 10000851, 11102790]), &clean, 60));
        // after Morgott (x60 condition d: 9104 on, 400001 off); x61 when she has left (11009260); x62 with 108
        assert!(own_talk(&flags(&[953, 4680, 9104]), &clean, 60));
        assert!(!own_talk(&flags(&[953, 4680, 9104, 400001]), &clean, 60));
        assert!(own_talk(&flags(&[953, 4680, 11009260, 9104]), &clean, 60) && !own_talk(&flags(&[953, 4680, 11009260]), &clean, 60));
        assert!(own_talk(&flags(&[953, 4680, 108]), &clean, 60) && !own_talk(&flags(&[953, 4680, 108, 35009360]), &clean, 60));
        // x60 condition c: one of 11002741 .. 11002744 before Morgott
        assert!(own_talk(&flags(&[953, 4680, 11002742]), &clean, 60) && !own_talk(&flags(&[953, 4680, 11002742, 11009260, 400001]), &clean, 60));
        // x59: right after the accord's scene (4698)
        assert!(own_talk(&flags(&[953, 4698]), &clean, 60) && !own_talk(&flags(&[4698]), &clean, 60));
    }

    #[test]
    fn the_menus_topic_is_the_first_of_x57_that_holds() {
        // the accord refused (4680 off): asked again (x57 state 2), or nothing once 1042372700 is on
        assert_eq!(topic(&flags(&[951, 953]), &clean), Some(Some(1042379200)));
        assert_eq!(topic(&flags(&[951, 953, 1042372700]), &clean), None);
        assert_eq!(topic(&flags(&[951, 953, 1042372700, 4699]), &clean), Some(None), "the developer entry");
        // nothing pending after the accord: no entry
        assert_eq!(topic(&flags(&[951, 953, 4680]), &clean), None);
        // told to wait for the Roundtable Hold: "Tell her you're off to Roundtable Hold" (10009656) wins over a place's talk
        assert_eq!(topic(&flags(&[953, 4680, 10009655, 1042372701]), &clean), Some(Some(10009656)));
        assert_eq!(topic(&flags(&[953, 4680, 10009655, 105]), &clean), None, "reached the Hold");
        // ... one of the six effects skips that entry; the place's talk is then not taken either (r0 = 10009655 on)
        assert_eq!(topic(&flags(&[953, 4680, 10009655, 1042372701]), &|e| e == 4286), None);
        // Limgrave's first talk near its grace (1042372701), then its follow-up (r1 = 1042379203 heard)
        assert_eq!(topic(&flags(&[953, 4680, 1042372701]), &clean), Some(Some(1042379202)));
        assert_eq!(topic(&flags(&[953, 4680, 1042379203]), &clean), Some(Some(1042379206)));
        assert_eq!(topic(&flags(&[953, 4680, 1042379203, 1042372702]), &clean), None);
        // the Forge of the Giants (x57 state 22)
        assert_eq!(topic(&flags(&[953, 4680, 1054532702]), &clean), Some(Some(1054539215)));
        // the registers that hold across conditions: 3940 (r2) for the Liurnia pair, 35002731 (r3), 35002733 (r4)
        assert_eq!(topic(&flags(&[953, 4680, 1036482710, 1036489213, 3940]), &clean), Some(Some(1036489250)));
        assert_eq!(topic(&flags(&[953, 4680, 1036482710, 1036489213]), &clean), None);
        assert_eq!(topic(&flags(&[953, 4680, 35002731, 35009351]), &clean), Some(Some(35009352)));
        assert_eq!(topic(&flags(&[953, 4680, 35002732, 35002733]), &clean), Some(Some(35009358)));
        assert_eq!(topic(&flags(&[953, 4680, 35002732]), &clean), Some(Some(35009355)));
        // every topic x57 can set is one of those x43 clears
        for on in [
            vec![953, 4680, 10009655],
            vec![953, 4680, 1042372703],
            vec![953, 4680, 1046382701],
            vec![953, 4680, 1043342700],
            vec![953, 4680, 1038502710],
            vec![953, 4680, 1043532710],
            vec![953, 4680, 11002740],
            vec![953, 4680, 1043502700],
            vec![953, 4680, 1054552700],
            vec![953, 4680, 1037512700],
            vec![953, 4680, 1039512710, 1039519209, 1036489210, 3940],
            vec![953, 4680, 11002745],
            vec![953, 4680, 1052562700],
            vec![953, 4680, 35002730],
            vec![953, 4680, 11109387],
            vec![953, 4680, 1049539207],
            vec![951, 953],
        ] {
            let t = topic(&flags(&on), &clean);
            assert!(matches!(t, Some(Some(f)) if TOPIC_FLAGS.contains(&f)), "{on:?} -> {t:?}");
        }
    }

    #[test]
    fn talk_to_melina_runs_her_own_talk_first_then_the_menus_entry() {
        // the invitation is due and a place's talk is near: her own talk first, as x30 runs before the menu
        assert_eq!(talk_for(&flags(&[951, 953, 4680, 10000851, 1042372701]), &clean, 60), Some(Talk::Own));
        // told to wait: the menu's entry
        assert_eq!(talk_for(&flags(&[951, 953, 4680, 10000851, 10009655]), &clean, 60), Some(Talk::Topic(Some(10009656))));
        // the accord refused: asked again from the menu
        assert_eq!(talk_for(&flags(&[951, 953]), &clean, 60), Some(Talk::Topic(Some(1042379200))));
        // nothing: no entry; the menu's gate holds (the Forge is shown only where the menu could hold it)
        assert_eq!(talk_for(&flags(&[951, 953, 4680]), &clean, 60), None);
        assert_eq!(talk_for(&flags(&[953, 4680, 1054532702]), &clean, 60), Some(Talk::Topic(Some(1054539215))));
        assert_eq!(talk_for(&flags(&[953, 4680, 1054532702, 108]), &clean, 60), Some(Talk::Own), "with 108: x62, she speaks by herself");
        assert_eq!(talk_for(&flags(&[953, 4680, 1054532702, 108, 35009360]), &clean, 60), None, "x43's gate: 108");
        assert_eq!(talk_for(&flags(&[4680, 10000851]), &clean, 60), None, "before the accord");
    }

    #[test]
    fn her_appearing_and_leaving_are_the_graces() {
        assert_eq!(appear(&flags(&[4680])), Some((20000, 3.5)));
        assert_eq!(appear(&flags(&[4680, 400001])), Some((20001, 3.5)));
        assert_eq!(appear(&flags(&[4680, 400001, 108])), Some((20000, 3.5)));
        assert_eq!(appear(&flags(&[4698])), Some((20006, 0.5)));
        assert_eq!(appear(&flags(&[4651])), None, "already there: no summon");
        // x36: 4680 off -> sent away at once (20010); x67 at the rest's end otherwise
        assert_eq!(leave_anim(Talk::Topic(Some(1042379200)), &flags(&[400001])), 20010);
        assert_eq!(leave_anim(Talk::Topic(Some(1054539215)), &flags(&[4680, 400001])), 20011);
        assert_eq!(leave_anim(Talk::Topic(Some(1054539215)), &flags(&[4680, 400001, 11002745])), 20010);
        assert_eq!(leave_anim(Talk::Own, &flags(&[4680])), 20010);
        assert_eq!(leave_anim(Talk::Own, &flags(&[4680, 400001])), 20011);
        assert_eq!(leave_anim(Talk::Own, &flags(&[4680, 400001, 108])), 20010);
        // the commands as the script writes them
        assert_eq!((SUMMON_COMMAND, SUMMON_HEAD, SUMMON_TAIL, DISMISS_COMMAND), (128, [2180, 21800000, 1, 3000], [1, 2, 60], 129));
        assert_eq!(MELINA_FLAGS, [4651, 4652, 4653, 4654, 4655, 4656, 4657]);
    }

    const GRACE: Near = Near { grace_talk: true, marker: false, talking: false, warp: false, invited: false };

    /// THE WHOLE TALK: the grace's object seen, summoned, 3.5 s of her appearing, talk started, her script clears 4652,
    /// she leaves. No rest, nothing pressed, no cover.
    #[test]
    fn talk_to_melina_summons_her_and_sends_her_away_after_her_talk() {
        let mut s = Summon::new(Talk::Topic(Some(1054539215)), Some((20000, 3.5)));
        assert!(!s.holds_pad());
        let there = Near { marker: true, ..GRACE };
        let talking = Near { talking: true, ..there };
        // (savepoint.rs sets 4652 at Start: from then on it reads on)
        let steps = call(&mut s, &[(GRACE, 0.1), (talking, 23.6)]);
        assert_eq!(steps.iter().map(|x| x.1).collect::<Vec<_>>(), [Order::Summon(20000), Order::Start], "{steps:?}");
        assert!(steps[0].0 < 0.03, "at once");
        assert!((steps[1].0 - steps[0].0 - 3.5).abs() < 0.03, "after her appearing");
        assert!(s.holds_pad(), "her talk has the pad");
        // her script ends the talk: sent away in that frame
        let end = s.step(DT, there);
        assert_eq!((end.order, end.end), (Order::Dismiss, Some(End::Talked("her talk is over (her script cleared flag 4652): she leaves"))));
        assert_eq!(s.phase, Phase::Done);
        assert!(!s.holds_pad());
        let after = s.step(DT, there);
        assert!(after.order == Order::None && after.end.is_none(), "nothing after Done");
    }

    #[test]
    fn the_summon_waits_for_the_grace_and_for_no_other_melina() {
        // the grace's talk object not seen yet (the world was paused under the menu): waits, then summons
        let mut s = Summon::new(Talk::Own, Some((20000, 3.5)));
        let steps = call(&mut s, &[(Near { grace_talk: false, ..GRACE }, 1.0), (GRACE, 0.1)]);
        assert_eq!(steps.first().map(|x| x.1), Some(Order::Summon(20000)));
        assert!(steps[0].0 > 1.0);
        // another Melina is still there (9600 / 9603 on the player): x35 waits; she goes: summoned
        let mut s = Summon::new(Talk::Own, Some((20000, 3.5)));
        let steps = call(&mut s, &[(Near { marker: true, ..GRACE }, 2.0), (GRACE, 0.1)]);
        assert_eq!(steps.first().map(|x| x.1), Some(Order::Summon(20000)));
        // never seen / never gone: nothing done, nothing set, said why
        for world in [Near { grace_talk: false, ..GRACE }, Near { marker: true, ..GRACE }] {
            let mut s = Summon::new(Talk::Own, Some((20000, 3.5)));
            let steps = call(&mut s, &[(world, Summon::CLEAR_LIMIT + 1.0)]);
            assert_eq!(steps.len(), 1);
            assert!(matches!(steps[0], (_, Order::None, Some(End::Failed(_)))), "{steps:?}");
        }
        // 4651 already on: no summon, the talk is started at once (x36 state 0 -> 4 -> 3)
        let mut s = Summon::new(Talk::Topic(Some(1042379200)), None);
        let steps = call(&mut s, &[(Near { marker: true, ..GRACE }, 0.1)]);
        assert_eq!(steps.first().map(|x| x.1), Some(Order::Start));
    }

    #[test]
    fn every_summon_ends_and_sends_her_away() {
        // she never came (x36's other way out: 5 s with no 9600 / 9603): the flags are cleared all the same
        let mut s = Summon::new(Talk::Own, Some((20000, 3.5)));
        let steps = call(&mut s, &[(GRACE, 0.1), (Near { talking: true, ..GRACE }, 9.6)]);
        assert!(matches!(steps.last(), Some((_, Order::Dismiss, Some(End::Nothing(_))))), "{steps:?}");
        // the Roundtable Hold: the game's warp follows, nothing is sent away under it
        let mut s = Summon::new(Talk::Own, Some((20000, 3.5)));
        let there = Near { marker: true, talking: true, ..GRACE };
        let steps = call(&mut s, &[(GRACE, 0.1), (there, 33.6), (Near { warp: true, ..there }, 0.1)]);
        assert!(matches!(steps.last(), Some((_, Order::None, Some(End::Talked(t)))) if t.contains("Roundtable")), "{steps:?}");
        // a talk that never ends: sent away after the limit
        let mut s = Summon::new(Talk::Own, Some((20000, 3.5)));
        let steps = call(&mut s, &[(GRACE, 0.1), (there, Summon::TALK_LIMIT + 5.0)]);
        assert!(matches!(steps.last(), Some((_, Order::Dismiss, Some(End::Failed(_))))));
        // a failed command: ended, sent away when she may be there
        let mut s = Summon::new(Talk::Own, Some((20000, 3.5)));
        assert_eq!(s.abort(true, "x").order, Order::Dismiss);
        assert_eq!(s.phase, Phase::Done);
        // EVERY WORLD: whatever is seen for ever, it is Done within the limits, and once summoned it ends sending her
        // away or in the warp
        let worlds = [GRACE, Near { grace_talk: false, ..GRACE }, Near { marker: true, ..GRACE }, Near { talking: true, ..GRACE }, Near { talking: true, marker: true, ..GRACE }, Near { warp: true, ..GRACE }, Near { talking: true, marker: true, invited: true, ..GRACE }, Near { talking: true, invited: true, ..GRACE }];
        for appear in [Some((20000, 3.5)), Some((20006, 0.5)), None] {
            for first in worlds {
                for second in worlds {
                    let mut s = Summon::new(Talk::Own, appear);
                    let (mut t, mut summoned, mut last) = (0.0f32, false, None);
                    while s.phase != Phase::Done && t < Summon::CLEAR_LIMIT + Summon::TALK_LIMIT + 30.0 {
                        let st = s.step(DT, if t < 2.0 { first } else { second });
                        summoned |= matches!(st.order, Order::Summon(_) | Order::Start);
                        last = Some(st);
                        t += DT;
                    }
                    assert_eq!(s.phase, Phase::Done, "{appear:?} {first:?} -> {second:?}");
                    let last = last.unwrap();
                    assert!(last.end.is_some());
                    if summoned && !matches!(last.end, Some(End::Talked(t)) if t.contains("Roundtable")) {
                        assert_eq!(last.order, Order::Dismiss, "{appear:?} {first:?} -> {second:?}: {last:?}");
                    }
                }
            }
        }
    }

    /// (MELINA-RT) Her script from x77 on, as t000003000 is written (the parts that decide flag 104): x77 state 2 waits for
    /// her line's end (query 57) or 9601 on the player (query 61); x80 state 3 waits for 9601 on the player ONLY; x80
    /// state 4 waits for its state time above 11 and the talk's end; then x80 state 1 sets 11109786 and 104. The model is
    /// fed only what the summon gives the player (`hand`); her line is a fixed time.
    struct HerScript {
        stage: u8,
        state_time: f32,
        flag_104: bool,
    }

    impl HerScript {
        fn frame(&mut self, dt: f32, talk_ended: bool, player_9601: bool) {
            self.state_time += dt;
            let next = match self.stage {
                0 if player_9601 || talk_ended => 1,
                1 if player_9601 => 2,
                2 if self.state_time > 11.0 && talk_ended => 3,
                s => s,
            };
            if next != self.stage {
                self.stage = next;
                self.state_time = 0.0;
                self.flag_104 |= next == 3;
            }
        }
    }

    /// Every step that did something, the first frame the hand was given, the moment her script set 104.
    type Replay = (Vec<(f32, Order, Option<End>)>, Option<f32>, Option<f32>);

    /// Replays the user's 2026-11-10 17:00 session (er/logs/kh-er.log): "Talk to Melina" at grace 10001955 with topic
    /// 10009656 (10009655 on since the load, 105 off), command 128 at 145.736, her appearing 3.5 s, 4652 at 149.236,
    /// her line 10010600 / 10010601 3.47 s (to 152.705), then nothing until 165.15 (12.4 s): no 104, no warp.
    fn the_logs_roundtable_talk(hand_given: bool) -> Replay {
        let mut s = Summon::new(Talk::Topic(Some(10009656)), Some((20000, 3.5)));
        let mut her = HerScript { stage: 0, state_time: 0.0, flag_104: false };
        let (mut out, mut t, mut first_hand, mut warp_at) = (Vec::new(), 0.0f32, None, None);
        // the summon at 145.736 is t = 0; 4652 from Start (3.5 s); the log ends at 165.15 (19.4 s); then 3 s more
        let line_end = 3.5 + (152.705 - 149.236);
        while t < 165.15 - 145.736 + 3.0 {
            let started = matches!(s.phase, Phase::Talk { .. } | Phase::Done);
            let seen = Near { grace_talk: true, marker: started, talking: started && !her.flag_104, warp: her.flag_104, invited: true };
            let st = s.step(DT, seen);
            t += DT;
            let hand = st.hand && hand_given;
            if hand && first_hand.is_none() {
                first_hand = Some(t);
            }
            if started {
                her.frame(DT, t >= line_end, hand);
            }
            if her.flag_104 && warp_at.is_none() {
                warp_at = Some(t);
            }
            if st.order != Order::None || st.end.is_some() {
                out.push((t, st.order, st.end));
            }
        }
        (out, first_hand, warp_at)
    }

    #[test]
    fn the_logs_roundtable_talk_gets_the_touch_of_her_hand_and_the_games_warp() {
        let (steps, first_hand, warp_at) = the_logs_roundtable_talk(true);
        assert_eq!(steps.iter().map(|x| x.1).take(2).collect::<Vec<_>>(), [Order::Summon(20000), Order::Start]);
        let first_hand = first_hand.expect("the touch of her hand is given");
        assert!((first_hand - 3.5).abs() < 0.05, "from her talk's start (4652): {first_hand}");
        let warp_at = warp_at.expect("her script sets 104 (the old code never did: the log's 12.4 s after her line had none)");
        assert!(warp_at < 165.15 - 145.736, "within the time the user waited: {warp_at}");
        let last = steps.last().unwrap();
        assert!(matches!(last, (_, Order::None, Some(End::Talked(t))) if t.contains("Roundtable")), "the game's warp, she is not sent away: {steps:?}");
        assert!(steps.iter().all(|x| x.1 != Order::Dismiss));
        assert_eq!(steps.iter().filter(|x| x.2.is_some()).count(), 1, "one end");
    }

    #[test]
    fn without_the_touch_of_her_hand_her_script_waits_for_ever_as_in_the_users_log() {
        // the old code (no 9601 given): x80 state 3 never moves, 104 never comes, the summon stays in her talk
        let (steps, first_hand, warp_at) = the_logs_roundtable_talk(false);
        assert!(first_hand.is_none() && warp_at.is_none());
        assert!(steps.iter().all(|x| x.2.is_none()), "nothing ended in the log's 12.4 s after her line: {steps:?}");
    }

    /// (MELINA-RT2) Her script from x77 on, query by query as t000003000 asks them: x77 state 0 calls x79 (her line
    /// 10010600; x79 returns once the line is up), x77 state 2 asks `61 9601 == 1 OR 57 == 1`, x80 state 3 asks
    /// `61 9601 == 1`, x80 state 4 asks `103 > 11` and only then `57 == 1`; then 11109786 and 104 (stage 4). Every answer
    /// goes through `ask` (the query detour: the game's answer, or the one `Hurry` gives).
    struct X80 {
        stage: u8,
        state_time: f32,
        flag_104: bool,
    }

    impl X80 {
        fn frame(&mut self, dt: f32, line_up: bool, talk_over: bool, player_9601: bool, ask: &mut dyn FnMut(i32, Option<f32>, f32) -> f32) {
            self.state_time += dt;
            let has = if player_9601 { 1.0 } else { 0.0 };
            let next = match self.stage {
                0 if line_up => 1,
                1 if ask(QUERY_PLAYER_EFFECT, Some(HAND as f32), has) == 1.0 || talk_over => 2,
                2 if ask(QUERY_PLAYER_EFFECT, Some(HAND as f32), has) == 1.0 => 3,
                3 if ask(QUERY_STATE_TIME, None, self.state_time) > X80_WAIT && talk_over => 4,
                s => s,
            };
            if next != self.stage {
                self.stage = next;
                self.state_time = 0.0;
                self.flag_104 |= next == 4;
            }
        }
    }

    /// The user's log 2026-11-10 (er/logs/kh-er.log, DEPLOY12): command 128 at 35.670 (t = 0), 4652 at 39.170, her
    /// invitation sets 10009655 and the hand is given at 49.888, her line 10010600 at 50.173, "talk: #1 ends" at 52.522,
    /// 104 ("DONE") at 61.156, "sora: the player is gone" at 61.912, "loadin: THE LOADING SCREEN is up" at 64.339.
    const T0: f32 = 35.670;
    const T_INVITED: f32 = 49.888 - T0;
    const T_LINE: f32 = 50.173 - T0;
    const T_TALK_OVER: f32 = 52.522 - T0;
    /// Common event 945's own part (104 -> WarpPlayer -> the old map gone -> the new map's first frame): the game's.
    const WARP_TO_GONE: f32 = 61.912 - 61.156;
    const WARP_TO_LOADING: f32 = 64.339 - 61.156;

    /// Her summoned talk replayed with these timings: the first frame of the hand, the frame her script set 104, the
    /// steps that did something. `hurry`: the query detour's `Hurry` (MELINA-RT2) or the game's answers alone (DEPLOY12);
    /// `agrees`: her talk reaches x77 (he goes to the Roundtable Hold), or not (her talk just ends).
    fn the_logs_warp(hurry: bool, agrees: bool) -> Replay {
        const INS: usize = 0x7ff6_1234_5670;
        let mut s = Summon::new(Talk::Own, Some((20000, 3.5)));
        let mut her = X80 { stage: 0, state_time: 0.0, flag_104: false };
        let mut rule = Hurry::default();
        let (mut out, mut t, mut first_hand, mut warp_at, mut returned) = (Vec::new(), 0.0f32, None, None, false);
        while t < 40.0 && s.phase != Phase::Done {
            let started = matches!(s.phase, Phase::Talk { .. });
            // x46's exit clears 4652 once her calls return: after 104, or at the talk's end when she never reached x77
            let seen = Near { grace_talk: true, marker: started, talking: started && !returned, warp: her.flag_104, invited: t >= T_INVITED };
            let st = s.step(DT, seen);
            t += DT;
            if st.hand && first_hand.is_none() {
                first_hand = Some(t);
            }
            if started && agrees {
                let armed = st.hand;
                her.frame(DT, t >= T_LINE, t >= T_TALK_OVER, st.hand, &mut |id, arg, answer| if hurry { rule.on_query(armed, INS, id, arg, answer).unwrap_or(answer) } else { answer });
            }
            returned |= her.flag_104 || (!agrees && t >= T_TALK_OVER);
            if her.flag_104 && warp_at.is_none() {
                warp_at = Some(t);
            }
            if st.order != Order::None || st.end.is_some() {
                out.push((t, st.order, st.end));
            }
        }
        (out, first_hand, warp_at)
    }

    #[test]
    fn the_logs_roundtable_warp_follows_her_talks_end_at_once() {
        let (steps, first_hand, warp_at) = the_logs_warp(true, true);
        assert!((first_hand.unwrap() - T_INVITED).abs() < 0.05);
        let warp_at = warp_at.expect("her script sets 104 itself");
        // her script moves on on the frame the talk is over (query 57): nothing in between
        assert!(warp_at >= T_TALK_OVER && warp_at - T_TALK_OVER < 0.05, "104 {:.3} s after the talk's end", warp_at - T_TALK_OVER);
        // what the user sees: the old map gone under a second after her talk, the loading screen about 3 s after it
        // (event 945's own part, the game's), where the log had 9.4 / 11.8 s
        assert!(warp_at + WARP_TO_GONE - T_TALK_OVER < 0.85);
        assert!(warp_at + WARP_TO_LOADING - T_TALK_OVER < 3.3, "loading screen {:.2} s after her talk", warp_at + WARP_TO_LOADING - T_TALK_OVER);
        let last = steps.last().unwrap();
        assert!(matches!(last, (_, Order::None, Some(End::Talked(t))) if t.contains("Roundtable")), "the game's warp, she is not sent away: {steps:?}");
        assert!(steps.iter().all(|x| x.1 != Order::Dismiss));
    }

    #[test]
    fn without_the_hurry_her_wait_is_the_logs_eleven_seconds() {
        // DEPLOY12 (the game's answers alone): 104 at 61.156 in the log = her line + 11 s; the loading screen 11.8 s after her talk
        let (_, _, warp_at) = the_logs_warp(false, true);
        let warp_at = warp_at.expect("104 after her own wait");
        assert!((warp_at - (61.156 - T0)).abs() < 0.1, "104 at {:.3} (the log: {:.3})", warp_at + T0, 61.156);
        let loading = warp_at + WARP_TO_LOADING - T_TALK_OVER;
        assert!((loading - (64.339 - 52.522)).abs() < 0.1 && loading > 11.0, "{loading}");
    }

    #[test]
    fn no_hurry_when_she_is_told_no() {
        // her talk ends without x77 (no line 10010600, no query 61 for 9601): no 104, she is sent away as before
        let (steps, first_hand, warp_at) = the_logs_warp(true, false);
        assert!(first_hand.is_some() && warp_at.is_none());
        assert!(matches!(steps.last(), Some((_, Order::Dismiss, Some(End::Talked(_))))), "{steps:?}");
    }

    #[test]
    fn the_hurry_answers_only_her_state_time_after_she_asked_for_the_hand() {
        let (a, b) = (0x1000usize, 0x2000usize);
        let mut h = Hurry::default();
        // armed, before x77: her state time is the game's (x45 state 10's 103 > 10 and the rest)
        assert_eq!(h.on_query(true, a, QUERY_STATE_TIME, None, 2.0), None);
        // other effects, or 9601 not carried: nothing latched
        assert_eq!(h.on_query(true, a, QUERY_PLAYER_EFFECT, Some(9604.0), 1.0), None);
        assert_eq!(h.on_query(true, a, QUERY_PLAYER_EFFECT, Some(HAND as f32), 0.0), None);
        assert_eq!(h.on_query(true, a, QUERY_STATE_TIME, None, 2.0), None);
        // x77 / x80 asked for the hand and got it: her state time is read X80_WAIT ahead, another talk object's is not
        assert_eq!(h.on_query(true, a, QUERY_PLAYER_EFFECT, Some(HAND as f32), 1.0), None);
        assert_eq!(h.on_query(true, a, QUERY_STATE_TIME, None, 0.5), Some(0.5 + X80_WAIT));
        assert_eq!(h.on_query(true, b, QUERY_STATE_TIME, None, 0.5), None);
        assert_eq!(h.on_query(true, a, 57, Some(0.0), 0.0), None, "query 57 stays the game's");
        assert_eq!(h.raised, 1);
        // the window over (104, the summon done): the game's answers again, the latch gone
        assert_eq!(h.on_query(false, a, QUERY_STATE_TIME, None, 0.5), None);
        assert_eq!(h, Hurry::default());
        h.on_query(true, a, QUERY_PLAYER_EFFECT, Some(HAND as f32), 1.0);
        h.disarm();
        assert_eq!(h.on_query(true, a, QUERY_STATE_TIME, None, 0.5), None);
    }

    #[test]
    fn the_touch_of_her_hand_only_in_the_roundtable_context_and_never_after_104() {
        let talking = Near { marker: true, talking: true, ..GRACE };
        // not invited (any other topic): never
        let mut s = Summon::new(Talk::Topic(Some(1042379200)), Some((20000, 3.5)));
        let mut handed = false;
        for _ in 0..(20.0 / DT) as usize {
            handed |= s.step(DT, if matches!(s.phase, Phase::Clear { .. }) { GRACE } else { talking }).hand;
        }
        assert!(!handed);
        // invited: from 4652 on, every frame, and the first frame logs it once
        let mut s = Summon::new(Talk::Topic(Some(10009656)), None);
        assert!(!s.step(DT, Near { invited: true, ..GRACE }).hand, "Start: 4652 is set this frame");
        let a = s.step(DT, Near { invited: true, ..talking });
        let b = s.step(DT, Near { invited: true, ..talking });
        assert!(a.hand && a.log.is_some() && b.hand && b.log.is_none());
        // 104: no more, the summon is done in the game's warp
        let w = s.step(DT, Near { invited: true, warp: true, ..talking });
        assert!(!w.hand && matches!(w.end, Some(End::Talked(t)) if t.contains("Roundtable")));
        // her talk over without 104 (she was told no): no more, sent away as before
        let mut s = Summon::new(Talk::Own, None);
        s.step(DT, Near { invited: true, ..GRACE });
        assert!(s.step(DT, Near { invited: true, ..talking }).hand);
        let e = s.step(DT, Near { invited: true, ..GRACE });
        assert!(!e.hand && e.order == Order::Dismiss);
        // a talk that never ends while invited still ends at the limit
        let mut s = Summon::new(Talk::Own, None);
        let steps = call(&mut s, &[(Near { invited: true, ..GRACE }, DT), (Near { invited: true, ..talking }, Summon::TALK_LIMIT + 1.0)]);
        assert!(matches!(steps.last(), Some((_, Order::Dismiss, Some(End::Failed(_))))), "{steps:?}");
    }

    #[test]
    fn the_accord_is_scene_then_her_talk_then_the_rest_ends_by_itself() {
        // MEASURED (the user's log 1128 - 1243 s): rest, the scene 1.5 s in, her talk, her choice box, the rest ends itself
        let mut d = Drive::new();
        let scene = Seen { cutscene: true, ..REST };
        let (steps, covers) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 6.3), (REST, 1.5), (scene, 14.0), (REST, 2.5), (Seen { talking: true, ..REST }, 43.0), (Seen { other_window: true, talking: true, ..REST }, 9.0), (REST, 38.0), (STAND, 0.1)]);
        assert_eq!(steps.iter().map(|s| s.1).collect::<Vec<_>>(), [Act::Rest, Act::None], "nothing but the rest is ever pressed: {steps:?}");
        assert_eq!(steps[1].2, Some(End::Talked("the rest ended by itself (her script ends it)")));
        let scene_at = ((6.3 + 1.5) / DT) as usize;
        assert!(covers[..scene_at - 2].iter().all(|c| *c) && covers[scene_at + 2..].iter().all(|c| !*c), "covered until the scene's first frame, never after");
        // the scene did not come (the menu did): left, covered, nothing selected in it
        let mut d = Drive::new();
        let (steps, covers) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 0.5), (REST, 2.0), (MENU, 0.5), (STAND, 0.1)]);
        assert!(steps.iter().all(|s| matches!(s.1, Act::Rest | Act::Back | Act::None)));
        assert!(matches!(steps.last().unwrap().2, Some(End::Nothing(_))));
        assert!(covers[..covers.len() - 6].iter().all(|c| *c), "never uncovered while the grace menu is up");
        // her talk ends in the grace menu: covered in that frame, B, he stands
        let mut d = Drive { stage: Stage::Talk { since: 0.0, seen: true } };
        let first = d.step(DT, MENU);
        assert!(first.cover && first.mask);
        let (steps, _) = run(&mut d, &[(MENU, 0.5), (STAND, 0.1)]);
        assert_eq!(steps.iter().map(|s| s.1).collect::<Vec<_>>(), [Act::Back, Act::None]);
        assert_eq!(steps[1].2, Some(End::Talked("the rest was left for the player after her talk")));
    }

    #[test]
    fn she_takes_him_to_the_roundtable_hold_and_nothing_fights_the_warp() {
        let mut d = Drive::new();
        let talking = Seen { talking: true, ..REST };
        let (steps, covers) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 0.5), (REST, 4.0), (talking, 30.0), (Seen { warp: true, ..talking }, 0.1)]);
        assert_eq!(steps.iter().map(|s| s.1).collect::<Vec<_>>(), [Act::Rest, Act::None]);
        assert!(matches!(steps[1].2, Some(End::Talked(t)) if t.contains("Roundtable")));
        assert!(!covers[covers.len() - 1]);
        assert_eq!(d.stage, Stage::Done);
        // also while leaving (the flag comes as the menu returns): no further B
        let mut d = Drive { stage: Stage::Leave { since: 0.0, presses: 0, next_at: 5.0, talked: true, why: "x" } };
        let s = d.step(DT, Seen { warp: true, grace_menu: true, ..REST });
        assert!(s.act == Act::None && !s.cover && matches!(s.end, Some(End::Talked(_))));
    }

    #[test]
    fn every_stuck_rest_ends_standing_with_the_cover_down() {
        // EVERY STUCK WORLD: whatever is seen for ever, from any reachable stage, the machine is Done with the cover down
        // within the limits, and never presses more than the limits allow
        let worlds = [
            Seen { resting: false, pressing: true, ..REST },
            Seen { resting: false, pressing: false, ..REST },
            REST,
            MENU,
            Seen { other_window: true, ..REST },
            Seen { cutscene: true, ..REST },
            Seen { talking: true, ..REST },
            Seen { talking: true, grace_menu: true, ..REST },
        ];
        for (a, first) in worlds.iter().enumerate() {
            for (b, second) in worlds.iter().enumerate() {
                for lead in [0.5f32, 3.0, 9.0] {
                    let mut d = Drive::new();
                    let mut presses = 0;
                    let mut last = None;
                    let limit = Drive::TALK_LIMIT + Drive::SCENE_LIMIT + 60.0;
                    let mut t = 0.0;
                    while d.stage != Stage::Done && t < limit {
                        let seen = if t < lead { *first } else { *second };
                        let s = d.step(DT, seen);
                        presses += usize::from(s.act != Act::None);
                        last = Some(s);
                        t += DT;
                    }
                    assert_eq!(d.stage, Stage::Done, "worlds {a} -> {b} lead {lead}: stuck in {:?}", d.stage);
                    let last = last.unwrap();
                    assert!(!last.cover && last.end.is_some(), "worlds {a} -> {b}: {last:?}");
                    assert!(presses <= 1 + 2 * Drive::BACKS as usize, "{presses} presses");
                    // after Done: nothing, for ever
                    let after = d.step(DT, *second);
                    assert!(!after.cover && !after.mask && after.act == Act::None && after.end.is_none());
                }
            }
        }
    }

    #[test]
    fn the_cover_is_never_up_longer_than_the_steps_limits() {
        let mut d = Drive::new();
        let (_, covers) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 13.9), (REST, 11.9), (MENU, 60.0)]);
        let longest = covers.split(|c| !*c).map(|run| run.len()).max().unwrap_or(0) as f32 * DT;
        assert!(longest < Drive::PRESS_LIMIT + Drive::REST_LIMIT + Drive::LEAVE_LIMIT + 1.0, "{longest}");
        assert_eq!(d.stage, Stage::Done);
        assert!(!covers[covers.len() - 1]);
    }
}

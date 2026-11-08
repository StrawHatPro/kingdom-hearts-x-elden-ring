//! (THE USER 2026-11-02: "Mimic Tear needs to mimic Sora, the mimic Sora can just use Sora's party member AI and uses his
//! current equipment (mimic sora is unable to use Summon) make sure mimic sora uses the shaders that mimic should use from
//! Elden Ring"; 2026-11-03: "build mimic Sora", "mimic Sora also needs to use the same outfit that Sora is currently using")
//! MIMIC SORA, THE ELDEN RING SIDE: the Mimic Tear ash (goods 207000 .. 207010) never calls Elden Ring's copy of the
//! Tarnished (its buddy system is never asked for it: spirit.rs's Mimic Tear branch comes here instead), it calls a SECOND
//! SORA. UNTESTED IN THE GAME until a log says otherwise.
//!
//! THE FOUR PIECES (kh-partyhost mimic.rs's plan, as built):
//! 1. HIS AI: AI Sora's own (kh-partyhost `MimicAi`: Goofy's tree `n_ex002` on its own controller, actor `MIMIC_ACTOR`, AI
//!    Sora's tools and settings, the friends' follow behind a running leader, the warp when he is far and unseen), following
//!    WHOEVER LEADS: the player's Sora (the trees' "player"), else the friend leader. Out of the formation (no camp-menu
//!    entry, no team attack, never Summon: his core's Summon rules are kept shut every step).
//! 2. HIS SIMULATION: his own `kh_core::Sora` (the copy of the leader Sora's kit at the summon, `kh_partyhost::mimic::
//!    copy_kit`), stepped in the host's fixed step right after the party's ([`step`]) against the same world and Elden
//!    Ring's characters (ErTargets: what ER's own lock-on accepts, the party's rule), his hits x the Mimic Tear's upgrade
//!    (`dopingSpEffect`'s damage rate) and fired by HIS OWN BODY (allybody_live `owner_of`: the enemies' hate goes to him); his
//!    pose by his own Animator of Sora's pack and his own keychain (loaded on a thread before the first summon; a fresh one
//!    for every summon); his effects and sounds through Sora's effect side with every place on him resolved to his own
//!    bones (`Attach::Sora` -> a world point; one held on him, e.g. a trail or an aura, moved to his bone after every step), their handles his own.
//! 3. HIS BODY: a fourth key of the ally-body pool (allybody.rs `Action::SpawnMimic`, `AllyWant::mimic`): AI Sora's body
//!    route on the spirit team, so Elden Ring's lock-on and its enemies treat him as an ally, his body is never one of
//!    Sora's targets (`is_body`), and a hit on it reaches his core ([`body_hit`]: his own maximum, / the upgrade's defense).
//! 4. HIS DRAW: kh-er-render `mimic::submit_mimic` (a second instance of the worn model, the Mimic Tear's surface from
//!    WetAspectParam 18 into the game's G-buffer, his KH colour pass skipped: Elden Ring's own light passes shade him).
//!    THE OUTFIT (THE USER 2026-11-03): he is drawn in the outfit Sora wears (the renderer's worn one); the outfit at the
//!    summon is kept in his kit for the log. DECIDED: a change of outfit while he is out (only at Boc's Outfits screen, a
//!    conversation) redraws him in the new one: (1) the user's words are "the same outfit that Sora is CURRENTLY using";
//!    (2) an outfit is a look, not equipment: everything of play (stats, gear, abilities, magic, items) stays the summon's
//!    copy as Elden Ring's Mimic Tear copies; (3) the renderer holds one outfit's assets: a frozen second outfit would need a
//!    second outfit set loaded beside the worn one.
//!
//! THE RULES of spirit.rs stay in force (it runs his timer of the ash's level, the KH spirit gauge with the Mimic icon,
//! Unsummon's refund, the save point's end); he leaves when it says ([`dismiss`]) or when his HP reach 0 ([`live`] says
//! so; spirit.rs ends him as Died). The summon itself empties Sora's MP into MP Charge (kh-core `command::summon`).
//!
//! KEYS: `summon.mimic` (1; 0 = the Mimic Tear greyed again). Logs: `mimic:` lines.

#[cfg(feature = "party")]
pub use live::*;

/// (THE USER 2026-11-03: "mimic sora, upon first being summoned, needs to play the proper mimic tear animation transforming
/// into Sora"; his death: "KH3 knockdown + silver dissolve") HIS ARRIVAL AND HIS LEAVING, the timing (pure, unit-tested).
///
/// ELDEN RING'S OWN, read from the game's data (build/sora_tmp/party/SUMMON/arrival_re/, offline, 2.7.2.0):
/// - ARRIVAL = the Mimic Tear's BuddyParam `generateAnimId` 60500 (c0000 a00.tae): sound c999997800 at 0 s ("a spirit
///   appears", the same in every spirit's arrival), the body HIDDEN 0 .. 2.1 s, FFX 640 at the weapon dummy 0.67 .. 1.83 s,
///   its opacity 0 -> 1 over 2.1 .. 3.1 s, SpEffect 16316 (its WetAspect 18 liquid silver) refreshed at 3.0 s;
/// - DISMISS (its time out, the player's dismissal) = the game's DisappearAll, anim 60505: sound c999997801 at 0 s, FFX 651
///   on the head / arms / legs / spine 0 .. 1.17 s, its opacity 1 -> 0 over 0.17 .. 1.67 s;
/// - DEATH = the death clip, then the game's spirit fade (ChrFadeOutDeleteSlot kind 3, every spirit's SpEffect 4302): sound
///   s999997700 at the start, a 2.0 s fade with sound s999997730 when it begins.
///
/// OURS ON MIMIC SORA: the same clock and sounds (kh-er ersound's play of the game's own sound ids); the game's FFX 640 / 651
/// / 691 cannot be spawned at a point by id (no SpEffect row carries the arrival or the dismiss effect; the death chain
/// 295100 ends in a permanent camouflage), so THE LIQUID IS DRAWN BY US with the Mimic Tear's own surface: a pool of liquid
/// silver at his feet (kh-er-render `MimicForm::pool`) and his figure FORMING from the feet up out of it where the game
/// fades its copy in (`MimicForm::height`, the forming writers: a liquid band at the edge), and the same backwards when he
/// goes. His death first plays KH3's knockdown (his own core's KO, as AI Sora's), then the silver dissolve.
#[cfg(feature = "party")]
pub mod phase {
    /// ARRIVAL (s): the pool fills to the game's FFX start, he forms until the game's fade-in ends, the pool drains.
    pub const POOL_FULL: f32 = 0.67;
    pub const WHOLE: f32 = 3.1;
    pub const ARRIVED: f32 = 3.6;
    /// DISMISS (s): the game's fade-out 0.17 .. 1.67 s, the pool drains after.
    pub const MELT_START: f32 = 0.17;
    pub const MELT_END: f32 = 1.67;
    pub const POOL_IN: f32 = 0.3;
    pub const POOL_OUT: f32 = 0.5;
    /// DEATH (s): KH3's knockdown held, then the game's spirit fade (2.0 s).
    pub const KO_HOLD: f32 = 2.0;
    pub const DEATH_FADE: f32 = 2.0;
    /// The pool's radius (cm).
    pub const POOL_CM: f32 = 60.0;
    /// The game's sounds: (kind, id) for kh-er ersound `play_at` ('c' = 1, 's' = 5).
    pub const SOUND_ARRIVE: (i32, i32) = (1, 999_997_800);
    pub const SOUND_DISMISS: (i32, i32) = (1, 999_997_801);
    pub const SOUND_DEATH: (i32, i32) = (5, 999_997_700);
    pub const SOUND_DEATH_FADE: (i32, i32) = (5, 999_997_730);

    /// Where he is in his summon.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Phase {
        /// Seconds since the summon: he forms (no AI, no body to hit).
        Arriving(f32),
        Out,
        /// Seconds since he was dismissed; `death`: his HP reached 0 (the knockdown first).
        Leaving { t: f32, death: bool },
    }

    /// (his formed height 0..1, the pool's radius cm) of a phase.
    pub fn form(p: Phase) -> (f32, f32) {
        let ease = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        match p {
            Phase::Out => (1.0, 0.0),
            Phase::Arriving(t) => {
                let height = ease((t - POOL_FULL) / (WHOLE - POOL_FULL));
                let pool = if t < WHOLE { ease(t / POOL_FULL) } else { 1.0 - ease((t - WHOLE) / (ARRIVED - WHOLE)) };
                (height, POOL_CM * pool)
            }
            Phase::Leaving { t, death } => {
                let (start, melt) = melt_window(death);
                let t = t - start;
                if t < 0.0 {
                    return (1.0, 0.0);
                }
                let height = 1.0 - ease((t - MELT_START) / melt);
                let end = MELT_START + melt;
                let pool = if t < end { ease(t / POOL_IN) } else { 1.0 - ease((t - end) / POOL_OUT) };
                (height, POOL_CM * pool)
            }
        }
    }

    /// (the dissolve's start after the dismissal, its length) s.
    fn melt_window(death: bool) -> (f32, f32) {
        if death {
            (KO_HOLD, DEATH_FADE)
        } else {
            (0.0, MELT_END - MELT_START)
        }
    }

    /// The phase after `dt` s; None = he is gone (the leaving is over).
    pub fn advance(p: Phase, dt: f32) -> Option<Phase> {
        match p {
            Phase::Arriving(t) if t + dt >= ARRIVED => Some(Phase::Out),
            Phase::Arriving(t) => Some(Phase::Arriving(t + dt)),
            Phase::Out => Some(Phase::Out),
            Phase::Leaving { t, death } => {
                let (start, melt) = melt_window(death);
                (t + dt < start + MELT_START + melt + POOL_OUT).then_some(Phase::Leaving { t: t + dt, death })
            }
        }
    }

    /// The game's sounds that start within (t0, t1] of a phase's clock.
    pub fn sounds(p: Phase, t0: f32, t1: f32) -> Vec<(i32, i32)> {
        // (a sound at 0 s plays in the first step, whose clock starts at exactly 0)
        let at = |s: f32| (t0 < s || (s <= 0.0 && t0 <= 0.0)) && s <= t1;
        match p {
            Phase::Arriving(_) => [(0.0, SOUND_ARRIVE)].into_iter().filter(|(s, _)| at(*s)).map(|x| x.1).collect(),
            Phase::Out => Vec::new(),
            Phase::Leaving { death: false, .. } => [(0.0, SOUND_DISMISS)].into_iter().filter(|(s, _)| at(*s)).map(|x| x.1).collect(),
            Phase::Leaving { death: true, .. } => [(0.0, SOUND_DEATH), (KO_HOLD, SOUND_DEATH_FADE)].into_iter().filter(|(s, _)| at(*s)).map(|x| x.1).collect(),
        }
    }

    /// He takes no AI input (forming, leaving).
    pub fn still(p: Phase) -> bool {
        !matches!(p, Phase::Out)
    }
}

#[cfg(not(feature = "party"))]
mod off {
    use glam::Vec3;
    use kh_core::command::summon::LeaveReason;

    /// No party host in this build (`--no-default-features`): no Mimic Sora (the Mimic Tear is greyed).
    pub fn available() -> bool {
        false
    }

    pub fn spawn(_sora: &kh_core::Sora, _at: Vec3, _yaw: f32, _level: u8, _full_time: f32) -> bool {
        false
    }

    pub fn dismiss(_reason: LeaveReason) {}

    pub fn live() -> Option<(i32, i32)> {
        None
    }

    pub fn frame(_control: &crate::control::Control) {}

    pub fn why_not() -> String {
        "this build has no party host (the cargo feature `party`)".to_string()
    }
}

#[cfg(not(feature = "party"))]
pub use off::*;

#[cfg(feature = "party")]
mod live {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    use glam::{Quat, Vec3};
    use kh_anim::{Animator, BodySlot, Frame as PoseFrame, Settings, Trs};
    use kh_core::command::summon::LeaveReason;
    use kh_core::host::{Ally, AttackShape, FinishKind, HitResult, HurtShape, PushBody, TargetBody, TargetDefense, TargetInfo};
    use kh_core::states::HitOnPlayer;
    use kh_core::{EventKind, HitRecord, Input, Output, Pose, Sora, TargetId, Targets};
    use kh_partyhost::mimic::{self as kit, MimicKit};
    use kh_partyhost::{AllyWant, MimicAi, MimicScene, PartyRuntime};

    use crate::allybody::rules::AllyHit;
    use crate::control::Control;
    use crate::erlog;
    use crate::targets::ErTargets;
    use crate::world::ErWorld;

    const STEP: f32 = 1.0 / 60.0;
    /// He is called this far to the side of Sora (cm): beside him, never inside him.
    const BESIDE_CM: f32 = 150.0;
    /// The Keyblade's appear / disappear sequence (sora.rs `WeaponShow`'s numbers).
    const WEAPON_DELAY: f32 = 0.22;
    const WEAPON_RAMP: f32 = 0.06;
    /// (MIMIC-FX) His own member id (kh_audio::copies): his effect / sound handles in its party range, never one of the player's
    /// Sora's (two cores count from 1 alike), his own taps, effects and audio state.
    const MEMBER: u8 = kh_audio::copies::MIMIC_SORA;
    /// Above his highest bone: the hair's tips and the top of his head (cm), for the forming cut's full height.
    const TOP_CM: f32 = 12.0;
    /// The longest his arrival waits for the renderer's pipeline states (steps, 60 Hz).
    const HOLD_STEPS: u64 = 240;

    /// His pose side: an Animator of Sora's pack and the Keyblade's keychain. (MIMIC-BOSS) shared with the Mimic Tear bosses'
    /// Sora copies (mimic_boss.rs), which load their own.
    pub(crate) struct Rig {
        pub(crate) animator: Animator,
        pub(crate) keychain: Option<kh_anim::Keychain>,
        /// (KEYCHAIN-DUAL) his hand keyblade's own keychain (not the Kingdom Key) and his form parts' (kbdraw.rs, the player's rule)
        pub(crate) chains: crate::sora::kbdraw::Keychains,
        /// (FORMWEP-FIX) his hand Keyblade between a form's start and its transformation (kh-host formstart.rs, the player's rule)
        pub(crate) form_gate: kh_host::formstart::FormStartGate,
    }

    enum Load {
        Idle,
        Loading(std::thread::JoinHandle<Result<Rig, String>>),
        Ready(Box<Rig>),
        Failed(String),
    }

    /// Mimic Sora while he is out.
    struct Out {
        core: Sora,
        ai: Option<MimicAi>,
        kit: MimicKit,
        rig: Box<Rig>,
        out: Output,
        out_before: Output,
        pose: kh_anim::Pose,
        pose_before: kh_anim::Pose,
        chain: Vec<Trs>,
        chain_before: Vec<Trs>,
        weapon: (bool, f32),
        weapon_shown: bool,
        hits: Vec<HitOnPlayer>,
        steps: u64,
        told_draw: bool,
        down_told: bool,
        /// (THE USER 2026-11-03) his arrival / leaving (`phase`), the clock of the phase as of the last step (its sounds).
        phase: super::phase::Phase,
        /// His height standing (cm, from his first pose: the forming cut's full height).
        tall: Option<f32>,
    }

    struct State {
        pack: Option<PathBuf>,
        load: Load,
        out: Option<Out>,
        /// The party host runs with its AI pack (his AI): set by every step the host makes.
        party_ok: bool,
        on: bool,
        /// (TEST AID, the test profile only) `test.mimic_down`: a lethal blow on Mimic Sora (his knockdown and dissolve seen in game).
        down_shot: Option<crate::control::OneShot>,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);
    /// His core's step is applying a hit on an enemy (allybody_live `owner_of`: his own body fires it).
    static FIRING: AtomicBool = AtomicBool::new(false);

    fn state() -> std::sync::MutexGuard<'static, Option<State>> {
        STATE.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
        let mut g = state();
        let s = g.get_or_insert_with(|| State { pack: None, load: Load::Idle, out: None, party_ok: false, on: true, down_shot: None });
        f(s)
    }

    /// sora.rs at the start of a Sora host: where Sora's pack is (his Animator and keychain are read from it).
    pub fn set_pack(pack: &Path) {
        with(|s| s.pack = Some(pack.to_path_buf()));
    }

    /// His core's step is applying a hit (allybody_live `owner_of`).
    pub fn firing() -> bool {
        FIRING.load(Ordering::Relaxed)
    }

    /// (MIMIC-BOSS) where Sora's pack is (the bosses' copies read their rigs from it).
    pub(crate) fn pack() -> Option<PathBuf> {
        with(|s| s.pack.clone())
    }

    pub(crate) fn load_rig(pack: &Path) -> Result<Rig, String> {
        let settings = Settings { leg_ik: true, kine_driver: true, face: true, bonamik: true, first_update_holds_position: kh_core::anim::FIRST_UPDATE_HOLDS_POSITION };
        let animator = kh_anim::Pack::load(pack).and_then(|p| Animator::new(p, settings))?;
        let keychain = kh_anim::Keychain::load(&pack.join("weapon")).ok();
        Ok(Rig { animator, keychain, chains: Default::default() /* (KEYCHAIN-DUAL) */, form_gate: Default::default() /* (FORMWEP-FIX) */ })
    }

    /// The loading of his next rig, moved on (a thread; never on the game thread's time).
    fn poll_load(s: &mut State) {
        match std::mem::replace(&mut s.load, Load::Idle) {
            Load::Idle => {
                if let Some(pack) = s.pack.clone() {
                    let started = std::time::Instant::now();
                    s.load = Load::Loading(std::thread::spawn(move || {
                        let r = load_rig(&pack);
                        if r.is_ok() {
                            erlog!("mimic: Mimic Sora's pose side (an Animator of {} and the keychain) read in {:.2} s on its own thread", pack.display(), started.elapsed().as_secs_f32());
                        }
                        r
                    }));
                }
            }
            Load::Loading(h) if h.is_finished() => {
                s.load = match h.join() {
                    Ok(Ok(rig)) => Load::Ready(Box::new(rig)),
                    Ok(Err(e)) => {
                        erlog!("mimic: !!! Mimic Sora's pose side could not be read ({e}): the Mimic Tear stays greyed");
                        Load::Failed(e)
                    }
                    Err(_) => Load::Failed("the loading thread panicked".to_string()),
                }
            }
            other => s.load = other,
        }
    }

    /// Mimic Sora can be called now: his pose side is read, the party host runs with its AI, `summon.mimic` is on and no
    /// Mimic Sora is out. (THE USER: the Mimic Tear is offered greyed while not.)
    pub fn available() -> bool {
        with(|s| {
            poll_load(s);
            s.on && s.party_ok && s.out.is_none() && matches!(s.load, Load::Ready(_))
        })
    }

    /// Why the Mimic Tear is greyed (for the log).
    pub fn why_not() -> String {
        with(|s| match &s.load {
            _ if !s.on => "summon.mimic = 0".to_string(),
            _ if !s.party_ok => "no party host with its AI pack runs (party.er = 0 or no PACK/party/ai)".to_string(),
            _ if s.out.is_some() => "Mimic Sora is out".to_string(),
            Load::Failed(e) => format!("his pose side failed: {e}"),
            Load::Ready(_) => "ready".to_string(),
            _ => "his pose side is being read".to_string(),
        })
    }

    /// The place beside Sora: `BESIDE_CM` to his right.
    pub fn beside(at: Vec3, yaw: f32) -> Vec3 {
        let r = (yaw - 90.0).to_radians();
        at + Vec3::new(r.cos(), r.sin(), 0.0) * BESIDE_CM
    }

    /// CALL HIM (spirit.rs, the Mimic Tear's Spawn): the copy of `sora`'s kit (the leader Sora at this moment) at the ash's
    /// `level`, beside Sora at `at` (KH3 feet) facing `yaw`; `full_time`: spirit.rs's timer (for the log). True: he is out.
    pub fn spawn(sora: &Sora, at: Vec3, yaw: f32, level: u8, full_time: f32) -> bool {
        let mut g = state();
        let Some(s) = g.as_mut() else { return false };
        poll_load(s);
        if s.out.is_some() || !s.party_ok || !s.on {
            return false;
        }
        let Load::Ready(rig) = std::mem::replace(&mut s.load, Load::Idle) else { return false };
        let mut k = kit::copy_kit(sora, level);
        k.outfit = crate::boc::worn();
        let place = beside(at, yaw);
        let mut core = kit::make_core(&k, Pose { position: Vec3::new(place.x, place.y, at.z + 5.0), yaw });
        core.set_body_scale(crate::boc::body_scale());
        let out = core.output();
        erlog!("mimic: MIMIC SORA IS CALLED beside Sora ({full_time:.0} s on the spirit timer): {}", kit::describe(&k));
        s.out = Some(Out { core, ai: None, kit: k, rig, out_before: out.clone(), out, pose: kh_anim::Pose::default(), pose_before: kh_anim::Pose::default(), chain: Vec::new(), chain_before: Vec::new(), weapon: (false, 10.0), weapon_shown: false, hits: Vec::new(), steps: 0, told_draw: false, down_told: false, phase: super::phase::Phase::Arriving(0.0), tall: None });
        // (the next summon's rig, read meanwhile: a fresh Animator for every Mimic Sora)
        poll_load(s);
        true
    }

    /// HE LEAVES (spirit.rs: Unsummon, the timer, his death, the save point, a rest, the mode's end). (THE USER 2026-11-03:
    /// "KH3 knockdown + silver dissolve") Died: his KO, then the silver dissolve; Unsummon / the timer / the save point / a
    /// rest: the dissolve at once (`phase`); the mode's end or a load (Gone): at once, nothing drawn. From the dismissal on he
    /// is no spirit any more (spirit.rs's gauge goes), takes no input, and his body is released (nobody wants it).
    pub fn dismiss(reason: LeaveReason) {
        let now = reason == LeaveReason::Gone;
        let gone = with(|s| {
            let o = s.out.as_mut()?;
            erlog!("mimic: Mimic Sora LEAVES ({}; {} steps, HP {}/{}, {} hit(s) waiting dropped){}", reason.name(), o.steps, o.core.status().hp, o.core.status().max_hp(), o.hits.len(), if now { ": at once" } else if reason == LeaveReason::Died { ": KH3's knockdown, then the silver dissolve" } else { ": the silver dissolve" });
            o.hits.clear();
            // (MIMIC-FX) his effects end with his leave (his dissolve and his KO still play); gone at once: everything of his
            if now {
                crate::fx::copyfx::copy_gone(MEMBER, true);
                return s.out.take().map(|_| ());
            }
            crate::fx::copyfx::end_effects(MEMBER);
            if !matches!(o.phase, super::phase::Phase::Leaving { .. }) {
                o.phase = super::phase::Phase::Leaving { t: 0.0, death: reason == LeaveReason::Died };
            }
            None
        });
        if gone.is_some() {
            kh_er_render::mimic::clear_mimic();
        }
    }

    /// While he is a spirit (arriving or out): (HP, max HP); HP 0 = he is down (spirit.rs: Died). None = no Mimic Sora, or
    /// he is leaving.
    pub fn live() -> Option<(i32, i32)> {
        with(|s| s.out.as_ref().filter(|o| !matches!(o.phase, super::phase::Phase::Leaving { .. })).map(|o| (o.core.status().hp.max(0), o.core.status().max_hp().max(1))))
    }

    /// (DEATHPLANE) Where Mimic Sora stands for Elden Ring's death barriers while he is out: (feet KH3, on the floor, alive).
    pub fn barrier_view() -> Option<(Vec3, bool, bool)> {
        with(|s| s.out.as_ref().filter(|o| o.phase == super::phase::Phase::Out).map(|o| (o.out.position, o.out.on_floor, o.core.status().hp > 0)))
    }

    /// (DEATHPLANE) A death barrier took him: the lethal blow on his own core (his KO, then spirit.rs ends him as Died: KH3's
    /// knockdown and the silver dissolve). True: given.
    pub fn barrier_down() -> bool {
        with(|s| match s.out.as_mut().filter(|o| o.phase == super::phase::Phase::Out && o.core.status().hp > 0) {
            Some(o) => {
                // (KILLPLANE) instant and past every ability he has (deathplane.rs `kill_core`), never a hit
                o.hits.clear();
                crate::deathplane::kill_core(&mut o.core, o.out.position);
                true
            }
            None => false,
        })
    }

    /// (allybody pool) HIS BODY'S WANT this frame: the fourth key (AI Sora's capsule and route), hittable while he is up.
    pub fn want() -> Option<AllyWant> {
        with(|s| {
            // (forming: his body stands there, nobody may hit it; leaving: nobody wants it, the pool releases it)
            s.out.as_ref().filter(|o| !matches!(o.phase, super::phase::Phase::Leaving { .. })).map(|o| {
                let st = o.core.status();
                let ko = st.hp < 1;
                let forming = matches!(o.phase, super::phase::Phase::Arriving(_));
                AllyWant { chara: kh_host::party::PartyChara::Sora, feet: o.out.position, yaw: o.out.yaw, capsule: kh_partyhost::SORA_CAPSULE, hittable: !ko && !forming, invulnerable: st.damage_invalid || st.invincible || forming, ko, mimic: true }
            })
        })
    }

    /// (allybody `deliver`) A HIT ON HIS BODY: made final with HIS maximum (the share rule of every AI ally) and divided by
    /// the Mimic Tear's upgrade defense (`physicsDiffenceRate` of its doping row), queued for his next step. True: queued.
    pub fn body_hit(h: &AllyHit, params: &crate::ermap::Params) -> bool {
        with(|s| {
            let Some(o) = s.out.as_mut().filter(|o| o.core.status().hp > 0 && o.phase == super::phase::Phase::Out) else { return false };
            let mut hit = h.sora_hit(params, o.core.status().max_hp());
            hit.amount /= o.kit.upgrade.defense.max(0.01);
            erlog!("mimic: Mimic Sora's body is hit for {:.1} (/ {:.3} the Mimic Tear's defense) from {} of {} Elden Ring HP", hit.amount, o.kit.upgrade.defense, h.lost, h.ref_max);
            o.hits.push(hit);
            true
        })
    }

    /// The keys, once a frame (sora.rs): `summon.mimic`.
    pub fn frame(control: &Control) {
        with(|s| {
            s.on = control.flag("summon.mimic") != Some(false);
            poll_load(s);
            // (the coordinator's polish round 2026-11-03) the liquid's look keys (`r.mimic_pool_*`), and his pipeline states made ahead of
            // any summon once his pose side is read (the first summon of a session no longer waits for them)
            kh_er_render::mimic::set_look(kh_er_render::mimic::MimicLook::with_keys(&|k| control.f32(k)));
            if s.on && matches!(s.load, Load::Ready(_)) {
                kh_er_render::mimic::preload_mimic();
            }
            let shot = s.down_shot.get_or_insert_with(|| crate::control::OneShot::new("test.mimic_down", control));
            if shot.fired(control).is_some() {
                if let Some(o) = s.out.as_mut().filter(|o| o.phase == super::phase::Phase::Out) {
                    let hp = o.core.status().hp;
                    o.hits.push(HitOnPlayer::new(kh_core::enums::ETresDamageKind::Blow, (hp * 10).max(9999) as f32, TargetId::NONE, o.out.position + Vec3::X * 100.0));
                    erlog!("mimic: test.mimic_down: a lethal blow is given to Mimic Sora (HP {hp})");
                }
            }
        });
    }

    /// What the host lends his step.
    pub struct MimicStep<'a> {
        /// The party's one Sora core (the player's Sora, or AI Sora while a friend leads).
        pub sora: &'a Sora,
        pub world: &'a ErWorld,
        pub targets: &'a mut ErTargets,
        pub fx: &'a mut crate::fx::Fx,
        /// The party host (None: party.er = 0 / not made: he leaves).
        pub rt: Option<&'a PartyRuntime>,
        pub camera: Option<&'a kh_camera::CameraView>,
        pub lock_on: TargetId,
        pub dt: f32,
        pub now: f64,
    }

    /// ONE FIXED STEP OF MIMIC SORA (sora.rs, right after the party's step): his AI's input, the hits on his body, his core's
    /// step, his pose and keychain.
    pub fn step(m: MimicStep<'_>) {
        let MimicStep { sora, world, targets, fx, rt, camera, lock_on, dt, now } = m;
        let mut g = state();
        let Some(s) = g.as_mut() else { return };
        s.party_ok = rt.is_some_and(|r| r.ai_pack.is_some());
        let Some(o) = s.out.as_mut() else { return };
        let (Some(rt), Some(camera)) = (rt, camera) else {
            erlog!("mimic: the party host stopped: Mimic Sora leaves with it");
            crate::fx::copyfx::copy_gone(MEMBER, false); // (MIMIC-FX)
            s.out = None;
            kh_er_render::mimic::clear_mimic();
            return;
        };
        o.steps += 1;
        // (THE USER 2026-11-03) his arrival / leaving: the clock, the game's sounds at his feet, the end of the leaving. The
        // arrival's clock waits at 0 while the renderer still makes his pipeline states (the first summon of a session: in
        // game 2026-11-03 that took 1.8 s of the 3.6 s arrival), at most `HOLD_STEPS`
        let making = matches!(o.phase, super::phase::Phase::Arriving(t) if t <= 0.0) && o.steps < HOLD_STEPS && !kh_er_render::mimic::mimic_ready();
        if making && o.steps == 1 {
            erlog!("mimic: his arrival waits for the renderer's pipeline states (at most {:.0} s)", HOLD_STEPS as f32 / 60.0);
        }
        let (t0, t1) = (clock_of(o.phase), clock_of(o.phase) + if making { 0.0 } else { dt });
        for (kind, id) in super::phase::sounds(o.phase, t0, t1).into_iter().filter(|_| !making) {
            let played = crate::ersound::play_at(kind, id, world.frame.pos_kh_to_er(o.out.position));
            erlog!("mimic: the game's sound {}{id} at Mimic Sora ({}; his step {})", if kind == 1 { 'c' } else { 's' }, if played { "played" } else { "NOT played: the game's sound play is not available" }, o.steps);
        }
        let before = o.phase;
        match super::phase::advance(o.phase, if making { 0.0 } else { dt }) {
            Some(p) => o.phase = p,
            None => {
                erlog!("mimic: Mimic Sora has melted away ({} steps): gone", o.steps);
                crate::fx::copyfx::copy_gone(MEMBER, false); // (MIMIC-FX)
                s.out = None;
                kh_er_render::mimic::clear_mimic();
                return;
            }
        }
        if matches!(before, super::phase::Phase::Arriving(_)) && o.phase == super::phase::Phase::Out {
            erlog!("mimic: Mimic Sora has FORMED out of the silver ({:.1} s): his AI takes over", super::phase::ARRIVED);
        }
        let ai = o.ai.get_or_insert_with(|| {
            let a = MimicAi::new(rt);
            erlog!("mimic: Mimic Sora's AI: AI Sora's own ({}), following whoever leads; no Summon", if a.has_tree() { "Goofy's tree n_ex002 on its own controller" } else { "NO TREE PACK: he only follows and warps" });
            a
        });
        ai.set_caught(crate::grabs::mimic_step(&mut o.core, dt)); // (GRABS) his body held by a throw: caught (KH3's rows), his AI stopped until he is up
        let input = if super::phase::still(o.phase) {
            // (forming / leaving: he stands, his KO plays out; no AI, no warp)
            kit::keep_summon_shut(&mut o.core);
            Input { camera_yaw: camera.rotation.yaw, ..Input::default() }
        } else {
            ai.input(rt, &mut o.core, MimicScene { sora, lock_on, targets: &kh_party_targets(targets, sora), camera, camera_yaw: camera.rotation.yaw, nav: None, dt, now })
        };
        let input = if o.core.status().hp > 0 { input } else { Input { camera_yaw: input.camera_yaw, ..Input::default() } };
        for hit in o.hits.drain(..) {
            o.core.take_hit(hit);
        }
        o.core.set_body_scale(crate::boc::body_scale());
        // (MIMIC-FX) his seams: his own member (kh_audio::copies::MIMIC_SORA), his handles, his effects on HIS bones, Keyblade and
        // form (the effect side's Sora instance, after his pose below), his sounds on HIS taps
        let mut his_fx = kh_host::seam::EffectQueue::default();
        let out = {
            let mut effects = crate::fx::copyfx::SoraCopyFx { queue: &mut his_fx, member: MEMBER };
            let mut audio = crate::fx::copyfx::SoraCopyAudio { inner: &mut fx.audio, member: MEMBER, retarget: None };
            let mut mt = MimicTargets { inner: targets, damage: o.kit.upgrade.damage, at: o.out.position };
            let mut ui = kh_core::host::NullUi;
            let mut host = kh_core::Host { world, targets: &mut mt, audio: &mut audio, effects: &mut effects, ui: &mut ui };
            o.core.step_hosted(dt, &input, &mut host)
        };
        for e in &out.events {
            match &e.kind {
                EventKind::WeaponVisible { visible, instant } => {
                    if *instant && *visible {
                        o.weapon = (true, 10.0);
                    } else if *visible != o.weapon.0 {
                        o.weapon = (*visible, 0.0);
                    }
                }
                EventKind::PlayerDied { causer } if !o.down_told => {
                    o.down_told = true;
                    erlog!("mimic: MIMIC SORA IS DOWN (by target {}): he leaves (spirit.rs: Died)", causer.0);
                }
                _ => {}
            }
        }
        o.weapon.1 += dt;
        o.out_before = std::mem::replace(&mut o.out, out);
        o.pose_before = std::mem::take(&mut o.pose);
        pose_step(o, world);
        if o.pose_before.component.is_empty() {
            o.pose_before = o.pose.clone();
        }
        // (MIMIC-FX) his effects on HIS scene (his bones, his Keyblade, his core's actors), their sounds and his tap
        let target = |id: TargetId| targets.get(id).map(|e| e.pos + Vec3::Z * e.half_height);
        crate::fx::copyfx::sora_copy_step(fx, crate::fx::copyfx::SoraCopyStep { member: MEMBER, out: &o.out, core: &o.core, skeleton: o.rig.animator.skeleton(), pose: &o.pose, commands: std::mem::take(&mut his_fx.0), target: &target, retarget: None });
        chain_step(o);
    }

    /// The clock of a phase (s).
    fn clock_of(p: super::phase::Phase) -> f32 {
        match p {
            super::phase::Phase::Arriving(t) | super::phase::Phase::Leaving { t, .. } => t,
            super::phase::Phase::Out => 0.0,
        }
    }

    /// His targets as the party's AI sees them (the latch of "attacked lately" is the party's own; here: the hostiles in
    /// an attack near his leader).
    fn kh_party_targets<'a>(targets: &'a mut ErTargets, sora: &Sora) -> crate::party::aiworld::ErPartyTargets<'a> {
        let mut latch = 0;
        crate::party::aiworld::ErPartyTargets::new(targets, sora.output().position, &mut latch)
    }

    pub(crate) fn weapon_opacity(w: (bool, f32)) -> f32 {
        let ramp = ((w.1 - WEAPON_DELAY) / WEAPON_RAMP).clamp(0.0, 1.0);
        if w.0 {
            ramp
        } else {
            1.0 - ramp
        }
    }

    /// The mesh component in the KH3 world (sora.rs `component_to_world`).
    pub(crate) fn component_to_world(out: &Output) -> Trs {
        Trs { translation: out.position, rotation: Quat::from_rotation_z((out.yaw - 90.0).to_radians()), scale: Vec3::ONE }
    }

    /// The socket R_wep_skt relative to bone R_buki (sora.rs `weapon_socket`).
    pub(crate) fn weapon_socket() -> Trs {
        let half = std::f32::consts::FRAC_1_SQRT_2;
        Trs { translation: Vec3::ZERO, rotation: Quat::from_xyzw(half, 0.0, 0.0, half), scale: Vec3::ONE }
    }

    /// His pose of one step (sora.rs `step_pose_over` without the body's borrowed actions: he has no Elden Ring body action).
    fn pose_step(o: &mut Out, world: &ErWorld) {
        o.pose = pose_of(&mut o.rig, &crate::grabs::mimic_out(&o.out), o.core.status().hp > 0, world); // (GRABS) his caught clip while caught
    }

    /// (MIMIC-BOSS) the pose of one step of a Sora copy's rig (Mimic Sora's, the bosses' copies).
    pub(crate) fn pose_of(rig: &mut Rig, out: &Output, alive: bool, world: &ErWorld) -> kh_anim::Pose {
        let (shown, ticked) = (out.anim, out.anim_ticked);
        let sequences = ticked.name.sequences();
        let (samples, time, blend_with) = match shown.locomotion {
            Some(l) => (l.run.sample_weights(shown.axis), l.run_time, Some((l.walk.sequences()[0], l.walk_time, 1.0 - l.run_weight))),
            None => {
                let [(first, first_weight, first_time), (second, second_weight, second_time)] = shown.body_samples();
                if second_weight > 0.0 && second_time != first_time {
                    ([(first, 1.0), (first, 0.0)], first_time, Some((second, second_time, second_weight)))
                } else {
                    ([(first, first_weight), (second, second_weight)], first_time, None)
                }
            }
        };
        let body = BodySlot { samples, blend_with, notify_sequences: &sequences, time, weight: shown.weight, active: shown.active, notify_window: ticked.notify_window };
        rig.animator.set_leg_ik(alive);
        rig.animator.step(&PoseFrame { dt: STEP, body, component_to_world: component_to_world(out) }, &Ik(world)).clone()
    }

    /// His keychain of one step (sora.rs `step_chain`).
    fn chain_step(o: &mut Out) {
        let visible = weapon_opacity(o.weapon) > 0.0;
        chain_of(&mut o.rig, &o.core, &o.pose, &o.out, visible, ChainState { shown: &mut o.weapon_shown, chain: &mut o.chain, before: &mut o.chain_before });
    }

    /// (MIMIC-BOSS) A Sora copy's keychain state (the latest two steps, the weapon shown at the last one).
    pub(crate) struct ChainState<'a> {
        pub(crate) shown: &'a mut bool,
        pub(crate) chain: &'a mut Vec<Trs>,
        pub(crate) before: &'a mut Vec<Trs>,
    }

    /// (MIMIC-BOSS) the keychain of one step of a Sora copy's rig.
    /// (KEYCHAIN-DUAL) `core` = his simulation: the keyblade in his hand has its OWN keychain (the Kingdom Key's is `keychain`),
    /// and the Keyblade parts his form holds have theirs (Double Form's two; kbdraw.rs `step_parts`, the player's rule).
    pub(crate) fn chain_of(rig: &mut Rig, core: &Sora, pose: &kh_anim::Pose, out: &Output, visible: bool, st: ChainState<'_>) {
        let ChainState { shown, chain: now_chain, before } = st;
        rig.form_gate.step(&core.combat().form, out.state); // (FORMWEP-FIX) once a step, before anything can return
        let Some(chain) = rig.keychain.as_mut() else { return };
        let Some(hand) = rig.animator.skeleton().index_of("R_buki").and_then(|h| pose.component.get(h)).copied() else { return };
        let kb = crate::sora::kbdraw::StepParts { sora: core, out, skeleton: rig.animator.skeleton(), component: &pose.component, to_world: component_to_world(out), shown: visible, test_aid: false, start_pending: rig.form_gate.on() };
        let input = crate::sora::kbdraw::ChainIn { animator: Some(&rig.animator), clip: kb.clip() };
        rig.chains.parts.step(STEP, &crate::sora::kbdraw::step_parts(&kb), &input);
        if visible {
            let appeared = !*shown;
            if appeared {
                chain.reset();
            }
            let weapon_to_world = weapon_socket().then(&hand).then(&component_to_world(out));
            let base = crate::sora::kbdraw::base_of(core.status());
            // (KEYCHAIN-DUAL) the Kingdom Key's too: Sora's routed clip states and its own clip's (kbdraw `step_keychain`)
            let now = rig.chains.hand.step(&base, STEP, &weapon_to_world, &input, appeared).unwrap_or_else(|| crate::sora::kbdraw::step_keychain(chain, kh_er_render::keyblade::KINGDOM_KEY, STEP, None, &weapon_to_world, &input).to_vec());
            *before = std::mem::replace(now_chain, now);
            if appeared || before.len() != now_chain.len() {
                *before = now_chain.clone();
            }
        }
        *shown = visible;
    }

    /// ONCE A FRAME (sora.rs, after the party's drawing): Mimic Sora to the renderer, between his two latest steps (`alpha`,
    /// `r.interp`), with the Mimic Tear material (kh-er-render mimic.rs); nothing when he is not out.
    pub fn draw(control: &Control, alpha: f32) {
        // (MIMIC-BOSS) one list for the renderer: Mimic Sora and the Mimic Tear bosses' Sora copies (mimic_boss.rs)
        let mut list = crate::mimic_boss::mimic_draws(control, alpha);
        with(|s| {
            let Some(o) = s.out.as_mut() else { return };
            if o.pose.component.is_empty() {
                return;
            }
            let mut d = mimic_draw_of(MimicPose { rig: &o.rig, out: (&o.out_before, &o.out), pose: (&o.pose_before, &o.pose), chain: (&o.chain_before, &o.chain), weapon: o.weapon }, alpha, control, &mut o.tall, super::phase::form(o.phase));
            // (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) HIS own wear form and drives (owner = member + 1): he changes form only when he does, never with the player
            (d.wear_form, d.owner) = (Some(o.core.combat().form.wear_form), Some(MEMBER + 1));
            list.push(d);
            if !o.told_draw && o.steps > 60 {
                o.told_draw = true;
                erlog!("mimic: Mimic Sora's drawing a second after the summon: {}", kh_er_render::mimic::mimic_status());
            }
        });
        if !list.is_empty() {
            kh_er_render::mimic::submit_mimic(list);
        }
    }

    /// (MIMIC-BOSS) A Sora copy's latest two steps as the drawing takes them.
    pub(crate) struct MimicPose<'a> {
        pub(crate) rig: &'a Rig,
        pub(crate) out: (&'a Output, &'a Output),
        pub(crate) pose: (&'a kh_anim::Pose, &'a kh_anim::Pose),
        pub(crate) chain: (&'a [Trs], &'a [Trs]),
        pub(crate) weapon: (bool, f32),
    }

    /// (MIMIC-BOSS) A Sora copy as the renderer's Mimic draw, between his two latest steps (`alpha`); `tall`: his standing
    /// height (kept from his first draw); `form`: (formed height 0..1, the pool's radius cm).
    pub(crate) fn mimic_draw_of(m: MimicPose<'_>, alpha: f32, control: &Control, tall: &mut Option<f32>, form: (f32, f32)) -> kh_er_render::mimic::MimicDraw {
        let (pose, mut to_world, chain) = crate::interp::sora_drawn(m.out.0, m.out.1, m.pose.0, m.pose.1, m.chain.0, m.chain.1, alpha);
        to_world.translation += crate::coords::dir_er_to_kh(Vec3::new(control.f32("r.dx").unwrap_or(0.0), 0.0, control.f32("r.dz").unwrap_or(0.0)));
        let opacity = weapon_opacity(m.weapon);
        let weapon = m.rig.animator.skeleton().index_of("R_buki").and_then(|hand| pose.component.get(hand)).filter(|_| opacity > 0.0).map(|hand| kh_er_render::Weapon { to_world: weapon_socket().then(hand).then(&to_world), component: chain, opacity });
        // (THE USER 2026-11-03) his arrival / leaving: the share of him formed out of the silver, the pool at his feet. The cut
        // runs from his soles (the lowest bone drawn) up his standing height (his first pose's, the hair's tips included)
        let zs: Vec<f32> = pose.component.iter().map(|t| t.then(&to_world).translation.z).filter(|z| z.is_finite()).collect();
        let (low, high) = zs.iter().fold((f32::MAX, f32::MIN), |(a, b), z| (a.min(*z), b.max(*z)));
        let tall = *tall.get_or_insert(((high - low) + TOP_CM).clamp(60.0, 400.0));
        let (height, pool) = form;
        let form = kh_er_render::mimic::MimicForm { height, feet_z: if low.is_finite() && low < f32::MAX { low } else { to_world.translation.z }, tall, pool };
        // (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) his wear form and owner are set by his caller (each copy's own)
        kh_er_render::mimic::MimicDraw { body: kh_er_render::Body { component_to_world: to_world, local: pose.local, component: pose.component }, weapon, form, wear_form: None, owner: None }
    }

    // ------------------------------------------------------------------------------------------------ his seams

    // (MIMIC-FX) his effects and sounds: fx/copyfx.rs (`SoraCopyFx`, `SoraCopyAudio`, `sora_copy_step`), the shared route of
    // every copy: on HIS bones, Keyblade and form, from HIS place

    /// His leg IK against the game's world without Sora's own floor clamp (party/mod.rs `PartyIk`'s rule).
    pub(crate) struct Ik<'a>(pub(crate) &'a ErWorld);

    impl kh_anim::IkWorld for Ik<'_> {
        fn sweep_sphere(&self, start: Vec3, end: Vec3, radius: f32) -> Option<kh_anim::SweepHit> {
            let hit = crate::sweep::sweep_first(self.0, start, end, radius, radius, &self.0.opts.get())?;
            Some(kh_anim::SweepHit { time: hit.time, impact_point: hit.position, impact_normal: hit.impact_normal, start_penetrating: hit.start_penetrating })
        }
    }

    /// His targets: Elden Ring's characters (ErTargets, what every party member hits); his hits x the Mimic Tear's damage
    /// rate and marked as his (his body fires them); his battle flag measured round HIM; no team attack (no friends).
    struct MimicTargets<'a> {
        inner: &'a mut ErTargets,
        damage: f32,
        at: Vec3,
    }

    impl Targets for MimicTargets<'_> {
        fn enumerate(&self, out: &mut Vec<TargetInfo>) {
            self.inner.enumerate(out)
        }
        fn info(&self, id: TargetId) -> Option<TargetInfo> {
            self.inner.info(id)
        }
        fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
            self.inner.hurt_shapes(id, out)
        }
        fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
            let mut h = *hit;
            h.damage *= self.damage.max(0.0);
            h.flags |= kh_core::host::HIT_FLAG_AI; // (PARTY-AI) Mimic Sora is an AI: he passes through friendly NPCs
            FIRING.store(true, Ordering::Relaxed);
            let r = self.inner.apply_hit(&h);
            FIRING.store(false, Ordering::Relaxed);
            r
        }
        fn defense(&self, id: TargetId) -> Option<TargetDefense> {
            self.inner.defense(id)
        }
        fn allies(&self, out: &mut Vec<Ally>) {
            self.inner.allies(out)
        }
        fn heal_ally(&mut self, id: TargetId, amount: i32, effect: u8) {
            self.inner.heal_ally(id, amount, effect)
        }
        fn lock_on_range(&self, id: TargetId) -> f32 {
            self.inner.lock_on_range(id)
        }
        fn engaged_count(&self) -> u32 {
            mimic_engaged(self.inner, self.at)
        }
        fn body(&self, id: TargetId, part: u32) -> TargetBody {
            self.inner.body(id, part)
        }
        fn take_armor(&mut self, id: TargetId, power: i32, weak: bool) -> i32 {
            self.inner.take_armor(id, power, weak)
        }
        fn take_reaction_power(&mut self, id: TargetId, power: i32) -> i32 {
            self.inner.take_reaction_power(id, power)
        }
        fn rush_suppression(&self, id: TargetId) -> bool {
            self.inner.rush_suppression(id)
        }
        fn terrain_dz(&self, id: TargetId) -> f32 {
            self.inner.terrain_dz(id)
        }
        fn push_body(&self, id: TargetId) -> Option<PushBody> {
            self.inner.push_body(id)
        }
        fn reaction_state(&self, id: TargetId) -> u8 {
            self.inner.reaction_state(id)
        }
        fn drawn_extent(&self, id: TargetId) -> Option<(f32, f32)> {
            self.inner.drawn_extent(id)
        }
        fn attack_shapes(&self, out: &mut Vec<AttackShape>) {
            self.inner.attack_shapes(out)
        }
        fn touched(&mut self, hit: &HitRecord) {
            self.inner.touched(hit)
        }
        fn finish_kind(&self, id: TargetId) -> FinishKind {
            self.inner.finish_kind(id)
        }
        fn size_class(&self, id: TargetId) -> u8 {
            self.inner.size_class(id)
        }
        fn stagger_ask(&mut self, id: TargetId, ask: kh_core::host::StaggerAsk) -> bool {
            self.inner.stagger_ask(id, ask)
        }
        fn element_affinity(&self, id: TargetId) -> Option<[i32; 8]> {
            self.inner.element_affinity(id)
        }
        // (AUDIT-B) forwarded: the hit's bone and its pose are the wrapped host's
        fn hit_bone(&self, id: kh_core::TargetId, at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
            self.inner.hit_bone(id, at)
        }
        fn bone_pose(&self, id: kh_core::TargetId, bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
            self.inner.bone_pose(id, bone)
        }
    }

    /// ErTargets' engaged rule (range + hit / attack memory) measured round HIM (ErTargets measures round `targets.sora`, the
    /// leader): the hostiles up and about within the party's engage range of him, or fighting the party lately.
    fn mimic_engaged(t: &ErTargets, at: Vec3) -> u32 {
        t.list.iter().filter(|e| e.hostile && e.alive && e.active && (e.pos.distance(at) - e.radius <= kh_partyhost::lead::PARTY_AGGRO_RANGE || t.fought_lately(e.id))).count() as u32
    }
}

#[cfg(all(test, feature = "party"))]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn mimic_he_is_called_beside_sora_never_inside_him() {
        let at = Vec3::new(100.0, 200.0, 0.0);
        for yaw in [0.0_f32, 90.0, 180.0, -45.0] {
            let p = beside(at, yaw);
            assert!(((p - at).length() - 150.0).abs() < 1e-3, "{p}");
            let facing = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
            assert!((p - at).dot(facing).abs() < 1e-3, "to his side, not in front: {p}");
        }
    }

    #[test]
    fn mimic_nothing_is_out_before_a_summon() {
        assert_eq!(live(), None);
        assert!(want().is_none());
        assert!(!firing());
    }
}

/// (THE USER 2026-11-03) his arrival and his leaving: the order of the states, the form, the game's sounds.
#[cfg(all(test, feature = "party"))]
mod phase_tests {
    use super::phase::*;

    /// What a run saw: every (phase, form), the sounds, the phase at the end (None: gone).
    type Run = (Vec<(Phase, (f32, f32))>, Vec<(i32, i32)>, Option<Phase>);

    /// Steps a phase at 60 Hz until it ends (None) or `max` s; every (clock, form, sounds) seen.
    fn run(mut p: Phase, max: f32) -> Run {
        let dt = 1.0 / 60.0;
        let (mut seen, mut heard) = (Vec::new(), Vec::new());
        let mut t = 0.0;
        while t < max {
            let clock = match p {
                Phase::Arriving(c) | Phase::Leaving { t: c, .. } => c,
                Phase::Out => 0.0,
            };
            heard.extend(sounds(p, clock, clock + dt));
            seen.push((p, form(p)));
            match advance(p, dt) {
                Some(n) => p = n,
                None => return (seen, heard, None),
            }
            t += dt;
        }
        (seen, heard, Some(p))
    }

    #[test]
    fn mimic_the_arrival_pool_first_then_he_rises_then_the_ai() {
        let (seen, heard, end) = run(Phase::Arriving(0.0), 5.0);
        assert_eq!(end, Some(Phase::Out), "formed: his AI takes over");
        assert_eq!(heard, vec![SOUND_ARRIVE], "the game's 'a spirit appears' once, at the start");
        let at = |s: f32| seen.iter().find(|(p, _)| matches!(p, Phase::Arriving(t) if *t >= s)).map(|x| x.1).unwrap();
        assert_eq!(at(0.0), (0.0, 0.0), "nothing at the summon's frame");
        assert!(at(0.5).0 == 0.0 && at(0.5).1 > 30.0, "the pool first, no figure: {:?}", at(0.5));
        let mid = at(1.9);
        assert!(mid.0 > 0.3 && mid.0 < 0.7 && (mid.1 - POOL_CM).abs() < 0.5, "rising out of the full pool: {mid:?}");
        assert!(at(3.2).0 >= 0.999 && at(3.2).1 < POOL_CM, "whole, the pool draining: {:?}", at(3.2));
        assert!(seen.windows(2).all(|w| w[1].1 .0 >= w[0].1 .0 - 1e-6), "he never shrinks while he forms");
        assert_eq!(form(Phase::Out), (1.0, 0.0));
        assert!(still(Phase::Arriving(1.0)) && !still(Phase::Out) && still(Phase::Leaving { t: 0.0, death: false }));
    }

    #[test]
    fn mimic_a_dismissal_melts_at_once_a_death_knocks_down_first() {
        let (seen, heard, end) = run(Phase::Leaving { t: 0.0, death: false }, 6.0);
        assert_eq!(end, None, "gone");
        assert_eq!(heard, vec![SOUND_DISMISS]);
        let last = seen.last().unwrap().0;
        assert!(matches!(last, Phase::Leaving { t, .. } if t < MELT_END + POOL_OUT + 0.05), "{last:?}");
        assert!(seen.iter().any(|(_, f)| f.0 > 0.2 && f.0 < 0.8 && f.1 > 30.0), "melting into the pool");
        assert!(seen.windows(2).all(|w| w[1].1 .0 <= w[0].1 .0 + 1e-6), "he never grows while he melts");
        // his death: KH3's knockdown held whole (no pool), then the game's spirit fade
        let (seen, heard, end) = run(Phase::Leaving { t: 0.0, death: true }, 8.0);
        assert_eq!(end, None);
        assert_eq!(heard, vec![SOUND_DEATH, SOUND_DEATH_FADE], "the death sound at once, the fade's when it begins");
        assert!(seen.iter().filter(|(p, _)| matches!(p, Phase::Leaving { t, .. } if *t < KO_HOLD)).all(|(_, f)| *f == (1.0, 0.0)), "the knockdown first, whole");
        let gone_at = match seen.last().unwrap().0 { Phase::Leaving { t, .. } => t, _ => 0.0 };
        assert!(gone_at > KO_HOLD + DEATH_FADE && gone_at < KO_HOLD + MELT_START + DEATH_FADE + POOL_OUT + 0.05, "{gone_at}");
    }
}

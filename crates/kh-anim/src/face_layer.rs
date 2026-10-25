//! (FACE) THE HOST FACE LAYER: what a host plays on a character's two face slots (Face = G_eye, Mouth = G_mouth) beyond
//! the face plays of the body clips' own notifies (`TresAnimNotify_FaceAnim*`, pose.rs; KH3 pairs every body clip with
//! its face that way). General: Donald and Goofy use it through kh-partyhost's poser; Sora's host can use it the same way.
//!
//! 1. LIP-SYNC OF A VOICE LINE (`FaceInput::voice`): a line the body clip saying it has KH3's own lips for (a voice notify
//!    with `MOUTH_OPEN` / lip plays around it: [`Animator::voiced_lips`]) is left to the clip, as in KH3. Any other line
//!    (a voice group the code plays, a battle shout over another clip) gets lips here: KH3's own mouth poses
//!    (MOUTH_OPEN_01 .. 05, closed = the NORMAL_AUTO lip) on the line's loudness ([`crate::lip`]); without the loudness,
//!    KH3's field-chatter lip row `TA_DE_M_03` (TresFieldVoice m_LipAnimName of Donald's / Goofy's 429 chat lines) for
//!    the line's length. The Mouth slot is held meanwhile (the body's mouth plays wait; KH3's play priority).
//! 2. A TALK LINE (`FaceInput::talk`, KH3's TresFieldVoice, the field-voice task FUN_1414f4510 / FUN_1414f6ed0): the
//!    line's eye row (TA_<mood>_E_00) and lip row (TA_<mood>_M_01) at its start, blend 0.5 (FUN_141274bb0 /
//!    FUN_141274ff0), the NPC BLINK TIMER on the eye row (TresNpcPawnBase m_FaceEyeBlinkMinTimer 3.0 / MaxTimer 4.0, ctor
//!    0x141254FF0; FUN_14127c3d0: at the timer, row "<prefix>_%02d" of the eye row's number + 1 = TA_<mood>_E_01, blend
//!    0.2, then the eye row again, blend 0.2, a new timer), and at its end face type 0 (NORMAL_AUTO) on both slots, blend
//!    0.5 (FUN_1414f6ed0 vt+0x430). Both slots are held meanwhile.
//! 3. A HOST CLIP over the face (kh-party's face-clip slot, Zettaflare's lip track): the clip owns the face; no lips and
//!    no talk here while it plays.
//!
//! BLINKS OUTSIDE TALK are KH3's own: baked into the looping face clips the body notifies play (the eye-look grids
//! NO_0_Eye / BA_00_Eye blink at 1.73 s of their 3 s loop, 0000EM, 0100EM, 0118EM ...).
//!
//! Frame-rate independent: the lip keys are on the line's own clock, the blink timer and the talk are in seconds.

use std::sync::Arc;

use crate::lip::{lip_track, LipKey, OPEN};
use crate::pose::Animator;

const EYE: usize = Animator::FACE_EYE;
const MOUTH: usize = Animator::FACE_MOUTH;

/// KH3's field-chatter lip row (TresFieldVoice m_LipAnimName of the chat lines): a talking loop, for a line without a
/// loudness.
pub const CHATTER_LIP: &str = "TA_DE_M_03";
/// TresNpcPawnBase m_FaceEyeBlinkMinTimer / m_FaceEyeBlinkMaxTimer (ctor 0x141254FF0: 3.0 / 4.0; no friend's blueprint
/// overrides them).
pub const BLINK_MIN: f32 = 3.0;
pub const BLINK_MAX: f32 = 4.0;
/// The blink's play and the eye row's play after it (FUN_14127c3d0: 0x3e4ccccd).
pub const BLINK_BLEND: f32 = 0.2;
/// A talk line's eye and lip plays (FUN_1414f4510 -> FUN_141274bb0 / FUN_141274ff0: 0x3f000000).
pub const TALK_BLEND: f32 = 0.5;
/// A talk line's end: face type 0 on both slots (FUN_1414f6ed0 vt+0x430: 0x3f000000).
pub const TALK_END_BLEND: f32 = 0.5;
/// The close of a lip track (the authored lips' nameless FaceAnimLip after a MOUTH_OPEN: 0.1, n_ex001 M_1301).
pub const LIP_CLOSE_BLEND: f32 = 0.1;
/// An open of a lip track (the authored MOUTH_OPEN plays' m_BlendInTime: 0.1 in most, 0.05 - 0.2; n_ex001 M_1301, M_1302,
/// MagicFire, MagicCast).
pub const LIP_OPEN_BLEND: f32 = 0.1;
/// A voice line said by the body clip has its own lips when they are in this window of clip time around the line's
/// start (s before, s after).
pub const AUTHORED_WINDOW: (f32, f32) = (0.2, 0.6);

/// A voice line that sounds now (kh_audio::VoiceLine as the host hands it on).
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceInput {
    /// The line's identity (a new id = a new line).
    pub id: u64,
    /// Seconds since it started sounding.
    pub elapsed: f32,
    /// Its length (s).
    pub length: f32,
    /// Its loudness (0..255 of its peak) and the samples per second; None = not in the pack's table.
    pub loudness: Option<(Arc<[u8]>, f32)>,
}

/// A talk line (KH3's TresFieldVoice face) as kh-party's face slot carries it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TalkInput {
    /// A new serial = a new line.
    pub serial: u32,
    /// The eye row (TA_DE_E_00), its blink row (TA_DE_E_01), the speaking lip row (TA_DE_M_01).
    pub eye: &'static str,
    pub blink: &'static str,
    pub lip: &'static str,
    /// Seconds into the line, and its length.
    pub time: f32,
    pub length: f32,
}

/// What the layer reads in a step.
#[derive(Clone, Copy, Debug, Default)]
pub struct FaceInput<'a> {
    pub voice: Option<&'a VoiceInput>,
    pub talk: Option<TalkInput>,
    /// A host clip owns the face this step (kh-party's face-clip slot).
    pub clip: bool,
    /// The body slot's sequences this step and its clip time (the authored-lips test).
    pub body: &'a [&'a str],
    pub body_time: f32,
    /// The host feeds the member's voice lines (`voice` is the truth about what sounds): THE VOICE GATE is on. A body clip
    /// whose own voice line did not sound (a clip the port plays as a pose without its line: Aero Float's MagicAirCast)
    /// keeps its beak shut after the line's time instead of moving its lips to a silence.
    pub gate: bool,
}

/// The voice gate's wait after a clip's voice notify: the audio side reports a line one step late.
pub const GATE_DELAY: f32 = 0.05;

/// The layer's state, one per character.
#[derive(Clone, Debug, Default)]
pub struct FaceLayer {
    /// The voice line in hand (its id), whether the clip has its own lips for it, its track and the next key.
    line: Option<u64>,
    authored: bool,
    track: Vec<LipKey>,
    next: usize,
    chatter: bool,
    mouth_held: bool,
    /// The talk line in hand (its serial), its blink timer and the blink's time left.
    talk: Option<TalkInput>,
    blink_timer: f32,
    blink_left: f32,
    rng: u32,
    /// The voice gate: the body clip and clip time when a voice line last sounded; the gate holds the mouth now.
    voice_seen: Option<(String, f32)>,
    gated: bool,
    /// What the layer did last (debug text of the hosts).
    pub said: String,
    /// Blinks played by the talk's timer (for tests and traces).
    pub blinks: u32,
}

impl FaceLayer {
    fn rand(&mut self) -> f32 {
        // xorshift32, seeded by the talk line
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn blink_time(&mut self) -> f32 {
        BLINK_MIN + (BLINK_MAX - BLINK_MIN) * self.rand()
    }

    /// The layer's lip-sync state: (line id, authored by the clip, keys, chatter loop, mouth held).
    pub fn lip_state(&self) -> (Option<u64>, bool, &[LipKey], bool, bool) {
        (self.line, self.authored, &self.track, self.chatter, self.mouth_held)
    }

    /// Whether a talk line holds the face now.
    pub fn talking(&self) -> bool {
        self.talk.is_some()
    }

    /// One step, BEFORE the animator's step (its plays then show from this step on, as a notify's).
    pub fn step(&mut self, a: &mut Animator, input: &FaceInput, dt: f32) {
        // a host clip takes the face: the layer lets go without playing anything (the clip holds both slots and gives
        // them back itself)
        if input.clip {
            self.gated = false;
            if self.talk.take().is_some() || self.line.take().is_some() {
                self.track.clear();
                self.chatter = false;
                self.authored = false;
                self.mouth_held = false;
                self.said = "a host clip has the face".to_string();
            }
            return;
        }
        let talk = input.talk.filter(|t| t.time < t.length);
        // ---- the talk line
        match (self.talk, talk) {
            (Some(had), Some(t)) if had.serial == t.serial => self.talk_tick(a, dt),
            (had, Some(t)) => {
                if had.is_some() {
                    self.talk_end(a);
                }
                self.lip_end(a);
                self.talk_begin(a, t);
            }
            (Some(_), None) => self.talk_end(a),
            (None, None) => {}
        }
        // ---- the voice gate (before the line's lips: a line that starts now lets go of it)
        self.gate(a, input);
        // ---- the voice line's lips (a talk line or a host clip owns the mouth)
        let voice = input.voice.filter(|_| self.talk.is_none());
        match (self.line, voice) {
            (Some(id), Some(v)) if id == v.id => self.lip_tick(a, v),
            (_, Some(v)) => {
                self.lip_end(a);
                self.lip_begin(a, v, input);
            }
            (Some(_), None) => self.lip_end(a),
            (None, None) => {}
        }
    }

    /// THE VOICE GATE (see [`FaceInput::gate`]): from GATE_DELAY after a voice notify of the body clip to its last lip play
    /// (+0.25 s), with no voice line heard since that notify, the mouth is held shut (NORMAL_AUTO lip, the close blend);
    /// the body's lip plays wait and the latest plays when the gate opens.
    fn gate(&mut self, a: &mut Animator, input: &FaceInput) {
        let seq = input.body.first().copied().unwrap_or("");
        let t = input.body_time;
        if input.voice.is_some() {
            self.voice_seen = Some((seq.to_string(), t));
        }
        let mute = input.gate && self.talk.is_none() && input.voice.is_none() && {
            let (triggers, last_lip) = a.voice_triggers(seq);
            let heard = |tv: f32| self.voice_seen.as_ref().is_some_and(|(s, at)| s == seq && *at >= tv - 0.1 && *at <= t + 1e-3);
            triggers.iter().any(|&tv| t >= tv + GATE_DELAY && t <= last_lip.max(tv) + 0.25 && !heard(tv))
        };
        if mute && !self.gated {
            a.hold_face(MOUTH);
            a.play_face_row(None, "NORMAL_AUTO", [false, true], LIP_CLOSE_BLEND, false);
            self.gated = true;
            self.said = format!("{seq}: its line did not sound, its lips wait");
        } else if !mute && self.gated {
            a.release_face(MOUTH, LIP_CLOSE_BLEND);
            self.gated = false;
        }
    }

    /// Whether the voice gate holds the mouth now.
    pub fn gated(&self) -> bool {
        self.gated
    }

    fn talk_begin(&mut self, a: &mut Animator, t: TalkInput) {
        a.hold_face(EYE);
        a.hold_face(MOUTH);
        self.gated = false;
        let eye = a.play_face_row(Some(t.eye), "", [true, false], TALK_BLEND, false);
        let lip = a.play_face_row(Some(t.lip), "", [false, true], TALK_BLEND, false);
        self.rng = t.serial.wrapping_mul(2_654_435_761) ^ 0x9E37_79B9;
        self.blink_timer = self.blink_time();
        self.blink_left = 0.0;
        self.talk = Some(t);
        self.said = format!("talk {} (eye {}, lip {})", t.serial, if eye { t.eye } else { "-" }, if lip { t.lip } else { "-" });
    }

    fn talk_tick(&mut self, a: &mut Animator, dt: f32) {
        let Some(t) = self.talk else { return };
        if self.blink_left > 0.0 {
            self.blink_left -= dt;
            if self.blink_left <= 0.0 {
                // FUN_14127c3d0: the blink montage stopped -> the eye row again, blend 0.2
                a.play_face_row(Some(t.eye), "", [true, false], BLINK_BLEND, false);
            }
            return;
        }
        self.blink_timer -= dt;
        if self.blink_timer <= 0.0 {
            self.blink_timer = self.blink_time();
            if a.has_face_row(Some(t.blink), "", [true, false]) {
                a.play_face_row(Some(t.blink), "", [true, false], BLINK_BLEND, false);
                self.blink_left = a.face_row_length(t.blink, EYE).unwrap_or(0.2).max(dt);
                self.blinks += 1;
            }
        }
    }

    fn talk_end(&mut self, a: &mut Animator) {
        if self.talk.take().is_none() {
            return;
        }
        // FUN_1414f6ed0: face type 0 (NORMAL_AUTO) on both slots, blend 0.5; then the body's own face plays again
        a.play_face_row(None, "NORMAL_AUTO", [true, true], TALK_END_BLEND, false);
        a.release_face(EYE, TALK_END_BLEND);
        a.release_face(MOUTH, TALK_END_BLEND);
        self.blink_left = 0.0;
        self.said = "talk end".to_string();
    }

    fn lip_begin(&mut self, a: &mut Animator, v: &VoiceInput, input: &FaceInput) {
        self.line = Some(v.id);
        self.track.clear();
        self.next = 0;
        self.chatter = false;
        let t = input.body_time;
        self.authored = input.body.iter().any(|s| a.voiced_lips(s, t - AUTHORED_WINDOW.0, t + AUTHORED_WINDOW.1));
        if self.authored {
            self.said = format!("line {}: the clip's own lips", v.id);
            return;
        }
        if let Some((loud, rate)) = &v.loudness {
            self.track = lip_track(loud, *rate);
        }
        if self.track.is_empty() {
            if v.loudness.is_none() && a.has_face_row(Some(CHATTER_LIP), "", [false, true]) {
                a.hold_face(MOUTH);
                self.mouth_held = true;
                a.play_face_row(Some(CHATTER_LIP), "", [false, true], LIP_CLOSE_BLEND, false);
                self.chatter = true;
                self.said = format!("line {}: {CHATTER_LIP} for {:.2} s", v.id, v.length);
            } else {
                self.said = format!("line {}: silent", v.id);
            }
            return;
        }
        a.hold_face(MOUTH);
        self.mouth_held = true;
        self.said = format!("line {}: {} lip keys", v.id, self.track.len());
        self.lip_tick(a, v);
    }

    fn lip_tick(&mut self, a: &mut Animator, v: &VoiceInput) {
        if self.authored {
            return;
        }
        if self.chatter {
            if v.elapsed >= v.length && self.mouth_held {
                self.close(a);
            }
            return;
        }
        // the keys passed by now; only the last one shows (a step that passes two plays the later)
        let mut last = None;
        while self.next < self.track.len() && self.track[self.next].time <= v.elapsed {
            last = Some(self.track[self.next]);
            self.next += 1;
        }
        if let Some(k) = last {
            self.apply(a, k);
        }
    }

    fn apply(&mut self, a: &mut Animator, k: LipKey) {
        match k.open {
            Some(i) => {
                // the widest type the pack has at or under this one (Donald has no NO_53: MOUTH_OPEN_04 -> 03)
                let mut i = i.min(4) as usize;
                while i > 0 && !a.has_face_row(None, OPEN[i], [false, true]) {
                    i -= 1;
                }
                a.play_face_row(None, OPEN[i], [false, true], LIP_OPEN_BLEND, false);
            }
            None => {
                a.play_face_row(None, "NORMAL_AUTO", [false, true], LIP_CLOSE_BLEND, false);
            }
        }
    }

    fn close(&mut self, a: &mut Animator) {
        a.play_face_row(None, "NORMAL_AUTO", [false, true], LIP_CLOSE_BLEND, false);
        a.release_face(MOUTH, LIP_CLOSE_BLEND);
        self.mouth_held = false;
    }

    fn lip_end(&mut self, a: &mut Animator) {
        if self.line.take().is_none() {
            return;
        }
        if self.mouth_held {
            self.close(a);
        }
        self.track.clear();
        self.chatter = false;
        self.authored = false;
    }
}

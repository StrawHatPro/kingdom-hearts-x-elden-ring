//! (ZETTA) THE MEMBER'S FACE-CLIP SLOT: any face clip (a `face/<sequence>.glb` of the member's pack: the Face / Mouth
//! tracks, kind `face-animation`) played on a party member's face, blended over the body pose, then the face the body's own
//! notifies gave it put back. Engine-free: the pawn holds the slot ([`NpcPawn::face`](crate::pawn::NpcPawn)), the step
//! ages it ([`party_step`], from `Party::step`), the view carries it ([`AnimView::face`](crate::view::AnimView)), both
//! hosts' posers draw it (kh-partyhost `PosedMember::set_pose_view`: kh-anim `Animator::play_face_clip` on BOTH face slots,
//! `face_save` / `face_restore` around it).
//!
//! Made for Zettaflare's "Zettaflare!" lip track (donald/zettaflare.rs); kept general for the FACE stream (face clips paired
//! with body clips, lip-sync for every voice, blinks):
//! - [`FaceSlot::play`] starts a clip (a new play of the same clip restarts it: every play has its own serial);
//! - [`FaceSlot::stop`] ends it (the host blends back to the face it had, over `blend_out`);
//! - a clip with a `length` and `looping == false` stops by itself at its end (the drawn face never loops a lip line);
//! - [`FaceSlot::view`] is what the host draws (time = seconds since the play, frame-rate independent).

/// One face clip on the slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceClip {
    /// The face clip's sequence name (the pack file `face/<sequence>.glb`, a `face-animation` item of the member's
    /// manifest).
    pub sequence: &'static str,
    /// Seconds since the play (the host's face montage runs its own clock from 0 at the play; this is the core's).
    pub time: f32,
    /// Blend-in over the face the member had (s).
    pub blend_in: f32,
    /// Its length (s); 0 = unknown (it then plays until [`FaceSlot::stop`]).
    pub length: f32,
    pub looping: bool,
    /// Every play's number (a host restarts the clip when it changes).
    pub serial: u32,
}

/// The slot (one per member).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct FaceSlot {
    pub clip: Option<FaceClip>,
    /// Blend back to the face it had after the clip (s; the last stop's).
    pub blend_out: f32,
    serial: u32,
    /// (FACE) A talk line's face (KH3's TresFieldVoice face: eye row + blink, talking lip row), see [`FaceSlot::talk`].
    pub talk: Option<TalkFace>,
}

/// What the host draws (`AnimView::face`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct FaceView {
    /// The clip on the face (None: the face the body's notifies give it).
    pub clip: Option<FaceClip>,
    /// Blend back when the clip goes (s).
    pub blend_out: f32,
    /// (FACE) The talk line's face (kh_anim::face_layer plays it: rows, the NPC blink timer, the end).
    pub talk: Option<TalkFace>,
}

/// (FACE) THE MOOD OF A TALK LINE: KH3's field voices pick their face rows by it (TresFieldVoice m_EyeAnimName /
/// m_LipAnimName; `ETresFieldVoiceLip` Default / Joy / Angly / Sorrow; the friends' anim sets add Surprise). Donald's and
/// Goofy's 77 talk lines use DE (Default) and SM (Joy) with TalkAct1-3 / TalkLoopN, SA (Sorrow) once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mood {
    #[default]
    Default,
    Joy,
    Angry,
    Sorrow,
    Surprise,
}

impl Mood {
    /// The rows of the friends' face anim set (n_ex00x_FaceAnimSet TA_*): (eye, its blink, the speaking lip).
    pub fn rows(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Mood::Default => ("TA_DE_E_00", "TA_DE_E_01", "TA_DE_M_01"),
            Mood::Joy => ("TA_SM_E_00", "TA_SM_E_01", "TA_SM_M_01"),
            Mood::Angry => ("TA_AN_E_00", "TA_AN_E_01", "TA_AN_M_01"),
            Mood::Sorrow => ("TA_SA_E_00", "TA_SA_E_01", "TA_SA_M_01"),
            Mood::Surprise => ("TA_SU_E_00", "TA_SU_E_01", "TA_DE_M_01"),
        }
    }

    pub fn from_name(name: &str) -> Option<Mood> {
        Some(match name.to_ascii_lowercase().as_str() {
            "default" | "de" | "normal" => Mood::Default,
            "joy" | "sm" | "smile" | "happy" => Mood::Joy,
            "angry" | "an" | "angly" => Mood::Angry,
            "sorrow" | "sa" | "sad" => Mood::Sorrow,
            "surprise" | "su" => Mood::Surprise,
            _ => return None,
        })
    }
}

/// (FACE) A talk line on the face: its rows, its clock and length, its serial (a new line restarts the rows).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TalkFace {
    pub mood: Mood,
    pub eye: &'static str,
    pub blink: &'static str,
    pub lip: &'static str,
    pub time: f32,
    pub length: f32,
    pub serial: u32,
}

/// (FACE) How long a line WITHOUT a voice is spoken (OUR DESIGN: the text's length at 14 characters a second, 1 - 6 s;
/// a voiced line's length is its voice's).
pub fn speech_length(text: &str) -> f32 {
    (text.chars().filter(|c| !c.is_whitespace()).count() as f32 / 14.0).clamp(1.0, 6.0)
}

/// The blend back when a clip ends by itself (s).
pub const DEFAULT_BLEND_OUT: f32 = 0.15;

impl FaceSlot {
    /// Plays `sequence` from its start (`length` 0 = unknown).
    pub fn play(&mut self, sequence: &'static str, blend_in: f32, length: f32, looping: bool) {
        self.serial = self.serial.wrapping_add(1);
        self.clip = Some(FaceClip { sequence, time: 0.0, blend_in: blend_in.max(0.0), length: length.max(0.0), looping, serial: self.serial });
        self.blend_out = DEFAULT_BLEND_OUT;
    }

    /// Ends the clip now (if `sequence` is given, only when that clip is the one playing): the host blends back.
    pub fn stop(&mut self, sequence: Option<&str>, blend_out: f32) {
        if self.clip.is_some_and(|c| sequence.map_or(true, |s| s == c.sequence)) {
            self.clip = None;
            self.blend_out = blend_out.max(0.0);
        }
    }

    /// (FACE) A talk line of `length` seconds in `mood` (its rows; a new call is a new line).
    pub fn talk(&mut self, mood: Mood, length: f32) {
        self.serial = self.serial.wrapping_add(1);
        let (eye, blink, lip) = mood.rows();
        self.talk = Some(TalkFace { mood, eye, blink, lip, time: 0.0, length: length.max(0.0), serial: self.serial });
    }

    /// (FACE) Ends the talk line now (the host's face layer puts the face back as KH3's field voice does).
    pub fn talk_end(&mut self) {
        self.talk = None;
    }

    /// The step: its clock; a one-shot with a length ends at it.
    pub fn step(&mut self, dt: f32) {
        // (FACE) the talk line's clock; it ends at its length
        if let Some(t) = self.talk.as_mut() {
            t.time += dt.max(0.0);
            if t.time >= t.length {
                self.talk = None;
            }
        }
        let Some(c) = self.clip.as_mut() else { return };
        c.time += dt.max(0.0);
        if c.length > 0.0 {
            if c.looping {
                c.time %= c.length;
            } else if c.time >= c.length {
                self.clip = None;
                self.blend_out = DEFAULT_BLEND_OUT;
            }
        }
    }

    /// The sequence playing, if any.
    pub fn playing(&self) -> Option<&'static str> {
        self.clip.map(|c| c.sequence)
    }

    pub fn view(&self) -> FaceView {
        FaceView { clip: self.clip, blend_out: self.blend_out, talk: self.talk /* (FACE) */ }
    }
}

/// (FACE) Member `id` says a talk line of `length` s in `mood` (the hosts' conversation lines: the recruit talk, the free
/// talk; the debug word `party don-talk`). False when the member is not in the party.
pub fn talk(party: &mut crate::party::Party, id: crate::ids::MemberId, mood: Mood, length: f32) -> bool {
    match party.member_mut(id) {
        Some(p) => {
            p.face.talk(mood, length);
            true
        }
        None => false,
    }
}

/// The party step's part (party.rs, marked): every member's slot ages by `dt`.
pub fn party_step(party: &mut crate::party::Party, dt: f32) {
    let ids: Vec<crate::ids::MemberId> = party.members().collect();
    for id in ids {
        if let Some(p) = party.member_mut(id) {
            p.face.step(dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_one_shot_ends_by_itself_and_a_new_play_restarts() {
        let mut s = FaceSlot::default();
        assert_eq!(s.view().clip, None);
        s.play("n_ex001_70291w_face", 0.1, 3.1, false);
        let first = s.clip.unwrap().serial;
        for _ in 0..60 {
            s.step(1.0 / 60.0);
        }
        assert!((s.clip.unwrap().time - 1.0).abs() < 1e-3);
        s.play("n_ex001_70291w_face", 0.1, 3.1, false);
        assert_ne!(s.clip.unwrap().serial, first, "a new play is a new serial");
        assert_eq!(s.clip.unwrap().time, 0.0);
        for _ in 0..200 {
            s.step(1.0 / 60.0);
        }
        assert_eq!(s.playing(), None, "ended at its length");
        assert_eq!(s.view().blend_out, DEFAULT_BLEND_OUT);
    }

    #[test]
    fn stop_by_name_and_loops() {
        let mut s = FaceSlot::default();
        s.play("a", 0.0, 1.0, true);
        for _ in 0..90 {
            s.step(1.0 / 60.0);
        }
        assert!((s.clip.unwrap().time - 0.5).abs() < 1e-3, "looped");
        s.stop(Some("b"), 0.3);
        assert_eq!(s.playing(), Some("a"), "another clip's stop leaves it");
        s.stop(Some("a"), 0.3);
        assert_eq!(s.playing(), None);
        assert_eq!(s.blend_out, 0.3);
    }

    // (FACE) the talk line's face
    #[test]
    fn face_a_talk_line_carries_its_mood_rows_and_ends_at_its_length_at_any_step() {
        for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0] {
            let mut s = FaceSlot::default();
            s.talk(Mood::Joy, 1.5);
            let t = s.view().talk.unwrap();
            assert_eq!((t.eye, t.blink, t.lip), ("TA_SM_E_00", "TA_SM_E_01", "TA_SM_M_01"));
            let mut n = 0;
            while s.talk.is_some() {
                s.step(dt);
                n += 1;
            }
            assert!(((n as f32) * dt - 1.5).abs() <= dt + 1e-4, "ends at 1.5 s at dt {dt}: {} s", n as f32 * dt);
        }
        let mut s = FaceSlot::default();
        s.talk(Mood::Default, 2.0);
        let a = s.talk.unwrap().serial;
        s.talk(Mood::Angry, 2.0);
        assert_ne!(s.talk.unwrap().serial, a, "a new line restarts the face");
        assert_eq!(s.talk.unwrap().eye, "TA_AN_E_00");
        s.talk_end();
        assert_eq!(s.view().talk, None);
    }

    #[test]
    fn face_moods_and_speech_lengths() {
        assert_eq!(Mood::from_name("angly"), Some(Mood::Angry));
        assert_eq!(Mood::from_name("SM"), Some(Mood::Joy));
        assert_eq!(Mood::from_name("x"), None);
        assert_eq!(speech_length("Hi!"), 1.0);
        assert!((speech_length("Aw, phooey! That old crone's a fake!") - 30.0 / 14.0).abs() < 1e-3);
        assert_eq!(speech_length(&"a".repeat(500)), 6.0);
    }
}

//! The animation side of the simulation: the pawn's one `FullBody` montage slot as the game
//! updates it once per frame INSIDE the movement tick (movement spec 1.2 step d; animation spec
//! Part II sections 5, 8, 9; change list items A2, A3, A6-A10, A13, A15): blend-in weight, clock,
//! notifies, the blend space's input filter, root motion. No poses are evaluated here: the host
//! (crate `kh-anim`) draws from [`AnimRequest`]; the core only needs the clock, the notify times
//! and the root tracks, which `tools/gen_port.js` generates from the pack
//! (`build/gen/rust/sora_anim.rs`).

include!(env!("KH_SORA_ANIM_RS"));

/// CONFLICT BETWEEN THE TWO SPECIFICATIONS (reported; runtime request R3 settles it).
///
/// - Animation spec 8 / change list A3 (`FAnimMontageInstance::Advance` 0x143F33C00, read at
///   instruction level at 0x143F33CE6): on the first update after a play call the position does
///   not move (dt is forced to 0 for the clock; the weight still advances). Update 1 is the t = 0
///   frame; `TresJumpStart` (0.0666 s) fires on update 5 at 60 Hz.
/// - Movement spec rev 2, 4.1 / change list A5: "the clock first advances in M(1) and passes
///   0.0666 in M(4); DoJump in M(5)": 5 frames from the press to the launch. It notes itself that
///   this is unverified ("unless the play call ticks the pose itself").
///
/// With the play call made by the state at the END of frame 0, the first rule puts update 1 in
/// M(1), the notify in M(5) and the launch in M(6); the second puts the launch in M(5). The
/// coordinator's rule is "the movement revision 2 wins": `false` here. Set it to `true` for the
/// animation spec's reading; every test derives its expected frame numbers from this constant.
pub const FIRST_UPDATE_HOLDS_POSITION: bool = false;

/// Offset of the notify window after a normal play (instance +0x17C), seconds: 1e-4. It becomes
/// 0 after the instance's first loop wrap (animation spec 9).
const NOTIFY_OFFSET: f32 = 1.0e-4;

// `AnimId` is GENERATED (sora_anim.rs): one variant per row of `p_ex001_AnimSet` that the pack
// carries, by the game's own row names ("1301" is `AnimId::A1301`). The 14 animations of the
// stage-1 states keep the values 0..13 they had when the enum was written by hand (tests/golden.rs
// hashes them).
impl AnimId {
    /// The generated table row of this animation.
    pub fn data(self) -> &'static AnimData {
        &ANIMS[self as usize]
    }

    /// The game's name of the animation: the key of the pack manifest's `logical_animations`.
    pub fn name(self) -> &'static str {
        self.data().name
    }

    /// Every sample sequence of this animation (one for a plain sequence), in the asset's order.
    /// A blend space in mode `SQEX_AllAnimation` fires the notifies of all of them on every
    /// update, whatever their weight (animation spec 9).
    pub fn sequences(self) -> Vec<&'static str> {
        self.data().samples.iter().map(|&(_, index)| SEQUENCES[index].name).collect()
    }

    /// The sequences to draw for an axis value and their weights (they sum to 1). A plain
    /// sequence gives one entry of weight 1; a BlendSpace1D the two samples around `axis`,
    /// weighted linearly (the samples lie on the blend space's grid points, so the engine's
    /// grid interpolation is this). The sequence name is the pack file `anims/<name>.glb`.
    /// A 2D blend space: see [`AnimId::sample_weights_2d`] (this gives its two heaviest samples
    /// at `(axis, 0)`).
    pub fn sample_weights(self, axis: f32) -> [(&'static str, f32); 2] {
        self.sample_weights_2d(axis, 0.0)
    }

    /// As [`AnimId::sample_weights`] with the second axis of a 2D blend space (`UBlendSpace`:
    /// the asset's grid, the four grid points around the input mixed bilinearly). Only the two
    /// heaviest samples are returned, renormalised: the pose host blends two sequences.
    /// TODO(actions: rail slide, ball ride, cover slide): a pose blend of up to four samples.
    pub fn sample_weights_2d(self, axis: f32, axis_y: f32) -> [(&'static str, f32); 2] {
        let [(a, wa), (b, wb)] = sample_indices_2d(self.data(), axis, axis_y);
        [(SEQUENCES[a].name, wa), (SEQUENCES[b].name, wb)]
    }

    /// The notifies of the animation's first sequence (every notify of a plain sequence; for a
    /// blend space see `data().samples`).
    pub fn notifies(self) -> &'static [NotifyDef] {
        SEQUENCES[self.data().samples[0].1].notifies
    }
}

/// The arguments of a play call other than the stage-1 ones (animation spec 2.2): `mode` bit 0
/// = loop, bit 1 = root motion (extracted only when the sequences have `bEnableRootMotion`);
/// `blend_in` in seconds, negative = the asset's `m_InterpolationTime`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayArgs {
    pub looping: bool,
    pub root_motion: bool,
    pub blend_in: f32,
}

impl PlayArgs {
    /// From the game's mode bits (bit 0 loop, bit 1 root motion) and blend-in argument.
    pub const fn mode(mode: u8, blend_in: f32) -> PlayArgs {
        PlayArgs { looping: mode & 1 != 0, root_motion: mode & 0x12 == 2, blend_in }
    }
}

/// When a notify reaches its handlers (stock `UAnimInstance::TriggerAnimNotifies`, 0x143F2DC50;
/// animation spec 9): an instant notify fires once; a notify state begins on the first update
/// whose window lists it, ticks on every update that lists it (the first included) and ends on
/// the first update that does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotifyPhase {
    Instant,
    Begin,
    Tick,
    End,
}

/// One notify dispatched by a pose tick: which notify of which sequence, and its phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NotifyEvent {
    /// Index into `SEQUENCES`.
    pub sequence: u16,
    /// Index into that sequence's `notifies`.
    pub index: u16,
    pub phase: NotifyPhase,
}

impl NotifyEvent {
    /// The notify itself: class, times, decoded and raw properties.
    pub fn def(&self) -> &'static NotifyDef {
        &SEQUENCES[self.sequence as usize].notifies[self.index as usize]
    }

    pub fn class(&self) -> NotifyClass {
        self.def().class
    }

    /// The sequence the notify belongs to (the pack file `anims/<name>.glb`).
    pub fn sequence_name(&self) -> &'static str {
        SEQUENCES[self.sequence as usize].name
    }
}

/// Indices into `SEQUENCES` and weights of the two heaviest samples at `(axis, axis_y)`.
fn sample_indices_2d(data: &AnimData, axis: f32, axis_y: f32) -> [(usize, f32); 2] {
    let Some(space) = data.space2d else { return sample_indices(data, axis) };
    // Stock UBlendSpace::GetSamplesFromBlendInput: the input in grid units, the four grid points
    // around it mixed bilinearly; each grid point lists up to three (sample, weight) entries.
    let unit = |value: f32, min: f32, max: f32, grid: usize| if max > min { (value.clamp(min, max) - min) / (max - min) * grid as f32 } else { 0.0 };
    let (gx, gy) = (unit(axis, space.x_min, space.x_max, space.x_grid), unit(axis_y, space.y_min, space.y_max, space.y_grid));
    let (ix, iy) = ((gx.floor() as usize).min(space.x_grid.saturating_sub(1)), (gy.floor() as usize).min(space.y_grid.saturating_sub(1)));
    let (fx, fy) = (gx - ix as f32, gy - iy as f32);
    let mut weights = vec![0.0f32; space.samples.len()];
    for (dx, dy, w) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
        let (x, y) = ((ix + dx).min(space.x_grid), (iy + dy).min(space.y_grid));
        let Some(entries) = space.grid.get(x * (space.y_grid + 1) + y) else { continue };
        for &(sample, weight) in *entries {
            if let Some(slot) = weights.get_mut(sample) {
                *slot += w * weight;
            }
        }
    }
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by(|a, b| weights[*b].total_cmp(&weights[*a]).then(a.cmp(b)));
    let first = order.first().copied().unwrap_or(0);
    let second = order.get(1).copied().unwrap_or(first);
    let (wa, wb) = (weights.get(first).copied().unwrap_or(1.0), if second == first { 0.0 } else { weights.get(second).copied().unwrap_or(0.0) });
    let total = wa + wb;
    let sequence = |sample: usize| space.samples.get(sample).map_or(data.samples[0].1, |s| s.2);
    if total > 0.0 {
        [(sequence(first), wa / total), (sequence(second), wb / total)]
    } else {
        [(sequence(first), 1.0), (sequence(first), 0.0)]
    }
}

/// Indices into `SEQUENCES` and weights of the (at most two) samples around `axis`.
fn sample_indices(data: &AnimData, axis: f32) -> [(usize, f32); 2] {
    let samples = data.samples;
    let first = samples[0];
    if samples.len() == 1 || axis <= first.0 {
        return [(first.1, 1.0), (first.1, 0.0)];
    }
    for pair in samples.windows(2) {
        let (low, high) = (pair[0], pair[1]);
        if axis <= high.0 {
            let t = (axis - low.0) / (high.0 - low.0);
            return [(low.1, 1.0 - t), (high.1, t)];
        }
    }
    let last = samples[samples.len() - 1];
    [(last.1, 1.0), (last.1, 0.0)]
}

/// The `FullBody` slot after this frame's pose tick: what the pose evaluation of this frame uses.
///
/// The game blends the slot against its SAVED POSE of the previous frame, recursively
/// (animation spec 6, 8): `S[n] = blend(montage pose at `time`, S[n-1], weight)`; when `active`
/// is false the slot shows the saved pose unchanged. One evaluation per simulation step: the
/// recurrence depends on the update rate and cannot be sampled at other instants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimRequest {
    /// Which animation (a sequence or a 1D blend space).
    pub name: AnimId,
    /// Counts the play calls.
    pub play_id: u32,
    /// Position in the animation, seconds (wrapped when it loops, held at the end otherwise).
    pub time: f32,
    /// The montage instance contributes to this frame's pose. False from the update AFTER a
    /// one-shot reached its end (that update still shows its last frame): the slot then shows the
    /// saved pose (change list A8).
    pub active: bool,
    /// Montage weight of this update: `min(1, time since the play call / blend_in)`, linear;
    /// already `dt / blend_in` on the first update (A2).
    pub weight: f32,
    /// Play-call mode bit 0; the asset has no say (0x14056E920).
    pub looping: bool,
    /// The resolved blend-in time of the play call, seconds.
    pub blend_in: f32,
    /// Blend-space axis value after the blend space's own input filter; 0 for a sequence. Draw
    /// with [`AnimId::sample_weights`].
    pub axis: f32,
    /// True when the core moves and turns the actor by this animation's root track (stage 1:
    /// `RunStartN`). The pose must then have its root bone locked to the mesh reference root
    /// (change list A12), or the motion is applied twice.
    pub root_motion: bool,
    /// The notify window of this update on the animation's clock, `(from, to]`, already shifted as
    /// the game shifts it; `from < 0` on the update that also fires the notifies at t = 0. For
    /// hosts that run their own notifies off the same clock (leg-IK windows, face plays).
    /// `(0, 0)` when nothing advanced. On an update that wrapped a loop, `to < from`: the window is
    /// `(from, length]` then `(0, to]`.
    pub notify_window: (f32, f32),
    /// PORT OPTION `Tuning::speed_matched_anim` (not the game's playback): when set, the slot
    /// plays the phase-synchronised blend of the walk cycle and the run blend space instead of
    /// one of them; `name` is then the DOMINANT side (whose notifies fire and whose clock `time`
    /// and `notify_window` are on), `axis` the run side's lean.
    pub locomotion: Option<Locomotion>,
    /// The length of the play on the clock `time` runs on, seconds: the animation's length, or,
    /// for a blend space whose samples differ in length (`RunStartB`, `TurnN`, `RailSlideLoop`,
    /// `BallRide`), the weight-blended length of the samples in use (stock blend-space playback:
    /// one NORMALISED position, each sample read at position x its own length).
    pub length: f32,
}

impl AnimRequest {
    /// The (at most two) sequences to draw with their weights AND the time to read each at: for
    /// a blend space whose samples differ in length each sample is at the same normalised
    /// position, i.e. at `time x its length / length`; otherwise both at `time`.
    pub fn body_samples(&self) -> [(&'static str, f32, f32); 2] {
        let data = self.name.data();
        let picked = sample_indices_2d(data, self.axis, 0.0);
        picked.map(|(index, weight)| {
            let sequence = &SEQUENCES[index];
            let time = if data.sample_lengths_differ && self.length > 0.0 { self.time * sequence.length / self.length } else { self.time };
            (sequence.name, weight, time)
        })
    }
}

/// The speed-matched locomotion loop of one update (see [`AnimRequest::locomotion`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Locomotion {
    /// Weight of the run blend space, 0 to 1; the walk cycle has the rest.
    pub run_weight: f32,
    /// The shared phase of both cycles, 0 to 1.
    pub phase: f32,
    /// Cycles per second in force for the next update.
    pub rate: f32,
    /// Where to sample the walk sequence and the run samples, seconds.
    pub walk_time: f32,
    pub run_time: f32,
    /// The clip set in use (actions implementer: `N` weapon away, `W` Keyblade out, `B` battle
    /// pose): the walk cycle and the run blend space to draw.
    pub walk: AnimId,
    pub run: AnimId,
}

/// A walk cycle + run blend space pair the speed-matched loop can play (PORT OPTION
/// `Tuning::speed_matched_anim`; generalised to the Keyblade-out sets by a coordinator decision,
/// 2026-10-11: the user runs mostly with the Keyblade out). `walk_phase` is the fundamental
/// phase of (left foot forward - right foot forward) of the walk cycle and `run_shift` the run
/// cycle's phase relative to it, both in cycles, computed from the pack by
/// build/sora_tmp/impl_actions/phase_sets.js (N: 0002 0.7310 / 0003 0.7211; W: 1002 0.7310 / 1003
/// 0.7396; B: 1016 0.8736 / 1017 0.6665).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocoSet {
    pub walk: AnimId,
    pub run: AnimId,
    walk_phase: f32,
    run_shift: f32,
}

impl LocoSet {
    pub const N: LocoSet = LocoSet { walk: AnimId::WalkLoopN, run: AnimId::RunLoopN, walk_phase: 0.7310, run_shift: RUN_PHASE_SHIFT };
    pub const W: LocoSet = LocoSet { walk: AnimId::WalkLoopW, run: AnimId::RunLoopW, walk_phase: 0.7310, run_shift: 0.0086 };
    pub const B: LocoSet = LocoSet { walk: AnimId::WalkLoopB, run: AnimId::RunLoopB, walk_phase: 0.8736, run_shift: -0.2071 };

    fn clip_phase(&self, name: AnimId, phase: f32) -> f32 {
        if name == self.run {
            (phase + self.run_shift).rem_euclid(1.0)
        } else {
            phase
        }
    }
}

/// Phase of the run cycle (0003) relative to the walk cycle (0002), in cycles: the fundamental
/// of (left foot forward - right foot forward) over each cycle, computed from the pack by
/// build/sora_tmp/turning/phase_check.js (walk 0.7310, run 0.7211). The run samples are read at
/// the shared phase plus this, so that the same foot leads in both.
pub const RUN_PHASE_SHIFT: f32 = -0.0099;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Loco {
    phase: f32,
    run_weight: f32,
    rate: f32,
    set: LocoSet,
}

/// Notifies fired by one pose tick, for the states and the generic handlers (notify.rs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Notifies {
    /// `TresJumpStart` fired (the stage-1 path: the first notify of that name per sequence).
    pub jump_start: bool,
    /// `TresEnableChange` fired (the same).
    pub enable_change: bool,
    /// EVERY notify of this update, in dispatch order: the instant notifies in queue order
    /// (sample by sample, each sequence's notifies by time), then the Ends of the notify states
    /// no longer listed, then the Begins of the new ones, then a Tick for every listed state.
    pub events: Vec<NotifyEvent>,
}

impl Notifies {
    /// The events of one class.
    #[allow(dead_code)]
    pub fn of(&self, class: NotifyClass) -> impl Iterator<Item = &NotifyEvent> + '_ {
        self.events.iter().filter(move |e| e.class() == class)
    }

    /// An instant notify of this class fired, or a notify state of it began.
    #[allow(dead_code)]
    pub fn fired(&self, class: NotifyClass) -> bool {
        self.of(class).any(|e| matches!(e.phase, NotifyPhase::Instant | NotifyPhase::Begin))
    }
}

/// The engine's time-based FIR filter as this exe has it (`FFIRFilterTimeBased`, 0x143F425B0;
/// animation spec 8, change list A6): one entry per update, stamped with the clock BEFORE that
/// update plus the window; Linear type: weight `1 - |clock - stamp| / window`. A triangular
/// kernel twice the window wide whose peak is one window in the past; not weighted by dt.
#[derive(Clone, Debug, PartialEq, Default)]
struct FirFilter {
    /// (input, stamp); a stamp of 0 marks a free entry.
    entries: Vec<(f32, f32)>,
    clock: f32,
    window: f32,
}

impl FirFilter {
    /// `Reset(value)` (0x143F4E310), called by the play: one entry holding the play-time parameter.
    fn reset(&mut self, window: f32, value: f32) {
        self.entries.clear();
        self.window = window;
        self.clock = 1.0;
        if window > 0.0 {
            self.entries.push((value, 1.0));
        }
    }

    fn update(&mut self, input: f32, dt: f32) -> f32 {
        self.clock += dt;
        if !(self.window > 0.0) {
            return input;
        }
        let (clock, window) = (self.clock, self.window);
        self.entries.retain(|e| clock - e.1 <= window);
        self.entries.push((input, (clock - dt) + window));
        let (mut sum_weight, mut sum) = (0.0, 0.0);
        for &(value, stamp) in &self.entries {
            let distance = (clock - stamp).abs();
            if distance > window {
                continue;
            }
            let weight = 1.0 - distance / window;
            if weight <= 0.0 {
                continue;
            }
            sum_weight += weight;
            sum += weight * value;
        }
        if sum_weight > 0.0 {
            sum / sum_weight
        } else {
            0.0
        }
    }
}

/// What one pose tick produced for the movement component and the states.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PoseTick {
    pub notifies: Notifies,
    /// Root motion of this update in the actor's CURRENT frame: (forward cm, right cm, up cm,
    /// yaw degrees). Up is 0 for every stage-1 clip.
    pub root_motion: Option<(f32, f32, f32, f32)>,
}

/// The pawn's `FullBody` montage slot.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AnimPlayer {
    pub name: AnimId,
    pub play_id: u32,
    /// The clock (montage position), seconds.
    pub time: f32,
    /// Real time since the play call, for the blend-in weight.
    blend_time: f32,
    /// The instance is playing (`bPlaying`): false once a one-shot has reached its end.
    playing: bool,
    /// "Is-anim-end" (0x14056BFD0): true from the update that reaches the end of a one-shot.
    pub ended: bool,
    /// The instance still contributes to the pose of the current frame.
    active: bool,
    /// Flag b3 of the instance: no pose tick has run since the play call.
    first_update: bool,
    /// Instance +0x17C.
    notify_offset: f32,
    /// Axis value as last set by the state; filtered value used this frame.
    axis_input: f32,
    axis: f32,
    filter: FirFilter,
    /// `m_bReverseLeg`: the `TresReverseLeg` notify state was in this update's window.
    pub reverse_leg: bool,
    window: (f32, f32),
    /// The speed-matched locomotion loop, when it plays (port option).
    loco: Option<Loco>,
    /// The length of the clock in force (see `AnimRequest::length`); 0 until the first update of
    /// a blend space whose samples differ in length.
    play_length: f32,
    /// The play-call arguments when the animation was played with `play_with`; None for a
    /// stage-1 `play`, whose arguments are the generated row's.
    args: Option<PlayArgs>,
    /// The notify states listed by the latest update: (index into `SEQUENCES`, index of the
    /// notify). Stock `ActiveAnimNotifyState`: a state of the animation that was playing before
    /// a play call ends on the next update, after that update's instant notifies.
    active_states: Vec<(u16, u16)>,
    /// (XSI, 2026-10-16) The clock stands while this is set (the pose of the current position is held, no root
    /// motion, no new notify): Diving Strike's dive pose (abilities/xs/dive.rs), Zantetsuken's draw stance. Cleared
    /// by every play call. Not a KH3 field.
    pub hold: bool,
}

impl AnimPlayer {
    pub fn new(name: AnimId) -> AnimPlayer {
        let mut player = AnimPlayer {
            name,
            play_id: 0,
            time: 0.0,
            blend_time: 0.0,
            playing: false,
            ended: false,
            active: false,
            first_update: true,
            notify_offset: NOTIFY_OFFSET,
            axis_input: 0.0,
            axis: 0.0,
            filter: FirFilter::default(),
            reverse_leg: false,
            window: (0.0, 0.0),
            loco: None,
            play_length: 0.0,
            args: None,
            active_states: Vec::new(),
            hold: false,
        };
        player.play(name);
        player
    }

    /// Play by name: always restarts; the previous instance is cut at once (animation spec 8).
    /// The blend space's filter is reset to the play-time parameter 0 (A7).
    pub fn play(&mut self, name: AnimId) {
        self.args = None;
        self.start(name);
    }

    /// Play by name with the play call's own arguments (mode bits, blend-in): what every state
    /// beyond stage 1 uses (animation spec 2.2). Always restarts, like `play`.
    pub fn play_with(&mut self, name: AnimId, args: PlayArgs) {
        self.args = Some(args);
        self.start(name);
    }

    /// (magic implementer) `SetAnimTime(t)` (anim interface vt+0xF0, 0x141507500 -> pawn
    /// 0x1405A1810; its getter vt+0xE8 is the "animation time" every state compares): the
    /// position of the playing instance. The notifies up to `t` are NOT fired (the next update's
    /// window starts at the new position): assumed (magic spec H open question 6).
    pub fn set_time(&mut self, t: f32) {
        self.time = t.clamp(0.0, self.name.data().length);
        self.first_update = false;
    }

    /// Mode bit 0 of the play call in force.
    pub fn looping(&self) -> bool {
        self.args.map_or(self.name.data().looping, |a| a.looping)
    }

    /// Root motion is extracted: the mode bit and `bEnableRootMotion` on the sequences.
    pub fn root_motion(&self) -> bool {
        let data = self.name.data();
        self.args.map_or(data.root_motion, |a| a.root_motion && data.root_motion_enabled)
    }

    /// The resolved blend-in of the play call in force, seconds.
    pub fn blend_in(&self) -> f32 {
        let data = self.name.data();
        match self.args {
            Some(a) if a.blend_in >= 0.0 => a.blend_in,
            _ => data.blend_in,
        }
    }

    /// A notify state of this class was listed by the latest update (it is active).
    #[allow(dead_code)]
    pub fn state_active(&self, class: NotifyClass) -> bool {
        self.active_states.iter().any(|&(s, k)| SEQUENCES[s as usize].notifies[k as usize].class == class)
    }

    fn start(&mut self, name: AnimId) {
        self.hold = false; // (XSI)
        self.loco = None;
        self.play_length = 0.0;
        self.name = name;
        self.play_id = self.play_id.wrapping_add(1);
        self.time = 0.0;
        self.blend_time = 0.0;
        self.playing = true;
        self.ended = false;
        self.active = true;
        self.first_update = true;
        self.notify_offset = NOTIFY_OFFSET;
        self.axis_input = 0.0;
        self.axis = 0.0;
        self.filter.reset(name.data().axis_smoothing, 0.0);
        // The stopped instance's notify states end.
        self.reverse_leg = false;
        self.window = (0.0, 0.0);
    }

    /// "Set blend-space parameter" (0x14056B030): stored; it enters the blend space's filter as a
    /// normal entry on the next pose tick (A7). No clamp here: the states clamp what they pass.
    pub fn set_axis(&mut self, value: f32) {
        if self.loco.is_some() || self.name.data().blend_space {
            self.axis_input = value;
        }
    }

    /// The axis range of the playing asset (`GetBlendSpaceParamRange`), if it is a blend space.
    pub fn axis_range(&self) -> Option<(f32, f32)> {
        // (The locomotion loop always has the run side's lean.)
        let data = self.loco.map_or(self.name, |l| l.set.run).data();
        data.blend_space.then_some((data.axis_min, data.axis_max))
    }

    pub fn weight(&self) -> f32 {
        let blend_in = self.blend_in();
        if blend_in <= 0.0 {
            1.0
        } else {
            (self.blend_time / blend_in).min(1.0)
        }
    }

    pub fn request(&self) -> AnimRequest {
        AnimRequest {
            name: self.name,
            play_id: self.play_id,
            time: self.time,
            active: self.active,
            weight: self.weight(),
            looping: self.looping(),
            blend_in: self.blend_in(),
            axis: self.axis,
            root_motion: self.root_motion(),
            notify_window: self.window,
            locomotion: self.loco.map(|l| Locomotion {
                run_weight: l.run_weight,
                phase: l.phase,
                rate: l.rate,
                walk_time: l.set.clip_phase(l.set.walk, l.phase) * l.set.walk.data().length,
                run_time: l.set.clip_phase(l.set.run, l.phase) * l.set.run.data().length,
                walk: l.set.walk,
                run: l.set.run,
            }),
            length: self.length(),
        }
    }

    /// PORT OPTION `Tuning::speed_matched_anim`: starts the locomotion loop with a play call (the
    /// slot's weight and blend-in start as for the game's loop play; both loops blend in over
    /// 0.2 s), in the gait given. The lean filter is the run blend space's.
    pub fn play_locomotion(&mut self, set: LocoSet, run: bool) {
        let dominant = if run { set.run } else { set.walk };
        if set == LocoSet::N {
            // The stage-1 call (tests/golden.rs pins it).
            self.play(dominant);
        } else {
            // The `W` / `B` loops take the play arguments of their `N` twin (Part I 9.3).
            let twin = if run { AnimId::RunLoopN } else { AnimId::WalkLoopN }.data();
            self.play_with(dominant, PlayArgs { looping: twin.looping, root_motion: twin.root_motion_bit, blend_in: twin.blend_in });
        }
        self.filter.reset(set.run.data().axis_smoothing, 0.0);
        self.loco = Some(Loco { phase: 0.0, run_weight: if run { 1.0 } else { 0.0 }, rate: 1.0 / dominant.data().length, set });
    }

    /// The clip set changed under the running loop (the Keyblade was drawn or put away, the
    /// battle pose began or ended): a play call of the new set's loop (the slot blends in from
    /// the saved pose over 0.2 s, as the game's own change of loop does), with the blend, the
    /// rate, the lean and the FEET's phase carried over.
    pub fn retarget_locomotion(&mut self, set: LocoSet) {
        let Some(old) = self.loco else { return };
        let (axis_input, axis, filter) = (self.axis_input, self.axis, self.filter.clone());
        self.play_locomotion(set, old.run_weight >= 0.5);
        (self.axis_input, self.axis, self.filter) = (axis_input, axis, filter);
        let phase = (old.phase + set.walk_phase - old.set.walk_phase).rem_euclid(1.0);
        self.loco = Some(Loco { phase, set, ..old });
    }

    pub fn in_locomotion(&self) -> bool {
        self.loco.is_some()
    }

    /// The clip set of the running locomotion loop.
    pub fn locomotion_set(&self) -> Option<LocoSet> {
        self.loco.map(|l| l.set)
    }

    /// The length of the clock in force (`AnimRequest::length`).
    pub fn length(&self) -> f32 {
        let data = self.name.data();
        if data.sample_lengths_differ && self.play_length > 0.0 {
            self.play_length
        } else {
            data.length
        }
    }

    /// Sets the blend and the cycle rate the next pose tick uses. No play call: the slot's
    /// weight, the saved pose and the phase go on; only the dominant side's name changes.
    pub fn set_locomotion(&mut self, run_weight: f32, rate: f32) {
        if let Some(l) = &mut self.loco {
            l.run_weight = run_weight.clamp(0.0, 1.0);
            l.rate = rate.max(0.0);
            self.name = if l.run_weight >= 0.5 { l.set.run } else { l.set.walk };
        }
    }

    /// The pose tick of the locomotion loop: the shared phase advances by `rate x dt`; the
    /// notifies are the dominant side's, at its own times mapped through the phase.
    fn locomotion_tick(&mut self, dt: f32, loco: Loco) -> PoseTick {
        let mut out = PoseTick::default();
        self.blend_time += dt;
        let first = std::mem::take(&mut self.first_update);
        self.axis = self.filter.update(self.axis_input, dt);
        let next = (loco.phase + loco.rate * dt).rem_euclid(1.0);
        self.loco = Some(Loco { phase: next, ..loco });
        let data = self.name.data();
        let (from, to) = (loco.set.clip_phase(self.name, loco.phase) * data.length, loco.set.clip_phase(self.name, next) * data.length);
        self.time = to;
        let start = if first { -1.0 } else { from };
        // A wrapped update is two pieces, as for a looping montage.
        let pieces = if to < from { [(start, data.length), (0.0, to)] } else { [(start, to), (0.0, 0.0)] };
        self.window = (start, to);
        let instant = |trigger: f32| trigger >= 0.0 && pieces.iter().any(|&(a, b)| trigger > a && trigger <= b);
        let state = |(begin, end): (f32, f32)| begin >= 0.0 && pieces.iter().any(|&(a, b)| begin <= b && end > a);
        self.reverse_leg = false;
        for &(_, index) in data.samples {
            let sequence = &SEQUENCES[index];
            out.notifies.jump_start |= instant(sequence.jump_start);
            out.notifies.enable_change |= instant(sequence.enable_change);
            self.reverse_leg |= state(sequence.reverse_leg);
        }
        collect_events(&mut self.active_states, data, self.axis, &|_| 1.0, &instant, &state, &mut out.notifies.events);
        out
    }

    /// The pose tick of one frame (`UAnimInstance::UpdateAnimation` through the movement
    /// component, 0x140595730): weight, clock, blend-space filter, root-motion extraction, notifies.
    pub fn pose_tick(&mut self, dt: f32) -> PoseTick {
        let mut out = PoseTick::default();
        if !self.playing {
            // The instance was stopped by its end (or nothing plays): the slot shows its source.
            self.active = false;
            self.window = (0.0, 0.0);
            self.reverse_leg = false;
            // No notify is queued: the states that were active end.
            for (sequence, index) in self.active_states.drain(..) {
                out.notifies.events.push(NotifyEvent { sequence, index, phase: NotifyPhase::End });
            }
            return out;
        }
        if let Some(loco) = self.loco {
            return self.locomotion_tick(dt, loco);
        }
        let data = self.name.data();
        // Weight: linear in real elapsed time; the first update already counts (A2).
        self.blend_time += dt;
        // Clock: flag b3 (A3; see FIRST_UPDATE_HOLDS_POSITION).
        let first = std::mem::take(&mut self.first_update);
        let step = if self.hold || (first && FIRST_UPDATE_HOLDS_POSITION) { 0.0 } else { dt }; // (XSI) hold
        // The blend space's own input filter (A6), ticked with the same dt as the clock.
        if data.blend_space {
            self.axis = self.filter.update(self.axis_input, step);
        }
        // A blend space whose samples differ in length (actions implementer; stock
        // `UBlendSpaceBase::TickAssetPlayer`): the clock's length is the weight-blended length of
        // the samples in use, the position is kept NORMALISED when that length changes, and
        // sample i is read at position x (its length / that length).
        let length = if data.sample_lengths_differ {
            let blended: f32 = sample_indices_2d(data, self.axis, 0.0).iter().map(|&(index, weight)| weight * SEQUENCES[index].length).sum();
            let blended = if blended > 0.0 { blended } else { data.length };
            if self.play_length > 0.0 && self.play_length != blended {
                self.time *= blended / self.play_length;
            }
            self.play_length = blended;
            blended
        } else {
            data.length
        };
        let scale = |index: usize| if data.sample_lengths_differ { SEQUENCES[index].length / length } else { 1.0 };
        let from = self.time;
        let offset = self.notify_offset;
        let mut to = from + step;
        let (looping, root_motion) = (self.looping(), self.root_motion());
        // The notify window of this update, as pieces (a, b] (animation spec 9, change list A9):
        // first update (the t = 0 frame): from the position itself, so t = 0 notifies fire;
        // later: both ends shifted by the offset; after the first loop wrap: unshifted.
        let mut pieces = [(0.0f32, 0.0f32); 2];
        let mut count = 1;
        let start = if first { -1.0 } else { from + offset };
        if looping {
            if length > 0.0 && to >= length {
                // The piece up to the end, then the remainder carried exactly (A11).
                pieces[0] = (start, length + offset);
                to %= length;
                pieces[1] = (0.0, to);
                count = 2;
                self.notify_offset = 0.0;
                self.window = (start, to);
            } else {
                pieces[0] = (start, to + offset);
                self.window = pieces[0];
            }
        } else {
            if to >= length {
                // The end of a one-shot: Stop is issued in this update; this frame shows the last
                // frame at the current weight; IsAnimEnd is true from now (A8).
                to = length;
                self.playing = false;
                self.ended = true;
            }
            // The piece is cut at the sequence end.
            pieces[0] = (start, (to + offset).min(length));
            self.window = pieces[0];
        }
        // Root motion over the position interval of this update (zero on a held first update),
        // consumed by this same frame's move (A13).
        if root_motion && to > from {
            out.root_motion = Some(self.root_delta(data, from, to, &scale));
        }
        self.time = to;

        // Notifies. An instant notify with trigger T fires when a < T <= b (stock
        // GetAnimNotifiesFromDeltaPositions, 0x143F654D0); a notify state is listed while
        // start <= b and end > a. A blend space in mode SQEX_AllAnimation fires every sample's
        // notifies (A10); the gameplay notifies of the stage-1 blend spaces are the three run-loop
        // samples' identical TresReverseLeg windows.
        let pieces = &pieces[..count];
        let instant = |trigger: f32| trigger >= 0.0 && pieces.iter().any(|&(a, b)| trigger > a && trigger <= b);
        let state = |(begin, end): (f32, f32)| begin >= 0.0 && pieces.iter().any(|&(a, b)| begin <= b && end > a);
        self.reverse_leg = false;
        for &(_, index) in data.samples {
            let sequence = &SEQUENCES[index];
            // (A sample's own times on the play's clock: divided by its scale; 1.0 unless the
            // samples differ in length.)
            let k = scale(index);
            out.notifies.jump_start |= instant(if sequence.jump_start >= 0.0 { sequence.jump_start / k } else { sequence.jump_start });
            out.notifies.enable_change |= instant(if sequence.enable_change >= 0.0 { sequence.enable_change / k } else { sequence.enable_change });
            self.reverse_leg |= state(if sequence.reverse_leg.0 >= 0.0 { (sequence.reverse_leg.0 / k, sequence.reverse_leg.1 / k) } else { sequence.reverse_leg });
        }
        collect_events(&mut self.active_states, data, self.axis, &scale, &instant, &state, &mut out.notifies.events);
        out
    }

    /// The blend of the samples' root-motion deltas between two clock positions, each taken
    /// relative to its own root at `from` (the engine extracts per sample and accumulates with the
    /// sample weights; animation spec 8). The rotation is blended as yaw numbers, which for these
    /// pure-yaw tracks equals the engine's quaternion blend.
    fn root_delta(&self, data: &AnimData, from: f32, to: f32, scale: &dyn Fn(usize) -> f32) -> (f32, f32, f32, f32) {
        let (mut x, mut y, mut z, mut yaw) = (0.0, 0.0, 0.0, 0.0);
        for (index, weight) in sample_indices(data, self.axis) {
            if weight <= 0.0 {
                continue;
            }
            let track = SEQUENCES[index].root;
            let k = scale(index);
            let (a, b) = (sample_root(track, from * k), sample_root(track, to * k));
            // Into the frame of the root at `from`: turn back by the yaw reached there.
            let (sin, cos) = (-a.yaw).to_radians().sin_cos();
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            x += weight * (dx * cos - dy * sin);
            y += weight * (dx * sin + dy * cos);
            z += weight * (b.z - a.z);
            yaw += weight * (b.yaw - a.yaw);
        }
        (x, y, z, yaw)
    }
}

/// The general notify dispatch of one update (animation spec 9; stock `TriggerAnimNotifies`
/// 0x143F2DC50): the instant notifies in queue order, then End for every state that was active
/// and is not listed any more, Begin for every newly listed one, Tick for every listed one.
/// `instant` and `state` are the update's window tests (the 1e-4 shift is in them). A blend space
/// in mode `SQEX_AllAnimation` queues the notifies of every sample; any other blend space those
/// of its heaviest sample only (stock `ENotifyTriggerMode::HighestWeightedAnimation`).
fn collect_events(active: &mut Vec<(u16, u16)>, data: &AnimData, axis: f32, scale: &dyn Fn(usize) -> f32, instant: &dyn Fn(f32) -> bool, state: &dyn Fn((f32, f32)) -> bool, events: &mut Vec<NotifyEvent>) {
    let heaviest = if data.blend_space && !data.all_animation { Some(sample_indices_2d(data, axis, 0.0)[0].0) } else { None };
    let mut listed: Vec<(u16, u16)> = Vec::new();
    for (n, &(_, index)) in data.samples.iter().enumerate() {
        if heaviest.is_some_and(|h| h != index) {
            continue;
        }
        // A sequence that is the sample of several points is queued once.
        if data.samples[..n].iter().any(|&(_, earlier)| earlier == index) {
            continue;
        }
        let per = scale(index);
        for (k, notify) in SEQUENCES[index].notifies.iter().enumerate() {
            if notify.state {
                if state((notify.time / per, notify.end / per)) {
                    listed.push((index as u16, k as u16));
                }
            } else if instant(notify.time / per) {
                events.push(NotifyEvent { sequence: index as u16, index: k as u16, phase: NotifyPhase::Instant });
            }
        }
    }
    for &(sequence, index) in active.iter() {
        if !listed.contains(&(sequence, index)) {
            events.push(NotifyEvent { sequence, index, phase: NotifyPhase::End });
        }
    }
    for &(sequence, index) in &listed {
        if !active.contains(&(sequence, index)) {
            events.push(NotifyEvent { sequence, index, phase: NotifyPhase::Begin });
        }
    }
    for &(sequence, index) in &listed {
        events.push(NotifyEvent { sequence, index, phase: NotifyPhase::Tick });
    }
    *active = listed;
}

/// A root track at `time`, linear between its frames.
fn sample_root(track: &[RootKey], time: f32) -> RootKey {
    let Some(&first) = track.first() else { return RootKey { time, x: 0.0, y: 0.0, z: 0.0, yaw: 0.0 } };
    if time <= first.time {
        return first;
    }
    for pair in track.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if time <= b.time {
            let t = if b.time > a.time { (time - a.time) / (b.time - a.time) } else { 1.0 };
            return RootKey { time, x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, z: a.z + (b.z - a.z) * t, yaw: a.yaw + (b.yaw - a.yaw) * t };
        }
    }
    track[track.len() - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn fir_filter_is_the_triangular_kernel_of_the_spec() {
        // Animation spec 8 / change list A6: reset entry weight 1 - elapsed / window; the newest
        // input has weight dt / window on its first update; peak one window in the past.
        let mut f = FirFilter::default();
        f.reset(0.4, 0.0);
        // First update with a constant input 10: entries {0 @ stamp 1.0}, {10 @ stamp 1.4}.
        let first = f.update(10.0, DT);
        let (w_reset, w_new) = (1.0 - DT / 0.4, DT / 0.4);
        assert!((first - 10.0 * w_new / (w_reset + w_new)).abs() < 1.0e-5, "first output {first}");
        // The reset entry is gone after one window; with a constant input the output is then it.
        let mut last = first;
        for _ in 0..40 {
            last = f.update(10.0, DT);
        }
        assert!((last - 10.0).abs() < 1.0e-4, "after 0.68 s: {last}");
        // A step in the input is felt most one window later: an impulse response peaking at 0.4 s.
        let mut f = FirFilter::default();
        f.reset(0.4, 0.0);
        for _ in 0..60 {
            f.update(0.0, DT);
        }
        let mut response = vec![f.update(1.0, DT)];
        for _ in 0..60 {
            response.push(f.update(0.0, DT));
        }
        let peak = response.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0;
        assert!((peak as f32 * DT - 0.4).abs() <= 2.0 * DT, "impulse peak after {} s", peak as f32 * DT);
        assert!(response[50] == 0.0, "support is two windows: {}", response[50]);
        // No window: pass-through (RunStartN).
        let mut f = FirFilter::default();
        f.reset(0.0, 0.0);
        assert_eq!(f.update(-135.0, DT), -135.0);
    }

    #[test]
    fn weight_is_linear_in_real_time_from_the_first_update() {
        // A2: w_n = min(1, (dt_1 + ... + dt_n) / blendIn); IdleN blends in over 0.067 s.
        let mut p = AnimPlayer::new(AnimId::IdleN);
        let mut weights = Vec::new();
        for _ in 0..6 {
            p.pose_tick(DT);
            weights.push(p.request().weight);
        }
        assert!((weights[0] - DT / 0.067).abs() < 1.0e-5 && (weights[2] - 3.0 * DT / 0.067).abs() < 1.0e-5);
        assert_eq!(weights[4], 1.0);
        // Blend-in 0: weight 1 on the first update.
        let mut p = AnimPlayer::new(AnimId::Land);
        p.pose_tick(DT);
        assert_eq!(p.request().weight, 1.0);
    }

    #[test]
    fn a_one_shot_ends_on_the_update_that_reaches_its_end_and_then_leaves_the_slot() {
        // A8: JumpNormalStart is 0.1 s long.
        let mut p = AnimPlayer::new(AnimId::JumpNormalStart);
        let mut ended_on = None;
        for update in 1..=10 {
            p.pose_tick(DT);
            if p.ended && ended_on.is_none() {
                ended_on = Some(update);
                assert!(p.request().active && (p.request().time - 0.1).abs() < 1.0e-6, "the ending update still shows the last frame");
            }
        }
        let expected = if FIRST_UPDATE_HOLDS_POSITION { 7 } else { 6 };
        assert!(ended_on == Some(expected) || ended_on == Some(expected + 1), "ended on update {ended_on:?}");
        assert!(!p.request().active, "from the next update the slot shows its saved pose");
    }

    #[test]
    fn notifies_fire_by_the_shifted_window() {
        // A9: TresJumpStart (0.0666445 s) at 60 Hz fires on the first update whose position is
        // >= T - 1e-4: position 4/60 = 0.066667.
        let mut p = AnimPlayer::new(AnimId::JumpNormalStart);
        let fired = (1..=8).find(|_| p.pose_tick(DT).notifies.jump_start).expect("TresJumpStart");
        assert_eq!(fired, if FIRST_UPDATE_HOLDS_POSITION { 5 } else { 4 });
        // TresEnableChange (0.0991 s) in Land: position 6/60 = 0.1.
        let mut p = AnimPlayer::new(AnimId::Land);
        let fired = (1..=10).find(|_| p.pose_tick(DT).notifies.enable_change).expect("TresEnableChange");
        assert_eq!(fired, if FIRST_UPDATE_HOLDS_POSITION { 7 } else { 6 });
        // Exactly once.
        assert!((0..30).all(|_| !p.pose_tick(DT).notifies.enable_change));
    }

    #[test]
    fn reverse_leg_follows_the_notify_state_of_the_loop() {
        // A10: TresReverseLeg 0.100371 - 0.333671 in the 0.5333 s run loop, listed on every update
        // whose (shifted) window overlaps it.
        let mut p = AnimPlayer::new(AnimId::RunLoopN);
        let mut on = Vec::new();
        for _ in 0..64 {
            p.pose_tick(DT);
            on.push(p.reverse_leg);
        }
        let begin = on.iter().position(|b| *b).unwrap();
        let end = begin + on[begin..].iter().position(|b| !*b).unwrap();
        let t = |update: usize| (update + if FIRST_UPDATE_HOLDS_POSITION { 0 } else { 1 }) as f32 * DT;
        // Begin: first update with position + 1e-4 >= 0.100371; End: first whose window starts at or after 0.333671.
        assert!(t(begin) + 1.0e-4 >= 0.100371 && t(begin - 1) + 1.0e-4 < 0.100371, "began at {}", t(begin));
        assert!(t(end - 1) + 1.0e-4 >= 0.333671 && t(end - 2) + 1.0e-4 < 0.333671, "ended at {}", t(end));
        // It comes round again one loop later.
        assert!(on[begin + 32]);
        // A play ends the state.
        let mut p = AnimPlayer::new(AnimId::RunLoopN);
        for _ in 0..10 {
            p.pose_tick(DT);
        }
        assert!(p.reverse_leg);
        p.play(AnimId::RunEndL);
        assert!(!p.reverse_leg);
    }

    #[test]
    fn loop_wrap_carries_the_remainder() {
        // A11.
        let mut p = AnimPlayer::new(AnimId::RunLoopN);
        let length = AnimId::RunLoopN.data().length;
        let mut total = 0.0f32;
        for _ in 0..200 {
            let before = p.time;
            p.pose_tick(DT);
            if !(FIRST_UPDATE_HOLDS_POSITION && total == 0.0 && before == 0.0 && p.time == 0.0) {
                total += DT;
            }
            assert!(p.time >= 0.0 && p.time < length);
        }
        assert!(((total % length) - p.time).abs() < 1.0e-3, "clock {} after {total} s", p.time);
    }

    #[test]
    fn root_motion_is_the_track_between_the_two_clock_positions() {
        // A13: RunStartN straight: 180 cm over 0.2333 s, nothing after the end.
        let mut p = AnimPlayer::new(AnimId::RunStartN);
        p.set_axis(0.0);
        let mut x = 0.0;
        let mut updates = 0;
        for _ in 0..20 {
            if let Some((dx, dy, _, yaw)) = p.pose_tick(DT).root_motion {
                x += dx;
                updates += 1;
                assert!(dy.abs() < 1.0e-4 && yaw == 0.0);
            }
        }
        assert!((x - 180.0).abs() < 1.0e-2, "moved {x} cm");
        assert_eq!(updates, 14);
        // The -90 sample turns him 90 degrees to the left while moving 180 cm to his left.
        let mut p = AnimPlayer::new(AnimId::RunStartN);
        p.set_axis(-90.0);
        let (mut heading, mut pos) = (0.0f32, (0.0f32, 0.0f32));
        for _ in 0..20 {
            if let Some((dx, dy, _, yaw)) = p.pose_tick(DT).root_motion {
                let (s, c) = heading.to_radians().sin_cos();
                pos = (pos.0 + dx * c - dy * s, pos.1 + dx * s + dy * c);
                heading += yaw;
            }
        }
        assert!((heading + 90.0).abs() < 0.01 && (pos.1 + 180.0).abs() < 0.5 && pos.0.abs() < 0.5, "ended at {pos:?} heading {heading}");
    }
}

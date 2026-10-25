//! The NPC's animation clock (contract B.7): the pawn's one montage slot as Sora's
//! (`kh_core::anim`, animation spec Part II 5, 8, 9) - blend-in weight over the saved pose,
//! clock, notify window with the 1e-4 offset, notify states begin / tick / end, loop wrap, root
//! motion - over a data table in this crate's own types, because Sora's generated tables are
//! his alone. Stream A generates the full tables (`party_anim_n_ex00x.rs`); until they exist the
//! crate falls back to `tables::MIN_CLIPS` (the clips the states play, read from the anim
//! sidecars by `tools/party_sheet.js`).
//!
//! No poses here: the host draws from [`crate::view::AnimView`].

use crate::ids::Chara;
use kh_core::anim::NotifyPhase;

/// One root-motion key: seconds; translation since frame 0 in the actor's frame-0 space (cm,
/// x forward, y right, z up); yaw since frame 0 (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootKey {
    pub time: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
}

/// The decoded part of a notify that the simulation acts on; everything else is `Raw` (E's
/// sound / effect layer reads the class and the raw text through the `Notify` event).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipNotifyData {
    None,
    /// `TresAnimNotifyState_Attack`: the attack-data row and the collision group it opens.
    Attack { attack: &'static str, collision: &'static str },
    /// A gameplay window (`TresAnimNotifyState_*` other than Attack): its class tail.
    State { name: &'static str },
    Sound { cue: &'static str },
    Effect { asset: &'static str, socket: &'static str },
    Raw(&'static str),
}

/// One notify of a clip. `end == start` is an instant notify; otherwise a notify state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipNotify {
    /// The game's class name (`TresAnimNotifyState_Attack`, `SQEXSEADAnimNotify_PlaySound` ...).
    pub class: &'static str,
    pub start: f32,
    pub end: f32,
    pub data: ClipNotifyData,
}

impl ClipNotify {
    pub fn is_state(&self) -> bool {
        self.end > self.start
    }
}

/// One anim-set row (contract B.7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipDef {
    /// The anim-set row name ("1301", "IdleN", "MagicFire").
    pub name: &'static str,
    /// The sequence asset it plays (first sample of a blend space).
    pub sequence: &'static str,
    /// Seconds.
    pub length: f32,
    /// `RateScale` of the sequence.
    pub rate: f32,
    /// The row's `m_InterpolationTime`, seconds.
    pub blend_in: f32,
    pub looping: bool,
    pub root_motion: bool,
    pub notifies: &'static [ClipNotify],
    pub root: &'static [RootKey],
    /// The weapon's attack socket (`ATK_00` / `ATK_01`) per key in the actor's frame (stream A's
    /// `manifest.attack_sockets`; empty = not packed yet: the hit test uses the stand-in sphere).
    pub socket: &'static [SocketKey],
}

/// One key of an attack-socket track: seconds; position in the actor frame (cm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SocketKey {
    pub time: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl ClipDef {
    /// A clip with only a length (tests, missing data).
    pub const fn bare(name: &'static str, length: f32, looping: bool) -> ClipDef {
        ClipDef { name, sequence: name, length, rate: 1.0, blend_in: 0.2, looping, root_motion: false, notifies: &[], root: &[], socket: &[] }
    }

    /// The root offset at `t` (linear between keys); zero when the clip has no root track.
    pub fn root_at(&self, t: f32) -> RootKey {
        let keys = self.root;
        if keys.is_empty() {
            return RootKey { time: t, x: 0.0, y: 0.0, z: 0.0, yaw: 0.0 };
        }
        if t <= keys[0].time {
            return keys[0];
        }
        for w in keys.windows(2) {
            if t <= w[1].time {
                let f = if w[1].time > w[0].time { (t - w[0].time) / (w[1].time - w[0].time) } else { 1.0 };
                let l = |a: f32, b: f32| a + (b - a) * f;
                return RootKey { time: t, x: l(w[0].x, w[1].x), y: l(w[0].y, w[1].y), z: l(w[0].z, w[1].z), yaw: l(w[0].yaw, w[1].yaw) };
            }
        }
        *keys.last().unwrap()
    }

    /// The first `TresAnimNotifyState_Attack` window `(start, end)`.
    pub fn attack_windows(&self) -> impl Iterator<Item = (f32, f32, &'static str)> + '_ {
        self.notifies.iter().filter_map(|n| match n.data {
            ClipNotifyData::Attack { attack, .. } => Some((n.start, n.end, attack)),
            _ => None,
        })
    }
}

/// One character's anim set.
#[derive(Clone, Copy, Debug)]
pub struct AnimSetTable {
    pub chara: Chara,
    pub clips: &'static [ClipDef],
}

impl AnimSetTable {
    pub fn get(&self, name: &str) -> Option<&'static ClipDef> {
        self.clips.iter().find(|c| c.name == name)
    }
}

/// Offset of the notify window after a play (animation spec 9), seconds.
const NOTIFY_OFFSET: f32 = 1.0e-4;

/// A notify dispatched by a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiredNotify {
    pub index: u16,
    pub phase: NotifyPhase,
    pub notify: &'static ClipNotify,
}

/// The montage slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NpcAnim {
    pub clip: &'static ClipDef,
    pub time: f32,
    pub weight: f32,
    blend_in: f32,
    pub prev: Option<(&'static ClipDef, f32)>,
    /// Notify states active after the last tick (bit per notify index < 64).
    active: u64,
    offset: f32,
    /// The clip reached its end (non-looping) on the last tick.
    pub finished: bool,
    /// Play-rate multiplier set by the state (1 = the asset's rate).
    pub speed: f32,
    /// Blend-space input for the drawing (X; Y of the 2D rows).
    pub axis: f32,
    pub axis_y: f32,
    /// (B4) PlayAnim mode 3 (loop) on a clip whose row does not loop: the clock wraps.
    pub force_loop: bool,
    /// (party P3) The notify states that were open when a play replaced their clip: (that clip, its open-state
    /// bits), oldest first, until the owner takes them ([`NpcAnim::take_cut`]). UE ends a notify state whose montage
    /// stops being updated (the anim instance's active-notify list loses it: NotifyEnd); without this a looping
    /// sound / persistent effect started by a state (Tornado's M_1322 AttachSound + AttachEffect, Turbo's M_1325,
    /// Discus' M_1331 ...) was never stopped once the action played its next clip.
    pub cut: [Option<(&'static ClipDef, u64)>; 4],
}

/// What one tick produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimTick {
    pub fired: Vec<FiredNotify>,
    /// Root motion of this tick in the actor's frame at the tick's start: (dx, dy, dz, dyaw).
    pub root: Option<(f32, f32, f32, f32)>,
    /// The clock wrapped (looping clip).
    pub wrapped: bool,
}

impl NpcAnim {
    pub fn new(clip: &'static ClipDef) -> NpcAnim {
        NpcAnim { clip, time: 0.0, weight: 1.0, blend_in: 0.0, prev: None, active: 0, offset: NOTIFY_OFFSET, finished: false, speed: 1.0, axis: 0.0, axis_y: 0.0, force_loop: false, cut: [None; 4] }
    }

    /// (party P3) The notify states cut by plays since the last call: (clip, open-state bits) each, and their End
    /// events (phase End, in index order) for the seam / the owner's End handling.
    pub fn take_cut(&mut self) -> Vec<(&'static ClipDef, Vec<FiredNotify>)> {
        let mut out = Vec::new();
        for slot in self.cut.iter_mut() {
            let Some((clip, bits)) = slot.take() else { continue };
            let ends = clip.notifies.iter().enumerate().take(64).filter(|(i, _)| bits & (1 << i) != 0).map(|(i, n)| FiredNotify { index: i as u16, phase: NotifyPhase::End, notify: n }).collect();
            out.push((clip, ends));
        }
        out
    }

    /// Plays `clip` from the start; `blend_in < 0` = the row's own interpolation time.
    pub fn play(&mut self, clip: &'static ClipDef, blend_in: f32) {
        // (party P3) the open notify states of the clip being replaced end (taken by the owner, `take_cut`)
        if self.active != 0 {
            let cut = (self.clip, self.active);
            match self.cut.iter_mut().find(|s| s.is_none()) {
                Some(s) => *s = Some(cut),
                None => {
                    // more than four cutting plays between two takes (the owner takes them every step; no state does
                    // that): the last slot is reused
                    self.cut[3] = Some(cut);
                }
            }
        }
        self.prev = Some((self.clip, self.time));
        self.clip = clip;
        self.time = 0.0;
        self.blend_in = if blend_in < 0.0 { clip.blend_in } else { blend_in };
        self.weight = if self.blend_in > 0.0 { 0.0 } else { 1.0 };
        self.active = 0;
        self.offset = NOTIFY_OFFSET;
        self.finished = false;
        self.speed = 1.0;
        self.force_loop = false;
    }

    /// (B4) `PlayAnim(name, blend, mode 3)`: the clip loops whatever its row says.
    pub fn play_loop(&mut self, clip: &'static ClipDef, blend_in: f32) {
        self.play(clip, blend_in);
        self.force_loop = true;
    }

    /// True when a notify state of `class` is open now.
    pub fn state_open(&self, class: &str) -> bool {
        self.clip.notifies.iter().enumerate().any(|(i, n)| i < 64 && self.active & (1 << i) != 0 && n.class == class)
    }

    /// True when a notify state whose decoded data is `State { name }` is open now.
    pub fn window_open(&self, name: &str) -> bool {
        self.clip.notifies.iter().enumerate().any(|(i, n)| i < 64 && self.active & (1 << i) != 0 && matches!(n.data, ClipNotifyData::State { name: s } if s == name))
    }

    /// The attack row whose window is open now, if any.
    pub fn open_attack(&self) -> Option<&'static str> {
        self.clip.notifies.iter().enumerate().find_map(|(i, n)| match n.data {
            ClipNotifyData::Attack { attack, .. } if i < 64 && self.active & (1 << i) != 0 => Some(attack),
            _ => None,
        })
    }

    /// The attack row and the collision group (`m_AttackCollisionGrpName`) of the open window.
    pub fn open_attack_group(&self) -> Option<(&'static str, &'static str)> {
        self.clip.notifies.iter().enumerate().find_map(|(i, n)| match n.data {
            ClipNotifyData::Attack { attack, collision } if i < 64 && self.active & (1 << i) != 0 => Some((attack, collision)),
            _ => None,
        })
    }

    /// Advances by `dt` (the pose tick inside the movement tick, as Sora's).
    pub fn tick(&mut self, dt: f32) -> AnimTick {
        let mut out = AnimTick::default();
        if self.blend_in > 0.0 {
            self.weight = (self.weight + dt / self.blend_in).min(1.0);
        } else {
            self.weight = 1.0;
        }
        let clip = self.clip;
        let len = clip.length.max(1.0e-4);
        let t0 = self.time;
        let mut t1 = t0 + dt * clip.rate * self.speed;
        let mut wrapped = false;
        if t1 >= len {
            if clip.looping || self.force_loop {
                t1 -= len * (t1 / len).floor();
                wrapped = true;
            } else {
                t1 = len;
                self.finished = true;
            }
        }
        // Notify window (t0 - offset, t1 - offset], or two windows across a wrap.
        let mut now_active: u64 = 0;
        let lo = t0 - self.offset;
        for (i, n) in clip.notifies.iter().enumerate() {
            let inside = |a: f32, b: f32| if n.is_state() { n.start <= b && n.end > a } else { n.start > a && n.start <= b };
            let hit = if wrapped { inside(lo, len) || inside(-1.0e-6, t1) } else { inside(lo, t1 - self.offset.min(t1)) || (t0 == 0.0 && n.start == 0.0 && !n.is_state()) };
            if !hit {
                continue;
            }
            let notify: &'static ClipNotify = &clip.notifies[i];
            if n.is_state() {
                if i < 64 {
                    now_active |= 1 << i;
                    if self.active & (1 << i) == 0 {
                        out.fired.push(FiredNotify { index: i as u16, phase: NotifyPhase::Begin, notify });
                    }
                }
                out.fired.push(FiredNotify { index: i as u16, phase: NotifyPhase::Tick, notify });
            } else {
                out.fired.push(FiredNotify { index: i as u16, phase: NotifyPhase::Instant, notify });
            }
        }
        for (i, n) in clip.notifies.iter().enumerate().take(64) {
            if self.active & (1 << i) != 0 && now_active & (1 << i) == 0 {
                out.fired.push(FiredNotify { index: i as u16, phase: NotifyPhase::End, notify: &clip.notifies[i] });
                let _ = n;
            }
        }
        if wrapped {
            self.offset = 0.0;
        }
        self.active = now_active;
        if clip.root_motion && !clip.root.is_empty() {
            let a = clip.root_at(t0);
            let b = if wrapped { clip.root_at(len) } else { clip.root_at(t1) };
            let (mut dx, mut dy, mut dz, mut dyaw) = (b.x - a.x, b.y - a.y, b.z - a.z, b.yaw - a.yaw);
            // Express the delta in the actor frame at t0 (the keys are in frame-0 space).
            let (s, c) = (-a.yaw).to_radians().sin_cos();
            let (rx, ry) = (dx * c - dy * s, dx * s + dy * c);
            dx = rx;
            dy = ry;
            if wrapped {
                let e = clip.root_at(t1);
                dx += e.x;
                dy += e.y;
                dz += e.z;
                dyaw += e.yaw;
            }
            out.root = Some((dx, dy, dz, dyaw));
        }
        out.wrapped = wrapped;
        self.time = t1;
        out
    }
}

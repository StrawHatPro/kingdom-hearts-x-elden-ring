//! (SPIRITSPRING, THE USER 2026-11-07: "cant you just make it that jumping at a point where torrent would normally launch
//! into the air would follow his jump height? Isn;t that a lightweight universal change or no?") THE PARTY LEADER'S NORMAL
//! JUMP AT A SPIRITSPRING RISES TO TORRENT'S HEIGHT. State note: build/sora_tmp/spiritspring/STATE.md.
//!
//! WHAT ELDEN RING DOES (READ in the data, read-only 2.7.2.0 copies; not seen in the game):
//! - A spiritspring's launch area is an MSB region of type 46 `MountJump` (a 5 m sphere, three bigger) whose type data
//!   is its JumpHeight in metres. Every one is a row of `sheets/port/kh-er-spiritsprings.json` (tools/kh_spiritsprings.js
//!   walks every map of the base game and the DLC; no row is written by hand): 52 springs (39 + 13), 8 .. 100 m.
//! - None is switched by the game: no activation part, no event script names one. A spring is live while its map is.
//! - Torrent's behaviour script (action/script/c0000.hks) asks `env(366)` (the height of the MountJump the body stands in,
//!   cm) when the ride's jump is pressed, sets `Ride_HighJump_Height = h + 700` and its rise loop
//!   (`RideJumpHighLoop_onUpdate`) ends when the body has risen that far above the take-off. Torrent's high jump
//!   (c8000 a000_006200) plays the spring's launch sound `c800007510`; it spawns no effect (no FFX event).
//!
//! WHAT WE DO (the leader on foot: Sora in any outfit, Donald, Goofy, anyone later; never while the body rides Torrent,
//! which the game handles itself, nor while the body leads):
//! 1. While the leader stands, the spring whose sphere holds the body's place (its block-local place in the row's map,
//!    gestureprompt.rs's rule) is remembered.
//! 2. HIS NORMAL JUMP leaves the floor there: for every step of the rise his core is asked to top out
//!    `JumpHeight x 100 + 700` cm above the take-off (kh-core `Sora::spring_lift` / kh-party `NpcPawn::spring_lift`: his
//!    own gravity and air control, the release cut cannot shorten it). No prompt, no new button. The rise ends at the top,
//!    on landing, on a stall (a ceiling) or when his jump becomes something else (an air attack, a glide, an air dash:
//!    KH3's own moves go on from there).
//! 3. The moment his core takes the lift (his upward speed past any KH3 jump's), the spring's own launch sound plays at
//!    the take-off, through the game's own sound play (ersound.rs): the world answering.
//! 4. The landing: the KH party takes no fall damage of any height (guards.rs `FallGuard`, `BodyFall` from the apex, the
//!    material module's "no fall damage", the no-death switch: sora.rs `fall_switches`), the same protection Elden Ring
//!    gives its spirit jump; nothing new is applied.
//! 5. The allies: when he lands at least `ALLY_MIN_UP_CM` above the take-off, every AI friend and AI Sora who was within
//!    `ALLY_RADIUS_CM` of the take-off jumps after him onto the places beside his landing (party/glue.rs, the ladder
//!    jump's own leap and slots, Torrent's wind at each take-off); the others come by their existing catch-up / warp.
//!
//! Keys: `spring.on` (1; 0 = off), `spring.log` (1). Log lines `spring:`.

use std::sync::OnceLock;

const SHEET: &str = include_str!("../../../sheets/port/kh-er-spiritsprings.json");

pub mod rules {
    use crate::gestureprompt::rules::{in_shape, local_in, parse_map, Place, Shape};
    use glam::Vec3;

    /// Torrent's rise past the spring's own height (c0000.hks: `Ride_HighJump_Height = env(366) + 700`), cm.
    pub const TORRENT_EXTRA_CM: f32 = 700.0;
    /// The lift took when his upward speed passes this (cm/s): above every KH3 jump (normal 1480, High Jump 1950, double
    /// 2300, super 2500) and under the slowest spring's (8 m + 7 m: about 3500 with Sora's rise gravity).
    pub const CONFIRM_VZ: f32 = 2600.0;
    /// The lift not taken within this long after the take-off (his core refused it: not a jump): given up, silently (s).
    pub const CONFIRM_S: f32 = 0.15;
    /// No height gained (`STALL_CM`) for this long while lifting (a ceiling): the lift ends (s, cm).
    pub const STALL_S: f32 = 0.3;
    pub const STALL_CM: f32 = 2.0;
    /// The longest lift (the 100 m spring takes about 2.3 s): a safety end (s).
    pub const MAX_S: f32 = 8.0;
    /// The allies who were this near the take-off jump after him (cm, KH3).
    pub const ALLY_RADIUS_CM: f32 = 2000.0;
    /// ... when he lands at least this much above the take-off (cm); a landing lower or level needs no jump after him.
    pub const ALLY_MIN_UP_CM: f32 = 300.0;

    /// One spiritspring of the sheet.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Spring {
        pub id: String,
        /// (area, BB, CC) of its map.
        pub map: (u8, i32, i32),
        pub entity: u32,
        pub shape: Shape,
        /// MSB metres of its map.
        pub pos: Vec3,
        pub rot_y_deg: f32,
        /// The MSB JumpHeight (m).
        pub height_m: f32,
        /// How far above the take-off the rise tops out (cm): JumpHeight x 100 + 700.
        pub launch_cm: f32,
        pub name: String,
    }

    /// (pure) The sheet's rows.
    pub fn parse(text: &str) -> Result<Vec<Spring>, String> {
        use serde_json::Value;
        let j: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let num = |v: &Value| v.as_f64().map(|n| n as f32);
        let mut out = Vec::new();
        for row in j["rows"].as_array().ok_or("no rows")? {
            let id = row[0].as_str().ok_or("a row without an id")?.to_string();
            let v = &row[1];
            let map = v["map"].as_str().and_then(parse_map).ok_or_else(|| format!("{id}: no map"))?;
            let p = |k: usize| num(&v["pos"][k]).ok_or_else(|| format!("{id}: no place"));
            let r = num(&v["dims"][0]).ok_or_else(|| format!("{id}: no size"))?;
            let shape = match v["shape"].as_str() {
                Some("sphere") => Shape::Sphere { r },
                Some("circle") => Shape::Circle { r },
                Some("cylinder") => Shape::Cylinder { r, h: num(&v["dims"][1]).unwrap_or(0.0) },
                other => return Err(format!("{id}: a shape this reader does not know: {other:?}")),
            };
            let height_m = num(&v["height_m"]).ok_or_else(|| format!("{id}: no height"))?;
            let launch_cm = num(&v["launch_cm"]).ok_or_else(|| format!("{id}: no launch height"))?;
            if (launch_cm - (height_m * 100.0 + TORRENT_EXTRA_CM)).abs() > 0.5 || height_m <= 0.0 {
                return Err(format!("{id}: launch {launch_cm} cm is not JumpHeight {height_m} m + {TORRENT_EXTRA_CM} cm"));
            }
            out.push(Spring {
                map,
                entity: v["entity"].as_u64().unwrap_or(0) as u32,
                shape,
                pos: Vec3::new(p(0)?, p(1)?, p(2)?),
                rot_y_deg: num(&v["rot"][1]).unwrap_or(0.0),
                height_m,
                launch_cm,
                name: v["msb_name"].as_str().unwrap_or("").to_string(),
                id,
            });
        }
        Ok(out)
    }

    /// (pure) The body's place is in the spring's launch area (its map: the same block, or an open-world tile of the same
    /// area; the MSB sphere).
    pub fn inside(s: &Spring, p: &Place) -> bool {
        local_in(s.map, p).is_some_and(|l| in_shape(s.shape, s.pos, s.rot_y_deg, l))
    }

    /// (pure) The spring the body stands in (the first of the sheet's order).
    pub fn find(springs: &[Spring], p: &Place) -> Option<usize> {
        springs.iter().position(|s| inside(s, p))
    }

    /// The leader as one step sees him (KH3 cm, Z up).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct LeaderIn {
        /// He drives himself on foot: not riding (the body on Torrent), not held by the body (a cutscene, an action, a
        /// ladder), alive. Anything else: no spring, a rise running ends.
        pub on_foot: bool,
        pub on_floor: bool,
        pub feet: Vec3,
        pub vz: f32,
        /// Who leads (a change of leader ends a rise).
        pub who: u8,
    }

    /// A rise running.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Rise {
        pub spring: usize,
        /// His feet on the floor he left.
        pub takeoff: Vec3,
        /// How far above the take-off he tops out (cm).
        pub height: f32,
        pub who: u8,
        pub t: f32,
        /// His core took the lift (his upward speed passed `CONFIRM_VZ`).
        pub confirmed: bool,
        /// Still asking for the lift.
        pub lifting: bool,
        pub best_z: f32,
        pub stall: f32,
    }

    /// Why a lift ended.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum End {
        Top,
        Landed,
        Stalled,
        Refused,
        TooLong,
        NotOnFoot,
        OtherLeader,
    }

    /// What one step asks of the host.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Step {
        /// Ask his core for this lift (cm above the take-off) for the coming step.
        pub lift: Option<f32>,
        /// The lift took this step (the launch sound; the log).
        pub confirmed: Option<Rise>,
        /// The lift ended this step, and why.
        pub ended: Option<(Rise, End)>,
        /// He landed after a spring jump: (the rise, where his feet landed).
        pub landed: Option<(Rise, Vec3)>,
    }

    /// THE WATCH: one per host, fed once a step before the leader's step.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Watch {
        on_floor_before: bool,
        /// The spring he stood in on his last step on the floor, and his feet there.
        standing: Option<(usize, Vec3)>,
        pub rise: Option<Rise>,
    }

    impl Watch {
        /// (pure) One step. `here`: the spring the body is in now (only read while he stands); `launch_cm`: a spring's
        /// launch height.
        pub fn step(&mut self, i: LeaderIn, here: Option<usize>, launch_cm: impl Fn(usize) -> Option<f32>, dt: f32) -> Step {
            let mut out = Step::default();
            if let Some(mut r) = self.rise.take() {
                r.t += dt;
                let end = if !i.on_foot {
                    Some(End::NotOnFoot)
                } else if i.who != r.who {
                    Some(End::OtherLeader)
                } else if i.on_floor && r.t > dt * 1.5 {
                    Some(End::Landed)
                } else {
                    None
                };
                match end {
                    Some(e) => {
                        if r.lifting || e != End::Landed {
                            out.ended = Some((r, e));
                        }
                        if e == End::Landed && r.confirmed {
                            out.landed = Some((r, i.feet));
                        }
                        r.lifting = false;
                    }
                    None if r.lifting => {
                        if i.feet.z > r.best_z + STALL_CM {
                            r.best_z = i.feet.z;
                            r.stall = 0.0;
                        } else {
                            r.stall += dt;
                        }
                        if !r.confirmed && i.vz > CONFIRM_VZ {
                            r.confirmed = true;
                            out.confirmed = Some(r);
                        }
                        let stop = if i.feet.z - r.takeoff.z >= r.height - 1.0 || (r.confirmed && i.vz <= 0.0) {
                            Some(End::Top)
                        } else if !r.confirmed && r.t > CONFIRM_S {
                            Some(End::Refused)
                        } else if r.stall > STALL_S {
                            Some(End::Stalled)
                        } else if r.t > MAX_S {
                            Some(End::TooLong)
                        } else {
                            None
                        };
                        match stop {
                            Some(e) => {
                                r.lifting = false;
                                out.ended = Some((r, e));
                            }
                            None => out.lift = Some(r.height),
                        }
                    }
                    None => {}
                }
                // kept until he lands (the allies' jump after him), a refused one dropped at once
                if end.is_none() && (r.lifting || r.confirmed) {
                    self.rise = Some(r);
                }
            } else if i.on_foot && self.on_floor_before && !i.on_floor && i.vz > 0.0 {
                // HIS JUMP LEFT THE FLOOR in a spring's launch area
                if let Some((s, takeoff)) = self.standing {
                    if let Some(height) = launch_cm(s).filter(|h| *h > 0.0) {
                        self.rise = Some(Rise { spring: s, takeoff, height, who: i.who, t: 0.0, confirmed: false, lifting: true, best_z: i.feet.z, stall: 0.0 });
                        out.lift = Some(height);
                    }
                }
            }
            if i.on_floor {
                self.standing = if i.on_foot { here.map(|s| (s, i.feet)) } else { None };
            }
            self.on_floor_before = i.on_floor;
            out
        }
    }

    /// (pure) The allies jump after him: he landed this much higher than he took off.
    pub fn allies_follow(r: &Rise, landing: Vec3) -> bool {
        r.confirmed && landing.z - r.takeoff.z >= ALLY_MIN_UP_CM
    }

}

use rules::Spring;

/// The sheet's springs (read once; a bad sheet: none, logged).
pub fn springs() -> &'static [Spring] {
    static ROWS: OnceLock<Vec<Spring>> = OnceLock::new();
    ROWS.get_or_init(|| match rules::parse(SHEET) {
        Ok(r) => r,
        Err(e) => {
            crate::erlog!("spring: sheets/port/kh-er-spiritsprings.json unreadable ({e}): no spiritsprings");
            Vec::new()
        }
    })
}

/// A spring's launch height (cm).
pub fn launch_cm(i: usize) -> Option<f32> {
    springs().get(i).map(|s| s.launch_cm)
}

/// The log's name of a spring.
pub fn describe(i: usize) -> String {
    springs().get(i).map_or_else(|| format!("#{i}"), |s| format!("{} ({}, JumpHeight {} m, entity {})", s.id, s.name.trim(), s.height_m, s.entity))
}

#[cfg(windows)]
pub mod game {
    use crate::gestureprompt::rules::Place;
    use eldenring::cs::PlayerIns;
    use glam::Vec3;

    /// The player body's place (gestureprompt.rs's `place`: its block, its block-local place = MSB metres of that block).
    pub fn place(player: &PlayerIns) -> Option<Place> {
        let b = player.current_block_id;
        let local = Vec3::new(player.block_position.x, player.block_position.y, player.block_position.z);
        let physics = crate::game::v3(&player.chr_ins.modules.physics.position);
        (local.is_finite() && physics.is_finite()).then_some(Place { block: (b.area(), b.block() as i32, b.region() as i32), overworld: b.is_overworld(), local, physics })
    }

    /// The body rides (or mounts) Torrent: Elden Ring's own spring jump; nothing of ours (SDK `CSChrRideModule`).
    pub fn riding(player: &PlayerIns) -> bool {
        let modules = &player.chr_ins.modules;
        if !crate::game::plausible(crate::word!(modules.ride)) {
            return false;
        }
        let ride = &modules.ride;
        ride.is_mounting || ride.is_mounted || (crate::game::plausible(crate::word!(ride.ride_node)) && ride.ride_node.ride_state != 0)
    }

    /// The spring the body stands in now (None: none, riding, or not readable).
    pub fn here(player: &PlayerIns) -> Option<usize> {
        if riding(player) {
            return None;
        }
        place(player).and_then(|p| super::rules::find(super::springs(), &p))
    }
}

#[cfg(test)]
mod tests {
    use super::rules::*;
    use super::*;
    use crate::gestureprompt::rules::{Place, Shape};
    use glam::Vec3;

    fn all() -> Vec<Spring> {
        parse(SHEET).expect("the sheet parses")
    }

    fn at(s: &Spring, local: Vec3) -> Place {
        Place { block: s.map, overworld: (50..89).contains(&s.map.0), local, physics: Vec3::ZERO }
    }

    #[test]
    fn spring_sheet_has_every_spring_of_base_game_and_dlc() {
        let rows = all();
        assert_eq!(rows.len(), 52, "52 springs (61 MountJump regions, the _10 map variants folded)");
        assert_eq!(rows.iter().filter(|s| s.map.0 == 61).count(), 13, "13 of the DLC");
        let mut ids: Vec<&str> = rows.iter().map(|s| s.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), rows.len(), "ids unique");
        for s in &rows {
            assert!(matches!(s.shape, Shape::Sphere { r } if r >= 5.0), "{}: a sphere of 5 m or more", s.id);
            assert!((8.0..=100.0).contains(&s.height_m), "{}: {} m", s.id, s.height_m);
            assert_eq!(s.launch_cm, s.height_m * 100.0 + TORRENT_EXTRA_CM, "{}: Torrent's launch = JumpHeight + 7 m", s.id);
        }
        // one spring read by hand from m60_39_53_00 (Mt. Gelmir): "騎乗大ジャンプ 教会3-2付近の崖下", JumpHeight 35 m
        let g = rows.iter().find(|s| s.map == (60, 39, 53) && s.entity == 11).expect("the Gelmir spring");
        assert_eq!((g.height_m, g.launch_cm), (35.0, 4200.0));
        assert_eq!(g.pos, Vec3::new(-27.993, 752.996, -96.912));
    }

    /// Spring detection: inside the sphere (centre, near the rim), outside (past the rim, above, in another map, in the
    /// same area's neighbouring tile at the same local place).
    #[test]
    fn spring_detection_inside_outside_and_other_maps() {
        let rows = all();
        for (k, s) in rows.iter().enumerate() {
            let Shape::Sphere { r } = s.shape else { unreachable!() };
            assert_eq!(find(&rows, &at(s, s.pos)), Some(k), "{}", s.id);
            assert!(inside(s, &at(s, s.pos)), "{}: the centre", s.id);
            assert!(inside(s, &at(s, s.pos + Vec3::new(r - 0.1, 0.0, 0.0))), "{}: just inside", s.id);
            assert!(!inside(s, &at(s, s.pos + Vec3::new(r + 0.1, 0.0, 0.0))), "{}: just outside", s.id);
            assert!(!inside(s, &at(s, s.pos + Vec3::new(0.0, r + 0.5, 0.0))), "{}: above the sphere", s.id);
            let mut other = at(s, s.pos);
            other.block = (s.map.0.wrapping_add(1), s.map.1, s.map.2);
            assert!(!inside(s, &other), "{}: another map", s.id);
        }
        // an open-world spring seen from the neighbouring tile: its local place there is 256 m off
        let s = rows.iter().find(|s| s.map == (60, 39, 53)).unwrap();
        let mut p = at(s, s.pos - Vec3::new(256.0, 0.0, 0.0));
        p.block = (60, 40, 53);
        assert!(inside(s, &p), "the same spot named from the next tile east");
    }

    const FLOOR: f32 = 1000.0;

    fn lead(on_floor: bool, z: f32, vz: f32) -> LeaderIn {
        LeaderIn { on_foot: true, on_floor, feet: Vec3::new(0.0, 0.0, z), vz, who: 0 }
    }

    /// A jump of a given gravity and power run through the watch with a core that obeys the lift exactly as kh-core's
    /// `Cmc::lift_to` (midpoint steps); returns (the highest point above the take-off, the steps, the sound frames).
    fn fly(w: &mut Watch, here: Option<usize>, launch: f32, gravity: f32, power: f32, takes_lift: bool) -> (f32, Vec<Step>) {
        let dt = 1.0 / 60.0;
        let mut steps = Vec::new();
        // standing two steps in the spring
        for _ in 0..2 {
            steps.push(w.step(lead(true, FLOOR, 0.0), here, |_| Some(launch), dt));
        }
        // the take-off: his own jump's power
        let (mut z, mut vz) = (FLOOR, power);
        let mut top = z;
        let mut lift: Option<f32> = None;
        for _ in 0..1200 {
            // the core's step: the lift asked last (lift_to), then gravity (midpoint)
            if let (Some(h), true) = (lift, takes_lift) {
                let missing = FLOOR + h - z;
                if missing > 0.0 && vz > 0.0 {
                    vz = vz.max((2.0 * gravity * (missing + gravity * dt * dt / 8.0)).sqrt());
                }
            }
            let v0 = vz;
            vz -= gravity * dt;
            z += (v0 + vz) * 0.5 * dt;
            let landed = z <= FLOOR;
            if landed {
                z = FLOOR;
                vz = 0.0;
            }
            top = top.max(z);
            let s = w.step(lead(landed, z, vz), None, |_| Some(launch), dt);
            lift = s.lift;
            steps.push(s);
            if landed {
                break;
            }
        }
        (top - FLOOR, steps)
    }

    /// The jump reaches Torrent's peak for every spring and every leader's gravity (Sora's rise gravity 4116 cm/s^2; the
    /// friends' pawns run the same movement with their own scales: two others), within 1 cm.
    #[test]
    fn spring_jump_reaches_torrents_peak_for_each_leader() {
        for s in all() {
            for (who, gravity, power) in [("Sora", 4116.0, 1480.0), ("Donald", 3500.0, 1300.0), ("Goofy", 4800.0, 1500.0)] {
                let mut w = Watch::default();
                let (top, steps) = fly(&mut w, Some(0), s.launch_cm, gravity, power, true);
                assert!((top - s.launch_cm).abs() <= 1.0 && top >= s.launch_cm - 0.01, "{} {who}: peak {top:.2} cm, Torrent's {:.0} cm", s.id, s.launch_cm);
                assert_eq!(steps.iter().filter(|x| x.confirmed.is_some()).count(), 1, "{} {who}: one launch sound", s.id);
                assert!(steps.iter().any(|x| matches!(x.ended, Some((_, End::Top)))), "{} {who}: the lift ends at the top", s.id);
            }
        }
    }

    /// Outside a spring (or in a spring whose height the sheet does not give): the plain KH3 jump, no lift, no sound.
    #[test]
    fn spring_outside_or_disabled_gives_the_plain_jump() {
        let mut w = Watch::default();
        let (top, steps) = fly(&mut w, None, 4200.0, 4116.0, 1480.0, true);
        assert!(steps.iter().all(|x| x.lift.is_none() && x.confirmed.is_none()), "no spring: nothing asked");
        assert!((top - 1480.0 * 1480.0 / (2.0 * 4116.0)).abs() < 5.0, "the plain jump: {top}");
        let mut w = Watch::default();
        assert!(w.step(lead(true, FLOOR, 0.0), Some(3), |_| None, 1.0 / 60.0).lift.is_none());
        assert!(w.step(lead(false, FLOOR + 20.0, 1480.0), None, |_| None, 1.0 / 60.0).lift.is_none(), "a spring without a height: no lift");
        // riding / held by the body (not on foot): never
        let mut w = Watch::default();
        let off = |on_floor, z, vz| LeaderIn { on_foot: false, ..lead(on_floor, z, vz) };
        w.step(off(true, FLOOR, 0.0), Some(0), |_| Some(4200.0), 1.0 / 60.0);
        assert!(w.step(off(false, FLOOR + 20.0, 1480.0), None, |_| Some(4200.0), 1.0 / 60.0).lift.is_none(), "riding: Elden Ring's own spring");
    }

    /// A core that refuses the lift (not a jump: a launch hit, a ladder leap): given up within `CONFIRM_S`, no sound.
    #[test]
    fn spring_lift_refused_by_the_core_is_given_up_silently() {
        let mut w = Watch::default();
        let (_, steps) = fly(&mut w, Some(0), 4200.0, 4116.0, 1480.0, false);
        assert!(steps.iter().all(|x| x.confirmed.is_none()), "no sound");
        assert!(steps.iter().any(|x| matches!(x.ended, Some((_, End::Refused)))));
        let asked = steps.iter().filter(|x| x.lift.is_some()).count();
        assert!(asked <= (CONFIRM_S * 60.0) as usize + 2, "asked {asked} steps");
    }

    /// The landing: the allies jump after him when he lands higher; the game's fall damage on the body is the KH party's
    /// rule's (guards.rs `BodyFall` measures from the apex: the spring's whole drop is in its window).
    #[test]
    fn spring_landing_is_protected_and_allies_follow_up_the_cliff() {
        let mut w = Watch::default();
        let dt = 1.0 / 60.0;
        w.step(lead(true, FLOOR, 0.0), Some(0), |_| Some(4200.0), dt);
        w.step(lead(true, FLOOR, 0.0), Some(0), |_| Some(4200.0), dt);
        w.step(lead(false, FLOOR + 20.0, 1480.0), None, |_| Some(4200.0), dt);
        let s = w.step(lead(false, FLOOR + 100.0, 5000.0), None, |_| Some(4200.0), dt);
        assert!(s.confirmed.is_some());
        w.step(lead(false, FLOOR + 4200.0, -10.0), None, |_| Some(4200.0), dt);
        // he glides onto the cliff 30 m up
        let s = w.step(lead(true, FLOOR + 3000.0, 0.0), None, |_| Some(4200.0), dt);
        let (r, landing) = s.landed.expect("landed after the spring");
        assert!(allies_follow(&r, landing));
        // a level landing: nobody needs to jump after him
        let level = Rise { takeoff: Vec3::ZERO, ..r };
        assert!(!allies_follow(&level, Vec3::new(0.0, 900.0, 100.0)));
        // the body's own fall from the apex (42 m up) onto the cliff (30 m up): 12 m, the fall window (ER metres, Y up)
        let mut body = crate::guards::BodyFall::default();
        body.frame(true, Vec3::new(0.0, 10.0, 0.0), false, dt);
        body.frame(false, Vec3::new(0.0, 52.0, 0.0), false, dt);
        body.frame(false, Vec3::new(0.0, 45.0, 0.0), false, dt);
        assert!(body.frame(true, Vec3::new(0.0, 40.0, 0.0), false, dt), "the landing opens the window");
        assert!(body.in_window(), "what the game takes then is its fall damage: discarded");
    }

    /// A ceiling over the spring: the lift ends on the stall.
    #[test]
    fn spring_lift_ends_at_a_ceiling() {
        let mut w = Watch::default();
        let dt = 1.0 / 60.0;
        w.step(lead(true, FLOOR, 0.0), Some(0), |_| Some(4200.0), dt);
        w.step(lead(false, FLOOR + 20.0, 1480.0), None, |_| Some(4200.0), dt);
        w.step(lead(false, FLOOR + 120.0, 5000.0), None, |_| Some(4200.0), dt);
        let mut ended = None;
        for _ in 0..60 {
            let s = w.step(lead(false, FLOOR + 300.0, 4000.0), None, |_| Some(4200.0), dt);
            if let Some((_, e)) = s.ended {
                ended = Some(e);
                break;
            }
        }
        assert_eq!(ended, Some(End::Stalled));
    }

    /// The real cores: Sora (kh-core) and a pad-driven friend (kh-party) jumping with the lift asked every step reach the
    /// spring's height within 1 cm, and with no lift asked their jump is the plain one.
    #[test]
    fn spring_lift_on_the_real_sora_core() {
        use kh_core::test_world::TestWorld;
        use kh_core::{Input, PadButton, PadButtons, PadState, Pose, Sora, SORA_PARAMS};
        let world = TestWorld::floor();
        for height in [1500.0f32, 4200.0, 10700.0] {
            for lifted in [false, true] {
                let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
                let dt = 1.0 / 60.0;
                for _ in 0..30 {
                    sora.step(dt, &Input::default(), &world);
                }
                let floor = sora.output().position.z;
                let mut before = PadButtons::NONE;
                let (mut top, mut left) = (floor, false);
                for k in 0..900 {
                    let mut held = PadButtons::NONE;
                    held.set(PadButton::JUMP, k < 3); // a tap: the release cut would shorten a plain rise
                    let input = Input::default().with_pad(PadState::from_held(held, before));
                    before = held;
                    if lifted {
                        sora.spring_lift(height);
                    }
                    let out = sora.step(dt, &input, &world);
                    top = top.max(out.position.z);
                    left |= !out.on_floor;
                    if left && out.on_floor {
                        break;
                    }
                }
                let risen = top - floor;
                if lifted {
                    assert!((risen - height).abs() <= 1.0, "Sora lifted to {height}: risen {risen:.2}");
                } else {
                    assert!(risen < 300.0, "the plain tap jump: {risen:.1}");
                }
            }
        }
    }
}

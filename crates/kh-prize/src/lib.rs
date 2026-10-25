//! KINGDOM HEARTS III'S PRIZES: the orbs an enemy bursts into (HP, MP, munny, focus) and the item
//! boxes, as `ATresPrizeMan` throws them and `ATresPrizeBase` lives them. docs/sora-prize-spec.md
//! is the text; the numbers are rows of sheets/port/sora-prizes.json ([`params::P`]).
//!
//! Engine-free (glam + std + kh-core for the `World` trait and the status types). Units are the
//! core's: centimetres, seconds, degrees; X forward, Y right, Z up.
//!
//! A host owns one [`Prizes`]. It
//! 1. asks for drops ([`Prizes::drop_table`] with an enemy's table, or [`Prizes::spawn`]),
//! 2. calls [`Prizes::step`] once per simulation step with its `World` and the [`Getter`] (Sora),
//! 3. acts on [`Prizes::take_events`]: draws / moves / removes the orb effects, plays the bounce
//!    and get sounds, and applies a collected prize ([`host::apply_to_sora`] does the last three
//!    through the core's own seams).
//!
//! Deterministic: the only randomness is the crate's own generator ([`Rng`], the game's
//! xorshift128), seeded by the host.

pub mod host;
pub mod params;

use glam::Vec3;
use kh_core::status::PrizeKind;
use kh_core::World;
use params::P;

/// The game's generator (0x142CFF460 is its state; every draw is the inlined xorshift128 step and
/// `(w >> 8) x 2^-24`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng([u32; 4]);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        // Marsaglia's constants with the seed folded into the first word (a seed of 0 is fine).
        Rng([123_456_789 ^ seed, 362_436_069, 521_288_629, 88_675_123 ^ seed.rotate_left(16)])
    }

    pub fn next_u32(&mut self) -> u32 {
        let t = self.0[0] ^ (self.0[0] << 11);
        self.0[0] = self.0[1];
        self.0[1] = self.0[2];
        let w = self.0[3];
        self.0[2] = w;
        let next = ((w >> 11) ^ t) >> 8 ^ w ^ t;
        self.0[3] = next;
        next
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }
}

/// The four item boxes (`ATresPrizeBox`, `PRIZE_KIND` 13 .. 16).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BoxKind {
    Heal = 13,
    Material = 14,
    Foodstuff = 15,
    Rare = 16,
}

impl BoxKind {
    /// (loot) THE BOX OF AN ITEM, by the high byte of its item id (= `ETresItemCategory`; item box spawn
    /// 0x141473110): 1 battle item and 2 camp item -> the heal box, 7 material -> the material box,
    /// 8 foodstuff -> the foodstuff box, EVERYTHING ELSE (3 weapon, 4 armour, 5 accessory, 9 key item ...)
    /// -> the rare box. `None`: item id 0 (the function makes nothing).
    pub fn of_item(item: u32) -> Option<BoxKind> {
        if item == 0 {
            return None;
        }
        Some(match (item >> 8) & 0xFF {
            1 | 2 => BoxKind::Heal,
            7 => BoxKind::Material,
            8 => BoxKind::Foodstuff,
            _ => BoxKind::Rare,
        })
    }
}

/// What an orb is: one of the nine prizes Sora's `ApplyPrize` knows, or an item box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrizeClass {
    Orb(PrizeKind),
    /// With the item id it holds (the game's item ids: 0x101 Potion ...).
    Box(BoxKind, u32),
}

impl PrizeClass {
    /// `PRIZE_KIND` (0 .. 8, 13 .. 16).
    pub fn kind_value(self) -> u8 {
        match self {
            PrizeClass::Orb(kind) => kind as u8,
            PrizeClass::Box(kind, _) => kind as u8,
        }
    }

    /// The radius of its sphere (`TresRoot` shape of its blueprint).
    pub fn radius(self) -> f32 {
        match self {
            PrizeClass::Orb(kind) => P.kind_radius[kind as usize],
            PrizeClass::Box(..) => P.box_radius,
        }
    }

    /// What an entry of an enemy's table means (`ETresDropItemID`, queue tick 0x141489120): 1 .. 9
    /// the orbs, 10 .. 19 and 22 the heal items, 23 .. 82 materials, 83 .. 141 foodstuff. `None`:
    /// nothing a port of Sora spawns (0, the crab prizes 20 / 21, the light prizes 142 / 143).
    pub fn from_drop_id(id: u8) -> Option<PrizeClass> {
        use PrizeKind::*;
        Some(match id {
            1 => PrizeClass::Orb(HpSmall),
            2 => PrizeClass::Orb(HpLarge),
            3 => PrizeClass::Orb(MpSmall),
            4 => PrizeClass::Orb(MpLarge),
            5 => PrizeClass::Orb(FocusSmall),
            6 => PrizeClass::Orb(FocusLarge),
            7 => PrizeClass::Orb(Munny1),
            8 => PrizeClass::Orb(Munny5),
            9 => PrizeClass::Orb(Munny10),
            10..=19 => PrizeClass::Box(BoxKind::Heal, 0x101 + (id as u32 - 10)),
            22 => PrizeClass::Box(BoxKind::Heal, 0x10B),
            23..=82 => PrizeClass::Box(BoxKind::Material, id as u32 + 0x6EA),
            83..=141 => PrizeClass::Box(BoxKind::Foodstuff, id as u32 + 0x7AE),
            _ => return None,
        })
    }
}

/// The prize abilities that change a prize WHEN IT IS SPAWNED (0x141472DC0; the party-wide count
/// of each, here: has it or not).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Converters {
    /// HP_CONVERTER (79), MP_CONVERTER (80), FP_CONVERTER (83): small -> large.
    pub hp: bool,
    pub mp: bool,
    pub focus: bool,
    /// MILLIONAIRE (176, "Rags to Riches"): everything becomes munny.
    pub millionaire: bool,
    /// MUNNY_CONVERTER (81): munny one size up.
    pub munny: bool,
}

impl Converters {
    /// In the function's own order: the three small -> large converters, then Rags to Riches,
    /// then the munny converter (so a small HP prize with HP Converter and Rags to Riches is a
    /// 5-munny orb, and with the Munny Converter on top a 10-munny one).
    pub fn convert(&self, kind: PrizeKind) -> PrizeKind {
        use PrizeKind::*;
        let mut kind = kind;
        if self.hp && kind == HpSmall {
            kind = HpLarge;
        }
        if self.mp && kind == MpSmall {
            kind = MpLarge;
        }
        if self.focus && kind == FocusSmall {
            kind = FocusLarge;
        }
        if self.millionaire {
            kind = match kind {
                HpSmall | MpSmall | FocusSmall => Munny1,
                HpLarge | MpLarge | Munny1 | FocusLarge => Munny5,
                Munny5 | Munny10 => Munny10,
            };
        }
        if self.munny {
            kind = match kind {
                Munny1 => Munny5,
                Munny5 => Munny10,
                other => other,
            };
        }
        kind
    }
}

/// What the party's abilities do to a drop (table spawn 0x141480420, spawn 0x141472DC0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpawnMods {
    /// A party member has PRIZE_FEEVER (175): `count += (count + 1) / 2`.
    pub prize_fever: bool,
    /// The summed LUCK_UP (39) count: item rates x (1 + 0.3 n).
    pub luck_up: u32,
    pub converters: Converters,
}

/// An enemy's drop table (`FTresChrBaseParamDataTable`: `m_DropPrize1` .. `m_DropItemRate3`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DropTable {
    /// (`ETresDropItemID`, count): both are always dropped.
    pub prizes: [(u8, u8); 2],
    /// (`ETresDropItemID`, rate in percent): each rolled by itself.
    pub items: [(u8, u8); 3],
    /// Bit 0 of the packed table's flag byte: ONE roll picks at most one of the three items.
    pub one_item: bool,
}

impl DropTable {
    /// From a sheet row `[drop id, count, drop id, count]`.
    pub fn of_row(row: [f32; 4]) -> DropTable {
        DropTable { prizes: [(row[0] as u8, row[1] as u8), (row[2] as u8, row[3] as u8)], ..DropTable::default() }
    }
}

/// Who collects: the nearest party pawn that can take a prize, as the host sees him this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Getter {
    /// The point an orb flies to (the getter's virtual +0x6E0; a host gives his capsule centre).
    pub point: Vec3,
    /// HP above 0 and not in the die state (0x14142C600).
    pub can_take: bool,
    /// The attraction radius, cm; `None` = no limit (Master Treasure Magnet).
    pub radius: Option<f32>,
}

impl Getter {
    /// Nobody: orbs lie until they vanish.
    pub const NONE: Getter = Getter { point: Vec3::ZERO, can_take: false, radius: Some(0.0) };

    /// Sora as the core has him: his capsule centre, alive, the radius of his Treasure Magnets
    /// (table 0x1469AB6D8 through `kh_core::abilities::passive::prize_draw_radius`).
    pub fn sora(sora: &kh_core::Sora, capsule_center: Vec3) -> Getter {
        let status = sora.status();
        Getter { point: capsule_center, can_take: status.hp > 0, radius: kh_core::abilities::passive::prize_draw_radius(status) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Thrown, bouncing, lying (the game's "Appear" state).
    Appear,
    /// Flying to the getter ("Get").
    Get,
}

/// One orb.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prize {
    /// Counts up from 1 per `Prizes`.
    pub id: u32,
    pub class: PrizeClass,
    pub position: Vec3,
    pub velocity: Vec3,
    pub state: State,
    /// 1 while it lies; the Get flight shrinks it to 0 (`SetActorScale3D(timer x 1.5)`).
    pub scale: f32,
    /// (loot) Degrees about the vertical: a box turns up to 30 on its way to the getter (0x14147C0A0).
    pub yaw: f32,
    /// What a munny orb is worth when not 0 (a host scaling; the game's orbs are worth 1 / 5 / 10).
    /// (loot) For a box: how many of its item it holds when above 1 (a host's lot count; KH3's boxes hold one).
    pub amount: i32,
    /// Seconds since it was made (prize+0x43C).
    pub age: f32,
    /// Bounces so far (prize+0x430).
    pub bounces: u32,
    /// `m_Timer`: the appear time, then the living time, then the Get flight.
    timer: f32,
    living: f32,
    /// prize+0x414 bit 0: the appear time is over, it can be collected.
    collectible: bool,
    /// prize+0x43A: no distance limit (the hit that killed was a magic hit with MAGIC_DRAW).
    magic_draw: bool,
    gravity_scale: f32,
    bounciness: f32,
    damping: f32,
    simulating: bool,
    in_water: bool,
    get_sound_done: bool,
    /// Get: `m_vDir`, `m_fLen`; for a box also where it started and its flight time.
    dir: Vec3,
    len: f32,
    start: Vec3,
    fly_time: f32,
    /// (party P2) `m_pGetter` (prize+0x418): the slot of the getter list FindGetter chose (0 with one getter).
    pub getter: u8,
}

impl Prize {
    /// It lies still (gravity off after the fourth bounce, or stopped by the engine's threshold).
    pub fn resting(&self) -> bool {
        self.state == State::Appear && (!self.simulating || self.gravity_scale == 0.0)
    }

    pub fn collectible(&self) -> bool {
        self.collectible
    }

    /// Seconds until it vanishes; `None` while it appears or flies, or when it never does.
    pub fn time_left(&self) -> Option<f32> {
        (self.state == State::Appear && self.collectible && self.living >= 0.0).then_some(self.timer)
    }
}

/// What happened in a step, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrizeEvent {
    Spawned { id: u32, class: PrizeClass, position: Vec3 },
    /// It hit the level. `sound`: one of the first four bounces (`m_SoundBound`).
    Bounced { id: u32, class: PrizeClass, position: Vec3, count: u32, sound: bool },
    /// The Get flight began. (party P2) `getter`: the slot of the getter list it flies to.
    GetStarted { id: u32, class: PrizeClass, getter: u8 },
    /// 0.2 s before it arrives: `m_SoundGet`, at the orb.
    GetSound { id: u32, class: PrizeClass, position: Vec3 },
    /// It arrived: apply it (`ApplyPrize`) and remove its effect. `amount` as `Prize::amount`. (party P2) `getter`: the
    /// slot of the getter list that got it.
    Collected { id: u32, class: PrizeClass, amount: i32, position: Vec3, getter: u8 },
    /// Its living time ran out: remove its effect. `amount` as `Prize::amount` (what is lost with it).
    Expired { id: u32, class: PrizeClass, amount: i32, position: Vec3 },
}

/// The prize manager and its orbs.
#[derive(Clone, Debug)]
pub struct Prizes {
    list: Vec<Prize>,
    events: Vec<PrizeEvent>,
    rng: Rng,
    next_id: u32,
}

/// UE's `MaxSimulationIterations` of a projectile movement component.
const MAX_BOUNCES_PER_STEP: usize = 8;
/// The engine's pull-back from a blocking hit, cm.
const SKIN: f32 = 0.125;

impl Prizes {
    pub fn new(seed: u32) -> Prizes {
        Prizes { list: Vec::new(), events: Vec::new(), rng: Rng::new(seed), next_id: 1 }
    }

    pub fn list(&self) -> &[Prize] {
        &self.list
    }

    pub fn get(&self, id: u32) -> Option<&Prize> {
        self.list.iter().find(|p| p.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The events since the last call.
    pub fn take_events(&mut self) -> Vec<PrizeEvent> {
        std::mem::take(&mut self.events)
    }

    /// (loot) What a prize carries (`Prize::amount`): a munny orb's worth, the count of a box's item. False: no such prize.
    pub fn set_amount(&mut self, id: u32, amount: i32) -> bool {
        match self.list.iter_mut().find(|p| p.id == id) {
            Some(p) => {
                p.amount = amount;
                true
            }
            None => false,
        }
    }

    /// Every orb gone, without events (a level reset).
    pub fn clear(&mut self) {
        self.list.clear();
        self.events.clear();
    }

    /// The velocity of one orb of a table drop (queue tick 0x141489120 with the default block):
    /// pitch, then yaw, then (with a spread) the speed are drawn in that order.
    pub fn throw_velocity(&mut self) -> Vec3 {
        let pitch = (P.spawn_pitch[0] + self.rng.unit() * (P.spawn_pitch[1] - P.spawn_pitch[0])).to_radians();
        let yaw = (P.spawn_yaw[0] + self.rng.unit() * (P.spawn_yaw[1] - P.spawn_yaw[0])).to_radians();
        let mut speed = P.spawn_speed;
        if P.spawn_speed_rand > 0.0 {
            speed = (speed + self.rng.unit() * 2.0 * P.spawn_speed_rand - P.spawn_speed_rand).max(0.0);
        }
        Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin()) * speed
    }

    /// One prize, thrown with `velocity` (`ATresPrizeBase` init, slot 241). An orb is converted
    /// by `mods` first. `amount`: see `Prize::amount`. Returns its id.
    pub fn spawn(&mut self, at: Vec3, class: PrizeClass, velocity: Vec3, mods: &SpawnMods, magic_draw: bool, amount: i32) -> u32 {
        let class = match class {
            PrizeClass::Orb(kind) => PrizeClass::Orb(mods.converters.convert(kind)),
            other => other,
        };
        let is_box = matches!(class, PrizeClass::Box(..));
        let living = if is_box { P.life_box_living_time } else { P.life_living_time };
        let id = self.next_id;
        self.next_id += 1;
        self.list.push(Prize {
            id,
            class,
            position: at,
            velocity,
            state: State::Appear,
            scale: 1.0,
            yaw: 0.0,
            amount,
            age: 0.0,
            bounces: 0,
            // A box can be collected from its first frame (0x141473590): its timer is its life at once.
            timer: if is_box { living } else { P.life_appear_time },
            living,
            collectible: is_box,
            magic_draw,
            gravity_scale: P.move_gravity_scale,
            bounciness: P.move_bounciness,
            // (loot) a box is initialised with 0.98, an orb with the request block's 0.955 (0x141473110)
            damping: if is_box { P.box_damping } else { P.move_damping }.clamp(0.0, 1.0),
            simulating: true,
            in_water: false,
            get_sound_done: false,
            dir: Vec3::ZERO,
            len: 0.0,
            start: at,
            fly_time: 0.0,
            getter: 0,
        });
        self.events.push(PrizeEvent::Spawned { id, class, position: at });
        id
    }

    /// `count` orbs of one kind, each thrown like a table drop's. `amount_each`: `Prize::amount`.
    pub fn drop_orbs(&mut self, at: Vec3, kind: PrizeKind, count: u32, amount_each: &[i32], mods: &SpawnMods, magic_draw: bool) {
        let count = if mods.prize_fever { count + ((count + 1) as f32 * P.table_prize_fever) as u32 } else { count };
        for k in 0..count {
            let velocity = self.throw_velocity();
            self.spawn(at, PrizeClass::Orb(kind), velocity, mods, magic_draw, amount_each.get(k as usize).copied().unwrap_or(0));
        }
    }

    /// An item box (0x141473110): X and Y each `(rand - rand) x rand x 300`, Z `600 + rand x 600`.
    pub fn drop_box(&mut self, at: Vec3, kind: BoxKind, item: u32, magic_draw: bool) -> u32 {
        let mut axis = || (self.rng.unit() - self.rng.unit()) * self.rng.unit() * P.box_speed_xy;
        let (x, y) = (axis(), axis());
        let z = P.box_speed_z[0] + self.rng.unit() * (P.box_speed_z[1] - P.box_speed_z[0]);
        self.spawn(at, PrizeClass::Box(kind, item), Vec3::new(x, y, z), &SpawnMods::default(), magic_draw, 0)
    }

    /// (loot) THE BOX OF ONE ITEM by its game item id (0x101 Potion, 0x701 Blazing Shard, 0x404 Defense Belt ...): the
    /// box kind is the item's ([`BoxKind::of_item`]). `None`: item 0.
    pub fn drop_item_box(&mut self, at: Vec3, item: u32, magic_draw: bool) -> Option<u32> {
        BoxKind::of_item(item).map(|kind| self.drop_box(at, kind, item, magic_draw))
    }

    /// (loot) KH3'S ITEM ROLL ON GAME ITEM IDS: every (item id, rate in percent) is rolled BY ITSELF,
    /// `(int)(rand x 99 + 1) <= rate` (table spawn 0x141480420), so none, one or all may drop; with LUCK_UP the
    /// rate is `round(rate x (1 + 0.3 x copies))`. A host whose table names item ids instead of
    /// `ETresDropItemID` values uses this (the test level; an enemy row's own three slots go through
    /// [`Prizes::drop_table`]). Returns the ids of the boxes made.
    pub fn drop_items(&mut self, at: Vec3, items: &[(u32, u32)], luck_up: u32, magic_draw: bool) -> Vec<u32> {
        let mut made = Vec::new();
        for &(item, rate) in items {
            if item == 0 || rate == 0 {
                continue;
            }
            let roll = (self.rng.unit() * P.table_item_roll as f32 + 1.0) as u32;
            if roll <= lucky_rate(rate, luck_up) {
                made.extend(self.drop_item_box(at, item, magic_draw));
            }
        }
        made
    }

    /// AN ENEMY DIED: its table at `at` (0x1405A6220 -> `ATresPrizeMan` 0x141480420, then the queue
    /// tick 0x141489120; the queue's per-tick budget is not modelled: everything appears at once).
    /// `magic_draw`: the killing hit was a magic hit of a Sora with MAGIC_DRAW (the orbs then come
    /// from any distance). Returns how many prizes were made.
    pub fn drop_table(&mut self, at: Vec3, table: &DropTable, mods: &SpawnMods, magic_draw: bool) -> usize {
        let before = self.list.len();
        for (id, count) in table.prizes {
            if id == 0 || count == 0 {
                continue;
            }
            match PrizeClass::from_drop_id(id) {
                Some(PrizeClass::Orb(kind)) => self.drop_orbs(at, kind, count as u32, &[], mods, magic_draw),
                Some(PrizeClass::Box(kind, item)) => {
                    for _ in 0..count {
                        self.drop_box(at, kind, item, magic_draw);
                    }
                }
                None => {}
            }
        }
        // Item rates: x (1 + 0.3 x LUCK_UP), rounded, for heal items, materials and foodstuff.
        let lucky = |id: u8, rate: u8| -> u32 {
            let item = matches!(id, 10..=22 | 23..=82 | 83..=141);
            if mods.luck_up > 0 && item {
                (rate as f32 * (1.0 + P.table_luck_up * mods.luck_up as f32)).round() as u32
            } else {
                rate as u32
            }
        };
        if table.one_item {
            // One roll over the three rates (at least 100 wide) picks at most one.
            let rates: Vec<u32> = table.items.iter().map(|&(id, rate)| if id == 0 || rate == 0 { 0 } else { lucky(id, rate).min(255) }).collect();
            let total = rates.iter().sum::<u32>().max(100);
            let mut roll = ((total as f32 - 1.0) * self.rng.unit() + 1.0) as i32;
            for (k, &(id, _)) in table.items.iter().enumerate() {
                if rates[k] == 0 {
                    continue;
                }
                if roll <= rates[k] as i32 {
                    self.drop_item(at, id, magic_draw);
                    break;
                }
                roll -= rates[k] as i32;
            }
        } else {
            for (id, rate) in table.items {
                if id == 0 || rate == 0 {
                    continue;
                }
                let roll = (self.rng.unit() * P.table_item_roll as f32 + 1.0) as u32;
                if roll <= lucky(id, rate) {
                    self.drop_item(at, id, magic_draw);
                }
            }
        }
        self.list.len() - before
    }

    fn drop_item(&mut self, at: Vec3, id: u8, magic_draw: bool) {
        match PrizeClass::from_drop_id(id) {
            Some(PrizeClass::Box(kind, item)) => {
                self.drop_box(at, kind, item, magic_draw);
            }
            Some(PrizeClass::Orb(kind)) => self.drop_orbs(at, kind, 1, &[], &SpawnMods::default(), magic_draw),
            None => {}
        }
    }

    /// ONE STEP of every orb: the state's tick, then the movement component's.
    /// (boss finish) `UTresGameBlueprintLibrary::AllPrizeCollection(player)` (0x14221E440 -> 0x140E469B0, the
    /// boss maps' scripts call it when the white-out is full): every prize on the field flies to the
    /// player. The native was not read past its name: here every orb that still appears becomes
    /// collectible now and loses its distance limit, so the next step starts its Get flight.
    pub fn collect_all(&mut self) -> usize {
        let mut count = 0;
        for p in self.list.iter_mut().filter(|p| p.state == State::Appear) {
            (p.collectible, p.magic_draw) = (true, true);
            count += 1;
        }
        count
    }

    pub fn step(&mut self, dt: f32, world: &dyn World, getter: &Getter) {
        self.step_party(dt, world, std::slice::from_ref(getter), &|_, _| true);
    }

    /// (party P2) ONE STEP WITH THE PARTY AS GETTERS: FindGetter 0x1414794C0 over the player and his party slots
    /// (player pawn vt+0xF10(i), i = 0 .. 6): among those that can take the prize (`takes(slot, class)`: the pawn's
    /// status component vt+0x728, the friends' 0x141259A50) and are inside their OWN radius (each pawn's PRIZE_DRAW
    /// count, `Getter::radius`), the NEAREST to the orb (squared distance; the first of equals). The orb then flies to
    /// that one (`Prize::getter`, the game's `m_pGetter`) and follows him; the events name the slot.
    pub fn step_party(&mut self, dt: f32, world: &dyn World, getters: &[Getter], takes: &dyn Fn(usize, PrizeClass) -> bool) {
        let mut k = 0;
        while k < self.list.len() {
            let mut prize = self.list[k];
            let keep = step_one(&mut prize, dt, world, getters, takes, &mut self.events);
            if keep {
                self.list[k] = prize;
                k += 1;
            } else {
                self.list.remove(k);
            }
        }
    }
}

/// False: the orb is gone.
fn step_one(p: &mut Prize, dt: f32, world: &dyn World, getters: &[Getter], takes: &dyn Fn(usize, PrizeClass) -> bool, events: &mut Vec<PrizeEvent>) -> bool {
    p.age += dt;
    // (party P2) in flight its getter is the one FindGetter chose
    let getter = &getters.get(p.getter as usize).copied().unwrap_or(Getter::NONE);
    match p.state {
        State::Appear => {
            // Appear tick 0x14147B760.
            let damp = p.damping.powf(dt * 60.0);
            p.velocity.x *= damp;
            p.velocity.y *= damp;
            if !p.in_water {
                if let Some(surface) = world.water_surface(p.position) {
                    if surface - p.position.z > 0.0 {
                        p.gravity_scale *= 0.1;
                        p.velocity.z = if p.velocity.z <= 0.0 { p.velocity.z * 0.1 } else { 0.0 };
                        p.bounciness *= 0.7;
                        p.in_water = true;
                    }
                }
            }
            p.timer -= dt;
            if !p.collectible {
                if p.timer <= 0.0 {
                    p.timer = p.living;
                    p.collectible = true;
                }
            } else if p.living >= 0.0 && p.timer <= 0.0 {
                // The Disappear state's tick removes it at once (0x14147BC70 -> 0x141479360).
                events.push(PrizeEvent::Expired { id: p.id, class: p.class, amount: p.amount, position: p.position });
                return false;
            }
            // (party P2) FindGetter over the getter slots (the nearest that takes it, inside his radius)
            let found = if p.collectible { find_getter(p, getters, takes) } else { None };
            if let Some(slot) = found {
                p.getter = slot as u8;
                let getter = &getters[slot];
                // Appear end (0x141478DD0): gravity and velocity off. Get begin.
                p.gravity_scale = 0.0;
                p.velocity = Vec3::ZERO;
                p.state = State::Get;
                let offset = p.position - getter.point;
                p.len = offset.length();
                p.dir = offset.normalize_or_zero();
                p.start = p.position;
                match p.class {
                    // 0x1414736D0.
                    PrizeClass::Orb(_) => p.timer = P.get_time,
                    // 0x1414738C0.
                    PrizeClass::Box(..) => {
                        p.fly_time = (p.len * P.box_fly_time[0]).clamp(P.box_fly_time[1], P.box_fly_time[2]);
                        p.timer = p.fly_time;
                        // (loot) THE ARC: it sets off toward the getter with 600 cm added to the height of the
                        // way there, so a box lying beside him first rises.
                        p.dir = (getter.point - p.position + Vec3::Z * P.box_arc[0]).normalize_or_zero();
                    }
                }
                events.push(PrizeEvent::GetStarted { id: p.id, class: p.class, getter: p.getter });
                return true;
            }
            move_orb(p, dt, world, events);
            true
        }
        State::Get => {
            p.timer -= dt;
            // (loot) 0x14147C0A0: a box whose direction has collapsed arrives at once.
            if matches!(p.class, PrizeClass::Box(..)) && p.dir.length() < 0.9 {
                p.timer = 0.0;
            }
            if p.timer <= P.get_sound_at && !p.get_sound_done {
                p.get_sound_done = true;
                events.push(PrizeEvent::GetSound { id: p.id, class: p.class, position: p.position });
            }
            if p.timer <= 0.0 {
                events.push(PrizeEvent::Collected { id: p.id, class: p.class, amount: p.amount, position: getter.point, getter: p.getter });
                return false;
            }
            match p.class {
                // Get tick 0x14147BC90: a shrinking spiral about the vertical through the getter.
                PrizeClass::Orb(_) => {
                    let s = p.timer * P.get_rate;
                    let radius = p.len * s;
                    let turn = if radius > 1.0e-4 { (P.get_turn_ref / radius * P.get_turn_gain).min(P.get_turn_max) } else { P.get_turn_max };
                    let (sin, cos) = (turn * dt).to_radians().sin_cos();
                    p.dir = Vec3::new(p.dir.x * cos - p.dir.y * sin, p.dir.x * sin + p.dir.y * cos, p.dir.z);
                    p.position = getter.point + p.dir * radius;
                    p.scale = s.clamp(0.0, 1.0);
                }
                // (loot) Box Get tick 0x14147C0A0: A HOMING ARC. With p the sine-eased progress and r = 1 - p:
                // the direction turns toward the getter by the fraction (1 + 15 p) x dt of the angle between
                // them (a slerp), then the box moves along it at (distance left) / r per second: slow and
                // upward at first, then curving in and closing ever faster; it turns 30 degrees about the
                // vertical on the way and shrinks over the last tenth.
                PrizeClass::Box(..) => {
                    let eased = ((1.0 - p.timer / p.fly_time.max(1.0e-4)) * std::f32::consts::FRAC_PI_2).sin();
                    let rest = if eased > 1.0 { 0.0 } else if eased <= 0.0 { 1.0 } else { 1.0 - eased };
                    let to = getter.point - p.position;
                    let distance = to.length();
                    if distance > 0.1 && rest > 0.01 {
                        let speed = distance / rest;
                        let blend = (((1.0 - rest) * P.box_arc[2] + P.box_arc[1]) * dt).clamp(0.0, 1.0);
                        let turn = glam::Quat::from_rotation_arc(p.dir.normalize_or_zero(), to / distance);
                        p.dir = glam::Quat::IDENTITY.slerp(turn, blend) * p.dir;
                        p.position += p.dir * (speed * dt);
                    }
                    p.yaw = (1.0 - rest) * P.box_spin;
                    p.scale = if rest < P.box_shrink[0] { rest * P.box_shrink[1] } else { 1.0 };
                }
            }
            true
        }
    }
}

/// (loot) An item rate with LUCK_UP (table spawn 0x141480420): `round(rate x (1 + 0.3 x copies))`.
pub fn lucky_rate(rate: u32, luck_up: u32) -> u32 {
    if luck_up == 0 {
        rate
    } else {
        (rate as f32 * (1.0 + P.table_luck_up * luck_up as f32)).round() as u32
    }
}

/// (loot) The chance that one item slot of rate `rate` percent drops: the roll `(int)(rand x 99 + 1)` is uniform
/// over 1 ..= 99, so a rate of r drops with r / 99 (a rate of 99 or more always).
pub fn item_chance(rate: u32, luck_up: u32) -> f32 {
    (lucky_rate(rate, luck_up) as f32 / P.table_item_roll as f32).min(1.0)
}

/// (party P2) FindGetter 0x1414794C0 over the slots: the nearest that can take it and is inside his radius (the first
/// of equals).
fn find_getter(p: &Prize, getters: &[Getter], takes: &dyn Fn(usize, PrizeClass) -> bool) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, g) in getters.iter().enumerate() {
        if takes(i, p.class) && wants(p, g) {
            let d = p.position.distance_squared(g.point);
            if best.is_none_or(|b| d < b.1) {
                best = Some((i, d));
            }
        }
    }
    best.map(|b| b.0)
}

/// FindGetter 0x1414794C0 for one candidate.
fn wants(p: &Prize, getter: &Getter) -> bool {
    if !getter.can_take {
        return false;
    }
    match (p.magic_draw, getter.radius) {
        (true, _) | (_, None) => true,
        (false, Some(radius)) => p.position.distance_squared(getter.point) <= radius * radius,
    }
}

/// Stock UE 4.17 `UProjectileMovementComponent::TickComponent` with the prize's settings, and the
/// prize's bounce handler 0x14147EDF0.
fn move_orb(p: &mut Prize, dt: f32, world: &dyn World, events: &mut Vec<PrizeEvent>) {
    if !p.simulating {
        return;
    }
    let radius = p.class.radius();
    let gravity = Vec3::Z * (P.move_world_gravity_z * p.gravity_scale);
    let mut remaining = dt;
    let mut iterations = 0;
    while remaining > 1.0e-6 && iterations < MAX_BOUNCES_PER_STEP && p.simulating {
        iterations += 1;
        let old = p.velocity;
        let new = old + gravity * remaining;
        let delta = old * remaining + (new - old) * (0.5 * remaining);
        if delta.length_squared() < 1.0e-10 {
            p.velocity = new;
            break;
        }
        let hit = world.sweep_capsule(p.position, p.position + delta, radius, radius);
        let Some(hit) = hit else {
            p.position += delta;
            p.velocity = new;
            break;
        };
        // Up to the contact, less the engine's skin; out of an overlap along its normal.
        if hit.start_penetrating {
            p.position += hit.normal * (hit.penetration_depth + SKIN);
        } else {
            let length = delta.length();
            let travelled = (length * hit.time - SKIN).max(0.0);
            p.position += delta / length * travelled;
        }
        p.velocity = if hit.time > 1.0e-4 { old + gravity * (remaining * hit.time) } else { old };
        // ComputeBounceResult: the part along the normal is reversed and scaled; the rest is kept
        // (friction 0).
        let normal = hit.normal;
        let along = p.velocity.dot(normal);
        if along <= 0.0 {
            let into = normal * -along;
            let tangent = (p.velocity + into) * (1.0 - P.move_friction).clamp(0.0, 1.0);
            p.velocity = tangent + into * p.bounciness.max(0.0);
        }
        // The prize's bounce handler.
        let sound = (p.bounces as i32) < P.bounce_sound_max;
        p.bounces += 1;
        events.push(PrizeEvent::Bounced { id: p.id, class: p.class, position: p.position, count: p.bounces, sound });
        if p.bounces as i32 >= P.rest_min_bounces && p.age >= P.rest_min_age && hit.impact_normal.z > P.rest_floor_normal_z {
            p.gravity_scale = 0.0;
            p.velocity = Vec3::ZERO;
        }
        if p.velocity.length_squared() < P.move_stop_speed * P.move_stop_speed {
            p.velocity = Vec3::ZERO;
            p.simulating = false;
        }
        remaining *= 1.0 - hit.time.clamp(0.0, 1.0);
    }
}

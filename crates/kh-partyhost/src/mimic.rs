//! (MIMIC SORA, THE USER 2026-11-02: "Mimic Tear needs to mimic Sora, the mimic Sora can just use Sora's party member AI and
//! uses his current equipment (mimic sora is unable to use Summon).") WHAT MIMIC SORA TAKES FROM SORA, engine-free: the copy of
//! his kit at the moment of the summon, the core it is put into, the Mimic Tear's upgrade table. kh-er's spirit_mimic.rs is the
//! host side (the body, the draw, the AI's input); his look is kh-er-render's mimic.rs (Elden Ring's own rows).
//!
//! BUILT (THE USER 2026-11-03: "build mimic Sora"; UNTESTED IN THE GAME until a log says otherwise). The four pieces:
//! 1. HIS AI: AI Sora's own (lead.rs `MimicAi` over `SoraAi`, whose step is now `drive` with any leader: a `Lead` and the
//!    trees' world), on its own tree controller ("n_ex002", actor `MIMIC_ACTOR`), following WHOEVER LEADS: the player's Sora
//!    (the trees' "player", `SoraAiWorld`) or the friend leader (`LeaderAiWorld`); his Summon rules shut every step;
//! 2. HIS SIMULATION in kh-er (spirit_mimic.rs): this core stepped in the fixed step after the party's, against the shared
//!    ErWorld and the enemies (ErTargets), his hits x `upgrade.damage` fired by his own body; his pose by his own Animator of
//!    Sora's pack and his own keychain;
//! 3. HIS BODY: a fourth key of the ally body pool (`AllyWant::mimic`, allybody.rs `Action::SpawnMimic`): AI Sora's body route
//!    on the spirit team; the hits on it reach this core (/ `upgrade.defense`);
//! 4. HIS DRAW: kh-er-render `mimic::submit_mimic`: a second instance of the worn model (THE USER 2026-11-03: "the same outfit
//!    that Sora is currently using") whose G-buffer draw carries the Mimic Tear's surface (er_char.wgsl `gb_main` KH_MIMIC,
//!    WetAspectParam 18), his KH colour draw skipped: Elden Ring's own light passes shade him.
//!
//! THE RULE, as Elden Ring's Mimic Tear copies the Tarnished (its BuddyParam row 20700001 names `npcParamId` 100000010, the
//! c0000 copy of the player): ONCE, AT THE SUMMON, never a live link afterwards (a later change of Sora's equipment does not
//! reach a mimic that is out). [`copy_kit`] lists what is copied and what is not:
//! - COPIED: his level and EXP, Strength / Magic / Defense and their boosts, max HP / MP / focus, the Keyblade and its upgrade,
//!   his armour and accessories, every ability with what is equipped of it, the magic he knows, his magic slots (with the
//!   hand-chosen tiers), the shortcut pages, the shotlock choice, the item slots and the items in them (his own copy: what the
//!   mimic uses is never taken from Sora's pouch), the command options, the shotlock / air step unlocks, the difficulty and the
//!   host's spell cost factor, the port options (`Tuning`);
//! - NOT COPIED: the gauges as they are now (the mimic arrives with full HP, MP and focus, no MP Charge, as a spirit arrives
//!   whole), a form he is in (the copy is in his normal form: a form's worn abilities are left out; (MIMIC-BOSS-KIT) a form
//!   kept in another Keyblade slot too: Elden Ring's Mimic Tear copies equipment, not what is active; but the copy may
//!   form-change BY ITSELF, its own gauge and offers, and switch Keyblades: lead.rs `full_kit_of`), a meal and the x5
//!   Strength buff (short-term effects; the Mimic Tear copies equipment, not buffs), the status immunity and the damage
//!   flags of the moment, his munny, the equipped SPIRIT ASHES and anything of Summon (the copy's [`summon_view`] has
//!   `allowed = false`: no Summon row, the user's rule), the command menu's position, his queue and his team attacks;
//! - ADDED: the Mimic Tear's own upgrade bonus ([`upgrade`]: Elden Ring's `dopingSpEffect_lv0..10` of row 20700001).
//!
//! WHO IS COPIED WHEN DONALD OR GOOFY LEADS (the coordinator's question). Decided: ALWAYS SORA (the one Sora core of the party,
//! the leader's or AI Sora's), because (1) the user's words are "mimic Sora" and Summon is a command of Sora's kit, (2) the
//! Summon row is only on the menu of the player's own Sora (`SummonView::allowed`: AI Sora and Mimic Sora have none), so a
//! summon is always made while Sora leads and the copy is of the leader Sora at that moment; (3) when the lead passes to a
//! friend while the mimic is out it stays as it is (its copy, its timer) and follows whoever leads, as the friends and the
//! spirits do; (4) a Donald or Goofy copy would need a second friend in kh-party's party, which this module cannot make.

use kh_core::command::summon::SummonView;
use kh_core::command::{CommandState, MenuState};
use kh_core::magic::items::Pouch;
use kh_core::{Pose, Sora, Status, Tuning, SORA_PARAMS};

/// The Mimic Tear's BuddyParam row (regulation 2.7.2.0; goods 207000's trigger SpEffect 207000 names it).
pub const MIMIC_BUDDY_ROW: u32 = 20_700_001;
/// Its first upgrade SpEffect (`dopingSpEffect_lv0`; +N = this + N, up to 290210).
pub const MIMIC_DOPING_BASE: u32 = 290_200;

/// What one upgrade level gives (an Elden Ring SpEffect row's rates): `hp` x max HP (`maxHpRate`), `defense` the damage taken
/// divided by it (`physicsDiffenceRate`, as `magicDiffenceRate` and the other elements), `damage` x the damage dealt to enemies
/// (`atkEnemyDmgCorrectRate_Physics`, as the other elements').
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Upgrade {
    pub hp: f32,
    pub defense: f32,
    pub damage: f32,
}

/// THE MIMIC TEAR'S UPGRADE TABLE, +0 .. +10: SpEffect 290200 .. 290210 (regulation 2.7.2.0, read with reg2.js; the other ashes'
/// rows are 290000 .. 290010, a different table). `physicsAttackPowerRate` is 1 on every row: the bonus is in the damage
/// correction. Every element's rate equals the physical one on these rows.
pub const UPGRADES: [Upgrade; 11] = [
    Upgrade { hp: 1.0, defense: 1.0, damage: 1.0 },
    Upgrade { hp: 1.141, defense: 1.013, damage: 1.0967 },
    Upgrade { hp: 1.281, defense: 1.026, damage: 1.2022 },
    Upgrade { hp: 1.656, defense: 1.039, damage: 1.4945 },
    Upgrade { hp: 1.813, defense: 1.053, damage: 1.4945 },
    Upgrade { hp: 1.953, defense: 1.066, damage: 1.6901 },
    Upgrade { hp: 2.266, defense: 1.079, damage: 1.7582 },
    Upgrade { hp: 2.406, defense: 1.093, damage: 1.8308 },
    Upgrade { hp: 2.688, defense: 1.106, damage: 2.0 },
    Upgrade { hp: 3.15, defense: 1.119, damage: 2.2791 },
    Upgrade { hp: 3.3, defense: 1.133, damage: 2.4725 },
];

/// The bonus of an ash level (above +10: +10's).
pub fn upgrade(level: u8) -> Upgrade {
    UPGRADES[(level as usize).min(UPGRADES.len() - 1)]
}

/// What the copy's core shows the Summon rules: never allowed (THE USER: "mimic sora is unable to use Summon"), so the core
/// shows no Summon row and refuses the command kinds.
pub fn summon_view() -> SummonView {
    SummonView { allowed: false, ..SummonView::default() }
}

/// SORA'S KIT AS THE MIMIC TAKES IT (see the module's lists).
#[derive(Clone, Debug, PartialEq)]
pub struct MimicKit {
    pub status: Status,
    pub command: CommandState,
    pub pouch: Pouch,
    pub tuning: Tuning,
    pub shotlock_unlocked: bool,
    pub airstep_unlocked: bool,
    /// The ash's upgrade level and its bonus (already in `status`'s max HP; `defense` / `damage` are the host's to apply on
    /// the mimic's hits).
    pub level: u8,
    pub upgrade: Upgrade,
    /// Sora was in a form at the summon (the copy is not): for the log.
    pub form_left_out: bool,
    /// (THE USER 2026-11-03: "mimic Sora also needs to use the same outfit that Sora is currently using") the outfit Sora
    /// wore at the summon (kh-er boc.rs's id, "" = KH3's own): set by the host, for the log (the drawing is the worn one's).
    pub outfit: String,
}

/// THE COPY of `sora` (the party's one Sora core, normally the leader at the summon) for a Mimic Tear of upgrade `level`.
pub fn copy_kit(sora: &Sora, level: u8) -> MimicKit {
    let up = upgrade(level);
    let mut status = sora.status().clone();
    // a form's worn abilities, a meal and the buff are not his equipment
    let form_left_out = status.abilities.any_form();
    status.abilities.set_form(&[]);
    // (MIMIC-BOSS-KIT) ... and a form waiting in another Keyblade slot (P2: a form is kept in its Keyblade's slot, resumed by a
    // switch): the slots, the current one and the levels rebuilt from the save's keys, which keep no stored form (KH3's run-time
    // store). His OWN forms stay his: his gauge, the formchange offers and the switch are the copy's own (lead.rs full kit)
    let k = status.keyblades();
    let (slots, levels) = (k.slot_keys(), k.levels_by_key());
    let (keyblades, _) = kh_core::status::Keyblades::from_keys(&slots, k.current_slot(), &levels);
    status.set_keyblades(keyblades);
    status.meal = None;
    status.abilities.set_meal(None);
    status.buff_level = 0;
    // the moment's flags and gauges are not copied: he arrives whole
    status.status_immunity = 0.0;
    status.in_damage_state = false;
    status.damage_invalid = false;
    status.invincible = false;
    status.hp_limit_one = false;
    status.combo_leave_timer = 0.0;
    status.mp_charging = false;
    status.mp_charge_timer = 0.0;
    status.focus_empty = false;
    status.item_effects = [0; 12];
    status.munny = 0;
    status.notes.clear();
    // the upgrade's max HP (Elden Ring `maxHpRate`): on the base, which only a level change recomputes (the mimic earns none)
    if !status.placeholder {
        let max = status.max_hp();
        status.hp_base += ((max as f32 * up.hp).round() as i32 - max).max(0);
        status.refresh();
        status.hp = status.hp_max;
        status.mp = status.mp_max;
        status.focus = status.focus_max;
    }
    let mut command = sora.command().clone();
    command.state = MenuState::MainSel;
    (command.main_cursor, command.sub_cursor, command.list) = (0, 0, 0);
    (command.lock_timer, command.reaction_block) = (0.0, 0.0);
    (command.pending, command.last_executed, command.last_shoot_flow, command.executed) = (0, 0, 0, 0);
    command.slow_applied = false;
    command.requests.clear();
    command.events.clear();
    // THE USER: "mimic sora is unable to use Summon": no ashes, no Summon row
    command.spirit_ashes = [0; kh_core::command::summon::SLOTS];
    command.summon = summon_view();
    command.calling = false;
    let magic = sora.magic();
    MimicKit { status, command, pouch: magic.pouch.clone(), tuning: sora.tuning(), shotlock_unlocked: magic.shotlock_unlocked, airstep_unlocked: magic.airstep_unlocked, level: level.min(10), upgrade: up, form_left_out, outfit: String::new() }
}

/// MIMIC SORA'S CORE: a new Sora at `pose` with the kit (his own simulation; nothing is shared with the leader's core).
pub fn make_core(kit: &MimicKit, pose: Pose) -> Sora {
    let mut core = Sora::new(SORA_PARAMS, pose).with_tuning(kit.tuning).with_status(kit.status.clone());
    *core.command_mut() = kit.command.clone();
    let magic = core.magic_mut();
    magic.pouch = kit.pouch.clone();
    magic.shotlock_unlocked = kit.shotlock_unlocked;
    magic.airstep_unlocked = kit.airstep_unlocked;
    // (HUD-REVIVE consistency, MIMIC-BOSS) a party member's KO from his first step (a copy KO'd before its AI's first step
    // never takes the leader's Die); the lying KO (M_0230), never AI Sora's game-over hover
    core.combat_mut().friend.sora_is_ai = true;
    core.combat_mut().friend.ko_game_over_pose = false;
    core
}

/// The host keeps the copy's Summon rules shut every frame (a host that fills `SummonView` for every core must give this one
/// [`summon_view`]); true when it had to.
pub fn keep_summon_shut(core: &mut Sora) -> bool {
    let c = core.command_mut();
    let open = c.summon.allowed || c.spirit_ashes.iter().any(|a| *a != 0) || c.calling;
    if open {
        c.summon = summon_view();
        c.spirit_ashes = [0; kh_core::command::summon::SLOTS];
        c.calling = false;
    }
    open
}

/// One line for the log: what the copy took.
pub fn describe(kit: &MimicKit) -> String {
    let s = &kit.status;
    format!(
        "Mimic Sora +{}: level {}, HP {} (x{:.3}), MP {}, Str {} Mag {} Def {}, Keyblade {} +{}, armour [{}], accessories [{}], {} magic slots, items in {} slots, outfit {}; damage x{:.3}, damage taken / {:.3}{}; Summon: none",
        kit.level,
        s.level,
        s.hp_max,
        kit.upgrade.hp,
        s.mp_max,
        s.stats.strength,
        s.stats.magic,
        s.stats.defense,
        s.weapon,
        s.weapon_upgrade,
        s.armour.join(", "),
        s.accessories.join(", "),
        kit.command.magic_slots.iter().filter(|k| **k != 0).count(),
        kit.pouch.slots.iter().filter(|p| p.exists).count(),
        if kit.outfit.is_empty() { "KH3's own" } else { kit.outfit.as_str() },
        kit.upgrade.damage,
        kit.upgrade.defense,
        if kit.form_left_out { " (Sora's form left out: the copy is in his normal form)" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::status::LoadoutValue;
    use kh_core::{Loadout, Vec3};

    fn level99() -> Status {
        let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
        let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim_start_matches('\u{feff}')).unwrap();
        let mut loadout = Loadout::new("sora-loadout-99");
        for row in json["rows"].as_array().unwrap() {
            let v = match &row[1] {
                serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
                serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
                serde_json::Value::Array(a) => LoadoutValue::List(a.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
                serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
                _ => continue,
            };
            loadout.set(row[0].as_str().unwrap(), &v).unwrap();
        }
        loadout.resolve().unwrap();
        Status::from_loadout(&loadout)
    }

    /// The player's Sora mid-fight: hurt, low on MP, ashes equipped, Summon allowed, a Potion's effect counted.
    fn leader() -> Sora {
        let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(level99());
        sora.command_mut().fill(&level99());
        let st = sora.status_mut();
        st.hp = 7;
        st.mp = 3;
        st.munny = 12345;
        st.item_effects[0] = 4;
        st.buff_level = 2;
        let c = sora.command_mut();
        c.spirit_ashes = [207000, 200000, 0, 0];
        c.summon.allowed = true;
        c.summon.bell = true;
        c.main_cursor = 3;
        sora.magic_mut().shotlock_unlocked = true;
        sora
    }

    #[test]
    fn mimic_copies_his_equipment_stats_and_magic_once() {
        let lead = leader();
        let kit = copy_kit(&lead, 0);
        let (a, b) = (lead.status(), &kit.status);
        assert!(!b.placeholder);
        assert_eq!((b.level, b.exp, b.weapon.as_str(), b.weapon_upgrade), (a.level, a.exp, a.weapon.as_str(), a.weapon_upgrade), "level and Keyblade");
        assert_eq!((&b.armour, &b.accessories, &b.magic, &b.items), (&a.armour, &a.accessories, &a.magic, &a.items), "armour, accessories, magic, items");
        assert_eq!(b.abilities.count(kh_core::enums::ETresAbilityKind::MpHaste), a.abilities.count(kh_core::enums::ETresAbilityKind::MpHaste), "abilities");
        assert_eq!((b.stats.strength, b.stats.magic, b.stats.defense), (a.stats.strength, a.stats.magic, a.stats.defense));
        assert_eq!(kit.command.magic_slots, lead.command().magic_slots);
        assert_eq!(kit.command.shortcuts, lead.command().shortcuts);
        assert_eq!(kit.pouch, lead.magic().pouch, "his items: a copy of the pouch");
        assert!(kit.shotlock_unlocked);
        // whole, not as he is now; nothing of the moment
        assert_eq!((b.hp, b.mp, b.focus), (b.hp_max, b.mp_max, b.focus_max), "full gauges");
        assert_eq!(b.hp_max, a.hp_max, "+0: his own max HP");
        assert_eq!((b.munny, b.buff_level, b.item_effects[0]), (0, 0, 0));
        assert_eq!(kit.command.main_cursor, 0);
        // THE USER: "mimic sora is unable to use Summon"
        assert_eq!(kit.command.spirit_ashes, [0; 4]);
        assert!(!kit.command.summon.allowed && !kit.command.summon.bell);
        let mut core = make_core(&kit, Pose { position: Vec3::new(100.0, 0.0, 0.0), yaw: 90.0 });
        assert_eq!(core.status().hp, b.hp_max);
        assert_eq!(core.command().magic_slots, lead.command().magic_slots);
        assert!(!core.command().summon.allowed);
        let (text, grey) = kh_core::command::summon::main_row(&core.command().summon, core.status());
        assert_eq!((text, grey), (kh_core::command::summon::TEXT_SUMMON, Some(kh_core::command::summon::Grey::NotAllowed)), "no Summon for the copy");
        // a host that opened it is put right
        core.command_mut().summon.allowed = true;
        assert!(keep_summon_shut(&mut core));
        assert!(!core.command().summon.allowed && !keep_summon_shut(&mut core));
        assert!(describe(&kit).contains("Summon: none"));
    }

    #[test]
    fn mimic_the_copy_is_not_a_live_link() {
        let mut lead = leader();
        let kit = copy_kit(&lead, 0);
        let core = make_core(&kit, Pose { position: Vec3::ZERO, yaw: 0.0 });
        lead.status_mut().weapon = "WEP_KEYBLADE_SO_01".to_string();
        lead.command_mut().magic_slots = [0; 6];
        assert_ne!(core.status().weapon, lead.status().weapon, "a change after the summon does not reach the mimic");
        assert_ne!(core.command().magic_slots, [0; 6]);
    }

    #[test]
    fn mimic_a_form_is_left_out_and_the_upgrade_raises_his_hp() {
        let mut lead = leader();
        lead.status_mut().abilities.set_form(&[(kh_core::enums::ETresAbilityKind::MpHaste, 1)]);
        let base = lead.status().max_hp();
        let kit = copy_kit(&lead, 10);
        assert!(kit.form_left_out && !kit.status.abilities.any_form(), "his normal form");
        assert_eq!(kit.status.hp_max, (base as f32 * 3.3).round() as i32, "+10: maxHpRate 3.3 (SpEffect 290210)");
        assert_eq!(kit.status.hp, kit.status.hp_max);
        assert_eq!(upgrade(10), Upgrade { hp: 3.3, defense: 1.133, damage: 2.4725 });
        assert_eq!(upgrade(99), upgrade(10));
        assert_eq!(upgrade(0), Upgrade { hp: 1.0, defense: 1.0, damage: 1.0 });
        assert!(UPGRADES.windows(2).all(|w| w[1].hp >= w[0].hp && w[1].damage >= w[0].damage && w[1].defense >= w[0].defense), "never weaker with a level");
        assert_eq!((MIMIC_BUDDY_ROW, MIMIC_DOPING_BASE), (20_700_001, 290_200));
    }

    // ---- (MIMIC SORA) his AI: AI Sora's, following the player's Sora (crate::lead::MimicAi)

    struct NoEnemies;

    impl kh_core::Targets for NoEnemies {
        fn enumerate(&self, _out: &mut Vec<kh_core::host::TargetInfo>) {}
        fn info(&self, _id: kh_core::TargetId) -> Option<kh_core::host::TargetInfo> {
            None
        }
        fn hurt_shapes(&self, _id: kh_core::TargetId, _out: &mut Vec<kh_core::host::HurtShape>) {}
        fn apply_hit(&mut self, _hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
            kh_core::host::HitResult::default()
        }
        // (AUDIT-B) no bones here: a rider attached to a hit follows the target's origin and yaw
        fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
            None
        }
        fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
            None
        }
    }

    impl crate::PartyTargets for NoEnemies {
        fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
        fn attacked_by_enemies(&self) -> bool {
            false
        }
    }

    fn runtime(ai: bool) -> crate::PartyRuntime {
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party/ai");
        let ai_pack = if ai { kh_ai::AiPack::load_dir(&pack).ok().map(std::sync::Arc::new) } else { None };
        crate::PartyRuntime::new(crate::RuntimeConfig { wanted: false, test: crate::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack, ai_pack_source: None })
    }

    fn camera() -> kh_camera::CameraView {
        kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false }
    }

    /// Steps the player's Sora (`input`) and Mimic Sora (his AI's input) `n` fixed steps on a floor; Mimic Sora's distance to
    /// the player's Sora after each step.
    fn run(rt: &crate::PartyRuntime, ai: &mut crate::MimicAi, sora: &mut Sora, mimic: &mut Sora, input: &kh_core::Input, n: usize) -> Vec<f32> {
        let world = kh_core::test_world::TestWorld::floor();
        let cam = camera();
        let mut out = Vec::new();
        for k in 0..n {
            sora.step(1.0 / 60.0, input, &world);
            let i = ai.input(rt, mimic, crate::MimicScene { sora, lock_on: kh_core::TargetId::NONE, targets: &NoEnemies, camera: &cam, camera_yaw: 0.0, nav: None, dt: 1.0 / 60.0, now: k as f64 / 60.0 });
            assert!(!i.pad.pressed.has(kh_core::PadButton::FaceTop) || mimic.combat().situation.count() > 0, "no Y of his without a situation command");
            mimic.step(1.0 / 60.0, &i, &world);
            assert!(!mimic.command().summon.allowed && mimic.command().spirit_ashes == [0; 4], "THE USER: no Summon for Mimic Sora");
            out.push((mimic.output().position - sora.output().position).truncate().length());
        }
        out
    }

    #[test]
    fn mimic_ai_warps_to_the_player_sora_when_far_and_unseen_and_keeps_summon_shut() {
        let rt = runtime(false);
        assert!(rt.leader.is_none(), "the player's Sora leads");
        let mut ai = crate::MimicAi::new(&rt);
        let lead = leader();
        let kit = copy_kit(&lead, 3);
        let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(lead.status().clone());
        // 40 m behind the camera (never seen): the friends' warp rule puts him behind his leader after a second
        let mut mimic = make_core(&kit, Pose { position: Vec3::new(-4000.0, 0.0, 0.0), yaw: 0.0 });
        mimic.command_mut().summon.allowed = true; // a host that opened it: shut at his next step
        let d = run(&rt, &mut ai, &mut sora, &mut mimic, &kh_core::Input::default(), 90);
        assert!(d[0] > 3000.0, "far at first: {}", d[0]);
        assert!(*d.last().unwrap() < 400.0, "warped behind the player's Sora: {:?}", &d[d.len() - 5..]);
        assert_eq!(mimic.status().hp, mimic.status().hp_max, "a warp is no hit");
    }

    #[test]
    fn mimic_ai_follows_the_player_sora_with_ai_soras_tree() {
        let rt = runtime(true);
        if rt.ai_pack.is_none() {
            eprintln!("no AI pack at build/sora_pack/party/ai: skipped");
            return;
        }
        let mut ai = crate::MimicAi::new(&rt);
        assert!(ai.has_tree());
        let lead = leader();
        let kit = copy_kit(&lead, 0);
        let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(1500.0, 0.0, 0.0), yaw: 0.0 }).with_status(lead.status().clone());
        // 15 m from the player's Sora, in view: no warp, the tree's follow walks him in
        let mut mimic = make_core(&kit, Pose { position: Vec3::ZERO, yaw: 0.0 });
        let d = run(&rt, &mut ai, &mut sora, &mut mimic, &kh_core::Input::default(), 360);
        let closest = d.iter().copied().fold(f32::MAX, f32::min);
        assert!(closest < 900.0, "he follows his leader (closest {closest:.0} cm of 1500)");
        assert!(d.iter().all(|x| *x > 30.0), "never on top of him");
    }
}

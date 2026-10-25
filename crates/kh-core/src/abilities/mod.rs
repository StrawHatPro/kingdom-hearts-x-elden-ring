//! OWNER: the abilities implementers (docs/sora-stats-spec.md section 5 for the ability table and
//! the hooks, docs/sora-actions-spec.md for the action abilities).
//!
//! THE QUERY every system uses: `cx.status.has(kind)` / `cx.status.count(kind)` (`status/mod.rs`:
//! the player pawn's `HasAbility` / `AbilityCount` with the script gates of spec 5.2), with `kind`
//! an [`crate::enums::ETresAbilityKind`]. WHICH abilities are owned, equipped, worn or eaten is
//! data (`status::AbilitySet`).
//!
//! WHAT IS IMPLEMENTED (2026-10-11, the stats implementer): the PASSIVE abilities whose effect is
//! purely numeric and fully read in the stats spec. Most of them live where their number is
//! used, in `status/`:
//!
//! | abilities | where |
//! |---|---|
//! | COMBO_UP, AIRCOMBO_UP, MAGIC_COMBO_UP, COUNTER_UP, CRITICAL_COUNTER (x2), FIRE_UP .. AERO_UP, HEARTLESS / NOBODY_BUSTER, FULLMP_BURST, JOIN_FORCE, CRITICAL_HALF | `status::compute_damage` |
//! | LAST_LEAVE, COMBO_LEAVE, DAMAGE_ASPIR, FIRE_ASPIR .. DARK_ASPIR | `Status::apply_damage_to_sora` |
//! | DEFENDER, CHARGE_BERSERK (Strength part), POWER_LUNCH | the stat getters |
//! | MP_HASTE / HASTERA / HASTEGA, MP_SAVE, MAGIC_COMBO_SAVE, MP_SAFETY, MP_LEAVE, WIZZARD_STAR | `Status::mp_charge_rate`, `effective_mp_cost`, `can_cast`, `use_mp` |
//! | WALK_HEALING, WALK_REGENE | `Status::tick` |
//! | ITEM_UP | `Status::item_multiplier` |
//! | EXPZERO, EXP_BARGAIN | `Status::adjust_exp` |
//! | BURN / CLOUD / SNEEZE / FREEZE / DISCHARGE / STUN_GUARD | `Status::resists_bad_status` |
//! | COMBO_PLUS, AIRCOMBO_PLUS | `Status::max_combo` |
//!
//! and the rest, which are numbers handed to another system, in `passive.rs` here: prize
//! conversion and the magnet radius, situation-command timers, the link timer, the attraction
//! marker chance, the Cure Converter gauge, Block Replenisher's MP, and the count tables of
//! Superslide / Air Slide / Focus Syphon for the states that use them.
//!
//! EVERYTHING ELSE (the "partial" and "hook only" rows of the catalogue: request gates, attack
//! replacements, finishers, counters, HUD switches) belongs to the actions / combat / magic / UI
//! implementers: the ownership table is in `build/sora_tmp/IMPL_STATUS_STATE.md`. No effect of
//! an ability may be invented: write it from its sheet row (`sheets/port/sora-abilities`).
//! An ability that is a whole action of its own (air slide, dodge roll ...) is a state:
//! `states/<name>.rs`.

pub mod passive;
/// (XSI, 2026-10-16) The 16 abilities of XS_SORA_ABILITIES.md (our design on KH3's clips, rows and rules).
pub mod xs;

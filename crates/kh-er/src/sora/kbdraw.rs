//! (P5 WEAPON DRAW, documentation/KH3_WEAPONS_PLAN.md 1.3) THE HOST'S SIDE OF THE EQUIPPED KEYBLADE: which keyblade the
//! renderer draws in his hand (kh-er-render keyblade.rs `set_equipped`: the keyblade of his active slot, the other two
//! slotted ones loaded with it), its own keychain (its SQEX_BonamikAsset on its own skeleton, simulated as the Kingdom
//! Key's: sora.rs `step_chain`), its sound sets (fx.rs: the swing / hit sets of its model, `Routes::select_weapon`), its
//! trail and effect sockets (the effect side: `EffectHost::set_keyblade`), and KH3's switch timing: after a D-pad switch
//! (kh-core `status.keyblade_switches`) the new keyblade is shown `appear_visible_delay()` (0.22 s) later, its appear
//! effect (spawned by the core) playing meanwhile.
//!
//! Test aids (TEST PROFILE ONLY: written into `er\mods_test\kh-er.ini`, never the user's): `test.keyblade = KEY[,KEY2,KEY3]`
//! (one-shot) puts those keyblades into his three slots, the first in his hand (`WEP_KEYBLADE_SO_06` or `w_so080`);
//! `test.kb_form = FORM[:GROW]` (held while set) draws the hand keyblade's form weapon parts instead of it (keyblade.rs
//! `draw_form_weapon_parts`: Crystal Snow's `07` = its four Blizzard Blades), `+t` after it adds the transform units.

use std::sync::Mutex;

use kh_anim::Trs;
use kh_core::status::{KeybladeId, Keyblades, Status};

use crate::control::{Control, OneShot};
use crate::erlog;

#[derive(Default)]
struct KbDraw {
    /// the keyblade in his hand (its id `w_soNNN`; the Kingdom Key = `w_so010`) and its base model
    current: String,
    base: String,
    /// its own keychain (None: the Kingdom Key's, the host's own; or this keyblade has none: rigid)
    /// (KEYCHAIN-DUAL: the one rule of the player and every Sora copy, [`HandChain`])
    chain: HandChain,
    /// (KEYCHAIN-DUAL) the keychains of the Keyblades his form holds (Double Form's Oathkeeper and Oblivion, Light / Dark Form's)
    parts: PartChains,
    /// (KEYCHAIN-DUAL) the parts whose keychains the log told last
    parts_told: Vec<String>,
    /// the core's switch counter seen, the time since the latest switch and the delay of that switch
    switches: u32,
    since: f32,
    delay: f32,
    /// the effect side was told this keyblade's trail and sockets
    effects_for: String,
    /// `test.kb_form`
    form: Option<(String, String, bool)>,
    /// (P2 hitfx) the weapon model his attacks hit with (kh-core `Status::attack_model`: a form's hand unit, else the hand keyblade)
    attack: String,
    /// (AUDIT-C TF) the weapon model in his left hand (kh-core `Status::left_attack_model`: Double Form's w_so210; "" none)
    left: String,
    aid: Option<OneShot>,
}

static STATE: Mutex<Option<KbDraw>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut KbDraw) -> R) -> R {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(|| KbDraw { since: 10.0, ..KbDraw::default() }))
}

/// The keyblade's id in the keyblades pack (`w_so080`) for a core id.
fn id_of(k: Option<KeybladeId>) -> String {
    let key = k.map_or("WEP_KEYBLADE_SO_00", |k| k.key());
    kh_er_render::keyblade::table().and_then(|t| t.keyblade(key)).map_or_else(|| kh_er_render::keyblade::KINGDOM_KEY.to_string(), |k| k.id.clone())
}

/// ONCE A STEP, with the control file (sora.rs, the main step, before `step_chain`): the test aids.
pub fn frame(control: &Control, status: &mut Status) {
    with(|s| {
        let aid = s.aid.get_or_insert_with(|| OneShot::new("test.keyblade", control));
        if let Some(text) = aid.fired(control) {
            let keys: Vec<String> = text.split(',').map(|k| k.trim()).filter(|k| !k.is_empty()).map(|k| kh_er_render::keyblade::table().and_then(|t| t.keyblade(k)).and_then(|kb| kb.keys.first().cloned()).unwrap_or_else(|| k.to_string())).collect();
            let slots: Vec<Option<&str>> = (0..3).map(|i| keys.get(i).map(String::as_str)).collect();
            let levels = status.keyblades().levels_by_key();
            let (k, notes) = Keyblades::from_keys(&slots, 0, &levels);
            status.set_keyblades(k);
            erlog!("kbdraw: test.keyblade = {text}: slots {:?}, in his hand {:?}{}", status.keyblades().slot_keys(), status.current_keyblade().map(KeybladeId::key), if notes.is_empty() { String::new() } else { format!(" ({})", notes.join("; ")) });
        }
        s.form = control.str("test.kb_form").map(str::trim).filter(|f| !f.is_empty() && *f != "0").map(|f| {
            let (f, transform) = f.strip_suffix("+t").map_or((f, false), |x| (x, true));
            let (form, grow) = f.split_once(':').unwrap_or((f, ""));
            (form.to_string(), grow.to_string(), transform)
        });
    });
}

/// ONCE A STEP (sora.rs `step_chain`, every path): the equipment to the renderer, the switch timing, the effect side.
pub fn equip(status: &Status) {
    let current = id_of(status.current_keyblade());
    let slots: Vec<String> = status.keyblades().slots().iter().map(|k| k.map(|k| id_of(Some(k))).unwrap_or_default()).filter(|k| !k.is_empty() && *k != current).collect();
    kh_er_render::keyblade::set_equipped(&current, &slots.iter().map(String::as_str).collect::<Vec<_>>());
    with(|s| {
        s.attack = status.attack_model.to_string(); // (P2 hitfx)
        s.left = status.left_attack_model.to_string(); // (AUDIT-C TF)
        s.since += crate::sora::STEP;
        if status.keyblade_switches != s.switches {
            s.switches = status.keyblade_switches;
            s.since = 0.0;
            // (INT) no switch recorded: the status was replaced (a load, a respawn), not switched: no hiding
            s.delay = status.keyblade_switch.map_or(0.0, |sw| sw.appear_visible_delay());
        }
        if s.current != current {
            erlog!("keyblade: {} in his hand{}", current, if s.current.is_empty() { String::new() } else { format!(" (was {})", s.current) });
            s.base = kh_er_render::keyblade::table().and_then(|t| t.keyblade(&current)).map_or_else(|| current.clone(), |k| k.base.clone());
            s.current.clone_from(&current);
        }
        // (the effect names of every slotted keyblade pass the other-keyblade rule: a switch's disappear and appear effects
        // are spawned in the step that switches, before this)
        let key = format!("{} {}", s.base, slots.join(","));
        if s.effects_for != key {
            let (trail, sockets) = effect_data(status, &s.base);
            let allowed = allowed_of(status, &current, &slots); // (MIMIC-FX: the rule shared with the Sora copies, `effect_facts`)
            // (LOADIN) THE SLOTTED KEYBLADES' OWN EFFECTS PARSED AHEAD (their trails, appear / disappear and hit effects: the base
            // models' `v<model>_*` of the keyblades' root, the trail, what their data spawns), a few a frame (fx.rs `Fx::frame`),
            // and named to the renderer: made ready before the first swing or switch, not at it (the user 2026-11-10: "keyblade
            // particle effects ... don't load in as soon as the character does"). Their form weapons' effects come with the form,
            // as before.
            let spawns: Vec<String> = status.keyblades().slots().iter().flatten().flat_map(|k| kh_core::status::keyblade::effect_names(*k)).map(str::to_string).collect();
            let bases: Vec<String> = kh_er_render::keyblade::table().map(|t| std::iter::once(current.as_str()).chain(slots.iter().map(String::as_str)).filter_map(|id| t.keyblade(id)).map(|k| k.base.clone()).filter(|b| b != kh_er_render::keyblade::KINGDOM_KEY).collect()).unwrap_or_default();
            let mut queued = 0;
            let told = crate::fx::with_effects(|e| {
                e.set_keyblade(trail.as_deref(), sockets.as_ref(), &allowed);
                queued = e.queue_keyblade_effects(trail.as_deref(), &spawns, &bases);
            });
            if told {
                if queued > 0 {
                    erlog!("keyblade: (LOADIN) {queued} effect(s) of the slotted keyblades ({}) parsed ahead of their first spawn", bases.join(", "));
                }
                erlog!("keyblade: the effect side takes {}'s trail {} and its {} socket(s); its effects and the slotted keyblades' pass ({})", s.base, trail.as_deref().unwrap_or("(the Kingdom Key's)"), sockets.as_ref().and_then(serde_json::Value::as_object).map_or(0, |o| o.len()), allowed.join(", "));
                s.effects_for = key;
            }
        }
    });
}

/// The trail effect (`kb.<KEY>.fx.trail`, its asset name) and the effect sockets (`<model>/sockets.json`) of the keyblade in
/// his hand; the Kingdom Key: none (the effect side's own).
fn effect_data(status: &Status, base: &str) -> (Option<String>, Option<serde_json::Value>) {
    if base == kh_er_render::keyblade::KINGDOM_KEY {
        return (None, None);
    }
    let trail = status.current_keyblade().and_then(|k| k.effect_name("fx.trail")).map(str::to_string);
    (trail, kh_er_render::keyblade::sockets_json(base))
}

/// The effect names that pass the other-keyblade rule for the keyblades of `status` (`current` in his hand, `slots` the other
/// slotted ones): their models' prefixes, every effect name their own data spawns, the other models' effects of their forms'
/// shared clips. (MIMIC-FX: cut out of `equip`, the same rule for the player and every Sora copy.)
fn allowed_of(status: &Status, current: &str, slots: &[String]) -> Vec<String> {
    let mut allowed = allowed_effects(current, slots);
    // (P2 hitfx) and every effect name the slotted keyblades' own data spawns (kh-core sora-keyblades fx / hit rows: Dead of
    // Night's vw_so120_*, Oblivion's map hit vw_so100_wht0 are named after other models)
    allowed.extend(status.keyblades().slots().iter().flatten().flat_map(|k| kh_core::status::keyblade::effect_names(*k)).map(str::to_lowercase));
    // (AUDIT-A SHIELD) and the other models' effects KH3 spawns in their forms' group-0 plays of shared clips (Frying
    // Pan's stab / GuardChange draw Counter Shield's vw_so032_tuk0 / vw_so033_start0: kh-core forms::shared_clip_effects)
    allowed.extend(status.keyblades().slots().iter().flatten().flat_map(|k| kh_core::combat::forms::shared_clip_effects(k.key())));
    allowed.sort();
    allowed.dedup();
    allowed
}

/// (MIMIC-FX) What the effect side needs of a Sora's Keyblades (`EffectHost::set_keyblade` / `set_instance_keyblade`).
#[cfg(feature = "party")]
pub struct EffectFacts {
    pub trail: Option<String>,
    pub sockets: Option<serde_json::Value>,
    pub allowed: Vec<String>,
}

/// (MIMIC-FX) The keyblade in hand of `status` and its slots as `equip` keys them for the effect side (a change of it = new
/// facts), and its base model.
#[cfg(feature = "party")]
pub fn effect_key(status: &Status) -> (String, String) {
    let current = id_of(status.current_keyblade());
    let slots: Vec<String> = status.keyblades().slots().iter().map(|k| k.map(|k| id_of(Some(k))).unwrap_or_default()).filter(|k| !k.is_empty() && *k != current).collect();
    let base = kh_er_render::keyblade::table().and_then(|t| t.keyblade(&current)).map_or_else(|| current.clone(), |k| k.base.clone());
    (format!("{base} {}", slots.join(",")), base)
}

/// (MIMIC-FX) THE EFFECT SIDE'S FACTS OF A SORA COPY'S KEYBLADES (`equip`'s rule on HIS status): his hand keyblade's trail and
/// sockets, the effect names his slotted keyblades pass.
#[cfg(feature = "party")]
pub fn effect_facts(status: &Status) -> EffectFacts {
    let current = id_of(status.current_keyblade());
    let slots: Vec<String> = status.keyblades().slots().iter().map(|k| k.map(|k| id_of(Some(k))).unwrap_or_default()).filter(|k| !k.is_empty() && *k != current).collect();
    let (_, base) = effect_key(status);
    let (trail, sockets) = effect_data(status, &base);
    EffectFacts { trail, sockets, allowed: allowed_of(status, &current, &slots) }
}

/// The effect name prefixes of the keyblades in his slots (`v<model>` = `vw_soNNN` of each of their models; the Kingdom Key's pass
/// anyway).
fn allowed_effects(current: &str, slots: &[String]) -> Vec<String> {
    let Some(t) = kh_er_render::keyblade::table() else { return Vec::new() };
    let mut out: Vec<String> = std::iter::once(current).chain(slots.iter().map(String::as_str)).filter_map(|id| t.keyblade(id)).flat_map(|k| std::iter::once(k.base.clone()).chain(k.models.iter().cloned())).filter(|m| m != kh_er_render::keyblade::KINGDOM_KEY).map(|m| format!("v{m}")).collect();
    out.sort();
    out.dedup();
    out
}

/// THE KEYCHAIN of the keyblade in his hand for one step (sora.rs `step_chain`): `Some(pose)` in THAT keyblade's bone
/// order (empty: it has no keychain, the renderer draws its reference pose); None for the Kingdom Key (the host's own).
pub fn step_chain(dt: f32, weapon_to_world: &Trs, input: &ChainIn<'_>, appeared: bool) -> Option<Vec<Trs>> {
    with(|s| s.chain.step(&s.base, dt, weapon_to_world, input, appeared))
}

// ---------------------------------------------------------------------------------------------------- (KEYCHAIN-DUAL)
// EVERY KEYBLADE SORA HOLDS HAS ITS OWN KEYCHAIN. KH3's equipment (UTresEquipmentComponent 0x141791120, AUDIT-B W-CORE) spawns
// one weapon actor per unit of a form (`RWeapon` / `LWeapon` / the transform units: the manifest's `blueprint`, w_so200_Wep,
// w_so210_Wep), each with its own mesh, whose own SQEX_BonamikAsset (Oathkeeper w_ex090_Bonamik, Oblivion w_ex100_Bonamik:
// `<model>/bonamik.json`) is simulated by its own KBD component on that mesh: its ComponentToWorld is its own socket (R_wep_skt
// on R_buki, L_wep_skt on L_buki), so its inertia is its own hand's travel; Sora's chara wind reaches it as a child of his mesh
// (kh-anim keychain.rs NOTIFIES). The port drew a form's parts by their clip alone (keyblade.rs `model_pose`): Double Form's
// two Keyblades and Light / Dark Form's one hung rigid. [`PartChains`] gives each drawn part with a keychain its own Bonamik,
// stepped at the fixed step on the part as placed at that step ([`step_parts`]) and drawn between the two latest steps.

/// (KEYCHAIN-DUAL) The keyblades table the parts' keychains and clip notifies are read from (the renderer's; the offline tests
/// read the pack's themselves, the renderer's is set by its install).
#[cfg(not(test))]
const KB_TABLE: fn() -> Option<&'static kh_er_render::keyblade::Table> = kh_er_render::keyblade::table;
#[cfg(test)]
const KB_TABLE: fn() -> Option<&'static kh_er_render::keyblade::Table> = tests::table;

fn kb_table() -> Option<&'static kh_er_render::keyblade::Table> {
    KB_TABLE()
}

/// (KEYCHAIN-DUAL) What every keychain of a Sora is fed at a step: his Animator (his solver's chara wind, mirrored; the
/// CharaWind / SpeedRate states of the clips he plays, routed: KH3's "children" calls) and the clip his weapons play (its
/// logical name and time: [`StepParts::clip`]; each weapon's own clip of it runs its Bonamik states on that weapon).
pub(crate) struct ChainIn<'a> {
    pub(crate) animator: Option<&'a kh_anim::Animator>,
    pub(crate) clip: (Option<&'a str>, f32),
}

/// (KEYCHAIN-DUAL) ONE STEP OF ONE WEAPON'S KEYCHAIN (the hand Keyblade's, the Kingdom Key's, a form part's; the player's and
/// every copy's): Sora's routed clip states, the weapon `model`'s own clip states (the hand Keyblades' 1814 SoKcIrradiate:
/// SpeedRate 0.2 + ChangeGroupParams; Storm Flag's F418 / F420), then the solver on its animated pose (None: its reference).
pub(crate) fn step_keychain<'c>(c: &'c mut kh_anim::Keychain, model: &str, dt: f32, animated: Option<&[Trs]>, to_world: &Trs, input: &ChainIn<'_>) -> &'c [Trs] {
    if let Some(a) = input.animator {
        c.route_states(&a.child_bonamik_states());
    }
    let notes: Vec<(u64, String, kh_anim::json::Json)> = kb_table().map(|t| kh_er_render::keyblade::bonamik_notifies(t, model, input.clip.0, input.clip.1)).unwrap_or_default().into_iter().filter_map(|(k, class, props)| kh_anim::json::Json::parse(&props).ok().map(|j| (k, class, j))).collect();
    c.own_notifies(&notes.iter().map(|(k, class, j)| (*k, class.as_str(), j)).collect::<Vec<_>>());
    c.step_posed(dt, animated, to_world, input.animator.and_then(kh_anim::Animator::bonamik))
}

/// (KEYCHAIN-DUAL) A keyblade model's keychain (None: it has none, or it is not readable: it hangs rigid).
fn load_chain(model: &str) -> Option<kh_anim::Keychain> {
    match kb_table().and_then(|t| kh_er_render::keyblade::keychain_in(t, model)) {
        Some(Ok(c)) => {
            erlog!("keyblade: {model}'s keychain: Bonamik on its skeleton ({} bones, {} driven)", c.skeleton().len(), c.driven_bones().len());
            Some(c)
        }
        Some(Err(e)) => {
            erlog!("keyblade: {model}'s keychain not read ({e}): it hangs rigid");
            None
        }
        None => None,
    }
}

/// (KEYCHAIN-DUAL, cut out of `step_chain`) THE HAND KEYBLADE'S OWN KEYCHAIN of one Sora (the player's, a copy's).
#[derive(Default)]
pub(crate) struct HandChain {
    chain: Option<kh_anim::Keychain>,
    chain_for: String,
    /// (KEYBLADE-SWAP) the keychains of the keyblades held before (model, its keychain; the latest few): a D-pad switch back
    /// to one reads no file on the game's thread (it is reset as a freshly read one)
    kept: Vec<(String, Option<kh_anim::Keychain>)>,
}

/// (KEYBLADE-SWAP) How many keychains of keyblades held before a [`HandChain`] keeps (his three slots and a spare).
const KEPT_CHAINS: usize = 4;

impl HandChain {
    /// One step of the keychain of `base` (the hand keyblade's base model) at `weapon_to_world`: `Some(pose)` in THAT model's
    /// bone order (empty: it has no keychain); None for the Kingdom Key (the host's own).
    pub(crate) fn step(&mut self, base: &str, dt: f32, weapon_to_world: &Trs, input: &ChainIn<'_>, appeared: bool) -> Option<Vec<Trs>> {
        if base.is_empty() || base == kh_er_render::keyblade::KINGDOM_KEY {
            return None;
        }
        if self.chain_for != base {
            // (KEYBLADE-SWAP) the one held so far is kept; the new one from those kept, else read
            let before = (std::mem::take(&mut self.chain_for), self.chain.take());
            if !before.0.is_empty() {
                self.kept.retain(|(m, _)| *m != before.0);
                self.kept.push(before);
                if self.kept.len() > KEPT_CHAINS {
                    self.kept.remove(0);
                }
            }
            self.chain = match self.kept.iter().position(|(m, _)| m == base) {
                Some(k) => self.kept.remove(k).1,
                None => load_chain(base),
            };
            self.chain_for = base.to_string();
            if let Some(c) = self.chain.as_mut() {
                c.reset();
            }
        } else if appeared {
            if let Some(c) = self.chain.as_mut() {
                c.reset();
            }
        }
        Some(self.chain.as_mut().map(|c| step_keychain(c, base, dt, None, weapon_to_world, input).to_vec()).unwrap_or_default())
    }
}

/// (KEYCHAIN-DUAL) One drawn part's keychain: its model, which of that model's parts it is, its poses of the two latest steps.
struct PartSim {
    model: String,
    nth: usize,
    chain: Option<kh_anim::Keychain>,
    now: Vec<Trs>,
    before: Vec<Trs>,
    /// it was a part at the latest step
    live: bool,
}

/// (KEYCHAIN-DUAL) THE KEYCHAINS OF THE KEYBLADE PARTS ONE SORA'S FORM HOLDS (the player's: [`step_form_chains`]; a copy's:
/// `spirit_mimic::Rig::chains`): one Bonamik per part (model, nth) whose model has a keychain, kept while the part is away (it
/// is reset when it is back, as the hand's keychain when the Keyblade appears).
#[derive(Default)]
pub(crate) struct PartChains {
    sims: Vec<PartSim>,
}

/// (KEYCHAIN-DUAL) The `kb:` parts of a list with their occurrence of that model: (index in the list, model, nth).
fn kb_parts(forms: &[kh_er_render::FormWeapon]) -> Vec<(usize, String, usize)> {
    let mut out: Vec<(usize, String, usize)> = Vec::new();
    for (i, f) in forms.iter().enumerate() {
        if let Some(model) = f.model.strip_prefix(kh_er_render::keyblade::PREFIX) {
            let nth = out.iter().filter(|p| p.1 == model).count();
            out.push((i, model.to_string(), nth));
        }
    }
    out
}

impl PartChains {
    /// ONCE A FIXED STEP: every part of `parts` (as placed at this step: [`step_parts`]) steps its own keychain on its own
    /// clip's pose at its own place (`FormWeapon::to_world`: its socket's ComponentToWorld); a part new at this step is reset.
    /// `clip` = the logical name of the clip the parts play (Sora's: [`StepParts::clip`]) and its time: each part's own clip of
    /// it runs its Bonamik notify states on its keychain (KH3's weapon anim instance -> its own KBD component).
    pub(crate) fn step(&mut self, dt: f32, parts: &[kh_er_render::FormWeapon], input: &ChainIn<'_>) {
        let mut seen = vec![false; self.sims.len()];
        for (i, model, nth) in kb_parts(parts) {
            let k = match self.sims.iter().position(|s| s.model == model && s.nth == nth) {
                Some(k) => k,
                None => {
                    self.sims.push(PartSim { chain: load_chain(&model), model, nth, now: Vec::new(), before: Vec::new(), live: false });
                    seen.push(false);
                    self.sims.len() - 1
                }
            };
            seen[k] = true;
            let sim = &mut self.sims[k];
            let Some(c) = sim.chain.as_mut() else { continue };
            let appeared = !sim.live;
            if appeared {
                c.reset();
            }
            let now = step_keychain(c, &sim.model, dt, Some(&parts[i].component), &parts[i].to_world, input).to_vec();
            sim.before = std::mem::replace(&mut sim.now, now);
            if appeared || sim.before.len() != sim.now.len() {
                sim.before.clone_from(&sim.now);
            }
        }
        for (s, seen) in self.sims.iter_mut().zip(seen) {
            s.live = seen;
        }
    }

    /// ONCE A FRAME, on the drawn list: each part with a live keychain takes its pose between the two latest steps (`alpha`):
    /// its clip's bones as the body's, its chain as simulated. A part of another bone count is left as it is.
    pub(crate) fn apply(&self, forms: &mut [kh_er_render::FormWeapon], alpha: f32) {
        for (i, model, nth) in kb_parts(forms) {
            let Some(s) = self.sims.iter().find(|s| s.live && s.chain.is_some() && s.model == model && s.nth == nth) else { continue };
            let f = &mut forms[i];
            if s.now.len() == f.component.len() && s.before.len() == s.now.len() {
                f.component = s.before.iter().zip(&s.now).map(|(a, b)| crate::interp::lerp_trs(a, b, alpha)).collect();
            }
        }
    }

    /// The models whose keychain stepped at the latest step (the tests, the log).
    pub(crate) fn live(&self) -> Vec<String> {
        self.sims.iter().filter(|s| s.live && s.chain.is_some()).map(|s| s.model.clone()).collect()
    }
}

/// (KEYCHAIN-DUAL) What one Sora's Keyblade parts are placed from at a fixed step (sora.rs `submit`'s rule, at the step).
pub(crate) struct StepParts<'a> {
    pub(crate) sora: &'a kh_core::Sora,
    pub(crate) out: &'a kh_core::Output,
    pub(crate) skeleton: &'a kh_anim::Skeleton,
    /// his component-space pose of the step (the player with an outfit on: the outfit's hands, as his hand's keychain)
    pub(crate) component: &'a [Trs],
    pub(crate) to_world: Trs,
    /// his weapon is shown (its opacity > 0)
    pub(crate) shown: bool,
    /// the player's `test.kb_form` aid counts
    pub(crate) test_aid: bool,
    /// (FORMWEP-FIX) a form just started, its transformation not begun (kh-host formstart.rs): the hand Keyblade, no form units
    pub(crate) start_pending: bool,
}

/// (KEYCHAIN-DUAL) THE KEYBLADE PARTS OF A SORA AT A STEP, as sora.rs `submit` places them: the test aid's form, a shotlock's
/// weapon form, else his form's equipped units (the Keyblade until StyleChange's equip, AUDIT-B W-CORE), each on its socket or
/// on the bone a weapon clip's ChangeAttachParent window names, a thrown part at its flight.
impl StepParts<'_> {
    /// (KEYCHAIN-DUAL) The logical name of the clip his weapons play (sora.rs `submit`'s `weapon_anim`: a blend-space row of the
    /// held form's set by its logical name, else his clip's sequence) and its time.
    pub(crate) fn clip(&self) -> (Option<&'static str>, f32) {
        let form = &self.sora.combat().form;
        (kh_core::combat::forms::weapon_logical(form.cmd, self.out.anim.name).or_else(|| self.out.anim.name.sequences().first().copied()), self.out.anim.time)
    }
}

pub(crate) fn step_parts(p: &StepParts<'_>) -> Vec<kh_er_render::FormWeapon> {
    let keyblade = id_of(p.sora.status().current_keyblade());
    let index_of = |b: &str| p.skeleton.index_of(b);
    let hand_socket = super::weapon_socket();
    let combat = p.sora.combat();
    let form = &combat.form;
    let magic = p.sora.magic();
    let anim = p.clip().0;
    let pose = kh_er_render::keyblade::FormPose { index_of: &index_of, component: p.component, to_world: &p.to_world, anim, time: p.out.anim.time, hand_socket: &hand_socket };
    let mut parts = if p.test_aid { form_parts(&pose) } else { Vec::new() };
    if magic.shot_weapon_form != 0 {
        let id = format!("{:02X}", magic.shot_weapon_form);
        if magic.shot_weapon_units.is_empty() {
            parts.extend(kh_er_render::keyblade::draw_form_weapon_parts(&keyblade, &id, "", false, &pose));
        } else {
            parts.extend(unit_parts(&keyblade, &id, magic.shot_weapon_units, &pose));
        }
    } else if let Some(row) = form.row().filter(|r| r.weapon_form_value > 1 && parts.is_empty() && form.equip != kh_core::combat::form::FormEquip::Keyblade && !p.start_pending) {
        let grow = form.grow.max(1).to_string();
        let transform = form.equip == kh_core::combat::form::FormEquip::Transform;
        if p.shown || p.out.state == kh_core::StateId::Stylechange {
            parts.extend(kh_er_render::keyblade::draw_equipped_parts(&keyblade, row.weapon_form, &grow, transform, &pose));
        }
    }
    let (seq, time) = (p.out.anim.name.sequences().first().copied().unwrap_or(""), p.out.anim.time);
    for model in kb_parts(&parts).into_iter().map(|k| k.1).collect::<Vec<_>>() {
        if let Some(b) = kh_core::combat::forms::placement::part_parent(form.cmd, &model, seq, time).or_else(|| kh_core::combat::forms::placement::clip_parent(&model, seq, time)).and_then(|name| {
            let (bone, loc, rot) = kh_core::combat::forms::placement::socket(name).unwrap_or((name, glam::Vec3::ZERO, glam::Vec3::ZERO));
            let socket = Trs { translation: loc, rotation: kh_anim::blend::Rotator { pitch: rot.x, yaw: rot.y, roll: rot.z }.quat(), scale: glam::Vec3::ONE };
            index_of(bone).and_then(|i| p.component.get(i)).map(|b| socket.then(b))
        }) {
            kh_er_render::keyblade::part_on_bone(&mut parts, &model, &b, &p.to_world);
        }
    }
    for t in &combat.thrown {
        let at = Trs { translation: t.position, rotation: t.rotation, scale: glam::Vec3::ONE };
        kh_er_render::keyblade::part_thrown(&mut parts, t.model, at, t.clip().0, t.clip_time());
    }
    parts
}

/// (KEYCHAIN-DUAL) ONCE A FIXED STEP (sora.rs `step_chain`): the player's form parts' keychains.
pub fn step_form_chains(dt: f32, p: &StepParts<'_>, animator: Option<&kh_anim::Animator>) {
    let parts = step_parts(p); // (outside `with`: the test aid's parts read this state)
    with(|s| {
        s.parts.step(dt, &parts, &ChainIn { animator, clip: p.clip() });
        let live = s.parts.live();
        if live != s.parts_told {
            erlog!("keyblade: the form's Keyblade parts with their own keychains: {}", if live.is_empty() { "none".to_string() } else { live.join(", ") });
            s.parts_told = live;
        }
    });
}

/// (KEYCHAIN-DUAL) ONCE A FRAME (sora.rs `submit`, the drawn list): the player's form parts with their keychains.
pub fn apply_form_chains(forms: &mut [kh_er_render::FormWeapon], alpha: f32) {
    with(|s| s.parts.apply(forms, alpha));
}

/// (KEYCHAIN-DUAL) A Sora copy's keychains (spirit_mimic.rs `Rig::chains`): his hand keyblade's own and his form parts'.
#[cfg(feature = "party")]
#[derive(Default)]
pub(crate) struct Keychains {
    pub(crate) hand: HandChain,
    pub(crate) parts: PartChains,
}

/// (KEYCHAIN-DUAL) The base model of the keyblade in a Sora's hand (`w_so200`; the Kingdom Key `w_so010`).
#[cfg(feature = "party")]
pub(crate) fn base_of(status: &Status) -> String {
    effect_key(status).1
}

/// His weapon's visibility from the switch (KH3's `m_AppearEffect` delay: hidden for `appear_visible_delay()` after a
/// switch, then shown over 0.06 s, the appear dither time).
pub fn visibility() -> f32 {
    with(|s| ((s.since - s.delay) / 0.06).clamp(0.0, 1.0))
}

/// (test aid `test.kb_form`) The hand keyblade's form parts for this frame (empty without the aid): sora.rs `submit` adds
/// them to the frame's forms (the hand keyblade is then hidden).
pub fn form_parts(pose: &kh_er_render::keyblade::FormPose<'_>) -> Vec<kh_er_render::FormWeapon> {
    let (base, form) = with(|s| (s.current.clone(), s.form.clone()));
    match form {
        Some((form, grow, transform)) => kh_er_render::keyblade::draw_form_weapon_parts(&base, &form, &grow, transform, pose),
        None => Vec::new(),
    }
}

/// (INT-F) The keyblade in his hand (its id in the keyblades pack, `w_so080`; the Kingdom Key `w_so010`): whose forms and
/// form pawns are drawn.
pub fn hand() -> String {
    with(|s| if s.current.is_empty() { kh_er_render::keyblade::KINGDOM_KEY.to_string() } else { s.current.clone() })
}

/// (INT-F, SHOT-A pellet) A shotlock's OWN equip set of the hand keyblade's form `form` (`PawnMagic::shot_weapon_units`:
/// `unit@part:socket=blueprint`, e.g. Atomic Deluge's transform set w_so031, then its LV3 parts w_so034 + w_so037): the form's
/// parts of every grow level and its transform units whose model a unit names (`.../w_so031_wep` -> `w_so031`), each once.
pub fn unit_parts(keyblade: &str, form: &str, units: &[&str], pose: &kh_er_render::keyblade::FormPose<'_>) -> Vec<kh_er_render::FormWeapon> {
    let Some(t) = kh_er_render::keyblade::table() else { return Vec::new() };
    let models: Vec<String> = units.iter().filter_map(|u| u.rsplit('/').next()).map(|b| b.to_ascii_lowercase().trim_end_matches("_wep").to_string()).collect();
    let loaded = kh_er_render::keyblade::loaded();
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for grow in ["1", "2", "3"] {
        for p in kh_er_render::keyblade::form_parts(keyblade, form, grow, true) {
            if models.contains(&p.model) && !seen.contains(&p.model) && loaded.contains(&p.model) {
                seen.push(p.model.clone());
                out.extend(kh_er_render::keyblade::place_part(t, &p, pose));
            }
        }
    }
    out
}

/// The model whose swing / hit sound sets his attacks play (fx.rs after a step): (P2 hitfx) the weapon the hits come from
/// (kh-core `Status::attack_model`: in a form its hand unit, `w_so052` ...; KH3 plays a window's HITn alias from the set of
/// the component it is on), else the hand keyblade's base model (the Kingdom Key: `w_so010`).
pub fn sound_model() -> String {
    with(|s| if !s.attack.is_empty() { s.attack.clone() } else if s.base.is_empty() { kh_er_render::keyblade::KINGDOM_KEY.to_string() } else { s.base.clone() })
}

/// (fx.rs, its audio step) Sora's swing / hit sets follow the keyblade in his hand (KH3: the weapon's own SE sets).
pub fn select_sounds(engine: &mut kh_audio::Engine) {
    let model = sound_model();
    // (AUDIT-A) KH3's rule (Routes::select_weapon_kh3): a weapon of Sora's without sets of its own (the Double Arrowguns
    // w_so022 / w_so023) is silent, it no longer keeps the previous weapon's sets
    if engine.routes.weapon.as_deref() != Some(model.as_str()) {
        engine.routes.select_weapon_kh3(&model);
    }
    // (AUDIT-C TF) the LH_WEAPON aliases from the left hand's weapon (Double Form's Oblivion), none without one
    engine.routes.select_left_weapon(&with(|s| s.left.clone()));
}

/// (KEYCHAIN-DUAL) the offline tests of every Keyblade part's own keychain (Double Form, Light / Dark Form, Storm Flag's banner)
#[cfg(test)]
#[path = "kbdraw_tests.rs"]
mod tests;

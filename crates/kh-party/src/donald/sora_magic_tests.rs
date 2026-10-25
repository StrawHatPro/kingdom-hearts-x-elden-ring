//! (DON-MAGIC part A) The magic Donald shares with Sora (donald/sora_magic.rs): every line-1 tier is a shot of Sora's own
//! kh-core spell actors with Sora's command (the -za tier: Sora's Grand Magic), the hosts get its actors and rails, the
//! shot leaves toward its target, and an AI Donald's cast clip is not run over by his locomotion.
//! A child of magic_tests (its rig).

use super::*;
use crate::command::MemberCommand;
use crate::states::NpcStateId;

/// Ready for the tier (the -za tier needs full power: attuned at Resonance 3, full this fight) and MP.
fn ready(rig: &mut Rig, e: Element, tier: u8) {
    if tier >= 4 {
        rig.attune(e, 3.0);
        rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap().core.fight_full.insert(e);
    }
    rig.party.status_mut(rig.don).unwrap().mp = 1000;
}

fn line1() -> Vec<(Element, u8)> {
    let mut v = Vec::new();
    for e in Element::ALL {
        for t in 1..=4u8 {
            v.push((e, t));
        }
    }
    v
}

#[test]
fn every_line1_tier_shoots_soras_own_kh_core_actors_with_soras_command() {
    for (e, tier) in line1() {
        let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
        let spell = SpellRef::Line1(e, tier);
        let target = if e == Element::Light { CastTarget::Member(rig.don) } else { e1() };
        ready(&mut rig, e, tier);
        assert!(rig.begin(spell, target), "{spell:?} began");
        let row = spell.row(&data()).copied().unwrap();
        rig.secs(row.cast + 0.05);
        // the shot is Sora's command through Sora's caster
        let want = magic_fx::sora_cmd(e, tier);
        assert!(rig.log().iter().any(|l| matches!(l, FxLog::Sora { line: 1, cmd } if *cmd == want)), "{spell:?}: Sora's command {want}");
        let c = rig.party.don_fx.casters.iter().find(|c| c.0 == rig.don).map(|c| &c.1).expect("Donald's kh-core SpellCaster");
        // the -za tier is Sora's Grand Magic: its "-gan" command (Firaza 112 .. Curaza 125)
        if tier == 4 {
            assert!((112..=125).contains(&want), "{spell:?}: the Grand Magic command {want}");
        }
        assert!(c.alive() > 0 || !rig.dummies.hits.is_empty() || e == Element::Light, "{spell:?}: Sora's actors are out (or have hit)");
    }
}

#[test]
fn the_hosts_get_donalds_line1_actors_and_ice_rails() {
    // Fire: the fireball is in the party's projectile taps (the hosts place its effect by them)
    let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)]);
    assert!(rig.begin(SpellRef::Line1(Element::Fire, 1), e1()));
    let row = SpellRef::Line1(Element::Fire, 1).row(&data()).copied().unwrap();
    rig.secs(row.cast + 0.1);
    let own: Vec<u32> = rig.party.don_fx.casters.iter().flat_map(|(_, c)| c.projectiles().filter_map(|v| v.effect.map(|e| e.0)).collect::<Vec<_>>()).collect();
    assert!(!own.is_empty(), "a fireball is flying");
    let taps: Vec<u32> = rig.party.caster_projectiles().map(|t| t.0).collect();
    assert!(own.iter().all(|h| taps.contains(h)), "the taps carry Donald's fireball: {own:?} in {taps:?}");
    // Blizzard: its ice rail nodes reach the hosts
    let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)]);
    assert!(rig.begin(SpellRef::Line1(Element::Blizzard, 2), e1()));
    let row = SpellRef::Line1(Element::Blizzard, 2).row(&data()).copied().unwrap();
    rig.secs(row.cast + 0.5);
    let own: usize = rig.party.don_fx.casters.iter().map(|(_, c)| c.rails().len()).sum();
    assert_eq!(rig.party.caster_rails().len(), own, "the rails reach the hosts");
}

#[test]
fn the_shot_leaves_toward_its_target_and_hits_it() {
    // Donald turned away from the dummy (the AI may turn him while the clip plays): the fireball still goes to it
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    assert!(rig.begin(SpellRef::Line1(Element::Fire, 1), e1()));
    rig.party.member_mut(rig.don).unwrap().yaw = 120.0;
    rig.secs(2.0);
    assert!(!rig.dummies.hits_on(1).is_empty(), "the fireball reached its target");
    assert!(rig.party.member(rig.don).unwrap().yaw.abs() < 5.0, "he faced it at the shot");
}

#[test]
fn an_ai_donald_keeps_his_cast_clip_and_refuses_moves_while_casting() {
    for (spell, clip) in [(SpellRef::Line1(Element::Fire, 1), "MagicFire"), (SpellRef::Line1(Element::Thunder, 2), "MagicCast"), (SpellRef::Line1(Element::Fire, 4), "MagicCast"), (SpellRef::Line1(Element::Light, 1), "MagicCast")] {
        let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
        rig.party.set_pad(rig.don, None);
        let target = if matches!(spell, SpellRef::Line1(Element::Light, _)) { CastTarget::Member(rig.don) } else { e1() };
        if let SpellRef::Line1(e, t) = spell {
            ready(&mut rig, e, t);
        }
        assert!(rig.begin(spell, target), "{spell:?}");
        assert_eq!(cast::clip_of(spell, false), clip, "KH3's clip for Donald's own spells");
        let p = rig.party.member(rig.don).unwrap();
        assert_eq!(p.state, NpcStateId::PlayMotion, "{spell:?}: held in the play-motion state");
        let want = p.anim.clip.sequence;
        // a move command while the cast runs is refused: the clip stays
        let p = rig.party.member_mut(rig.don).unwrap();
        let goal = crate::command::MoveGoal::Point(Vec3::new(-800.0, 0.0, 0.0));
        let t = p.command(MemberCommand::MoveTo { goal, locomotion: crate::command::LocomotionDef::default(), walk: false, acceptable_radius: 50.0, follow_path: false });
        for _ in 0..10 {
            rig.step();
        }
        let p = rig.party.member(rig.don).unwrap();
        assert_eq!(p.anim.clip.sequence, want, "{spell:?}: the cast clip plays on");
        assert_eq!(p.command_status(t), crate::command::CommandStatus::Failed, "{spell:?}: the move was refused");
    }
}

#[test]
fn donalds_spells_leave_from_his_wands_tip() {
    // (the user, 2026-10-22: "Sora's spells need to come from the tip of his keyblade"; the same code drives Donald's: his
    // wand's tip) the fire effects of each shot (the cast flash on the wand) at the wand's tip at the shot frame
    for (e, tier) in [(Element::Fire, 1u8), (Element::Blizzard, 1), (Element::Thunder, 1), (Element::Water, 1), (Element::Aero, 1)] {
        let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
        let spell = SpellRef::Line1(e, tier);
        ready(&mut rig, e, tier);
        assert!(rig.begin(spell, e1()));
        let mut found = None;
        for _ in 0..120 {
            let tip = cast::muzzle(rig.party.member(rig.don).unwrap());
            let before = rig.effects.spawned.len();
            rig.step();
            // the fire effects: the actor's class name again, one-shot, at a world point (the body is the persistent one)
            let fired = rig.effects.spawned[before..].iter().find(|s| s.1.starts_with("TresProj_") && !s.3).map(|s| s.2);
            if let Some(Attach::World(at)) = fired {
                found = Some((at, tip));
                break;
            }
        }
        let (at, tip) = found.unwrap_or_else(|| panic!("{spell:?}: no shot"));
        // a bolt's spawn point is held below actor Z + 163 and wall-guarded; the wand of a short Donald is far below that
        assert!((at - tip).length() < 15.0, "{spell:?}: fired at {at:?}, the wand's tip at {tip:?}");
    }
}

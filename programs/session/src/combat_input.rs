//! Builds `deadroll_combat` hit inputs from session and character state. Pure, no accounts.
//! A player is a `HeroStats`: the character with what it wears (`Participation::hero`).
//!
//! The rules live in `deadroll_rules::input` (shared with `tools/balance`); these wrappers feed
//! them a skill as the rules see it (`SkillStats`, the participation's map skill snapshot) and
//! the program's numbers (`rules::BALANCE`).
use deadroll_character::state::{SkillExtra, SkillRecord};
use deadroll_combat::{AccuracyBuff, Applied, Attacker, Defender, States, SupportInput, TimedMods};
use deadroll_rules::{input, items, HeroStats, MonsterDefense};
use deadroll_types::{ActiveState, Aspect, Form, Monster, RangeTag, EQUIP_SLOTS, STATES_MAX};

pub use deadroll_rules::input::{
    effect_action, heal, take_damage, EffectAction, Side, SkillStats, SkillUse,
};

use crate::errors::SessionError;
use crate::rules::{monster_profile, BALANCE};

/// The rules' view of a stored skill: its `SkillSet` record and its `SkillExtras` part.
pub fn skill_stats(skill: &SkillRecord, extra: &SkillExtra) -> Result<SkillStats, SessionError> {
    Ok(SkillStats {
        blocks: skill.blocks,
        shares_bp: skill.shares_bp,
        power_bp: skill.power_bp,
        cooldown: skill.cooldown,
        ext: extra.ext().ok_or(SessionError::InvalidSkill)?,
        ap_cost: extra.ap_cost,
    })
}

pub fn skill_use(skill: &SkillStats) -> SkillUse {
    input::skill_use(skill)
}

/// The side of `skill` (`None` = the weapon's basic attack).
pub fn target_side(skill: Option<&SkillStats>) -> Side {
    input::target_side(skill)
}

/// Form of a player attack: the skill's, or the weapon's basic attack's.
pub fn player_form(h: &HeroStats, skill: Option<&SkillStats>) -> Form {
    input::player_form(BALANCE, h, skill)
}

/// Range of a player action: the skill's, or the weapon's basic attack's.
pub fn player_range(h: &HeroStats, skill: Option<&SkillStats>) -> RangeTag {
    input::player_range(BALANCE, h, skill)
}

/// Action points of a player action: the skill's own, or the basic attack's.
pub fn action_ap(skill: Option<&SkillStats>) -> u8 {
    input::action_ap(BALANCE, skill)
}

/// Range of a player action from the participation's equipment snapshot (no character needed):
/// the skill's range tag, or the weapon's basic attack.
pub fn action_range(equipment: &[u8; EQUIP_SLOTS], skill: Option<&SkillStats>) -> RangeTag {
    match skill {
        Some(s) => s.ext.range,
        None => items::basic_attack(&items::Equipment(*equipment), &BALANCE.items).0,
    }
}

/// A player attacking with `skill` (`None` = the weapon's basic attack). `unit` is the
/// turn-order id, written as the source of applied states. `mods`: the player's buffs and
/// debuffs (auras included); `accuracy`: its resonance accuracy buff.
pub fn player_attacker(
    unit: u8,
    h: &HeroStats,
    skill: Option<&SkillStats>,
    mods: &TimedMods,
    accuracy: &AccuracyBuff,
) -> Attacker {
    input::player_attacker(BALANCE, unit, h, skill, mods, accuracy)
}

/// A player as the target.
pub fn player_defender(
    h: &HeroStats,
    states: &[ActiveState; STATES_MAX],
    mods: &TimedMods,
) -> Defender {
    input::player_defender(BALANCE, h, states, mods)
}

/// A monster attacking with its natural attack. `None` for an unknown bestiary id.
pub fn monster_attacker(
    unit: u8,
    m: &Monster,
    defense: &MonsterDefense,
    mods: &TimedMods,
) -> Option<Attacker> {
    let p = monster_profile(m.bestiary_id, m.level, defense)?;
    Some(input::monster_attacker(BALANCE, unit, &p, mods))
}

/// A monster as the target, with its registry defenses. `None` for an unknown bestiary id.
pub fn monster_defender(
    m: &Monster,
    defense: &MonsterDefense,
    mods: &TimedMods,
) -> Option<Defender> {
    let p = monster_profile(m.bestiary_id, m.level, defense)?;
    Some(input::monster_defender(&p, &m.states, mods))
}

/// Strength range of `action` cast by `h` with a skill of `power_bp`: HP for a heal (from the
/// weapon), bp for a modifier, none (`(0, 0)`) for hard control.
pub fn effect_range(h: &HeroStats, power_bp: u32, action: EffectAction) -> (u32, u32) {
    input::effect_range(BALANCE, h, power_bp, action)
}

/// The rolls of a heal, buff or debuff cast by `h` with `skill`. `defense`: the enemy's, for a
/// debuff (it rolls to land); `None` for a heal or buff on an ally.
pub fn support_input(
    h: &HeroStats,
    skill: &SkillStats,
    action: EffectAction,
    defense: Option<u32>,
) -> SupportInput {
    input::support_input(BALANCE, h, skill, action, defense)
}

/// Lands `action` (not a heal) of strength `amount` on a unit: a modifier for its duration plus
/// `extra_turns`, or the existing Stun / Freeze state. `aspect`: the skill's; `source`: the
/// caster's turn-order id.
pub fn land_effect(
    action: EffectAction,
    amount: u32,
    aspect: Aspect,
    source: u8,
    extra_turns: u8,
    states: &mut States,
    mods: &mut TimedMods,
) -> Applied {
    input::land_effect(
        BALANCE,
        action,
        amount,
        aspect,
        source,
        extra_turns,
        states,
        mods,
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::rules::{heal_range, hero_weapon, player_accuracy, starter_equipment};
    use deadroll_combat::{
        mod_turns, preview_range, resolve_hit, Balance, HitInput, Mod, ModFilter, ModKind, Outcome,
        SeededRoll, SupportBalance,
    };
    use deadroll_mapcheck::roster::ids;
    use deadroll_skillcheck::{derive, presets, SkillBlocks, SkillExt};
    use deadroll_skillcheck::{Category, Effect, Purpose};
    use deadroll_types::CharacterData;
    use deadroll_types::StateKind;
    use deadroll_types::{Aspect, Class, Race, Source};

    pub(crate) const NONE: TimedMods = [deadroll_combat::TimedMod {
        kind: deadroll_combat::ModKind::Haste,
        aspect: Aspect::Fire,
        turns: 0,
        power_bp: 0,
        source: 0,
    }; deadroll_combat::MODS_MAX];

    fn character(race: Race, class: Class, predisposition: Aspect) -> CharacterData {
        CharacterData {
            version: 1,
            owner: Default::default(),
            index: 0,
            nft: None,
            race,
            class,
            level: 1,
            xp: 0,
            stats: [10, 10, 10, 10],
            predisposition,
            predisposition_tier: 14,
            hp_max: 30,
            alive: true,
            in_session: None,
            appearance: Default::default(),
            state_version: 0,
            bump: 255,
        }
    }

    /// `c` wearing its class starter kit, as `create_character` gives it.
    pub(crate) fn kit(c: &CharacterData) -> HeroStats {
        HeroStats::new(c, 14).with_equipment(starter_equipment(c.class))
    }

    /// A skill as the participation holds it (`set_ready` copies it from the character).
    fn record(blocks: SkillBlocks) -> SkillStats {
        let d = derive(&blocks);
        SkillStats {
            blocks,
            shares_bp: d.shares_bp,
            power_bp: d.power_bp,
            ext: SkillExt::MELEE,
            ..SkillStats::default()
        }
    }

    fn monster(id: u8) -> Monster {
        Monster {
            bestiary_id: id,
            level: 1,
            hp: 20,
            hp_max: 20,
            alive: true,
            ..Monster::default()
        }
    }

    /// Every demo hit shape: both demo characters (weapon and each usable preset skill) against
    /// all 5 P0 monsters, and each monster's natural attack against the warrior.
    pub(crate) fn preset_inputs() -> Vec<HitInput> {
        let warrior = character(Race::Human, Class::Warrior, Aspect::Piercing);
        let mage = character(Race::ForestElf, Class::Mage, Aspect::Cold);
        let mut out = Vec::new();
        for (c, class) in [(&warrior, Class::Warrior), (&mage, Class::Mage)] {
            let mut skills = vec![None];
            for preset in presets::for_class(class) {
                let skill = record(preset.blocks);
                if skill_use(&skill) == SkillUse::Ok && skill.blocks.purpose == Purpose::Damage {
                    skills.push(Some(skill));
                }
            }
            for skill in &skills {
                for id in 0..=4 {
                    out.push(HitInput {
                        attacker: player_attacker(
                            0,
                            &kit(c),
                            skill.as_ref(),
                            &NONE,
                            &Default::default(),
                        ),
                        defender: monster_defender(
                            &monster(id),
                            &crate::state::test_support::defense(id),
                            &NONE,
                        )
                        .unwrap(),
                    });
                }
            }
        }
        let target = player_defender(&kit(&warrior), &[ActiveState::default(); STATES_MAX], &NONE);
        for id in 0..=4 {
            out.push(HitInput {
                attacker: monster_attacker(
                    4,
                    &monster(id),
                    &crate::state::test_support::defense(id),
                    &NONE,
                )
                .unwrap(),
                defender: target,
            });
        }
        out
    }

    #[test]
    fn warrior_and_mage_presets_resolve() {
        let cases = [
            (
                character(Race::Human, Class::Warrior, Aspect::Piercing),
                Class::Warrior,
            ),
            (
                character(Race::ForestElf, Class::Mage, Aspect::Cold),
                Class::Mage,
            ),
        ];
        for (c, class) in cases {
            for preset in presets::for_class(class) {
                let skill = record(preset.blocks);
                if skill_use(&skill) != SkillUse::Ok || skill.blocks.purpose != Purpose::Damage {
                    continue;
                }
                let attacker =
                    player_attacker(0, &kit(&c), Some(&skill), &NONE, &Default::default());
                assert_eq!(attacker.tints[0].aspect, c.predisposition);
                assert_eq!(attacker.added[0].aspect, c.predisposition);
                for id in 0..=4 {
                    let input = HitInput {
                        attacker,
                        defender: monster_defender(
                            &monster(id),
                            &crate::state::test_support::defense(id),
                            &NONE,
                        )
                        .unwrap(),
                    };
                    let (lo, hi) = preview_range(&input, &Balance::DEFAULT);
                    assert!(lo >= 1 && hi >= lo, "{class:?} vs {id}: {lo}..{hi}");
                    for seed in 0..20 {
                        let r = resolve_hit(
                            &input,
                            &Balance::DEFAULT,
                            &crate::state::test_support::tables(),
                            &mut SeededRoll::new(seed),
                        );
                        if r.outcome == Outcome::Hit {
                            assert!(r.damage >= 1);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn basic_attack_uses_the_worn_weapon_and_class_source() {
        let c = character(Race::Human, Class::Warrior, Aspect::Piercing);
        let a = player_attacker(2, &kit(&c), None, &NONE, &Default::default());
        assert_eq!(a.slot, 2);
        assert_eq!(a.skill.aspect_count, 0);
        assert_eq!(a.skill.source, Source::Combat);
        assert_eq!(a.skill.form, Form::Strike);
        assert_eq!(a.weapon, hero_weapon(&kit(&c)));
        assert_eq!((a.weapon.min, a.weapon.max), (4, 8));
        // Bare hands: nothing worn.
        let bare = player_attacker(2, &HeroStats::new(&c, 14), None, &NONE, &Default::default());
        assert_eq!(
            (bare.weapon.min, bare.weapon.max),
            (BALANCE.items.unarmed[0], BALANCE.items.unarmed[1])
        );
        assert_eq!(
            a.increased[0],
            Mod::new(ModFilter::Source(Source::Combat), 2_000)
        );
        assert_eq!(player_form(&kit(&c), None), Form::Strike);
        // A sword strikes in melee; a staff and a wand shoot (docs/SKILL_BUILDER.md 3.1).
        assert_eq!(player_range(&kit(&c), None), RangeTag::Melee);
        let mage = character(Race::Human, Class::Mage, Aspect::Blunt);
        assert_eq!(player_form(&kit(&mage), None), Form::Projectile);
        assert_eq!(player_range(&kit(&mage), None), RangeTag::Ranged);
        assert_eq!(
            action_range(&starter_equipment(Class::Support).0, None),
            RangeTag::Ranged
        );
    }

    #[test]
    fn monsters_attack_and_defend() {
        let c = character(Race::Human, Class::Warrior, Aspect::Piercing);
        let target = player_defender(&kit(&c), &[ActiveState::default(); STATES_MAX], &NONE);
        // Class armor 20 + leather armor 10.
        assert_eq!((target.defense, target.armor), (12, 30));
        let bare = player_defender(
            &HeroStats::new(&c, 14),
            &[ActiveState::default(); STATES_MAX],
            &NONE,
        );
        assert_eq!(bare.armor, 20);
        for id in 0..=4 {
            let a = monster_attacker(
                4,
                &monster(id),
                &crate::state::test_support::defense(id),
                &NONE,
            )
            .unwrap();
            let (lo, hi) = preview_range(
                &HitInput {
                    attacker: a,
                    defender: target,
                },
                &Balance::DEFAULT,
            );
            // The preview has no minimum-damage floor (resolve_hit does): only the top is >= 1.
            assert!(hi >= 1 && hi >= lo, "{id}: {lo}..{hi}");
        }
        assert!(monster_attacker(
            4,
            &monster(9),
            &crate::state::test_support::defense(9),
            &NONE
        )
        .is_none());
        assert!(
            monster_defender(&monster(9), &crate::state::test_support::defense(9), &NONE).is_none()
        );
        let wolf = monster_defender(
            &monster(ids::WOLF),
            &crate::state::test_support::defense(ids::WOLF),
            &NONE,
        )
        .unwrap();
        assert_eq!(wolf.resist_bp[Aspect::Cold as usize], 5_000);
    }

    #[test]
    fn states_carry_into_the_hit() {
        let mut m = monster(ids::GOBLIN);
        m.states[0] = ActiveState {
            kind: deadroll_types::StateKind::Burning,
            turns: 2,
            power: 3,
            source: 0,
        };
        assert_eq!(
            monster_defender(
                &m,
                &crate::state::test_support::defense(m.bestiary_id),
                &NONE
            )
            .unwrap()
            .states,
            m.states
        );
    }

    #[test]
    fn buffs_and_debuffs_reach_the_hit() {
        use deadroll_combat::{apply_mod, mod_turns, ModKind, TimedMod};
        let warrior = character(Race::Human, Class::Warrior, Aspect::Piercing);
        let timed = |kind, power_bp| TimedMod {
            kind,
            aspect: Aspect::Blunt,
            turns: mod_turns(2),
            power_bp,
            source: 1,
        };
        let mut mods = NONE;
        apply_mod(&mut mods, timed(ModKind::Empowerment, 3_000));
        apply_mod(&mut mods, timed(ModKind::Weakening, 2_000));
        apply_mod(&mut mods, timed(ModKind::Protection, 4_000));
        apply_mod(&mut mods, timed(ModKind::Vulnerability, 5_000));

        let a = player_attacker(0, &kit(&warrior), None, &mods, &Default::default());
        // Slot 0 is the class increase, 1-3 the core stats (Strength on martial strikes first),
        // the buff takes the next one.
        assert_eq!(
            a.increased[1].filter,
            ModFilter::SourceForm(Source::Combat, Form::Strike)
        );
        assert_eq!(a.increased[4], Mod::new(ModFilter::Any, 3_000));
        assert_eq!(a.weaken_bp, 2_000);
        let d = player_defender(&kit(&warrior), &[ActiveState::default(); STATES_MAX], &mods);
        assert!(d.resist_bp.iter().all(|&r| r == 4_000));
        assert_eq!(d.vuln_bp[Aspect::Blunt as usize], 5_000);

        let wolf = monster(ids::WOLF);
        let m = monster_defender(
            &wolf,
            &crate::state::test_support::defense(wolf.bestiary_id),
            &mods,
        )
        .unwrap();
        // Protection adds to the bestiary resistance.
        assert_eq!(m.resist_bp[Aspect::Cold as usize], 9_000);
        assert_eq!(m.resist_bp[Aspect::Fire as usize], 4_000);
        let bite = monster_attacker(
            4,
            &wolf,
            &crate::state::test_support::defense(wolf.bestiary_id),
            &mods,
        )
        .unwrap();
        assert_eq!(bite.increased[0], Mod::new(ModFilter::Any, 3_000));
        assert_eq!(bite.weaken_bp, 2_000);
    }

    #[test]
    fn every_active_preset_is_usable_on_its_side() {
        for class in [Class::Warrior, Class::Mage, Class::Support] {
            for preset in presets::for_class(class) {
                let skill = record(preset.blocks);
                let expected = match preset.blocks.category {
                    Category::Active => SkillUse::Ok,
                    _ => SkillUse::NotActive,
                };
                assert_eq!(skill_use(&skill), expected, "{}", preset.name);
                let side = match preset.blocks.purpose {
                    Purpose::Buff => Side::Ally,
                    Purpose::Damage | Purpose::Debuff => Side::Enemy,
                };
                assert_eq!(target_side(Some(&skill)), side, "{}", preset.name);
            }
        }
        assert_eq!(target_side(None), Side::Enemy);
        // A buff without an effect is refused.
        let mut empty = record(presets::SUPPORT[0].blocks);
        empty.blocks.effect = Effect::None;
        assert_eq!(skill_use(&empty), SkillUse::NoEffect);
        let mut heal = record(presets::SUPPORT[0].blocks);
        heal.blocks.effect = Effect::Heal;
        assert_eq!(
            (skill_use(&heal), target_side(Some(&heal))),
            (SkillUse::Ok, Side::Ally)
        );
    }

    #[test]
    fn every_effect_has_an_action() {
        assert_eq!(effect_action(Effect::None), None);
        for effect in Effect::ALL.iter().skip(1) {
            assert!(effect_action(*effect).is_some(), "{effect:?}");
        }
        assert_eq!(effect_action(Effect::Heal), Some(EffectAction::Heal));
        assert_eq!(effect_action(Effect::Freeze), Some(EffectAction::Freeze));
        assert_eq!(
            effect_action(Effect::Vulnerability),
            Some(EffectAction::Mod(ModKind::Vulnerability))
        );
    }

    #[test]
    fn heals_stop_at_the_maximum() {
        assert_eq!(heal(10, 30, 8), 18);
        assert_eq!(heal(25, 30, 8), 30);
        assert_eq!(heal(30, 30, 8), 30);
        assert_eq!(heal(u32::MAX - 1, u32::MAX, u32::MAX), u32::MAX);
        // A maximum below the HP (never written) does not take HP away.
        assert_eq!(heal(12, 0, 5), 12);
    }

    #[test]
    fn support_rolls_from_the_caster() {
        let support = character(Race::Human, Class::Support, Aspect::Lightning);
        let quicken = record(presets::SUPPORT[0].blocks);
        let haste = effect_action(quicken.blocks.effect).unwrap();
        let input = support_input(&kit(&support), &quicken, haste, None);
        assert!(!input.hostile);
        // Haste 2_500..5_000 at full power, +20% for Support.
        assert_eq!((input.min, input.max), (3_000, 6_000));
        assert_eq!(input.attack_bonus, player_accuracy(&support.stats));
        let sap = record(presets::SUPPORT[1].blocks);
        let weaken = effect_action(sap.blocks.effect).unwrap();
        let input = support_input(&kit(&support), &sap, weaken, Some(11));
        assert!(input.hostile && input.defense == 11);
        let mut mend = quicken;
        mend.blocks.effect = Effect::Heal;
        let input = support_input(&kit(&support), &mend, EffectAction::Heal, None);
        assert_eq!(
            (input.min, input.max),
            heal_range(
                Class::Support,
                14,
                hero_weapon(&kit(&support)),
                mend.power_bp
            )
        );
        // Hard control has no strength.
        let mage = character(Race::ForestElf, Class::Mage, Aspect::Cold);
        let prison = record(presets::MAGE[2].blocks);
        assert_eq!(
            effect_action(prison.blocks.effect),
            Some(EffectAction::Freeze)
        );
        let input = support_input(&kit(&mage), &prison, EffectAction::Freeze, Some(10));
        assert_eq!((input.min, input.max), (0, 0));
    }

    #[test]
    fn effects_land_as_modifiers_or_states() {
        use deadroll_combat::states::has;
        let mut states = [ActiveState::default(); STATES_MAX];
        let mut mods = NONE;
        let slow = EffectAction::Mod(ModKind::Slow);
        assert_eq!(
            land_effect(slow, 3_000, Aspect::Cold, 2, 0, &mut states, &mut mods),
            Applied::Added
        );
        assert_eq!(
            (
                mods[0].kind,
                mods[0].power_bp,
                mods[0].aspect,
                mods[0].source
            ),
            (ModKind::Slow, 3_000, Aspect::Cold, 2)
        );
        assert_eq!(
            mods[0].turns,
            mod_turns(SupportBalance::DEFAULT.duration(ModKind::Slow))
        );
        assert_eq!(
            land_effect(slow, 1_000, Aspect::Fire, 3, 0, &mut states, &mut mods),
            Applied::Refreshed
        );
        assert_eq!(mods[0].power_bp, 3_000);

        assert_eq!(
            land_effect(
                EffectAction::Stun,
                0,
                Aspect::Blunt,
                2,
                0,
                &mut states,
                &mut mods
            ),
            Applied::Added
        );
        assert!(has(&states, StateKind::Stun));
        assert_eq!(
            land_effect(
                EffectAction::Freeze,
                0,
                Aspect::Cold,
                2,
                0,
                &mut states,
                &mut mods
            ),
            Applied::Froze
        );
        assert!(has(&states, StateKind::Freeze));
        assert_eq!(
            land_effect(
                EffectAction::Heal,
                9,
                Aspect::Cold,
                2,
                0,
                &mut states,
                &mut mods
            ),
            Applied::None
        );
    }

    #[test]
    fn damage_never_goes_below_zero() {
        assert_eq!(take_damage(10, 3), (7, false));
        assert_eq!(take_damage(10, 10), (0, true));
        assert_eq!(take_damage(10, u32::MAX), (0, true));
        assert_eq!(take_damage(0, 5), (0, false));
    }
}

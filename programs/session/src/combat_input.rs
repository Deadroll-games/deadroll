//! Builds `deadroll_combat` hit inputs from session and character state. Pure, no accounts.
//!
//! Numbers come from `rules.rs` (placeholders until the registry tables, L4).
use deadroll_character::state::SkillRecord;
use deadroll_combat::{
    apply_mod, apply_state, apply_to_attacker, apply_to_defender, mod_range, mod_turns,
    states::freeze, Applied, Attacker, Balance, Carrier, Defender, Mod, ModFilter, ModKind, Skill,
    States, SupportBalance, SupportInput, TimedMod, TimedMods, Tint,
};
use deadroll_skillcheck::{Category, Effect, Purpose};
use deadroll_types::{ActiveState, Aspect, CharacterData, Form, Monster, StateKind, STATES_MAX};

use crate::rules::{
    class_increase, class_source, class_weapon, heal_range, monster_profile, player_attack_bonus,
    player_crit_bp, player_defense, predisposition_added, support_increase, MonsterProfile,
    BASIC_STATE_CHANCE_BP, SKILL_STATE_CHANCE_BP,
};

/// Why a skill cannot be used with `act`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillUse {
    Ok,
    /// Reactive and passive skills are not actions.
    NotActive,
    /// A buff or debuff without an effect (never passes the builder; the program checks anyway).
    NoEffect,
}

pub fn skill_use(skill: &SkillRecord) -> SkillUse {
    if skill.blocks.category != Category::Active {
        SkillUse::NotActive
    } else if skill.blocks.purpose != Purpose::Damage
        && effect_action(skill.blocks.effect).is_none()
    {
        SkillUse::NoEffect
    } else {
        SkillUse::Ok
    }
}

/// Which side an action targets (docs/SKILL_BUILDER.md, section 2: a buff goes on the caster or
/// an ally, a debuff on an enemy).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// A monster: the weapon, damage and debuff skills.
    Enemy,
    /// A player of the party, the caster included: heal and buff skills.
    Ally,
}

/// The side of `skill` (`None` = the weapon's basic attack).
pub fn target_side(skill: Option<&SkillRecord>) -> Side {
    match skill.map(|s| s.blocks.purpose) {
        Some(Purpose::Buff) => Side::Ally,
        _ => Side::Enemy,
    }
}

/// What a skill's effect does to its target (docs/SKILL_BUILDER.md, section 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectAction {
    /// HP back, up to the maximum.
    Heal,
    /// A timed modifier (`deadroll_combat::support`).
    Mod(ModKind),
    /// Hard control: the existing states.
    Stun,
    Freeze,
}

/// `None` for `Effect::None` (pure damage).
pub fn effect_action(effect: Effect) -> Option<EffectAction> {
    Some(match effect {
        Effect::None => return None,
        Effect::Weakening => EffectAction::Mod(ModKind::Weakening),
        Effect::Slow => EffectAction::Mod(ModKind::Slow),
        Effect::Vulnerability => EffectAction::Mod(ModKind::Vulnerability),
        Effect::Stun => EffectAction::Stun,
        Effect::Freeze => EffectAction::Freeze,
        Effect::Haste => EffectAction::Mod(ModKind::Haste),
        Effect::Empowerment => EffectAction::Mod(ModKind::Empowerment),
        Effect::Protection => EffectAction::Mod(ModKind::Protection),
        Effect::Heal => EffectAction::Heal,
    })
}

/// Form of a player attack: the skill's, or a strike for the weapon's basic attack.
pub fn player_form(skill: Option<&SkillRecord>) -> Form {
    skill.map_or(Form::Strike, |s| s.blocks.form)
}

/// A player attacking with `skill` (`None` = the weapon's basic attack). `unit` is the
/// turn-order id, written as the source of applied states. `mods`: the player's buffs and debuffs.
pub fn player_attacker(
    unit: u8,
    c: &CharacterData,
    skill: Option<&SkillRecord>,
    mods: &TimedMods,
) -> Attacker {
    let source = skill.map_or(class_source(c.class), |s| s.blocks.source);
    let combat_skill = match skill {
        Some(s) => Skill {
            aspects: s.blocks.aspects,
            aspect_count: s.blocks.aspect_count,
            shares_bp: s.shares_bp,
            power_bp: s.power_bp,
            source,
            form: s.blocks.form,
            state_chance_bp: SKILL_STATE_CHANCE_BP,
        },
        None => Skill::basic(source, Form::Strike, BASIC_STATE_CHANCE_BP),
    };
    let mut attacker = Attacker {
        slot: unit,
        attack_bonus: player_attack_bonus(c.level, &c.stats, source),
        crit_chance_bp: player_crit_bp(&c.stats),
        weapon: class_weapon(c.class),
        skill: combat_skill,
        ..Attacker::default()
    };
    // Race trait: the predisposition adds a flat packet and tints resonances (CHARACTER 6.2).
    attacker.added[0] = predisposition_added(c.predisposition);
    attacker.tints[0] = Tint::new(c.predisposition, c.predisposition_tier, Carrier::Race);
    if let Some((tag, bp)) = class_increase(c.class) {
        attacker.increased[0] = Mod::new(ModFilter::Source(tag), bp);
    }
    apply_to_attacker(&mut attacker, mods);
    attacker
}

/// A player as the target.
pub fn player_defender(
    c: &CharacterData,
    states: &[ActiveState; STATES_MAX],
    mods: &TimedMods,
) -> Defender {
    let (defense, armor) = player_defense(c.class, &c.stats);
    let mut defender = Defender {
        defense,
        armor,
        states: *states,
        ..Defender::default()
    };
    apply_to_defender(&mut defender, mods);
    defender
}

/// A monster attacking with its natural attack. `None` for an unknown bestiary id.
pub fn monster_attacker(unit: u8, m: &Monster, mods: &TimedMods) -> Option<Attacker> {
    let p = monster_profile(m.bestiary_id, m.level)?;
    let mut attacker = Attacker {
        slot: unit,
        attack_bonus: p.attack_bonus,
        weapon: p.weapon,
        skill: Skill::basic(p.source, p.form, BASIC_STATE_CHANCE_BP),
        ..Attacker::default()
    };
    apply_to_attacker(&mut attacker, mods);
    Some(attacker)
}

/// A monster as the target. `None` for an unknown bestiary id.
pub fn monster_defender(m: &Monster, mods: &TimedMods) -> Option<Defender> {
    let p: MonsterProfile = monster_profile(m.bestiary_id, m.level)?;
    let mut defender = Defender {
        defense: p.defense,
        armor: p.armor,
        states: m.states,
        ..Defender::default()
    };
    if let Some((aspect, bp)) = p.resist {
        defender.resist_bp[aspect as usize] = bp;
    }
    apply_to_defender(&mut defender, mods);
    Some(defender)
}

/// Strength range of `action` cast by a `class` with a skill of `power_bp`: HP for a heal, bp for
/// a modifier, none (`(0, 0)`) for hard control.
pub fn effect_range(
    class: deadroll_types::Class,
    power_bp: u32,
    action: EffectAction,
) -> (u32, u32) {
    match action {
        EffectAction::Heal => heal_range(class, power_bp),
        EffectAction::Mod(kind) => mod_range(
            kind,
            power_bp,
            support_increase(class),
            &SupportBalance::DEFAULT,
        ),
        EffectAction::Stun | EffectAction::Freeze => (0, 0),
    }
}

/// The rolls of a heal, buff or debuff cast by `c` with `skill`. `defense`: the enemy's, for a
/// debuff (it rolls to land); `None` for a heal or buff on an ally.
pub fn support_input(
    c: &CharacterData,
    skill: &SkillRecord,
    action: EffectAction,
    defense: Option<u32>,
) -> SupportInput {
    let (min, max) = effect_range(c.class, skill.power_bp, action);
    SupportInput {
        hostile: defense.is_some(),
        attack_bonus: player_attack_bonus(c.level, &c.stats, skill.blocks.source),
        defense: defense.unwrap_or(0),
        crit_chance_bp: player_crit_bp(&c.stats),
        min,
        max,
    }
}

/// Lands `action` (not a heal) of strength `amount` on a unit: a modifier for its duration, or
/// the existing Stun / Freeze state. `aspect`: the skill's; `source`: the caster's turn-order id.
pub fn land_effect(
    action: EffectAction,
    amount: u32,
    aspect: Aspect,
    source: u8,
    states: &mut States,
    mods: &mut TimedMods,
) -> Applied {
    let balance = &Balance::DEFAULT;
    match action {
        EffectAction::Mod(kind) => apply_mod(
            mods,
            TimedMod {
                kind,
                aspect,
                turns: mod_turns(SupportBalance::DEFAULT.duration(kind)),
                power_bp: amount,
                source,
            },
        ),
        EffectAction::Stun => apply_state(
            states,
            ActiveState {
                kind: StateKind::Stun,
                turns: balance.turns(StateKind::Stun),
                power: 0,
                source,
            },
        ),
        EffectAction::Freeze => freeze(states, source, balance),
        EffectAction::Heal => Applied::None,
    }
}

/// HP after a heal: up to the maximum, never down (docs/COMBAT_MATH.md, 11).
pub fn heal(hp: u32, hp_max: u32, amount: u32) -> u32 {
    hp.saturating_add(amount).min(hp_max).max(hp)
}

/// HP after `damage`, and whether that killed (HP never goes below 0).
pub fn take_damage(hp: u32, damage: u32) -> (u32, bool) {
    let left = hp.saturating_sub(damage);
    (left, hp > 0 && left == 0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use deadroll_combat::{preview_range, resolve_hit, Balance, HitInput, Outcome, SeededRoll};
    use deadroll_mapcheck::roster::ids;
    use deadroll_skillcheck::{derive, presets, SkillBlocks};
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

    fn record(blocks: SkillBlocks) -> SkillRecord {
        let d = derive(&blocks);
        SkillRecord {
            blocks,
            shares_bp: d.shares_bp,
            power_bp: d.power_bp,
            ..SkillRecord::default()
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
                        attacker: player_attacker(0, c, skill.as_ref(), &NONE),
                        defender: monster_defender(&monster(id), &NONE).unwrap(),
                    });
                }
            }
        }
        let target = player_defender(&warrior, &[ActiveState::default(); STATES_MAX], &NONE);
        for id in 0..=4 {
            out.push(HitInput {
                attacker: monster_attacker(4, &monster(id), &NONE).unwrap(),
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
                let attacker = player_attacker(0, &c, Some(&skill), &NONE);
                assert_eq!(attacker.tints[0].aspect, c.predisposition);
                assert_eq!(attacker.added[0].aspect, c.predisposition);
                for id in 0..=4 {
                    let input = HitInput {
                        attacker,
                        defender: monster_defender(&monster(id), &NONE).unwrap(),
                    };
                    let (lo, hi) = preview_range(&input, &Balance::DEFAULT);
                    assert!(lo >= 1 && hi >= lo, "{class:?} vs {id}: {lo}..{hi}");
                    for seed in 0..20 {
                        let r = resolve_hit(&input, &Balance::DEFAULT, &mut SeededRoll::new(seed));
                        if r.outcome == Outcome::Hit {
                            assert!(r.damage >= 1);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn basic_attack_uses_class_weapon_and_source() {
        let c = character(Race::Human, Class::Warrior, Aspect::Piercing);
        let a = player_attacker(2, &c, None, &NONE);
        assert_eq!(a.slot, 2);
        assert_eq!(a.skill.aspect_count, 0);
        assert_eq!(a.skill.source, Source::Combat);
        assert_eq!(a.skill.form, Form::Strike);
        assert_eq!(a.weapon, class_weapon(Class::Warrior));
        assert_eq!(
            a.increased[0],
            Mod::new(ModFilter::Source(Source::Combat), 2_000)
        );
        assert_eq!(player_form(None), Form::Strike);
    }

    #[test]
    fn monsters_attack_and_defend() {
        let c = character(Race::Human, Class::Warrior, Aspect::Piercing);
        let target = player_defender(&c, &[ActiveState::default(); STATES_MAX], &NONE);
        assert_eq!((target.defense, target.armor), (12, 30));
        for id in 0..=4 {
            let a = monster_attacker(4, &monster(id), &NONE).unwrap();
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
        assert!(monster_attacker(4, &monster(9), &NONE).is_none());
        assert!(monster_defender(&monster(9), &NONE).is_none());
        let wolf = monster_defender(&monster(ids::WOLF), &NONE).unwrap();
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
        assert_eq!(monster_defender(&m, &NONE).unwrap().states, m.states);
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

        let a = player_attacker(0, &warrior, None, &mods);
        // Slot 0 is the class increase, the buff takes the next one.
        assert_eq!(a.increased[1], Mod::new(ModFilter::Any, 3_000));
        assert_eq!(a.weaken_bp, 2_000);
        let d = player_defender(&warrior, &[ActiveState::default(); STATES_MAX], &mods);
        assert!(d.resist_bp.iter().all(|&r| r == 4_000));
        assert_eq!(d.vuln_bp[Aspect::Blunt as usize], 5_000);

        let wolf = monster(ids::WOLF);
        let m = monster_defender(&wolf, &mods).unwrap();
        // Protection adds to the bestiary resistance.
        assert_eq!(m.resist_bp[Aspect::Cold as usize], 9_000);
        assert_eq!(m.resist_bp[Aspect::Fire as usize], 4_000);
        let bite = monster_attacker(4, &wolf, &mods).unwrap();
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
        let input = support_input(&support, &quicken, haste, None);
        assert!(!input.hostile);
        // Haste 2_500..5_000 at full power, +20% for Support.
        assert_eq!((input.min, input.max), (3_000, 6_000));
        assert_eq!(
            input.attack_bonus,
            player_attack_bonus(1, &support.stats, quicken.blocks.source)
        );
        let sap = record(presets::SUPPORT[1].blocks);
        let weaken = effect_action(sap.blocks.effect).unwrap();
        let input = support_input(&support, &sap, weaken, Some(11));
        assert!(input.hostile && input.defense == 11);
        let mut mend = quicken;
        mend.blocks.effect = Effect::Heal;
        let input = support_input(&support, &mend, EffectAction::Heal, None);
        assert_eq!(
            (input.min, input.max),
            heal_range(Class::Support, mend.power_bp)
        );
        // Hard control has no strength.
        let mage = character(Race::ForestElf, Class::Mage, Aspect::Cold);
        let prison = record(presets::MAGE[2].blocks);
        assert_eq!(
            effect_action(prison.blocks.effect),
            Some(EffectAction::Freeze)
        );
        let input = support_input(&mage, &prison, EffectAction::Freeze, Some(10));
        assert_eq!((input.min, input.max), (0, 0));
    }

    #[test]
    fn effects_land_as_modifiers_or_states() {
        use deadroll_combat::states::has;
        let mut states = [ActiveState::default(); STATES_MAX];
        let mut mods = NONE;
        let slow = EffectAction::Mod(ModKind::Slow);
        assert_eq!(
            land_effect(slow, 3_000, Aspect::Cold, 2, &mut states, &mut mods),
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
            land_effect(slow, 1_000, Aspect::Fire, 3, &mut states, &mut mods),
            Applied::Refreshed
        );
        assert_eq!(mods[0].power_bp, 3_000);

        assert_eq!(
            land_effect(
                EffectAction::Stun,
                0,
                Aspect::Blunt,
                2,
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

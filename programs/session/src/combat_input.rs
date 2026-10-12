//! Builds `deadroll_combat` hit inputs from session and character state. Pure, no accounts.
//!
//! Numbers come from `rules.rs` (placeholders until the registry tables, L4).
use deadroll_character::state::SkillRecord;
use deadroll_combat::{Attacker, Carrier, Defender, Mod, ModFilter, Skill, Tint};
use deadroll_skillcheck::{Category, Purpose};
use deadroll_types::{ActiveState, CharacterData, Form, Monster, STATES_MAX};

use crate::rules::{
    class_increase, class_source, class_weapon, monster_profile, player_attack_bonus,
    player_crit_bp, player_defense, predisposition_added, MonsterProfile, BASIC_STATE_CHANCE_BP,
    SKILL_STATE_CHANCE_BP,
};

/// Why a skill cannot be used with `act`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillUse {
    Ok,
    /// Reactive and passive skills are not actions.
    NotActive,
    /// P0 resolves damage skills only. TODO(post-demo): buff and debuff skills.
    NotDamage,
}

pub fn skill_use(skill: &SkillRecord) -> SkillUse {
    if skill.blocks.category != Category::Active {
        SkillUse::NotActive
    } else if skill.blocks.purpose != Purpose::Damage {
        SkillUse::NotDamage
    } else {
        SkillUse::Ok
    }
}

/// Form of a player attack: the skill's, or a strike for the weapon's basic attack.
pub fn player_form(skill: Option<&SkillRecord>) -> Form {
    skill.map_or(Form::Strike, |s| s.blocks.form)
}

/// A player attacking with `skill` (`None` = the weapon's basic attack). `unit` is the
/// turn-order id, written as the source of applied states.
pub fn player_attacker(unit: u8, c: &CharacterData, skill: Option<&SkillRecord>) -> Attacker {
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
    attacker
}

/// A player as the target.
pub fn player_defender(c: &CharacterData, states: &[ActiveState; STATES_MAX]) -> Defender {
    let (defense, armor) = player_defense(c.class, &c.stats);
    Defender {
        defense,
        armor,
        states: *states,
        ..Defender::default()
    }
}

/// A monster attacking with its natural attack. `None` for an unknown bestiary id.
pub fn monster_attacker(unit: u8, m: &Monster) -> Option<Attacker> {
    let p = monster_profile(m.bestiary_id, m.level)?;
    Some(Attacker {
        slot: unit,
        attack_bonus: p.attack_bonus,
        weapon: p.weapon,
        skill: Skill::basic(p.source, p.form, BASIC_STATE_CHANCE_BP),
        ..Attacker::default()
    })
}

/// A monster as the target. `None` for an unknown bestiary id.
pub fn monster_defender(m: &Monster) -> Option<Defender> {
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
    Some(defender)
}

/// HP after `damage`, and whether that killed (HP never goes below 0).
pub fn take_damage(hp: u32, damage: u32) -> (u32, bool) {
    let left = hp.saturating_sub(damage);
    (left, hp > 0 && left == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_combat::{preview_range, resolve_hit, Balance, HitInput, Outcome, SeededRoll};
    use deadroll_mapcheck::roster::ids;
    use deadroll_skillcheck::{derive, presets, SkillBlocks};
    use deadroll_types::{Aspect, Class, Race, Source};

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
                if skill_use(&skill) != SkillUse::Ok {
                    continue;
                }
                let attacker = player_attacker(0, &c, Some(&skill));
                assert_eq!(attacker.tints[0].aspect, c.predisposition);
                assert_eq!(attacker.added[0].aspect, c.predisposition);
                for id in 0..=4 {
                    let input = HitInput {
                        attacker,
                        defender: monster_defender(&monster(id)).unwrap(),
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
        let a = player_attacker(2, &c, None);
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
        let target = player_defender(&c, &[ActiveState::default(); STATES_MAX]);
        assert_eq!((target.defense, target.armor), (12, 30));
        for id in 0..=4 {
            let a = monster_attacker(4, &monster(id)).unwrap();
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
        assert!(monster_attacker(4, &monster(9)).is_none());
        assert!(monster_defender(&monster(9)).is_none());
        let wolf = monster_defender(&monster(ids::WOLF)).unwrap();
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
        assert_eq!(monster_defender(&m).unwrap().states, m.states);
    }

    #[test]
    fn damage_never_goes_below_zero() {
        assert_eq!(take_damage(10, 3), (7, false));
        assert_eq!(take_damage(10, 10), (0, true));
        assert_eq!(take_damage(10, u32::MAX), (0, true));
        assert_eq!(take_damage(0, 5), (0, false));
    }
}

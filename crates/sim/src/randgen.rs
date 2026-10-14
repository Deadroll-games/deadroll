//! Random heroes for the "random builds" mode. Every list comes from the enums' `ALL`, and a skill
//! is kept only when `skillcheck::validate_starting` accepts it for the class.
use deadroll_combat::{RollSource, SeededRoll};
use deadroll_rules::{player::predisposition_allowed, BalanceConfig};
use deadroll_skillcheck::{
    validate_starting, Category, Effect, Purpose, SkillBlocks, Target, Trigger, ACTIVE_MAX,
    SKILL_ASPECTS_MAX,
};
use deadroll_types::{Aspect, Class, Form, Race, Source, STATS_COUNT};

use crate::{
    named::Named,
    presets::{HeroPreset, SkillPreset},
};

fn pick<T: Copy>(rng: &mut SeededRoll, items: &[T]) -> T {
    items[(rng.next_u64() % items.len() as u64) as usize]
}

/// A random active skill the class may start with; `None` after many rejected tries.
pub fn random_skill(rng: &mut SeededRoll, class: Class) -> Option<SkillBlocks> {
    for _ in 0..1_000 {
        let count = 1 + (rng.next_u64() % SKILL_ASPECTS_MAX as u64) as usize;
        let mut aspects = [Aspect::default(); SKILL_ASPECTS_MAX];
        let mut pool: Vec<Aspect> = Aspect::all().to_vec();
        for slot in aspects.iter_mut().take(count) {
            let i = (rng.next_u64() % pool.len() as u64) as usize;
            *slot = pool.swap_remove(i);
        }
        let blocks = SkillBlocks {
            category: Category::Active,
            purpose: pick(rng, Purpose::all()),
            source: pick(rng, Source::all()),
            form: pick(rng, Form::all()),
            target: pick(rng, Target::all()),
            aspects,
            aspect_count: count as u8,
            effect: pick(rng, Effect::all()),
            trigger: Trigger::None,
            aura_radius: 0,
        };
        if validate_starting(&blocks, class).is_ok() {
            return Some(blocks);
        }
    }
    None
}

/// A random level-1 hero by the creation rules, with up to `ACTIVE_MAX` random active skills
/// (inline, named `r1`..).
pub fn random_hero(rng: &mut SeededRoll, cfg: &BalanceConfig, class: Option<Class>) -> HeroPreset {
    let t = &cfg.players;
    let race = pick(rng, Race::all());
    let class = class.unwrap_or_else(|| pick(rng, Class::all()));
    let allowed: Vec<Aspect> = Aspect::all()
        .iter()
        .copied()
        .filter(|&a| predisposition_allowed(race, a))
        .collect();
    let predisposition = if allowed.is_empty() {
        pick(rng, Aspect::all())
    } else {
        pick(rng, &allowed)
    };
    let mut stats = [t.stat_base; STATS_COUNT];
    for _ in 0..t.stat_points_start {
        let open: Vec<usize> = (0..STATS_COUNT)
            .filter(|&i| stats[i] < t.stat_max_start)
            .collect();
        if open.is_empty() {
            break;
        }
        stats[pick(rng, &open)] += 1;
    }
    let inline_skills: Vec<SkillPreset> = (0..ACTIVE_MAX)
        .filter_map(|i| {
            random_skill(rng, class).map(|b| SkillPreset::from_blocks(&format!("r{}", i + 1), &b))
        })
        .collect();
    HeroPreset {
        name: format!("random {} {}", race.name(), class.name()),
        race,
        class,
        level: 1,
        stats,
        predisposition,
        tier: t.start_tier,
        skills: inline_skills.iter().map(|s| s.name.clone()).collect(),
        inline_skills,
        equipment: None,
    }
}

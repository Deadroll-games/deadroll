//! The builder rules of a map from the registry table bodies (patch 13): what `finalize_map` and
//! the website's mapcheck WASM give `deadroll_mapcheck::check_map`. One decoder of the bestiary,
//! monster skills and rewards records, so the builder's verdict is the chain's.

use deadroll_mapcheck::{loot::TableRules, roster::SpawnRules};
use deadroll_skillcheck::monster::{MonsterSkill, RECORD_SIZE as SKILL_RECORD};

use crate::config::MonsterScaling;
use crate::loot::{self, LootTable};
use crate::monster::{record, MonsterTemplate};

fn record_of(body: &[u8], size: usize, index: usize) -> Option<&[u8]> {
    body.get(index * size..(index + 1) * size)
}

/// The builder rules of bestiary id `id`: its template and the categories of its pool skills
/// (`None`: no such template, or a pool skill missing).
pub fn spawn_rules(
    bestiary: &[u8],
    monster_skills: &[u8],
    id: u8,
    s: &MonsterScaling,
) -> Option<SpawnRules> {
    let template = MonsterTemplate::from_record(record_of(bestiary, record::SIZE, id as usize)?)?;
    template.spawn_rules(
        |i| {
            let r = record_of(monster_skills, SKILL_RECORD, i)?;
            // `write_table` validated it; only the category is read.
            (r[0] == 1)
                .then(|| MonsterSkill::decode_session(&r[1..]))
                .flatten()
                .map(|k| k.blocks.category)
        },
        s,
    )
}

/// The builder view of reward table `id` (`None`: no such table).
pub fn table_rules(rewards: &[u8], id: u8) -> Option<TableRules> {
    record_of(rewards, loot::record::SIZE, id as usize)
        .and_then(LootTable::from_record)
        .map(|t| t.rules())
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::{registry::Registry, BalanceConfig};

    #[test]
    fn the_same_rules_as_the_registry_view() {
        let reg = Registry::shipped().unwrap();
        let cfg = &BalanceConfig::DEFAULT.monsters;
        for id in 0..40u8 {
            assert_eq!(
                spawn_rules(&reg.bestiary, &reg.monster_skills, id, cfg),
                reg.spawn_rules(id, cfg),
                "bestiary {id}"
            );
            assert_eq!(
                table_rules(&reg.rewards, id),
                reg.reward_table(id).map(|t| t.rules()),
                "rewards {id}"
            );
        }
    }
}

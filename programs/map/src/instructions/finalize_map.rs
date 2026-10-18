use anchor_lang::prelude::*;
use deadroll_mapcheck::{check_map, roster::SpawnRules};
use deadroll_registry::state::{
    records::BESTIARY_MAX,
    view::{record, table_body},
    TableKind,
};
use deadroll_rules::{config::BalanceConfig, MonsterTemplate};
use deadroll_skillcheck::monster::MonsterSkill;
use deadroll_types::{seeds, MapLayoutData, MapStatus};
use solana_sha256_hasher::hash;

use crate::{
    errors::MapError,
    events::MapFinalized,
    state::{Map, MapLayout},
};

/// Registry `["bestiary"]` and `["monster_skills"]` tables (checked against their derivation in
/// the tests).
pub const BESTIARY_TABLE: Pubkey = pubkey!("3uJyWiz7NeVekP4w1iZkKxKnJ5pMbxC6ciz7kxN3MZtM");
pub const MONSTER_SKILLS_TABLE: Pubkey = pubkey!("NQNEuKDUZhACnTJ9sD3dW49BoiPtJPatp6bpY61r81p");

#[derive(Accounts)]
pub struct FinalizeMap<'info> {
    pub dm: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::MAP, map.core.dm.as_ref(), &map.core.index.to_le_bytes()],
        bump = map.core.bump,
        constraint = map.core.dm == dm.key() @ MapError::NotDm,
        constraint = map.core.status == MapStatus::Draft @ MapError::NotDraft,
    )]
    pub map: Account<'info, Map>,
    #[account(seeds = [seeds::LAYOUT, map.key().as_ref()], bump = layout.core.bump)]
    pub layout: Box<Account<'info, MapLayout>>,
    /// CHECK: registry `["bestiary"]` table (patch 6): roles, minimum tiers and skill pools of the
    /// spawns, read in place by `deadroll_registry::state::view`.
    #[account(address = BESTIARY_TABLE @ MapError::InvalidRegistryTable)]
    pub bestiary: UncheckedAccount<'info>,
    /// CHECK: registry `["monster_skills"]` table: the categories of the pool skills.
    #[account(address = MONSTER_SKILLS_TABLE @ MapError::InvalidRegistryTable)]
    pub monster_skills: UncheckedAccount<'info>,
}

/// The builder rules of every bestiary template a spawn names, from the registry (`None`: no
/// such template, or a pool skill missing).
fn templates(
    bestiary: &AccountInfo,
    skills: &AccountInfo,
    layout: &MapLayoutData,
) -> Result<Box<[Option<SpawnRules>; BESTIARY_MAX]>> {
    let beasts = table_body(bestiary, TableKind::Bestiary)
        .map_err(|_| error!(MapError::InvalidRegistryTable))?;
    let pool = table_body(skills, TableKind::MonsterSkills)
        .map_err(|_| error!(MapError::InvalidRegistryTable))?;
    let cfg = &BalanceConfig::DEFAULT;
    let mut out = Box::new([None; BESTIARY_MAX]);
    for spawn in &layout.spawns[..(layout.spawn_count as usize).min(layout.spawns.len())] {
        let id = spawn.bestiary_id as usize;
        if id >= BESTIARY_MAX || out[id].is_some() {
            continue;
        }
        out[id] = record(&beasts, TableKind::Bestiary, id)
            .and_then(MonsterTemplate::from_record)
            .and_then(|t| {
                t.spawn_rules(
                    |i| {
                        let r = record(&pool, TableKind::MonsterSkills, i)?;
                        // `write_table` validated it; only the category is read.
                        (r[0] == 1)
                            .then(|| MonsterSkill::decode_session(&r[1..]))
                            .flatten()
                            .map(|k| k.blocks.category)
                    },
                    &cfg.monsters,
                )
            });
    }
    Ok(out)
}

/// Runs every builder rule, then freezes the map: `layout_hash` = SHA-256 of the layout core
/// bytes (account data `[8 .. 8 + MapLayoutData::SIZE]`).
pub fn handler(ctx: Context<FinalizeMap>, secrets_root: [u8; 32]) -> Result<()> {
    let accounts = ctx.accounts;
    let rules = templates(
        &accounts.bestiary,
        &accounts.monster_skills,
        &accounts.layout.core,
    )?;
    let map = &mut accounts.map;
    check_map(
        map.core.width,
        map.core.height,
        map.tier,
        &accounts.layout.core,
        |id| rules.get(id as usize).copied().flatten(),
    )
    .map_err(MapError::from)?;

    let layout_hash: [u8; 32] = {
        let info = accounts.layout.to_account_info();
        let data = info.try_borrow_data()?;
        hash(&data[8..8 + MapLayoutData::SIZE]).to_bytes()
    };
    map.core.layout_hash = layout_hash;
    // TODO(post-demo): declared secret counts with the commit-reveal trap (L6.5).
    map.core.secrets_root = secrets_root;
    map.core.status = MapStatus::Finalized;

    emit!(MapFinalized {
        map: map.key(),
        dm: map.core.dm,
        layout_hash,
        secrets_root,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_addresses_match_their_derivation() {
        for (seed, address) in [
            (seeds::BESTIARY, BESTIARY_TABLE),
            (seeds::MONSTER_SKILLS, MONSTER_SKILLS_TABLE),
        ] {
            let (pda, _) = Pubkey::find_program_address(&[seed], &deadroll_registry::ID);
            assert_eq!(pda, address);
        }
    }
}

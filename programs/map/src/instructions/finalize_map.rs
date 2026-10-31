use anchor_lang::prelude::*;
use deadroll_mapcheck::{
    check_public,
    loot::{MapLoot, TableRules},
    roster::SpawnRules,
};
use deadroll_registry::state::{
    records::{BESTIARY_MAX, REWARDS_MAX},
    view::table_body,
    TableKind,
};
use deadroll_rules::{config::BalanceConfig, loot::map_rules, map_tables, tiles::tile_rules};
use deadroll_types::{seeds, MapLayoutDataV2, MapStatus};

use crate::state::{decode_arenas, decode_core, decode_declared, off, LAYOUT_SPACE};
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
/// Patch 11: registry `["rewards"]`.
pub const REWARDS_TABLE: Pubkey = pubkey!("FxXF2viQZ2zBSS4tEBFoMEVZWharoUA4PtQgimZCnytK");
/// Patch 13: registry `["tileset"]`.
pub const TILESET_TABLE: Pubkey = pubkey!("GqzUadKRKmJNT2RQAhrTz31qdoMfTGmya3Cfz2vAhXHu");

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
    /// Owner and discriminator checked, never decoded whole (`state::LayoutBytes`): the core, the
    /// arenas and the public secrets are decoded into boxes one at a time.
    #[account(seeds = [seeds::LAYOUT, map.key().as_ref()], bump)]
    pub layout: LazyAccount<'info, MapLayout>,
    /// CHECK: registry `["bestiary"]` table (patch 6): roles, minimum tiers and skill pools of the
    /// spawns, read in place by `deadroll_registry::state::view`.
    #[account(address = BESTIARY_TABLE @ MapError::InvalidRegistryTable)]
    pub bestiary: UncheckedAccount<'info>,
    /// CHECK: registry `["monster_skills"]` table: the categories of the pool skills.
    #[account(address = MONSTER_SKILLS_TABLE @ MapError::InvalidRegistryTable)]
    pub monster_skills: UncheckedAccount<'info>,
    /// CHECK: registry `["rewards"]` table (patch 11): the chest and boss reward tables.
    #[account(address = REWARDS_TABLE @ MapError::InvalidRegistryTable)]
    pub rewards: UncheckedAccount<'info>,
    /// CHECK: registry `["tileset"]` table (patch 13): passability of every tile type, the same
    /// records `open_session` gives movement.
    #[account(address = TILESET_TABLE @ MapError::InvalidRegistryTable)]
    pub tileset: UncheckedAccount<'info>,
}

/// The builder rules of every bestiary template a spawn names, from the registry (`None`: no
/// such template, or a pool skill missing). Decoded by `deadroll_rules::map_tables`, the code the
/// website's mapcheck WASM runs.
fn templates(
    bestiary: &AccountInfo,
    skills: &AccountInfo,
    layout: &MapLayoutDataV2,
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
        out[id] = map_tables::spawn_rules(&beasts, &pool, spawn.bestiary_id, &cfg.monsters);
    }
    Ok(out)
}

/// The builder view of every reward table the layout names (`None`: no such table).
fn reward_tables(
    rewards: &AccountInfo,
    layout: &MapLayoutDataV2,
) -> Result<[Option<TableRules>; REWARDS_MAX]> {
    let body = table_body(rewards, TableKind::Rewards)
        .map_err(|_| error!(MapError::InvalidRegistryTable))?;
    let mut out = [None; REWARDS_MAX];
    let ids = layout.chests().map(|c| c.data).chain([layout.boss_table]);
    for id in ids {
        if let Some(slot) = out.get_mut(id as usize) {
            *slot = map_tables::table_rules(&body, id);
        }
    }
    Ok(out)
}

/// Runs every builder rule on the public part, then freezes the map: `layout_hash` = SHA-256 of
/// the layout bytes after the discriminator up to the encrypted secrets (the core, the arenas,
/// the threats and the declared counts). Patch C: the threats and the declared counts are public
/// (docs/MAP_DESCRIPTION.md 8.3, 17.2, written with the encrypted secrets by `write_secrets`);
/// the packs' compositions and the secrets are checked when a party reveals them. The map must
/// have its encrypted secrets and a root.
pub fn handler(ctx: Context<FinalizeMap>, secrets_root: [u8; 32]) -> Result<()> {
    let accounts = ctx.accounts;
    let (core, arenas, secrets) = {
        let data = accounts.layout.as_ref().try_borrow_data()?;
        require!(
            data.len() >= LAYOUT_SPACE && data[8] == crate::state::LAYOUT_VERSION,
            MapError::InvalidLayout
        );
        (
            decode_core(&data)?,
            decode_arenas(&data)?,
            decode_declared(&data)?,
        )
    };
    let rules = templates(&accounts.bestiary, &accounts.monster_skills, &core)?;
    let tables = reward_tables(&accounts.rewards, &core)?;
    let tiles = tile_rules(
        &table_body(&accounts.tileset, TableKind::Tileset)
            .map_err(|_| error!(MapError::InvalidRegistryTable))?,
    );
    let map = &mut accounts.map;
    let secret_limits = BalanceConfig::DEFAULT
        .secrets
        .limits(map.tier)
        .ok_or(MapError::TierOutOfRange)?;
    let loot = MapLoot {
        min_players: map.min_players,
        rules: map_rules(&BalanceConfig::DEFAULT.loot, map.tier),
        table_of: |id: u8| tables.get(id as usize).copied().flatten(),
    };
    check_public(
        map.core.width,
        map.core.height,
        map.tier,
        &core,
        &arenas,
        &secrets,
        BalanceConfig::DEFAULT.arena.limits(),
        &secret_limits,
        &tiles,
        |id| rules.get(id as usize).copied().flatten(),
        &loot,
    )
    .map_err(MapError::from)?;
    require!(
        secrets_root != [0; 32] && secrets.blob_len > 0,
        MapError::SecretsMissing
    );

    let layout_hash: [u8; 32] = {
        let data = accounts.layout.as_ref().try_borrow_data()?;
        hash(&data[8..off::BLOB_LEN]).to_bytes()
    };
    map.core.layout_hash = layout_hash;
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
            (seeds::REWARDS, REWARDS_TABLE),
            (seeds::TILESET, TILESET_TABLE),
        ] {
            let (pda, _) = Pubkey::find_program_address(&[seed], &deadroll_registry::ID);
            assert_eq!(pda, address);
        }
    }
}

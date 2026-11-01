use anchor_lang::prelude::*;
use deadroll_mapcheck::{
    check_public_map,
    loot::{MapLoot, TableRules},
    roster::SpawnRules,
    wide::{ReachView, Row, FINALIZE_BUDGET},
    MapParts,
};
use deadroll_registry::state::{
    records::{BESTIARY_MAX, REWARDS_MAX},
    view::table_body,
    TableKind,
};
use deadroll_rules::{config::BalanceConfig, loot::map_rules, map_tables};
use deadroll_types::{seeds, LayoutArenas, LayoutCore, MapStatus, MAP_ROW_WORDS};
use solana_sha256_hasher::hash;

use crate::{
    errors::MapError,
    events::{MapChecked, MapFinalized},
    instructions::check_step::{advance, tiles_of},
    state::{LayoutBytes, Map},
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
    /// CHECK: the map's `MapLayout` (seeds), checked by `LayoutBytes::load`, never decoded whole:
    /// the cells are read in place, the core and the arenas decoded into boxes. Patch D: the
    /// reachability check's last share is written in place.
    #[account(mut, seeds = [seeds::LAYOUT, map.key().as_ref()], bump)]
    pub layout: UncheckedAccount<'info>,
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
    layout: &LayoutCore,
) -> Result<Box<[Option<SpawnRules>; BESTIARY_MAX]>> {
    let beasts = table_body(bestiary, TableKind::Bestiary)
        .map_err(|_| error!(MapError::InvalidRegistryTable))?;
    let pool = table_body(skills, TableKind::MonsterSkills)
        .map_err(|_| error!(MapError::InvalidRegistryTable))?;
    let cfg = &BalanceConfig::DEFAULT;
    let mut out = Box::new([None; BESTIARY_MAX]);
    for spawn in layout.spawn_list() {
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
    layout: &LayoutCore,
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
/// the layout's public bytes (`LayoutGeom::hash_range`: the header, the spawns, points, packs,
/// threats, declared counts, arenas and cells; not the encrypted secrets or the check's scratch).
/// Patch C: the threats and the declared counts are public (docs/MAP_DESCRIPTION.md 8.3, 17.2,
/// written with the encrypted secrets by `write_secrets`); the packs' compositions and the
/// secrets are checked when a party reveals them. The map must have its encrypted secrets and a
/// root. Patch D: the reachability check finishes here (a Small map's whole check fits); a bigger
/// map needs its `check_step` calls first. The limits are the tier's times the size class's
/// multipliers (`BalanceConfig.map_size`).
pub fn handler(ctx: Context<FinalizeMap>, secrets_root: [u8; 32]) -> Result<()> {
    let accounts = ctx.accounts;
    let tiles = tiles_of(&accounts.tileset)?;
    let layout = LayoutBytes::load(&accounts.layout)?;
    let g = layout.geom;
    let state = advance(&layout, &tiles, FINALIZE_BUDGET)?;
    emit!(MapChecked {
        map: accounts.map.key(),
        phase: state.phase,
        row: state.row,
    });
    require!(state.done(), MapError::CheckNotDone);
    let h = g.height as usize;
    let mut pass: Vec<Row> = vec![[0; MAP_ROW_WORDS]; h];
    let mut reach: Vec<Row> = vec![[0; MAP_ROW_WORDS]; h];
    layout.load_rows(&mut pass, &mut reach)?;
    drop(pass);
    let (core, secrets) = {
        let data = layout.data()?;
        (Box::new(g.core(&data)), g.secrets(&data))
    };
    let mut arenas = Box::new(LayoutArenas::EMPTY);
    g.arenas_into(&layout.data()?, &mut arenas);
    let rules = templates(&accounts.bestiary, &accounts.monster_skills, &core)?;
    let tables = reward_tables(&accounts.rewards, &core)?;
    let map = &mut accounts.map;
    let cfg = &BalanceConfig::DEFAULT;
    let secret_limits = cfg.map_size.secrets(
        cfg.secrets
            .limits(map.tier)
            .ok_or(MapError::TierOutOfRange)?,
        g.class,
    );
    let loot = MapLoot {
        min_players: map.min_players,
        rules: cfg.map_size.loot(map_rules(&cfg.loot, map.tier), g.class),
        table_of: |id: u8| tables.get(id as usize).copied().flatten(),
    };
    {
        let data = layout.data()?;
        let parts = MapParts {
            board: g.board(&data),
            core: &core,
            spawns: core.spawn_list(),
            arenas: &arenas,
        };
        let view = ReachView {
            rows: &reach,
            width: g.width,
            height: g.height,
        };
        check_public_map(
            &parts,
            map.tier,
            cfg.map_size.scale(g.class),
            &secrets,
            cfg.arena.limits(),
            &secret_limits,
            &tiles,
            |id| rules.get(id as usize).copied().flatten(),
            &loot,
            &view,
        )
        .map_err(MapError::from)?;
    }
    require!(
        secrets_root != [0; 32] && secrets.blob_len > 0,
        MapError::SecretsMissing
    );
    require!(
        (map.core.width, map.core.height) == (g.width, g.height),
        MapError::InvalidLayout
    );

    let layout_hash: [u8; 32] = hash(&layout.data()?[g.hash_range()]).to_bytes();
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

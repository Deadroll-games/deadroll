use anchor_lang::prelude::*;
use deadroll_mapcheck::{
    arena::check_roster,
    limits::tier_limits,
    roster::{RosterTotals, SpawnRules},
};
use deadroll_registry::state::{records::BESTIARY_MAX, view::table_body, TableKind};
use deadroll_rules::map_tables;
use deadroll_types::{seeds, PackRoster, NO_SPAWN, SECRET_SALT_LEN, SPAWNS_MAX};
use session_keys::{session_auth_or, Session, SessionTokenV2};

use crate::{
    auth,
    errors::SessionError,
    events::PackRevealed,
    instructions::open_session::{copy_loot_table, copy_skills},
    rules::BALANCE,
    secrets::{audit_mode, rejected, tiles, verify_pack},
    state::{
        external::{
            read_bestiary, MapLayoutView, BESTIARY_TABLE, MONSTER_SKILLS_TABLE, REWARDS_TABLE,
        },
        ArenaMode, GameSession, Monsters,
    },
};

#[derive(Accounts, Session)]
pub struct RevealPack<'info> {
    /// The DM's main wallet or the DM's session key bound to this party.
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [seeds::SESSION, session.core.map.as_ref()],
        bump = session.core.bump,
    )]
    pub session: Box<Account<'info, GameSession>>,
    /// CHECK: the session's `MapLayout` (address pinned at `open_session`): the pack's public
    /// monster and its arena.
    #[account(address = session.layout @ SessionError::InvalidLayout)]
    pub layout: UncheckedAccount<'info>,
    /// CHECK: `MonsterState` of this session, checked by `Monsters::load` (written in place): the
    /// pack's templates, skills and reward tables are copied there.
    #[account(mut)]
    pub monsters: UncheckedAccount<'info>,
    /// CHECK: registry `["bestiary"]` table, read in place.
    #[account(address = BESTIARY_TABLE @ SessionError::InvalidRegistryTable)]
    pub bestiary: UncheckedAccount<'info>,
    /// CHECK: registry `["monster_skills"]` table, read in place.
    #[account(address = MONSTER_SKILLS_TABLE @ SessionError::InvalidRegistryTable)]
    pub monster_skills: UncheckedAccount<'info>,
    /// CHECK: registry `["rewards"]` table, read in place: the monsters' body tables.
    #[account(address = REWARDS_TABLE @ SessionError::InvalidRegistryTable)]
    pub rewards: UncheckedAccount<'info>,
    #[session(signer = signer, authority = session.core.dm)]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
}

/// The builder rules of every template the roster names (`None`: no such template).
fn templates(
    bestiary: &AccountInfo,
    skills: &AccountInfo,
    roster: &PackRoster,
) -> Result<Box<[Option<SpawnRules>; BESTIARY_MAX]>> {
    let beasts = table_body(bestiary, TableKind::Bestiary)
        .map_err(|_| error!(SessionError::InvalidRegistryTable))?;
    let pool = table_body(skills, TableKind::MonsterSkills)
        .map_err(|_| error!(SessionError::InvalidRegistryTable))?;
    let mut out = Box::new([None; BESTIARY_MAX]);
    for s in roster.list() {
        let id = s.bestiary_id as usize;
        if id < BESTIARY_MAX && out[id].is_none() {
            out[id] = map_tables::spawn_rules(&beasts, &pool, s.bestiary_id, &BALANCE.monsters);
        }
    }
    Ok(out)
}

/// Patch C (docs/MAP_DESCRIPTION.md 17.2, 17.5): the DM reveals pack `pack`'s composition,
/// `roster` with its `salt` and the Merkle `proof` to the map's `secrets_root`. During the party:
/// the pack the party engaged, before `enter_arena`; after it, in the final audit, any pack not
/// revealed yet (docs/PARTY_AND_TURNS.md 3.2). The roster must start with the pack's public
/// monster, follow the pack rules (`deadroll_mapcheck::arena::check_roster`), give the threat the
/// DM declared (a mismatch rejects the reveal) and keep the map within its tier's totals. During
/// the party its monsters join the session's spawns and their templates, skills and reward tables
/// are copied for the fight.
#[session_auth_or(
    ctx.accounts.signer.key() == ctx.accounts.session.core.dm,
    SessionError::Unauthorized
)]
pub fn handler(
    ctx: Context<RevealPack>,
    pack: u8,
    roster: PackRoster,
    salt: [u8; SECRET_SALT_LEN],
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let accounts = ctx.accounts;
    let session = &mut accounts.session;
    let session_key = session.key();
    auth::check_bound(accounts.signer.key, &session.core.dm, &session.dm_actor)?;
    let audit = audit_mode(session)?;
    let bit = 1u8.checked_shl(pack as u32).unwrap_or(0);
    require!(
        bit != 0 && session.used_packs() & bit != 0 && session.packs_revealed & bit == 0,
        SessionError::PackNotToReveal
    );
    if !audit {
        require!(!session.core.pending_vrf, SessionError::RollPending);
        require!(
            session.mode == ArenaMode::Engaged && session.arena_pack == pack,
            SessionError::PackNotToReveal
        );
    }
    verify_pack(session, pack, &roster, &salt, &proof)?;

    let rules = templates(&accounts.bestiary, &accounts.monster_skills, &roster)?;
    let layout = MapLayoutView::load(&accounts.layout, None)?;
    let record = session.packs[pack as usize];
    let visible = match record.visible {
        NO_SPAWN => None,
        v => Some(layout.spawn(v as usize)?),
    };
    let count = check_roster(
        session.tier,
        pack,
        &roster,
        visible.as_ref(),
        &*layout.arenas()?,
        &tiles(session),
        |id| rules.get(id as usize).copied().flatten(),
        BALANCE.secrets.threat(),
        session.threat[pack as usize],
    )
    .map_err(rejected)?;
    drop(layout);
    let [monsters_n, elites, bosses, packs] = session.roster_totals;
    let mut totals = RosterTotals {
        monsters: monsters_n,
        elites,
        bosses,
        packs,
    };
    let limits = tier_limits(session.tier).ok_or(SessionError::SecretRejected)?;
    totals.add(&count, &limits).map_err(rejected)?;
    session.roster_totals = [totals.monsters, totals.elites, totals.bosses, totals.packs];
    session.packs_revealed |= bit;

    if !audit {
        let list = roster.list();
        let at = session.spawn_count as usize;
        require!(at + list.len() <= SPAWNS_MAX, SessionError::InvalidLayout);
        session.spawns[at..at + list.len()].copy_from_slice(list);
        session.spawn_count += list.len() as u8;
        let monsters = Monsters::load(&accounts.monsters, &session_key)?;
        for spawn in list {
            let (record, template) = read_bestiary(&accounts.bestiary, spawn.bestiary_id)?;
            monsters.add_template(spawn.bestiary_id, &record)?;
            copy_skills(&monsters, spawn, &template, &accounts.monster_skills)?;
            copy_loot_table(&monsters, &accounts.rewards, template.loot_table)?;
        }
    }
    emit!(PackRevealed {
        session: session_key,
        pack,
        count: roster.count,
        threat: session.threat[pack as usize],
        audit,
    });
    Ok(())
}

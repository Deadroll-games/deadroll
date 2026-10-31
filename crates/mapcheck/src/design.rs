//! Patch C: the whole design of a map as the builder holds it (every spawn, every secret) and its
//! split into what `finalize_map` stores in the open and what goes under the Merkle root
//! (docs/MAP_DESCRIPTION.md 8, 17.2, 17.5).
//!
//! The builder keeps one spawn list with every monster and each pack's `visible` index into it.
//! [`split`] keeps only the visible monsters in the public list (renumbering `visible`) and gives
//! each pack's composition with the visible monster first, then the others in list order; the
//! program checks that order when the pack is revealed ([`crate::arena::check_roster`]).

use deadroll_types::{
    Cell, FoundSecret, MapArenasData, MapLayoutDataV2, MapSecretsData, PackLayout, PackRoster,
    PointKind, SecretKind, Spawn, ARENA_MONSTERS_MAX, CHESTS_MAX, NO_SPAWN, NO_TABLE, PACKS_MAX,
    SECRETS_MAX, SECRET_LEAVES_MAX, SECRET_TYPES,
};

use crate::{
    arena::{arena_byte, check_roster, ArenaLimits},
    check_map, check_public,
    limits::tier_limits,
    loot::{allowed_table, check_stash_budget, MapLoot, TableRules},
    roster::{RosterTotals, SpawnRules},
    secrets::{check_secret, threat, Board, Leaves, SecretLimits, SecretPlace, ThreatWeights},
    tiles::{cell_index, TileRules},
    MapCheckError,
};

/// A design split for publishing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Split {
    /// The layout with only the visible monsters in `spawns`.
    pub public: MapLayoutDataV2,
    /// The packs with `visible` into the public spawns (`NO_SPAWN` for an ambush pack).
    pub packs: [PackLayout; PACKS_MAX],
    /// Each pack's composition (`count == 0` for an unused pack number).
    pub rosters: [PackRoster; PACKS_MAX],
    /// Each pack's threat (0 for an unused pack number).
    pub threat: [u8; PACKS_MAX],
}

/// Splits a design (module docs). `packs` are the design's packs, `visible` into
/// `layout.spawns`.
pub fn split(
    layout: &MapLayoutDataV2,
    packs: &[PackLayout; PACKS_MAX],
    template_of: impl Fn(u8) -> Option<SpawnRules>,
    weights: ThreatWeights,
) -> Result<Split, MapCheckError> {
    use MapCheckError as E;
    let spawns = &layout.spawns[..(layout.spawn_count as usize).min(layout.spawns.len())];
    let mut public = layout.clone();
    public.spawns = [Spawn::default(); deadroll_types::SPAWNS_MAX];
    public.spawn_count = 0;
    let mut out = Split {
        public,
        packs: *packs,
        rosters: [PackRoster::default(); PACKS_MAX],
        threat: [0; PACKS_MAX],
    };
    for (n, p) in packs.iter().enumerate() {
        if !p.is_used() {
            continue;
        }
        let roster = &mut out.rosters[n];
        let mut push = |s: &Spawn| -> Result<(), MapCheckError> {
            if roster.count as usize == ARENA_MONSTERS_MAX {
                return Err(E::PackTooBig);
            }
            roster.spawns[roster.count as usize] = *s;
            roster.count += 1;
            Ok(())
        };
        if p.visible != NO_SPAWN {
            let v = spawns
                .get(p.visible as usize)
                .filter(|s| s.pack == n as u8)
                .ok_or(E::VisibleNotInPack)?;
            push(v)?;
            let at = out.public.spawn_count;
            out.public.spawns[at as usize] = *v;
            out.public.spawn_count += 1;
            out.packs[n].visible = at;
        }
        for (i, s) in spawns.iter().enumerate() {
            if s.pack == n as u8 && i != p.visible as usize {
                push(s)?;
            }
        }
        let role_of = |id: u8| template_of(id).map(|t| t.role);
        let t = threat(out.rosters[n].list(), role_of, weights);
        out.threat[n] = u8::try_from(t).map_err(|_| E::ThreatOutOfRange)?;
    }
    Ok(out)
}

/// Every rule of a design: the whole layout ([`check_map`]), its public part ([`check_public`]),
/// each pack as the program checks it when revealed and the map's totals, and every secret
/// (`secrets`: the cells that hide something, by leaf) against the declared counts, its place
/// and the reward budget. Returns the split to publish.
#[allow(clippy::too_many_arguments)]
pub fn check_design<R: Fn(u8) -> Option<TableRules>>(
    width: u8,
    height: u8,
    tier: u8,
    layout: &MapLayoutDataV2,
    arenas: &MapArenasData,
    declared: &[u8; SECRET_TYPES],
    secrets: &[FoundSecret],
    arena_limits: ArenaLimits,
    secret_limits: &SecretLimits,
    weights: ThreatWeights,
    tiles: &TileRules,
    template_of: impl Fn(u8) -> Option<SpawnRules>,
    loot: &MapLoot<R>,
) -> Result<Split, MapCheckError> {
    use MapCheckError as E;
    check_map(
        width,
        height,
        tier,
        layout,
        arenas,
        arena_limits,
        tiles,
        &template_of,
        loot,
    )?;
    let out = split(layout, &arenas.packs, &template_of, weights)?;
    let mut public_arenas = arenas.clone();
    public_arenas.packs = out.packs;
    let mut public_secrets = MapSecretsData::EMPTY;
    public_secrets.threat = out.threat;
    public_secrets.declared = *declared;
    check_public(
        width,
        height,
        tier,
        &out.public,
        &public_arenas,
        &public_secrets,
        arena_limits,
        secret_limits,
        tiles,
        &template_of,
        loot,
    )?;

    let limits = tier_limits(tier).ok_or(E::TierOutOfRange)?;
    let mut totals = RosterTotals::default();
    for (n, p) in out.packs.iter().enumerate() {
        if !p.is_used() {
            continue;
        }
        let visible = (p.visible != NO_SPAWN).then(|| &out.public.spawns[p.visible as usize]);
        let count = check_roster(
            tier,
            n as u8,
            &out.rosters[n],
            visible,
            &public_arenas,
            tiles,
            &template_of,
            weights,
            out.threat[n],
        )?;
        totals.add(&count, &limits)?;
    }
    totals.finish()?;

    check_cells(
        width,
        height,
        &out,
        &public_arenas,
        declared,
        secrets,
        secret_limits,
        tiles,
        limits.level_max,
        loot,
    )?;
    Ok(out)
}

/// The secrets of a design: places, kinds, counts, one ambush per ambush pack, the budget.
#[allow(clippy::too_many_arguments)]
fn check_cells<R: Fn(u8) -> Option<TableRules>>(
    width: u8,
    height: u8,
    out: &Split,
    arenas: &MapArenasData,
    declared: &[u8; SECRET_TYPES],
    secrets: &[FoundSecret],
    limits: &SecretLimits,
    tiles: &TileRules,
    level_cap: u8,
    loot: &MapLoot<R>,
) -> Result<(), MapCheckError> {
    use MapCheckError as E;
    if secrets.len() > SECRETS_MAX {
        return Err(E::TooManySecrets);
    }
    let leaves = Leaves::new(width, height, &out.packs);
    let ambush = crate::secrets::ambush_mask(&out.packs);
    let mut seen = [0u8; SECRET_LEAVES_MAX.div_ceil(8)];
    let mut counts = [0u8; SECRET_TYPES];
    let mut linked = 0u8;
    let mut stashes = [NO_TABLE; SECRETS_MAX];
    let mut stash_count = 0;
    for f in secrets {
        let board = leaves.board(f.leaf).ok_or(E::SecretInvalid)?;
        let k = f.secret.kind.counted().ok_or(E::SecretInvalid)?;
        let bit = 1 << (f.leaf % 8);
        if seen[f.leaf as usize / 8] & bit != 0 {
            return Err(E::DuplicateSecret);
        }
        seen[f.leaf as usize / 8] |= bit;
        let place = place_of(board, &out.public, arenas, &out.packs);
        check_secret(&place, &f.secret, tiles, limits, ambush, |id| {
            allowed_table(level_cap, id, loot).map(|_| ())
        })?;
        counts[k] += 1;
        match f.secret.kind {
            SecretKind::Ambush => {
                if linked & (1 << f.secret.a) != 0 {
                    return Err(E::AmbushUnlinked);
                }
                linked |= 1 << f.secret.a;
            }
            SecretKind::Stash => {
                stashes[stash_count] = f.secret.a;
                stash_count += 1;
            }
            SecretKind::None | SecretKind::Trap | SecretKind::Door => {}
        }
    }
    for (c, d) in counts.iter().zip(declared) {
        if c > d {
            return Err(E::TooManySecrets);
        }
        if c < d {
            return Err(E::TooFewSecrets);
        }
    }
    let mut chests = [NO_TABLE; CHESTS_MAX];
    let mut chest_count = 0;
    for c in out.public.chests().take(CHESTS_MAX) {
        chests[chest_count] = c.data;
        chest_count += 1;
    }
    check_stash_budget(
        level_cap,
        &chests[..chest_count],
        out.public.boss_table,
        &stashes[..stash_count],
        loot,
    )
}

/// The place of a leaf on a public layout: its cell byte, and on the map whether the cell is the
/// entrance, the exit, a visible chest or a pack's map cell.
pub fn place_of(
    board: Board,
    public: &MapLayoutDataV2,
    arenas: &MapArenasData,
    packs: &[PackLayout; PACKS_MAX],
) -> SecretPlace {
    match board {
        Board::Map(c) => SecretPlace {
            board,
            byte: public.cells[cell_index(c.x, c.y)],
            occupied: occupied(c, public, packs),
        },
        Board::Arena(n, c) => SecretPlace {
            board,
            byte: arena_byte(arenas, n, c.x as usize, c.y as usize),
            occupied: false,
        },
        Board::Pack(_) => SecretPlace {
            board,
            byte: 0,
            occupied: true,
        },
    }
}

/// A map cell taken by a point (the entrance, the exit, a visible chest) or a pack's visible
/// monster.
pub fn occupied(c: Cell, public: &MapLayoutDataV2, packs: &[PackLayout; PACKS_MAX]) -> bool {
    let ends = [
        public.entrance().unwrap_or(c),
        public.exit().unwrap_or(c),
    ];
    let chests = public
        .point_list()
        .iter()
        .filter(|p| p.kind == PointKind::Chest)
        .map(|p| p.cell());
    occupied_cell(c, ends, chests, packs)
}

/// The same from its parts (the session program has no decoded layout): the entrance and the
/// exit, the map's chests, the packs.
pub fn occupied_cell(
    c: Cell,
    ends: [Cell; 2],
    mut chests: impl Iterator<Item = Cell>,
    packs: &[PackLayout; PACKS_MAX],
) -> bool {
    ends.contains(&c)
        || chests.any(|p| p == c)
        || packs
            .iter()
            .any(|p| p.is_used() && p.visible != NO_SPAWN && p.cell() == c)
}

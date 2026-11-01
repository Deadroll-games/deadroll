//! Patch C: the whole design of a map as the builder holds it (every spawn, every secret) and its
//! split into what `finalize_map` stores in the open and what goes under the Merkle root
//! (docs/MAP_DESCRIPTION.md 8, 17.2, 17.5).
//!
//! The builder keeps one spawn list with every monster and each pack's `visible` index into it.
//! [`split`] keeps only the visible monsters in the public list (renumbering `visible`) and gives
//! each pack's composition with the visible monster first, then the others in list order; the
//! program checks that order when the pack is revealed ([`crate::arena::check_roster`]).
//!
//! Patch D: a design of any size class ([`MapParts`] with every spawn), checked off-chain only
//! (the reachability check runs on the caller's stack, [`crate::with_reach`]).

use deadroll_types::{
    Cell, FoundSecret, LayoutArenas, LayoutCore, LayoutSecrets, PackLayout, PackRoster, PointKind,
    SecretKind, Spawn, ARENA_MONSTERS_MAX, CHESTS_CAP, CLASS_SECRETS, MAP_CLASSES, NO_SPAWN,
    NO_TABLE, PACKS_CAP, SECRETS_CAP, SECRET_LEAVES_CAP, SECRET_TYPES,
};

use crate::{
    arena::{arena_byte, check_roster, ArenaLimits},
    check_design_map, check_public_map,
    limits::{tier_limits, SizeScale},
    loot::{allowed_table, check_stash_budget, MapLoot, TableRules},
    roster::{RosterTotals, SpawnRules},
    secrets::{check_secret, threat, Board, Leaves, SecretLimits, SecretPlace, ThreatWeights},
    tiles::TileRules,
    with_reach, MapCheckError, MapParts,
};

/// A design split for publishing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Split {
    /// The points and boss table, with only the visible monsters in `spawns`.
    pub public: LayoutCore,
    /// The packs with `visible` into the public spawns (`NO_SPAWN` for an ambush pack).
    pub packs: [PackLayout; PACKS_CAP],
    /// Each pack's composition (`count == 0` for an unused pack number).
    pub rosters: [PackRoster; PACKS_CAP],
    /// Each pack's threat (0 for an unused pack number).
    pub threat: [u8; PACKS_CAP],
}

/// Splits a design (module docs). `spawns` are every monster, `packs` the design's packs with
/// `visible` into `spawns`; `core` gives the points and the boss table.
pub fn split(
    core: &LayoutCore,
    spawns: &[Spawn],
    packs: &[PackLayout; PACKS_CAP],
    template_of: impl Fn(u8) -> Option<SpawnRules>,
    weights: ThreatWeights,
) -> Result<Split, MapCheckError> {
    use MapCheckError as E;
    let mut public = core.clone();
    public.spawns = [Spawn::default(); PACKS_CAP];
    public.spawn_count = 0;
    let mut out = Split {
        public,
        packs: *packs,
        rosters: [PackRoster::default(); PACKS_CAP],
        threat: [0; PACKS_CAP],
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

/// Every rule of a design: the whole layout ([`check_design_map`]), its public part
/// ([`check_public_map`]), each pack as the program checks it when revealed and the map's totals,
/// and every secret (`secrets`: the cells that hide something, by leaf) against the declared
/// counts, its place and the reward budget. `parts.spawns` are every monster; `secret_limits`
/// and `loot` are the tier's at this size. Returns the split to publish.
#[allow(clippy::too_many_arguments)]
pub fn check_design<R: Fn(u8) -> Option<TableRules>>(
    parts: &MapParts,
    tier: u8,
    scale: SizeScale,
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
    with_reach(&parts.board, parts.core, tiles, |reach| {
        check_design_map(
            parts,
            tier,
            scale,
            arena_limits,
            tiles,
            &template_of,
            loot,
            reach,
        )?;
        let out = split(
            parts.core,
            parts.spawns,
            &parts.arenas.packs,
            &template_of,
            weights,
        )?;
        let mut public_arenas = parts.arenas.clone();
        public_arenas.packs = out.packs;
        let mut public_secrets = LayoutSecrets::EMPTY;
        public_secrets.threat = out.threat;
        public_secrets.declared = *declared;
        let public = MapParts {
            board: parts.board,
            core: &out.public,
            spawns: out.public.spawn_list(),
            arenas: &public_arenas,
        };
        check_public_map(
            &public,
            tier,
            scale,
            &public_secrets,
            arena_limits,
            secret_limits,
            tiles,
            &template_of,
            loot,
            reach,
        )?;

        let limits = tier_limits(tier).ok_or(E::TierOutOfRange)?.scaled(scale);
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
            &public,
            scale,
            &out,
            declared,
            secrets,
            secret_limits,
            tiles,
            limits.level_max,
            loot,
        )?;
        Ok(out)
    })?
}

/// The secrets of a design: places, kinds, counts, one ambush per ambush pack, the budget.
#[allow(clippy::too_many_arguments)]
fn check_cells<R: Fn(u8) -> Option<TableRules>>(
    public: &MapParts,
    scale: SizeScale,
    out: &Split,
    declared: &[u8; SECRET_TYPES],
    secrets: &[FoundSecret],
    limits: &SecretLimits,
    tiles: &TileRules,
    level_cap: u8,
    loot: &MapLoot<R>,
) -> Result<(), MapCheckError> {
    use MapCheckError as E;
    let class = (scale.class as usize).min(MAP_CLASSES - 1);
    if secrets.len() > CLASS_SECRETS[class] {
        return Err(E::TooManySecrets);
    }
    let (width, height) = (public.board.width, public.board.height);
    let leaves = Leaves::new(width, height, scale.class, &out.packs);
    let ambush = crate::secrets::ambush_mask(&out.packs);
    let mut seen = [0u8; SECRET_LEAVES_CAP.div_ceil(8)];
    let mut counts = [0u8; SECRET_TYPES];
    let mut linked = 0u64;
    let mut stashes = [NO_TABLE; SECRETS_CAP];
    let mut stash_count = 0;
    for f in secrets {
        let board = leaves.board(f.leaf).ok_or(E::SecretInvalid)?;
        let k = f.secret.kind.counted().ok_or(E::SecretInvalid)?;
        let bit = 1 << (f.leaf % 8);
        if seen[f.leaf as usize / 8] & bit != 0 {
            return Err(E::DuplicateSecret);
        }
        seen[f.leaf as usize / 8] |= bit;
        let place = place_of(board, &public.board, &out.public, public.arenas, &out.packs);
        check_secret(&place, &f.secret, tiles, limits, ambush, |id| {
            allowed_table(level_cap, id, loot).map(|_| ())
        })?;
        counts[k] = counts[k].saturating_add(1);
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
    let mut chests = [NO_TABLE; CHESTS_CAP];
    let mut chest_count = 0;
    for c in out.public.chests().take(CHESTS_CAP) {
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
    map: &deadroll_types::Board,
    public: &LayoutCore,
    arenas: &LayoutArenas,
    packs: &[PackLayout; PACKS_CAP],
) -> SecretPlace {
    match board {
        Board::Map(c) => SecretPlace {
            board,
            byte: map.byte(c.x as usize, c.y as usize),
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
pub fn occupied(c: Cell, public: &LayoutCore, packs: &[PackLayout; PACKS_CAP]) -> bool {
    let ends = [public.entrance().unwrap_or(c), public.exit().unwrap_or(c)];
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
    packs: &[PackLayout; PACKS_CAP],
) -> bool {
    ends.contains(&c)
        || chests.any(|p| p == c)
        || packs
            .iter()
            .any(|p| p.is_used() && p.visible != NO_SPAWN && p.cell() == c)
}

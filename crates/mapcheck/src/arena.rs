//! Arena update: the packs' arenas and how a party enters one (docs/MAP_DESCRIPTION.md 17.2 and
//! 17.3, docs/PARTY_AND_TURNS.md 4.1.3).
//!
//! A spawn's `x`, `y` are its cell in its pack's arena. The pack's [`PackLayout`] holds the map
//! cell of the one monster shown on the map, the pack's facing and the arena size. A character who
//! steps next to that monster takes the whole living party into the arena, on the edge they came
//! from ([`entry_cells`]); the monsters stand where the DM put them.

use deadroll_types::{
    Cell, Facing, MapArenasData, MapLayoutDataV2, PackLayout, Spawn, ARENA_MONSTERS_MAX,
    ARENA_SIDE_MAX, PACKS_MAX, PARTY_MAX,
};

use crate::{
    footprint::Footprint,
    path::{reachable_over, Reach},
    tiles::{in_bounds, TileRules},
    MapCheckError,
};

/// The arena sizes a DM may draw (`deadroll_rules::BalanceConfig.arena`), inclusive, never above
/// the storage cap `ARENA_SIDE_MAX`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArenaLimits {
    pub side_min: u8,
    pub side_max: u8,
}

/// One axis of a direction. The sign is the variant: no negative values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Minus,
    Zero,
    Plus,
}

impl Sign {
    /// The sign of `to - from`.
    pub const fn of(from: u8, to: u8) -> Sign {
        if to < from {
            Sign::Minus
        } else if to > from {
            Sign::Plus
        } else {
            Sign::Zero
        }
    }
}

/// The side a character came from, seen from the pack's visible monster: the signs of
/// `character - monster` on each axis (north is `y - 1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Approach {
    pub dx: Sign,
    pub dy: Sign,
}

impl Approach {
    /// The 8 neighbors of a cell, clockwise from the north.
    pub const ALL: [Approach; 8] = [
        Approach::of_facing(Facing::N),
        Approach::of_facing(Facing::NE),
        Approach::of_facing(Facing::E),
        Approach::of_facing(Facing::SE),
        Approach::of_facing(Facing::S),
        Approach::of_facing(Facing::SW),
        Approach::of_facing(Facing::W),
        Approach::of_facing(Facing::NW),
    ];

    /// From the monster's cell to a neighboring one (Chebyshev distance 1); `None` otherwise.
    pub const fn between(monster: Cell, character: Cell) -> Option<Approach> {
        let (dx, dy) = (
            monster.x.abs_diff(character.x),
            monster.y.abs_diff(character.y),
        );
        if dx > 1 || dy > 1 || dx + dy == 0 {
            return None;
        }
        Some(Approach {
            dx: Sign::of(monster.x, character.x),
            dy: Sign::of(monster.y, character.y),
        })
    }

    /// The direction a facing points to.
    pub const fn of_facing(facing: Facing) -> Approach {
        use Sign::{Minus, Plus, Zero};
        let (dx, dy) = match facing {
            Facing::N => (Zero, Minus),
            Facing::NE => (Plus, Minus),
            Facing::E => (Plus, Zero),
            Facing::SE => (Plus, Plus),
            Facing::S => (Zero, Plus),
            Facing::SW => (Minus, Plus),
            Facing::W => (Minus, Zero),
            Facing::NW => (Minus, Minus),
        };
        Approach { dx, dy }
    }
}

/// The character came from the pack's back: the approach points against the facing (their dot
/// product is negative), one of the 3 cells behind. From the front and the 2 side cells the pack
/// sees them coming (Ivan's decision: the side counts as the front).
pub const fn from_back(facing: Facing, approach: Approach) -> bool {
    let f = Approach::of_facing(facing);
    let mut against = 0;
    let mut along = 0;
    let axes = [(f.dx, approach.dx), (f.dy, approach.dy)];
    let mut i = 0;
    while i < axes.len() {
        match axes[i] {
            (Sign::Minus, Sign::Plus) | (Sign::Plus, Sign::Minus) => against += 1,
            (Sign::Minus, Sign::Minus) | (Sign::Plus, Sign::Plus) => along += 1,
            _ => {}
        }
        i += 1;
    }
    against > along
}

/// The arena cell a party coming from `approach` enters at: the edge on each axis the approach
/// comes from, the middle on an axis it does not (a diagonal gives a corner).
pub const fn anchor(approach: Approach, width: u8, height: u8) -> Cell {
    const fn pick(sign: Sign, side: u8) -> u8 {
        match sign {
            Sign::Minus => 0,
            Sign::Zero => side.saturating_sub(1) / 2,
            Sign::Plus => side.saturating_sub(1),
        }
    }
    Cell {
        x: pick(approach.dx, width),
        y: pick(approach.dy, height),
    }
}

/// The cell byte at `(x, y)` of `pack`'s arena (0, the void, outside the storage).
pub fn arena_byte(arenas: &MapArenasData, pack: u8, x: usize, y: usize) -> u8 {
    match arenas.cells.get(pack as usize) {
        Some(cells) if x < ARENA_SIDE_MAX && y < ARENA_SIDE_MAX => cells[y * ARENA_SIDE_MAX + x],
        _ => 0,
    }
}

fn chebyshev(a: Cell, b: Cell) -> u8 {
    a.x.abs_diff(b.x).max(a.y.abs_diff(b.y))
}

/// The cells the living party takes entering `pack`'s arena from `approach`, one per entry of
/// `out` (in slot order): the cells that can be entered and are not `taken` (the monsters),
/// nearest to the [`anchor`] by Chebyshev distance, then by row, then by column. Returns how many
/// it found: fewer than `out.len()` only on a map that failed [`check_arenas`].
pub fn entry_cells(
    arenas: &MapArenasData,
    pack: u8,
    tiles: &TileRules,
    approach: Approach,
    taken: &[Cell],
    out: &mut [Cell],
) -> usize {
    let Some(p) = arenas.packs.get(pack as usize) else {
        return 0;
    };
    let (w, h) = (
        p.width.min(ARENA_SIDE_MAX as u8),
        p.height.min(ARENA_SIDE_MAX as u8),
    );
    let at = anchor(approach, w, h);
    let mut found = 0;
    // Ring by ring around the anchor (Chebyshev distance `d`), each ring row by row: the order
    // of distance, then row, then column, without sorting.
    let mut take = |x: u8, y: u8| -> bool {
        let c = Cell { x, y };
        if tiles.is_passable(arena_byte(arenas, pack, x as usize, y as usize))
            && !taken.contains(&c)
        {
            out[found] = c;
            found += 1;
        }
        found == out.len()
    };
    for d in 0..w.max(h) {
        let (y0, y1) = (at.y.saturating_sub(d), (at.y + d).min(h - 1));
        let (x0, x1) = (at.x.saturating_sub(d), (at.x + d).min(w - 1));
        for y in y0..=y1 {
            if y.abs_diff(at.y) == d {
                for x in x0..=x1 {
                    if take(x, y) {
                        return out.len();
                    }
                }
                continue;
            }
            // Inside the ring only its two side cells are at distance `d`.
            if let Some(x) = at.x.checked_sub(d) {
                if take(x, y) {
                    return out.len();
                }
            }
            if d > 0 && at.x + d < w && take(at.x + d, y) {
                return out.len();
            }
        }
    }
    found
}

/// Patch B: the most cells an arena's pack stands on (every monster 2×2).
pub const ARENA_TAKEN_MAX: usize = ARENA_MONSTERS_MAX * 4;

/// Writes the cells of `units` into `out` (the `taken` of [`entry_cells`], the `blocked` of
/// [`arena_reach`]); returns how many fit.
pub fn footprint_cells(units: impl Iterator<Item = Footprint>, out: &mut [Cell]) -> usize {
    let mut n = 0;
    for f in units {
        for c in f.cells() {
            if n == out.len() {
                return n;
            }
            out[n] = c;
            n += 1;
        }
    }
    n
}

/// The cells reachable inside `pack`'s arena from `start`, around `blocked`.
pub fn arena_reach(
    arenas: &MapArenasData,
    pack: u8,
    tiles: &TileRules,
    start: Cell,
    blocked: &[Cell],
) -> Reach {
    let p = arenas.packs.get(pack as usize).copied().unwrap_or_default();
    reachable_over(
        |x, y| arena_byte(arenas, pack, x, y),
        tiles,
        p.width.min(ARENA_SIDE_MAX as u8),
        p.height.min(ARENA_SIDE_MAX as u8),
        start,
        blocked,
    )
}

/// The cells around `c` (Chebyshev distance 1) inside a `width` x `height` field.
fn neighbors(c: Cell, width: u8, height: u8) -> impl Iterator<Item = Cell> {
    (0..9u8).filter_map(move |i| {
        let (x, y) = (
            (c.x as u16 + (i % 3) as u16).checked_sub(1)?,
            (c.y as u16 + (i / 3) as u16).checked_sub(1)?,
        );
        let n = Cell {
            x: u8::try_from(x).ok()?,
            y: u8::try_from(y).ok()?,
        };
        (n != c && in_bounds(n, width, height)).then_some(n)
    })
}

/// The arena rules of a map (docs/MAP_DESCRIPTION.md 17.2, 17.3). `map_reach`: the map cells the
/// party reaches from the entrance (around the closed chests); `ends`: the entrance and the exit;
/// `chests`: the visible chests' cells.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_arenas(
    width: u8,
    height: u8,
    layout: &MapLayoutDataV2,
    arenas: &MapArenasData,
    limits: ArenaLimits,
    tiles: &TileRules,
    ends: [Cell; 2],
    chests: &[Cell],
    map_reach: &Reach,
    size_of: &dyn Fn(u8) -> u8,
) -> Result<(), MapCheckError> {
    use MapCheckError as E;
    let spawns = &layout.spawns[..(layout.spawn_count as usize).min(layout.spawns.len())];
    let footprint = |s: &Spawn| Footprint::new(Cell { x: s.x, y: s.y }, size_of(s.bestiary_id));

    // Every monster stands in its pack's arena, with every cell of its footprint (patch B).
    let mut in_pack = [0u8; PACKS_MAX];
    for (i, s) in spawns.iter().enumerate() {
        let p = arenas.packs.get(s.pack as usize).ok_or(E::PackOutOfRange)?;
        if !p.is_used() {
            return Err(E::PackWithoutArena);
        }
        let f = footprint(s);
        if !f.inside(p.width, p.height) {
            return Err(E::SpawnOutOfBounds);
        }
        let passable = f
            .cells()
            .all(|c| tiles.is_passable(arena_byte(arenas, s.pack, c.x as usize, c.y as usize)));
        if !passable {
            return Err(E::SpawnNotPassable);
        }
        let taken = spawns[..i]
            .iter()
            .any(|o| o.pack == s.pack && footprint(o).overlaps(&f));
        if taken {
            return Err(E::SpawnOverlap);
        }
        in_pack[s.pack as usize] += 1;
        if in_pack[s.pack as usize] as usize > ARENA_MONSTERS_MAX {
            return Err(E::PackTooBig);
        }
    }

    let side_max = limits.side_max.min(ARENA_SIDE_MAX as u8);
    let entrance = ends[0];
    for (n, p) in arenas.packs.iter().enumerate() {
        if !p.is_used() {
            continue;
        }
        let pack = n as u8;
        let side_ok = |side: u8| (limits.side_min..=side_max).contains(&side);
        if !side_ok(p.width) || !side_ok(p.height) {
            return Err(E::ArenaSizeOutOfRange);
        }
        for y in 0..p.height as usize {
            for x in 0..p.width as usize {
                if !tiles.knows(arena_byte(arenas, pack, x, y)) {
                    return Err(E::ArenaInvalidTile);
                }
            }
        }
        match spawns.get(p.visible as usize) {
            Some(s) if s.pack == pack => {}
            _ => return Err(E::VisibleNotInPack),
        }
        check_pack_cell(width, height, layout, arenas, p, n, tiles, ends, chests)?;
        if chebyshev(entrance, p.cell()) <= 1 {
            return Err(E::PackAtEntrance);
        }
        if !neighbors(p.cell(), width, height).any(|c| map_reach.contains(c)) {
            return Err(E::PackUnreachable);
        }
        check_entries(arenas, pack, tiles, spawns, size_of)?;
    }
    Ok(())
}

/// The visible monster's map cell: on the map, can be entered, not on a point, a chest or another
/// pack's cell.
#[allow(clippy::too_many_arguments)]
fn check_pack_cell(
    width: u8,
    height: u8,
    layout: &MapLayoutDataV2,
    arenas: &MapArenasData,
    p: &PackLayout,
    n: usize,
    tiles: &TileRules,
    ends: [Cell; 2],
    chests: &[Cell],
) -> Result<(), MapCheckError> {
    use MapCheckError as E;
    let cell = p.cell();
    if !in_bounds(cell, width, height) {
        return Err(E::PackOutOfBounds);
    }
    if !tiles.is_passable(layout.cells[crate::tiles::cell_index(cell.x, cell.y)]) {
        return Err(E::PackNotPassable);
    }
    let taken = ends.contains(&cell)
        || chests.contains(&cell)
        || arenas.packs[..n]
            .iter()
            .any(|o| o.is_used() && o.cell() == cell);
    if taken {
        return Err(E::PackOverlap);
    }
    Ok(())
}

/// From every side the party may come from, the arena has room for a full party and every monster
/// can be reached from where it stands.
fn check_entries(
    arenas: &MapArenasData,
    pack: u8,
    tiles: &TileRules,
    spawns: &[Spawn],
    size_of: &dyn Fn(u8) -> u8,
) -> Result<(), MapCheckError> {
    use MapCheckError as E;
    let p = arenas.packs[pack as usize];
    // Every cell the pack stands on (a large monster's whole footprint).
    let mut cells = [Cell::default(); ARENA_TAKEN_MAX];
    let count = footprint_cells(
        spawns
            .iter()
            .filter(|s| s.pack == pack)
            .map(|s| Footprint::new(Cell { x: s.x, y: s.y }, size_of(s.bestiary_id))),
        &mut cells,
    );
    let monsters = &cells[..count];
    // One search: the cells of every side must lie in the part of the arena the first side's
    // party reaches, and from there every monster can be reached (steps go both ways).
    let mut reach = None;
    for approach in Approach::ALL {
        let mut party = [Cell::default(); PARTY_MAX];
        if entry_cells(arenas, pack, tiles, approach, monsters, &mut party) < PARTY_MAX {
            return Err(E::ArenaNoRoom);
        }
        let reach =
            reach.get_or_insert_with(|| arena_reach(arenas, pack, tiles, party[0], monsters));
        if !party.iter().all(|c| reach.contains(*c)) {
            return Err(E::ArenaUnreachable);
        }
    }
    let reach = reach.ok_or(E::ArenaNoRoom)?;
    // A large monster is reached next to any cell of its square.
    let every_monster = spawns
        .iter()
        .filter(|s| s.pack == pack)
        .map(|s| Footprint::new(Cell { x: s.x, y: s.y }, size_of(s.bestiary_id)))
        .all(|f| {
            f.cells()
                .any(|m| neighbors(m, p.width, p.height).any(|c| reach.contains(c)))
        });
    if !every_monster {
        return Err(E::ArenaUnreachable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiles::tests::starter;
    use deadroll_types::{TileKind, ARENA_CELLS};

    const FLOOR: u8 = TileKind::StoneFloor as u8 * 8;
    const WALL: u8 = TileKind::Wall as u8 * 8;

    const fn c(x: u8, y: u8) -> Cell {
        Cell { x, y }
    }

    fn approach(f: Facing) -> Approach {
        Approach::of_facing(f)
    }

    #[test]
    fn approach_is_the_side_the_character_came_from() {
        let m = c(5, 5);
        assert_eq!(Approach::between(m, c(5, 4)), Some(approach(Facing::N)));
        assert_eq!(Approach::between(m, c(6, 6)), Some(approach(Facing::SE)));
        assert_eq!(Approach::between(m, c(4, 5)), Some(approach(Facing::W)));
        assert_eq!(Approach::between(m, m), None);
        assert_eq!(Approach::between(m, c(7, 5)), None);
        assert_eq!(
            Approach::between(c(0, 0), c(1, 1)),
            Some(approach(Facing::SE))
        );
        // Every neighbor is one of the 8 approaches, each once.
        let mut seen = [false; 8];
        for (i, a) in Approach::ALL.iter().enumerate() {
            assert!(!seen[i]);
            seen[i] = true;
            assert_eq!(*a, approach(Facing::ALL[i]));
        }
    }

    #[test]
    fn the_back_is_the_three_cells_behind() {
        for &facing in Facing::ALL {
            let back: [bool; 8] = core::array::from_fn(|i| from_back(facing, Approach::ALL[i]));
            assert_eq!(back.iter().filter(|b| **b).count(), 3, "{facing:?}");
            // The cell straight behind is the facing turned around (4 steps clockwise).
            let behind = (facing as usize + 4) % 8;
            assert!(back[behind] && back[(behind + 1) % 8] && back[(behind + 7) % 8]);
        }
        // Looking east: from the west it is the back, from the north (a side) the front.
        assert!(from_back(Facing::E, approach(Facing::W)));
        assert!(!from_back(Facing::E, approach(Facing::N)));
        assert!(!from_back(Facing::E, approach(Facing::E)));
        // Looking north-east: north and east are in front, south and west behind.
        assert!(!from_back(Facing::NE, approach(Facing::N)));
        assert!(from_back(Facing::NE, approach(Facing::S)));
        assert!(!from_back(Facing::NE, approach(Facing::NW)));
    }

    #[test]
    fn the_anchor_is_on_the_edge_the_party_came_from() {
        assert_eq!(anchor(approach(Facing::W), 8, 6), c(0, 2));
        assert_eq!(anchor(approach(Facing::E), 8, 6), c(7, 2));
        assert_eq!(anchor(approach(Facing::N), 8, 6), c(3, 0));
        assert_eq!(anchor(approach(Facing::SE), 8, 6), c(7, 5));
        assert_eq!(anchor(approach(Facing::NW), 8, 6), c(0, 0));
    }

    fn arena(rows: &[&str]) -> MapArenasData {
        let mut a = MapArenasData::EMPTY;
        a.packs[1] = PackLayout {
            width: rows[0].len() as u8,
            height: rows.len() as u8,
            ..PackLayout::EMPTY
        };
        let mut cells = [WALL; ARENA_CELLS];
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.bytes().enumerate() {
                cells[y * ARENA_SIDE_MAX + x] = if ch == b'#' { WALL } else { FLOOR };
            }
        }
        a.cells[1] = cells;
        a
    }

    #[test]
    fn a_large_monster_in_a_corner_is_reached_next_to_any_of_its_cells() {
        // Patch B: the 2x2 chieftain fills the corner (10..12, 10..12) of an open 12 x 12 arena;
        // its corner cell (11, 11) has no free neighbor, the others do.
        let a = arena(&["............"; 12]);
        let spawn = |bestiary_id, x, y| Spawn {
            bestiary_id,
            x,
            y,
            pack: 1,
            ..Spawn::default()
        };
        let spawns = [spawn(2, 10, 10), spawn(3, 9, 10), spawn(4, 10, 9)];
        let size = |id: u8| if id == 2 { 2 } else { 1 };
        assert_eq!(check_entries(&a, 1, &starter(), &spawns, &size), Ok(()));
        // Walled in: a wall ring around its square and its helpers.
        let mut rows = ["............"; 12];
        rows[8] = "........####";
        rows[9] = "........#...";
        rows[10] = "........#...";
        rows[11] = "........#...";
        let walled = arena(&rows);
        let spawns = [spawn(2, 10, 10), spawn(3, 9, 10), spawn(4, 9, 9)];
        assert_eq!(
            check_entries(&walled, 1, &starter(), &spawns, &size),
            Err(MapCheckError::ArenaUnreachable)
        );
    }

    #[test]
    fn the_party_takes_the_free_cells_nearest_to_the_anchor() {
        let a = arena(&["......", "#.....", "......", "......", "......", "......"]);
        let mut out = [Cell::default(); PARTY_MAX];
        // From the west: anchor (0, 2); (0, 1) is a wall, the monster stands on (1, 2).
        let n = entry_cells(&a, 1, &starter(), approach(Facing::W), &[c(1, 2)], &mut out);
        assert_eq!(n, PARTY_MAX);
        assert_eq!(out, [c(0, 2), c(1, 1), c(0, 3), c(1, 3)]);
        // A party of two takes the first two of the same cells.
        let mut two = [Cell::default(); 2];
        entry_cells(&a, 1, &starter(), approach(Facing::W), &[c(1, 2)], &mut two);
        assert_eq!(two, [out[0], out[1]]);
        // An unused pack has no cells.
        assert_eq!(
            entry_cells(&a, 0, &starter(), approach(Facing::W), &[], &mut out),
            0
        );
    }

    #[test]
    fn a_walled_arena_has_no_room() {
        let a = arena(&["######", "#..###", "######", "######", "######", "######"]);
        let mut out = [Cell::default(); PARTY_MAX];
        assert_eq!(
            entry_cells(&a, 1, &starter(), approach(Facing::N), &[], &mut out),
            2
        );
    }

    #[test]
    fn arena_reach_stops_at_walls() {
        let a = arena(&["..#...", "..#...", "..#...", "......", "......", "......"]);
        let r = arena_reach(&a, 1, &starter(), c(0, 0), &[]);
        assert!(r.contains(c(5, 0)));
        let r = arena_reach(&a, 1, &starter(), c(0, 0), &[c(0, 3), c(1, 3), c(2, 3)]);
        assert!(!r.contains(c(5, 0)) && r.contains(c(1, 2)));
        // Nothing outside the arena.
        assert!(!r.contains(c(6, 0)));
    }
}

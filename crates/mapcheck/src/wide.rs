//! Patch D: reachability over a whole map of any size class, in steps (docs/MAP_DESCRIPTION.md
//! 10.3, 17.6).
//!
//! The same flood fill as [`crate::path`] (8 neighbors, a diagonal does not cut a corner), over rows
//! of up to [`MAP_ROW_WORDS`] `u64` words. A Dungeon map can need more sweeps than one transaction
//! computes, so the fill runs as a state machine ([`CheckState`]) that stops when its work budget
//! is spent and goes on from the same place in the next call: `programs/map` keeps the state and
//! the two bitsets in the layout account (`check_step`, `finalize_map`), the website counts the
//! calls a map needs with the same numbers ([`steps_needed`]).
//!
//! Phases: `PASS` builds the passable rows from the tiles, checking every tile on the way (the
//! `InvalidTile` rule); then sweeps go down and up the map growing the reached rows until a whole
//! down-and-up pair changes nothing (`DONE`). The work is counted in units: a cell of the pass
//! phase, [`GROW_WORD`] per row word of a sweep. Where the calls stop does not change the work:
//! the same rows in the same order.

use deadroll_types::{cell_kind, Board, Cell, MAP_ROW_WORDS};

use crate::{tiles::TileRules, MapCheckError, Reached};

/// One map row of a bitset: bit `x % 64` of word `x / 64`.
pub type Row = [u64; MAP_ROW_WORDS];

/// Work units of one sweep row per word (a pass cell is one unit, ~30 CU on chain).
pub const GROW_WORD: u32 = 4;
/// Work units of joining a changed row along itself (`fill_row`), per word, on top of
/// [`GROW_WORD`]: the costliest part of a sweep.
pub const FILL_WORD: u32 = 34;
/// Work units one `check_step` transaction does: about 1.1M CU (calibrated on chain:
/// `tests/map.test.ts`, a Dungeon serpentine and an open Dungeon hall).
pub const STEP_BUDGET: u32 = 36_000;
/// Work units `finalize_map` does before its own rules (a Small map finishes there).
pub const FINALIZE_BUDGET: u32 = 16_000;

pub const PHASE_PASS: u8 = 0;
pub const PHASE_DOWN: u8 = 1;
pub const PHASE_UP: u8 = 2;
pub const PHASE_DONE: u8 = 3;

/// Where the check stands. Stored in the layout (`LayoutGeom::CHECK_LEN` bytes); all zero is the
/// start, and every write to the layout resets it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckState {
    pub phase: u8,
    /// The next row of the phase.
    pub row: u16,
    /// Some row changed in the current down-and-up pair.
    pub changed: bool,
}

impl CheckState {
    pub const fn to_bytes(self) -> [u8; 8] {
        let [r0, r1] = self.row.to_le_bytes();
        [self.phase, r0, r1, self.changed as u8, 0, 0, 0, 0]
    }

    pub const fn from_bytes(b: [u8; 8]) -> Self {
        Self {
            phase: b[0],
            row: u16::from_le_bytes([b[1], b[2]]),
            changed: b[3] != 0,
        }
    }

    pub const fn done(&self) -> bool {
        self.phase == PHASE_DONE
    }
}

/// Words of a row of a map `width` wide.
pub const fn row_words(width: u8) -> usize {
    (width as usize).div_ceil(64)
}

#[inline(always)]
fn shl(a: &Row, k: u32, nw: usize) -> Row {
    let mut out = [0u64; MAP_ROW_WORDS];
    let (ws, bs) = ((k / 64) as usize, k % 64);
    for (src, o) in out.iter_mut().take(nw).skip(ws).enumerate() {
        let mut v = a[src] << bs;
        if bs != 0 && src > 0 {
            v |= a[src - 1] >> (64 - bs);
        }
        *o = v;
    }
    out
}

#[inline(always)]
fn shr(a: &Row, k: u32, nw: usize) -> Row {
    let mut out = [0u64; MAP_ROW_WORDS];
    let (ws, bs) = ((k / 64) as usize, k % 64);
    for (i, o) in out.iter_mut().take(nw.saturating_sub(ws)).enumerate() {
        let src = i + ws;
        let mut v = a[src] >> bs;
        if bs != 0 && src + 1 < nw {
            v |= a[src + 1] << (64 - bs);
        }
        *o = v;
    }
    out
}

#[inline(always)]
fn and(a: &Row, b: &Row, nw: usize) -> Row {
    let mut out = [0u64; MAP_ROW_WORDS];
    for i in 0..nw {
        out[i] = a[i] & b[i];
    }
    out
}

#[inline(always)]
fn or_into(a: &mut Row, b: &Row, nw: usize) {
    for i in 0..nw {
        a[i] |= b[i];
    }
}

/// The cells of `row` that are `open`, joined to them along the row (Kogge-Stone, as
/// `path::fill_row`). `row` must be a subset of `open`.
fn fill_row(row: &Row, open: &Row, width: u32, nw: usize) -> Row {
    // After round k, `g` marks the cells ending an open run of 2^k: none left, nothing more to
    // join (a corridor one cell wide stops after the first round).
    let none = |g: &Row| g.iter().take(nw).all(|w| *w == 0);
    let mut up = *row;
    let mut g = *open;
    let mut s = 1;
    while s < width && !none(&g) {
        let moved = and(&g, &shl(&up, s, nw), nw);
        or_into(&mut up, &moved, nw);
        g = and(&g, &shl(&g, s, nw), nw);
        s <<= 1;
    }
    let mut down = *row;
    let mut g = *open;
    let mut s = 1;
    while s < width && !none(&g) {
        let moved = and(&g, &shr(&down, s, nw), nw);
        or_into(&mut down, &moved, nw);
        g = and(&g, &shr(&g, s, nw), nw);
        s <<= 1;
    }
    or_into(&mut up, &down, nw);
    up
}

/// What a step from the cells `from` of one row enters in the next row (`path::step_rows`).
#[inline(always)]
fn step_rows(from: &Row, from_pass: &Row, to_pass: &Row, nw: usize) -> Row {
    let mut out = *from;
    let right = and(
        &and(&shl(from, 1, nw), from_pass, nw),
        &shl(to_pass, 1, nw),
        nw,
    );
    let left = and(
        &and(&shr(from, 1, nw), from_pass, nw),
        &shr(to_pass, 1, nw),
        nw,
    );
    or_into(&mut out, &right, nw);
    or_into(&mut out, &left, nw);
    out
}

#[inline(always)]
fn bit(row: &Row, x: usize) -> bool {
    row[x / 64] & (1 << (x % 64)) != 0
}

/// The rows a step can enter: the passable rows without the `blocked` cells (closed chests).
pub fn open_rows(board: &Board, pass: &[Row], blocked: &[Cell], open: &mut [Row]) {
    let h = board.height as usize;
    open[..h].copy_from_slice(&pass[..h]);
    for c in blocked {
        if board.contains(*c) {
            open[c.y as usize][c.x as usize / 64] &= !(1 << (c.x % 64));
        }
    }
}

/// Runs the check from `state` until it is done or `budget` units are spent (what is left stays
/// in `budget`). `pass`, `open` and `reach` hold at least `board.height` rows; `open` is scratch,
/// rebuilt on every call. Returns whether the check is done. `InvalidTile`: a cell inside the map
/// with a tile type the `Tileset` does not have.
#[allow(clippy::too_many_arguments)]
pub fn step(
    board: &Board,
    tiles: &TileRules,
    start: Cell,
    blocked: &[Cell],
    pass: &mut [Row],
    open: &mut [Row],
    reach: &mut [Row],
    state: &mut CheckState,
    budget: &mut u32,
) -> Result<bool, MapCheckError> {
    let (w, h) = (board.width as usize, board.height as usize);
    let nw = row_words(board.width);
    if state.phase == PHASE_PASS {
        let passable = tiles.passable_mask();
        while (state.row as usize) < h {
            if *budget < w as u32 {
                return Ok(false);
            }
            *budget -= w as u32;
            let y = state.row as usize;
            let mut row = [0u64; MAP_ROW_WORDS];
            for (x, &b) in board.row(y).iter().enumerate() {
                if !tiles.knows(b) {
                    return Err(MapCheckError::InvalidTile);
                }
                row[x / 64] |= (((passable >> cell_kind(b)) & 1) as u64) << (x % 64);
            }
            pass[y] = row;
            state.row += 1;
        }
        open_rows(board, pass, blocked, open);
        reach[..h].fill([0; MAP_ROW_WORDS]);
        state.row = 0;
        state.changed = false;
        if !board.contains(start) || !bit(&open[start.y as usize], start.x as usize) {
            state.phase = PHASE_DONE;
            return Ok(true);
        }
        let mut first = [0u64; MAP_ROW_WORDS];
        first[start.x as usize / 64] = 1 << (start.x % 64);
        reach[start.y as usize] = fill_row(&first, &open[start.y as usize], w as u32, nw);
        state.phase = PHASE_DOWN;
    } else if state.phase != PHASE_DONE {
        open_rows(board, pass, blocked, open);
    }
    let cost = GROW_WORD * nw as u32;
    let fill = FILL_WORD * nw as u32;
    while state.phase != PHASE_DONE {
        // A row starts only with room for its fill.
        if *budget < cost + fill {
            return Ok(false);
        }
        *budget -= cost;
        let y = state.row as usize;
        let above = if y > 0 {
            step_rows(&reach[y - 1], &pass[y - 1], &pass[y], nw)
        } else {
            [0; MAP_ROW_WORDS]
        };
        let below = if y + 1 < h {
            step_rows(&reach[y + 1], &pass[y + 1], &pass[y], nw)
        } else {
            [0; MAP_ROW_WORDS]
        };
        let mut seed = reach[y];
        or_into(&mut seed, &above, nw);
        or_into(&mut seed, &below, nw);
        let seed = and(&seed, &open[y], nw);
        if seed != reach[y] {
            *budget -= fill;
            reach[y] = fill_row(&seed, &open[y], w as u32, nw);
            state.changed = true;
        }
        match state.phase {
            PHASE_DOWN if y + 1 < h => state.row += 1,
            PHASE_DOWN => state.phase = PHASE_UP,
            _ if y > 0 => state.row -= 1,
            _ if state.changed => {
                state.changed = false;
                state.phase = PHASE_DOWN;
            }
            _ => state.phase = PHASE_DONE,
        }
    }
    Ok(true)
}

/// The whole check in one go (the website, the tests).
pub fn run(
    board: &Board,
    tiles: &TileRules,
    start: Cell,
    blocked: &[Cell],
    pass: &mut [Row],
    open: &mut [Row],
    reach: &mut [Row],
) -> Result<(), MapCheckError> {
    let mut state = CheckState::default();
    let mut budget = u32::MAX;
    step(
        board,
        tiles,
        start,
        blocked,
        pass,
        open,
        reach,
        &mut state,
        &mut budget,
    )
    .map(|_| ())
}

/// The bitsets of a check: passable rows, scratch, reached rows (`height` rows each).
pub type Bitsets<'a> = (&'a mut [Row], &'a mut [Row], &'a mut [Row]);

/// `check_step` calls a map needs before `finalize_map`: the program's calls replayed, calls of
/// [`STEP_BUDGET`] until the [`FINALIZE_BUDGET`] of `finalize_map` would finish the check.
/// `main` carries the calls, `trial` each try of `finalize_map`. `Err` for a tile the `Tileset`
/// does not have.
pub fn steps_needed(
    board: &Board,
    tiles: &TileRules,
    start: Cell,
    blocked: &[Cell],
    main: Bitsets,
    trial: Bitsets,
) -> Result<u32, MapCheckError> {
    let (pass, open, reach) = main;
    let (tpass, topen, treach) = trial;
    let h = board.height as usize;
    let mut state = CheckState::default();
    let mut steps = 0;
    loop {
        tpass[..h].copy_from_slice(&pass[..h]);
        treach[..h].copy_from_slice(&reach[..h]);
        let mut t = state;
        let mut b = FINALIZE_BUDGET;
        if step(
            board, tiles, start, blocked, tpass, topen, treach, &mut t, &mut b,
        )? {
            return Ok(steps);
        }
        let mut b = STEP_BUDGET;
        step(
            board, tiles, start, blocked, pass, open, reach, &mut state, &mut b,
        )?;
        steps += 1;
    }
}

/// The reached cells of a map.
#[derive(Clone, Copy, Debug)]
pub struct ReachView<'a> {
    pub rows: &'a [Row],
    pub width: u8,
    pub height: u8,
}

impl Reached for ReachView<'_> {
    fn contains(&self, cell: Cell) -> bool {
        cell.x < self.width
            && cell.y < self.height
            && self
                .rows
                .get(cell.y as usize)
                .is_some_and(|r| bit(r, cell.x as usize))
    }
}

/// Rows from their stored bytes (`nw` little-endian words each).
pub fn rows_from_bytes(bytes: &[u8], nw: usize, out: &mut [Row]) {
    for (row, chunk) in out.iter_mut().zip(bytes.chunks_exact(nw * 8)) {
        *row = [0; MAP_ROW_WORDS];
        for (i, w) in chunk.chunks_exact(8).enumerate() {
            let mut b = [0u8; 8];
            b.copy_from_slice(w);
            row[i] = u64::from_le_bytes(b);
        }
    }
}

/// Rows into their stored bytes.
pub fn rows_to_bytes(rows: &[Row], nw: usize, out: &mut [u8]) {
    for (row, chunk) in rows.iter().zip(out.chunks_exact_mut(nw * 8)) {
        for (i, w) in chunk.chunks_exact_mut(8).enumerate() {
            w.copy_from_slice(&row[i].to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::tiles::tests::starter;
    use deadroll_types::{TileKind, MAP_SIDE_CAP};
    use std::{collections::VecDeque, vec, vec::Vec};

    const FLOOR: u8 = TileKind::StoneFloor as u8 * 8;
    const WALL: u8 = TileKind::Wall as u8 * 8;

    type Bufs = (Vec<Row>, Vec<Row>, Vec<Row>);

    fn buffers() -> Bufs {
        let z = vec![[0u64; MAP_ROW_WORDS]; MAP_SIDE_CAP];
        (z.clone(), z.clone(), z)
    }

    /// Breadth-first search with the same rules, cell by cell.
    fn bfs(board: &Board, start: Cell, blocked: &[Cell]) -> Vec<bool> {
        let tiles = starter();
        let (w, h) = (board.width as i32, board.height as i32);
        let pass = |x: i32, y: i32| {
            x >= 0
                && y >= 0
                && x < w
                && y < h
                && tiles.is_passable(board.byte(x as usize, y as usize))
        };
        let open = |x: i32, y: i32| {
            pass(x, y)
                && !blocked.contains(&Cell {
                    x: x as u8,
                    y: y as u8,
                })
        };
        let mut seen = vec![false; (w * h) as usize];
        if !open(start.x as i32, start.y as i32) {
            return seen;
        }
        let mut q = VecDeque::from([(start.x as i32, start.y as i32)]);
        seen[(start.y as i32 * w + start.x as i32) as usize] = true;
        while let Some((x, y)) = q.pop_front() {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if (dx, dy) == (0, 0) || !open(nx, ny) {
                        continue;
                    }
                    if dx != 0 && dy != 0 && !(pass(x + dx, y) && pass(x, y + dy)) {
                        continue;
                    }
                    let i = (ny * w + nx) as usize;
                    if !seen[i] {
                        seen[i] = true;
                        q.push_back((nx, ny));
                    }
                }
            }
        }
        seen
    }

    fn same_as_bfs(board: &Board, start: Cell, blocked: &[Cell]) {
        let (mut pass, mut open, mut reach) = buffers();
        run(
            board,
            &starter(),
            start,
            blocked,
            &mut pass,
            &mut open,
            &mut reach,
        )
        .unwrap();
        let view = ReachView {
            rows: &reach,
            width: board.width,
            height: board.height,
        };
        let want = bfs(board, start, blocked);
        for y in 0..board.height {
            for x in 0..board.width {
                let c = Cell { x, y };
                assert_eq!(
                    view.contains(c),
                    want[y as usize * board.width as usize + x as usize],
                    "{c:?} on {}x{}",
                    board.width,
                    board.height
                );
            }
        }
    }

    /// A serpentine of vertical corridors: every corridor needs its own sweep.
    fn serpentine(w: u8, h: u8) -> Vec<u8> {
        let mut cells = vec![FLOOR; w as usize * h as usize];
        for x in (1..w as usize).step_by(2) {
            for y in 0..h as usize {
                cells[y * w as usize + x] = WALL;
            }
            let gap = if (x / 2) % 2 == 0 { h as usize - 1 } else { 0 };
            cells[gap * w as usize + x] = FLOOR;
        }
        cells
    }

    #[test]
    fn matches_bfs_on_open_and_walled_maps_of_every_class() {
        for (w, h) in [
            (8u8, 8u8),
            (32, 32),
            (65, 9),
            (72, 72),
            (101, 101),
            (143, 143),
            (143, 20),
        ] {
            let open = vec![FLOOR; w as usize * h as usize];
            same_as_bfs(&Board::new(w, h, &open), Cell { x: 0, y: 0 }, &[]);
            let s = serpentine(w, h);
            same_as_bfs(&Board::new(w, h, &s), Cell { x: 0, y: 0 }, &[]);
            same_as_bfs(
                &Board::new(w, h, &s),
                Cell { x: w - 1, y: h - 1 },
                &[Cell { x: 2, y: h - 1 }],
            );
        }
    }

    #[test]
    fn matches_bfs_on_noise() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for (w, h) in [(17u8, 13u8), (64, 64), (130, 70), (143, 143)] {
            let cells: Vec<u8> = (0..w as usize * h as usize)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    if seed % 100 < 38 {
                        WALL
                    } else {
                        FLOOR
                    }
                })
                .collect();
            same_as_bfs(&Board::new(w, h, &cells), Cell { x: 1, y: 1 }, &[]);
        }
    }

    fn steps(board: &Board, start: Cell, calls: u32, (pass, open, reach): &mut Bufs) -> bool {
        let tiles = starter();
        let mut state = CheckState::default();
        for _ in 0..calls {
            let mut b = STEP_BUDGET;
            if step(
                board,
                &tiles,
                start,
                &[],
                pass,
                open,
                reach,
                &mut state,
                &mut b,
            )
            .unwrap()
            {
                return true;
            }
            // What the program stores between calls.
            state = CheckState::from_bytes(state.to_bytes());
        }
        let mut b = FINALIZE_BUDGET;
        step(
            board,
            &tiles,
            start,
            &[],
            pass,
            open,
            reach,
            &mut state,
            &mut b,
        )
        .unwrap()
    }

    #[test]
    fn steps_give_the_same_reach_as_one_go_and_are_counted_exactly() {
        let tiles = starter();
        let start = Cell { x: 0, y: 0 };
        for (w, h) in [(143u8, 143u8), (101, 101), (72, 72)] {
            let s = serpentine(w, h);
            let board = Board::new(w, h, &s);
            let (mut pass, mut open, mut reach) = buffers();
            run(&board, &tiles, start, &[], &mut pass, &mut open, &mut reach).unwrap();
            let want = reach.clone();
            let needed = count(&board, start);
            assert!(needed > 0, "{w}x{h}");
            let mut bufs = buffers();
            assert!(steps(&board, start, needed, &mut bufs));
            assert_eq!(bufs.2, want);
            let mut bufs = buffers();
            assert!(
                !steps(&board, start, needed - 1, &mut bufs),
                "one call fewer"
            );
        }
        let open = vec![FLOOR; 32 * 32];
        let small = Board::new(32, 32, &open);
        assert_eq!(
            count(&small, start),
            0,
            "a Small map finishes in finalize_map"
        );
        // An open Dungeon hall: the pass rows and a couple of sweeps.
        let hall = vec![FLOOR; 143 * 143];
        let hall = Board::new(143, 143, &hall);
        let n = count(&hall, start);
        let mut bufs = buffers();
        assert!(steps(&hall, start, n, &mut bufs));
    }

    fn count(board: &Board, start: Cell) -> u32 {
        let (mut a, mut b, mut c) = buffers();
        let (mut d, mut e, mut f) = buffers();
        steps_needed(
            board,
            &starter(),
            start,
            &[],
            (&mut a, &mut b, &mut c),
            (&mut d, &mut e, &mut f),
        )
        .unwrap()
    }

    #[test]
    fn refuses_an_unknown_tile_and_a_blocked_start() {
        let mut cells = vec![FLOOR; 80 * 10];
        let (mut pass, mut open, mut reach) = buffers();
        reach[3] = [1; MAP_ROW_WORDS];
        let board = Board::new(80, 10, &cells);
        let blocked = [Cell { x: 3, y: 3 }];
        run(
            &board,
            &starter(),
            Cell { x: 3, y: 3 },
            &blocked,
            &mut pass,
            &mut open,
            &mut reach,
        )
        .unwrap();
        assert!(reach.iter().all(|r| *r == [0; MAP_ROW_WORDS]));
        cells[9 * 80 + 79] = u8::MAX;
        let board = Board::new(80, 10, &cells);
        assert_eq!(
            run(
                &board,
                &starter(),
                Cell { x: 0, y: 0 },
                &[],
                &mut pass,
                &mut open,
                &mut reach
            ),
            Err(MapCheckError::InvalidTile)
        );
    }

    #[test]
    fn state_and_rows_survive_their_bytes() {
        let s = CheckState {
            phase: PHASE_UP,
            row: 141,
            changed: true,
        };
        assert_eq!(CheckState::from_bytes(s.to_bytes()), s);
        let rows = [[1u64, 2, 3], [u64::MAX, 0, 7]];
        let mut bytes = [0u8; 48];
        rows_to_bytes(&rows, 3, &mut bytes);
        let mut back = [[0u64; 3]; 2];
        rows_from_bytes(&bytes, 3, &mut back);
        assert_eq!(back, rows);
    }
}

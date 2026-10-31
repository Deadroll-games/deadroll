//! Patch C: secrets under the Merkle root (docs/MAP_DESCRIPTION.md 8, 17.2, 17.4, 17.5).
//!
//! A map commits to every cell of the map and of each arena, and to each pack's composition, in
//! one SHA-256 Merkle tree whose root is `Map.secrets_root`. The leaves, in order: one per pack
//! number (its [`PackRoster`]), the map's `width * height` cells row by row, then each used pack's
//! arena cells row by row, packs in number order ([`Leaves`]). The count is padded with zero
//! hashes to a power of two; a padding index is never a valid leaf.
//!
//! - cell leaf: `sha256(0x00 ‖ map ‖ leaf u16 LE ‖ CellSecret 4 bytes ‖ salt 16)`;
//! - pack leaf: `sha256(0x02 ‖ map ‖ leaf u16 LE ‖ PackRoster bytes ‖ salt 16)`;
//! - node: `sha256(0x01 ‖ left ‖ right)`, left and right by the index bit.
//!
//! The tags keep a leaf from passing for a node and a cell for a pack. The program uses the
//! SHA-256 syscall, the website the same code through WASM (`solana-sha256-hasher`).
//!
//! The threat of a pack (Ivan's decision) is the sum of its monsters' levels, an elite's counted
//! [`ThreatWeights::elite`] times and a boss's [`ThreatWeights::boss`] times.

use deadroll_types::{Cell, NO_SPAWN};
use deadroll_types::{
    CellSecret, PackLayout, PackRoster, SecretKind, Spawn, ARENA_SIDE_MAX, PACKS_MAX, PACK_LEAVES,
    SECRET_DEPTH_MAX, SECRET_SALT_LEN, SECRET_TYPES, SPAWN_ELITE,
};

use crate::{roster::Role, tiles::TileRules, MapCheckError};

/// A SHA-256 hash.
pub type Hash = [u8; 32];

pub const TAG_CELL: u8 = 0;
pub const TAG_NODE: u8 = 1;
pub const TAG_PACK: u8 = 2;

fn sha(parts: &[&[u8]]) -> Hash {
    solana_sha256_hasher::hashv(parts).to_bytes()
}

/// The leaf of a cell.
pub fn cell_leaf(
    map: &[u8; 32],
    leaf: u16,
    secret: &CellSecret,
    salt: &[u8; SECRET_SALT_LEN],
) -> Hash {
    sha(&[&[TAG_CELL], map, &leaf.to_le_bytes(), &secret.bytes(), salt])
}

/// The leaf of a pack's composition.
pub fn pack_leaf(
    map: &[u8; 32],
    leaf: u16,
    roster: &PackRoster,
    salt: &[u8; SECRET_SALT_LEN],
) -> Hash {
    sha(&[&[TAG_PACK], map, &leaf.to_le_bytes(), &roster.bytes(), salt])
}

pub fn node(left: &Hash, right: &Hash) -> Hash {
    sha(&[&[TAG_NODE], left, right])
}

/// Hashes in a proof of a tree of `count` leaves: `count` rounded up to a power of two, log 2.
pub const fn depth(count: u16) -> usize {
    let mut d = 0;
    while (1u32 << d) < count as u32 {
        d += 1;
    }
    d
}

/// `leaf` is leaf `index` of the tree of `count` leaves with this `root`.
pub fn verify(root: &Hash, leaf: Hash, index: u16, count: u16, proof: &[Hash]) -> bool {
    if index >= count || proof.len() != depth(count) || proof.len() > SECRET_DEPTH_MAX {
        return false;
    }
    let mut at = index;
    let mut h = leaf;
    for sibling in proof {
        h = if at & 1 == 0 {
            node(&h, sibling)
        } else {
            node(sibling, &h)
        };
        at >>= 1;
    }
    h == *root
}

/// The root of the tree whose first `count` leaves are in `level` (the website and the tests).
/// `level` must hold `count` rounded up to a power of two; it is overwritten. The proof of leaf
/// `index` goes into `proof[..depth(count)]`.
pub fn build(level: &mut [Hash], count: usize, index: usize, proof: &mut [Hash]) -> Hash {
    let d = depth(count as u16);
    let mut width = 1usize << d;
    for h in level[count..width].iter_mut() {
        *h = [0; 32];
    }
    let mut at = index;
    for slot in proof.iter_mut().take(d) {
        *slot = level[at ^ 1];
        for i in 0..width / 2 {
            level[i] = node(&level[2 * i], &level[2 * i + 1]);
        }
        width /= 2;
        at >>= 1;
    }
    level[0]
}

/// Where a leaf belongs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Board {
    /// A pack's composition.
    Pack(u8),
    /// A cell of the map.
    Map(Cell),
    /// A cell of a pack's arena.
    Arena(u8, Cell),
}

/// The leaf numbering of one map (module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leaves {
    pub width: u8,
    pub height: u8,
    /// Each pack's arena size (`0` for an unused pack number) and its first leaf.
    pub arena_w: [u8; PACKS_MAX],
    pub arena_h: [u8; PACKS_MAX],
    pub arena_at: [u16; PACKS_MAX],
    pub count: u16,
}

impl Leaves {
    pub fn new(width: u8, height: u8, packs: &[PackLayout; PACKS_MAX]) -> Leaves {
        let mut at = PACK_LEAVES as u16 + width as u16 * height as u16;
        let mut out = Leaves {
            width,
            height,
            arena_w: [0; PACKS_MAX],
            arena_h: [0; PACKS_MAX],
            arena_at: [0; PACKS_MAX],
            count: 0,
        };
        for (n, p) in packs.iter().enumerate() {
            if !p.is_used() {
                continue;
            }
            let (w, h) = (
                p.width.min(ARENA_SIDE_MAX as u8),
                p.height.min(ARENA_SIDE_MAX as u8),
            );
            out.arena_w[n] = w;
            out.arena_h[n] = h;
            out.arena_at[n] = at;
            at += w as u16 * h as u16;
        }
        out.count = at;
        out
    }

    /// The leaf of a place (`None` outside the map, the arena, or for an unused pack's arena).
    pub fn of(&self, board: Board) -> Option<u16> {
        match board {
            Board::Pack(n) => ((n as usize) < PACK_LEAVES).then_some(n as u16),
            Board::Map(c) => (c.x < self.width && c.y < self.height)
                .then(|| PACK_LEAVES as u16 + c.y as u16 * self.width as u16 + c.x as u16),
            Board::Arena(n, c) => {
                let n = n as usize;
                let (w, h) = (*self.arena_w.get(n)?, *self.arena_h.get(n)?);
                (c.x < w && c.y < h).then(|| self.arena_at[n] + c.y as u16 * w as u16 + c.x as u16)
            }
        }
    }

    /// The place of a leaf (`None` past the count).
    pub fn board(&self, leaf: u16) -> Option<Board> {
        if leaf >= self.count {
            return None;
        }
        if (leaf as usize) < PACK_LEAVES {
            return Some(Board::Pack(leaf as u8));
        }
        let map = leaf - PACK_LEAVES as u16;
        let cells = self.width as u16 * self.height as u16;
        if map < cells {
            let w = self.width as u16;
            return Some(Board::Map(Cell {
                x: (map % w) as u8,
                y: (map / w) as u8,
            }));
        }
        for n in 0..PACKS_MAX {
            let (w, h) = (self.arena_w[n] as u16, self.arena_h[n] as u16);
            let at = self.arena_at[n];
            if w != 0 && leaf >= at && leaf < at + w * h {
                let i = leaf - at;
                return Some(Board::Arena(
                    n as u8,
                    Cell {
                        x: (i % w) as u8,
                        y: (i / w) as u8,
                    },
                ));
            }
        }
        None
    }
}

/// The threat weights (`deadroll_rules::BalanceConfig.secrets`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreatWeights {
    pub elite: u8,
    pub boss: u8,
}

/// A pack's threat: each monster's level times its weight (a boss's `boss`, an elite's `elite`,
/// any other's 1), summed. `role_of` gives a template's role (`None`: counted as regular).
pub fn threat(spawns: &[Spawn], role_of: impl Fn(u8) -> Option<Role>, w: ThreatWeights) -> u16 {
    spawns
        .iter()
        .map(|s| {
            let weight = match role_of(s.bestiary_id) {
                Some(Role::Boss) => w.boss,
                _ if s.flags & SPAWN_ELITE != 0 => w.elite,
                _ => 1,
            };
            s.level as u16 * weight as u16
        })
        .fold(0u16, u16::saturating_add)
}

/// The secret limits of one map tier (`deadroll_rules::BalanceConfig.secrets`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecretLimits {
    /// Traps, ambush traps included, inclusive.
    pub traps_min: u8,
    pub traps_max: u8,
    /// Most of each type, by `SecretKind::counted`.
    pub declared_max: [u8; SECRET_TYPES],
    pub trap_damage_max: u8,
}

/// The declared counts are within the tier's limits, and every ambush pack has its ambush trap
/// (`ambush_packs`: how many used packs have no monster on the map).
pub fn check_declared(
    declared: &[u8; SECRET_TYPES],
    limits: &SecretLimits,
    ambush_packs: u8,
) -> Result<(), MapCheckError> {
    use MapCheckError as E;
    if declared
        .iter()
        .zip(limits.declared_max)
        .any(|(d, max)| *d > max)
    {
        return Err(E::TooManySecrets);
    }
    let at = |k: SecretKind| declared[k.counted().unwrap_or(0)];
    let traps = at(SecretKind::Trap) + at(SecretKind::Ambush);
    if traps > limits.traps_max {
        return Err(E::TooManySecrets);
    }
    if traps < limits.traps_min {
        return Err(E::TooFewTraps);
    }
    if at(SecretKind::Ambush) != ambush_packs {
        return Err(E::AmbushUnlinked);
    }
    Ok(())
}

/// The bit of every used pack with no monster on the map (an ambush pack).
pub fn ambush_mask(packs: &[PackLayout; PACKS_MAX]) -> u8 {
    packs
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_used() && p.visible == NO_SPAWN)
        .fold(0, |m, (n, _)| m | 1 << n)
}

/// What a cell's secret is checked against.
pub struct SecretPlace {
    pub board: Board,
    /// The cell byte there.
    pub byte: u8,
    /// The cell is the entrance, the exit, a visible chest or a pack's map cell.
    pub occupied: bool,
}

/// The rules of one cell's contents (docs/MAP_DESCRIPTION.md 8.1, 17.4): a trap or an ambush on a
/// free cell that can be entered, an ambush and a hidden stash on the map only, an ambush linked
/// to an ambush pack, a secret door in a wall, a stash table the tier allows (`table_ok`), unused
/// bytes zero.
pub fn check_secret(
    place: &SecretPlace,
    secret: &CellSecret,
    tiles: &TileRules,
    limits: &SecretLimits,
    ambush_packs: u8,
    table_ok: impl Fn(u8) -> Result<(), MapCheckError>,
) -> Result<(), MapCheckError> {
    use MapCheckError as E;
    let on_map = matches!(place.board, Board::Map(_));
    if matches!(place.board, Board::Pack(_)) {
        return Err(E::SecretInvalid);
    }
    let free = || -> Result<(), MapCheckError> {
        if !tiles.is_passable(place.byte) {
            return Err(E::SecretNotPassable);
        }
        if place.occupied {
            return Err(E::SecretOnPoint);
        }
        Ok(())
    };
    match secret.kind {
        SecretKind::None => {
            if secret.a | secret.b | secret.c != 0 {
                return Err(E::SecretInvalid);
            }
        }
        SecretKind::Trap => {
            if secret.c != 0 {
                return Err(E::SecretInvalid);
            }
            if secret.a == 0 || secret.a > limits.trap_damage_max {
                return Err(E::TrapDamageOutOfRange);
            }
            free()?;
        }
        SecretKind::Ambush => {
            if !on_map {
                return Err(E::AmbushInArena);
            }
            if secret.b | secret.c != 0 {
                return Err(E::SecretInvalid);
            }
            if secret.a as usize >= PACKS_MAX || ambush_packs & (1 << secret.a) == 0 {
                return Err(E::AmbushPackInvalid);
            }
            free()?;
        }
        SecretKind::Stash => {
            if !on_map {
                return Err(E::StashInArena);
            }
            if secret.b | secret.c != 0 {
                return Err(E::SecretInvalid);
            }
            table_ok(secret.a)?;
        }
        SecretKind::Door => {
            if secret.a | secret.b | secret.c != 0 {
                return Err(E::SecretInvalid);
            }
            if !tiles.knows(place.byte) || tiles.is_passable(place.byte) {
                return Err(E::DoorNotWall);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_types::{cell_byte, Facing, TileKind};

    const MAP: [u8; 32] = [7; 32];

    fn pack(w: u8, h: u8) -> PackLayout {
        PackLayout {
            visible: 0,
            x: 1,
            y: 1,
            facing: Facing::N,
            width: w,
            height: h,
        }
    }

    fn tree(count: usize, index: usize) -> (Hash, Hash, [Hash; SECRET_DEPTH_MAX]) {
        let mut level = [[0u8; 32]; 1 << SECRET_DEPTH_MAX];
        for (i, h) in level.iter_mut().take(count).enumerate() {
            *h = cell_leaf(&MAP, i as u16, &CellSecret::NONE, &[i as u8; 16]);
        }
        let leaf = level[index];
        let mut proof = [[0u8; 32]; SECRET_DEPTH_MAX];
        let root = build(&mut level, count, index, &mut proof);
        (root, leaf, proof)
    }

    #[test]
    fn depth_rounds_up() {
        assert_eq!(depth(0), 0);
        assert_eq!(depth(1), 0);
        assert_eq!(depth(2), 1);
        assert_eq!(depth(3), 2);
        assert_eq!(depth(1024), 10);
        assert_eq!(
            depth(deadroll_types::SECRET_LEAVES_MAX as u16),
            SECRET_DEPTH_MAX
        );
    }

    #[test]
    fn every_leaf_proves_and_nothing_else_does() {
        for count in [1usize, 2, 3, 7, 8, 9, 300] {
            for index in [0, count / 2, count - 1] {
                let (root, leaf, proof) = tree(count, index);
                let d = depth(count as u16);
                assert!(verify(&root, leaf, index as u16, count as u16, &proof[..d]));
                // Another index, a changed leaf, a short or long proof, a padding index.
                if count > 1 {
                    let other = (index + 1) % count;
                    assert!(!verify(
                        &root,
                        leaf,
                        other as u16,
                        count as u16,
                        &proof[..d]
                    ));
                }
                let mut bad = leaf;
                bad[0] ^= 1;
                assert!(!verify(&root, bad, index as u16, count as u16, &proof[..d]));
                if d > 0 {
                    assert!(!verify(
                        &root,
                        leaf,
                        index as u16,
                        count as u16,
                        &proof[..d - 1]
                    ));
                }
                assert!(!verify(
                    &root,
                    leaf,
                    index as u16,
                    count as u16,
                    &proof[..d + 1]
                ));
                assert!(!verify(
                    &root,
                    leaf,
                    count as u16,
                    count as u16,
                    &proof[..d]
                ));
            }
        }
    }

    #[test]
    fn the_dm_cannot_swap_contents() {
        // A trap is committed; "nothing here" and another trap do not prove.
        let mut level = [[0u8; 32]; 4];
        let salt = [3; 16];
        let trap = CellSecret::trap(5, 12);
        level[0] = cell_leaf(&MAP, 0, &CellSecret::NONE, &[1; 16]);
        level[1] = cell_leaf(&MAP, 1, &trap, &salt);
        level[2] = cell_leaf(&MAP, 2, &CellSecret::NONE, &[2; 16]);
        let mut proof = [[0u8; 32]; 2];
        let root = build(&mut level, 3, 1, &mut proof);
        assert!(verify(
            &root,
            cell_leaf(&MAP, 1, &trap, &salt),
            1,
            3,
            &proof
        ));
        for lie in [
            CellSecret::NONE,
            CellSecret::trap(1, 12),
            CellSecret::door(),
        ] {
            assert!(!verify(
                &root,
                cell_leaf(&MAP, 1, &lie, &salt),
                1,
                3,
                &proof
            ));
        }
        // Another map's leaf, or the right contents under the pack tag.
        assert!(!verify(
            &root,
            cell_leaf(&[8; 32], 1, &trap, &salt),
            1,
            3,
            &proof
        ));
        let as_pack = pack_leaf(&MAP, 1, &PackRoster::default(), &salt);
        assert!(!verify(&root, as_pack, 1, 3, &proof));
    }

    #[test]
    fn leaves_number_packs_map_then_arenas() {
        let mut packs = [PackLayout::EMPTY; PACKS_MAX];
        packs[1] = pack(6, 7);
        packs[4] = pack(8, 6);
        let l = Leaves::new(10, 9, &packs);
        let map0 = PACK_LEAVES as u16;
        assert_eq!(l.count, map0 + 90 + 42 + 48);
        assert_eq!(l.of(Board::Pack(3)), Some(3));
        assert_eq!(l.of(Board::Pack(PACKS_MAX as u8)), None);
        assert_eq!(l.of(Board::Map(Cell { x: 9, y: 8 })), Some(map0 + 89));
        assert_eq!(l.of(Board::Map(Cell { x: 10, y: 0 })), None);
        assert_eq!(l.of(Board::Arena(1, Cell { x: 0, y: 0 })), Some(map0 + 90));
        assert_eq!(
            l.of(Board::Arena(4, Cell { x: 7, y: 5 })),
            Some(l.count - 1)
        );
        assert_eq!(l.of(Board::Arena(4, Cell { x: 8, y: 0 })), None);
        assert_eq!(l.of(Board::Arena(0, Cell { x: 0, y: 0 })), None);
        for leaf in 0..l.count {
            let b = l.board(leaf).unwrap();
            assert_eq!(l.of(b), Some(leaf));
        }
        assert_eq!(l.board(l.count), None);
    }

    #[test]
    fn threat_is_levels_weighted() {
        let role = |id: u8| Some(if id == 9 { Role::Boss } else { Role::Swarm });
        let w = ThreatWeights { elite: 2, boss: 3 };
        let s = |id, level, flags| Spawn {
            bestiary_id: id,
            level,
            flags,
            ..Spawn::default()
        };
        assert_eq!(threat(&[], role, w), 0);
        assert_eq!(threat(&[s(1, 2, 0), s(1, 1, 0)], role, w), 3);
        assert_eq!(threat(&[s(1, 2, SPAWN_ELITE)], role, w), 4);
        assert_eq!(threat(&[s(9, 4, 0), s(1, 3, 0)], role, w), 15);
    }

    const LIMITS: SecretLimits = SecretLimits {
        traps_min: 1,
        traps_max: 2,
        declared_max: [2, 1, 1, 2],
        trap_damage_max: 8,
    };

    #[test]
    fn declared_counts_follow_the_tier() {
        assert_eq!(check_declared(&[1, 0, 1, 2], &LIMITS, 0), Ok(()));
        assert_eq!(check_declared(&[1, 1, 0, 0], &LIMITS, 1), Ok(()));
        assert_eq!(
            check_declared(&[0, 0, 0, 0], &LIMITS, 0),
            Err(MapCheckError::TooFewTraps)
        );
        assert_eq!(
            check_declared(&[2, 1, 0, 0], &LIMITS, 1),
            Err(MapCheckError::TooManySecrets)
        );
        assert_eq!(
            check_declared(&[1, 0, 2, 0], &LIMITS, 0),
            Err(MapCheckError::TooManySecrets)
        );
        assert_eq!(
            check_declared(&[1, 1, 0, 0], &LIMITS, 0),
            Err(MapCheckError::AmbushUnlinked)
        );
        let mut packs = [PackLayout::EMPTY; PACKS_MAX];
        packs[2] = pack(6, 6);
        packs[2].visible = NO_SPAWN;
        packs[3] = pack(6, 6);
        assert_eq!(ambush_mask(&packs), 1 << 2);
    }

    #[test]
    fn cell_secret_rules() {
        use MapCheckError as E;
        let tiles = crate::tiles::tests::starter();
        let floor = cell_byte(TileKind::StoneFloor as u8, 0);
        let wall = cell_byte(TileKind::Wall as u8, 0);
        let map = |byte, occupied| SecretPlace {
            board: Board::Map(Cell { x: 1, y: 1 }),
            byte,
            occupied,
        };
        let arena = |byte| SecretPlace {
            board: Board::Arena(0, Cell { x: 1, y: 1 }),
            byte,
            occupied: false,
        };
        let ok = |_| Ok(());
        let check = |p: &SecretPlace, s: CellSecret| check_secret(p, &s, &tiles, &LIMITS, 1, ok);
        assert_eq!(check(&map(floor, false), CellSecret::NONE), Ok(()));
        assert_eq!(check(&map(floor, false), CellSecret::trap(8, 20)), Ok(()));
        assert_eq!(check(&arena(floor), CellSecret::trap(1, 0)), Ok(()));
        assert_eq!(check(&map(floor, false), CellSecret::ambush(0)), Ok(()));
        assert_eq!(check(&map(wall, false), CellSecret::stash(1)), Ok(()));
        assert_eq!(check(&map(wall, false), CellSecret::door()), Ok(()));
        assert_eq!(check(&arena(wall), CellSecret::door()), Ok(()));

        let junk = CellSecret {
            a: 1,
            ..CellSecret::NONE
        };
        assert_eq!(check(&map(floor, false), junk), Err(E::SecretInvalid));
        let pack_place = SecretPlace {
            board: Board::Pack(0),
            byte: floor,
            occupied: false,
        };
        assert_eq!(check(&pack_place, CellSecret::NONE), Err(E::SecretInvalid));
        for dmg in [0, 9] {
            assert_eq!(
                check(&map(floor, false), CellSecret::trap(dmg, 1)),
                Err(E::TrapDamageOutOfRange)
            );
        }
        assert_eq!(
            check(&map(wall, false), CellSecret::trap(1, 1)),
            Err(E::SecretNotPassable)
        );
        assert_eq!(
            check(&map(floor, true), CellSecret::trap(1, 1)),
            Err(E::SecretOnPoint)
        );
        assert_eq!(
            check(&arena(floor), CellSecret::ambush(0)),
            Err(E::AmbushInArena)
        );
        assert_eq!(
            check(&map(floor, false), CellSecret::ambush(1)),
            Err(E::AmbushPackInvalid)
        );
        assert_eq!(
            check(&arena(wall), CellSecret::stash(1)),
            Err(E::StashInArena)
        );
        assert_eq!(
            check(&map(floor, false), CellSecret::door()),
            Err(E::DoorNotWall)
        );
        let refused = check_secret(
            &map(wall, false),
            &CellSecret::stash(3),
            &tiles,
            &LIMITS,
            1,
            |_| Err(E::TableNotAllowed),
        );
        assert_eq!(refused, Err(E::TableNotAllowed));
    }
}

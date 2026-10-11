//! Monster roles for pack rules (docs/BESTIARY.md, section 2.1) and the P0 roster.
//!
//! TODO(post-demo): read roles from the registry `Bestiary`; `check_map` takes the lookup as a
//! function so the table can be swapped without touching the checks.

/// How a monster template enters a pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Swarm,
    Fast,
    Tank,
    /// At most one per pack.
    Loner,
    /// Its own pack; exactly one per map.
    Boss,
    /// A boss summon: only in the boss pack (docs/BESTIARY.md, section 6).
    Helper,
}

/// P0 bestiary ids. Shared with the registry `Bestiary` and the session program.
pub mod ids {
    pub const WOLF: u8 = 0;
    pub const GOBLIN: u8 = 1;
    pub const GOBLIN_CHIEFTAIN: u8 = 2;
    pub const GOBLIN_WARRIOR: u8 = 3;
    pub const GOBLIN_SHAMAN: u8 = 4;
}

/// Role of a P0 bestiary id, `None` for unknown ids.
pub const fn p0_role(bestiary_id: u8) -> Option<Role> {
    match bestiary_id {
        ids::WOLF => Some(Role::Fast),
        ids::GOBLIN => Some(Role::Swarm),
        ids::GOBLIN_CHIEFTAIN => Some(Role::Boss),
        ids::GOBLIN_WARRIOR | ids::GOBLIN_SHAMAN => Some(Role::Helper),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_roster() {
        assert_eq!(p0_role(ids::WOLF), Some(Role::Fast));
        assert_eq!(p0_role(ids::GOBLIN), Some(Role::Swarm));
        assert_eq!(p0_role(ids::GOBLIN_CHIEFTAIN), Some(Role::Boss));
        assert_eq!(p0_role(ids::GOBLIN_WARRIOR), Some(Role::Helper));
        assert_eq!(p0_role(ids::GOBLIN_SHAMAN), Some(Role::Helper));
        assert_eq!(p0_role(5), None);
    }
}

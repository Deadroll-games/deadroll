//! Damage by aspect, read from what a hit already reports (nothing here changes a fight).
//!
//! A hit's damage is `max(floor(sum of the packets after defense / BP), min_damage)` plus the
//! over-time damage a resonance made tick at once. The split follows the shares the combat math
//! used: each packet's `after_defense`, the whole numbers handed out by the largest remainder so
//! the parts add up to `HitResult::damage` exactly.
use deadroll_combat::{
    hit::armor_vs,
    modifiers::{armor_cut, resist_cut},
    states::{tick_turn_start_with, States},
    Balance, HitInput, HitResult, OpKind, ResonanceTables,
};
use deadroll_types::{Aspect, StateKind, BP};

/// Aspect-indexed whole numbers.
pub type ByAspect = [u64; Aspect::COUNT];

/// One hit by aspect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitSplit {
    /// The hit's damage, adding up to `HitResult::damage`.
    pub damage: ByAspect,
    /// What each packet's resistance cut, after armor, rounded to a whole number.
    pub resisted: ByAspect,
}

/// `total` shared out in proportion to `weights`, whole numbers adding up to `total`: each gets
/// the floor of its share, the rest goes one by one to the biggest fractions (the lower index
/// first on a tie). With no weight at all everything goes to the first entry.
pub fn share_out(total: u64, weights: &[u64]) -> Vec<u64> {
    let mut out = vec![0u64; weights.len()];
    if out.is_empty() {
        return out;
    }
    let sum: u128 = weights.iter().map(|&w| w as u128).sum();
    if sum == 0 {
        out[0] = total;
        return out;
    }
    let mut fractions: Vec<(u128, usize)> = Vec::with_capacity(weights.len());
    for (i, &w) in weights.iter().enumerate() {
        let scaled = total as u128 * w as u128;
        out[i] = (scaled / sum) as u64;
        fractions.push((scaled % sum, i));
    }
    let given: u64 = out.iter().sum();
    fractions.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for &(_, i) in fractions.iter().take((total - given) as usize) {
        out[i] += 1;
    }
    out
}

/// Splits the last hit of the sim (`result` and the `input` it was rolled on) by aspect.
pub fn split_hit(result: &HitResult, input: &HitInput, balance: &Balance) -> HitSplit {
    let mut split = HitSplit::default();
    let tick = result.tick_damage as u64;
    let direct = (result.damage as u64).saturating_sub(tick);
    let live: Vec<_> = result.packets().iter().filter(|p| !p.immune).collect();
    let weights: Vec<u64> = live.iter().map(|p| p.after_defense).collect();
    if !live.is_empty() {
        for (p, part) in live.iter().zip(share_out(direct, &weights)) {
            split.damage[p.aspect as usize] += part;
        }
    } else if direct > 0 {
        split.damage[result.packets().first().map_or(0, |p| p.aspect as usize)] += direct;
    }
    // The instant ticks: each acting TickNow op deals `amount`, on the base of its resonance.
    let mut owners = Vec::new();
    let mut amounts = Vec::new();
    for (slot, res) in result.resonances.iter().enumerate() {
        let Some(res) = res.filter(|r| r.active) else {
            continue;
        };
        for op in result
            .ops(slot)
            .filter(|o| o.acted && o.kind == OpKind::TickNow)
        {
            owners.push(res.base);
            amounts.push(op.amount as u64);
        }
    }
    if tick > 0 {
        if owners.is_empty() {
            let first = live.first().map_or(0, |p| p.aspect as usize);
            split.damage[first] += tick;
        } else {
            for (aspect, part) in owners.iter().zip(share_out(tick, &amounts)) {
                split.damage[*aspect as usize] += part;
            }
        }
    }
    for p in &live {
        let mut value = p.rolled;
        if deadroll_combat::is_physical(p.aspect) {
            let armor = armor_vs(&input.defender, p.aspect);
            let pen = p.pen_attacker_bp.saturating_add(p.pen_state_bp);
            value = armor_cut(value, armor, pen, balance.armor_k);
        }
        let after = resist_cut(value, p.resist_bp, balance.resist_cap_bp);
        let cut = value.saturating_sub(after);
        split.resisted[p.aspect as usize] += (cut + BP as u64 / 2) / BP as u64;
    }
    split
}

/// The aspects that own the over-time states: the aspect whose native state it is. A thawing
/// Freeze is the Chill aspect's (Burning Frost).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DotAspects {
    pub burning: Aspect,
    pub bleeding: Aspect,
    pub thaw: Aspect,
}

impl DotAspects {
    pub fn of(tables: &ResonanceTables) -> Self {
        let owner = |kind: StateKind| {
            Aspect::ALL
                .iter()
                .copied()
                .find(|a| tables.native(*a) == Some(kind))
                .unwrap_or_default()
        };
        DotAspects {
            burning: owner(StateKind::Burning),
            bleeding: owner(StateKind::Bleeding),
            thaw: owner(StateKind::Chill),
        }
    }
}

/// The over-time damage of one turn start by state kind: `(burning, bleeding, thaw)`, adding up
/// to `total`. `before` are the states before the tick; the tick is the same pure function the
/// rules call, run on a copy without the Bleeding states.
pub fn split_dot(before: &States, total: u32, thaw: u32, balance: &Balance) -> (u32, u32, u32) {
    let mut rest = *before;
    for s in rest.iter_mut().filter(|s| s.kind == StateKind::Bleeding) {
        *s = Default::default();
    }
    let without_bleeding = tick_turn_start_with(&mut rest, balance).dot_damage;
    let bleeding = total.saturating_sub(without_bleeding);
    let burning = without_bleeding.saturating_sub(thaw);
    (burning, bleeding, total - bleeding - burning)
}

#[cfg(test)]
mod tests {
    use deadroll_combat::PacketResult;
    use deadroll_types::ActiveState;

    use super::*;

    #[test]
    fn shares_add_up_and_follow_the_weights() {
        assert_eq!(share_out(10, &[1, 1, 1]), vec![4, 3, 3]);
        assert_eq!(share_out(7, &[0, 5, 0]), vec![0, 7, 0]);
        assert_eq!(share_out(5, &[0, 0]), vec![5, 0]);
        assert_eq!(share_out(0, &[3, 4]), vec![0, 0]);
        assert_eq!(share_out(9, &[]), Vec::<u64>::new());
        for total in [0u64, 1, 2, 9, 100, 12_345] {
            for w in [[1u64, 2, 3], [10, 0, 7], [999_999, 1, 1]] {
                assert_eq!(share_out(total, &w).iter().sum::<u64>(), total);
            }
        }
    }

    fn packet(aspect: Aspect, after_defense: u64) -> PacketResult {
        PacketResult {
            aspect,
            after_defense,
            rolled: after_defense,
            ..Default::default()
        }
    }

    fn result(packets: &[PacketResult], damage: u32) -> HitResult {
        let mut r = HitResult {
            damage,
            ..Default::default()
        };
        r.packet_count = packets.len() as u8;
        r.packets[..packets.len()].copy_from_slice(packets);
        r
    }

    #[test]
    fn a_mixed_hit_is_split_by_its_packets_and_adds_up() {
        let bp = BP as u64;
        let r = result(
            &[packet(Aspect::Fire, 6 * bp), packet(Aspect::Cold, 4 * bp)],
            10,
        );
        let s = split_hit(&r, &HitInput::default(), &Balance::DEFAULT);
        assert_eq!(s.damage.iter().sum::<u64>(), 10);
        assert_eq!(s.damage[Aspect::Fire as usize], 6);
        assert_eq!(s.damage[Aspect::Cold as usize], 4);
    }

    #[test]
    fn an_odd_split_still_adds_up_and_the_minimum_damage_stays_with_the_packets() {
        let bp = BP as u64;
        let r = result(
            &[packet(Aspect::Fire, bp / 2), packet(Aspect::Cold, bp / 2)],
            1,
        );
        let s = split_hit(&r, &HitInput::default(), &Balance::DEFAULT);
        assert_eq!(s.damage.iter().sum::<u64>(), 1);
        // Minimum damage on a hit whose packets were all cut to nothing.
        let r = result(&[packet(Aspect::Fire, 0), packet(Aspect::Cold, 0)], 3);
        let s = split_hit(&r, &HitInput::default(), &Balance::DEFAULT);
        assert_eq!(s.damage.iter().sum::<u64>(), 3);
    }

    #[test]
    fn immune_packets_get_nothing_and_instant_ticks_go_to_the_base() {
        let bp = BP as u64;
        let mut immune = packet(Aspect::Cold, 5 * bp);
        immune.immune = true;
        let mut r = result(&[immune, packet(Aspect::Fire, 8 * bp)], 8 + 5);
        r.tick_damage = 5;
        let s = split_hit(&r, &HitInput::default(), &Balance::DEFAULT);
        assert_eq!(s.damage.iter().sum::<u64>(), 13);
        assert_eq!(s.damage[Aspect::Cold as usize], 0);
        // No resonance recorded: the tick falls to the first live packet.
        assert_eq!(s.damage[Aspect::Fire as usize], 13);
    }

    #[test]
    fn resistance_cut_is_what_the_cap_took_off() {
        let bp = BP as u64;
        let mut p = packet(Aspect::Fire, 5 * bp);
        p.rolled = 10 * bp;
        p.resist_bp = BP / 2;
        let b = Balance::DEFAULT;
        let r = result(&[p], 5);
        let s = split_hit(&r, &HitInput::default(), &b);
        let after = resist_cut(10 * bp, BP / 2, b.resist_cap_bp);
        assert_eq!(
            s.resisted[Aspect::Fire as usize],
            (10 * bp - after + bp / 2) / bp
        );
        assert!(s.resisted[Aspect::Fire as usize] > 0);
    }

    #[test]
    fn dot_damage_splits_by_state_kind() {
        let b = Balance::DEFAULT;
        let mut states: States = Default::default();
        states[0] = ActiveState {
            kind: StateKind::Burning,
            turns: 3,
            power: 7,
            source: 1,
        };
        states[1] = ActiveState {
            kind: StateKind::Bleeding,
            turns: 2,
            power: 4,
            source: 1,
        };
        states[2] = ActiveState {
            kind: StateKind::Freeze,
            turns: 1,
            power: 5,
            source: 1,
        };
        let mut live = states;
        let tick = tick_turn_start_with(&mut live, &b);
        let (burning, bleeding, thaw) = split_dot(&states, tick.dot_damage, tick.thaw_damage, &b);
        assert_eq!((burning, bleeding, thaw), (7, 4, 5));
        assert_eq!(burning + bleeding + thaw, tick.dot_damage);
    }
}

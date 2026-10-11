---
name: combat-math
description: Use when working in crates/combat or crates/types: dice, damage packets, modifiers, armor, resistances, states and resonances. Pure Rust, integers only, shared by the program and the website.
---

# Working in `crates/combat`

Pure Rust. No Anchor, no Solana SDK, no floats. `#![no_std]` + `alloc` so it compiles for the program and for WASM.
Rules come from `docs/COMBAT_MATH.md` and `docs/DAMAGE_TYPES_AND_MIXES.md`. If the code and the doc disagree, the doc wins: ask before changing behaviour.

## Numbers

- Amounts (HP, damage, cells, turns): `u32`.
- Percentages and multipliers: basis points `u32`, `BP = 10_000` means 100% or x1.
- Intermediate math in `u64` (or `u128` for long chains): **multiply first, divide last**, round down once at the end.
- **No subtraction for debuffs.** Bonus multiplies, counterweight divides:
  `value * (BP + bonus) / (BP + counterweight)`.
- Armor with diminishing returns, never 100%: `reduction = armor * BP / (armor + K)`.
- Saturate instead of overflow.

## Dice: `rng.rs`

```rust
pub trait RollSource {
    /// Uniform u64 for roll number `index` of the current action.
    fn next_u64(&mut self) -> u64;
}
pub fn d20(src: &mut impl RollSource) -> u8 { (src.next_u64() % 20) as u8 + 1 }
pub fn d100(src: &mut impl RollSource) -> u8 { (src.next_u64() % 101) as u8 } // 0..=100, point in range
pub fn chance(src: &mut impl RollSource, bp: u32) -> bool { (src.next_u64() % BP as u64) < bp as u64 }
```

Sources:
- `VrfRoll { seed: [u8; 32], index: u32 }`: the production source. `next_u64` = first 8 bytes of `hash(seed ‖ index)`, then `index += 1`. The `session` program builds it from the MagicBlock VRF callback value, one VRF value per action.
- `SeededRoll` (splitmix64 from a `u64` seed) for tests and the simulator.
- `SlotHashesRoll` lives in the `session` program only, as the VRF fallback.

The hash function must be available in `no_std` and in WASM (e.g. `sha2` with `default-features = false`), so the website can replay any hit from the seed in the event and get the same breakdown.

## Hit order (fixed, see COMBAT_MATH section 6)

1. Immunity to the skill base aspect -> no damage of that aspect, no tint rolls for it.
2. `d20` + attack bonus vs target defense. Nat 20 = hit + crit, nat 1 = miss. Miss ends the hit.
3. Tint rolls: for each skill aspect (base), each predisposition tint rolls `chance(tier_bp)`. 1 success = pair resonance (stretch: 2 = triple).
4. Packets: weapon base range converted to skill aspects by descending shares + added packets (predisposition flat damage). Each packet keeps its aspect and inherits skill tags (magical/combat, strike/projectile).
5. Per packet: increased (sum of matching % bonuses), then multipliers (crit, resonance), then attacker counterweights.
6. One `d100` = point inside every packet range: `min + (max - min) * roll / 100`.
7. Defense per packet: physical -> armor (reduced by pierce), elemental -> resistance as counterweight; vulnerabilities as multipliers.
8. Sum packets, round down once, minimum 1 damage on a hit unless immune.
9. Native state per packet: `chance(state_bp)` vs target state resistance. One instance per state kind on a target: strongest stays, duration refreshed to the larger.

## Required outputs

`resolve_hit(...) -> HitResult` returns everything the UI and the event log need:
rolls (d20, tint rolls, d100, state rolls), per-packet numbers after each step, resonance triggered (if any), states applied, final damage.

## Tests

- Unit test every step with fixed `SeededRoll` seeds.
- Property tests (if `proptest` is available): damage never negative, never overflows, counterweights never reach zero.
- A golden test: a full hit with known seed and expected breakdown, so the program and the website can compare against it.

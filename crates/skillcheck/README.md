# deadroll-skillcheck

Skill builder checks from `docs/SKILL_BUILDER.md`, one crate for the character program and the
website so the limits cannot be bypassed.

```rust
use deadroll_skillcheck::{presets, validate_starting, check_loadout, derive};
use deadroll_types::Class;

for p in presets::for_class(Class::Mage) {
    validate_starting(&p.blocks, Class::Mage)?;   // builder rules + class limits
    let d = derive(&p.blocks);                    // shares_bp, power_bp, fusion, cooldown
}
check_loadout(presets::MAGE.iter().map(|p| &p.blocks))?;
```

`SkillBlocks` (12 bytes borsh): `category, purpose, source, form, target, aspects[3],
aspect_count, effect, trigger, aura_radius`. Unused aspect slots stay at the default (`Fire`).
Fusion id of an ordered pair: `1 + first * 6 + second` (`0` = none).

Monster skills (patch 6, `monster`): the registry `monster_skills` record (32 B: present, `SkillBlocks`, `SkillExt`, flags with bit 0 kin only) and its 16-byte session copy. `validate_monster_skill` is `validate_skill` without class limits and with Heal allowed as an active or reactive skill on one unit (`HealCategory` for a passive or aura heal). `check_monster_selection(pool, mask, slots)` is the spawn rule: exactly `min(slots, pool)` chosen skills of each category, no bit past the pool.

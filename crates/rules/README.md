# deadroll-rules

Game rules shared by the session program and the balance bench. Pure Rust, `no_std`, integers only.

`crates/combat` resolves one hit or one effect; this crate decides what goes into it and what happens around it:

| Module | What |
|---|---|
| `config` | `BalanceConfig`: every balance number (`combat::Balance`, `combat::SupportBalance`, `skillcheck::SkillTuning`, `PlayerTuning`, `MonsterScaling`). `DEFAULT` is what the programs use. |
| `units` | unit ids, initiative roll, cooldowns, reach |
| `player` | speed and AP, initiative, accuracy, evasion, stat damage increases, which stat scales a state (by power source), class and race trait tiers, HP, levels, spending stat points, creation rules |
| `monster` | `MonsterTemplate::from_record` (a registry bestiary record) → `MonsterProfile` at a level and rank (regular, elite, boss); skill slots and the spawn's choice (`select_skills`), kin |
| `monster_ai` | the monsters' choice of action (`plan`): the simulator and the website's auto mode |
| `registry` | (feature `serde`) `assets/registry/*.json` into the bytes the chain holds, for tools and tests |
| `input` | `Attacker` / `Defender` / `SupportInput` from a hero, a skill or a monster; effects of skills |
| `turn` | `start_of_turn`: over-time damage, Freeze / Stun skip, durations, cooldowns |
| `resolve` | one action from its dice: what `consume_randomness` does to the state |
| `aura` | auras of passive and reactive skills (docs/SKILL_BUILDER.md 3.5): who they reach, the strongest per kind merged into a unit's mods |
| `line` | the line target: from the caster through the target to the reach, a wall ends it |
| `reactive` | reactive skills (docs/SKILL_BUILDER.md 3.2): which events trigger them and what they do |

```rust
use deadroll_rules::{resolve, BalanceConfig};
let out = resolve::attack_monster(&BalanceConfig::DEFAULT, attacker, &hero, skill, &mods,
    &profile, &mut monster, &mut monster_mods, &mut input, &mut result, &mut rolls);
```

The program calls everything with `BalanceConfig::DEFAULT` (`programs/session/src/rules.rs`, `combat_input.rs`, `consume_randomness.rs`; `programs/character/src/rules.rs`). `tools/balance` calls the same functions with a tuned config, so a rule changed here reaches both.

Feature `serde`: `Serialize` / `Deserialize` on `BalanceConfig` (tools only).

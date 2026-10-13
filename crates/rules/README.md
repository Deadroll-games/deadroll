# deadroll-rules

Game rules shared by the session program and the balance bench. Pure Rust, `no_std`, integers only.

`crates/combat` resolves one hit or one effect; this crate decides what goes into it and what happens around it:

| Module | What |
|---|---|
| `config` | `BalanceConfig`: every balance number (`combat::Balance`, `combat::SupportBalance`, `skillcheck::SkillTuning`, `PlayerTuning`, `MonsterScaling`). `DEFAULT` is what the programs use. |
| `units` | unit ids, initiative roll, cooldowns, reach |
| `player` | AP, initiative, class weapon and increases, attack bonus, crit, defense, HP, creation rules |
| `monster` | `MonsterBase` (a template, content) → `MonsterProfile` at a level; the P0 table in `monster::p0` |
| `input` | `Attacker` / `Defender` / `SupportInput` from a hero, a skill or a monster; effects of skills |
| `turn` | `start_of_turn`: over-time damage, Freeze / Stun skip, durations, cooldowns |
| `resolve` | one action from its dice: what `consume_randomness` does to the state |

```rust
use deadroll_rules::{resolve, BalanceConfig};
let out = resolve::attack_monster(&BalanceConfig::DEFAULT, attacker, &hero, skill, &mods,
    &profile, &mut monster, &mut monster_mods, &mut input, &mut result, &mut rolls);
```

The program calls everything with `BalanceConfig::DEFAULT` (`programs/session/src/rules.rs`, `combat_input.rs`, `consume_randomness.rs`; `programs/character/src/rules.rs`). `tools/balance` calls the same functions with a tuned config, so a rule changed here reaches both.

Feature `serde`: `Serialize` / `Deserialize` on `BalanceConfig` (tools only).

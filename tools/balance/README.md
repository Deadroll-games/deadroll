# balance

Native balance bench over `deadroll-sim`.

```
cargo run -p balance --release            # presets from tools/balance/presets
cargo run -p balance --release -- <dir>   # or another preset folder (also DEADROLL_PRESETS)
```

Tabs:
- **Heroes**: race, class, level, stats, predisposition, tier, skills (ids of skill presets). Shows HP, AP, initiative, defense, attack bonus, crit, and the problems a Run would report.
- **Skills**: the in-game builder (every block from its enum), checked by `skillcheck` like the character program: builder rules, starting limits per class, derived power, shares and cooldown.
- **Monsters**: a template at level 0 (attack, defense, armor, HP, speed, initiative, XP, boss) and its numbers by level. Resistances and immunities of a bestiary monster are its registry record (`assets/registry/bestiary.json`), shown read-only; a monster without a `bestiary_id` has its own lists.
- **Encounters**: monsters, levels and counts on an open arena.
- **Config**: every number of `BalanceConfig`; basis points as percent, changed values marked, Save / Load JSON.
- **Results**: win rate, rounds to win, hits to kill (p10 / p50 / p90), damage per turn by hero, crit and over-time shares, control uptime, histograms, and random builds (top 10 by damage per turn, "Save as preset").

The top bar picks the party (up to 4 heroes) and the encounter, the number of fights, the seed, the round cap and the AI heal threshold. Run works in the background with a progress bar and Cancel. **Save presets** writes every preset back to its JSON file (deleted presets lose their file); **Reload** drops unsaved changes.

Presets: `presets/{heroes,skills,monsters,encounters}/<id>.json`, the id is the file name. The shipped files are the current game data (skillcheck starter skills, the demo characters, the P0 bestiary, the demo map packs).

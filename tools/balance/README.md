# balance

Native balance bench over `deadroll-sim`.

```
cargo run -p balance --release            # presets from tools/balance/presets
cargo run -p balance --release -- <dir>   # or another preset folder (also DEADROLL_PRESETS)
```

Tabs:
- **Heroes**: race, class, level, stats, predisposition, tier, skills (ids of skill presets). Shows HP, AP, initiative, defense, attack bonus, crit, and the problems a Run would report.
- **Skills**: the in-game builder (every block from its enum), checked by `skillcheck` like the character program: builder rules, starting limits per class, derived power, shares and cooldown.
- **Monsters**: the registry bestiary (`assets/registry/bestiary.json`): every number of a record, its skill pool (`monster_skills.json`), its slots by rank and its numbers by level. **Save bestiary** writes the JSON back; `tools/scripts` seeds it on chain.
- **Encounters**: bestiary monsters, levels, counts, elite and the chosen skills (checked like `finalize_map`) on an open arena.
- **Config**: every number of `BalanceConfig`; basis points as percent, changed values marked, Save / Load JSON.
- **Results**: win rate, rounds to win, hits to kill (p10 / p50 / p90), damage per turn by hero, crit and over-time shares, control uptime, histograms, and random builds (top 10 by damage per turn, "Save as preset").

The top bar picks the party (up to 4 heroes) and the encounter, the number of fights, the seed, the round cap and the AI heal threshold. Run works in the background with a progress bar and Cancel. **Save presets** writes every preset back to its JSON file (deleted presets lose their file); **Reload** drops unsaved changes.

Presets: `presets/{heroes,skills,encounters}/<id>.json`, the id is the file name. The shipped files are the current game data (skillcheck starter skills, the demo characters, the demo map packs). Monsters are not presets: they are the registry records.

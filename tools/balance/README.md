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

Results also breaks the numbers down by skill, by aspect (damage, its share of the hero's or the party's damage, and what resistance cut) and by monster kind.

## Run history (Google Sheets)

After every finished Run (not a cancelled one) the bench sends its results to a Google Apps Script web app: sheet `Runs` gets one row per Run, sheet `Metrics` one row per number per Run (party, heroes, skills, aspects, monsters), each next to the same number of the previous Run with the same party, encounter, number of fights and seed. The `Runs` row lists every number that changed since the previous Run (Config, heroes, skills, monsters, encounters) as `name: old -> new`. The **Note** field next to Run is sent with each Run and kept until you change it.

- The web app URL is read from the environment variable `DEADROLL_SHEET_URL`, or from `private/balance-sheet-url.txt`. It is never shown, logged or committed. Without a URL the bench works as before and the Results tab says "history off".
- Sending runs on a background thread. Every request is first written to `private/balance-log-queue.jsonl`, then the queue is sent oldest first. A failed send (no network, an error status, a reply other than `ok`) keeps the request there, shows a warning in the Results tab and is retried before the next Run. The web app answers a POST with a redirect after it has written the rows, so the reply is read with a GET and the POST is never repeated.
- Local memory, all in `private/` (not committed): `balance-log-last.json` (snapshot of the last Run), `balance-log-state.json` (last run id and the numbers of the last Run per party / encounter / fights / seed), `balance-log-queue.jsonl`. Delete them to start the history over.
- Values are per-fight averages over the batch, except rates (percent), percentiles and the largest hit. Skill damage is the skill's direct damage (hit, splash, instant tick); damage over time has no skill owner and shows under hero, aspect and party, in the aspect that owns the state (Burning, Bleeding, a thawing Freeze).

Presets: `presets/{heroes,skills,encounters}/<id>.json`, the id is the file name. The shipped files are the current game data (skillcheck starter skills, the demo characters, the demo map packs). Monsters are not presets: they are the registry records.

# GROK_FOR_CLAUDE.md

Grok Build is a separate local coding agent. Claude may call it only through the `grok` CLI. Claude subagents cannot run Grok models. Never open interactive `grok` without `-p`: that starts a TUI and blocks.

Prerequisite: `grok --version` works and the user has already logged in. If the binary is missing, tell the user to install Grok Build and login. Do not invent API keys.

Use Grok for isolated tests, scripts, boilerplate, docs, second-opinion diff review, and a bounded module that does not touch architecture. Do not use Grok for architecture, public API design, multi-file domain refactors, taste-sensitive frontend, vague "finish the project", or anything that needs Claude's current session unless that context is copied into the Grok prompt. Never run two write-capable agents on the same files at once.

Every prompt must include: goal; allowed files/dirs; forbidden files/dirs; definition of done; "at the end list changed files and a 5-line summary". Prefer `--prompt-file` for long briefs. Always run from the repo root or pass `--cwd`. Always add `--no-auto-update --no-alt-screen`. Headless runs without `--always-approve` often hang on permission prompts, so automation must pass `--always-approve` plus either `--sandbox read-only` or narrow `--allow/--deny` and/or `--worktree`. After every write run, inspect `git status` and `git diff`. Do not trust Grok blindly. Do not let Grok `git push`, `git reset --hard`, or delete files. On quota/usage errors stop delegating.

Base: `grok --no-auto-update --no-alt-screen --cwd "$PWD" -p "TASK"`

Read-only scout/review:
`grok --no-auto-update --no-alt-screen --cwd "$PWD" --sandbox read-only --always-approve --permission-mode plan --effort medium -p "TASK. Do not edit files. Return findings only."`

Write in a tight box:
`grok --no-auto-update --no-alt-screen --cwd "$PWD" --always-approve --sandbox workspace --effort high --max-turns 30 --allow 'Write(tests/**)' --allow 'Edit(tests/**)' --allow 'Read' --allow 'Grep' --allow 'Bash(git *)' --deny 'Bash(rm *)' --deny 'Bash(git push*)' --deny 'Bash(git reset --hard*)' -p "TASK"`
Tighten Write/Edit globs to the real folder.

Safer bigger write:
`grok --no-auto-update --cwd "$PWD" --worktree --always-approve --sandbox workspace --effort high -p "TASK"`

Long brief:
`grok --prompt-file /tmp/grok-task.md --always-approve --no-auto-update --no-alt-screen --cwd "$PWD"`

Continue last session in this directory: add `-c`. Resume id: `-r SESSION_ID`. Fork that session: `--fork-session -r SESSION_ID`. Capture id: `--output-format json` and read `sessionId`.

Recipes:

- Map a file: read-only command + "Map how FILE works. List the 5 most relevant files and one sentence each. Do not edit."
- Review diff: read-only command + "Review this working tree. Do not edit." then embed `git status --short` and `git diff`.
- Tests only: write-in-a-box command + "Write tests for FILE. Change only test files. Do not edit FILE."
- Bounded bug: worktree command + "Fix DESC. Touch only FILE_LIST. Do not refactor unrelated code. Run relevant tests."
- Second opinion on Claude's patch: read-only command + "Adversarial review of this diff. Look for broken contracts, missing edge cases, extra scope." plus `git diff`.

Flags allowed: `-p`/`--single` one-shot prompt; `--prompt-file PATH`; `--cwd PATH`; `--always-approve`/`--yolo`; `--permission-mode plan|acceptEdits|auto`; `--sandbox read-only|workspace|strict`; `--worktree`; `--allow RULE`; `--deny RULE` (repeatable; `Read`, `Grep`, `Write(tests/**)`, `Edit(src/foo.ts)`, `Bash(git *)`, `Bash(npm test*)`); `--tools a,b`; `--disallowed-tools web_search,web_fetch,Agent`; `--no-subagents`; `--disable-web-search`; `--effort low|medium|high|xhigh`; `-m grok-4.6`; `--max-turns N` (20-40 for writes); `--check`; `-c`; `-r ID`; `--session-id UUID` (start, does not resume); `--fork-session`; `--output-format plain|json`; `--rules "..."`; `--no-plan`; `--no-auto-update`; `--no-alt-screen`.

Default effort is medium. Use high/xhigh only for hard bugs. Prefer plain output unless a session id is needed. If Grok wrote the wrong files, revert, tighten globs, retry once. Then keep, edit, or throw away the diff.

Default session policy: each Grok task starts in a fresh session. Do not use `-c` or `-r` unless the user explicitly asks to continue the same Grok job. One task = one new session; do not reuse a session across tasks or folders.

---

## Deadroll addendum

### Good Grok tasks here
Everything marked **(G)** in `agents/TASKS.md`: sprite DSL files, presets and registry JSON, test suites for existing code, upload/deploy scripts, UI components on mock data, `tools/art`.

### Never give Grok
`crates/types`, `agents/CONTRACTS.md`, account layouts, CPI and signer checks, VRF or Session Keys integration, `resolve_hit` order, anything in `docs/`, anything touching keys or `.env`.

### Always paste into the brief
- the root `CLAUDE.md` hard rules (section 5), including "never mention secret lore" and "no em dashes in generated text";
- the zone `CLAUDE.md` of the folder Grok writes to;
- for sprites: the full `pixel-sprite` skill text and one reference DSL file;
- for UI: the exact props/types interface the component must accept.

### Sprite task example

```
grok --no-auto-update --no-alt-screen --cwd "$PWD" --always-approve --sandbox workspace \
  --effort medium --max-turns 20 --disable-web-search \
  --allow 'Read' --allow 'Grep' --allow 'Write(assets/sprites/wolf.dsl)' --allow 'Edit(assets/sprites/wolf.dsl)' \
  --allow 'Bash(cargo run -p art*)' --deny 'Bash(rm *)' --deny 'Bash(git *)' \
  --prompt-file /tmp/grok-wolf.md
```

Then Claude runs `cargo run -p art -- assets/sprites/wolf.dsl --scale 8 --out /tmp/wolf.png`, looks at the PNG, and keeps or rejects.

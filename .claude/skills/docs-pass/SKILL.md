---
name: docs-pass
description: Use for edits to docs/, a docs pass, docs-proposals, or any request to change a design document.
---

# Docs pass

Agents never edit `docs/`. Proposals go into a copy that the user reviews and moves.

## Steps

1. Copy `docs/` to `target/docs-new/`. Make every change in the copy only.
2. Style: English, no em dashes, no secret lore, only the requested changes. If a doc contradicts the code, write the contradiction down; do not guess which one is right.
3. Report: changed sections per file, one line each, then the contradictions.
4. Give the user two commands for PowerShell, run from `D:\RPG`:

```
git diff --no-index docs target/docs-new
Copy-Item -Recurse -Force target\docs-new\* docs\
```

5. When the user says it is moved: commit `docs/` in one commit, open a PR, delete `target/docs-new/`.

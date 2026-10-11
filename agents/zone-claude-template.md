# Template for a zone CLAUDE.md

Copy into `programs/<name>/CLAUDE.md` or `crates/<name>/CLAUDE.md` and fill the brackets. Keep it under 40 lines.

```
# Zone: <programs/session>

Owner lane: <L7>. Read the root CLAUDE.md first.

## What lives here
<one paragraph from docs/ARCHITECTURE.md section 3.1 or 5>

## Rules for this zone
- Docs: <docs/... sections>
- Accounts owned here: <list from agents/CONTRACTS.md>
- CPI out: <list> (wrappers in `src/cpi_out/`, never `src/cpi/`) / CPI in (who may call us): <list>
- Forbidden: editing other zones, crates/types, agents/CONTRACTS.md

## Commands
<build and test commands for this zone only. Programs always build with the flag:
 anchor build --tools-version v1.54 -p <program_lib_name>
 cargo test -p <package>

## Done means
- builds, all tests green, clippy clean
- every instruction has happy path + rejection tests
- events emitted for every state change
```

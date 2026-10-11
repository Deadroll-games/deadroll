---
name: anchor-instruction
description: Use when adding or changing an instruction in any Deadroll Anchor program (registry, render, character, items, map, session, city). Covers file layout, accounts, signer and trust rules, CPI between game programs, VRF and Session Keys seams, errors, events and required tests.
---

# Adding an Anchor instruction

## Steps

1. Read the zone `CLAUDE.md`, the doc section the task links and `agents/CONTRACTS.md` (seeds, fields, CPI surface).
2. Plan in 5-10 lines: accounts (`mut`, PDA seeds), signer, checks, CPI out, state changes, events, errors, tests.
3. Create `src/instructions/<name>.rs` with `#[derive(Accounts)] pub struct <Name>` and `pub fn handler(...)`.
4. Register in `src/instructions/mod.rs`, add one line in `src/lib.rs`.
5. Errors in `src/errors.rs`, one variant per distinct failure, clear message.
6. `emit!` an event for every state change (names from `CONTRACTS.md` section 7).
7. Tests: happy path + one per rejection. LiteSVM for single-program tests; cross-program flows go to `tests/`.
8. `anchor build --tools-version v1.54 -p <program>` and tests until green (always pass the flag, see root `CLAUDE.md`, section 2). If the IDL changed, regenerate `sdk/`.

## Accounts

- `#[derive(InitSpace)]`, `space = 8 + X::INIT_SPACE`. Fixed arrays, no unbounded `Vec`.
- `version: u8` first field. Store `bump`.
- Seeds exactly as `CONTRACTS.md` section 2. Never invent new seeds for existing accounts.
- **Game moves never create accounts.** Only setup instructions `init`.
- Large accounts (> 10 KB, e.g. `RasterizerCode`) cannot be created by `init` through CPI: the client creates them with `SystemProgram::create_account` (owner = program) in the same transaction, then the program initializes the header. See the `rasterizer` skill.

## Trust between game programs

Each program has a signer PDA `["authority"]`. A sensitive instruction (XP, death, rewards, status changes) checks the caller:

```rust
#[account(
    seeds = [b"authority"],
    bump,
    seeds::program = config.programs[ProgramIdx::Session as usize],
)]
pub caller_authority: Signer<'info>,
pub config: Account<'info, GameConfig>, // read from registry, no CPI needed
```

The caller signs with `CpiContext::new_with_signer(..., &[&[b"authority", &[bump]]])`.
CPI only through the callee crate with `features = ["cpi"]`, only the instructions in `CONTRACTS.md` section 5. Max depth: user -> program -> program.
Wrappers for outgoing CPI live in `src/cpi_out/` (not `src/cpi/`: `#[program]` generates a crate-level `cpi` module, and Anchor's IDL build does not follow `#[path]`).

Reading another program's account needs no CPI: deserialize with the owner check (`Account<'info, T>` from the other crate, or `AccountLoader` + `owner = other::ID`).

## Actor check (Session Keys seam)

Every game action in `session` calls one helper:

```rust
auth::check_actor(&ctx.accounts.signer, &ctx.accounts.participation, ctx.accounts.session_token.as_ref())?;
```

- Target: MagicBlock Session Keys token (`session-keys` crate), the token's authority must be the character owner and the target program must be `session`.
- Fallback: `signer == participation.actor || signer == character.owner`.
- A session key must never be able to move SOL or items out (`docs/GAME_COMMISSIONS.md`). Test it.

## Randomness (VRF seam)

- `act` / `monster_act` store a `PendingAction` in `GameSession`, set `pending_vrf = true` and CPI the MagicBlock VRF request with the callback = `consume_randomness`.
- `consume_randomness` checks the VRF identity signer, builds `VrfRoll(randomness)`, calls `deadroll_combat::resolve_hit`, applies results, clears `pending_vrf`, `action_counter += 1`.
- No other action is accepted while `pending_vrf` is true.
- Local tests: cargo feature `mock-vrf` in `programs/session` swaps the callback signer check from the VRF identity to a fixed test keypair. Everything else in `consume_randomness` stays the same code path. Never use `SlotHashesRoll` for tests. The devnet build must never enable `mock-vrf` (the deploy script fails if it is on).
- Production fallback `SlotHashesRoll` (only if the lead switches VRF to its fallback): same callback body called inline. Mark with `// FALLBACK(vrf)`.
- Check the installed `ephemeral-vrf-sdk` for exact names (request builder, identity constant, callback account metas). Do not guess.

## Rules

- Turn check on every game action: status `Active`, it is this slot's turn, alive, enough action points.
- Checked arithmetic for counters and HP (`checked_add`, `saturating_sub`).
- No floats. Combat math only through `deadroll_combat`.
- Never log or store private keys.

## Template

```rust
use anchor_lang::prelude::*;
use crate::{auth, errors::SessionError, events::*, state::*};

#[derive(Accounts)]
pub struct Move<'info> {
    #[account(mut, seeds = [b"session", session.map.as_ref()], bump = session.bump)]
    pub session: Account<'info, GameSession>,
    #[account(
        mut,
        seeds = [b"participation", session.key().as_ref(), participation.character.as_ref()],
        bump = participation.bump,
    )]
    pub participation: Account<'info, Participation>,
    /// CHECK: owner and layout checked in auth::check_actor
    pub session_token: Option<UncheckedAccount<'info>>,
    pub signer: Signer<'info>,
}

pub fn handler(ctx: Context<Move>, path: [u8; 8], len: u8) -> Result<()> {
    let s = &mut ctx.accounts.session;
    let p = &mut ctx.accounts.participation;
    auth::check_actor(&ctx.accounts.signer, p, ctx.accounts.session_token.as_ref())?;
    require!(s.is_turn_of(p.slot), SessionError::NotYourTurn);
    require!(!s.pending_vrf, SessionError::RollPending);
    // validate path against MapLayout (read, no CPI), spend action points
    emit!(Moved { session: s.key(), slot: p.slot, x: p.x, y: p.y });
    Ok(())
}
```

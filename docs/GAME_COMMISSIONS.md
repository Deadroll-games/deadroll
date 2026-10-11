# GAME_COMMISSIONS: sessions and fee prepayment in Deadroll

Version 0.1. How a player plays a party without confirming every roll in the wallet, who pays the fees and how, and what can and cannot be done without the main wallet.

---

## 1. Problem and solution

**Problem:** during a party every step, attack and dice roll is a transaction. If a Phantom window opens for each of them, the game is unplayable.

**Solution:** session keys.

- The player plays with their regular wallet (Phantom, etc.). Characters, items and SOL stay **only in it**.
- When joining a party, the browser creates a **temporary key**, and the player signs **once** a permission: "this key may make game moves on my behalf in this party", and transfers a small **fee prepayment** to the key.
- After that, all game actions are signed by the temporary key, with no windows. The player just clicks the dice.
- When the party ends: the permission is revoked, and the remainder of the prepayment is returned to the main wallet.

There is **no** in-game wallet that stores assets. The key in the browser never owns anything valuable.

---

## 2. Tooling

- The Session Keys program (originally by Gum, maintained by MagicBlock), address `KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5`.
- Rust: the `session-keys` crate (`SessionTokenV2`, `#[derive(Session)]`, the `session_auth_or` macro).
- Client: the TS package `@session-keys/*` for creating the token.
- For local tests the program must be cloned into `solana-test-validator` (`solana program dump` + `--bpf-program`).

Before implementation, check the current versions and API in the MagicBlock documentation.

---

## 3. Party flow

```
1. Joining the party
   a. The browser generates a temporary keypair (only in the tab's memory/local storage).
   b. The player signs ONE transaction with the MAIN wallet:
      - create a SessionToken: temporary key, Deadroll program, expiry;
      - transfer the prepayment to the temporary key (0.01 SOL by default);
      - write the token into the player's participation PDA for this party (section 5).
2. Play
   Every turn/roll/attack: the transaction is signed and paid for by the temporary key.
3. End of the party
   a. The DM closes the party → all participants' tokens stop passing validation.
   b. The client revokes the SessionToken (the rent deposit for the token account is returned)
      and transfers the remainder of the prepayment back to the main wallet.
```

If the player closed the tab and the temporary key is lost, only the remainder of the prepayment is lost (no more than the top-up amount). All assets stay on the main wallet, and the token will expire on its own.

---

## 4. What can be done through a session

Each program instruction explicitly decides whom to let in. Only game actions that do not move value out are allowed through a session.

| Via session key                    | Main wallet only                          |
| ---------------------------------- | ----------------------------------------- |
| step on the map                    | registration and character creation       |
| attack, using a skill              | crafting (burns resources)                |
| dice roll                          | listing and buying on the market          |
| picking up loot in the party       | transfer and sale of characters and items |
| equipping and swapping weapons in the party | withdrawing SOL                  |
| ending the turn                    | creating and closing the session          |

Even if the temporary key is stolen (for example, via XSS), the thief can only make a few moves with the character until the end of the party. They will not get a single item or lamport from the main wallet.

### Check in the instruction

```rust
#[derive(Accounts, Session)]
pub struct PlayerAction<'info> {
    pub character: Account<'info, Character>,
    pub game_session: Account<'info, GameSession>,
    pub participation: Account<'info, Participation>,

    #[session(signer = signer, authority = character.owner)]
    pub session_token: Option<Account<'info, SessionTokenV2>>,

    #[account(mut)]
    pub signer: Signer<'info>,
}

#[session_auth_or(
    ctx.accounts.character.owner == ctx.accounts.signer.key(),
    GameError::Unauthorized
)]
pub fn move_character(ctx: Context<PlayerAction>, dir: u8) -> Result<()> {
    // additional checks from section 5
    // ...
}
```

Instructions from the right column of the table do **not** contain `session_token` and check only the owner's signature.

---

## 5. Binding to the party

A standard token limits the program and the time. Deadroll adds a third condition: the token is valid **only in one party**.

The `Participation` PDA (player × party) stores:

| Field          | Purpose                         |
| -------------- | ------------------------------- |
| `player`       | main wallet                     |
| `character`    | the character in this party     |
| `session_key`  | public key of the temporary key |
| `game_session` | PDA of the party                |

Additional checks in every game instruction called through a session:

- `participation.session_key == signer`
- `participation.game_session == game_session`
- `game_session.status == Active`
- `participation.character == character`

The DM closed the party → `status != Active` → all participants' session keys are instantly useless, even if the token's expiry has not yet passed.

---

## 6. Fees

### 6.1 What a turn costs

- Base Solana fee: 5000 lamports per signature. A game transaction is signed by one temporary key → **0.000005 SOL per turn**.
- Priority fee: 0 or minimal in normal mode. When the network is congested, the client adds a small price per compute unit, with the limit set in the settings.
- Game instructions **do not create new accounts**, so that turns do not require rent deposits. All accounts of the party are created when it is opened (by the DM) and when a player joins (by the main wallet).

### 6.2 Prepayment

| Parameter                            | Default value                                                        |
| ------------------------------------ | -------------------------------------------------------------------- |
| Top-up when joining the party        | 0.01 SOL (~2000 turns without priority fee)                          |
| Low balance warning                  | below 0.002 SOL                                                      |
| Top-up during the party              | a "top up" button, signed by the main wallet, without leaving the party |
| Maximum balance of the temporary key | 0.05 SOL (the interface does not allow topping up more)              |
| Refund                               | automatically on leaving the party                                   |

The rent deposit for the `SessionToken` account (small, on the order of thousandths of a SOL) is paid by the main wallet and returned when the token is revoked.

### 6.3 Dice rolls

- Randomness comes from **MagicBlock Solana VRF**: one random value per action, and all rolls of the action are derived from it.
- The action has two steps (request and callback), but both parts are paid for without the player's involvement.
- **VRF has a request fee.** It is paid from the temporary key and must be accounted for in the size of the prepayment (section 6.2). Check the exact price in the MagicBlock documentation before calculating the default prepayment.

### 6.4 Party price

The party price is **separate money**, not the fee prepayment.

|                    | Fee prepayment                | Contribution to the party price                      |
| ------------------ | ----------------------------- | ---------------------------------------------------- |
| Where it is held   | on the player's session key   | in the party escrow (PDA)                            |
| What it is for     | the player's turn and VRF fees | payment to the DM for running the game              |
| Who sets it        | the player (0.01 SOL by default) | the DM sets the total price, players pay shares   |
| When it is returned | the remainder on leaving the party | only if the DM abandoned the party or the party did not fill |

Joining a party pays both amounts in one transaction of the main wallet. The party price rules are in `PARTY_AND_TURNS.md`, section 3.

---

## 7. DM

The DM uses the same scheme on their side:

- when opening a party, signs once with the main wallet the permission for their temporary key and the prepayment;
- after that, without windows, moves monsters, rolls their dice, applies their skills, opens tiles (revealing the map commitment);
- DM session rights: only monster actions and map reveals in **their own** party. Rewards, experience and closing the party are signed by the DM's main wallet.

---

## 8. UX

- Joining a party: one button "Start session", one wallet window.
- The interface always shows the prepayment balance and the time until the session ends.
- Leaving a party: one button, refund of the remainder and revocation of the token in one transaction.
- If the tab is closed during a party: on return, the client tries to restore the temporary key from the tab's storage; if there is no key, it offers to start a new session in the same party (the old key in `Participation` is replaced with the main wallet's signature).

---

## 9. Implementation checklist

- [ ] Add `session-keys`, check the current version and `SessionTokenV2`
- [ ] Split instructions into session-allowed and owner-only (section 4)
- [ ] The `Participation` PDA and the party binding checks (section 5)
- [ ] Game instructions do not create accounts
- [ ] Client: session creation + top-up in one transaction
- [ ] Client: leaving, token revocation and refund of the remainder in one transaction
- [ ] Low balance warning and top-up without leaving the party
- [ ] DM session with limited rights
- [ ] Test: a stolen temporary key cannot withdraw SOL, transfer an item, or list on the market
- [ ] Test: after the DM closes the party, the session key passes no instruction

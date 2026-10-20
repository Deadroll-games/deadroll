# STORAGE_AND_DELIVERY: storage and delivery of Deadroll art

Version 0.1. This document covers **where** the art data and the rendering function are stored, how clients load them, and how the image reaches wallets and marketplaces.

How the geometry, the rasterizer and the output PNG work is described in `ART_DESCRIPTION.md`.

---

## 1. Principle

- **Everything is stored on-chain:** sprite geometry, the palette, character and item state, **and the rendering function itself** (the WASM rasterizer).
- **There is no game data off-chain.** Off-chain there is only the **execution** of the function, for those who cannot run it themselves (wallets and marketplaces).
- Reason: on Solana, unlike `tokenURI()` in Ethereum, a program cannot compute an image at read time. A wallet sees only the `uri` string of the NFT and downloads a file from it.

---

## 2. On-chain accounts

| Account                                | What it stores                                                                                                                   | Approximate size     |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| `GameConfig`                           | rasterizer version, address of the account with the WASM, SHA-256 of the WASM, addresses of `Palette` and of 4 sprite book slots, base URL for `uri` | ~560 bytes           |
| `RasterizerCode` (one per version)     | the WASM binary of the rasterizer                                                                                                | 30-60 KB             |
| `Palette`                              | master palette (ramps) and the table of materials and aspects                                                                    | ~600-800 bytes       |
| `SpriteBook` (one or several)          | sprite index + geometry of all sprites                                                                                           | 20-80 bytes per sprite |
| Character PDA                          | sprite ids by layer, race ramps, injury flags, equipment, `state_version`                                                        | tens of bytes        |
| Item PDA                               | sprite ids (in hand, portrait, icon), materials, aspect, sharpening, `state_version`                                             | tens of bytes        |

Injury flags do not affect the image.

`state_version` increases on every change that affects the image, including `equip` and `unequip` (the image depends on what is worn). It is needed for immutable URLs (section 6).

---

## 3. The rasterizer in a Solana account

### 3.1 Cost

Rent-exempt deposit: `(size + 128 bytes) × 6960 lamports`. The deposit is returned when the account is closed.

| WASM size | Deposit   |
| --------- | --------- |
| 30 KB     | ~0.21 SOL |
| 50 KB     | ~0.36 SOL |
| 100 KB    | ~0.71 SOL |

Reference for a rasterizer with its own PNG encoder: 30-60 KB, that is **~0.2-0.4 SOL**.

### 3.2 Upload

- A transaction is limited to 1232 bytes, so the binary is written in chunks of ~900-1000 bytes. For 50 KB that is about 55 transactions, with fees totaling less than 0.001 SOL.
- Order: create an account of the required size → write the chunks → the `finalize` instruction, which computes the SHA-256 of the contents, compares it with the expected one, and writes the hash and the address to `GameConfig`.
- After `finalize` the account is immutable.
- The rasterizer `finalize` writes into `GameConfig`. The palette and the sprite books are set in `GameConfig` by separate instructions.

### 3.3 Versions

- A new rasterizer version = a new `RasterizerCode` account.
- `GameConfig` points to the current version. Old accounts can be kept, so that any historical image can be reproduced with the version it was drawn with, or closed to get the deposit back.

---

## 4. Storage budget

| What                             | Size            | Deposit               |
| -------------------------------- | --------------- | --------------------- |
| Rasterizer                       | 30-60 KB        | ~0.2-0.4 SOL          |
| Library of 200 sprites           | ~10 KB          | ~0.07 SOL             |
| Palette                          | ~0.6-0.8 KB     | ~0.005 SOL            |
| `GameConfig`                     | ~560 bytes      | ~0.005 SOL            |
| Character or item state          | tens of bytes   | included in their PDA |

Total art infrastructure for the whole game: **less than 0.5 SOL** in deposits.

---

## 5. Game website

The website runs the rasterizer directly in the player's browser.

1. Read `GameConfig`.
2. Load `RasterizerCode`. If the browser cache (IndexedDB) already holds a WASM with the same hash, take it from there.
3. Check `SHA-256(WASM) == hash from GameConfig`.
4. Load `Palette` and the needed sprites of `SpriteBook` (also cached by hash or version).
5. With a single `getMultipleAccounts`, read the character PDA and its item PDAs.
6. `render(..., RAW_RGBA)` → canvas.

The website talks to RPC like any dApp. The RPC provider key is public in the browser, so it is better to use a restricted key (for example, by domain, if the provider supports it).

---

## 6. NFTs, wallets and marketplaces

### 6.1 How wallets display NFTs

Wallets and marketplaces do not read program accounts and do not run WASM. They take the `uri` field of the Metaplex Core NFT, download the JSON from it, and show the picture from the `image` field.

### 6.2 Immutable versioned URLs

- The **update authority** of character and item NFTs belongs to the **program PDA**.
- On every change that affects the image, the program in the same transaction increases `state_version` and updates `uri` via CPI:

```
https://<render-domain>/char/<address>/v42.json
```

- Such a URL **never changes its content**. A new state = a new URL.
- The one who learns about the change is not the worker but the wallet or marketplace: they read the NFT metadata themselves with their own indexers and see the new `uri`.
- The base domain is stored in `GameConfig`. If the hosting changes, the program updates the base, and the `uri` values are rewritten at the next change or by a separate migration instruction.

---

## 7. Worker for wallets

### 7.1 What it is

A small serverless function (for example, a Cloudflare Worker), ~100 lines of TypeScript, deployed with `wrangler deploy`. It **stores nothing:** no database, no files. Both the rendering code and the data come from the chain.

### 7.2 Endpoints

```
GET /char/<address>/v<N>.json   → metadata: name, description, attributes, image = .../v<N>.png
GET /char/<address>/v<N>.png    → image
GET /item/<address>/v<N>.json
GET /item/<address>/v<N>.png
```

### 7.3 Request handling

```
1. Check the URL format (address is base58 of the correct length, N is a number).
   Invalid format → 400 without a single RPC call.
2. In the CDN cache → serve from the cache (the worker does not even start).
3. Otherwise:
   a. getMultipleAccounts: the asset PDA + its items (one RPC call)
   b. is state_version on-chain == N ?
        no, the chain has a higher one → redirect to the current version, cache for 1 minute
        no, no such asset             → 404, cache the refusal for 10 minutes
   c. WASM, palette, sprites: from memory/cache by hash,
      on a miss load from the chain and verify the hash
   d. render(..., PNG)
   e. respond with Cache-Control: public, max-age=31536000, immutable
```

### 7.4 RPC usage

- RPC is called only on a **cache miss**, that is, roughly once per each new state version.
- Views create no load: repeat requests are served by the CDN, and marketplaces and wallets also cache images on their side.
- Usage is counted in **state changes**, not in views. A hundred thousand changes per month = on the order of a hundred thousand simple RPC calls.

### 7.5 Protection

- URL format check before any calls.
- Caching of refusals (404 and redirects).
- Rate limit per IP using Cloudflare.
- Without this, an attacker could hammer random addresses and versions so that every request is a cache miss and spends RPC.

### 7.6 Limits (at the time of writing)

- Cloudflare Workers Free: 100,000 requests per day, 10 ms CPU per request. Network waiting (RPC) does not count toward CPU. Render + PNG 512×512 needs to be measured, it may be tight.
- Cloudflare Workers Paid: $5 per month, default CPU limit 30 seconds.
- RPC: a provider with a free tier (Helius, Triton and similar). The public Solana RPC is not suitable because of its limits.

### 7.7 Replaceability

- The worker code is open. It checks the rasterizer hash from `GameConfig`, so the result matches the website 1:1.
- If the worker disappears, the data and the function remain on-chain. Anyone can bring up an identical worker, and the program switches the base domain in `GameConfig`.

---

## 8. Alternatives and phases

| Phase         | Option                                                                                                         | Off-chain                | Where the live image is visible                        |
| ------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------ |
| Hackathon MVP | game website + a static cover in `image`                                                                       | nothing                  | only on the website                                    |
| Option        | `animation_url` → a static HTML page that, in the viewer's browser, reads the chain and runs the WASM          | a static HTML file       | on the NFT page, where the marketplace supports HTML   |
| After MVP     | worker with versioned URLs                                                                                     | execution of the function | everywhere, including previews in wallets             |

Notes on `animation_url`:

- HTML support in wallets and marketplaces is uneven, and an iframe may not allow network requests. Check manually on Phantom, Magic Eden, Tensor.
- Every view = an RPC request from the viewer's browser with a public key, and caching this is harder than in the worker.
- Previews in lists are still taken from `image`.

For tombstones (final state, no longer changes) it is optionally possible to save the PNG to Arweave once and rewrite `uri` to point to it. Then the tombstone is shown even without the worker.

// Prepares local `anchor test` runs until registry `init_config` exists (L4.1):
// 1. creates a throwaway test wallet at .anchor/test-wallet.json (git-ignored) if missing;
// 2. writes .anchor/fixtures/game_config.json, a registry-owned GameConfig whose authority is
//    that wallet. Anchor.toml loads it into the local validator ([[test.validator.account]]).
// Never prints key material.
// Usage (repo root): node tests/fixtures/make-config.mjs
import { generateKeyPairSync } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";

const root = new URL("../../", import.meta.url);
const walletPath = new URL(".anchor/test-wallet.json", root);
const fixturePath = new URL(".anchor/fixtures/game_config.json", root);

const REGISTRY_ID = "dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG";
// PDA ["config"] of the registry program.
const CONFIG_PDA = "HYGYnuyssMfrHr2gT3AC3hkQpD6xt37a5hrCc2Z97JHN";

mkdirSync(new URL(".anchor/fixtures/", root), { recursive: true });
if (!existsSync(walletPath)) {
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const seed = Buffer.from(privateKey.export({ format: "jwk" }).d, "base64url");
  const pub = Buffer.from(publicKey.export({ format: "jwk" }).x, "base64url");
  writeFileSync(walletPath, JSON.stringify([...seed, ...pub]));
}
const wallet = JSON.parse(readFileSync(walletPath, "utf8"));
const authority = Buffer.from(wallet.slice(32, 64));

// discriminator (8, unchecked by render) + GameConfigData (agents/CONTRACTS.md 4):
// version u8, authority, programs [Pubkey; 7], rasterizer_version u16, rasterizer, rasterizer_sha256,
// palette, uri_base [u8; 64], bump u8.
const data = Buffer.concat([
  Buffer.alloc(8),
  Buffer.from([1]),
  authority,
  Buffer.alloc(7 * 32 + 2 + 32 + 32 + 32 + 64),
  Buffer.from([255]),
]);
writeFileSync(
  fixturePath,
  JSON.stringify({
    pubkey: CONFIG_PDA,
    account: {
      lamports: 10_000_000,
      data: [data.toString("base64"), "base64"],
      owner: REGISTRY_ID,
      executable: false,
      rentEpoch: 0,
      space: data.length,
    },
  }),
);
console.log(`fixture ${fixturePath.pathname} (${data.length} bytes), wallet .anchor/test-wallet.json`);

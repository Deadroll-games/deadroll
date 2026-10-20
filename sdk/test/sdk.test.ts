import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { address, getBase64Decoder } from "@solana/kit";
import { PROGRAM_IDLS } from "../scripts/generate.ts";
import { decodeEvents, errorName, eventsFromLogs, PROGRAM_CRATES, PROGRAMS, programNameOf, type ProgramName } from "../src/index.ts";
import { getPlayerDiedEventEncoder } from "../src/session.ts";

const root = new URL("../../", import.meta.url);
const idl = (p: ProgramName) => JSON.parse(readFileSync(new URL(`sdk/idl/${PROGRAM_IDLS[p]}`, root), "utf8"));

test("PROGRAMS equal Anchor.toml on localnet and devnet", () => {
  const toml = readFileSync(new URL("Anchor.toml", root), "utf8");
  for (const cluster of ["localnet", "devnet"]) {
    const block = new RegExp(`\\[programs\\.${cluster}\\]([^\\[]*)`).exec(toml)![1]!;
    for (const [name, crate] of Object.entries(PROGRAM_CRATES)) {
      const id = new RegExp(`^${crate}\\s*=\\s*"(\\w+)"`, "m").exec(block)?.[1];
      assert.equal(id, PROGRAMS[name as ProgramName], `${cluster} ${name}`);
    }
  }
});

test("PROGRAMS equal the IDL addresses", () => {
  for (const name of Object.keys(PROGRAMS) as ProgramName[]) assert.equal(PROGRAMS[name], idl(name).address);
  assert.equal(programNameOf(PROGRAMS.session), "session");
  assert.equal(programNameOf(address("11111111111111111111111111111111")), null);
});

test("errorName follows the IDL errors", () => {
  for (const name of Object.keys(PROGRAMS) as ProgramName[]) {
    for (const e of idl(name).errors ?? []) assert.equal(errorName(name, e.code), e.name);
  }
  assert.equal(errorName("session", 1), null);
});

test("decodeEvents reads events inside the emitting program only", () => {
  const ev = { session: PROGRAMS.map, character: PROGRAMS.character, slot: 2, by: 7 };
  const b64 = getBase64Decoder().decode(getPlayerDiedEventEncoder().encode(ev));
  const logs = [
    `Program ${PROGRAMS.session} invoke [1]`,
    `Program data: ${b64}`,
    `Program ${PROGRAMS.character} invoke [2]`,
    `Program data: ${b64}`,
    `Program ${PROGRAMS.character} success`,
    "Program data: not-base64!",
    `Program ${PROGRAMS.session} success`,
  ];
  const evs = decodeEvents(logs);
  assert.equal(evs.length, 1);
  assert.deepEqual(evs[0], { program: "session", name: "PlayerDied", data: ev });
  assert.equal(eventsFromLogs(logs, PROGRAMS.character).length, 1);
});

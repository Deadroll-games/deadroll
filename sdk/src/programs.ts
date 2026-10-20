// Program addresses: the one place the website, tests/ and tools/scripts read them from.
// Deadroll ids come from the generated clients (the IDL `address`, agents/CONTRACTS.md 1).
import { address } from "@solana/kit";
import { DEADROLL_CHARACTER_PROGRAM_ADDRESS } from "./generated/character/index.js";
import { DEADROLL_CITY_PROGRAM_ADDRESS } from "./generated/city/index.js";
import { DEADROLL_ITEMS_PROGRAM_ADDRESS } from "./generated/items/index.js";
import { DEADROLL_MAP_PROGRAM_ADDRESS } from "./generated/map/index.js";
import { DEADROLL_REGISTRY_PROGRAM_ADDRESS } from "./generated/registry/index.js";
import { DEADROLL_RENDER_PROGRAM_PROGRAM_ADDRESS } from "./generated/render/index.js";
import { DEADROLL_SESSION_PROGRAM_ADDRESS } from "./generated/session/index.js";

/** Deadroll programs in `ProgramIdx` order. Same ids on localnet and devnet. */
export const PROGRAMS = {
  registry: DEADROLL_REGISTRY_PROGRAM_ADDRESS,
  render: DEADROLL_RENDER_PROGRAM_PROGRAM_ADDRESS,
  character: DEADROLL_CHARACTER_PROGRAM_ADDRESS,
  items: DEADROLL_ITEMS_PROGRAM_ADDRESS,
  map: DEADROLL_MAP_PROGRAM_ADDRESS,
  session: DEADROLL_SESSION_PROGRAM_ADDRESS,
  city: DEADROLL_CITY_PROGRAM_ADDRESS,
} as const;

export type ProgramName = keyof typeof PROGRAMS;

/** Anchor crate name of each program (target/deploy/<crate>.so, Anchor.toml keys). */
export const PROGRAM_CRATES: Record<ProgramName, string> = {
  registry: "deadroll_registry",
  render: "deadroll_render_program",
  character: "deadroll_character",
  items: "deadroll_items",
  map: "deadroll_map",
  session: "deadroll_session",
  city: "deadroll_city",
};

export const MPL_CORE_PROGRAM = address("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
export const SESSION_KEYS_PROGRAM = address("KeyspM2ssCJbqUhQ4k7sveSiY4WjnYsrXkC8oDbwde5");
export const SYSTEM_PROGRAM = address("11111111111111111111111111111111");
export const COMPUTE_BUDGET_PROGRAM = address("ComputeBudget111111111111111111111111111111");

/** MagicBlock VRF accounts (ephemeral-vrf-sdk consts); under mock-vrf they only pass address checks. */
export const VRF = {
  program: address("Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz"),
  oracleQueue: address("Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh"),
  slotHashes: address("SysvarS1otHashes111111111111111111111111111"),
} as const;

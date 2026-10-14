// Runtime config from Vite env (app/.env.local). Program ids: agents/CONTRACTS.md, section 1.
// `vite --mode localnet` (pnpm dev:localnet) defaults everything to the shared local validator.
import { address } from "@solana/kit";
import { loadCustomRpc, PUBLIC_DEVNET_RPC, PUBLIC_DEVNET_WS } from "./rpcSettings";

export type DataSource = "mock" | "chain";

const env = import.meta.env;
const LOCALNET_MODE = env.MODE === "localnet";
/** `vite build --mode devnet`: the hosted site. Chain data and real wallets only. */
export const DEVNET_BUILD = import.meta.env.MODE === "devnet";
/** Mock data, `/dev/*` demo pages, the `?dev=1` wallet and the mock VRF oracle. Off in the devnet build. */
export const DEV_TOOLS = !DEVNET_BUILD;

// A hosted (production) build reads the chain over the public devnet RPC. No provider key is ever bundled:
// vite.config.ts refuses to build with one. Dev runs may use VITE_RPC_URL from an untracked .env.*.local.
const HOSTED = env.PROD && !env.VITE_RPC_URL;
const custom = loadCustomRpc();
export const DATA_SOURCE: DataSource =
  DEVNET_BUILD || (env.VITE_DATA ?? (LOCALNET_MODE || env.PROD ? "chain" : "mock")) === "chain" ? "chain" : "mock";
/** Settings > Custom RPC (localStorage) wins over the build/env value. */
export const CUSTOM_RPC = custom;
export const RPC_URL: string = custom?.url ?? env.VITE_RPC_URL ?? (HOSTED ? PUBLIC_DEVNET_RPC : "http://127.0.0.1:8899");
export const RPC_WS_URL: string | undefined = custom ? custom.ws : (env.VITE_RPC_WS_URL ?? (HOSTED ? PUBLIC_DEVNET_WS : undefined));
/** Wallet Standard chain id. */
export const CHAIN = (env.VITE_CHAIN ?? (LOCALNET_MODE ? "solana:localnet" : "solana:devnet")) as `${string}:${string}`;
/** Sprite book ids to load, comma separated (FORMAT.md 3.3). */
export const SPRITE_BOOK_IDS: number[] = (env.VITE_SPRITE_BOOKS ?? "1")
  .split(",")
  .map((s: string) => Number(s.trim()))
  .filter((n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff);
/** Palette version the characters are drawn with (`palette 1` in assets/palette). */
export const PALETTE_VERSION = Number(env.VITE_PALETTE_VERSION ?? "1");

/** The RPC is a validator on this machine (airdrops, the mock VRF oracle). */
export const IS_LOCALNET = /^https?:\/\/(127\.0\.0\.1|localhost)(:\d+)?\/?$/.test(RPC_URL);

/**
 * Localnet only: the app plays the VRF oracle for a session program built with `mock-vrf`
 * (agents/TASKS.md decision block, rule 2). Never on for a non-local RPC.
 */
export const MOCK_VRF: boolean = DEV_TOOLS && IS_LOCALNET &&(env.VITE_MOCK_VRF ?? (LOCALNET_MODE ? "1" : "0")) === "1";

export const PROGRAMS = {
  registry: address("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG"),
  render: address("DgQCsXAe7Q7h8zdtToiTvk8XqNjjbXuD6xith2BmEiRB"),
  character: address("9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL"),
  items: address("48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS"),
  map: address("2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH"),
  session: address("3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ"),
  city: address("2Cr5vB5awqDVx6XRcxtFFYQEG6Zy3aSAcKcqzh1xbVjS"),
} as const;

export const MPL_CORE_PROGRAM = address("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
export const SYSTEM_PROGRAM = address("11111111111111111111111111111111");

/** MagicBlock VRF accounts (ephemeral-vrf-sdk consts); under mock-vrf they only pass address checks. */
export const VRF = {
  program: address("Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz"),
  oracleQueue: address("Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh"),
  slotHashes: address("SysvarS1otHashes111111111111111111111111111"),
} as const;

/** Explorer link for a transaction or account on the configured cluster. */
export function explorerUrl(kind: "tx" | "address", id: string): string {
  const cluster = IS_LOCALNET
    ? `cluster=custom&customUrl=${encodeURIComponent(RPC_URL)}`
    : RPC_URL.includes("devnet")
      ? "cluster=devnet"
      : `cluster=custom&customUrl=${encodeURIComponent(RPC_URL)}`;
  return `https://explorer.solana.com/${kind}/${id}?${cluster}`;
}

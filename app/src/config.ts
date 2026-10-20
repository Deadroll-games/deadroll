// Runtime config from Vite env (app/.env.local). Program ids: @deadroll/sdk (agents/CONTRACTS.md, section 1).
// `vite --mode localnet` (pnpm dev:localnet) defaults everything to the shared local validator.
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

/** The RPC is a validator on this machine (airdrops, the mock VRF oracle). */
export const IS_LOCALNET = /^https?:\/\/(127\.0\.0\.1|localhost)(:\d+)?\/?$/.test(RPC_URL);

/**
 * Localnet only: the app plays the VRF oracle for a session program built with `mock-vrf`
 * (agents/TASKS.md decision block, rule 2). Never on for a non-local RPC.
 */
export const MOCK_VRF: boolean = DEV_TOOLS && IS_LOCALNET &&(env.VITE_MOCK_VRF ?? (LOCALNET_MODE ? "1" : "0")) === "1";

// Program addresses live in the sdk (generated from the IDLs).
export { MPL_CORE_PROGRAM, PROGRAMS, SYSTEM_PROGRAM, VRF } from "@deadroll/sdk";

/** Explorer link for a transaction or account on the configured cluster. */
export function explorerUrl(kind: "tx" | "address", id: string): string {
  const cluster = IS_LOCALNET
    ? `cluster=custom&customUrl=${encodeURIComponent(RPC_URL)}`
    : RPC_URL.includes("devnet")
      ? "cluster=devnet"
      : `cluster=custom&customUrl=${encodeURIComponent(RPC_URL)}`;
  return `https://explorer.solana.com/${kind}/${id}?${cluster}`;
}

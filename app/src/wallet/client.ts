// One kit client for the app: Wallet Standard signer + RPC (solana-dev skill, kit/react.md).
import { createClient } from "@solana/kit";
import { solanaRpc } from "@solana/kit-plugin-rpc";
import { walletSigner } from "@solana/kit-plugin-wallet";
import { CHAIN, RPC_URL, RPC_WS_URL } from "../config";

export const client = createClient()
  .use(walletSigner({ chain: CHAIN, autoConnect: true }))
  .use(solanaRpc({ rpcUrl: RPC_URL, ...(RPC_WS_URL ? { rpcSubscriptionsUrl: RPC_WS_URL } : {}) }));

export type AppClient = typeof client;

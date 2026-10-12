// Where account bytes come from. The chain source reads RPC; the mock source serves synthetic
// accounts so every decoder and the hash check run the same code in mock mode.
import type { Address, Base64EncodedDataResponse, Rpc, GetAccountInfoApi } from "@solana/kit";
import { fromBase64 } from "./bytes";

export type AccountSource = {
  /** Account data, or null if the account does not exist. */
  get(address: Address): Promise<Uint8Array | null>;
  readonly label: "chain" | "mock";
};

export function rpcAccountSource(rpc: Rpc<GetAccountInfoApi>): AccountSource {
  return {
    label: "chain",
    async get(address) {
      const { value } = await rpc.getAccountInfo(address, { encoding: "base64", commitment: "confirmed" }).send();
      if (!value) return null;
      const [b64] = value.data as Base64EncodedDataResponse;
      return fromBase64(b64);
    },
  };
}

export function mapAccountSource(accounts: Map<string, Uint8Array>): AccountSource {
  return {
    label: "mock",
    async get(address) {
      return accounts.get(address) ?? null;
    },
  };
}

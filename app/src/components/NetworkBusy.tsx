import { useNetworkBusy } from "../chain/rpcRetry";

/** Small note shown while the RPC node rate-limits us and a request is waiting to be retried. */
export function NetworkBusy() {
  return useNetworkBusy() ? (
    <span className="badge" role="status" title="The RPC node is rate-limiting; requests are retried automatically.">
      network busy, retrying
    </span>
  ) : null;
}

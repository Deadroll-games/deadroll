// Fired on window after this tab confirmed a transaction (detail: the fee payer address). The wallet badge
// re-reads its balance when the payer is the wallet, so it needs no fast poll.
export const TX_SENT_EVENT = "deadroll:tx-sent";

/** One turn on the arena's turn bar. */
export type TurnBarItem = {
  /** Turn-order id (players `0..4`, then monsters). */
  id: number;
  kind: "player" | "monster";
  label: string;
  /** The unit acting now (the first entry). */
  now: boolean;
  /** One of this tab's units (its player, or a monster for the DM). */
  mine?: boolean;
};

export type TurnBarProps = {
  turns: readonly TurnBarItem[];
  title?: string;
};

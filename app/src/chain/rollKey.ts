/**
 * The key of an event's roll: its transaction signature and its place among that transaction's
 * events. It does not depend on the position in the list, which shifts when the list is trimmed or
 * replayed, so the same hit keeps its key and a new hit always gets a new one.
 */
export function rollKeyOf(events: { signature: string }[], index: number): string {
  const sig = events[index]?.signature ?? "";
  let ordinal = 0;
  for (let j = 0; j < index; j++) if (events[j]?.signature === sig) ordinal++;
  return `${sig}:${ordinal}`;
}

import { describe, expect, it } from "vitest";
import { rollKeyOf } from "./rollKey";

const ev = (signature: string) => ({ signature });

describe("rollKeyOf", () => {
  it("keeps the key of a hit when other events are appended after it", () => {
    const before = [ev("a"), ev("hit"), ev("hit")];
    const after = [...before, ev("move"), ev("turn")];
    expect(rollKeyOf(after, 2)).toBe(rollKeyOf(before, 2));
  });

  it("keeps the key when the list is trimmed from the front", () => {
    const full = [ev("a"), ev("b"), ev("hit"), ev("hit")];
    const trimmed = full.slice(2);
    expect(rollKeyOf(trimmed, 1)).toBe(rollKeyOf(full, 3));
  });

  it("gives another event of the same transaction, and another transaction, another key", () => {
    const list = [ev("hit"), ev("hit"), ev("next")];
    expect(new Set([0, 1, 2].map((i) => rollKeyOf(list, i))).size).toBe(3);
  });
});

import { describe, expect, it } from "vitest";
import { nameValid } from "./format";

describe("nameValid (same rule as the character program)", () => {
  it("accepts 1..=32 printable ASCII without a leading space", () => {
    expect(nameValid("Bran")).toBe(true);
    expect(nameValid("A b-c_9!")).toBe(true);
    expect(nameValid("x".repeat(32))).toBe(true);
  });
  it("rejects empty, too long, leading space and non-ASCII", () => {
    expect(nameValid("")).toBe(false);
    expect(nameValid("x".repeat(33))).toBe(false);
    expect(nameValid(" Bran")).toBe(false);
    expect(nameValid("Bränn")).toBe(false);
    expect(nameValid("tab\there")).toBe(false);
  });
});

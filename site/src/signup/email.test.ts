import { describe, expect, it } from "vitest";
import { normalizeEmail } from "./email";

describe("normalizeEmail", () => {
  it("trims and lowercases valid email addresses", () => {
    expect(normalizeEmail("  Gleb@Example.COM ")).toBe("gleb@example.com");
  });

  it("rejects malformed addresses", () => {
    expect(normalizeEmail("gleb")).toBeNull();
    expect(normalizeEmail("gleb@example")).toBeNull();
    expect(normalizeEmail("gleb example.com")).toBeNull();
  });

  it("rejects overlong addresses", () => {
    expect(normalizeEmail(`${"a".repeat(250)}@x.com`)).toBeNull();
  });
});

import { describe, expect, it } from "vitest";
import { generateToken, hashEmail, hashIp, hashToken } from "./crypto";

const secret = "0123456789abcdef0123456789abcdef";

describe("signup crypto helpers", () => {
  it("generates URL-safe high-entropy tokens", () => {
    const token = generateToken();

    expect(token).toMatch(/^[A-Za-z0-9_-]+$/);
    expect(token.length).toBeGreaterThanOrEqual(40);
    expect(generateToken()).not.toBe(token);
  });

  it("hashes email, token, and IP with separate namespaces", () => {
    expect(hashEmail("a@example.com", secret)).toHaveLength(64);
    expect(hashToken("a@example.com", secret)).toHaveLength(64);
    expect(hashIp("a@example.com", secret)).toHaveLength(64);
    expect(hashEmail("a@example.com", secret)).not.toBe(hashToken("a@example.com", secret));
  });

  it("rejects weak secrets", () => {
    expect(() => hashEmail("a@example.com", "short")).toThrow("SIGNUP_SECRET");
  });
});

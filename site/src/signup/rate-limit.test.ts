import { describe, expect, it } from "vitest";
import { consumeRateLimit, rateLimitKeys, sevenDayExpiry } from "./rate-limit";
import { MemorySignupStore } from "./store";

describe("consumeRateLimit", () => {
  it("allows requests under the limit", async () => {
    const store = new MemorySignupStore();
    const now = new Date("2026-06-16T00:00:00.000Z");

    await expect(consumeRateLimit(store, "email/hash", 2, 1000, now)).resolves.toEqual({
      allowed: true,
    });
    await expect(consumeRateLimit(store, "email/hash", 2, 1000, now)).resolves.toEqual({
      allowed: true,
    });
  });

  it("blocks requests over the limit until reset", async () => {
    const store = new MemorySignupStore();
    const now = new Date("2026-06-16T00:00:00.000Z");

    await consumeRateLimit(store, "email/hash", 1, 1000, now);

    await expect(consumeRateLimit(store, "email/hash", 1, 1000, now)).resolves.toEqual({
      allowed: false,
      retryAfterSeconds: 1,
    });
    await expect(
      consumeRateLimit(store, "email/hash", 1, 1000, new Date("2026-06-16T00:00:01.001Z")),
    ).resolves.toEqual({ allowed: true });
  });
});

describe("rate limit helpers", () => {
  it("builds scoped keys", () => {
    expect(rateLimitKeys("email-hash", "ip-hash")).toEqual(["email/email-hash", "ip/ip-hash"]);
    expect(rateLimitKeys("email-hash")).toEqual(["email/email-hash"]);
  });

  it("uses seven-day signup expiry", () => {
    expect(sevenDayExpiry(new Date("2026-06-16T00:00:00.000Z"))).toBe("2026-06-23T00:00:00.000Z");
  });
});

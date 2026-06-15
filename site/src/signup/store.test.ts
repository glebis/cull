import { describe, expect, it } from "vitest";
import { MemorySignupStore } from "./store";
import type { ConfirmedSignup, PendingSignup } from "./types";

const pending: PendingSignup = {
  email: "gleb@example.com",
  tokenHash: "token-hash",
  emailHash: "email-hash",
  createdAt: "2026-06-16T00:00:00.000Z",
  expiresAt: "2026-06-23T00:00:00.000Z",
  source: "landing",
};

describe("MemorySignupStore", () => {
  it("stores pending signups by token and email hash", async () => {
    const store = new MemorySignupStore();

    await store.savePending(pending);

    await expect(store.getPending("token-hash")).resolves.toEqual(pending);
    await expect(store.getPendingByEmailHash("email-hash")).resolves.toEqual(pending);
  });

  it("deletes pending signups from both indexes", async () => {
    const store = new MemorySignupStore();
    await store.savePending(pending);

    await store.deletePending(pending);

    await expect(store.getPending("token-hash")).resolves.toBeNull();
    await expect(store.getPendingByEmailHash("email-hash")).resolves.toBeNull();
  });

  it("stores confirmed signups", async () => {
    const store = new MemorySignupStore();
    const confirmed: ConfirmedSignup = {
      email: "gleb@example.com",
      emailHash: "email-hash",
      confirmedAt: "2026-06-16T00:00:00.000Z",
      source: "landing",
    };

    await store.saveConfirmed(confirmed);

    await expect(store.getConfirmed("email-hash")).resolves.toEqual(confirmed);
  });
});

import { describe, expect, it } from "vitest";
import { hashEmail, hashToken } from "./crypto";
import { confirm, subscribe, type SignupConfig } from "./service";
import { MemorySignupStore } from "./store";
import type { ConfirmationEmail, EmailSender } from "./resend";

const secret = "0123456789abcdef0123456789abcdef";
const config: SignupConfig = {
  secret,
  siteUrl: "https://cull.company",
};

class MemorySender implements EmailSender {
  sent: ConfirmationEmail[] = [];
  fail = false;

  async sendConfirmation(email: ConfirmationEmail): Promise<void> {
    if (this.fail) {
      throw new Error("send failed");
    }
    this.sent.push(email);
  }
}

describe("subscribe", () => {
  it("creates a pending signup and sends a confirmation email", async () => {
    const store = new MemorySignupStore();
    const sender = new MemorySender();

    const result = await subscribe(
      { email: " GLEB@example.com ", ip: "127.0.0.1", now: new Date("2026-06-16T00:00:00.000Z") },
      store,
      sender,
      config,
    );

    expect(result).toEqual({ status: "pending", email: "gleb@example.com" });
    expect(sender.sent).toHaveLength(1);
    expect(sender.sent[0]?.confirmUrl).toMatch(/^https:\/\/cull\.company\/api\/confirm\?token=/);
    await expect(store.getPendingByEmailHash(hashEmail("gleb@example.com", secret))).resolves.toMatchObject({
      email: "gleb@example.com",
      expiresAt: "2026-06-23T00:00:00.000Z",
    });
  });

  it("rejects invalid email", async () => {
    await expect(subscribe({ email: "gleb" }, new MemorySignupStore(), new MemorySender(), config)).resolves.toEqual({
      status: "invalid_email",
    });
  });

  it("returns already confirmed without sending another email", async () => {
    const store = new MemorySignupStore();
    const sender = new MemorySender();
    await store.saveConfirmed({
      email: "gleb@example.com",
      emailHash: hashEmail("gleb@example.com", secret),
      confirmedAt: "2026-06-16T00:00:00.000Z",
      source: "landing",
    });

    await expect(subscribe({ email: "gleb@example.com" }, store, sender, config)).resolves.toEqual({
      status: "already_confirmed",
      email: "gleb@example.com",
    });
    expect(sender.sent).toHaveLength(0);
  });

  it("refreshes a pending signup for the same email", async () => {
    const store = new MemorySignupStore();
    const sender = new MemorySender();

    await subscribe({ email: "gleb@example.com", now: new Date("2026-06-16T00:00:00.000Z") }, store, sender, config);
    const first = await store.getPendingByEmailHash(hashEmail("gleb@example.com", secret));
    await subscribe({ email: "gleb@example.com", now: new Date("2026-06-16T00:01:00.000Z") }, store, sender, config);
    const second = await store.getPendingByEmailHash(hashEmail("gleb@example.com", secret));

    expect(first).not.toBeNull();
    expect(second).not.toBeNull();
    expect(first!.tokenHash).not.toBe(second!.tokenHash);
    await expect(store.getPending(first!.tokenHash)).resolves.toBeNull();
    expect(store.pending.size).toBe(1);
    expect(sender.sent).toHaveLength(2);
  });

  it("rate limits repeated requests", async () => {
    const store = new MemorySignupStore();
    const sender = new MemorySender();
    const now = new Date("2026-06-16T00:00:00.000Z");

    await subscribe({ email: "gleb@example.com", now }, store, sender, config);
    await subscribe({ email: "gleb@example.com", now }, store, sender, config);
    await subscribe({ email: "gleb@example.com", now }, store, sender, config);

    await expect(subscribe({ email: "gleb@example.com", now }, store, sender, config)).resolves.toMatchObject({
      status: "rate_limited",
    });
  });

  it("returns temporary failure if Resend fails", async () => {
    const sender = new MemorySender();
    sender.fail = true;

    await expect(subscribe({ email: "gleb@example.com" }, new MemorySignupStore(), sender, config)).resolves.toEqual({
      status: "temporary_failure",
    });
  });
});

describe("confirm", () => {
  it("confirms a pending signup", async () => {
    const store = new MemorySignupStore();
    const token = "token";
    await store.savePending({
      email: "gleb@example.com",
      tokenHash: hashToken(token, secret),
      emailHash: hashEmail("gleb@example.com", secret),
      createdAt: "2026-06-16T00:00:00.000Z",
      expiresAt: "2026-06-23T00:00:00.000Z",
      source: "landing",
    });

    await expect(confirm(token, store, config, new Date("2026-06-16T00:00:00.000Z"))).resolves.toEqual({
      status: "confirmed",
      email: "gleb@example.com",
    });
    await expect(store.getConfirmed(hashEmail("gleb@example.com", secret))).resolves.toMatchObject({
      email: "gleb@example.com",
    });
    await expect(store.getPending(hashToken(token, secret))).resolves.toBeNull();
  });

  it("rejects missing or unknown tokens", async () => {
    const store = new MemorySignupStore();

    await expect(confirm(undefined, store, config)).resolves.toEqual({ status: "invalid_token" });
    await expect(confirm("unknown", store, config)).resolves.toEqual({ status: "invalid_token" });
  });

  it("expires old pending signups", async () => {
    const store = new MemorySignupStore();
    const token = "token";
    await store.savePending({
      email: "gleb@example.com",
      tokenHash: hashToken(token, secret),
      emailHash: hashEmail("gleb@example.com", secret),
      createdAt: "2026-06-16T00:00:00.000Z",
      expiresAt: "2026-06-17T00:00:00.000Z",
      source: "landing",
    });

    await expect(confirm(token, store, config, new Date("2026-06-18T00:00:00.000Z"))).resolves.toEqual({
      status: "expired_token",
      email: "gleb@example.com",
    });
  });
});

import { del, get, put } from "@vercel/blob";
import type { ConfirmedSignup, PendingSignup, RateLimitRecord } from "./types.js";

const JSON_OPTIONS = {
  access: "private" as const,
  contentType: "application/json",
  allowOverwrite: true,
};

export interface SignupStore {
  getPending(tokenHash: string): Promise<PendingSignup | null>;
  getPendingByEmailHash(emailHash: string): Promise<PendingSignup | null>;
  savePending(record: PendingSignup): Promise<void>;
  deletePending(record: PendingSignup): Promise<void>;
  getConfirmed(emailHash: string): Promise<ConfirmedSignup | null>;
  saveConfirmed(record: ConfirmedSignup): Promise<void>;
  getRateLimit(key: string): Promise<RateLimitRecord | null>;
  saveRateLimit(key: string, record: RateLimitRecord): Promise<void>;
}

export class MemorySignupStore implements SignupStore {
  pending = new Map<string, PendingSignup>();
  pendingByEmail = new Map<string, string>();
  confirmed = new Map<string, ConfirmedSignup>();
  rateLimits = new Map<string, RateLimitRecord>();

  async getPending(tokenHash: string): Promise<PendingSignup | null> {
    return this.pending.get(tokenHash) ?? null;
  }

  async getPendingByEmailHash(emailHash: string): Promise<PendingSignup | null> {
    const tokenHash = this.pendingByEmail.get(emailHash);
    return tokenHash ? (this.pending.get(tokenHash) ?? null) : null;
  }

  async savePending(record: PendingSignup): Promise<void> {
    const existingTokenHash = this.pendingByEmail.get(record.emailHash);
    if (existingTokenHash && existingTokenHash !== record.tokenHash) {
      this.pending.delete(existingTokenHash);
    }
    this.pending.set(record.tokenHash, record);
    this.pendingByEmail.set(record.emailHash, record.tokenHash);
  }

  async deletePending(record: PendingSignup): Promise<void> {
    this.pending.delete(record.tokenHash);
    this.pendingByEmail.delete(record.emailHash);
  }

  async getConfirmed(emailHash: string): Promise<ConfirmedSignup | null> {
    return this.confirmed.get(emailHash) ?? null;
  }

  async saveConfirmed(record: ConfirmedSignup): Promise<void> {
    this.confirmed.set(record.emailHash, record);
  }

  async getRateLimit(key: string): Promise<RateLimitRecord | null> {
    return this.rateLimits.get(key) ?? null;
  }

  async saveRateLimit(key: string, record: RateLimitRecord): Promise<void> {
    this.rateLimits.set(key, record);
  }
}

export class VercelBlobSignupStore implements SignupStore {
  async getPending(tokenHash: string): Promise<PendingSignup | null> {
    return readJson<PendingSignup>(pendingPath(tokenHash));
  }

  async getPendingByEmailHash(emailHash: string): Promise<PendingSignup | null> {
    const index = await readJson<{ tokenHash: string }>(pendingEmailPath(emailHash));
    return index ? this.getPending(index.tokenHash) : null;
  }

  async savePending(record: PendingSignup): Promise<void> {
    const existing = await this.getPendingByEmailHash(record.emailHash);

    await Promise.all([
      writeJson(pendingPath(record.tokenHash), record),
      writeJson(pendingEmailPath(record.emailHash), { tokenHash: record.tokenHash }),
    ]);

    if (existing && existing.tokenHash !== record.tokenHash) {
      await del(pendingPath(existing.tokenHash));
    }
  }

  async deletePending(record: PendingSignup): Promise<void> {
    await Promise.allSettled([del(pendingPath(record.tokenHash)), del(pendingEmailPath(record.emailHash))]);
  }

  async getConfirmed(emailHash: string): Promise<ConfirmedSignup | null> {
    return readJson<ConfirmedSignup>(confirmedPath(emailHash));
  }

  async saveConfirmed(record: ConfirmedSignup): Promise<void> {
    await writeJson(confirmedPath(record.emailHash), record);
  }

  async getRateLimit(key: string): Promise<RateLimitRecord | null> {
    return readJson<RateLimitRecord>(rateLimitPath(key));
  }

  async saveRateLimit(key: string, record: RateLimitRecord): Promise<void> {
    await writeJson(rateLimitPath(key), record);
  }
}

function pendingPath(tokenHash: string): string {
  return `signups/pending/${tokenHash}.json`;
}

function pendingEmailPath(emailHash: string): string {
  return `signups/pending-email/${emailHash}.json`;
}

function confirmedPath(emailHash: string): string {
  return `signups/confirmed/${emailHash}.json`;
}

function rateLimitPath(key: string): string {
  return `signups/rate/${key}.json`;
}

async function readJson<T>(pathname: string): Promise<T | null> {
  const result = await get(pathname, { access: "private", useCache: false });
  if (!result || result.statusCode !== 200) {
    return null;
  }

  const text = await new Response(result.stream).text();
  return JSON.parse(text) as T;
}

async function writeJson(pathname: string, value: unknown): Promise<void> {
  await put(pathname, JSON.stringify(value), JSON_OPTIONS);
}

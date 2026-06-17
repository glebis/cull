import { addDays, iso } from "./time.js";
import type { SignupStore } from "./store.js";
import type { RateLimitDecision } from "./types.js";

export async function consumeRateLimit(
  store: SignupStore,
  key: string,
  limit: number,
  windowMs: number,
  now = new Date(),
): Promise<RateLimitDecision> {
  const current = await store.getRateLimit(key);

  if (!current || Date.parse(current.resetAt) <= now.getTime()) {
    await store.saveRateLimit(key, {
      count: 1,
      resetAt: iso(new Date(now.getTime() + windowMs)),
    });
    return { allowed: true };
  }

  if (current.count >= limit) {
    return {
      allowed: false,
      retryAfterSeconds: Math.max(1, Math.ceil((Date.parse(current.resetAt) - now.getTime()) / 1000)),
    };
  }

  await store.saveRateLimit(key, {
    count: current.count + 1,
    resetAt: current.resetAt,
  });
  return { allowed: true };
}

export function rateLimitKeys(emailHash: string, ipHash?: string): string[] {
  const keys = [`email/${emailHash}`];
  if (ipHash) {
    keys.push(`ip/${ipHash}`);
  }
  return keys;
}

export const SUBSCRIBE_RATE_LIMITS = {
  email: { limit: 3, windowMs: 60 * 60 * 1000 },
  ip: { limit: 12, windowMs: 60 * 60 * 1000 },
};

export function sevenDayExpiry(now = new Date()): string {
  return iso(addDays(now, 7));
}

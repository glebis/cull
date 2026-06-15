import { generateToken, hashEmail, hashIp, hashToken } from "./crypto";
import { normalizeEmail } from "./email";
import { consumeRateLimit, sevenDayExpiry, SUBSCRIBE_RATE_LIMITS } from "./rate-limit";
import { isExpired, iso } from "./time";
import type { EmailSender } from "./resend";
import type { SignupStore } from "./store";
import type { ConfirmedSignup, PendingSignup, SignupStatus } from "./types";

export type SignupConfig = {
  secret: string;
  siteUrl: string;
};

export type SubscribeInput = {
  email: string;
  ip?: string;
  userAgent?: string;
  source?: string;
  now?: Date;
};

export type SubscribeResult = {
  status: SignupStatus;
  email?: string;
  retryAfterSeconds?: number;
};

export type ConfirmResult = {
  status: SignupStatus;
  email?: string;
};

export async function subscribe(
  input: SubscribeInput,
  store: SignupStore,
  sender: EmailSender,
  config: SignupConfig,
): Promise<SubscribeResult> {
  const email = normalizeEmail(input.email);
  if (!email) {
    return { status: "invalid_email" };
  }

  const now = input.now ?? new Date();
  const emailHash = hashEmail(email, config.secret);
  const ipHash = input.ip ? hashIp(input.ip, config.secret) : undefined;

  if (await store.getConfirmed(emailHash)) {
    return { status: "already_confirmed", email };
  }

  const emailLimit = await consumeRateLimit(
    store,
    `email/${emailHash}`,
    SUBSCRIBE_RATE_LIMITS.email.limit,
    SUBSCRIBE_RATE_LIMITS.email.windowMs,
    now,
  );
  if (!emailLimit.allowed) {
    return { status: "rate_limited", retryAfterSeconds: emailLimit.retryAfterSeconds };
  }

  if (ipHash) {
    const ipLimit = await consumeRateLimit(
      store,
      `ip/${ipHash}`,
      SUBSCRIBE_RATE_LIMITS.ip.limit,
      SUBSCRIBE_RATE_LIMITS.ip.windowMs,
      now,
    );
    if (!ipLimit.allowed) {
      return { status: "rate_limited", retryAfterSeconds: ipLimit.retryAfterSeconds };
    }
  }

  const existingPending = await store.getPendingByEmailHash(emailHash);
  if (existingPending) {
    await store.deletePending(existingPending);
  }

  const token = generateToken();
  const tokenHash = hashToken(token, config.secret);
  const pending: PendingSignup = {
    email,
    tokenHash,
    emailHash,
    createdAt: iso(now),
    expiresAt: sevenDayExpiry(now),
    source: input.source ?? "landing",
    userAgent: input.userAgent,
    ipHash,
  };

  try {
    await store.savePending(pending);
    await sender.sendConfirmation({
      to: email,
      confirmUrl: confirmationUrl(config.siteUrl, token),
    });
  } catch {
    return { status: "temporary_failure" };
  }

  return { status: "pending", email };
}

export async function confirm(
  token: string | undefined,
  store: SignupStore,
  config: SignupConfig,
  now = new Date(),
): Promise<ConfirmResult> {
  if (!token) {
    return { status: "invalid_token" };
  }

  const tokenHash = hashToken(token, config.secret);
  const pending = await store.getPending(tokenHash);
  if (!pending) {
    return { status: "invalid_token" };
  }

  if (isExpired(pending.expiresAt, now)) {
    await store.deletePending(pending);
    return { status: "expired_token", email: pending.email };
  }

  const existing = await store.getConfirmed(pending.emailHash);
  if (existing) {
    await store.deletePending(pending);
    return { status: "already_confirmed", email: pending.email };
  }

  const confirmed: ConfirmedSignup = {
    email: pending.email,
    emailHash: pending.emailHash,
    confirmedAt: iso(now),
    source: pending.source,
    userAgent: pending.userAgent,
  };

  await store.saveConfirmed(confirmed);
  await store.deletePending(pending);

  return { status: "confirmed", email: pending.email };
}

function confirmationUrl(siteUrl: string, token: string): string {
  const url = new URL("/api/confirm", siteUrl);
  url.searchParams.set("token", token);
  return url.toString();
}

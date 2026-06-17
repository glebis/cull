export type SignupStatus =
  | "pending"
  | "confirmed"
  | "already_confirmed"
  | "rate_limited"
  | "invalid_email"
  | "invalid_token"
  | "expired_token"
  | "temporary_failure";

export type PendingSignup = {
  email: string;
  tokenHash: string;
  emailHash: string;
  createdAt: string;
  expiresAt: string;
  source: string;
  userAgent?: string;
  ipHash?: string;
};

export type ConfirmedSignup = {
  email: string;
  emailHash: string;
  confirmedAt: string;
  source: string;
  userAgent?: string;
};

export type RateLimitRecord = {
  count: number;
  resetAt: string;
};

export type RateLimitDecision = {
  allowed: boolean;
  retryAfterSeconds?: number;
};

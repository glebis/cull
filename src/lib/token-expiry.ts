// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

/**
 * Pure token-expiry formatting and state helpers, shared between McpSettings
 * (token list) and any other surface that needs to render relative expiry.
 *
 * Tokens expire 90 days after creation by default (SEC-004); this layer makes
 * that legible: a relative phrase ("expires in 12 days" / "expired 3 days ago")
 * and a three-way state used to drive the warn/expired design tokens.
 */

const DAY_MS = 86_400_000;

/** Days remaining before an expiry is treated as "soon" and warned. */
export const WARN_THRESHOLD_DAYS = 7;

export type ExpiryState = 'ok' | 'warn' | 'expired';

function daysBetween(fromMs: number, toMs: number): number {
    return Math.floor((toMs - fromMs) / DAY_MS);
}

/**
 * Human-readable relative expiry. `null` means a non-expiring token.
 * `now` is injectable for deterministic tests.
 */
export function relativeExpiry(iso: string | null, now: number = Date.now()): string {
    if (!iso) return 'no expiry';
    const target = new Date(iso).getTime();
    if (Number.isNaN(target)) return 'no expiry';

    if (target <= now) {
        const days = daysBetween(target, now);
        if (days < 1) return 'expired today';
        return `expired ${days} day${days === 1 ? '' : 's'} ago`;
    }

    const days = daysBetween(now, target);
    if (days < 1) return 'expires today';
    return `expires in ${days} day${days === 1 ? '' : 's'}`;
}

/**
 * Three-way expiry state for styling. Tokens within WARN_THRESHOLD_DAYS warn,
 * past-due tokens are expired, everything else is ok.
 */
export function expiryState(iso: string | null, now: number = Date.now()): ExpiryState {
    if (!iso) return 'ok';
    const target = new Date(iso).getTime();
    if (Number.isNaN(target)) return 'ok';

    if (target <= now) return 'expired';
    if (daysBetween(now, target) <= WARN_THRESHOLD_DAYS) return 'warn';
    return 'ok';
}

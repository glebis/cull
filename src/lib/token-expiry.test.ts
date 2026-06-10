import { describe, expect, it } from 'vitest';
import { relativeExpiry, expiryState, WARN_THRESHOLD_DAYS } from '$lib/token-expiry';

const NOW = new Date('2026-06-10T12:00:00Z').getTime();
const day = 86_400_000;

describe('relativeExpiry', () => {
    it('returns "no expiry" for a null timestamp', () => {
        expect(relativeExpiry(null, NOW)).toBe('no expiry');
    });

    it('reports days remaining for a future expiry', () => {
        expect(relativeExpiry(new Date(NOW + 12 * day).toISOString(), NOW)).toBe('expires in 12 days');
    });

    it('uses singular day at one day out', () => {
        expect(relativeExpiry(new Date(NOW + 1 * day).toISOString(), NOW)).toBe('expires in 1 day');
    });

    it('reports days since expiry for a past timestamp', () => {
        expect(relativeExpiry(new Date(NOW - 3 * day).toISOString(), NOW)).toBe('expired 3 days ago');
    });

    it('uses singular day for a one-day-old expiry', () => {
        expect(relativeExpiry(new Date(NOW - 1 * day).toISOString(), NOW)).toBe('expired 1 day ago');
    });

    it('says "expires today" inside the final day', () => {
        expect(relativeExpiry(new Date(NOW + 3_600_000).toISOString(), NOW)).toBe('expires today');
    });
});

describe('expiryState', () => {
    it('treats a null expiry as ok', () => {
        expect(expiryState(null, NOW)).toBe('ok');
    });

    it('treats a far-future expiry as ok', () => {
        expect(expiryState(new Date(NOW + 30 * day).toISOString(), NOW)).toBe('ok');
    });

    it('warns when expiry is within the warn threshold', () => {
        expect(WARN_THRESHOLD_DAYS).toBe(7);
        expect(expiryState(new Date(NOW + 5 * day).toISOString(), NOW)).toBe('warn');
    });

    it('warns at exactly the warn threshold boundary (within 7 days)', () => {
        expect(expiryState(new Date(NOW + 7 * day).toISOString(), NOW)).toBe('warn');
    });

    it('is ok just beyond the warn threshold (8 days out)', () => {
        expect(expiryState(new Date(NOW + 8 * day).toISOString(), NOW)).toBe('ok');
    });

    it('reports expired when the timestamp is in the past', () => {
        expect(expiryState(new Date(NOW - day).toISOString(), NOW)).toBe('expired');
    });
});

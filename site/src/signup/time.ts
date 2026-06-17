const DAY_MS = 24 * 60 * 60 * 1000;

export function addDays(date: Date, days: number): Date {
  return new Date(date.getTime() + days * DAY_MS);
}

export function isExpired(expiresAt: string, now = new Date()): boolean {
  const expires = Date.parse(expiresAt);
  return Number.isNaN(expires) || expires <= now.getTime();
}

export function iso(date: Date): string {
  return date.toISOString();
}

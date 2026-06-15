import { describe, expect, it } from "vitest";
import { addDays, isExpired, iso } from "./time";

describe("time helpers", () => {
  it("adds whole days", () => {
    expect(iso(addDays(new Date("2026-06-16T00:00:00.000Z"), 7))).toBe(
      "2026-06-23T00:00:00.000Z",
    );
  });

  it("detects expired timestamps", () => {
    const now = new Date("2026-06-16T12:00:00.000Z");

    expect(isExpired("2026-06-16T11:59:59.999Z", now)).toBe(true);
    expect(isExpired("2026-06-16T12:00:00.000Z", now)).toBe(true);
    expect(isExpired("2026-06-16T12:00:00.001Z", now)).toBe(false);
    expect(isExpired("not-a-date", now)).toBe(true);
  });
});

import { describe, expect, it } from "vitest";
import { formatAddedAt } from "./dates";

describe("formatAddedAt", () => {
  const now = new Date("2026-10-26T12:00:00Z").getTime();
  it("is relative for recent additions and a date for older ones", () => {
    expect(formatAddedAt("2026-10-26T11:55:00Z", now)).toBe("5 мин. назад");
    expect(formatAddedAt("2026-10-25T11:00:00Z", now)).toBe("вчера");
    expect(formatAddedAt("2026-01-03T12:00:00Z", now)).toBe("3 янв. 2026 г.");
  });
});

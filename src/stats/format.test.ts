import { describe, expect, it } from "vitest";
import { formatListenTime, playsWord } from "./format";

describe("stats format", () => {
  it("formats time and plurals", () => {
    expect(formatListenTime(40_000)).toBe("40 с");
    expect(formatListenTime(45 * 60000)).toBe("45 мин");
    expect(formatListenTime(192 * 60000)).toBe("3 ч 12 мин");
    expect(playsWord(1)).toBe("1 прослушивание");
    expect(playsWord(3)).toBe("3 прослушивания");
    expect(playsWord(11)).toBe("11 прослушиваний");
    expect(playsWord(22)).toBe("22 прослушивания");
  });
});

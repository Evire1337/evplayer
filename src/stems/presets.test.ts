import { describe, expect, it } from "vitest";
import { FLAT_STEMS } from "./model";
import { addPreset, decodeStemPreset, encodeStemPreset, movePreset, newPreset, overwritePreset, removePreset, renamePreset, samePresetLevels } from "./presets";

const make = (name: string, n: number) => newPreset(name, { ...FLAT_STEMS, bass: 1.2 }, ["vocals"], n);

describe("stem presets", () => {
  it("keeps a copy of the levels and cuts a long name", () => {
    const faders = { ...FLAT_STEMS };
    const p = newPreset("  Очень длинное название пресета, которое не помещается  ", faders, ["vocals"], 1);
    faders.drums = 0;
    expect(p.faders.drums).toBe(1);
    expect(p.name.length).toBe(32);
    expect(newPreset("   ", FLAT_STEMS, [], 2).name).toBe("Пресет");
  });

  it("adds, renames, updates in place, removes and reorders", () => {
    let list = addPreset(addPreset([], make("A", 1)), make("B", 2));
    expect(list.map((p) => p.name)).toEqual(["A", "B"]);
    list = renamePreset(list, list[0].id, "Первый");
    expect(list[0].name).toBe("Первый");
    expect(renamePreset(list, list[0].id, "  ")[0].name).toBe("Первый");
    list = overwritePreset(list, list[1].id, { ...FLAT_STEMS, drums: 0 }, ["bass"]);
    expect(list[1].faders.drums).toBe(0);
    expect(list[1].muted).toEqual(["bass"]);
    list = movePreset(list, 1, 0);
    expect(list.map((p) => p.name)).toEqual(["B", "Первый"]);
    expect(removePreset(list, list[0].id).map((p) => p.name)).toEqual(["Первый"]);
  });

  it("tells whether the levels are a preset's", () => {
    const p = make("A", 1);
    expect(samePresetLevels(p, { ...FLAT_STEMS, bass: 1.2 }, ["vocals"])).toBe(true);
    expect(samePresetLevels(p, FLAT_STEMS, ["vocals"])).toBe(false);
    expect(samePresetLevels(p, { ...FLAT_STEMS, bass: 1.2 }, [])).toBe(false);
  });

  it("goes through a code and back, also with line breaks and a byte-order mark", () => {
    const p = make("Караоке ♪", 5);
    const code = encodeStemPreset(p);
    expect(code.startsWith("EVS1.")).toBe(true);
    const back = decodeStemPreset(`﻿${code.replace(/(.{12})/g, "$1\n")}`, 9);
    expect(back?.name).toBe("Караоке ♪");
    expect(back?.faders.bass).toBe(1.2);
    expect(back?.muted).toEqual(["vocals"]);
  });

  it("refuses what is not a code", () => {
    expect(decodeStemPreset("hello")).toBeNull();
    expect(decodeStemPreset("EVS1.@@@")).toBeNull();
    expect(decodeStemPreset("EVS1.e30")).toBeNull();
  });
});

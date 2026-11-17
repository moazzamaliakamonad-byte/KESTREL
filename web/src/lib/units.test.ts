import { describe, expect, it } from "vitest";
import { UNITS, splitUnits } from "./units";

const units = (s: string) => splitUnits(s).filter((p) => p.unit).map((p) => p.unit);

describe("units", () => {
  it("finds units after numbers, longest match first", () => {
    expect(units("61.2 m/s² · 6.24 g")).toEqual(["m/s²", "g"]);
    expect(units("1528 kg/s")).toEqual(["kg/s"]);
    expect(units("19.25 MN·m · arm 26.0 m")).toEqual(["MN·m", "m"]);
    expect(units("454 m/s · Mach 1.53")).toEqual(["m/s", "Mach"]);
    expect(units("36.8 kN / rad")).toEqual(["kN", "/ rad"]);
  });

  it("does not take letters inside words for units", () => {
    expect(units("Engines 3 of 13")).toEqual([]);
    expect(units("guidance")).toEqual([]);
  });

  it("keeps all the text", () => {
    const s = "2.6 / 15.0 ° · 0.5 s old";
    expect(splitUnits(s).map((p) => p.text).join("")).toBe(s);
  });

  it("explains every unit it knows", () => {
    for (const [k, v] of Object.entries(UNITS)) {
      expect(v.name.length, k).toBeGreaterThan(0);
      expect(v.what.length, k).toBeGreaterThan(10);
    }
  });
});

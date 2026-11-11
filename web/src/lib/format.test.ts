import { describe, expect, it } from "vitest";
import { fmtForce, fmtMass, fmtNumber, fmtSpeed, fmtTime } from "./format";

describe("format", () => {
  it("shows a dash for non-finite values", () => {
    expect(fmtNumber(NaN, 1)).toBe("—");
    expect(fmtForce(Infinity)).toBe("—");
    expect(fmtTime(NaN)).toBe("—");
  });

  it("formats force in kilonewtons", () => {
    expect(fmtForce(13258.17)).toBe("13.26 kN");
  });

  it("formats mission time", () => {
    expect(fmtTime(12.34)).toBe("T+12.3 s");
  });

  it("formats mass and speed", () => {
    expect(fmtMass(194.39)).toBe("194.4 kg");
    expect(fmtSpeed(-75)).toBe("-75.0 m/s");
  });
});

import { describe, expect, it } from "vitest";
import { glideMargin, thrustShare, tiltMargin } from "./margins";

const deg = (r: number) => (r * Math.PI) / 180;

describe("constraint margins", () => {
  it("glide slope: elevation above the cone, degrees", () => {
    // 1000 m out, 1000 m up: 45° elevation, cone at 4°.
    expect(glideMargin([1000, 0, 1000], deg(4))).toBeCloseTo(41);
    expect(glideMargin([0, 1000, 1000], deg(4))).toBeCloseTo(41);
    // Exactly on the cone: zero; below it: negative.
    expect(glideMargin([1000, 0, 1000 * Math.tan(deg(4))], deg(4))).toBeCloseTo(0);
    expect(glideMargin([3000, 0, 100], deg(4))).toBeLessThan(0);
    // Straight above the pad.
    expect(glideMargin([0, 0, 50], deg(4))).toBeCloseTo(86);
  });

  it("tilt: limit minus the thrust's angle from vertical, degrees", () => {
    expect(tiltMargin([0, 0, 5000], deg(45))).toBeCloseTo(45);
    expect(tiltMargin([5000, 0, 5000], deg(45))).toBeCloseTo(0);
    expect(tiltMargin([0, 0, 0], deg(45))).toBeNull();
  });

  it("thrust as a share of what the working engines can give", () => {
    expect(thrustShare([0, 0, 6000], 12000)).toBeCloseTo(0.5);
    expect(thrustShare([0, 0, 6000], 0)).toBe(0);
  });
});

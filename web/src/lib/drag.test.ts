import { describe, expect, it } from "vitest";
import { clampPosition, clampVelocity, LIMITS, velocityFromHandle } from "./drag";

const gamma = (4 * Math.PI) / 180;

describe("drag limits", () => {
  it("keeps the start inside the glide-slope cone with margin", () => {
    const p = clampPosition(3000, 10, gamma);
    expect(p.z).toBeGreaterThanOrEqual(1.05 * Math.tan(gamma) * 3000 + 20);
  });

  it("bounds the box", () => {
    const p = clampPosition(9999, 9999, gamma);
    expect(p.x).toBe(LIMITS.maxAbsX);
    expect(p.z).toBe(LIMITS.maxZ);
    expect(clampPosition(-9999, 500, gamma).x).toBe(-LIMITS.maxAbsX);
  });

  it("turns non-finite pointer values into finite, allowed ones", () => {
    for (const [x, z] of [
      [NaN, NaN],
      [Infinity, -Infinity],
      [-Infinity, NaN],
    ]) {
      const p = clampPosition(x, z, gamma);
      expect(Number.isFinite(p.x) && Number.isFinite(p.z)).toBe(true);
      expect(Math.abs(p.x)).toBeLessThanOrEqual(LIMITS.maxAbsX);
      expect(p.z).toBeLessThanOrEqual(LIMITS.maxZ);
      expect(p.z).toBeGreaterThanOrEqual(1.05 * Math.tan(gamma) * Math.abs(p.x) + 20);
    }
    const v = clampVelocity(NaN, Infinity);
    expect(Number.isFinite(v.vx) && Number.isFinite(v.vz)).toBe(true);
  });

  it("stays consistent for a steep cone (no x can satisfy z ≤ max)", () => {
    const steep = (80 * Math.PI) / 180;
    const p = clampPosition(4000, 100, steep);
    expect(p.z).toBeLessThanOrEqual(LIMITS.maxZ);
    expect(p.z).toBeGreaterThanOrEqual(1.05 * Math.tan(steep) * Math.abs(p.x) + 20 - 1e-9);
  });

  it("caps speed at 150 m/s", () => {
    const v = clampVelocity(300, -400);
    expect(Math.hypot(v.vx, v.vz)).toBeCloseTo(LIMITS.maxSpeed);
    expect(v.vx / v.vz).toBeCloseTo(300 / -400);
  });

  it("velocity from the arrow handle is displacement over 2 s", () => {
    expect(velocityFromHandle({ x: 0, z: 1000 }, { x: 100, z: 900 })).toEqual({ vx: 50, vz: -50 });
  });
});

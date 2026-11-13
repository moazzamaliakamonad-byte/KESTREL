import { describe, expect, it } from "vitest";
import { heightAt, TERRAIN } from "./terrain";

describe("terrain", () => {
  it("is deterministic for a seed", () => {
    expect(heightAt(1234, -567, 7)).toBe(heightAt(1234, -567, 7));
    expect(heightAt(1234, -567, 7)).not.toBe(heightAt(1234, -567, 8));
  });

  it("is flat around the pad", () => {
    for (const [x, y] of [
      [0, 0],
      [80, -60],
      [-119, 0],
      [0, 119],
    ]) {
      expect(Math.abs(heightAt(x, y, 7))).toBeLessThan(0.01);
    }
  });

  it("is finite and bounded everywhere on the map", () => {
    const half = TERRAIN.size / 2;
    for (let x = -half; x <= half; x += 250) {
      for (let y = -half; y <= half; y += 250) {
        const h = heightAt(x, y, 7);
        expect(Number.isFinite(h)).toBe(true);
        expect(Math.abs(h)).toBeLessThan(200);
      }
    }
  });

  it("has real relief away from the pad", () => {
    let min = Infinity;
    let max = -Infinity;
    for (let x = -3000; x <= 3000; x += 100) {
      const h = heightAt(x, 2000, 7);
      min = Math.min(min, h);
      max = Math.max(max, h);
    }
    expect(max - min).toBeGreaterThan(10);
  });
});

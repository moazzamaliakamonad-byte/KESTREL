import { describe, expect, it } from "vitest";
import { mix, SPEED_OF_SOUND, type Hearing } from "./audio";

const base: Hearing = { power: 1, engines: 13, distance: 300, onboard: false, q: 20_000, air: 1, paused: false };

describe("what the camera hears", () => {
  it("is silent while paused or with no engine running (apart from the wind)", () => {
    expect(mix({ ...base, paused: true })).toMatchObject({ roar: 0, rumble: 0, wind: 0 });
    const off = mix({ ...base, engines: 0 });
    expect(off.roar).toBe(0);
    expect(off.wind).toBeGreaterThan(0);
  });

  it("gets quieter and duller with distance, and later", () => {
    const near = mix({ ...base, distance: 150 });
    const far = mix({ ...base, distance: 3000 });
    expect(far.roar).toBeLessThan(near.roar);
    expect(far.cutoff).toBeLessThan(near.cutoff);
    expect(far.crackle).toBeLessThan(near.crackle / 4);
    expect(far.delay).toBeCloseTo(3000 / SPEED_OF_SOUND);
    expect(near.delay).toBeLessThan(0.5);
  });

  it("onboard is immediate, deep, and windy with dynamic pressure", () => {
    const calm = mix({ ...base, onboard: true, q: 1000 });
    const fast = mix({ ...base, onboard: true, q: 60_000 });
    expect(fast.delay).toBe(0);
    expect(fast.cutoff).toBeLessThan(400);
    expect(fast.rumble).toBeGreaterThan(0.5);
    expect(fast.wind).toBeGreaterThan(calm.wind * 3);
  });

  it("thin air carries no engine sound to an outside camera", () => {
    expect(mix({ ...base, air: 0.0005 }).roar).toBeLessThan(0.01);
    expect(mix({ ...base, air: 0.0005, onboard: true }).rumble).toBeGreaterThan(0.5);
  });

  it("more thrust is louder", () => {
    expect(mix({ ...base, power: 1 }).roar).toBeGreaterThan(mix({ ...base, power: 0.2 }).roar);
  });
});

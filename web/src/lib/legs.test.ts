import { describe, expect, it } from "vitest";
import { legDeploy } from "./legs";

describe("landing legs", () => {
  it("are stowed high up", () => {
    expect(legDeploy(500)).toBe(0);
    expect(legDeploy(100)).toBe(0);
  });

  it("are fully out near the ground", () => {
    expect(legDeploy(50)).toBe(1);
    expect(legDeploy(0)).toBe(1);
  });

  it("unfold monotonically on the way down", () => {
    let last = 0;
    for (let alt = 100; alt >= 50; alt -= 1) {
      const d = legDeploy(alt);
      expect(d).toBeGreaterThanOrEqual(last);
      last = d;
    }
  });
});

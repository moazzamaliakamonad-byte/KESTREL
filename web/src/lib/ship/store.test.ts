import { beforeEach, describe, expect, it } from "vitest";
import { clampStart, START_LIMITS, useShip, verdict } from "./store";

describe("ship store", () => {
  beforeEach(() => useShip.setState({ scenario: "earth", mode: "editing" }));

  it("keeps starts in the scenario's box and the x–z plane", () => {
    const s = clampStart("earth", { position: [9e9, 5, -3], velocity: [0, 7, 0] });
    expect(s.position).toEqual([START_LIMITS.earth.x[1], 0, START_LIMITS.earth.z[0]]);
    expect(s.velocity[1]).toBe(0);
    expect(s.velocity[2]).toBe(START_LIMITS.earth.vz[1]);
  });

  it("switching scenario resets the start, not during a flight", () => {
    useShip.getState().setScenario("mars");
    expect(useShip.getState().start.position[2]).toBeGreaterThanOrEqual(10_000);
    useShip.getState().land();
    useShip.getState().setScenario("earth");
    expect(useShip.getState().scenario).toBe("mars");
  });

  it("edits only while editing", () => {
    useShip.getState().reset();
    useShip.getState().land();
    const before = useShip.getState().start;
    useShip.getState().setStart({ position: [0, 0, 6000], velocity: [0, 0, -80] });
    expect(useShip.getState().start).toBe(before);
  });

  it("says what happened", () => {
    expect(verdict("earth", "caught", null)).toBe("Caught by the tower");
    expect(verdict("mars", "landed", null)).toBe("Landed on Mars");
    expect(verdict("earth", "crashed", "hit the tower")).toBe("Lost: hit the tower");
  });
});

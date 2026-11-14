import { describe, expect, it } from "vitest";
import { sampleOf, viewPatch, type Snapshot } from "./flightSnapshot";
import type { FlightView } from "./state";

const snap: Snapshot = {
  t: 12.5,
  position: [900, 0, 600],
  velocity: [-40, 0, -50],
  mass: 1830,
  thrust: [3000, 0, 4000],
  command: [3100, 0, 4100],
  nav_position: [903, 1, 598],
  engines: 6,
  min_thrust: 4972,
  max_thrust: 13258,
  lag: 0,
  landed: false,
  mode: "pinpoint",
  miss: 0,
  plan_remaining: 20,
  outcome: null,
};

const view: FlightView = {
  mode: "pinpoint",
  miss: 0,
  engines: 6,
  plan: null,
  nearTouchdown: false,
  outcome: null,
};

describe("sampleOf", () => {
  it("reads the true state and actual thrust", () => {
    const s = sampleOf(snap);
    expect(s).toMatchObject({ t: 12.5, position: [900, 0, 600], mass: 1830, done: false });
    expect(s.thrustMag).toBe(5000);
    expect(sampleOf({ ...snap, landed: true }).done).toBe(true);
  });
});

describe("viewPatch", () => {
  it("is null when nothing the page shows has changed", () => {
    expect(viewPatch(snap, view)).toBeNull();
    expect(viewPatch({ ...snap, miss: 0.4 }, view)).toBeNull(); // under a metre
  });

  it("reports mode, engines and miss changes", () => {
    expect(viewPatch({ ...snap, mode: "closest", miss: 250 }, view)).toEqual({ mode: "closest", miss: 250 });
    expect(viewPatch({ ...snap, engines: 5 }, view)).toEqual({ engines: 5 });
  });

  it("flags the last seconds before touchdown", () => {
    expect(viewPatch({ ...snap, plan_remaining: 7 }, view)).toEqual({ nearTouchdown: true });
    expect(viewPatch({ ...snap, plan_remaining: null }, { ...view, nearTouchdown: true })).toBeNull();
  });
});

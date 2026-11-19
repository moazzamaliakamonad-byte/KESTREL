import { describe, expect, it } from "vitest";
import { decisionsBetween, frameAt, gateAt, planDecision, planPositionAt, pushFrame, replanReason, windReadout, windWords, WindWatch, FRAMES_MAX } from "./record";
import type { EngineState, PlanDraw, ShipSnapshot } from "./types";

function snap(patch: Partial<ShipSnapshot>): ShipSnapshot {
  return {
    t: 10,
    vehicle: "ship",
    phase: "belly-flop",
    position: [100, 0, 1000],
    velocity: [0, 0, -80],
    quat: [1, 0, 0, 0],
    omega: [0, 0, 0],
    tilt_deg: 90,
    mass: 140e3,
    dry_mass: 100e3,
    propellant: 40e3,
    engines: ["off", "off", "off"] as EngineState[],
    thrust: [0, 0, 0],
    thrust_inertial: [0, 0, 0],
    thrust_mag: 0,
    thrust_limits: [0, 0],
    gimbal_deg: 0,
    flaps: [0.5, 0.5, 0.5, 0.5],
    rcs: [],
    q_dyn: 0,
    mach: 0.2,
    accel_g: 0,
    aero_force: [0, 0, 0],
    weight: 1.37e6,
    density: 1.1,
    heat_flux: 0,
    arms: false,
    wind: [0, 0, 0],
    wind_est: [0, 0],
    wind_sigma: [15, 15],
    wind_aware: true,
    bearing: 0,
    bearing_chosen: false,
    guide: "none",
    leg_engines: 0,
    ignition_z: 900,
    grabbed: false,
    outcome: null,
    reason: null,
    end_velocity: null,
    engine_out_margin: null,
    entry: null,
    entry_summary: null,
    earth_frame: null,
    ...patch,
  };
}

const plan: PlanDraw = {
  t0: 50,
  final_time: 10,
  position: [
    [100, 0, 600],
    [50, 0, 400],
    [10, 0, 114],
  ],
  quat: [],
  thrust: [3e6, 1.6e6, 1.2e6],
};

describe("flight record", () => {
  it("records the start, ignition, an engine shutdown, a failure and a relight with their numbers", () => {
    const start = decisionsBetween(null, snap({}));
    expect(start[0]).toMatchObject({ kind: "phase", title: "Belly flop begins" });
    expect(start[0].facts).toContainEqual(["Ignition altitude (rule)", "900 m"]);

    const lit = decisionsBetween(snap({}), snap({ phase: "flip", engines: ["igniting", "igniting", "igniting"] }));
    expect(lit.map((d) => d.kind)).toEqual(["ignition"]);
    expect(lit[0].facts).toContainEqual(["Engines lit", "3"]);

    const down = decisionsBetween(snap({ engines: ["running", "running", "running"] }), snap({ engines: ["running", "shutdown", "shutdown"] }));
    expect(down[0]).toMatchObject({ kind: "engines", title: "Down to 1 engine" });

    const fail = decisionsBetween(snap({ engines: ["running", "shutdown", "shutdown"] }), snap({ engines: ["failed", "shutdown", "shutdown"] }));
    expect(fail[0]).toMatchObject({ kind: "failure", title: "Engine 1 failed" });

    const relit = decisionsBetween(snap({ engines: ["failed", "shutdown", "shutdown"] }), snap({ engines: ["failed", "igniting", "shutdown"] }));
    expect(relit[0]).toMatchObject({ kind: "relight", title: "Spare engine 2 relit" });
  });

  it("records the outcome with the contact numbers", () => {
    const d = decisionsBetween(snap({ phase: "terminal" }), snap({ phase: "done", outcome: "caught", end_velocity: [0.3, 0.4, -1.2], position: [1, 0, 94] }));
    const out = d.find((x) => x.kind === "outcome")!;
    expect(out.title).toBe("Caught");
    expect(out.facts).toContainEqual(["Down at contact", "1.20 m/s"]);
    expect(out.facts).toContainEqual(["Sideways at contact", "0.50 m/s"]);
  });

  it("says why guidance re-plans: no plan, drift, age, or an event", () => {
    expect(replanReason(snap({}), null, null).why).toMatch(/No plan/);
    expect(replanReason(snap({}), null, "Gust thrown: x").why).toBe("Gust thrown: x");
    // On the plan at t = 55: halfway along the first leg (75, 0, 500).
    expect(planPositionAt(plan, 55)).toEqual([50, 0, 400]);
    const drifted = replanReason(snap({ t: 55, position: [50, 8, 400] }), plan, null);
    expect(drifted.why).toMatch(/drifted more than 5 m/);
    expect(drifted.facts).toContainEqual(["Off the plan", "8.0 m"]);
    const old = replanReason(snap({ t: 54, position: [60, 0, 440] }), plan, null);
    expect(old.why).toMatch(/3 s old/);
  });

  it("records a plan with its solve time and shape, and a failure", () => {
    const ok = planDecision(snap({ t: 50 }), { why: "No plan to follow yet.", facts: [] }, { ok: true, plan, ms: 42, iterations: 4, converged: true, engines: 1, gateZ: 114 });
    expect(ok.title).toBe("New plan: 10.0 s to the gate on 1 engine");
    expect(ok.facts).toContainEqual(["Solve time", "42 ms"]);
    expect(ok.facts).toContainEqual(["Peak planned thrust", "3.00 MN"]);
    const bad = planDecision(snap({ t: 50 }), { why: "x", facts: [] }, { ok: false, ms: 300, error: "Infeasible" });
    expect(bad.kind).toBe("plan-failed");
    expect(bad.why).toMatch(/Infeasible/);
  });

  it("keeps frames bounded and finds the nearest one", () => {
    let frames: ShipSnapshot[] = [];
    for (let i = 0; i <= FRAMES_MAX; i++) frames = pushFrame(frames, snap({ t: i * 0.1 }));
    expect(frames.length).toBeLessThanOrEqual(FRAMES_MAX);
    expect(frameAt(frames, 12.34)?.t).toBeCloseTo(12.4, 5);
    expect(frameAt(frames, -5)?.t).toBe(0);
    expect(frameAt([], 3)).toBeNull();
  });
});

describe("wind in the flight record", () => {
  it("names where the wind comes from", () => {
    // Blowing toward −y comes from the north (+y).
    expect(windWords([0, -12])).toBe("12 m/s from the north (+y)");
    expect(windWords([-8, 0])).toBe("8 m/s from the sea (+x)");
    expect(windWords([0.2, 0.1])).toBe("calm");
  });

  it("records once when the estimate settles", () => {
    const w = new WindWatch();
    const out = [15, 8, 3, 1.9, 1.5, 1.2].flatMap((sig, i) =>
      w.update(snap({ t: i, wind_est: [0, -12], wind_sigma: [sig, sig] })),
    );
    expect(out).toHaveLength(1);
    expect(out[0].kind).toBe("wind");
    expect(out[0].t).toBe(3);
    expect(out[0].title).toBe("Wind estimated: 12 m/s from the north (+y)");
    expect(out[0].facts).toContainEqual(["Uncertainty (1σ)", "±1.9 m/s"]);
  });

  it("records a gust when the estimate moves more than 4 m/s within 3 s, at most every 4 s", () => {
    const w = new WindWatch();
    const at = (t: number, wy: number) => w.update(snap({ t, wind_est: [0, wy], wind_sigma: [1, 1] }));
    at(0, -10); // settles
    expect(at(1, -10)).toHaveLength(0);
    const gust = at(2.5, -16);
    expect(gust).toHaveLength(1);
    expect(gust[0].kind).toBe("gust");
    expect(gust[0].title).toBe("Gust felt: 6 m/s change");
    expect(at(4, -22)).toHaveLength(0); // within 4 s of the last entry
    expect(at(6.6, -10)).toHaveLength(1); // back by 12 m/s within 3 s, 4.1 s after the last entry
  });

  it("says nothing while the estimate is still unsure", () => {
    const w = new WindWatch();
    expect(w.update(snap({ t: 0, wind_est: [0, 0], wind_sigma: [15, 15] }))).toHaveLength(0);
    expect(w.update(snap({ t: 1, wind_est: [9, 0], wind_sigma: [9, 9] }))).toHaveLength(0);
  });
});

describe("wind readout", () => {
  it("says it is still estimating until it is sure, and when guidance ignores it", () => {
    expect(windReadout({ wind_est: [3, 4], wind_sigma: [9, 9], wind_aware: true })).toBe("estimating…");
    expect(windReadout({ wind_est: [3, 4], wind_sigma: [0.4, 0.3], wind_aware: true })).toBe("5.0 m/s ±0.4");
    expect(windReadout({ wind_est: [3, 4], wind_sigma: [0.4, 0.3], wind_aware: false })).toBe("5.0 m/s ±0.4 (not used)");
    // Within two of its standard deviations of calm, guidance flies calm (D-046), and says so.
    expect(windReadout({ wind_est: [1.2, 0], wind_sigma: [0.9, 0.9], wind_aware: true })).toBe("1.2 m/s ±0.9 · flown as calm");
  });
});

describe("the approach bearing", () => {
  it("turns the gate about the catch axis, and leaves it at bearing 0", () => {
    expect(gateAt([10, 0, 114], 0)).toEqual([10, 0, 114]);
    const g = gateAt([10, 0, 114], Math.PI / 2);
    expect(g[0]).toBeCloseTo(0, 9);
    expect(g[1]).toBeCloseTo(10, 9);
    expect(g[2]).toBe(114);
  });

  it("records the approach turned, once, when the bearing is chosen and not +x", () => {
    const before = snap({ t: 5, bearing: 0, bearing_chosen: false });
    const turned = snap({ t: 6, bearing: -0.82, bearing_chosen: true });
    const out = decisionsBetween(before, turned).filter((d) => d.kind === "approach");
    expect(out).toHaveLength(1);
    expect(out[0].title).toBe("Approach turned 47° toward −y");
    expect(decisionsBetween(turned, snap({ t: 7, bearing: -0.82, bearing_chosen: true })).filter((d) => d.kind === "approach")).toHaveLength(0);
    // Chosen but kept on the +x side: nothing to say.
    expect(decisionsBetween(before, snap({ t: 6, bearing: 0, bearing_chosen: true })).filter((d) => d.kind === "approach")).toHaveLength(0);
  });
});

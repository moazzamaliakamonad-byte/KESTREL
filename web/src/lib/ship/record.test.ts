import { describe, expect, it } from "vitest";
import { decisionsBetween, frameAt, planDecision, planPositionAt, pushFrame, replanReason, FRAMES_MAX } from "./record";
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
    guide: "none",
    leg_engines: 0,
    ignition_z: 900,
    grabbed: false,
    outcome: null,
    reason: null,
    end_velocity: null,
    engine_out_margin: null,
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

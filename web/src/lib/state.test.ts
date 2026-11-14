import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/example-response.json";
import { canLand, initialState, isPreviewCurrent, reduce, type AppState } from "./state";
import type { PlanResponse, Vec3 } from "./types";

const ok = fixture as PlanResponse;
const fail: PlanResponse = { ok: false, error: { kind: "Infeasible", message: "x" } };
const start = { position: [2000, 0, 1500] as Vec3, velocity: [-50, 0, -75] as Vec3 };
const landedOutcome = { landed: true, speed: 0.5, vertical: 0.5, horizontal: 0, miss: 0.1, fuel_used: 230 };

function editingWith(response: PlanResponse, id = 1): AppState {
  let s = reduce(initialState, { type: "ready", start });
  s = reduce(s, { type: "requested", id, at: 0, start });
  return reduce(s, { type: "response", id, response });
}

describe("state machine", () => {
  it("starts loading", () => {
    expect(initialState.phase).toBe("loading");
    expect(canLand(initialState)).toBe(false);
  });

  it("ready enters editing with the start state", () => {
    const s = reduce(initialState, { type: "ready", start });
    expect(s.phase).toBe("editing");
    expect(s.start).toEqual(start);
  });

  it("a response for the latest request becomes the preview", () => {
    const s = editingWith(ok, 3);
    expect(s.preview?.requestId).toBe(3);
    expect(s.requestedAt).toBeNull();
  });

  it("a response for an older request is ignored", () => {
    let s = reduce(initialState, { type: "ready", start });
    s = reduce(s, { type: "requested", id: 1, at: 0, start });
    s = reduce(s, { type: "requested", id: 2, at: 5, start });
    s = reduce(s, { type: "response", id: 1, response: ok });
    expect(s.preview).toBeNull();
    expect(s.requestedAt).toBe(5);
  });

  it("land requires a successful preview for the latest request", () => {
    expect(canLand(editingWith(fail))).toBe(false);
    expect(reduce(editingWith(fail), { type: "land" }).phase).toBe("editing");
    const s = reduce(editingWith(ok), { type: "land" });
    expect(s.phase).toBe("flying");
    expect(s.flight).toMatchObject({ mode: "none", engines: 6, outcome: null, plan: null });
  });

  it("land is ignored while a newer solve is pending", () => {
    const s = reduce(editingWith(ok, 1), { type: "requested", id: 2, at: 9, start });
    expect(canLand(s)).toBe(false);
    expect(reduce(s, { type: "land" }).phase).toBe("editing");
  });

  it("moving the start makes the current plan unflyable immediately", () => {
    // Review finding: drag then press Space before the next solve is even
    // requested must not fly the plan for the old start.
    const moved = { position: [-1500, 0, 2500] as Vec3, velocity: start.velocity };
    const s = reduce(editingWith(ok), { type: "setStart", start: moved });
    expect(canLand(s)).toBe(false);
    expect(reduce(s, { type: "land" }).phase).toBe("editing");
  });

  it("an answer solved for an older start does not make it landable", () => {
    const moved = { position: [-1500, 0, 2500] as Vec3, velocity: start.velocity };
    let s = reduce(initialState, { type: "ready", start });
    s = reduce(s, { type: "requested", id: 1, at: 0, start });
    s = reduce(s, { type: "setStart", start: moved });
    s = reduce(s, { type: "response", id: 1, response: ok });
    expect(canLand(s)).toBe(false);
    expect(isPreviewCurrent(s)).toBe(false);
  });

  it("a flight update merges into the flight while flying", () => {
    let s = reduce(editingWith(ok), { type: "land" });
    s = reduce(s, { type: "flightUpdate", patch: { mode: "closest", miss: 240, engines: 5 } });
    expect(s.flight).toMatchObject({ mode: "closest", miss: 240, engines: 5, outcome: null });
    // Ignored once the flight is over.
    s = reduce(s, { type: "touchdown", outcome: landedOutcome });
    expect(reduce(s, { type: "flightUpdate", patch: { mode: "brake" } }).flight?.mode).toBe("closest");
  });

  it("touchdown ends the flight with its outcome", () => {
    let s = reduce(editingWith(ok), { type: "land" });
    s = reduce(s, { type: "touchdown", outcome: landedOutcome });
    expect(s.phase).toBe("touchdown");
    expect(s.flight?.outcome).toEqual(landedOutcome);
    expect(reduce(editingWith(ok), { type: "touchdown", outcome: landedOutcome }).phase).toBe("editing");
  });

  it("lag and noise are settings that survive a reset", () => {
    let s = reduce(editingWith(ok), { type: "setLag", lag: 0.3 });
    s = reduce(s, { type: "setNoise", noise: "high" });
    s = reduce(s, { type: "land" });
    s = reduce(s, { type: "reset" });
    expect(s.settings).toEqual({ lag: 0.3, noise: "high" });
  });

  it("reset returns to editing and keeps the start", () => {
    let s = reduce(editingWith(ok), { type: "land" });
    s = reduce(s, { type: "reset" });
    expect(s.phase).toBe("editing");
    expect(s.flight).toBeNull();
    expect(s.start).toEqual(start);
  });

  it("a worker error fails from any phase", () => {
    for (const s0 of [initialState, editingWith(ok), reduce(editingWith(ok), { type: "land" })]) {
      const s = reduce(s0, { type: "workerError", message: "wasm missing" });
      expect(s.phase).toBe("failed");
      expect(s.failure).toBe("wasm missing");
    }
  });
});

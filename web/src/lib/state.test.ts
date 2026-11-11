import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/example-response.json";
import { canLand, initialState, reduce, type AppState } from "./state";
import type { PlanResponse, Vec3 } from "./types";

const ok = fixture as PlanResponse;
const fail: PlanResponse = { ok: false, error: { kind: "Infeasible", message: "x" } };
const start = { position: [2000, 0, 1500] as Vec3, velocity: [-50, 0, -75] as Vec3 };
const finalTime = ok.ok ? ok.trajectory.final_time : NaN;

function editingWith(response: PlanResponse, id = 1): AppState {
  let s = reduce(initialState, { type: "ready", start });
  s = reduce(s, { type: "requested", id, at: 0 });
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
    s = reduce(s, { type: "requested", id: 1, at: 0 });
    s = reduce(s, { type: "requested", id: 2, at: 5 });
    s = reduce(s, { type: "response", id: 1, response: ok });
    expect(s.preview).toBeNull();
    expect(s.requestedAt).toBe(5);
  });

  it("land requires a successful preview for the latest request", () => {
    expect(canLand(editingWith(fail))).toBe(false);
    expect(reduce(editingWith(fail), { type: "land" }).phase).toBe("editing");
    const s = reduce(editingWith(ok), { type: "land" });
    expect(s.phase).toBe("flying");
    expect(s.flight?.t).toBe(0);
  });

  it("land is ignored while a newer solve is pending", () => {
    const s = reduce(editingWith(ok, 1), { type: "requested", id: 2, at: 9 });
    expect(canLand(s)).toBe(false);
    expect(reduce(s, { type: "land" }).phase).toBe("editing");
  });

  it("tick advances time and touches down at the end", () => {
    let s = reduce(editingWith(ok), { type: "land" });
    s = reduce(s, { type: "tick", dt: 1 });
    expect(s.flight?.t).toBeCloseTo(1);
    s = reduce(s, { type: "tick", dt: 1e6 });
    expect(s.phase).toBe("touchdown");
    expect(s.flight?.t).toBe(finalTime);
  });

  it("tick respects playback speed", () => {
    let s = reduce(editingWith(ok), { type: "setSpeed", speed: 4 });
    s = reduce(s, { type: "land" });
    s = reduce(s, { type: "tick", dt: 0.5 });
    expect(s.flight?.t).toBeCloseTo(2);
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

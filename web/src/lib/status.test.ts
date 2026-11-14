import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/example-response.json";
import { initialState, reduce, type AppState } from "./state";
import { statusText } from "./status";
import type { PlanResponse, Vec3 } from "./types";

const ok = fixture as PlanResponse;
const start = { position: [2000, 0, 1500] as Vec3, velocity: [-50, 0, -75] as Vec3 };

function editing(response: PlanResponse | null, pendingAt: number | null = null): AppState {
  let s = reduce(initialState, { type: "ready", start });
  s = reduce(s, { type: "requested", id: 1, at: 0, start });
  if (response) s = reduce(s, { type: "response", id: 1, response });
  if (pendingAt !== null) s = reduce(s, { type: "requested", id: 2, at: pendingAt, start });
  return s;
}

const failure = (kind: "Infeasible" | "NotLossless"): PlanResponse => ({
  ok: false,
  error: { kind, message: "x" },
});

describe("status line", () => {
  it("explains an impossible start", () => {
    const st = statusText(editing(failure("Infeasible")), 0);
    expect(st.tone).toBe("error");
    expect(st.text).toBe(
      "No landing exists from here: too fast, too low or too little propellant.",
    );
  });

  it("explains an uncertified plan", () => {
    expect(statusText(editing(failure("NotLossless")), 0).text).toBe(
      "Couldn't certify a flyable plan from here; try another start.",
    );
  });

  it("shows planning while a solve is pending", () => {
    expect(statusText(editing(ok, 1000), 1500).text).toBe("Planning…");
  });

  it("shows still planning when a solve is slow", () => {
    expect(statusText(editing(ok, 1000), 4500).text).toBe("Still planning…");
  });

  it("does not invite landing once the start has moved", () => {
    const moved = { position: [-1500, 0, 2500] as Vec3, velocity: start.velocity };
    const s = reduce(editing(ok), { type: "setStart", start: moved });
    expect(statusText(s, 0).text).toBe("Planning…");
  });

  it("invites landing when a plan is ready", () => {
    const st = statusText(editing(ok), 0);
    expect(st.tone).toBe("ok");
    expect(st.text).toBe("Ready: press Land");
  });

  it("reports what guidance is doing in flight", () => {
    const flying = reduce(editing(ok), { type: "land" });
    const say = (patch: object) => statusText(reduce(flying, { type: "flightUpdate", patch }), 0);
    expect(statusText(flying, 0).text).toBe("Computing the first plan…");
    expect(say({ mode: "pinpoint" })).toEqual({ tone: "info", text: "Guiding to the pad" });
    expect(say({ mode: "closest", miss: 247.4 })).toEqual({
      tone: "warn",
      text: "No pinpoint landing from here: guiding to the closest point, ≈247 m from the pad",
    });
    expect(say({ mode: "closest", miss: 0.3 }).text).toBe("Outside the glide cone: landing without it");
    expect(say({ mode: "brake" })).toEqual({ tone: "error", text: "No safe landing exists: braking" });
  });

  it("reports how the flight ended", () => {
    const flying = reduce(editing(ok), { type: "land" });
    const end = (o: object) =>
      statusText(
        reduce(flying, {
          type: "touchdown",
          outcome: { landed: true, speed: 0.5, vertical: 0.5, horizontal: 0, miss: 0.4, fuel_used: 230, ...o },
        }),
        0,
      );
    expect(end({})).toEqual({ tone: "ok", text: "Landed on the pad" });
    expect(end({ miss: 247 }).text).toBe("Landed 247 m from the pad");
    expect(end({ landed: false, speed: 38.2 })).toEqual({ tone: "error", text: "Crashed at 38 m/s" });
  });
});

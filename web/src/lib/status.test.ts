import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/example-response.json";
import { initialState, reduce, type AppState } from "./state";
import { statusText } from "./status";
import type { PlanResponse, Vec3 } from "./types";

const ok = fixture as PlanResponse;
const start = { position: [2000, 0, 1500] as Vec3, velocity: [-50, 0, -75] as Vec3 };

function editing(response: PlanResponse | null, pendingAt: number | null = null): AppState {
  let s = reduce(initialState, { type: "ready", start });
  s = reduce(s, { type: "requested", id: 1, at: 0 });
  if (response) s = reduce(s, { type: "response", id: 1, response });
  if (pendingAt !== null) s = reduce(s, { type: "requested", id: 2, at: pendingAt });
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

  it("invites landing when a plan is ready", () => {
    const st = statusText(editing(ok), 0);
    expect(st.tone).toBe("ok");
    expect(st.text).toBe("Ready: press Land");
  });

  it("reports landing and touchdown", () => {
    let s = reduce(editing(ok), { type: "land" });
    expect(statusText(s, 0).text).toBe("Landing");
    s = reduce(s, { type: "tick", dt: 1e6 });
    expect(statusText(s, 0).text).toBe("Touchdown");
  });
});

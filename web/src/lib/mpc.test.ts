import { describe, expect, it } from "vitest";
import { clampDt, createMpc, REPLAN_PERIOD } from "./mpc";

describe("MPC scheduler", () => {
  it("asks at once, then once per period of sim time", () => {
    const m = createMpc();
    expect(m.shouldRequest(0, true)).toBe(true);
    const e = m.sent(0);
    expect(m.received(e)).toBe(true);
    expect(m.shouldRequest(REPLAN_PERIOD - 0.01, true)).toBe(false);
    expect(m.shouldRequest(REPLAN_PERIOD, true)).toBe(true);
  });

  it("keeps one solve in flight", () => {
    const m = createMpc();
    m.sent(0);
    expect(m.shouldRequest(5, true)).toBe(false);
  });

  it("does not ask when the flight does not want a re-plan", () => {
    expect(createMpc().shouldRequest(10, false)).toBe(false);
  });

  it("rejects answers from before a reset", () => {
    const m = createMpc();
    const old = m.sent(0);
    m.reset();
    expect(m.received(old)).toBe(false);
    expect(m.shouldRequest(0, true)).toBe(true); // not stuck "in flight"
    const fresh = m.sent(0);
    expect(m.received(fresh)).toBe(true);
    expect(m.received(fresh)).toBe(false);
  });

  it("counts re-plans and keeps recent solve times", () => {
    const m = createMpc();
    for (let i = 0; i < 50; i++) {
      const e = m.sent(i);
      m.received(e, 10 + i);
    }
    expect(m.replans).toBe(50);
    expect(m.solveMs.length).toBe(40);
    expect(m.solveMs.at(-1)).toBe(59);
  });
});

describe("clampDt", () => {
  it("scales by speed and caps stalls", () => {
    expect(clampDt(0.016, 2)).toBeCloseTo(0.032);
    expect(clampDt(5, 1)).toBe(0.1);
    expect(clampDt(5, 4)).toBeCloseTo(0.4);
  });

  it("ignores nonsense", () => {
    expect(clampDt(Number.NaN, 1)).toBe(0);
    expect(clampDt(-1, 1)).toBe(0);
  });
});

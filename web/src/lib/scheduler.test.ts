import { describe, expect, it } from "vitest";
import { createScheduler, startChangedWhileEditing } from "./scheduler";
import { initialState, reduce } from "./state";
import type { Vec3 } from "./types";

const at = (x: number) => ({ position: [x, 0, 1500] as Vec3, velocity: [0, 0, -50] as Vec3 });

describe("solve scheduler", () => {
  it("sends at once when nothing is in flight", () => {
    const sent: number[] = [];
    const s = createScheduler((st) => sent.push(st.position[0]));
    s.submit(at(1));
    expect(sent).toEqual([1]);
  });

  it("keeps one solve in flight and sends only the newest start after it", () => {
    // Review finding: every drag step used to queue a full solve in the
    // worker, so lag grew with drag length.
    const sent: number[] = [];
    const s = createScheduler((st) => sent.push(st.position[0]));
    s.submit(at(1));
    s.submit(at(2));
    s.submit(at(3));
    s.submit(at(4));
    expect(sent).toEqual([1]);
    s.done();
    expect(sent).toEqual([1, 4]);
    s.done();
    expect(sent).toEqual([1, 4]);
  });

  it("is idle again after the last answer", () => {
    const sent: number[] = [];
    const s = createScheduler((st) => sent.push(st.position[0]));
    s.submit(at(1));
    s.done();
    s.submit(at(2));
    expect(sent).toEqual([1, 2]);
  });
});

describe("startChangedWhileEditing", () => {
  it("ignores the loading → editing transition (no duplicate first solve)", () => {
    const ready = reduce(initialState, { type: "ready", start: at(1) });
    expect(startChangedWhileEditing(ready, initialState)).toBe(false);
  });

  it("fires when the visitor moves the start", () => {
    const ready = reduce(initialState, { type: "ready", start: at(1) });
    const moved = reduce(ready, { type: "setStart", start: at(2) });
    expect(startChangedWhileEditing(moved, ready)).toBe(true);
  });

  it("does not fire for unrelated changes", () => {
    const ready = reduce(initialState, { type: "ready", start: at(1) });
    const faster = reduce(ready, { type: "setSpeed", speed: 4 });
    expect(startChangedWhileEditing(faster, ready)).toBe(false);
  });
});

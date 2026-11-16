import { describe, expect, it } from "vitest";
import { callouts, pickVoice, timeToIgnition } from "./commentary";
import { allPhrases, clipId } from "./phrases";
import type { EngineState, ShipSnapshot } from "./types";

function snap(patch: Partial<ShipSnapshot>): ShipSnapshot {
  return {
    t: 0,
    vehicle: "booster",
    phase: "descent",
    position: [0, 0, 12_000],
    velocity: [0, 0, -400],
    quat: [1, 0, 0, 0],
    omega: [0, 0, 0],
    tilt_deg: 0,
    mass: 380e3,
    dry_mass: 275e3,
    propellant: 105e3,
    engines: Array<EngineState>(13).fill("off"),
    thrust: [0, 0, 0],
    thrust_inertial: [0, 0, 0],
    thrust_mag: 0,
    thrust_limits: [0, 0],
    gimbal_deg: 0,
    flaps: [0, 0, 0, 0],
    rcs: [],
    q_dyn: 0,
    mach: 1.3,
    accel_g: 0,
    aero_force: [0, 0, 0],
    weight: 0,
    density: 0,
    heat_flux: 0,
    legs: false,
    arms: false,
    wind: [0, 0, 0],
    guide: "none",
    leg_engines: 0,
    ignition_z: 2000,
    grabbed: false,
    outcome: null,
    reason: null,
    end_velocity: null,
    ...patch,
  };
}

const texts = (c: { text: string }[]) => c.map((x) => x.text).join(" | ");
const running = (n: number) => Array.from({ length: 13 }, (_, i): EngineState => (i < n ? "running" : "shutdown"));

describe("commentary", () => {
  it("opens with the vehicle's situation", () => {
    expect(texts(callouts("booster", null, snap({})))).toMatch(/engines first/);
    expect(texts(callouts("earth", null, snap({ vehicle: "ship", phase: "belly-flop", mach: 7.4 })))).toMatch(/in entry/);
  });

  it("calls start-up, engine counts and the shutdown to three", () => {
    const a = snap({});
    const b = snap({ phase: "startup" });
    expect(callouts("booster", a, b)[0]).toEqual({ text: "Landing burn start-up.", urgent: true });
    const c = snap({ phase: "landing-burn", engines: running(13) });
    expect(texts(callouts("booster", b, c))).toBe("Thirteen engines running.");
    const d = snap({ phase: "landing-burn", engines: running(3) });
    expect(texts(callouts("booster", c, d))).toMatch(/Three centre engines continue/);
  });

  it("an engine failure is urgent and names the engine", () => {
    const a = snap({ phase: "landing-burn", engines: running(13) });
    const engines = running(13);
    engines[4] = "failed";
    const out = callouts("booster", a, snap({ phase: "landing-burn", engines }));
    expect(out).toEqual([{ text: "Engine five is out. Guidance is re-planning.", urgent: true }]);
  });

  it("calls altitudes, going subsonic and ten seconds to the burn only while unpowered", () => {
    const out = callouts("booster", snap({ position: [0, 0, 10_010], mach: 1.01 }), snap({ position: [0, 0, 9990], mach: 0.99 }));
    expect(texts(out)).toBe("Ten kilometres. | Booster is subsonic.");
    expect(timeToIgnition(snap({ position: [0, 0, 6000] }))).toBeCloseTo(10);
    const ten = callouts("booster", snap({ position: [0, 0, 6010] }), snap({ position: [0, 0, 5990] }));
    expect(texts(ten)).toBe("Ten seconds to the landing burn.");
    const burning = { phase: "landing-burn" as const };
    expect(callouts("booster", snap({ ...burning, position: [0, 0, 1010] }), snap({ ...burning, position: [0, 0, 990] }))).toEqual([]);
  });

  it("ends with the outcome and nothing else", () => {
    const out = callouts("booster", snap({ phase: "terminal" }), snap({ phase: "done", outcome: "caught", arms: true }));
    expect(out).toEqual([{ text: "Catch confirmed. Booster is on the tower.", urgent: true }]);
    const lost = callouts("earth", snap({}), snap({ outcome: "crashed", reason: "hit the tower" }));
    expect(lost[0].text).toBe("We have lost the vehicle. It hit the tower.");
    expect(callouts("earth", snap({}), snap({ outcome: "crashed", reason: "out of propellant" }))[0].text).toMatch(/ran out of propellant/);
  });

  it("every line it can say is in the list of clips to render, under a unique file name", () => {
    const all = new Set(allPhrases());
    const flights: [Parameters<typeof callouts>[0], ShipSnapshot | null, ShipSnapshot][] = [
      ["booster", null, snap({})],
      ["mars", null, snap({ vehicle: "ship", phase: "belly-flop" })],
      ["booster", snap({}), snap({ phase: "startup" })],
      ["earth", snap({ phase: "belly-flop" }), snap({ phase: "flip" })],
      ["booster", snap({ phase: "landing-burn", engines: running(3) }), snap({ phase: "landing-burn", engines: running(2) })],
      ["booster", snap({ position: [0, 0, 5001], mach: 1.2, heat_flux: 0 }), snap({ position: [0, 0, 4999], mach: 0.9, heat_flux: 70_000, arms: true, guide: "brake" })],
      ["earth", snap({}), snap({ outcome: "crashed", reason: "too fast" })],
      ["mars", snap({}), snap({ outcome: "landed" })],
    ];
    for (const [kind, a, b] of flights) for (const c of callouts(kind, a, b)) expect(all.has(c.text), c.text).toBe(true);
    const ids = allPhrases().map(clipId);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids.every((id) => /^[a-z0-9-]+$/.test(id))).toBe(true);
  });

  it("prefers a natural English voice, then any English one", () => {
    const voices = [
      { name: "Microsoft David", lang: "en-US" },
      { name: "Google Deutsch", lang: "de-DE" },
      { name: "Microsoft Aria Online (Natural) - English (United States)", lang: "en-US" },
    ];
    expect(pickVoice(voices)).toBe(2);
    expect(pickVoice(voices.slice(0, 2))).toBe(0);
    expect(pickVoice([])).toBe(-1);
  });
});

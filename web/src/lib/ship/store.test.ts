import { beforeEach, describe, expect, it } from "vitest";
import { clampStart, configOf, DEFAULT_OPTIONS, DEFAULT_VIEW, kindOf, phasesOf, PRESETS, START_LIMITS, useShip, verdict } from "./store";

const reset = () =>
  useShip.setState({
    parked: false,
    flight: null,
    vehicle: "booster",
    planet: "earth",
    scenario: "booster",
    mode: "editing",
    start: PRESETS.booster[0].start,
    preset: 0,
    options: { ...DEFAULT_OPTIONS },
    view: { ...DEFAULT_VIEW },
  });

describe("ship store", () => {
  beforeEach(reset);

  it("maps vehicle and planet to a flight kind", () => {
    expect(kindOf("ship", "earth")).toBe("earth");
    expect(kindOf("ship", "mars")).toBe("mars");
    expect(kindOf("booster", "earth")).toBe("booster");
    expect(kindOf("booster", "mars")).toBe("booster");
  });

  it("Mars is ship-only: picking Mars switches to the ship, picking the booster returns to Earth", () => {
    const s = useShip.getState();
    s.setPlanet("mars");
    expect(useShip.getState()).toMatchObject({ vehicle: "ship", planet: "mars", scenario: "mars" });
    useShip.getState().setVehicle("booster");
    expect(useShip.getState()).toMatchObject({ vehicle: "booster", planet: "earth", scenario: "booster" });
  });

  it("every preset is inside its kind's start box", () => {
    for (const kind of ["earth", "mars", "booster"] as const) {
      for (const p of PRESETS[kind]) expect(clampStart(kind, p.start)).toEqual(p.start);
    }
  });

  it("keeps starts in the box; the ship stays in the x-z plane, the booster may start off to the side", () => {
    const ship = clampStart("earth", { position: [9e9, 5, -3], velocity: [0, 7, 0] });
    expect(ship.position).toEqual([START_LIMITS.earth.x[1], 0, START_LIMITS.earth.z[0]]);
    expect(ship.velocity[1]).toBe(0);
    expect(clampStart("booster", { position: [0, 300, 9000], velocity: [0, 10, -300] }).position[1]).toBe(300);
  });

  it("changing vehicle keeps the weather and drops engine failures and vehicle numbers", () => {
    const s = useShip.getState();
    s.setOptions({ windSpeed: 12, gimbal: 8, noise: "high" });
    s.toggleFailed(4);
    s.setVehicle("ship");
    const o = useShip.getState().options;
    expect(o).toMatchObject({ windSpeed: 12, noise: "high", gimbal: null, failed: [] });
  });

  it("toggles failed engines, sorted", () => {
    const s = useShip.getState();
    s.toggleFailed(7);
    s.toggleFailed(2);
    expect(useShip.getState().options.failed).toEqual([2, 7]);
    useShip.getState().toggleFailed(7);
    expect(useShip.getState().options.failed).toEqual([2]);
  });

  it("builds the Rust config: wind as a vector, propellant in kg, only the changed physics", () => {
    const s = useShip.getState();
    s.setOptions({ windSpeed: 10, windDir: 90, propellant: 100, isp: 340 });
    const c = configOf(useShip.getState());
    expect(c.scenario).toBe("booster");
    expect(c.wind![0]).toBeCloseTo(0);
    expect(c.wind![1]).toBeCloseTo(10);
    expect(c.propellant).toBe(100_000);
    expect(c.tuning).toEqual({ density_scale: 1, isp: 340 });
  });

  it("edits only while editing; pause and analysis only in a flight", () => {
    const s = useShip.getState();
    s.setView({ paused: true, analysis: true });
    expect(useShip.getState().view).toMatchObject({ paused: false, analysis: false });
    s.fly();
    useShip.getState().beginFlight();
    const before = useShip.getState().start;
    useShip.getState().setStart({ position: [0, 0, 6000], velocity: [0, 0, -300] });
    expect(useShip.getState().start).toBe(before);
    useShip.getState().setVehicle("ship");
    expect(useShip.getState().vehicle).toBe("booster");
    useShip.getState().setView({ paused: true, analysis: true });
    expect(useShip.getState().view).toMatchObject({ paused: true, analysis: true });
    useShip.getState().reset();
    expect(useShip.getState().view).toMatchObject({ paused: false, analysis: false });
  });

  it("flying starts with the opening sequence: the stack on Earth, a card for Mars", () => {
    useShip.getState().fly();
    expect(useShip.getState()).toMatchObject({ mode: "launch", intro: "stack" });
    useShip.getState().beginFlight();
    expect(useShip.getState().mode).toBe("flying");
    useShip.getState().reset();
    useShip.getState().setPlanet("mars");
    useShip.getState().fly();
    expect(useShip.getState()).toMatchObject({ mode: "launch", intro: "card" });
  });

  it("after a booster catch the ship's return can follow, with the booster parked", () => {
    const s = useShip.getState();
    s.continueWithShip();
    expect(useShip.getState().vehicle).toBe("booster");
    s.setOptions({ windSpeed: 9 });
    useShip.setState({ mode: "done", flight: { phase: "done", engines: [], guide: "none", legEngines: 0, outcome: "caught", reason: null } });
    useShip.getState().continueWithShip();
    expect(useShip.getState()).toMatchObject({ vehicle: "ship", scenario: "earth", mode: "launch", intro: "card", parked: true });
    expect(useShip.getState().options.windSpeed).toBe(9);
    useShip.getState().reset();
    expect(useShip.getState().parked).toBe(false);
  });

  it("names the phases and the outcome per vehicle", () => {
    expect(phasesOf("booster")[0]).toBe("descent");
    expect(phasesOf("earth")[1]).toBe("flip");
    expect(verdict("booster", "caught", null)).toBe("Booster caught by the tower");
    expect(verdict("mars", "landed", null)).toBe("Landed on Mars");
    expect(verdict("earth", "crashed", "hit the tower")).toBe("Lost: hit the tower");
  });
});

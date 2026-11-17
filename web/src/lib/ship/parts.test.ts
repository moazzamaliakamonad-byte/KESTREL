import { describe, expect, it } from "vitest";
import { airflow, partOf, partSheet, partsOf, pinsAboveArms } from "./parts";
import type { EngineState, ScenarioInfo, ShipSnapshot } from "./types";

const info = (vehicle: "ship" | "booster"): ScenarioInfo => ({
  scenario: vehicle === "ship" ? "earth" : "booster",
  vehicle,
  gravity: 9.80665,
  start: { position: [0, 0, 5000], velocity: [0, 0, -80] },
  propellant: 120e3,
  aim: [120, 0],
  tuning: { density_scale: 1, gimbal_max_deg: 15, throttle_min: 0.4, isp: 327, ignition_time: 1 },
  ship: {
    length: 72.3,
    radius: 4.5,
    cm_height: 26,
    pin_height: 62,
    engine_max: 2.45e6,
    engines: 13,
    final_engines: 3,
    isp: 327,
    throttle_min: 0.4,
    fin_station: 34,
    fin_max_deg: 20,
    fore_station: 20,
    aft_station: -18,
    flap_y: 5,
    gimbal_max_deg: 15,
  },
  model: {
    c_a: 0.8,
    c_n: 1.2,
    c_n_alpha: 2,
    cp_offset: 2,
    c_flap: 1.3,
    fore_area: 16,
    aft_area: 28,
    fin_area: 12,
    fin_c_delta: 2,
    fin_radius: 6.5,
    rcs_thrust: 50e3,
    rcs_fore: 30,
    rcs_aft: -20,
    inertia_per_kg: [440, 440, 10],
    gimbal_rate_deg: 20,
    ignition_time: 1,
  },
  limits: { catch_down: 2, catch_side: 1, catch_tilt_deg: 5, catch_rate_deg: 5, arms_close_above: 8 },
  start_mass: 395e3,
  dry_mass: 275e3,
  gate: [14, 0, 71],
  rest_cm_z: 59,
  catch: { arm_z: 95, mast_x: -14, mast_half: 4.5, tower_top: 145, capture_radius: 4 },
});

function snap(patch: Partial<ShipSnapshot>): ShipSnapshot {
  return {
    t: 30,
    vehicle: "booster",
    phase: "landing-burn",
    position: [3, 4, 100],
    velocity: [0, 0, -50],
    quat: [1, 0, 0, 0],
    omega: [0, 0, 0],
    tilt_deg: 0,
    mass: 300e3,
    dry_mass: 275e3,
    propellant: 25e3,
    engines: Array.from({ length: 13 }, (_, i): EngineState => (i < 3 ? "running" : "shutdown")),
    thrust: [0, 0, 4.9e6],
    thrust_inertial: [0, 0, 4.9e6],
    thrust_mag: 4.9e6,
    thrust_limits: [2.94e6, 7.35e6],
    gimbal_deg: 0,
    flaps: [0.1, -0.1, 0, 0],
    rcs: [false, false, false, false, false, false],
    q_dyn: 1531,
    mach: 0.15,
    accel_g: 1.67,
    aero_force: [0, 0, 1e5],
    weight: 300e3 * 9.80665,
    density: 1.225,
    heat_flux: 100,
    arms: false,
    wind: [0, 0, 0],
    guide: "plan",
    leg_engines: 3,
    ignition_z: 2400,
    grabbed: false,
    outcome: null,
    reason: null,
    end_velocity: null,
    engine_out_margin: null,
    entry: null,
    entry_summary: null,
    ...patch,
  };
}

const value = (id: string, label: string, s: ShipSnapshot, i: ScenarioInfo) => partSheet(id, s, i).calcs.find((c) => c.label === label)!;

describe("vehicle parts", () => {
  it("lists parts with unique ids, and the booster has fins where the ship has flaps", () => {
    for (const v of ["ship", "booster"] as const) {
      const ids = partsOf(v).map((p) => p.id);
      expect(new Set(ids).size).toBe(ids.length);
      expect(ids).toContain("engines");
    }
    expect(partOf("booster", "fin2")?.name).toMatch(/Grid fin/);
    expect(partOf("booster", "flap0")).toBeNull();
    expect(partOf("ship", "flap3")?.name).toMatch(/Aft flap/);
    expect(partOf("ship", null)).toBeNull();
  });

  it("splits the airflow into along and across the body", () => {
    // Upright and falling: all of it is along the axis.
    expect(airflow(snap({}))).toMatchObject({ axial: -50, normal: 0, speed: 50, alphaDeg: 0 });
    // A crosswind adds a part across.
    const a = airflow(snap({ wind: [10, 0, 0] }));
    expect(a.normal).toBeCloseTo(10);
    expect(a.alphaDeg).toBeCloseTo((Math.atan2(10, 50) * 180) / Math.PI);
  });

  it("the engine sheet uses the rocket equation's numbers", () => {
    const s = snap({});
    const i = info("booster");
    // ṁ = T / (Isp g0) = 4.9e6 / (327 × 9.80665) ≈ 1528 kg/s.
    expect(value("engines", "Propellant flow", s, i).value).toBe("1528 kg/s");
    // Each of three engines: 1.63 MN, two thirds of 2.45 MN.
    expect(value("engines", "Each engine", s, i).value).toBe("1.63 MN · 67 % throttle");
    expect(value("engines", "Thrust", s, i).share).toBeCloseTo(4.9 / 7.35);
    // A 5° gimbal: side force T sin δ, torque × the 26 m arm.
    const g = snap({ gimbal_deg: 5 });
    expect(value("engines", "Steering force", g, i).value).toBe(`${((4.9e6 * Math.sin((5 * Math.PI) / 180)) / 1e3).toFixed(1)} kN`);
    expect(value("engines", "Gimbal angle", g, i).share).toBeCloseTo(1 / 3);
  });

  it("a grid fin's force is the model's k·δ", () => {
    const s = snap({});
    const i = info("booster");
    // k = ½ ρ w_a² A C_δ = 0.5 × 1.225 × 2500 × 12 × 2 = 36.75 kN/rad; δ = 0.1 rad.
    expect(value("fin0", "Force per radian", s, i).value).toBe("36.8 kN / rad");
    expect(value("fin0", "Force now", s, i).value).toBe("3.68 kN");
    expect(value("fin1", "Force now", s, i).value).toBe("-3.68 kN");
  });

  it("a flap's drag follows its exposure and the air across the hull", () => {
    const i = info("ship");
    // Belly first at 80 m/s: quaternion turning the nose to the horizon.
    const s = snap({ vehicle: "ship", quat: [Math.SQRT1_2, 0, Math.SQRT1_2, 0], velocity: [0, 0, -80], flaps: [0.5, 1, 0, 0.25] });
    expect(airflow(s).normal).toBeCloseTo(80);
    // F = ½ ρ C_f A e w_n² = 0.5 × 1.225 × 1.3 × 16 × 0.5 × 6400 = 40.8 kN.
    expect(value("flap0", "Drag on this flap", s, i).value).toMatch(/^40\.8 kN of 81\.5 kN/);
    expect(value("flap2", "Exposure", s, i).share).toBe(0);
  });

  it("the catch sheet measures the pins against the arms and the limits", () => {
    const i = info("booster");
    const s = snap({ position: [3, 4, 61], velocity: [0.5, 0, -1] });
    // Pins are 36 m above the centre of mass: 61 + 36 − 95 = 2 m above the arms.
    expect(pinsAboveArms(s, i)).toBeCloseTo(2);
    expect(value("pins", "From the catch axis", s, i).value).toBe("5.00 m");
    expect(value("pins", "From the catch axis", s, i).share).toBe(1);
    expect(value("pins", "Sink rate", s, i).share).toBeCloseTo(0.5);
    expect(value("pins", "Sideways speed", s, i).share).toBeCloseTo(0.5);
  });

  it("an unknown or missing part gives the guidance sheet", () => {
    expect(partSheet("nonsense", snap({}), info("booster")).title).toBe("Guidance and attitude");
  });
});

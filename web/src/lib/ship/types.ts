import type { PQuat, Vec3 } from "./frames";

/** A flight kind as Rust names it: the ship's return ("earth") or the booster's. Both end on the tower. */
export type ScenarioKind = "earth" | "booster";
export type Vehicle = "ship" | "booster";
export type ShipPhase = "entry" | "belly-flop" | "flip" | "descent" | "startup" | "landing-burn" | "terminal" | "done";

/** The entry from orbit now (`entry` in the snapshot; Rust `entry_json`). SI units, angles in degrees. */
export interface EntrySnapshot {
  t: number;
  altitude: number;
  speed: number;
  mach: number;
  q_dyn: number;
  density: number;
  gamma_deg: number;
  /** Stagnation-point heating, W/m². */
  heat_flux: number;
  /** Tile temperature (radiative equilibrium), K, and its limit. */
  tile: number;
  tile_limit: number;
  /** Seconds above the limit so far, and how many the shield survives. */
  overheat: number;
  overheat_limit: number;
  /** J/m². */
  heat_load: number;
  g: number;
  lift_to_drag: number;
  bank_deg: number;
  bank_cmd_deg: number;
  reversals: number;
  heat_limited: boolean;
  heading_error_deg: number;
  /** Where guidance predicts the entry ends, past (+) or short of (−) its aim, m. */
  predicted_overshoot: number;
  predicted_time: number;
  /** Along the surface, m. */
  to_site: number;
  to_aim: number;
  lat: number;
  lon: number;
  site: [number, number];
  handover_altitude: number;
  /** Earth-centred, m and m/s. */
  position: Vec3;
  velocity: Vec3;
  peak_tile: number;
  peak_g: number;
}

/** What the entry did (after the hand-over). */
export interface EntrySummary {
  duration: number;
  peak_heat_flux: number;
  peak_tile: number;
  heat_load: number;
  peak_g: number;
  reversals: number;
  miss: number;
}
export type EngineState = "off" | "igniting" | "running" | "shutdown" | "failed";
export type Outcome = "caught" | "crashed";

/** `ShipFlight.snapshot()` (crates/wasm/src/ship_js.rs). SI units. */
export interface ShipSnapshot {
  t: number;
  vehicle: Vehicle;
  phase: ShipPhase;
  position: Vec3;
  velocity: Vec3;
  quat: PQuat;
  omega: Vec3;
  tilt_deg: number;
  mass: number;
  dry_mass: number;
  propellant: number;
  /** One per landing engine (ship 3; booster 13, the centre three first). */
  engines: EngineState[];
  /** Body axes, N. */
  thrust: Vec3;
  thrust_inertial: Vec3;
  thrust_mag: number;
  thrust_limits: [number, number];
  gimbal_deg: number;
  /** Ship: flap exposures 0–1. Booster: grid-fin deflections, rad (three used). */
  flaps: [number, number, number, number];
  rcs: boolean[];
  q_dyn: number;
  mach: number;
  /** Thrust over mass, in g. */
  accel_g: number;
  /** Inertial, N. */
  aero_force: Vec3;
  weight: number;
  density: number;
  /** Stagnation-point heating estimate, W/m² (not part of the dynamics). */
  heat_flux: number;
  arms: boolean;
  wind: Vec3;
  guide: "plan" | "none" | "brake";
  leg_engines: number;
  ignition_z: number;
  grabbed: boolean;
  outcome: Outcome | null;
  reason: string | null;
  end_velocity: Vec3 | null;
  /** The entry from orbit while it lasts, else null. */
  entry: EntrySnapshot | null;
  entry_summary: EntrySummary | null;
  /**
   * Engine-out envelope: how much slower than the survivable limit the ship
   * is sinking, m/s (negative: losing the running engine now could not be
   * recovered by relighting a spare). Null when it does not apply.
   */
  engine_out_margin: number | null;
}

/** Physics options; a missing field keeps the scenario's value. */
export interface Tuning {
  density_scale?: number;
  gimbal_max_deg?: number;
  throttle_min?: number;
  isp?: number;
  ignition_time?: number;
}

/** `ship_scenario(kind)`. */
export interface ScenarioInfo {
  scenario: ScenarioKind;
  vehicle: Vehicle;
  gravity: number;
  start: { position: Vec3; velocity: Vec3 };
  /** Landing propellant, kg. */
  propellant: number;
  aim: [number, number];
  tuning: Required<Tuning>;
  ship: {
    length: number;
    radius: number;
    cm_height: number;
    pin_height: number;
    engine_max: number;
    engines: number;
    final_engines: number;
    isp: number;
    throttle_min: number;
    fin_station: number | null;
    fin_max_deg: number | null;
    fore_station: number;
    aft_station: number;
    flap_y: number;
    gimbal_max_deg: number;
  };
  /** The model's coefficients (for the analysis view's per-part formulas). */
  model: {
    c_a: number;
    c_n: number;
    c_n_alpha: number;
    cp_offset: number;
    c_flap: number;
    fore_area: number;
    aft_area: number;
    fin_area: number | null;
    fin_c_delta: number | null;
    fin_radius: number | null;
    rcs_thrust: number;
    rcs_fore: number;
    rcs_aft: number;
    inertia_per_kg: Vec3;
    gimbal_rate_deg: number;
    ignition_time: number;
  };
  /** What the arms accept at contact. */
  limits: { catch_down: number; catch_side: number; catch_tilt_deg: number; catch_rate_deg: number; arms_close_above: number };
  start_mass: number;
  dry_mass: number;
  gate: Vec3;
  rest_cm_z: number;
  catch: { arm_z: number; mast_x: number; mast_half: number; tower_top: number; capture_radius: number } | null;
}

/** What `ShipFlight.create` and `preview_ship` take. */
export interface FlightConfig {
  scenario: ScenarioKind;
  position: Vec3;
  velocity: Vec3;
  seed?: number;
  lag?: number;
  noise?: "off" | "low" | "high";
  tuning?: Tuning;
  /** kg */
  propellant?: number;
  /** m/s (x, y) */
  wind?: [number, number];
  gust_speed?: number;
  failed?: number[];
  /** Which convex solver the planner uses (read by the worker, not by the flight). */
  solver?: PlannerSolver;
  /** Start from orbit (the ship only). */
  entry?: { range_km: number; gamma_deg: number };
}

/** Clarabel (the default) or Kestrel's own allocation-free solver (D-041). */
export type PlannerSolver = "clarabel" | "kestrel";

/** `ShipFlight.plan_json()`. */
export interface PlanDraw {
  t0: number;
  final_time: number;
  position: Vec3[];
  quat: PQuat[];
  thrust: number[];
}

/** `preview_ship(config)`. */
export interface Preview {
  ok: boolean;
  path: Vec3[];
  phases: { t: number; phase: ShipPhase; position?: Vec3 }[];
  outcome: Outcome | null;
  reason: string | null;
  t: number;
  propellant?: number;
  /** From orbit: what the entry did, and its ground track every 10 s ([lat, lon, altitude]). */
  entry?: EntrySummary | null;
  entry_track?: [number, number, number][];
}

import type { PQuat, Vec3 } from "./frames";

export type ScenarioKind = "earth" | "mars";
export type ShipPhase = "belly-flop" | "flip" | "landing-burn" | "terminal" | "done";
export type EngineState = "off" | "igniting" | "running" | "shutdown" | "failed";
export type Outcome = "caught" | "landed" | "crashed";

/** `ShipFlight.snapshot()` (crates/wasm/src/ship_js.rs). */
export interface ShipSnapshot {
  t: number;
  phase: ShipPhase;
  position: Vec3;
  velocity: Vec3;
  quat: PQuat;
  omega: Vec3;
  tilt_deg: number;
  mass: number;
  propellant: number;
  engines: [EngineState, EngineState, EngineState];
  thrust: Vec3;
  thrust_mag: number;
  gimbal_deg: number;
  flaps: [number, number, number, number];
  rcs: boolean[];
  q_dyn: number;
  legs: boolean;
  arms: boolean;
  wind: Vec3;
  guide: "plan" | "none" | "brake";
  leg_engines: number;
  ignition_z: number;
  grabbed: boolean;
  outcome: Outcome | null;
  reason: string | null;
  end_velocity: Vec3 | null;
}

/** `ship_scenario(kind)`. */
export interface ScenarioInfo {
  scenario: ScenarioKind;
  planet: ScenarioKind;
  gravity: number;
  start: { position: Vec3; velocity: Vec3 };
  ship: {
    length: number;
    radius: number;
    cm_height: number;
    pin_height: number;
    leg_drop: number;
    leg_radius: number;
    engine_max: number;
    engines: number;
    fore_station: number;
    aft_station: number;
    flap_y: number;
    gimbal_max_deg: number;
  };
  start_mass: number;
  dry_mass: number;
  gate: Vec3;
  rest_cm_z: number;
  catch: { arm_z: number; mast_x: number; mast_half: number; tower_top: number; capture_radius: number } | null;
}

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
}

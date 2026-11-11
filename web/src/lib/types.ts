/** Shapes shared with the Rust bridge (crates/wasm). SI units, z up. */

export type Vec3 = [number, number, number];

export interface Vehicle {
  wet_mass: number;
  dry_mass: number;
  isp: number;
  min_thrust: number;
  max_thrust: number;
  gravity: number;
  /** Minimum elevation angle seen from the pad, radians. */
  glide_slope: number;
  /** Largest angle between thrust and vertical, radians. */
  max_tilt: number;
}

/** Nodes: t, position, velocity, mass (N + 1). Intervals: thrust (N). */
export interface Trajectory {
  t: number[];
  position: Vec3[];
  velocity: Vec3[];
  mass: number[];
  thrust: Vec3[];
  final_time: number;
  fuel_used: number;
}

export type ErrorKind =
  | "BadRequest"
  | "InvalidParams"
  | "InvalidProblem"
  | "Infeasible"
  | "NotLossless"
  | "SolverFailed";

export type PlanSuccess = { ok: true; trajectory: Trajectory; vehicle: Vehicle };
export type PlanFailure = { ok: false; error: { kind: ErrorKind; message: string } };
export type PlanResponse = PlanSuccess | PlanFailure;

export interface PlanRequest {
  position: Vec3;
  velocity: Vec3;
  steps?: number;
  vehicle?: Vehicle;
}

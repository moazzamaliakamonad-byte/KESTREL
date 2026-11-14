import { PAD_CAM_SECONDS } from "./camera";
import type { Sample } from "./playback";
import type { FlightView, GuideMode, Outcome } from "./state";
import type { Vec3 } from "./types";

/** `Flight.snapshot()` (crates/wasm/src/flight_js.rs). SI, z up. */
export interface Snapshot {
  t: number;
  /** True state. */
  position: Vec3;
  velocity: Vec3;
  mass: number;
  /** Thrust the engines produce now, N. */
  thrust: Vec3;
  /** Thrust the controller asks for, N. */
  command: Vec3;
  /** Where the sensors say the vehicle is. */
  nav_position: Vec3;
  engines: number;
  /** Available thrust band with the engines that still work, N. */
  min_thrust: number;
  max_thrust: number;
  lag: number;
  landed: boolean;
  mode: GuideMode;
  miss: number | null;
  plan_remaining: number | null;
  outcome: Outcome | null;
}

/** The scene's per-frame sample. */
export function sampleOf(s: Snapshot): Sample {
  const [tx, ty, tz] = s.thrust;
  return {
    t: s.t,
    position: s.position,
    velocity: s.velocity,
    mass: s.mass,
    thrust: s.thrust,
    thrustMag: Math.hypot(tx, ty, tz),
    done: s.landed,
  };
}

/**
 * What changed for React since `view`, or null. Small changes are ignored
 * (miss under a metre) so the store updates only a few times a flight.
 */
export function viewPatch(s: Snapshot, view: FlightView): Partial<FlightView> | null {
  const patch: Partial<FlightView> = {};
  if (s.mode !== view.mode) patch.mode = s.mode;
  const missMoved =
    (s.miss === null) !== (view.miss === null) ||
    (s.miss !== null && view.miss !== null && Math.abs(s.miss - view.miss) >= 1);
  if (missMoved) patch.miss = s.miss;
  if (s.engines !== view.engines) patch.engines = s.engines;
  // Once near touchdown, stay there (the plan ends; remaining becomes null).
  const near = view.nearTouchdown || (s.plan_remaining !== null && s.plan_remaining < PAD_CAM_SECONDS);
  if (near !== view.nearTouchdown) patch.nearTouchdown = near;
  return Object.keys(patch).length > 0 ? patch : null;
}

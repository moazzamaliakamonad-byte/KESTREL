import type { Vec3 } from "./frames";
import type { PlanDraw, ShipSnapshot } from "./types";

/** One point of the flight's history (for the graphs). SI units. */
export interface Sample {
  t: number;
  altitude: number;
  speed: number;
  thrust: number;
  tilt: number;
  q: number;
  propellant: number;
  mach: number;
  heat: number;
  g: number;
}

/**
 * Per-frame flight state, written by the runtime and read by the scene and
 * HUD inside animation callbacks (no React re-render at 60 fps).
 */
export const shipLive: {
  snapshot: ShipSnapshot | null;
  trail: Vec3[];
  plan: PlanDraw | null;
  /** Bumped when `plan` changes. */
  planVersion: number;
  solveMs: number[];
  replans: number;
  history: Sample[];
  caption: { text: string; at: number } | null;
  /** The flight has begun but waits (behind the title card). */
  hold: boolean;
  /** Mission clock of the opening sequence, s (negative before liftoff), or null. */
  launchClock: number | null;
} = {
  snapshot: null,
  trail: [],
  plan: null,
  planVersion: 0,
  solveMs: [],
  replans: 0,
  history: [],
  caption: null,
  hold: false,
  launchClock: null,
};

/** Longest history kept; when full, every other sample is dropped. */
export const HISTORY_MAX = 1200;

export function sampleOf(s: ShipSnapshot): Sample {
  return {
    t: s.t,
    altitude: s.position[2],
    speed: Math.hypot(...s.velocity),
    thrust: s.thrust_mag,
    tilt: s.tilt_deg,
    q: s.q_dyn,
    propellant: s.propellant,
    mach: s.mach,
    heat: s.heat_flux,
    g: s.accel_g,
  };
}

export function pushSample(history: Sample[], sample: Sample): Sample[] {
  history.push(sample);
  if (history.length <= HISTORY_MAX) return history;
  return history.filter((_, i) => i % 2 === 0);
}

import type { Snapshot } from "./flightSnapshot";
import type { Sample } from "./playback";
import type { Vec3 } from "./types";

/**
 * Per-frame state, written by the flight driver and read by scene and HUD
 * components inside animation callbacks. Kept outside React state so 60 fps
 * animation doesn't re-render the component tree.
 */
export const live: {
  /** The booster now (true state while flying, the start while editing). */
  sample: Sample | null;
  maxThrust: number;
  /** The full closed-loop snapshot while flying, else null. */
  snapshot: Snapshot | null;
  /** Where the booster has actually been this flight. */
  trail: { t: number; position: Vec3 }[];
  /** Bumped on every re-plan answer (charts redraw on change). */
  solveVersion: number;
} = {
  sample: null,
  maxThrust: 1,
  snapshot: null,
  trail: [],
  solveVersion: 0,
};

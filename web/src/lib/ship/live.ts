import type { Vec3 } from "./frames";
import type { PlanDraw, ShipSnapshot } from "./types";

/**
 * Per-frame ship state, written by the runtime and read by the scene and
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
} = {
  snapshot: null,
  trail: [],
  plan: null,
  planVersion: 0,
  solveMs: [],
  replans: 0,
};

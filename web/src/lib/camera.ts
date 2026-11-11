import type { Phase } from "./state";

export type CameraMode = "orbit" | "chase" | "pad";

/** Seconds before touchdown when the camera cuts to the pad. */
export const PAD_CAM_SECONDS = 8;

/**
 * Camera the page picks on its own: free orbit while editing, chase during
 * flight, the pad camera for the final seconds, orbit again after touchdown.
 * With reduced motion requested, never move the camera automatically.
 */
export function autoMode(
  phase: Phase,
  t: number,
  finalTime: number,
  reducedMotion: boolean,
): CameraMode {
  if (reducedMotion || phase !== "flying") return "orbit";
  return t > finalTime - PAD_CAM_SECONDS ? "pad" : "chase";
}

import type { Sample } from "./playback";

/**
 * The booster's state for the current frame, written once per frame by the
 * flight driver and read by scene components inside `useFrame`. Kept outside
 * React state so 60 fps animation doesn't re-render the component tree.
 */
export const live: { sample: Sample | null; maxThrust: number } = {
  sample: null,
  maxThrust: 1,
};

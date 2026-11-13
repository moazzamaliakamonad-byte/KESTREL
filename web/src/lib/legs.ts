/** Legs start unfolding here (m above the pad)… */
const DEPLOY_START = 100;
/** …and are fully out by here (2 s at a typical 25 m/s descent). */
const DEPLOY_END = DEPLOY_START - 2 * 25;

/** Landing-leg deployment: 0 stowed … 1 fully out, eased. */
export function legDeploy(altitude: number): number {
  const t = Math.min(Math.max((DEPLOY_START - altitude) / (DEPLOY_START - DEPLOY_END), 0), 1);
  return t * t * (3 - 2 * t);
}

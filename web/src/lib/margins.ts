import type { Vec3 } from "./types";

const DEG = 180 / Math.PI;

/** Elevation seen from the pad minus the glide-slope angle, degrees. Negative = outside the cone. */
export function glideMargin(position: Vec3, glideSlope: number): number {
  const [x, y, z] = position;
  return (Math.atan2(z, Math.hypot(x, y)) - glideSlope) * DEG;
}

/** Tilt limit minus the thrust's angle from vertical, degrees; null with the engine off. */
export function tiltMargin(thrust: Vec3, maxTilt: number): number | null {
  const [x, y, z] = thrust;
  if (Math.hypot(x, y, z) < 1) return null;
  return (maxTilt - Math.atan2(Math.hypot(x, y), z)) * DEG;
}

/** Thrust magnitude as a fraction of the available maximum. */
export function thrustShare(thrust: Vec3, maxThrust: number): number {
  return maxThrust > 0 ? Math.hypot(...thrust) / maxThrust : 0;
}

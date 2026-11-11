/** Limits on where the visitor can put the booster and how fast it moves. */
export const LIMITS = {
  maxAbsX: 4000,
  maxZ: 3000,
  /** Stay this factor above the glide-slope cone… */
  coneMargin: 1.05,
  /** …plus this many metres. */
  coneOffset: 20,
  maxSpeed: 150,
  /** The velocity arrow's tip sits where the booster will be after this long. */
  handleSeconds: 2,
} as const;

const finiteOr0 = (v: number) => (Number.isFinite(v) ? v : 0);

/** Clamp a start position (x–z plane, metres) into the allowed region. */
export function clampPosition(x: number, z: number, glideSlope: number): { x: number; z: number } {
  const slope = LIMITS.coneMargin * Math.tan(glideSlope);
  // With a steep cone, far-out x may need more height than maxZ allows.
  const reachableX = slope > 0 ? (LIMITS.maxZ - LIMITS.coneOffset) / slope : Infinity;
  const maxX = Math.min(LIMITS.maxAbsX, reachableX);
  const cx = Math.min(Math.max(finiteOr0(x), -maxX), maxX);
  const zMin = slope * Math.abs(cx) + LIMITS.coneOffset;
  const cz = Math.min(Math.max(finiteOr0(z), zMin), LIMITS.maxZ);
  return { x: cx, z: cz };
}

/** Cap the start speed, keeping its direction. */
export function clampVelocity(vx: number, vz: number): { vx: number; vz: number } {
  const x = finiteOr0(vx);
  const z = finiteOr0(vz);
  const speed = Math.hypot(x, z);
  if (speed <= LIMITS.maxSpeed) return { vx: x, vz: z };
  const k = LIMITS.maxSpeed / speed;
  return { vx: x * k, vz: z * k };
}

/** Velocity implied by dragging the arrow tip to `tip`. */
export function velocityFromHandle(
  pos: { x: number; z: number },
  tip: { x: number; z: number },
): { vx: number; vz: number } {
  return clampVelocity(
    (tip.x - pos.x) / LIMITS.handleSeconds,
    (tip.z - pos.z) / LIMITS.handleSeconds,
  );
}

/**
 * Physics ↔ scene frames for the ship. Physics: z up, metres, quaternion
 * scalar-first [w, x, y, z] body→inertial (Rust `model::quat`). Scene
 * (three.js): y up, `toScene([x, y, z]) = [x, z, −y]`.
 *
 * The ship model is built with its nose along local +Y (body z), its belly
 * (heat shield) along local +X (body x) and body +y along local −Z, the
 * same map, so one quaternion conversion places it.
 */

export type Vec3 = [number, number, number];
/** Physics quaternion, scalar first. */
export type PQuat = [number, number, number, number];
/** three.js quaternion order [x, y, z, w], scene frame. */
export type SQuat = [number, number, number, number];

export function toScene(p: Vec3): Vec3 {
  return [p[0], p[2], -p[1]];
}

/** Body→inertial physics quaternion → scene quaternion for the model. */
export function quatToScene(q: PQuat): SQuat {
  const [w, x, y, z] = q;
  return [x, z, -y, w];
}

/** Rotate a physics vector by a physics quaternion (for tests and HUD). */
export function rotate(q: PQuat, v: Vec3): Vec3 {
  const [w, x, y, z] = q;
  // v' = v + 2w(u×v) + 2u×(u×v), u = (x, y, z)
  const u: Vec3 = [x, y, z];
  const c1 = cross(u, v);
  const c2 = cross(u, c1);
  return [v[0] + 2 * (w * c1[0] + c2[0]), v[1] + 2 * (w * c1[1] + c2[1]), v[2] + 2 * (w * c1[2] + c2[2])];
}

function cross(a: Vec3, b: Vec3): Vec3 {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}

/** Rotate a scene vector by a scene quaternion. */
export function rotateScene(q: SQuat, v: Vec3): Vec3 {
  const [x, y, z, w] = q;
  return rotate([w, x, y, z], v);
}

/** Shortest-arc physics quaternion taking +z onto the unit vector `d`. */
export function quatFromZ(d: Vec3): PQuat {
  const n = Math.hypot(d[0], d[1], d[2]);
  if (n < 1e-9) return [1, 0, 0, 0];
  const [x, y, z] = [d[0] / n, d[1] / n, d[2] / n];
  // axis = ẑ × d = (−y, x, 0); w = 1 + ẑ·d (then normalize).
  const w = 1 + z;
  if (w < 1e-9) return [0, 0, 1, 0];
  const m = Math.hypot(w, x, y);
  return [w / m, -y / m, x / m, 0];
}

/** Inverse rotation: an inertial vector into body axes. */
export function rotateInv(q: PQuat, v: Vec3): Vec3 {
  return rotate([q[0], -q[1], -q[2], -q[3]], v);
}

/** The attitude a vehicle starts with (as Rust's `start_state`): the ship belly down with its nose toward −x, the booster engines first along its fall. */
export function startAttitude(vehicle: "ship" | "booster", velocity: Vec3): PQuat {
  if (vehicle === "ship") return [0, -Math.SQRT1_2, 0, Math.SQRT1_2];
  return quatFromZ([-velocity[0], -velocity[1], -velocity[2]]);
}

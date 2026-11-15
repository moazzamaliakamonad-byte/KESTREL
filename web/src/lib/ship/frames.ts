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

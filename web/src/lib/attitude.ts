import * as THREE from "three";
import { toScene } from "./playback";
import type { Vec3 } from "./types";

/** Quaternion as [x, y, z, w]. */
export type Quat = [number, number, number, number];

const UP = new THREE.Vector3(0, 1, 0);

/**
 * Orientation that points the booster's body axis (+Y in scene space) along
 * the thrust. With negligible thrust there is no direction to follow, so the
 * previous orientation is kept.
 */
export function targetQuaternion(thrust: Vec3, previous: Quat): Quat {
  if (Math.hypot(thrust[0], thrust[1], thrust[2]) < 1) return previous;
  const dir = new THREE.Vector3(...toScene(thrust)).normalize();
  const q = new THREE.Quaternion().setFromUnitVectors(UP, dir);
  return [q.x, q.y, q.z, q.w];
}

/** Ease `current` toward `target` with time constant `tau` seconds. */
export function smoothQuaternion(current: Quat, target: Quat, dt: number, tau = 0.15): Quat {
  const k = 1 - Math.exp(-Math.max(dt, 0) / tau);
  const q = new THREE.Quaternion(...current).slerp(new THREE.Quaternion(...target), k);
  return [q.x, q.y, q.z, q.w];
}

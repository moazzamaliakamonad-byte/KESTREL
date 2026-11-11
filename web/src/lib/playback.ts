import type { Trajectory, Vec3, Vehicle } from "./types";

/** The vehicle's state at one moment of a planned landing (SI, z up). */
export interface Sample {
  t: number;
  position: Vec3;
  velocity: Vec3;
  mass: number;
  thrust: Vec3;
  thrustMag: number;
  done: boolean;
}

/**
 * State at time `t` along a solved trajectory, exact between nodes.
 *
 * Within interval k the solver holds thrust *per kilogram* constant
 * (u = T_k / m_k), so acceleration is constant and the motion follows the
 * constant-acceleration formulas exactly. Log-mass falls linearly in time.
 * Times outside [0, final_time] (and NaN) are clamped.
 */
export function sampleAt(traj: Trajectory, vehicle: Vehicle, t: number): Sample {
  const n = traj.thrust.length;
  const tf = traj.final_time;
  const tc = Number.isFinite(t) ? Math.min(Math.max(t, 0), tf) : 0;

  if (tc >= tf) {
    return {
      t: tf,
      position: [...traj.position[n]] as Vec3,
      velocity: [...traj.velocity[n]] as Vec3,
      mass: traj.mass[n],
      thrust: [0, 0, 0],
      thrustMag: 0,
      done: true,
    };
  }

  const k = intervalAt(traj.t, tc, n);
  const tau = tc - traj.t[k];
  const dt = traj.t[k + 1] - traj.t[k];
  const mk = traj.mass[k];
  const u = traj.thrust[k].map((c) => c / mk) as Vec3;
  const a: Vec3 = [u[0], u[1], u[2] - vehicle.gravity];
  const r = traj.position[k];
  const v = traj.velocity[k];

  const position = [0, 1, 2].map((i) => r[i] + v[i] * tau + 0.5 * a[i] * tau * tau) as Vec3;
  const velocity = [0, 1, 2].map((i) => v[i] + a[i] * tau) as Vec3;
  const mass = mk * Math.pow(traj.mass[k + 1] / mk, tau / dt);
  const thrust = u.map((c) => c * mass) as Vec3;

  return {
    t: tc,
    position,
    velocity,
    mass,
    thrust,
    thrustMag: Math.hypot(thrust[0], thrust[1], thrust[2]),
    done: false,
  };
}

/** Index k of the interval [t[k], t[k+1]) containing `t` (binary search). */
function intervalAt(times: number[], t: number, n: number): number {
  let lo = 0;
  let hi = n - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (times[mid] <= t) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** Physics (x, y, z) with z up → three.js (x, y, z) with y up. */
export function toScene(p: Vec3): Vec3 {
  return [p[0], p[2], -p[1]];
}

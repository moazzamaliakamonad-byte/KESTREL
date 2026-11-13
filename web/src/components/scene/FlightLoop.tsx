"use client";

import { useFrame } from "@react-three/fiber";
import { live } from "@/lib/live";
import { sampleAt } from "@/lib/playback";
import { useApp } from "@/lib/store";

/** Longest frame step fed to the playback clock (s): no jumps after a stall. */
const MAX_DT = 0.1;

/**
 * Advances the flight clock each frame and publishes the booster's current
 * state to `live` for the scene: the planned trajectory while flying, the
 * editable start state otherwise.
 */
export function FlightLoop() {
  useFrame((_, dt) => {
    const s = useApp.getState();
    if (s.phase === "flying") s.dispatch({ type: "tick", dt: Math.min(dt, MAX_DT) });
    const now = useApp.getState();
    if (now.vehicle) live.maxThrust = now.vehicle.max_thrust;

    if (now.flight && (now.phase === "flying" || now.phase === "touchdown")) {
      const { trajectory, vehicle } = now.flight.response;
      live.sample = sampleAt(trajectory, vehicle, now.flight.t);
    } else {
      live.sample = {
        t: 0,
        position: now.start.position,
        velocity: now.start.velocity,
        mass: now.vehicle?.wet_mass ?? 0,
        thrust: [0, 0, 0],
        thrustMag: 0,
        done: false,
      };
    }
  });
  return null;
}

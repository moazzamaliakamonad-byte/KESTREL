"use client";

import { useFrame } from "@react-three/fiber";
import { frame } from "@/lib/flightRuntime";
import { live } from "@/lib/live";
import { useApp } from "@/lib/store";

/**
 * Drives the closed-loop flight each frame (see `flightRuntime.ts`) and
 * publishes the booster's state to `live`: the simulated vehicle while
 * flying, where it landed after touchdown, the editable start otherwise.
 */
export function FlightLoop() {
  useFrame((_, dt) => {
    const s = useApp.getState();
    if (s.vehicle) live.maxThrust = s.vehicle.max_thrust;
    if (s.phase === "flying" && frame(dt)) return;
    if (s.phase === "touchdown" && live.snapshot) return; // stay where it landed
    live.sample = {
      t: 0,
      position: s.start.position,
      velocity: s.start.velocity,
      mass: s.vehicle?.wet_mass ?? 0,
      thrust: [0, 0, 0],
      thrustMag: 0,
      done: false,
    };
  });
  return null;
}

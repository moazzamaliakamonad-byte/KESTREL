import type { Vehicle } from "./types";

/** One engine in the base plane, looking up at the engines: metres from the axis. */
export interface EngineSpot {
  x: number;
  z: number;
  /** A landing engine (it has a state and can be failed); its index is its place in the list. */
  landing: boolean;
  /** Bell exit radius, m. */
  r: number;
}

function ring(n: number, radius: number, phase: number, landing: boolean, r: number): EngineSpot[] {
  return Array.from({ length: n }, (_, i) => {
    const a = phase + (i * 2 * Math.PI) / n;
    return { x: radius * Math.sin(a), z: radius * Math.cos(a), landing, r };
  });
}

/**
 * Engine positions, landing engines first so their index matches the
 * simulator's. Booster: the centre three, the ring of ten, then the fixed
 * outer twenty (33 in all). Ship: three sea-level engines, then the three
 * vacuum engines (not used to land).
 */
export function engineLayout(vehicle: Vehicle): EngineSpot[] {
  if (vehicle === "booster") {
    return [...ring(3, 0.95, Math.PI / 2, true, 0.62), ...ring(10, 2.4, 0, true, 0.62), ...ring(20, 3.95, Math.PI / 20, false, 0.62)];
  }
  return [...ring(3, 1.55, 0, true, 0.65), ...ring(3, 3.15, Math.PI / 3, false, 1.2)];
}

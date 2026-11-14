/**
 * The closed loop on the main thread. Owns the WebAssembly `Flight`
 * (simulator + sensors + controller), steps it every frame, and asks the
 * worker for a re-plan every REPLAN_PERIOD of sim time (see `mpc.ts`).
 *
 * The same .wasm runs here and in the worker as two instances: the
 * simulator must advance every frame, guidance is slow and asynchronous.
 * Simulation only.
 */

import init, { Flight } from "@/wasm/kestrel_wasm";
import { sampleOf, viewPatch, type Snapshot } from "./flightSnapshot";
import { live } from "./live";
import { clampDt, createMpc } from "./mpc";
import type { NoiseLevel } from "./state";
import type { SolverClient } from "./solverClient";
import { useApp } from "./store";
import type { Trajectory, Vec3, Vehicle } from "./types";

export interface FlightConfig {
  position: Vec3;
  velocity: Vec3;
  vehicle: Vehicle;
  seed: number;
  lag: number;
  noise: NoiseLevel;
}

/** Seconds between points of the flown-path trail. */
const TRAIL_EVERY = 0.1;

let wasm: Promise<unknown> | null = null;
let flight: Flight | null = null;
let client: SolverClient | null = null;
const mpc = createMpc();
/** Every re-plan time this flight, ms (for the summary and measurements). */
let allMs: number[] = [];

/** Load the main-thread copy of the WebAssembly module (once). */
export function loadFlightWasm(): Promise<unknown> {
  wasm ??= init();
  return wasm;
}

export function setSolverClient(c: SolverClient | null) {
  client = c;
}

export function currentFlight(): Flight | null {
  return flight;
}

/** Re-plans received this flight, the latest solve times and all of them (ms). */
export function solveStats() {
  return { replans: mpc.replans, solveMs: mpc.solveMs, allMs };
}

/** Start a flight; false if the configuration was rejected. */
export function startFlight(config: FlightConfig): boolean {
  stopFlight();
  flight = Flight.create(JSON.stringify(config)) ?? null;
  return flight !== null;
}

export function stopFlight() {
  flight?.free();
  flight = null;
  mpc.reset();
  allMs = [];
  live.trail = [];
  live.snapshot = null;
}

/**
 * Advance the flight by one frame of `dt` real seconds and publish its state
 * to `live` and (when something visible changed) to the store.
 */
export function frame(dt: number): Snapshot | null {
  const f = flight;
  if (!f) return null;
  const s = useApp.getState();

  // Hold still until guidance has said something (the first plan takes a
  // full search, ~0.25 s), and while the visitor holds the booster.
  const before = JSON.parse(f.snapshot()) as Snapshot;
  if (before.mode !== "none") f.step(clampDt(dt, s.speed));

  if (client && mpc.shouldRequest(before.t, f.wants_replan())) {
    const { t, request } = JSON.parse(f.measure_request()) as { t: number; request: unknown };
    const epoch = mpc.sent(before.t);
    client.replan(request).then(({ response, ms }) => {
      // A reply for a flight that has since ended or been replaced is dropped.
      if (!mpc.received(epoch, ms) || flight !== f) return;
      allMs.push(ms);
      f.apply_replan(JSON.stringify(response), t);
      const trajectory = JSON.parse(f.plan_json()) as Trajectory | null;
      const ok = (response as { ok?: boolean }).ok === true;
      if (ok && trajectory) {
        useApp.getState().dispatch({ type: "flightUpdate", patch: { plan: { trajectory, t0: t } } });
      } else if (!trajectory) {
        useApp.getState().dispatch({ type: "flightUpdate", patch: { plan: null } });
      }
      live.solveVersion += 1;
    });
  }

  const snap = JSON.parse(f.snapshot()) as Snapshot;
  live.snapshot = snap;
  live.sample = sampleOf(snap);
  const last = live.trail.at(-1);
  if (!last || snap.t - last.t >= TRAIL_EVERY || snap.landed) {
    live.trail.push({ t: snap.t, position: snap.position });
  }

  const now = useApp.getState();
  if (now.flight && now.phase === "flying") {
    const patch = viewPatch(snap, now.flight);
    if (patch) now.dispatch({ type: "flightUpdate", patch });
    if (snap.outcome) now.dispatch({ type: "touchdown", outcome: snap.outcome });
  }
  return snap;
}

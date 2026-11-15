/**
 * The ship's closed loop on the main thread: owns the WebAssembly
 * `ShipFlight`, steps it every frame, asks the worker for SCvx plans
 * (`plan_ship`) when the flight wants one, at most one in flight, and
 * publishes the state to `shipLive` and (rarely) the store. Simulation only.
 */

import init, { ShipFlight } from "@/wasm/kestrel_wasm";
import { clampDt, createMpc } from "../mpc";
import type { SolverClient } from "../solverClient";
import { shipLive } from "./live";
import { useShip, type FlightView, type Noise, type Start } from "./store";
import type { PlanDraw, ScenarioKind, ShipSnapshot } from "./types";

/** Seconds of sim time between plan requests (the flight decides if it needs one). */
export const SHIP_REPLAN_PERIOD = 0.5;
const TRAIL_EVERY = 0.25;

let wasm: Promise<unknown> | null = null;
let flight: ShipFlight | null = null;
let client: SolverClient | null = null;
const mpc = createMpc(SHIP_REPLAN_PERIOD);
let lastTrail = -Infinity;

export function loadShipWasm(): Promise<unknown> {
  wasm ??= init();
  return wasm;
}

export function setShipClient(c: SolverClient | null) {
  client = c;
}

export function shipClient(): SolverClient | null {
  return client;
}

export function currentShip(): ShipFlight | null {
  return flight;
}

export function startShip(kind: ScenarioKind, start: Start, lag: number, noise: Noise): boolean {
  stopShip();
  const config = { scenario: kind, ...start, seed: Math.floor(Math.random() * 2 ** 32), lag, noise };
  flight = ShipFlight.create(JSON.stringify(config)) ?? null;
  return flight !== null;
}

export function stopShip() {
  flight?.free();
  flight = null;
  mpc.reset();
  shipLive.snapshot = null;
  shipLive.trail = [];
  shipLive.plan = null;
  shipLive.planVersion += 1;
  shipLive.solveMs = [];
  shipLive.replans = 0;
  lastTrail = -Infinity;
}

function viewOf(s: ShipSnapshot): FlightView {
  return {
    phase: s.phase,
    engines: s.engines,
    guide: s.guide,
    legEngines: s.leg_engines,
    outcome: s.outcome,
    reason: s.reason,
  };
}

function sameView(a: FlightView | null, b: FlightView): boolean {
  return (
    !!a &&
    a.phase === b.phase &&
    a.guide === b.guide &&
    a.legEngines === b.legEngines &&
    a.outcome === b.outcome &&
    a.engines.every((e, i) => e === b.engines[i])
  );
}

/** Advance one frame of `dt` real seconds. */
export function shipFrame(dt: number): ShipSnapshot | null {
  const f = flight;
  if (!f) return null;
  const st = useShip.getState();
  f.step(clampDt(dt, st.speed));
  const snap = JSON.parse(f.snapshot()) as ShipSnapshot;

  if (client && mpc.shouldRequest(snap.t, f.wants_replan())) {
    const { t, request } = JSON.parse(f.measure_request()) as { t: number; request: unknown };
    const epoch = mpc.sent(snap.t);
    void client.call("plan_ship", request).then(({ response, ms }) => {
      if (!mpc.received(epoch, ms) || flight !== f) return;
      f.apply_replan(JSON.stringify(response), t);
      shipLive.plan = JSON.parse(f.plan_json()) as PlanDraw | null;
      shipLive.planVersion += 1;
      shipLive.solveMs = [...mpc.solveMs];
      shipLive.replans = mpc.replans;
    });
  }

  shipLive.snapshot = snap;
  if (snap.t - lastTrail >= TRAIL_EVERY || snap.outcome) {
    shipLive.trail.push(snap.position);
    lastTrail = snap.t;
  }
  const view = viewOf(snap);
  if (!sameView(st.view, view)) st.setView(view);
  if (snap.outcome) st.finish();
  return snap;
}

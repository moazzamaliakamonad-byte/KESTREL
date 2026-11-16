/**
 * The closed loop on the main thread: owns the WebAssembly `ShipFlight`
 * (ship or booster), steps it every frame unless paused, asks the worker
 * for SCvx plans (`plan_ship`) when the flight wants one, at most one in
 * flight, and publishes the state to `shipLive` and (rarely) the store.
 * Simulation only.
 */

import init, { ShipFlight } from "@/wasm/kestrel_wasm";
import { clampDt, createMpc } from "../mpc";
import type { SolverClient } from "../solverClient";
import { Commentator, callouts } from "./commentary";
import { pushSample, sampleOf, shipLive } from "./live";
import { configOf, useShip, type FlightView } from "./store";
import type { PlanDraw, ShipSnapshot } from "./types";

/** Seconds of sim time between plan requests (the flight decides if it needs one). */
export const SHIP_REPLAN_PERIOD = 0.5;
const TRAIL_EVERY = 0.25;
const SAMPLE_EVERY = 0.1;

let wasm: Promise<unknown> | null = null;
let flight: ShipFlight | null = null;
let client: SolverClient | null = null;
const mpc = createMpc(SHIP_REPLAN_PERIOD);
let lastTrail = -Infinity;
let lastSample = -Infinity;
export const commentator = new Commentator();

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

/** Start a flight from the page's current state. False if Rust refuses it. */
export function startShip(): boolean {
  stopShip();
  const st = useShip.getState();
  const seed = st.options.seed ?? Math.floor(Math.random() * 2 ** 32);
  flight = ShipFlight.create(JSON.stringify({ ...configOf(st), seed })) ?? null;
  if (!flight) return false;
  // Until a solve has been timed: a typical one, in sim time.
  flight.set_solve_time(0.3 * st.view.speed);
  const snap = JSON.parse(flight.snapshot()) as ShipSnapshot;
  publish(snap, null);
  return true;
}

export function stopShip() {
  flight?.free();
  flight = null;
  mpc.reset();
  commentator.stop();
  shipLive.snapshot = null;
  shipLive.trail = [];
  shipLive.plan = null;
  shipLive.planVersion += 1;
  shipLive.solveMs = [];
  shipLive.replans = 0;
  shipLive.history = [];
  shipLive.caption = null;
  lastTrail = -Infinity;
  lastSample = -Infinity;
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
    a.engines.length === b.engines.length &&
    a.engines.every((e, i) => e === b.engines[i])
  );
}

function publish(snap: ShipSnapshot, prev: ShipSnapshot | null) {
  const st = useShip.getState();
  commentator.enabled = st.view.voice;
  commentator.say(callouts(st.scenario, prev, snap), st.view.speed);
  shipLive.caption = commentator.caption;
  shipLive.snapshot = snap;
  if (snap.t - lastTrail >= TRAIL_EVERY || snap.outcome) {
    shipLive.trail.push(snap.position);
    lastTrail = snap.t;
  }
  if (snap.t - lastSample >= SAMPLE_EVERY || snap.outcome) {
    shipLive.history = pushSample(shipLive.history, sampleOf(snap));
    lastSample = snap.t;
  }
  const view = viewOf(snap);
  if (!sameView(st.flight, view)) st.setFlight(view);
  if (snap.outcome) st.finish();
}

/** Ask the worker for a plan if the flight wants one (at most one request in flight). */
function requestPlan(f: ShipFlight, snap: ShipSnapshot) {
  if (!client || !mpc.shouldRequest(snap.t, f.wants_replan())) return;
  const { t, request } = JSON.parse(f.measure_request()) as { t: number; request: unknown };
  const epoch = mpc.sent(snap.t);
  void client.call("plan_ship", request).then(({ response, ms }) => {
    if (!mpc.received(epoch, ms) || flight !== f) return;
    f.apply_replan(JSON.stringify(response), t);
    // Tell the flight how long solves take (in sim time), so the booster
    // plans from where it will be when the next one arrives.
    const recent = mpc.solveMs.slice(-4);
    const mean = recent.reduce((a, b) => a + b, 0) / Math.max(1, recent.length);
    f.set_solve_time(Math.min(1.5, (mean / 1000) * useShip.getState().view.speed + 0.05));
    shipLive.plan = JSON.parse(f.plan_json()) as PlanDraw | null;
    shipLive.planVersion += 1;
    shipLive.solveMs = [...mpc.solveMs];
    shipLive.replans = mpc.replans;
  });
}

/** Advance one frame of `dt` real seconds (nothing moves while paused). */
export function shipFrame(dt: number): ShipSnapshot | null {
  const f = flight;
  if (!f) return null;
  const st = useShip.getState();
  if (st.view.paused || st.mode !== "flying" || shipLive.hold) return shipLive.snapshot;
  const prev = shipLive.snapshot;
  f.step(clampDt(dt, st.view.speed));
  const snap = JSON.parse(f.snapshot()) as ShipSnapshot;
  requestPlan(f, snap);
  publish(snap, prev);
  return snap;
}

/**
 * Step a paused flight forward by `seconds` of sim time (the analysis
 * view's step buttons). Guidance is asked for plans as usual; while the
 * flight stands still between steps a plan simply arrives "instantly".
 */
export function stepShip(seconds: number): ShipSnapshot | null {
  const f = flight;
  const st = useShip.getState();
  if (!f || st.mode !== "flying" || !st.view.paused || shipLive.hold || !(seconds > 0)) return shipLive.snapshot;
  let snap = shipLive.snapshot;
  for (let left = Math.min(seconds, 5); left > 1e-9 && !snap?.outcome; left -= 1 / 60) {
    const prev = snap;
    f.step(Math.min(left, 1 / 60));
    snap = JSON.parse(f.snapshot()) as ShipSnapshot;
    requestPlan(f, snap);
    publish(snap, prev);
  }
  return snap;
}

/** Throw a gust toward the page's gust direction. */
export function throwGust() {
  const o = useShip.getState().options;
  const a = (o.gustDir * Math.PI) / 180;
  flight?.set_gust_speed(o.gustSpeed);
  flight?.gust_toward(Math.cos(a), Math.sin(a));
}

/** Fail landing engine `i` in flight. */
export function failEngine(i: number): boolean {
  return flight?.fail_engine(i) ?? false;
}

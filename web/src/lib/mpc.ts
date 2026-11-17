/**
 * When to ask the worker for a re-plan during a closed-loop flight (MPC).
 *
 * Every REPLAN_PERIOD seconds of *sim* time, with at most one solve in
 * flight. Each request is stamped with an epoch; a reset (new flight, or
 * back to editing) bumps it, so late answers for an old flight are dropped.
 */

/** Seconds of sim time between re-plans. */
export const REPLAN_PERIOD = 0.25;
/** Longest real frame fed to the simulator, s: no jumps after a stall. */
export const MAX_FRAME = 0.1;
/** Solve times kept for the sparkline. */
const KEEP = 40;

export function clampDt(dt: number, speed: number): number {
  if (!(Number.isFinite(dt) && dt > 0)) return 0;
  return Math.min(dt, MAX_FRAME) * speed;
}

export interface Mpc {
  /** `urgent`: something happened; skip the minimum gap since the last request. */
  shouldRequest(simT: number, flightWants: boolean, urgent?: boolean): boolean;
  /** Record a request sent at sim time `simT`; returns its epoch. */
  sent(simT: number): number;
  /** An answer arrived; false if it belongs to an old flight (ignore it). */
  received(epoch: number, ms?: number): boolean;
  reset(): void;
  readonly replans: number;
  readonly solveMs: readonly number[];
}

export function createMpc(period: number = REPLAN_PERIOD): Mpc {
  let epoch = 0;
  let inFlight = false;
  let lastSent = Number.NEGATIVE_INFINITY;
  let replans = 0;
  const solveMs: number[] = [];
  return {
    shouldRequest(simT, flightWants, urgent = false) {
      return flightWants && !inFlight && (urgent || simT - lastSent >= period);
    },
    sent(simT) {
      inFlight = true;
      lastSent = simT;
      return epoch;
    },
    received(e, ms) {
      if (e !== epoch || !inFlight) return false;
      inFlight = false;
      replans += 1;
      if (ms !== undefined) {
        solveMs.push(ms);
        if (solveMs.length > KEEP) solveMs.shift();
      }
      return true;
    },
    reset() {
      epoch += 1;
      inFlight = false;
      lastSent = Number.NEGATIVE_INFINITY;
      replans = 0;
      solveMs.length = 0;
    },
    get replans() {
      return replans;
    },
    get solveMs() {
      return solveMs;
    },
  };
}

/**
 * The flight record: every decision the flight software and guidance
 * made, with its reason and the numbers behind it, and the full state
 * every 0.1 s, so the analysis view can replay any instant. Pure
 * functions; `runtime.ts` feeds them.
 */

import type { PlanDraw, ShipSnapshot } from "./types";

export type DecisionKind = "phase" | "ignition" | "plan" | "plan-failed" | "engines" | "failure" | "relight" | "arms" | "outcome" | "event" | "wind" | "gust";

export interface Decision {
  /** Sim time, s. */
  t: number;
  kind: DecisionKind;
  title: string;
  /** Why, in a sentence. */
  why: string;
  /** The numbers it was decided on. */
  facts: [string, string][];
}

const n = (v: number, d = 0) => (Number.isFinite(v) ? v.toFixed(d) : "—");
const speed = (s: ShipSnapshot) => Math.hypot(...s.velocity);
const lit = (s: ShipSnapshot) => s.engines.filter((e) => e === "running" || e === "igniting").length;

const PHASE_WHY: Record<string, [string, string]> = {
  flip: ["Ignition and flip", "The fall reached the ignition altitude: below it the engines could no longer stop the ship in time."],
  startup: ["Landing-burn start-up", "The fall reached the ignition altitude: below it the engines could no longer stop the booster in time."],
  "landing-burn": ["Landing burn", "Upright and on its burn engines: guidance's plan now steers the burn to the approach gate."],
  terminal: ["Final approach", "The plan reached its gate above the arms: the terminal controller flies the last metres onto them."],
};

const FROM = ["the sea (+x)", "the north-east", "the north (+y)", "the north-west", "the tower side (−x)", "the south-west", "the south (−y)", "the south-east"];

/** A wind (the air's velocity, x and y) in words: its speed and where it comes from. */
export function windWords(w: readonly [number, number] | readonly number[]): string {
  const speed = Math.hypot(w[0], w[1]);
  if (speed < 0.5) return "calm";
  // It comes from the opposite of where it blows.
  const deg = (Math.atan2(-w[1], -w[0]) * 180) / Math.PI;
  return `${n(speed)} m/s from ${FROM[Math.round((((deg % 360) + 360) % 360) / 45) % 8]}`;
}

/** Guidance's wind for a readout: "estimating…", "12.0 m/s ±0.4", or "off" when guidance does not use it. */
export function windReadout(s: Pick<ShipSnapshot, "wind_est" | "wind_sigma" | "wind_aware">): string {
  const sigma = Math.max(s.wind_sigma[0], s.wind_sigma[1]);
  const text = sigma < WIND_SETTLED ? `${n(Math.hypot(s.wind_est[0], s.wind_est[1]), 1)} m/s ±${n(sigma, 1)}` : "estimating…";
  return s.wind_aware ? text : `${text} (not used)`;
}

/** The estimate counts as settled once its 1σ is below this, m/s. */
export const WIND_SETTLED = 2;
/** A gust: the estimate moves more than GUST_JUMP m/s within GUST_WINDOW s; at most one entry per GUST_GAP s. */
export const GUST_JUMP = 4;
export const GUST_WINDOW = 3;
export const GUST_GAP = 4;

/** Watches guidance's wind estimate and records when it settles and when it feels a gust (D-046). */
export class WindWatch {
  private settled = false;
  private samples: { t: number; w: [number, number] }[] = [];
  private lastGust = -Infinity;

  update(s: ShipSnapshot): Decision[] {
    const sigma = Math.max(s.wind_sigma[0], s.wind_sigma[1]);
    const truth: [string, string] = ["True wind (the simulator's; guidance does not know it)", windWords([s.wind[0], s.wind[1]])];
    if (!this.settled) {
      if (!(sigma < WIND_SETTLED)) return [];
      this.settled = true;
      this.samples = [{ t: s.t, w: s.wind_est }];
      return [
        {
          t: s.t,
          kind: "wind",
          title: `Wind estimated: ${windWords(s.wind_est)}`,
          why: "The accelerometers' force, less gravity and the engines' thrust, is the air's push; the wind that explains it is now known to within 2 m/s. Guidance plans and steers with it from here.",
          facts: [["Uncertainty (1σ)", `±${n(sigma, 1)} m/s`], truth, ["Used by guidance", s.wind_aware ? "yes" : "no (switched off)"]],
        },
      ];
    }
    this.samples = this.samples.filter((p) => s.t - p.t <= GUST_WINDOW);
    this.samples.push({ t: s.t, w: s.wind_est });
    if (s.t - this.lastGust < GUST_GAP) return [];
    const jump = Math.max(...this.samples.map((p) => Math.hypot(s.wind_est[0] - p.w[0], s.wind_est[1] - p.w[1])));
    if (!(jump > GUST_JUMP)) return [];
    this.lastGust = s.t;
    this.samples = [{ t: s.t, w: s.wind_est }];
    return [
      {
        t: s.t,
        kind: "gust",
        title: `Gust felt: ${n(jump)} m/s change`,
        why: "The air's push changed faster than a steady wind would: the estimate follows the gust, and the next plan flies in it.",
        facts: [["Wind now (estimated)", windWords(s.wind_est)], truth],
      },
    ];
  }
}

/** What changed between two consecutive snapshots that is worth recording. */
export function decisionsBetween(prev: ShipSnapshot | null, next: ShipSnapshot): Decision[] {
  const out: Decision[] = [];
  const base = (s: ShipSnapshot): [string, string][] => [
    ["Altitude", `${n(s.position[2])} m`],
    ["Speed", `${n(speed(s), 1)} m/s`],
    ["Descent rate", `${n(-s.velocity[2], 1)} m/s`],
    ["Tilt", `${n(s.tilt_deg, 1)}°`],
    ["Propellant", `${n(s.propellant / 1000, 1)} t`],
  ];
  if (!prev) {
    out.push({
      t: next.t,
      kind: "phase",
      title: next.vehicle === "booster" ? "Descent begins" : "Belly flop begins",
      why: next.vehicle === "booster" ? "Engines off, grid fins steer the fall toward the ignition point." : "Engines off, four flaps hold the ship belly to the wind and steer it.",
      facts: [...base(next), ["Ignition altitude (rule)", `${n(next.ignition_z)} m`]],
    });
    return out;
  }
  if (next.phase !== prev.phase && PHASE_WHY[next.phase]) {
    const [title, why] = PHASE_WHY[next.phase];
    const facts = base(next);
    if (next.phase === "flip" || next.phase === "startup") facts.push(["Ignition altitude (rule)", `${n(next.ignition_z)} m`], ["Engines lit", String(lit(next))]);
    out.push({ t: next.t, kind: next.phase === "flip" || next.phase === "startup" ? "ignition" : "phase", title, why, facts });
  }
  // Engine changes.
  next.engines.forEach((e, i) => {
    const was = prev.engines[i];
    if (e === was) return;
    if (e === "failed") {
      out.push({ t: next.t, kind: "failure", title: `Engine ${i + 1} failed`, why: "The engine stopped giving thrust. Guidance must re-plan with what is left.", facts: [...base(next), ["Engines still lit", String(lit(next))]] });
    } else if (e === "igniting" && was === "shutdown") {
      out.push({
        t: next.t,
        kind: "relight",
        title: `Spare engine ${i + 1} relit`,
        why: "Fewer engines were lit than the burn needs, so the flight software lit a shut-down engine. It gives thrust after its ignition time; guidance re-plans from where the ship will be then.",
        facts: base(next),
      });
    }
  });
  const shutdown = next.engines.filter((e, i) => e === "shutdown" && prev.engines[i] !== "shutdown").length;
  if (shutdown > 0) {
    out.push({
      t: next.t,
      kind: "engines",
      title: `Down to ${lit(next)} engine${lit(next) === 1 ? "" : "s"}`,
      why:
        lit(next) <= 1
          ? "The flip is done: one engine can throttle low enough to hover, which the precise end of the landing needs."
          : "The plan's engine count for this part of the burn.",
      facts: [...base(next), ["Shut down", String(shutdown)]],
    });
  }
  if (!prev.arms && next.arms) {
    out.push({ t: next.t, kind: "arms", title: "Tower arms closing", why: "The catch pins came within reach of the arms and close to the catch axis.", facts: [...base(next), ["From the catch axis", `${n(Math.hypot(next.position[0], next.position[1]), 2)} m`]] });
  }
  if (next.outcome && !prev.outcome) {
    const v = next.end_velocity ?? next.velocity;
    out.push({
      t: next.t,
      kind: "outcome",
      title: next.outcome === "caught" ? "Caught" : `Lost: ${next.reason ?? "crashed"}`,
      why: next.outcome === "caught" ? "Pins on the arms within the speed, tilt and position limits." : "A limit of the catch, or of the vehicle, was broken.",
      facts: [
        ["Down at contact", `${n(-v[2], 2)} m/s`],
        ["Sideways at contact", `${n(Math.hypot(v[0], v[1]), 2)} m/s`],
        ["Off the catch axis", `${n(Math.hypot(next.position[0], next.position[1]), 2)} m`],
        ["Tilt", `${n(next.tilt_deg, 2)}°`],
        ["Propellant left", `${n(next.propellant / 1000, 1)} t`],
      ],
    });
  }
  return out;
}

/** Position of a plan at sim time `t` (linear between nodes), or null outside it. */
export function planPositionAt(plan: PlanDraw, t: number): [number, number, number] | null {
  const k = plan.position.length;
  if (k < 2) return null;
  const tau = t - plan.t0;
  if (tau < 0 || tau > plan.final_time) return null;
  const f = (tau / plan.final_time) * (k - 1);
  const i = Math.min(k - 2, Math.floor(f));
  const a = f - i;
  const p = plan.position[i];
  const q = plan.position[i + 1];
  return [p[0] + a * (q[0] - p[0]), p[1] + a * (q[1] - p[1]), p[2] + a * (q[2] - p[2])];
}

/** Why guidance asks for a new plan now, in words and numbers. */
export function replanReason(s: ShipSnapshot, plan: PlanDraw | null, event: string | null): { why: string; facts: [string, string][] } {
  if (event) return { why: event, facts: [] };
  if (!plan) return { why: "No plan to follow yet.", facts: [] };
  const age = s.t - plan.t0;
  const p = planPositionAt(plan, s.t);
  const off = p ? Math.hypot(s.position[0] - p[0], s.position[1] - p[1], s.position[2] - p[2]) : NaN;
  const facts: [string, string][] = [
    ["Off the plan", `${n(off, 1)} m`],
    ["Plan age", `${n(age, 1)} s`],
  ];
  if (off > 5) return { why: "The ship drifted more than 5 m off its plan (sensor noise, wind, the engines' lag).", facts };
  if (age >= 3) return { why: "The plan is 3 s old: a fresh one starts from what the sensors say now.", facts };
  return { why: "The ship's speed drifted off the plan's.", facts };
}

/** A plan guidance accepted (or failed to find). */
export function planDecision(
  s: ShipSnapshot,
  reason: { why: string; facts: [string, string][] },
  result: { ok: true; plan: PlanDraw; ms: number; iterations?: number; converged?: boolean; engines: number; gateZ: number } | { ok: false; ms: number; error: string },
): Decision {
  if (!result.ok) {
    return {
      t: s.t,
      kind: "plan-failed",
      title: "No plan found",
      why: `${reason.why} The solver found no path that meets every limit (${result.error}); the ship keeps its last plan if it is still near it, else brakes.`,
      facts: [...reason.facts, ["Solve time", `${n(result.ms)} ms`]],
    };
  }
  const p = result.plan;
  const peak = p.thrust.reduce((m, v) => Math.max(m, v), 0);
  return {
    t: s.t,
    kind: "plan",
    title: `New plan: ${n(p.final_time, 1)} s to the gate on ${result.engines} engine${result.engines === 1 ? "" : "s"}`,
    why: `${reason.why} Successive convexification found the burn that uses the least propellant while meeting every limit.`,
    facts: [
      ...reason.facts,
      ["Solve time", `${n(result.ms)} ms`],
      ...(result.iterations !== undefined ? ([["Iterations", `${result.iterations}${result.converged === false ? " (not fully converged)" : ""}`]] as [string, string][]) : []),
      ["Flight time to gate", `${n(p.final_time, 1)} s`],
      ["Gate altitude", `${n(result.gateZ)} m`],
      ["Peak planned thrust", `${n(peak / 1e6, 2)} MN`],
      ["Nodes", String(p.position.length)],
    ],
  };
}

/** One recorded instant: the full snapshot. */
export const FRAME_EVERY = 0.1;
export const FRAMES_MAX = 4000;

export function pushFrame(frames: ShipSnapshot[], s: ShipSnapshot): ShipSnapshot[] {
  frames.push(s);
  // Keep memory bounded: when full, drop every other frame.
  return frames.length <= FRAMES_MAX ? frames : frames.filter((_, i) => i % 2 === 0);
}

/** The recorded frame nearest sim time `t` (frames in time order). */
export function frameAt(frames: readonly ShipSnapshot[], t: number): ShipSnapshot | null {
  if (!frames.length) return null;
  let lo = 0;
  let hi = frames.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (frames[mid].t < t) lo = mid + 1;
    else hi = mid;
  }
  const a = frames[Math.max(0, lo - 1)];
  const b = frames[lo];
  return Math.abs(a.t - t) <= Math.abs(b.t - t) ? a : b;
}

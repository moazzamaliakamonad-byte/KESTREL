import type { PlanResponse, Trajectory, Vec3 } from "./types";

export type Phase = "loading" | "editing" | "flying" | "touchdown" | "failed";
export type Speed = 1 | 2 | 4;
export type NoiseLevel = "off" | "low" | "high";

/** What guidance is doing (from `Flight.snapshot().mode`). */
export type GuideMode = "none" | "pinpoint" | "closest" | "brake";

/** How a flight ended (from `Flight.snapshot().outcome`). */
export interface Outcome {
  landed: boolean;
  speed: number;
  vertical: number;
  horizontal: number;
  /** Horizontal distance from the pad, m. */
  miss: number;
  fuel_used: number;
}

/**
 * The closed-loop flight as React sees it: only values that change a few
 * times a second at most. Per-frame state lives in `live`.
 */
export interface FlightView {
  mode: GuideMode;
  /** Planned miss distance in closest mode, m. */
  miss: number | null;
  engines: number;
  /** The plan being flown (positions relative to the pad) and the sim time it starts at. */
  plan: { trajectory: Trajectory; t0: number } | null;
  /** True in the last seconds of the plan (pad camera, blinking pad). */
  nearTouchdown: boolean;
  outcome: Outcome | null;
}

export interface StartState {
  position: Vec3;
  velocity: Vec3;
}

export interface AppState {
  phase: Phase;
  start: StartState;
  /** Id of the newest solve request; only its response may be used. */
  latestRequestId: number;
  /** When the newest request was sent (ms), or null when it has answered. */
  requestedAt: number | null;
  /** The start state the newest request was solved for. */
  requestedStart: StartState | null;
  /** The newest answer, and the exact start state it is a plan for. */
  preview: { requestId: number; response: PlanResponse; start: StartState } | null;
  flight: FlightView | null;
  speed: Speed;
  /** Disturbances chosen before or during a flight. */
  settings: { lag: number; noise: NoiseLevel };
  failure: string | null;
}

export type Action =
  | { type: "ready"; start: StartState }
  | { type: "setStart"; start: StartState }
  | { type: "requested"; id: number; at: number; start: StartState }
  | { type: "response"; id: number; response: PlanResponse }
  | { type: "land" }
  | { type: "flightUpdate"; patch: Partial<Omit<FlightView, "outcome">> }
  | { type: "touchdown"; outcome: Outcome }
  | { type: "setLag"; lag: number }
  | { type: "setNoise"; noise: NoiseLevel }
  | { type: "reset" }
  | { type: "setSpeed"; speed: Speed }
  | { type: "workerError"; message: string };

export const initialState: AppState = {
  phase: "loading",
  start: { position: [0, 0, 0], velocity: [0, 0, 0] },
  latestRequestId: 0,
  requestedAt: null,
  requestedStart: null,
  preview: null,
  flight: null,
  speed: 1,
  settings: { lag: 0, noise: "off" },
  failure: null,
};

/**
 * The preview answers the newest request *and* was solved for the start the
 * visitor sees now. Moving the start makes it stale at once, before any new
 * request is even sent.
 */
export function isPreviewCurrent(s: AppState): boolean {
  return (
    s.preview !== null &&
    s.preview.requestId === s.latestRequestId &&
    s.preview.start === s.start
  );
}

/** Land is allowed only on a successful plan for the current start state. */
export function canLand(s: AppState): boolean {
  return s.phase === "editing" && isPreviewCurrent(s) && !!s.preview?.response.ok;
}

export function reduce(s: AppState, a: Action): AppState {
  switch (a.type) {
    case "ready":
      return s.phase === "loading" ? { ...s, phase: "editing", start: a.start } : s;
    case "setStart":
      return s.phase === "editing" ? { ...s, start: a.start } : s;
    case "requested":
      return { ...s, latestRequestId: a.id, requestedAt: a.at, requestedStart: a.start };
    case "response":
      if (a.id !== s.latestRequestId || !s.requestedStart) return s;
      return {
        ...s,
        preview: { requestId: a.id, response: a.response, start: s.requestedStart },
        requestedAt: null,
      };
    case "land": {
      if (!canLand(s) || !s.preview?.response.ok) return s;
      const flight: FlightView = {
        mode: "none",
        miss: null,
        engines: 6,
        plan: null,
        nearTouchdown: false,
        outcome: null,
      };
      return { ...s, phase: "flying", flight };
    }
    case "flightUpdate":
      if (s.phase !== "flying" || !s.flight) return s;
      return { ...s, flight: { ...s.flight, ...a.patch } };
    case "touchdown":
      if (s.phase !== "flying" || !s.flight) return s;
      return { ...s, phase: "touchdown", flight: { ...s.flight, outcome: a.outcome, nearTouchdown: false } };
    case "setLag":
      return Number.isFinite(a.lag) && a.lag >= 0 ? { ...s, settings: { ...s.settings, lag: a.lag } } : s;
    case "setNoise":
      return { ...s, settings: { ...s.settings, noise: a.noise } };
    case "reset":
      return s.phase === "flying" || s.phase === "touchdown"
        ? { ...s, phase: "editing", flight: null }
        : s;
    case "setSpeed":
      return { ...s, speed: a.speed };
    case "workerError":
      return { ...s, phase: "failed", failure: a.message };
  }
}

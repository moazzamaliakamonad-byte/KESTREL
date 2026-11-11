import type { PlanResponse, PlanSuccess, Vec3 } from "./types";

export type Phase = "loading" | "editing" | "flying" | "touchdown" | "failed";
export type Speed = 1 | 2 | 4;

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
  preview: { requestId: number; response: PlanResponse } | null;
  flight: { response: PlanSuccess; t: number } | null;
  speed: Speed;
  failure: string | null;
}

export type Action =
  | { type: "ready"; start: StartState }
  | { type: "setStart"; start: StartState }
  | { type: "requested"; id: number; at: number }
  | { type: "response"; id: number; response: PlanResponse }
  | { type: "land" }
  | { type: "tick"; dt: number }
  | { type: "reset" }
  | { type: "setSpeed"; speed: Speed }
  | { type: "workerError"; message: string };

export const initialState: AppState = {
  phase: "loading",
  start: { position: [0, 0, 0], velocity: [0, 0, 0] },
  latestRequestId: 0,
  requestedAt: null,
  preview: null,
  flight: null,
  speed: 1,
  failure: null,
};

/** Land is allowed only on a successful plan for the current start state. */
export function canLand(s: AppState): boolean {
  return (
    s.phase === "editing" &&
    s.preview !== null &&
    s.preview.response.ok &&
    s.preview.requestId === s.latestRequestId
  );
}

export function reduce(s: AppState, a: Action): AppState {
  switch (a.type) {
    case "ready":
      return s.phase === "loading" ? { ...s, phase: "editing", start: a.start } : s;
    case "setStart":
      return s.phase === "editing" ? { ...s, start: a.start } : s;
    case "requested":
      return { ...s, latestRequestId: a.id, requestedAt: a.at };
    case "response":
      if (a.id !== s.latestRequestId) return s;
      return { ...s, preview: { requestId: a.id, response: a.response }, requestedAt: null };
    case "land": {
      if (!canLand(s) || !s.preview?.response.ok) return s;
      return { ...s, phase: "flying", flight: { response: s.preview.response, t: 0 } };
    }
    case "tick": {
      if (s.phase !== "flying" || !s.flight) return s;
      const end = s.flight.response.trajectory.final_time;
      const step = Number.isFinite(a.dt) && a.dt > 0 ? a.dt * s.speed : 0;
      const t = s.flight.t + step;
      if (t >= end) return { ...s, phase: "touchdown", flight: { ...s.flight, t: end } };
      return { ...s, flight: { ...s.flight, t } };
    }
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

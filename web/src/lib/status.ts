import { isPreviewCurrent, type AppState } from "./state";
import type { ErrorKind } from "./types";

export type Tone = "info" | "ok" | "warn" | "error";

/** After this long without an answer, say so (ms). */
const SLOW_SOLVE_MS = 3000;

const ERROR_TEXT: Record<ErrorKind, string> = {
  Infeasible: "No landing exists from here: too fast, too low or too little propellant.",
  NotLossless: "Couldn't certify a flyable plan from here; try another start.",
  SolverFailed: "Solver failed",
  InvalidProblem: "Invalid start",
  InvalidParams: "Invalid vehicle parameters",
  BadRequest: "Bad request",
};

/** Touchdowns closer than this count as "on the pad", m. */
export const PAD_RADIUS = 5;

function flightText(s: AppState): { tone: Tone; text: string } {
  const f = s.flight;
  switch (f?.mode ?? "none") {
    case "none":
      return { tone: "info", text: "Computing the first plan…" };
    case "pinpoint":
      return { tone: "info", text: "Guiding to the pad" };
    case "brake":
      return { tone: "error", text: "No safe landing exists: braking" };
    case "closest": {
      const miss = f?.miss ?? 0;
      if (miss < 1) return { tone: "warn", text: "Outside the glide cone: landing without it" };
      return {
        tone: "warn",
        text: `No pinpoint landing from here: guiding to the closest point, ≈${Math.round(miss)} m from the pad`,
      };
    }
  }
}

/** The one-line status shown in the HUD. `now` is `performance.now()`. */
export function statusText(s: AppState, now: number): { tone: Tone; text: string } {
  switch (s.phase) {
    case "loading":
      return { tone: "info", text: "Loading the guidance solver…" };
    case "failed":
      return { tone: "error", text: s.failure ?? "Something went wrong" };
    case "flying":
      return flightText(s);
    case "touchdown": {
      const o = s.flight?.outcome;
      if (!o) return { tone: "ok", text: "Touchdown" };
      if (!o.landed) return { tone: "error", text: `Crashed at ${Math.round(o.speed)} m/s` };
      if (o.miss < PAD_RADIUS) return { tone: "ok", text: "Landed on the pad" };
      return { tone: "ok", text: `Landed ${Math.round(o.miss)} m from the pad` };
    }
    case "editing": {
      if (s.requestedAt !== null) {
        const slow = now - s.requestedAt > SLOW_SOLVE_MS;
        return { tone: "info", text: slow ? "Still planning…" : "Planning…" };
      }
      // No answer yet, or the answer is for a start the visitor has since moved.
      if (!isPreviewCurrent(s)) return { tone: "info", text: "Planning…" };
      const r = s.preview?.response;
      if (!r) return { tone: "info", text: "Planning…" };
      if (r.ok) return { tone: "ok", text: "Ready: press Land" };
      const base = ERROR_TEXT[r.error.kind];
      const tone: Tone = r.error.kind === "Infeasible" || r.error.kind === "NotLossless" ? "error" : "warn";
      const detail = r.error.kind === "Infeasible" || r.error.kind === "NotLossless" ? "" : ` (${r.error.message})`;
      return { tone, text: base + detail };
    }
  }
}

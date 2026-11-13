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

/** The one-line status shown in the HUD. `now` is `performance.now()`. */
export function statusText(s: AppState, now: number): { tone: Tone; text: string } {
  switch (s.phase) {
    case "loading":
      return { tone: "info", text: "Loading the guidance solver…" };
    case "failed":
      return { tone: "error", text: s.failure ?? "Something went wrong" };
    case "flying":
      return { tone: "info", text: "Landing" };
    case "touchdown":
      return { tone: "ok", text: "Touchdown" };
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

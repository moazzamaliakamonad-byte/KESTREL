import type { AppState, StartState } from "./state";

/**
 * Keeps at most one solve in flight. Starts submitted meanwhile overwrite
 * each other; when the answer arrives (`done`), only the newest one is
 * sent. So a long drag never builds a queue of stale solves, and the path
 * keeps re-planning while the pointer moves.
 */
export function createScheduler(send: (start: StartState) => void) {
  let inFlight = false;
  let pending: StartState | null = null;
  return {
    submit(start: StartState) {
      if (inFlight) {
        pending = start;
        return;
      }
      inFlight = true;
      send(start);
    },
    done() {
      inFlight = false;
      if (pending) {
        const next = pending;
        pending = null;
        inFlight = true;
        send(next);
      }
    },
  };
}

/**
 * True when the visitor moved the start while editing. The loading → editing
 * transition (first start from Rust) is excluded: that one is solved
 * explicitly, so it must not trigger a second, duplicate solve.
 */
export function startChangedWhileEditing(s: AppState, prev: AppState): boolean {
  return s.phase === "editing" && prev.phase === "editing" && s.start !== prev.start;
}

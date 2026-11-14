import type { PlanRequest, PlanResponse } from "./types";

/**
 * Messages from the page to the solver worker. A re-plan carries the
 * request JSON object produced by `Flight.measure_request()` as-is.
 */
export type PageToWorker =
  | { id: number; request: PlanRequest }
  | { id: 0; kind: "example" }
  | { id: number; kind: "replan"; request: unknown };

/** Messages from the solver worker to the page. Id 0 = example, −1 = fatal. */
export type WorkerToPage =
  | { id: number; response: PlanResponse; ms: number }
  | { id: 0; example: PlanRequest }
  | { id: number; replan: unknown; ms: number }
  | { id: -1; fatal: string };

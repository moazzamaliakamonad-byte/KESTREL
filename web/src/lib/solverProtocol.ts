import type { PlanRequest, PlanResponse } from "./types";

/** Messages from the page to the solver worker. */
export type PageToWorker = { id: number; request: PlanRequest } | { id: 0; kind: "example" };

/** Messages from the solver worker to the page. Id 0 = example, −1 = fatal. */
export type WorkerToPage =
  | { id: number; response: PlanResponse; ms: number }
  | { id: 0; example: PlanRequest }
  | { id: -1; fatal: string };

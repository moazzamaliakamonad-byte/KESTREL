import type { CallKind, PageToWorker, WorkerToPage } from "./solverProtocol";
import type { PlanRequest, PlanResponse } from "./types";

/** Hands out increasing ids and accepts only the newest one, once. */
export class LatestOnly {
  private current = 0;
  private delivered = true;

  next(): number {
    this.current += 1;
    this.delivered = false;
    return this.current;
  }

  accept(id: number): boolean {
    if (id !== this.current || this.delivered) return false;
    this.delivered = true;
    return true;
  }
}

/** The parts of `Worker` the client uses (so tests can pass a fake). */
export interface WorkerLike {
  postMessage(message: unknown): void;
  addEventListener(type: "message", listener: (e: MessageEvent<WorkerToPage>) => void): void;
}

export interface SolverClient {
  /** The default request (vehicle and start) from the Rust side. */
  loadExample(): Promise<PlanRequest>;
  /** Send a solve; returns its id. Only the newest id's answer is delivered. */
  request(req: PlanRequest): number;
  /** A closed-loop re-plan (`Flight.measure_request().request`). Every
   * answer is delivered; staleness is the caller's call (see `mpc.ts`). */
  replan(request: unknown): Promise<{ response: unknown; ms: number }>;
  /** Any id-answered worker call (ship plans and previews). */
  call(kind: CallKind, request: unknown): Promise<{ response: unknown; ms: number }>;
}

export function createSolverClient(
  worker: WorkerLike,
  onResult: (id: number, response: PlanResponse, ms: number) => void,
  onFatal: (message: string) => void,
): SolverClient {
  const latest = new LatestOnly();
  const exampleWaiters: ((req: PlanRequest) => void)[] = [];
  // Re-plan ids are their own sequence: replies are told apart by key.
  let nextReplan = 1;
  const replanWaiters = new Map<number, (r: { response: unknown; ms: number }) => void>();

  worker.addEventListener("message", (e) => {
    const msg = e.data;
    if ("fatal" in msg) onFatal(msg.fatal);
    else if ("example" in msg) exampleWaiters.splice(0).forEach((resolve) => resolve(msg.example));
    else if ("replan" in msg) {
      const resolve = replanWaiters.get(msg.id);
      replanWaiters.delete(msg.id);
      resolve?.({ response: msg.replan, ms: msg.ms });
    } else if (latest.accept(msg.id)) onResult(msg.id, msg.response, msg.ms);
  });

  const post = (m: PageToWorker) => worker.postMessage(m);

  return {
    loadExample() {
      const p = new Promise<PlanRequest>((resolve) => exampleWaiters.push(resolve));
      post({ id: 0, kind: "example" });
      return p;
    },
    request(req) {
      const id = latest.next();
      post({ id, request: req });
      return id;
    },
    replan(request) {
      return this.call("replan", request);
    },
    call(kind, request) {
      const id = nextReplan++;
      const answer = new Promise<{ response: unknown; ms: number }>((resolve) =>
        replanWaiters.set(id, resolve),
      );
      post({ id, kind, request });
      return answer;
    },
  };
}

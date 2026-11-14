import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/example-response.json";
import { createSolverClient, LatestOnly, type WorkerLike } from "./solverClient";
import type { WorkerToPage } from "./solverProtocol";
import type { PlanRequest, PlanResponse } from "./types";

const ok = fixture as PlanResponse;
const req: PlanRequest = { position: [2000, 0, 1500], velocity: [-50, 0, -75] };

/** A stand-in for a Web Worker: records posts, lets the test reply. */
function fakeWorker() {
  const target = new EventTarget();
  const posted: unknown[] = [];
  const worker: WorkerLike = {
    postMessage: (m: unknown) => void posted.push(m),
    addEventListener: (type, listener) => target.addEventListener(type, listener as EventListener),
  };
  const reply = (data: WorkerToPage) => target.dispatchEvent(new MessageEvent("message", { data }));
  return { worker, posted, reply };
}

describe("LatestOnly", () => {
  it("accepts only the newest id, once", () => {
    const l = new LatestOnly();
    const a = l.next();
    const b = l.next();
    expect(l.accept(a)).toBe(false);
    expect(l.accept(b)).toBe(true);
    expect(l.accept(b)).toBe(false);
  });
});

describe("solver client", () => {
  it("drops stale responses", () => {
    const { worker, reply } = fakeWorker();
    const got: number[] = [];
    const client = createSolverClient(worker, (id) => got.push(id), () => {});
    const ids = [client.request(req), client.request(req), client.request(req)];
    reply({ id: ids[2], response: ok, ms: 1 });
    reply({ id: ids[0], response: ok, ms: 1 });
    reply({ id: ids[1], response: ok, ms: 1 });
    expect(got).toEqual([ids[2]]);
  });

  it("delivers fatal worker errors", () => {
    const { worker, reply } = fakeWorker();
    const errors: string[] = [];
    createSolverClient(worker, () => {}, (m) => errors.push(m));
    reply({ id: -1, fatal: "could not load wasm" });
    expect(errors).toEqual(["could not load wasm"]);
  });

  it("answers each re-plan by its own id, apart from preview solves", async () => {
    const { worker, posted, reply } = fakeWorker();
    const previews: number[] = [];
    const client = createSolverClient(worker, (id) => previews.push(id), () => {});
    const a = client.replan({ problem: "a" });
    const b = client.replan({ problem: "b" });
    const [ma, mb] = posted.slice(-2) as { id: number; kind: string; request: unknown }[];
    expect(ma).toMatchObject({ kind: "replan", request: { problem: "a" } });
    const previewId = client.request(req);
    reply({ id: mb.id, replan: { ok: true, n: 2 }, ms: 7 });
    reply({ id: ma.id, replan: { ok: true, n: 1 }, ms: 12 });
    await expect(a).resolves.toEqual({ response: { ok: true, n: 1 }, ms: 12 });
    await expect(b).resolves.toEqual({ response: { ok: true, n: 2 }, ms: 7 });
    expect(previews).toEqual([]);
    reply({ id: previewId, response: ok, ms: 1 });
    expect(previews).toEqual([previewId]);
  });

  it("loads the example request", async () => {
    const { worker, posted, reply } = fakeWorker();
    const client = createSolverClient(worker, () => {}, () => {});
    const pending = client.loadExample();
    expect(posted).toContainEqual({ id: 0, kind: "example" });
    reply({ id: 0, example: req });
    await expect(pending).resolves.toEqual(req);
  });
});

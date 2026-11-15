// Runs the Rust guidance (WebAssembly) off the main thread, so a ~250 ms
// solve never stalls rendering. Protocol: src/lib/solverProtocol.ts.

import init, { example_request, plan_landing, plan_ship, preview_ship, replan } from "@/wasm/kestrel_wasm";
import type { PageToWorker, WorkerToPage } from "@/lib/solverProtocol";
import type { PlanRequest, PlanResponse } from "@/lib/types";

// Typed view of the worker global (the project's TS lib is "dom").
const ctx = self as unknown as {
  postMessage(message: WorkerToPage): void;
  addEventListener(type: "message", listener: (e: MessageEvent<PageToWorker>) => void): void;
};

// No path: the glue's own `new URL(..., import.meta.url)` points at the
// bundled, hashed .wasm asset.
const ready = init();

ready.catch((err: unknown) => {
  ctx.postMessage({ id: -1, fatal: `Could not load the solver: ${String(err)}` });
});

ctx.addEventListener("message", async (e) => {
  try {
    await ready;
  } catch {
    return; // already reported
  }
  const msg = e.data;
  if ("kind" in msg && (msg.kind === "replan" || msg.kind === "plan_ship" || msg.kind === "preview_ship")) {
    const fn = msg.kind === "replan" ? replan : msg.kind === "plan_ship" ? plan_ship : preview_ship;
    const t0 = performance.now();
    const answer: unknown = JSON.parse(fn(JSON.stringify(msg.request)));
    ctx.postMessage({ id: msg.id, replan: answer, ms: performance.now() - t0 });
    return;
  }
  if ("kind" in msg) {
    ctx.postMessage({ id: 0, example: JSON.parse(example_request()) as PlanRequest });
    return;
  }
  const t0 = performance.now();
  const response = JSON.parse(plan_landing(JSON.stringify(msg.request))) as PlanResponse;
  ctx.postMessage({ id: msg.id, response, ms: performance.now() - t0 });
});

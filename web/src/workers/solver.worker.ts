// Runs the Rust guidance (WebAssembly) off the main thread, so a ~250 ms
// solve never stalls rendering. Protocol: src/lib/solverProtocol.ts.

import init, { example_request, plan_landing } from "@/wasm/kestrel_wasm";
import { BASE_PATH } from "@/lib/env";
import type { PageToWorker, WorkerToPage } from "@/lib/solverProtocol";
import type { PlanRequest, PlanResponse } from "@/lib/types";

// Typed view of the worker global (the project's TS lib is "dom").
const ctx = self as unknown as {
  postMessage(message: WorkerToPage): void;
  addEventListener(type: "message", listener: (e: MessageEvent<PageToWorker>) => void): void;
};

const ready = init({ module_or_path: `${BASE_PATH}/wasm/kestrel_wasm_bg.wasm` });

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
  if ("kind" in msg) {
    ctx.postMessage({ id: 0, example: JSON.parse(example_request()) as PlanRequest });
    return;
  }
  const t0 = performance.now();
  const response = JSON.parse(plan_landing(JSON.stringify(msg.request))) as PlanResponse;
  ctx.postMessage({ id: msg.id, response, ms: performance.now() - t0 });
});

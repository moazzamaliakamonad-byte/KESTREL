// Time the WebAssembly solver in Node.js (same V8 engine as Chrome), with no
// rendering competing for the CPU.  Run after `npm run wasm`:
//   node scripts/bench-wasm.mjs [runs]
import { readFileSync } from "node:fs";
import init, { example_request, plan_landing } from "../src/wasm/kestrel_wasm.js";

const runs = Number(process.argv[2] ?? 10);
await init({ module_or_path: readFileSync(new URL("../src/wasm/kestrel_wasm_bg.wasm", import.meta.url)) });
const req = example_request();
const times = [];
let last;
for (let i = 0; i < runs + 2; i++) {
  const t0 = performance.now();
  last = JSON.parse(plan_landing(req));
  const ms = performance.now() - t0;
  if (i >= 2) times.push(ms); // skip warm-up runs
}
times.sort((a, b) => a - b);
const median = times[Math.floor(times.length / 2)];
console.log(`node ${process.version}: ${runs} runs, median ${median.toFixed(0)} ms, range ${times[0].toFixed(0)}–${times.at(-1).toFixed(0)} ms`);
console.log(`result: ok=${last.ok} final_time=${last.trajectory?.final_time.toFixed(2)} fuel=${last.trajectory?.fuel_used.toFixed(2)}`);

// Build the Rust guidance bridge for the browser.
//
// 1. cargo build -p kestrel-wasm --release --target wasm32-unknown-unknown
// 2. wasm-bindgen --target web → JS glue in web/src/wasm/
// 3. move the .wasm binary to web/public/wasm/ (served as a static file)
//
// Run from anywhere: `node scripts/build-wasm.mjs` (web/ calls it via `npm run wasm`).

import { spawnSync } from "node:child_process";
import { mkdirSync, renameSync, rmSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { homedir } from "node:os";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const glueDir = join(root, "web", "src", "wasm");
const publicDir = join(root, "web", "public", "wasm");
const wasmIn = join(root, "target", "wasm32-unknown-unknown", "release", "kestrel_wasm.wasm");

// rustup puts tools here; it is not always on PATH in fresh shells.
const cargoBin = join(homedir(), ".cargo", "bin");
const env = { ...process.env, PATH: `${cargoBin}${process.platform === "win32" ? ";" : ":"}${process.env.PATH}` };

function run(cmd, args) {
  console.log(`> ${cmd} ${args.join(" ")}`);
  const r = spawnSync(cmd, args, { cwd: root, env, stdio: "inherit", shell: process.platform === "win32" });
  if (r.error || r.status !== 0) {
    console.error(`\nbuild-wasm: '${cmd}' failed. Is it installed? See docs/SETUP.md.`);
    process.exit(r.status || 1);
  }
}

run("cargo", ["build", "-p", "kestrel-wasm", "--release", "--target", "wasm32-unknown-unknown"]);

rmSync(glueDir, { recursive: true, force: true });
mkdirSync(glueDir, { recursive: true });
run("wasm-bindgen", ["--target", "web", "--out-dir", glueDir, "--out-name", "kestrel_wasm", wasmIn]);

mkdirSync(publicDir, { recursive: true });
const wasmOut = join(glueDir, "kestrel_wasm_bg.wasm");
if (!existsSync(wasmOut)) {
  console.error("build-wasm: wasm-bindgen did not produce kestrel_wasm_bg.wasm");
  process.exit(1);
}
renameSync(wasmOut, join(publicDir, "kestrel_wasm_bg.wasm"));
console.log("build-wasm: done");

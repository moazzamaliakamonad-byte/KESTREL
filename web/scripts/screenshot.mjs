// Screenshot the built site (web/out) in headless Chromium.
//
//   node scripts/screenshot.mjs <out.png> [waitMs] [script.js]
//
// The optional script runs in the page after loading (e.g. to press keys).
// Used to check visuals during development and to make README images.
// Set SHOT_URL (e.g. http://localhost:3000) to use a running server instead.

import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { chromium } from "@playwright/test";

const [, , outFile = "shot.png", waitMs = "6000", scriptFile] = process.argv;
const port = 4173;
const external = process.env.SHOT_URL;
const url = external ?? `http://localhost:${port}/`;

const server = external ? null : spawn("npx", ["--no-install", "serve", "out", "-l", String(port)], {
  shell: process.platform === "win32",
  stdio: "ignore",
});

async function waitForServer() {
  for (let i = 0; i < 60; i++) {
    try {
      const r = await fetch(url);
      if (r.ok) return;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error("static server did not start");
}

try {
  await waitForServer();
  const browser = await chromium.launch({
    args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
  });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  await page.goto(url);
  await page.waitForTimeout(Number(waitMs));
  if (scriptFile) {
    await page.evaluate(readFileSync(scriptFile, "utf8"));
    await page.waitForTimeout(Number(waitMs));
  }
  await page.screenshot({ path: outFile });
  console.log(`wrote ${outFile}`);
  if (errors.length) console.log("page errors:\n" + errors.join("\n"));
  await browser.close();
} finally {
  server?.kill();
  if (server && process.platform === "win32") spawn("taskkill", ["/pid", String(server.pid), "/T", "/F"], { stdio: "ignore" });
}

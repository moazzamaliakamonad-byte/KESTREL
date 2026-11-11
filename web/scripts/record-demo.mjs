// Record the README demo: drag the booster (the plan re-bends live), then
// Land and fly to touchdown. Writes PNG frames; scripts/make-gif.py turns
// them into a GIF.
//
//   npm run build && PW_GPU=1 node scripts/record-demo.mjs <framesDir>

import { spawn } from "node:child_process";
import { mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "@playwright/test";

const outDir = process.argv[2] ?? "demo-frames";
const port = 4175;
rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

const server = spawn("npx", ["--no-install", "serve", "out", "-l", String(port)], {
  shell: process.platform === "win32",
  stdio: "ignore",
});

let frame = 0;
const shot = (page) =>
  page.screenshot({ path: join(outDir, `f${String(frame++).padStart(4, "0")}.png`) });

try {
  for (let i = 0; i < 60; i++) {
    try {
      if ((await fetch(`http://localhost:${port}/`)).ok) break;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  const browser = await chromium.launch({
    args: process.env.PW_GPU
      ? ["--use-angle=d3d11", "--enable-gpu", "--ignore-gpu-blocklist"]
      : ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
  });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.goto(`http://localhost:${port}/`);
  await page.getByTestId("status").filter({ hasText: "Ready: press Land" }).waitFor({ timeout: 60_000 });
  await page.waitForTimeout(800);
  for (let i = 0; i < 6; i++) await shot(page);

  // Drag the booster in an arc; the plan re-solves as it moves.
  const k = () => page.evaluate(() => window.__kestrel.getState().start.position);
  const screenOf = (p) => page.evaluate((p) => window.__kestrel.screenOf(p), p);
  const start = await screenOf(await k());
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  const path = [
    [-80, -60],
    [-180, -90],
    [-300, -60],
    [-380, 20],
    [-300, 90],
    [-160, 60],
    [-60, 10],
  ];
  for (const [dx, dy] of path) {
    await page.mouse.move(start.x + dx, start.y + dy, { steps: 6 });
    for (let i = 0; i < 4; i++) {
      await page.waitForTimeout(140);
      await shot(page);
    }
  }
  await page.mouse.up();
  await page.getByTestId("status").filter({ hasText: "Ready: press Land" }).waitFor({ timeout: 30_000 });
  for (let i = 0; i < 4; i++) await shot(page);

  // Land at ×4 and follow the flight to touchdown.
  await page.getByRole("button", { name: "×4" }).click();
  await page.keyboard.press("Space");
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    await page.waitForTimeout(110);
    await shot(page);
    if ((await page.getByTestId("status").textContent()) === "Touchdown") break;
  }
  for (let i = 0; i < 14; i++) {
    await page.waitForTimeout(150);
    await shot(page);
  }
  console.log(`recorded ${frame} frames in ${outDir}`);
  await browser.close();
} finally {
  server.kill();
  if (process.platform === "win32") spawn("taskkill", ["/pid", String(server.pid), "/T", "/F"], { stdio: "ignore" });
}

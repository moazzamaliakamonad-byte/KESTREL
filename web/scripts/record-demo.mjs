// Record the README demo of the one-page console: the opening sequence,
// the booster's landing burn and catch, the blueprint analysis view, and
// the ship's entry from orbit. Writes PNG frames; scripts/make-gif.py turns
// them into a GIF.
//
//   PW_GPU=1 node scripts/record-demo.mjs <framesDir> [url]
//
// `url` defaults to a running dev server (http://localhost:3000). Frames
// are taken at moments in the flight, so the GIF runs faster than real
// time (it says so).

import { mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "@playwright/test";

const outDir = process.argv[2] ?? "demo-frames";
const url = process.argv[3] ?? "http://localhost:3000";
rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

let frame = 0;
const shot = (page) => page.screenshot({ path: join(outDir, `f${String(frame++).padStart(4, "0")}.png`) });
const k = (page, js) => page.evaluate(js);
const until = (page, js, timeout = 300_000) => page.waitForFunction(js, null, { timeout, polling: 100 });
/** `n` frames `ms` apart. */
async function take(page, n, ms) {
  for (let i = 0; i < n; i++) {
    await shot(page);
    await page.waitForTimeout(ms);
  }
}

const browser = await chromium.launch({
  args: process.env.PW_GPU
    ? ["--use-angle=d3d11", "--enable-gpu", "--ignore-gpu-blocklist", "--autoplay-policy=no-user-gesture-required"]
    : ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
try {
  await page.goto(url);
  await until(page, "window.__kestrel !== undefined");
  // Voice off (the GIF is silent; clips would only cost time).
  await k(page, "window.__kestrel.getState().setView({ voice: false, sound: false })");
  await until(page, "(document.querySelector('[data-testid=prediction]')?.textContent ?? '').includes(' after ')");
  await take(page, 4, 400);

  // The opening sequence: countdown, liftoff, staging (an illustration).
  await page.keyboard.press("Space");
  await page.waitForTimeout(7_000);
  await take(page, 10, 700); // the last seconds of the count and liftoff
  await take(page, 12, 1_400); // climb and staging
  await until(page, "window.__kestrel.getState().mode === 'flying'", 120_000);

  // The booster's return at ×2: the burn and the catch.
  await k(page, "window.__kestrel.getState().setView({ speed: 2 })");
  await until(page, "window.__kestrel.live.snapshot?.phase === 'landing-burn'", 200_000);
  await take(page, 8, 600);

  // Paused mid-burn: the analysis view, the vehicle as a blueprint, one part open.
  await page.keyboard.press("Space");
  await page.keyboard.press("a");
  await page.waitForTimeout(1_800);
  await take(page, 3, 400);
  await page.getByRole("navigation", { name: "Parts" }).getByRole("button").nth(2).click();
  await page.waitForTimeout(1_500);
  await take(page, 5, 400);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Space");

  // On to the catch.
  await take(page, 10, 600);
  await until(page, "window.__kestrel.getState().mode === 'done'", 200_000);
  await take(page, 3, 500);

  // The ship from orbit: the entry (sped up).
  await page.keyboard.press("r");
  await k(page, "window.__kestrel.getState().setVehicle('ship')");
  await page.getByRole("button", { name: /From orbit/ }).click();
  await until(page, "(document.querySelector('[data-testid=prediction]')?.textContent ?? '').includes('Entry:')", 120_000);
  await page.keyboard.press("Space");
  await page.waitForTimeout(500);
  await page.keyboard.press("Space");
  await until(page, "(window.__kestrel.live.snapshot?.entry?.altitude ?? 1e9) < 80000", 200_000);
  await take(page, 16, 900);
  console.log(`${frame} frames in ${outDir}`);
} finally {
  await browser.close();
}

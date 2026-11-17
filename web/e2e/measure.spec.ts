import { expect, test, type Page } from "@playwright/test";

// @measure: solve times of the planner in the browser, on the real page
// (3D scene rendering). Run with the machine's GPU:
//   PW_GPU=1 npm run measure            (static build)
//   E2E_BASE_URL=http://localhost:3000 PW_GPU=1 npx playwright test --grep @measure
// Each test flies one whole flight at ×1 (opening sequence skipped) and
// prints every plan's solve time, timed in the worker (performance.now()).
// MEASURE_SOLVER=kestrel measures Kestrel's own solver instead of Clarabel.

interface Probe {
  getState(): { mode: string; setVehicle(v: string): void; setOptions(o: Record<string, unknown>): void };
  live: { solveMs: number[]; replans: number };
}

const k = (page: Page) => page.evaluate(() => (window as unknown as { __kestrel?: Probe }).__kestrel !== undefined);

async function flyAndTime(page: Page, vehicle: "booster" | "ship") {
  await page.goto("/");
  await expect.poll(() => k(page), { timeout: 60_000 }).toBe(true);
  if (vehicle === "ship") await page.evaluate(() => (window as unknown as { __kestrel: Probe }).__kestrel.getState().setVehicle("ship"));
  const solver = process.env.MEASURE_SOLVER ?? "clarabel";
  await page.evaluate((s) => (window as unknown as { __kestrel: Probe }).__kestrel.getState().setOptions({ solver: s }), solver);
  await expect(page.getByTestId("prediction")).toContainText(" after ", { timeout: 120_000 });
  await page.keyboard.press("Space");
  await expect.poll(() => page.evaluate(() => (window as unknown as { __kestrel: Probe }).__kestrel.getState().mode)).toBe("launch");
  await page.keyboard.press("Space");
  await expect(page.getByTestId("verdict")).not.toHaveText("", { timeout: 200_000 });
  const verdict = await page.getByTestId("verdict").textContent();
  const ms = await page.evaluate(() => [...(window as unknown as { __kestrel: Probe }).__kestrel.live.solveMs]);
  const sorted = [...ms].sort((a, b) => a - b);
  const q = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))].toFixed(0);
  console.log(
    `${vehicle}, ${solver} (${page.context().browser()?.version()}): ${verdict}; ${ms.length} plans, ` +
      `median ${q(0.5)} ms, p90 ${q(0.9)} ms, max ${q(1)} ms; all: ${ms.map((m) => m.toFixed(0)).join(" ")}`,
  );
}

test("@measure booster plan solve times during a flight", async ({ page }) => {
  await flyAndTime(page, "booster");
});

test("@measure ship plan solve times during a flight", async ({ page }) => {
  await flyAndTime(page, "ship");
});

import { expect, test, type Page } from "@playwright/test";

// End-to-end tests of the one-page console (M3): real browser, real WebAssembly
// planner in its workers, real 3D scene. Flights run at ×1 with the opening
// sequence skipped (Space), so each takes about as long as the flight itself.
// For realistic solve times run with the machine's GPU: `PW_GPU=1 npm run e2e`.

interface Probe {
  getState(): {
    mode: string;
    vehicle: string;
    scenario: string;
    view: { paused: boolean; analysis: boolean; part: string | null };
    flight: { outcome: string | null; phase: string; engines: string[] } | null;
    setView(patch: Record<string, unknown>): void;
    setVehicle(v: string): void;
  };
  live: { snapshot: { t: number; phase: string } | null; replans: number; solveMs: number[]; decisions: unknown[] };
}

const probe = (page: Page) => page.evaluate(() => (window as unknown as { __kestrel: Probe }).__kestrel !== undefined);
const state = <T,>(page: Page, pick: (p: Probe) => T) =>
  page.evaluate(`(${pick.toString()})(window.__kestrel)`) as Promise<T>;

function collectErrors(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => {
    // Missing voice clips in a fresh checkout fall back to speech; not an error of the page.
    if (m.type() === "error" && !/voice\/.*\.mp3/.test(m.text())) errors.push(m.text());
  });
  return errors;
}

async function open(page: Page) {
  await page.goto("/");
  await expect.poll(() => probe(page), { timeout: 60_000 }).toBe(true);
  // The prediction is a whole flight flown ahead of time in the second worker.
  await expect(page.getByTestId("prediction")).toContainText(/caught|Lost/, { timeout: 120_000 });
}

/** Press Fly, skip the opening sequence, and wait for the simulated flight to run. */
async function fly(page: Page) {
  await page.keyboard.press("Space");
  await expect.poll(() => state(page, (k) => k.getState().mode)).toBe("launch");
  await page.keyboard.press("Space");
  await expect.poll(() => state(page, (k) => k.getState().mode)).toBe("flying");
}

test("booster: predicted, flown and caught by the tower", async ({ page }) => {
  const errors = collectErrors(page);
  await open(page);
  await expect(page.getByTestId("prediction")).toContainText("Booster caught by the tower");
  await fly(page);
  await expect(page.getByTestId("verdict")).toHaveText("Booster caught by the tower", { timeout: 150_000 });
  const replans = await state(page, (k) => k.live.replans);
  expect(replans).toBeGreaterThan(3);
  // The flight record kept the decisions guidance made.
  expect(await state(page, (k) => k.live.decisions.length)).toBeGreaterThan(replans);
  await expect(page.getByRole("button", { name: "Fly the ship's return" })).toBeVisible();
  expect(errors).toEqual([]);
});

test("ship: an engine failed mid-burn, a spare relights, caught", async ({ page }) => {
  const errors = collectErrors(page);
  await open(page);
  await page.evaluate(() => (window as unknown as { __kestrel: Probe }).__kestrel.getState().setVehicle("ship"));
  await expect(page.getByTestId("prediction")).toContainText("Ship caught by the tower", { timeout: 120_000 });
  await fly(page);
  // Wait for the single-engine landing burn, then fail the engine that is running.
  await expect.poll(() => state(page, (k) => k.live.snapshot?.phase), { timeout: 150_000, intervals: [250] }).toBe("landing-burn");
  await page.waitForTimeout(3000);
  const failed = await page.evaluate(() => {
    const k = (window as unknown as { __kestrel: Probe }).__kestrel;
    const engines = k.getState().flight?.engines ?? [];
    return engines.findIndex((e) => e === "running");
  });
  expect(failed).toBeGreaterThanOrEqual(0);
  // The engine map on the telemetry rail: clicking an engine fails it.
  await page.getByLabel(`Engine ${failed + 1}: running`).click();
  await expect.poll(() => state(page, (k) => k.getState().flight?.engines.includes("failed"))).toBe(true);
  await expect(page.getByTestId("verdict")).toHaveText("Ship caught by the tower", { timeout: 150_000 });
  expect(errors).toEqual([]);
});

test("pause, analysis view, open a part, step, resume", async ({ page }) => {
  const errors = collectErrors(page);
  await open(page);
  await fly(page);
  await page.waitForTimeout(4000);
  await page.keyboard.press("Space");
  await expect.poll(() => state(page, (k) => k.getState().view.paused)).toBe(true);
  await page.keyboard.press("a");
  await expect.poll(() => state(page, (k) => k.getState().view.analysis)).toBe(true);
  // Open the first part after "Whole vehicle".
  const parts = page.getByRole("navigation", { name: "Parts" }).getByRole("button");
  await parts.nth(1).click();
  await expect.poll(() => state(page, (k) => k.getState().view.part)).not.toBeNull();
  const t0 = await state(page, (k) => k.live.snapshot?.t ?? 0);
  await page.keyboard.press("n");
  await expect.poll(() => state(page, (k) => k.live.snapshot?.t ?? 0)).toBeGreaterThan(t0 + 0.9);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect.poll(() => state(page, (k) => k.getState().view.analysis)).toBe(false);
  await page.keyboard.press("Space");
  await expect.poll(() => state(page, (k) => k.getState().view.paused)).toBe(false);
  expect(errors).toEqual([]);
});

test("reset mid-flight goes back to the pad; a second flight starts cleanly", async ({ page }) => {
  const errors = collectErrors(page);
  await open(page);
  await fly(page);
  await page.waitForTimeout(3000);
  await page.keyboard.press("r");
  await expect.poll(() => state(page, (k) => k.getState().mode)).toBe("editing");
  await fly(page);
  await expect.poll(() => state(page, (k) => k.live.snapshot?.t ?? 0), { timeout: 30_000 }).toBeGreaterThan(2);
  expect(errors).toEqual([]);
});

test("ship from orbit: entry sped up, hand-over at real time, caught", async ({ page }) => {
  const errors = collectErrors(page);
  await open(page);
  await page.evaluate(() => (window as unknown as { __kestrel: Probe }).__kestrel.getState().setVehicle("ship"));
  await page.getByRole("button", { name: /From orbit/ }).click();
  await expect(page.getByTestId("prediction")).toContainText("Entry: tiles peak", { timeout: 120_000 });
  await fly(page);
  await expect.poll(() => state(page, (k) => k.live.snapshot?.phase)).toBe("entry");
  await expect(page.getByTestId("phase")).toHaveText("Entry");
  // The entry ends, and the flight continues in real time.
  await expect.poll(() => state(page, (k) => k.live.snapshot?.phase), { timeout: 150_000, intervals: [500] }).not.toBe("entry");
  await expect.poll(() => state(page, (k) => (k.getState().view as unknown as { speed: number }).speed)).toBe(1);
  await expect(page.getByTestId("verdict")).toHaveText("Ship caught by the tower", { timeout: 300_000 });
  expect(errors).toEqual([]);
});

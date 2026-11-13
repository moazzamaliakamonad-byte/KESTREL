import { expect, test } from "@playwright/test";

// @measure: in-browser solve times (headless Chromium, software WebGL).
// Run with: npm run measure
test("@measure solve time in the browser worker", async ({ page }) => {
  await page.goto("/");
  const status = page.getByTestId("status");
  await expect(status).toHaveText("Ready: press Land", { timeout: 60_000 });
  const times: number[] = [];
  for (let i = 0; i < 10; i++) {
    await page.evaluate((i) => {
      const k = (window as unknown as { __kestrel: { getState(): { dispatch(a: unknown): void } } }).__kestrel;
      k.getState().dispatch({
        type: "setStart",
        start: { position: [2000 + i, 0, 1500], velocity: [-50, 0, -75] },
      });
    }, i);
    await expect(status).toHaveText("Planning…");
    await expect(status).toHaveText("Ready: press Land", { timeout: 60_000 });
    times.push(
      await page.evaluate(
        () => (window as unknown as { __kestrel: { getState(): { lastSolveMs: number } } }).__kestrel.getState().lastSolveMs,
      ),
    );
  }
  times.sort((a, b) => a - b);
  const version = page.context().browser()?.version();
  console.log(
    `browser ${version}: median ${times[5].toFixed(0)} ms, range ${times[0].toFixed(0)}–${times[9].toFixed(0)} ms`,
  );
});

// @measure: latency after a drag (review finding: queued solves made this grow
// with drag length). Eight start changes 140 ms apart, then time to "Ready".
test("@measure re-plan latency after a drag", async ({ page }) => {
  await page.goto("/");
  const status = page.getByTestId("status");
  await expect(status).toHaveText("Ready: press Land", { timeout: 60_000 });
  for (let i = 0; i < 8; i++) {
    await page.evaluate((i) => {
      const k = (window as unknown as { __kestrel: { getState(): { dispatch(a: unknown): void } } }).__kestrel;
      k.getState().dispatch({
        type: "setStart",
        start: { position: [2000 - 40 * i, 0, 1500 + 20 * i], velocity: [-50, 0, -75] },
      });
    }, i);
    await page.waitForTimeout(140);
  }
  const t0 = Date.now();
  const before = await page.evaluate(() => (window as unknown as { __kestrel: { getState(): { latestRequestId: number } } }).__kestrel.getState().latestRequestId);
  await expect(status).toHaveText("Ready: press Land", { timeout: 60_000 });
  const after = await page.evaluate(() => (window as unknown as { __kestrel: { getState(): { latestRequestId: number; lastSolveMs: number } } }).__kestrel.getState());
  console.log(`after-drag latency: ${Date.now() - t0} ms; requests during drag+settle: ${after.latestRequestId} total (was ${before} at release); last solve ${after.lastSolveMs.toFixed(0)} ms`);
});

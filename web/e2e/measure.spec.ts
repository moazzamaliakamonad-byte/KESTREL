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

import { expect, test, type Page } from "@playwright/test";

type Vec3 = [number, number, number];
interface Probe {
  getState(): {
    phase: string;
    start: { position: Vec3; velocity: Vec3 };
    lastSolveMs: number | null;
  };
  screenOf(p: Vec3): { x: number; y: number };
}

const status = (page: Page) => page.getByTestId("status");

function collectErrors(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  return errors;
}

test("loads, plans in the browser and lands", async ({ page }) => {
  const errors = collectErrors(page);
  await page.goto("/");
  await expect(status(page)).toHaveText("Ready: press Land", { timeout: 60_000 });
  await page.getByRole("button", { name: "×4" }).click();
  await page.keyboard.press("Space");
  await expect(status(page)).toHaveText("Landing");
  await expect(status(page)).toHaveText("Touchdown", { timeout: 100_000 });
  await expect(page.getByLabel("Touchdown summary")).toContainText("0.0 m/s");
  expect(errors).toEqual([]);
});

test("dragging the booster changes the start and re-plans", async ({ page }) => {
  const errors = collectErrors(page);
  await page.goto("/");
  await expect(status(page)).toHaveText("Ready: press Land", { timeout: 60_000 });

  const before = await page.evaluate(() => (window as unknown as { __kestrel: Probe }).__kestrel.getState().start.position);
  const at = await page.evaluate(
    (p) => (window as unknown as { __kestrel: Probe }).__kestrel.screenOf(p),
    before,
  );
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  await page.mouse.move(at.x - 120, at.y + 40, { steps: 8 });
  await page.mouse.up();

  const after = await page.evaluate(() => (window as unknown as { __kestrel: Probe }).__kestrel.getState().start.position);
  expect(after[0]).not.toBeCloseTo(before[0], 0);
  await expect(status(page)).toHaveText(/Ready: press Land|No landing exists/, { timeout: 60_000 });
  expect(errors).toEqual([]);
});

test("an impossible start is explained, and Land stays disabled", async ({ page }) => {
  await page.goto("/");
  await expect(status(page)).toHaveText("Ready: press Land", { timeout: 60_000 });
  await page.evaluate(() =>
    (window as unknown as { __kestrel: { getState(): { dispatch(a: unknown): void } } }).__kestrel
      .getState()
      .dispatch({ type: "setStart", start: { position: [2000, 0, 1500], velocity: [0, 0, -150] } }),
  );
  await expect(status(page)).toHaveText(
    "No landing exists from here: too fast, too low or too little propellant.",
    { timeout: 60_000 },
  );
  await expect(page.getByRole("button", { name: "Land" })).toBeDisabled();
});

test("without WebGL the page explains instead of going blank", async ({ page }) => {
  // Simulate a browser/GPU without WebGL.
  await page.addInitScript(() => {
    const original = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (this: HTMLCanvasElement, kind: string, ...rest: unknown[]) {
      if (kind.startsWith("webgl")) return null;
      return (original as (...a: unknown[]) => unknown).call(this, kind, ...rest);
    } as typeof HTMLCanvasElement.prototype.getContext;
  });
  await page.goto("/");
  await expect(page.getByRole("alert").filter({ hasText: "3D view" })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole("button", { name: "Reload" })).toBeVisible();
});

test("a cancelled drag does not leave the page stuck dragging", async ({ page }) => {
  await page.goto("/");
  await expect(status(page)).toHaveText("Ready: press Land", { timeout: 60_000 });
  const pos = await page.evaluate(() => (window as unknown as { __kestrel: Probe }).__kestrel.getState().start.position);
  const at = await page.evaluate((p) => (window as unknown as { __kestrel: Probe }).__kestrel.screenOf(p), pos);
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  await page.mouse.move(at.x - 30, at.y, { steps: 3 });
  expect(await page.evaluate(() => (window as unknown as { __kestrel: { getState(): { dragging: boolean } } }).__kestrel.getState().dragging)).toBe(true);
  // The OS/browser cancels the gesture (e.g. a touch turned into a scroll).
  await page.evaluate(({ x, y }) => {
    const canvas = document.querySelector("canvas")!;
    canvas.dispatchEvent(new PointerEvent("pointercancel", { bubbles: true, pointerId: 1, clientX: x, clientY: y }));
  }, at);
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { __kestrel: { getState(): { dragging: boolean } } }).__kestrel.getState().dragging))
    .toBe(false);
});

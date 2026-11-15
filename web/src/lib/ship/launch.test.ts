import { describe, expect, it } from "vitest";
import { CARD_ONLY, FADE, LAUNCH, LAUNCH_TOTAL, SEPARATION_AT, cardStart, launchState } from "./launch";

describe("launch sequence timing", () => {
  it("counts 5 to 1 on the pad with the engines coming up at the end", () => {
    expect(launchState(0.1)).toMatchObject({ phase: "countdown", count: 5, height: 0, power: 0 });
    expect(launchState(LAUNCH.countdown * 0.5).count).toBe(3);
    const last = launchState(LAUNCH.countdown - 0.1);
    expect(last.count).toBe(1);
    expect(last.power).toBeGreaterThan(0.9);
  });

  it("lifts off slowly, cuts to staging where the stages come apart, shows the card, then ends", () => {
    const early = launchState(LAUNCH.countdown + 1.5);
    const late = launchState(LAUNCH.countdown + LAUNCH.climb - 0.1);
    expect(early.phase).toBe("climb");
    expect(early.height).toBeLessThan(5);
    expect(late.height).toBeGreaterThan(300);
    const joined = launchState(LAUNCH.countdown + LAUNCH.climb + SEPARATION_AT - 0.5);
    expect(joined).toMatchObject({ phase: "staging", apart: 0 });
    const split = launchState(LAUNCH.countdown + LAUNCH.climb + SEPARATION_AT + 2);
    expect(split.apart).toBeCloseTo(2);
    expect(launchState(cardStart(true) + 1).phase).toBe("card");
    expect(launchState(LAUNCH_TOTAL + 0.01).phase).toBe("over");
  });

  it("fades through black at every cut and is clear in between", () => {
    expect(launchState(0).fade).toBe(1);
    expect(launchState(FADE + 0.5).fade).toBe(0);
    const cut = LAUNCH.countdown + LAUNCH.climb;
    expect(launchState(cut - 0.01).fade).toBeGreaterThan(0.95);
    expect(launchState(cut + 0.01).fade).toBeGreaterThan(0.95);
    expect(launchState(cut + 3).fade).toBe(0);
    expect(launchState(cardStart(true) - 0.01).fade).toBeGreaterThan(0.95);
  });

  it("without a stack only the card plays", () => {
    expect(launchState(0.5, false).phase).toBe("card");
    expect(launchState(CARD_ONLY + 0.01, false).phase).toBe("over");
    expect(cardStart(false)).toBe(0);
  });
});

import { describe, expect, it } from "vitest";
import { CARD_ONLY, COUNT_FROM, FADE, IGNITION_BEFORE, LAUNCH, LAUNCH_TOTAL, LEAD_IN, SEPARATION_AT, cardStart, launchCues, launchState } from "./launch";
import { LINES } from "./phrases";

describe("launch sequence timing", () => {
  it("counts ten to one, one number a second, ending exactly at liftoff", () => {
    expect(launchState(0.1)).toMatchObject({ phase: "countdown", count: 0, height: 0, power: 0 });
    for (let n = COUNT_FROM; n >= 1; n--) {
      // Just after the second begins and just before it ends, the number is the same.
      expect(launchState(LAUNCH.countdown - n + 0.01).count).toBe(n);
      expect(launchState(LAUNCH.countdown - n + 0.99).count).toBe(n);
    }
    expect(launchState(LAUNCH.countdown + 0.01).phase).toBe("climb");
  });

  it("the mission clock runs from minus the count through zero at liftoff", () => {
    expect(launchState(LEAD_IN).clock).toBeCloseTo(-COUNT_FROM);
    expect(launchState(LAUNCH.countdown - 3).clock).toBeCloseTo(-3);
    expect(launchState(LAUNCH.countdown + 4).clock).toBeCloseTo(4);
    expect(launchState(cardStart(true) + 1).clock).toBeNull();
  });

  it("the engines come up over the last seconds of the count", () => {
    expect(launchState(LAUNCH.countdown - IGNITION_BEFORE - 0.5).power).toBe(0);
    expect(launchState(LAUNCH.countdown - 0.05).power).toBeGreaterThan(0.95);
  });

  it("every number is spoken on its own second, and nothing talks over the count", () => {
    const cues = launchCues();
    const counts = cues.filter((c) => c.count);
    expect(counts.map((c) => c.count)).toEqual([10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
    for (const c of counts) expect(launchState(c.at + 0.001).count).toBe(c.count);
    const liftoff = cues.find((c) => c.key === "liftoff")!;
    expect(liftoff.at).toBe(LAUNCH.countdown);
    // In order, at least two seconds apart after liftoff, all inside the sequence, all real lines.
    const spoken = cues.filter((c) => c.line);
    for (let i = 1; i < spoken.length; i++) expect(spoken[i].at - spoken[i - 1].at).toBeGreaterThanOrEqual(2);
    for (const c of spoken) {
      expect(c.at).toBeLessThan(cardStart(true));
      expect(LINES[c.line as keyof typeof LINES]).toBeTruthy();
    }
    const staging = cues.find((c) => c.key === "staging")!;
    expect(launchState(staging.at + 0.01).apart).toBeGreaterThan(0);
    expect(launchState(staging.at - 0.01).apart).toBe(0);
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

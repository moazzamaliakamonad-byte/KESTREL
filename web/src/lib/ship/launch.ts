/**
 * The opening sequence before a flight on Earth: countdown, liftoff of the
 * full stack, stage separation at the edge of space, then a title card
 * that skips ahead to the return. It is an illustration, timed by hand:
 * the ascent and staging are not simulated (the screen says so). Pure
 * timing here; the scene and the console read it.
 */

/** Seconds: countdown on the pad, the climb, stage separation high up, the title card. */
export const LAUNCH = { countdown: 6.5, climb: 12, staging: 10, card: 5 } as const;
export const LAUNCH_TOTAL = LAUNCH.countdown + LAUNCH.climb + LAUNCH.staging + LAUNCH.card;
/** Where the staging shot is set, m (about where the real vehicles separate). */
export const STAGING_ALTITUDE = 64_000;
/** Seconds into the staging shot at which the stages part. */
export const SEPARATION_AT = 3.6;
/** Without a stack to launch (Mars, or the ship after the booster), only the card is shown. */
export const CARD_ONLY = LAUNCH.card;
/** The simulated flight starts this long into the card, hidden behind it. */
export const CARD_HANDOVER = 1.6;
/** Length of a fade through black at a cut, s. */
export const FADE = 0.9;

export type LaunchPhase = "countdown" | "climb" | "staging" | "card" | "over";

export interface LaunchState {
  phase: LaunchPhase;
  /** Seconds into the phase. */
  t: number;
  /** Height of the stack above the mount, m (illustrative). */
  height: number;
  /** Engine glow and shake, 0–1. */
  power: number;
  /** The countdown number on screen (5 … 1) or 0. */
  count: number;
  /** Seconds since the stages separated (0 before). */
  apart: number;
  /** Black over the picture, 0–1: cuts fade through black. */
  fade: number;
}

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/** Where the sequence is `elapsed` seconds after it began. `withStack` false = card only. */
export function launchState(elapsed: number, withStack = true): LaunchState {
  const none = { height: 0, power: 0, count: 0, apart: 0, fade: 0 };
  if (!withStack) {
    return elapsed < CARD_ONLY ? { ...none, phase: "card", t: elapsed } : { ...none, phase: "over", t: 0 };
  }
  if (elapsed < LAUNCH.countdown) {
    // Engines spin up over the last two seconds of the count.
    const power = clamp01((elapsed - (LAUNCH.countdown - 2)) / 2);
    const count = Math.min(5, Math.max(1, Math.ceil(5 - (elapsed / LAUNCH.countdown) * 5)));
    return { ...none, phase: "countdown", t: elapsed, power, count, fade: clamp01(1 - elapsed / FADE) };
  }
  const t = elapsed - LAUNCH.countdown;
  // A slow, heavy start (thrust barely beats weight), then gathering speed.
  const rise = (s: number) => 0.9 * s * s + 0.22 * s * s * s;
  if (t < LAUNCH.climb) {
    return { ...none, phase: "climb", t, height: rise(t), power: 1, fade: clamp01((t - (LAUNCH.climb - FADE)) / FADE) };
  }
  // Cut to the edge of space.
  const s = t - LAUNCH.climb;
  const apart = Math.max(0, s - SEPARATION_AT);
  if (s < LAUNCH.staging) {
    const fade = Math.max(clamp01(1 - s / FADE), clamp01((s - (LAUNCH.staging - FADE)) / FADE));
    return { ...none, phase: "staging", t: s, height: STAGING_ALTITUDE, power: 0.5, apart, fade };
  }
  const c = s - LAUNCH.staging;
  if (c < LAUNCH.card) return { ...none, phase: "card", t: c, height: STAGING_ALTITUDE, power: 0.5, apart, fade: 1 };
  return { ...none, phase: "over", t: 0 };
}

/** When the title card starts, s after the sequence began. */
export function cardStart(withStack: boolean): number {
  return withStack ? LAUNCH.countdown + LAUNCH.climb + LAUNCH.staging : 0;
}

/**
 * The opening sequence before a flight: a ten-second countdown, liftoff of
 * the full stack, stage separation at the edge of space, then a title card
 * that skips ahead to the return. It is an illustration, timed by hand:
 * the ascent and staging are not simulated (the screen says so). Pure
 * timing here; the scene, the console and the voice all read this one
 * clock, so the number on screen, the number spoken and the engines agree.
 */

/** Seconds of quiet before the count starts (the picture fades in). */
export const LEAD_IN = 2.5;
/** The count starts here and steps once a second. */
export const COUNT_FROM = 10;
/** Seconds: countdown on the pad, the climb, stage separation high up, the title card. */
export const LAUNCH = { countdown: LEAD_IN + COUNT_FROM, climb: 17, staging: 15, card: 7 } as const;
export const LAUNCH_TOTAL = LAUNCH.countdown + LAUNCH.climb + LAUNCH.staging + LAUNCH.card;
/** Engines start this long before liftoff, s. */
export const IGNITION_BEFORE = 3;
/** Where the staging shot is set, m (about where the real vehicles separate). */
export const STAGING_ALTITUDE = 64_000;
/** Mission time shown when the staging shot begins, s (about when the real vehicles stage). */
export const STAGING_CLOCK = 156;
/** Seconds into the staging shot at which the stages part. */
export const SEPARATION_AT = 6;
/** Without a stack to launch (the ship's return after the booster's), only the card is shown. */
export const CARD_ONLY = LAUNCH.card;
/** The simulated flight starts this long into the card, hidden behind it. */
export const CARD_HANDOVER = 2.2;
/** Length of a fade through black at a cut, s. */
export const FADE = 1.1;

export type LaunchPhase = "countdown" | "climb" | "staging" | "card" | "over";

export interface LaunchState {
  phase: LaunchPhase;
  /** Seconds into the phase. */
  t: number;
  /** Height of the stack above the mount, m (illustrative). */
  height: number;
  /** Engine glow and shake, 0–1. */
  power: number;
  /** The countdown number on screen and spoken (10 … 1), or 0 when none. */
  count: number;
  /** Mission clock, s: negative before liftoff. `null` when the sequence shows none. */
  clock: number | null;
  /** Seconds since the stages separated (0 before). */
  apart: number;
  /** Black over the picture, 0–1: cuts fade through black. */
  fade: number;
}

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/** Where the sequence is `elapsed` seconds after it began. `withStack` false = card only. */
export function launchState(elapsed: number, withStack = true): LaunchState {
  const none = { height: 0, power: 0, count: 0, apart: 0, fade: 0, clock: null };
  if (!withStack) {
    return elapsed < CARD_ONLY ? { ...none, phase: "card", t: elapsed } : { ...none, phase: "over", t: 0 };
  }
  if (elapsed < LAUNCH.countdown) {
    // Mission time: −(lead-in + 10) … 0. The number changes on the second.
    const clock = elapsed - LAUNCH.countdown;
    const count = clock >= -COUNT_FROM ? Math.max(1, Math.ceil(-clock)) : 0;
    // Engines spin up over the last seconds of the count.
    const power = clamp01((clock + IGNITION_BEFORE) / IGNITION_BEFORE);
    return { ...none, phase: "countdown", t: elapsed, power, count, clock, fade: clamp01(1 - elapsed / FADE) };
  }
  const t = elapsed - LAUNCH.countdown;
  // A slow, heavy start (thrust barely beats weight), then gathering speed.
  const rise = (s: number) => 0.55 * s * s + 0.1 * s * s * s;
  if (t < LAUNCH.climb) {
    return { ...none, phase: "climb", t, height: rise(t), power: 1, clock: t, fade: clamp01((t - (LAUNCH.climb - FADE)) / FADE) };
  }
  // Cut to the edge of space.
  const s = t - LAUNCH.climb;
  const apart = Math.max(0, s - SEPARATION_AT);
  if (s < LAUNCH.staging) {
    const fade = Math.max(clamp01(1 - s / FADE), clamp01((s - (LAUNCH.staging - FADE)) / FADE));
    return { ...none, phase: "staging", t: s, height: STAGING_ALTITUDE, power: 0.5, apart, fade, clock: STAGING_CLOCK + s };
  }
  const c = s - LAUNCH.staging;
  if (c < LAUNCH.card) return { ...none, phase: "card", t: c, height: STAGING_ALTITUDE, power: 0.5, apart, fade: 1 };
  return { ...none, phase: "over", t: 0 };
}

/** When the title card starts, s after the sequence began. */
export function cardStart(withStack: boolean): number {
  return withStack ? LAUNCH.countdown + LAUNCH.climb + LAUNCH.staging : 0;
}

/** One thing the sequence says or shows at a fixed moment. */
export interface Cue {
  /** Seconds after the sequence began. */
  at: number;
  key: string;
  /** A key of `LINES` (phrases.ts), or a countdown number to speak. */
  line?: string;
  count?: number;
  urgent?: boolean;
}

/**
 * Everything spoken in the opening sequence, in order, each with room to
 * finish before the next (the old sequence talked over itself).
 */
export function launchCues(): Cue[] {
  const cues: Cue[] = [];
  for (let n = COUNT_FROM; n >= 1; n--) cues.push({ at: LAUNCH.countdown - n, key: `count${n}`, count: n, urgent: true });
  const lift = LAUNCH.countdown;
  cues.push({ at: lift, key: "liftoff", line: "liftoff", urgent: true });
  cues.push({ at: lift + 7.5, key: "cleared", line: "cleared" });
  const stage = lift + LAUNCH.climb;
  cues.push({ at: stage + 1.6, key: "throttling", line: "throttling" });
  cues.push({ at: stage + SEPARATION_AT, key: "staging", line: "staging", urgent: true });
  cues.push({ at: stage + SEPARATION_AT + 4.5, key: "home", line: "home" });
  return cues;
}

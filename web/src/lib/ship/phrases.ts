/**
 * Every line the commentator can say. The list is finite on purpose: each
 * line is rendered once to an audio clip with a neural voice
 * (`scripts/make-voice.mjs`) and played back instantly, instead of being
 * synthesized by whatever voice the visitor's computer has. This module
 * has no imports, so the script can load it as it is.
 */

const NUMBER = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen"];
const word = (n: number) => NUMBER[n] ?? String(n);
const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export const LINES = {
  // Opening sequence.
  countdown: "Five. Four. Three. Two. One.",
  liftoff: "Liftoff.",
  cleared: "The stack has cleared the tower.",
  throttling: "Booster engines are throttling down for staging.",
  staging: "Stage separation. The ship is away.",
  home: "The booster turns for home.",
  // Opening lines of a flight.
  boosterOpen: "Booster is falling back engines first. Grid fins are steering it toward the tower.",
  shipEntry: "Ship is in entry, belly to the wind.",
  shipMars: "Ship is descending toward the Martian surface, belly first.",
  shipOpen: "Ship is in the belly flop. Four flaps are holding it steady.",
  // Phases.
  startup: "Landing burn start-up.",
  flip: "Engines are lighting. Here comes the flip.",
  shipBurn: "Ship is upright. Landing burn.",
  finalMars: "Final descent. Legs are down.",
  finalTower: "Final approach. Sliding in over the arms.",
  outerRing: "Shutdown of the outer ring. Three centre engines continue.",
  // Calls on the way down.
  km50: "Fifty kilometres.",
  km20: "Twenty kilometres.",
  km10: "Ten kilometres.",
  km5: "Five kilometres.",
  km1: "One kilometre.",
  boosterSubsonic: "Booster is subsonic.",
  shipSubsonic: "Ship is subsonic.",
  tenSeconds: "Ten seconds to the landing burn.",
  heating: "Heating is building. Plasma on the heat shield.",
  peakHeating: "Through peak heating.",
  arms: "Tower arms are closing.",
  braking: "No landing plan from here. Braking.",
  gust: "Wind gust.",
  // Outcomes.
  boosterCaught: "Catch confirmed. Booster is on the tower.",
  shipCaught: "Catch confirmed. Ship is on the tower.",
  landed: "Touchdown. The ship is standing on Mars.",
} as const;

/** The simulator's reasons for losing a vehicle (`CrashReason::as_str`). */
export const CRASH_REASONS = ["hit the ground", "hit the tower", "missed the arms", "too fast", "tipped over", "out of propellant"] as const;

export const engineOut = (index: number) => `Engine ${word(index + 1)} is out. Guidance is re-planning.`;
export const enginesRunning = (n: number) => `${cap(word(n))} engine${n === 1 ? "" : "s"} running.`;
export const downTo = (n: number) => `Down to ${word(n)} engine${n === 1 ? "" : "s"}.`;
export const lost = (reason: string | null) => {
  const r = reason ?? "crashed";
  if (r === "too fast") return "We have lost the vehicle. It came in too fast.";
  if (r === "out of propellant") return "We have lost the vehicle. It ran out of propellant.";
  if (r === "crashed") return "We have lost the vehicle.";
  return `We have lost the vehicle. It ${r}.`;
};

/** Every line that can be spoken: the clips to render. */
export function allPhrases(): string[] {
  const out: string[] = Object.values(LINES);
  for (let i = 0; i < 13; i++) out.push(engineOut(i));
  for (let n = 1; n <= 13; n++) out.push(enginesRunning(n));
  for (let n = 1; n <= 12; n++) out.push(downTo(n));
  for (const r of CRASH_REASONS) out.push(lost(r));
  out.push(lost(null));
  return [...new Set(out)];
}

/** File name (without extension) of a line's clip: readable, and unique by a short hash. */
export function clipId(text: string): string {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  const slug = text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 40);
  return `${slug}-${(h >>> 0).toString(36)}`;
}

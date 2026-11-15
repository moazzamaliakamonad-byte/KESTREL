/**
 * Live commentary: what a launch-webcast commentator would say about this
 * flight, decided from two consecutive snapshots (so every line is caused
 * by something that actually happened in the simulation), and spoken with
 * the browser's built-in speech voices. No recordings, no network.
 */

import type { ScenarioKind, ShipSnapshot } from "./types";

export interface Callout {
  text: string;
  /** Urgent lines interrupt whatever is being said. */
  urgent?: boolean;
}

const NUMBER = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen"];
const say = (n: number) => NUMBER[n] ?? String(n);

const lit = (s: ShipSnapshot) => s.engines.filter((e) => e === "running" || e === "igniting").length;
const failed = (s: ShipSnapshot) => s.engines.filter((e) => e === "failed").length;
const unpowered = (s: ShipSnapshot) => s.phase === "belly-flop" || s.phase === "descent";

/** Altitudes called on the way down, m. */
const ALTITUDES: [number, string][] = [
  [50_000, "Fifty kilometres."],
  [20_000, "Twenty kilometres."],
  [10_000, "Ten kilometres."],
  [5000, "Five kilometres."],
  [1000, "One kilometre."],
];

/** Heating (W/m²) above which the plasma is worth mentioning, and below which it is over. */
export const HEATING_ON = 60_000;
export const HEATING_OFF = 25_000;

/** Seconds until the engines light, at the present sink rate. */
export function timeToIgnition(s: ShipSnapshot): number {
  if (!unpowered(s) || s.velocity[2] >= -1) return Infinity;
  return Math.max(0, (s.position[2] - s.ignition_z) / -s.velocity[2]);
}

/** What to say about the step from `prev` to `next` (`prev` null = the flight has just begun). */
export function callouts(kind: ScenarioKind, prev: ShipSnapshot | null, next: ShipSnapshot): Callout[] {
  const out: Callout[] = [];
  const name = kind === "booster" ? "Booster" : "Ship";
  if (!prev) {
    if (kind === "booster") out.push({ text: "Booster is falling back engines first. Grid fins are steering it toward the tower." });
    else if (next.mach > 4) out.push({ text: `Ship is in entry at Mach ${Math.round(next.mach)}, belly to the wind.` });
    else if (kind === "mars") out.push({ text: "Ship is descending toward the Martian surface, belly first." });
    else out.push({ text: "Ship is in the belly flop. Four flaps are holding it steady." });
    return out;
  }

  if (next.outcome && !prev.outcome) {
    if (next.outcome === "caught") out.push({ text: `Catch confirmed. ${name} is on the tower.`, urgent: true });
    else if (next.outcome === "landed") out.push({ text: "Touchdown. The ship is standing on Mars.", urgent: true });
    else out.push({ text: `We have lost the vehicle. It ${next.reason ?? "crashed"}.`, urgent: true });
    return out;
  }

  const newlyFailed = next.engines.findIndex((e, i) => e === "failed" && prev.engines[i] !== "failed");
  if (failed(next) > failed(prev) && newlyFailed >= 0) {
    out.push({ text: `Engine ${say(newlyFailed + 1)} is out. Guidance is re-planning.`, urgent: true });
  }

  if (next.phase !== prev.phase) {
    switch (next.phase) {
      case "startup":
        out.push({ text: "Landing burn start-up.", urgent: true });
        break;
      case "flip":
        out.push({ text: "Engines are lighting. Here comes the flip.", urgent: true });
        break;
      case "landing-burn":
        out.push({ text: kind === "booster" ? `${say(lit(next))} engines running.` : "Ship is upright. Landing burn." });
        break;
      case "terminal":
        out.push({ text: kind === "mars" ? "Final descent. Legs are down." : "Final approach. Sliding in over the arms." });
        break;
      default:
        break;
    }
  } else if (lit(next) < lit(prev) && failed(next) === failed(prev) && lit(next) > 0) {
    // The flight software shut engines down (not a failure).
    if (kind === "booster" && lit(prev) > 3 && lit(next) <= 3) out.push({ text: "Shutdown of the outer ring. Three centre engines continue." });
    else out.push({ text: `Down to ${say(lit(next))} engine${lit(next) === 1 ? "" : "s"}.` });
  }

  if (unpowered(next)) {
    for (const [z, text] of ALTITUDES) {
      if (prev.position[2] > z && next.position[2] <= z) out.push({ text });
    }
    if (prev.mach >= 1 && next.mach < 1) out.push({ text: `${name} is subsonic.` });
    if (timeToIgnition(prev) > 10 && timeToIgnition(next) <= 10) out.push({ text: "Ten seconds to the landing burn." });
  }
  if (prev.heat_flux < HEATING_ON && next.heat_flux >= HEATING_ON) out.push({ text: "Heating is building. Plasma on the heat shield." });
  if (prev.heat_flux > HEATING_OFF && next.heat_flux <= HEATING_OFF && prev.heat_flux < HEATING_ON) {
    out.push({ text: "Through peak heating." });
  }
  if (!prev.arms && next.arms) out.push({ text: "Tower arms are closing." });
  if (prev.guide !== "brake" && next.guide === "brake") out.push({ text: "No landing plan from here. Braking.", urgent: true });
  return out;
}

/** Voices that sound like a webcast host, best first. */
const PREFERRED = [/natural.*(aria|jenny|ava|emma|sonia|libby|michelle)/i, /(aria|jenny|ava|emma|sonia|libby|michelle).*natural/i, /samantha/i, /google us english/i, /google uk english female/i, /zira/i, /female/i];

export function pickVoice(voices: { name: string; lang: string }[]): number {
  const english = voices.map((v, i) => ({ v, i })).filter(({ v }) => v.lang.toLowerCase().startsWith("en"));
  for (const pattern of PREFERRED) {
    const hit = english.find(({ v }) => pattern.test(v.name));
    if (hit) return hit.i;
  }
  return english[0]?.i ?? (voices.length ? 0 : -1);
}

/** Speaks callouts; keeps the latest line for the on-screen caption. */
export class Commentator {
  caption: { text: string; at: number } | null = null;
  enabled = true;
  private pending: string | null = null;

  private get synth(): SpeechSynthesis | null {
    return typeof window !== "undefined" && "speechSynthesis" in window ? window.speechSynthesis : null;
  }

  say(lines: Callout[], rate = 1) {
    for (const line of lines) {
      this.caption = { text: line.text, at: performance.now() };
      const synth = this.synth;
      if (!synth || !this.enabled) continue;
      if (line.urgent) {
        synth.cancel();
        this.pending = null;
        this.speak(line.text, rate);
      } else if (synth.speaking) {
        // One line waits at most; older small talk is dropped, not queued up.
        this.pending = line.text;
      } else {
        this.speak(line.text, rate);
      }
    }
  }

  private speak(text: string, rate: number) {
    const synth = this.synth;
    if (!synth) return;
    const u = new SpeechSynthesisUtterance(text);
    const voices = synth.getVoices();
    const i = pickVoice(voices);
    if (i >= 0) u.voice = voices[i];
    u.rate = Math.min(1.6, Math.max(0.9, 1.02 * rate));
    u.pitch = 1.05;
    u.onend = () => {
      const next = this.pending;
      this.pending = null;
      if (next && this.enabled) this.speak(next, rate);
    };
    synth.speak(u);
  }

  stop() {
    this.pending = null;
    this.caption = null;
    this.synth?.cancel();
  }
}

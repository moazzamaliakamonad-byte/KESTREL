/**
 * Live commentary: what a launch-webcast commentator would say about this
 * flight, decided from two consecutive snapshots (so every line is caused
 * by something that actually happened in the simulation).
 *
 * Each line is one of a fixed list (`phrases.ts`) with a clip rendered in
 * advance by a neural voice, so it plays at once and sounds human. If a
 * clip is missing the browser's own speech voice says the line instead.
 */

import { LINES, clipId, downTo, engineOut, enginesRunning, lost } from "./phrases";
import type { ScenarioKind, ShipSnapshot } from "./types";

export interface Callout {
  text: string;
  /** Urgent lines interrupt whatever is being said. */
  urgent?: boolean;
  /** Spoken but not captioned (the countdown has its own display). */
  quiet?: boolean;
}

const lit = (s: ShipSnapshot) => s.engines.filter((e) => e === "running" || e === "igniting").length;
const failed = (s: ShipSnapshot) => s.engines.filter((e) => e === "failed").length;
const unpowered = (s: ShipSnapshot) => s.phase === "belly-flop" || s.phase === "descent";

/** Altitudes called on the way down, m. */
const ALTITUDES: [number, string][] = [
  [50_000, LINES.km50],
  [20_000, LINES.km20],
  [10_000, LINES.km10],
  [5000, LINES.km5],
  [1000, LINES.km1],
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
  const booster = kind === "booster";
  if (!prev) {
    if (booster) out.push({ text: LINES.boosterOpen });
    else if (next.mach > 4) out.push({ text: LINES.shipEntry });
    else out.push({ text: LINES.shipOpen });
    return out;
  }

  if (next.outcome && !prev.outcome) {
    if (next.outcome === "caught") out.push({ text: booster ? LINES.boosterCaught : LINES.shipCaught, urgent: true });
    else out.push({ text: lost(next.reason), urgent: true });
    return out;
  }

  const newlyFailed = next.engines.findIndex((e, i) => e === "failed" && prev.engines[i] !== "failed");
  if (failed(next) > failed(prev) && newlyFailed >= 0) out.push({ text: engineOut(newlyFailed), urgent: true });
  // A shut-down engine lighting again: the flight replacing a failed one.
  if (next.engines.some((e, i) => e === "igniting" && prev.engines[i] === "shutdown")) out.push({ text: LINES.relight });

  if (next.phase !== prev.phase) {
    switch (next.phase) {
      case "startup":
        out.push({ text: LINES.startup, urgent: true });
        break;
      case "flip":
        out.push({ text: LINES.flip, urgent: true });
        break;
      case "landing-burn":
        out.push({ text: booster ? enginesRunning(lit(next)) : LINES.shipBurn });
        break;
      case "terminal":
        out.push({ text: LINES.finalTower });
        break;
      default:
        break;
    }
  } else if (lit(next) < lit(prev) && failed(next) === failed(prev) && lit(next) > 0) {
    // The flight software shut engines down (not a failure).
    if (booster && lit(prev) > 3 && lit(next) <= 3) out.push({ text: LINES.outerRing });
    else out.push({ text: downTo(lit(next)) });
  }

  if (unpowered(next)) {
    for (const [z, text] of ALTITUDES) {
      if (prev.position[2] > z && next.position[2] <= z) out.push({ text });
    }
    if (prev.mach >= 1 && next.mach < 1) out.push({ text: booster ? LINES.boosterSubsonic : LINES.shipSubsonic });
    if (timeToIgnition(prev) > 10 && timeToIgnition(next) <= 10) out.push({ text: LINES.tenSeconds });
  }
  if (prev.heat_flux < HEATING_ON && next.heat_flux >= HEATING_ON) out.push({ text: LINES.heating });
  if (prev.heat_flux > HEATING_OFF && next.heat_flux <= HEATING_OFF && prev.heat_flux < HEATING_ON) out.push({ text: LINES.peakHeating });
  if (!prev.arms && next.arms) out.push({ text: LINES.arms });
  if (prev.guide !== "brake" && next.guide === "brake") out.push({ text: LINES.braking, urgent: true });
  return out;
}

/** Voices that sound like a webcast host, best first (the fallback when a clip is missing). */
const PREFERRED = [/natural.*(aria|jenny|ava|emma|sonia|libby|michelle)/i, /(aria|jenny|ava|emma|sonia|libby|michelle).*natural/i, /samantha/i, /google us english/i, /google uk english female/i, /zira/i, /female/i];

export function pickVoice(voices: { name: string; lang: string }[]): number {
  const english = voices.map((v, i) => ({ v, i })).filter(({ v }) => v.lang.toLowerCase().startsWith("en"));
  for (const pattern of PREFERRED) {
    const hit = english.find(({ v }) => pattern.test(v.name));
    if (hit) return hit.i;
  }
  return english[0]?.i ?? (voices.length ? 0 : -1);
}

/** Where the rendered clips are served from (relative, so a site base path works). */
const VOICE_DIR = "voice";

/**
 * Says callouts: the pre-rendered clip when there is one, the browser's
 * speech voice otherwise. One line may wait behind the one being said;
 * older small talk is dropped rather than queued up. Keeps the latest line
 * for the on-screen caption.
 */
export class Commentator {
  caption: { text: string; at: number } | null = null;
  enabled = true;
  private pending: string | null = null;
  private clips: Set<string> | null = null;
  private loading = false;
  private playing: HTMLAudioElement | null = null;
  private cache = new Map<string, HTMLAudioElement>();

  private get synth(): SpeechSynthesis | null {
    return typeof window !== "undefined" && "speechSynthesis" in window ? window.speechSynthesis : null;
  }

  /** Fetch the list of rendered clips (once). Lines said before it arrives use the fallback voice. */
  load() {
    if (this.clips || this.loading || typeof fetch === "undefined") return;
    this.loading = true;
    fetch(`${VOICE_DIR}/manifest.json`)
      .then((r) => (r.ok ? r.json() : []))
      .then((ids: unknown) => {
        this.clips = new Set(Array.isArray(ids) ? (ids as string[]) : []);
      })
      .catch(() => {
        this.clips = new Set();
      });
  }

  /** Fetch these lines' clips now, so they start the instant they are called (the countdown). */
  preload(texts: string[]) {
    if (typeof Audio === "undefined") return;
    for (const text of texts) {
      const id = clipId(text);
      if (this.cache.has(id) || (this.clips && !this.clips.has(id))) continue;
      const audio = new Audio(`${VOICE_DIR}/${id}.mp3`);
      audio.preload = "auto";
      audio.load();
      this.cache.set(id, audio);
    }
  }

  private get busy(): boolean {
    return (!!this.playing && !this.playing.ended && !this.playing.paused) || !!this.synth?.speaking;
  }

  say(lines: Callout[], rate = 1) {
    for (const line of lines) {
      if (!line.quiet) this.caption = { text: line.text, at: performance.now() };
      if (!this.enabled || typeof window === "undefined") continue;
      if (line.urgent) {
        this.silence();
        this.speak(line.text, rate);
      } else if (this.busy) {
        this.pending = line.text;
      } else {
        this.speak(line.text, rate);
      }
    }
  }

  private next(rate: number) {
    const text = this.pending;
    this.pending = null;
    if (text && this.enabled) this.speak(text, rate);
  }

  private speak(text: string, rate: number) {
    const id = clipId(text);
    if (this.clips?.has(id)) {
      let audio = this.cache.get(id);
      if (!audio) {
        audio = new Audio(`${VOICE_DIR}/${id}.mp3`);
        this.cache.set(id, audio);
      }
      audio.currentTime = 0;
      // A little quicker at high sim speed, never chipmunk-fast.
      audio.playbackRate = Math.min(1.3, Math.max(1, 0.9 + 0.1 * rate));
      audio.onended = () => this.next(rate);
      this.playing = audio;
      audio.play().catch(() => this.fallback(text, rate));
      return;
    }
    this.fallback(text, rate);
  }

  private fallback(text: string, rate: number) {
    const synth = this.synth;
    if (!synth) return;
    const u = new SpeechSynthesisUtterance(text);
    const voices = synth.getVoices();
    const i = pickVoice(voices);
    if (i >= 0) u.voice = voices[i];
    u.rate = Math.min(1.6, Math.max(0.9, 1.02 * rate));
    u.pitch = 1.05;
    u.onend = () => this.next(rate);
    synth.speak(u);
  }

  private silence() {
    this.pending = null;
    this.playing?.pause();
    this.playing = null;
    this.synth?.cancel();
  }

  stop() {
    this.silence();
    this.caption = null;
  }
}

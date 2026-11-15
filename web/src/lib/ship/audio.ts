/**
 * Flight sound, synthesized in the browser (Web Audio; no recordings).
 *
 * What is heard depends on where the camera is, as it would for a
 * microphone there:
 * - loudness falls with distance and the air swallows the high
 *   frequencies first, so a distant burn is a low rumble;
 * - sound takes time to arrive (343 m/s): from a ground camera a
 *   kilometre away the engines are heard three seconds after they light;
 * - onboard, the engines come through the structure (deep, immediate) and
 *   the airflow roars with dynamic pressure;
 * - a ground camera hears the double sonic boom when the vehicle goes
 *   subsonic, again delayed by distance.
 *
 * The mixing rule (`mix`) is a pure function so it can be tested; the
 * class only turns its numbers into audio nodes.
 */

export const SPEED_OF_SOUND = 343;

export interface Hearing {
  /** Total engine thrust as a share of the vehicle's landing engines at full, 0–1.5. */
  power: number;
  /** Engines running. */
  engines: number;
  /** Camera to vehicle, m. */
  distance: number;
  /** The camera rides on the vehicle. */
  onboard: boolean;
  /** Dynamic pressure, Pa. */
  q: number;
  /** Air density relative to sea level, 0–1 (no air, no sound through it). */
  air: number;
  paused: boolean;
}

export interface Mix {
  /** Engine roar gain, 0–1. */
  roar: number;
  /** Low-pass cutoff of the roar, Hz. */
  cutoff: number;
  /** Crackle (the tearing sound close to a big jet) gain, 0–1. */
  crackle: number;
  /** Deep rumble (sub-bass) gain, 0–1. */
  rumble: number;
  /** Airflow gain, 0–1, and its centre frequency, Hz. */
  wind: number;
  windHz: number;
  /** Seconds the sound takes to reach the camera. */
  delay: number;
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** What a microphone at the camera would get. */
export function mix(h: Hearing): Mix {
  const silent: Mix = { roar: 0, cutoff: 200, crackle: 0, rumble: 0, wind: 0, windHz: 300, delay: 0 };
  if (h.paused) return silent;
  const loud = Math.pow(clamp(h.power, 0, 1.5), 0.6) * (h.engines > 0 ? 1 : 0);
  if (h.onboard) {
    // Through the structure: immediate, deep, and the air rushing past.
    const wind = clamp(Math.sqrt(h.q / 60_000), 0, 1);
    return { roar: 0.5 * loud, cutoff: 260, crackle: 0, rumble: 0.9 * loud, wind: 0.85 * wind, windHz: 250 + 900 * wind, delay: 0 };
  }
  // Through the air: needs air around the vehicle to carry it.
  const carried = clamp(h.air * 3, 0, 1);
  const near = 300 / (300 + h.distance);
  const cutoff = clamp(16_000 * Math.exp(-h.distance / 1400), 160, 16_000);
  const wind = clamp(Math.sqrt(h.q / 60_000), 0, 1) * clamp(400 / (400 + h.distance), 0, 1);
  return {
    roar: clamp(loud * near * 1.6 * carried, 0, 1),
    cutoff,
    crackle: clamp(loud * Math.pow(near, 1.5) * 1.2 * carried, 0, 0.8),
    rumble: clamp(loud * Math.sqrt(near) * 0.8 * carried, 0, 1),
    wind: 0.5 * wind,
    windHz: 300 + 500 * wind,
    delay: clamp(h.distance / SPEED_OF_SOUND, 0, 12),
  };
}

function noiseBuffer(ctx: AudioContext, brown: boolean): AudioBuffer {
  const n = ctx.sampleRate * 3;
  const buffer = ctx.createBuffer(1, n, ctx.sampleRate);
  const data = buffer.getChannelData(0);
  let last = 0;
  for (let i = 0; i < n; i++) {
    const white = Math.random() * 2 - 1;
    if (brown) {
      last = (last + 0.02 * white) / 1.02;
      data[i] = last * 3.5;
    } else data[i] = white;
  }
  return buffer;
}

/** The audio graph. Create it from a user gesture (browsers require one). */
export class FlightAudio {
  private ctx: AudioContext | null = null;
  private master!: GainNode;
  private roar!: GainNode;
  private roarFilter!: BiquadFilterNode;
  private crackle!: GainNode;
  private rumble!: GainNode;
  private wind!: GainNode;
  private windFilter!: BiquadFilterNode;
  private delay!: DelayNode;
  private lastDelay = 0;
  enabled = true;

  /** Start (or resume) audio. Safe to call again. */
  start() {
    if (typeof window === "undefined") return;
    if (this.ctx) {
      void this.ctx.resume();
      return;
    }
    const Ctx = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!Ctx) return;
    const ctx = new Ctx();
    this.ctx = ctx;
    this.master = ctx.createGain();
    this.master.gain.value = 0.7;
    const limiter = ctx.createDynamicsCompressor();
    limiter.threshold.value = -10;
    limiter.ratio.value = 8;
    this.master.connect(limiter).connect(ctx.destination);

    // Everything that travels through the air goes through one delay line.
    this.delay = ctx.createDelay(13);
    this.delay.connect(this.master);

    const brown = noiseBuffer(ctx, true);
    const white = noiseBuffer(ctx, false);
    const loop = (buffer: AudioBuffer) => {
      const src = ctx.createBufferSource();
      src.buffer = buffer;
      src.loop = true;
      src.start();
      return src;
    };

    this.roarFilter = ctx.createBiquadFilter();
    this.roarFilter.type = "lowpass";
    this.roar = ctx.createGain();
    this.roar.gain.value = 0;
    loop(brown).connect(this.roarFilter).connect(this.roar).connect(this.delay);

    // Crackle: band-passed white noise chopped by a fast random-ish tremolo.
    const band = ctx.createBiquadFilter();
    band.type = "bandpass";
    band.frequency.value = 1500;
    band.Q.value = 0.6;
    const chop = ctx.createGain();
    chop.gain.value = 0.5;
    const lfo = ctx.createOscillator();
    lfo.type = "sawtooth";
    lfo.frequency.value = 23;
    const lfoGain = ctx.createGain();
    lfoGain.gain.value = 0.5;
    lfo.connect(lfoGain).connect(chop.gain);
    lfo.start();
    this.crackle = ctx.createGain();
    this.crackle.gain.value = 0;
    loop(white).connect(band).connect(chop).connect(this.crackle).connect(this.delay);

    // Sub-bass: two detuned low oscillators and filtered noise.
    this.rumble = ctx.createGain();
    this.rumble.gain.value = 0;
    for (const f of [31, 47]) {
      const o = ctx.createOscillator();
      o.type = "triangle";
      o.frequency.value = f;
      const g = ctx.createGain();
      g.gain.value = 0.35;
      o.connect(g).connect(this.rumble);
      o.start();
    }
    const low = ctx.createBiquadFilter();
    low.type = "lowpass";
    low.frequency.value = 90;
    loop(brown).connect(low).connect(this.rumble);
    this.rumble.connect(this.delay);

    // Airflow is heard where the camera is: no travel delay.
    this.windFilter = ctx.createBiquadFilter();
    this.windFilter.type = "bandpass";
    this.windFilter.Q.value = 0.5;
    this.wind = ctx.createGain();
    this.wind.gain.value = 0;
    loop(white).connect(this.windFilter).connect(this.wind).connect(this.master);
  }

  /** Follow the scene. Call every frame. */
  update(h: Hearing) {
    const ctx = this.ctx;
    if (!ctx) return;
    const m = this.enabled ? mix(h) : mix({ ...h, paused: true });
    const t = ctx.currentTime;
    const smooth = 0.08;
    this.roar.gain.setTargetAtTime(m.roar * 0.9, t, smooth);
    this.roarFilter.frequency.setTargetAtTime(m.cutoff, t, smooth);
    this.crackle.gain.setTargetAtTime(m.crackle * 0.25, t, smooth);
    this.rumble.gain.setTargetAtTime(m.rumble * 0.8, t, smooth);
    this.wind.gain.setTargetAtTime(m.wind * 0.35, t, 0.15);
    this.windFilter.frequency.setTargetAtTime(m.windHz, t, 0.15);
    // A camera cut changes the distance at once: jump the delay (sliding
    // it would bend the pitch); small changes glide.
    if (Math.abs(m.delay - this.lastDelay) > 0.25) this.delay.delayTime.setValueAtTime(m.delay, t);
    else this.delay.delayTime.setTargetAtTime(m.delay, t, 0.3);
    this.lastDelay = m.delay;
  }

  /** A low thump after `after` seconds: one half of a sonic boom, or a hard contact. */
  thump(after: number, gain: number, pitch = 55) {
    const ctx = this.ctx;
    if (!ctx || !this.enabled) return;
    const t = ctx.currentTime + Math.max(0, after);
    const o = ctx.createOscillator();
    o.type = "sine";
    o.frequency.setValueAtTime(pitch, t);
    o.frequency.exponentialRampToValueAtTime(pitch * 0.45, t + 0.35);
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(clamp(gain, 0.001, 1), t + 0.012);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 0.5);
    o.connect(g).connect(this.master);
    o.start(t);
    o.stop(t + 0.6);
  }

  /** The double boom of a vehicle going subsonic, heard `distance` metres away. */
  sonicBoom(distance: number) {
    const after = distance / SPEED_OF_SOUND;
    const gain = clamp(900 / (900 + distance), 0.15, 0.9);
    this.thump(after, gain, 70);
    this.thump(after + 0.14, gain * 0.9, 62);
  }

  /** Steel on steel: the catch. */
  clunk(after: number) {
    const ctx = this.ctx;
    if (!ctx || !this.enabled) return;
    this.thump(after, 0.6, 90);
    const t = ctx.currentTime + Math.max(0, after);
    for (const f of [212, 331, 547]) {
      const o = ctx.createOscillator();
      o.type = "sine";
      o.frequency.value = f;
      const g = ctx.createGain();
      g.gain.setValueAtTime(0.0001, t);
      g.gain.exponentialRampToValueAtTime(0.12, t + 0.01);
      g.gain.exponentialRampToValueAtTime(0.0001, t + 1.6);
      o.connect(g).connect(this.master);
      o.start(t);
      o.stop(t + 1.7);
    }
  }

  /** The end of a failed flight. */
  explosion(after: number) {
    const ctx = this.ctx;
    if (!ctx || !this.enabled) return;
    this.thump(after, 1, 45);
    const t = ctx.currentTime + Math.max(0, after);
    const src = ctx.createBufferSource();
    src.buffer = noiseBuffer(ctx, true);
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(0.9, t + 0.03);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 2.6);
    src.connect(g).connect(this.master);
    src.start(t);
    src.stop(t + 2.8);
  }

  /** The node speech clips should play into (so one volume governs all), if started. */
  get output(): AudioNode | null {
    return this.ctx ? this.master : null;
  }

  get context(): AudioContext | null {
    return this.ctx;
  }
}

export const flightAudio = new FlightAudio();

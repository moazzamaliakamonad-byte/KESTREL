import { create } from "zustand";
import type { Vec3 } from "./frames";
import type {
  EngineState,
  FlightConfig,
  Outcome,
  Preview,
  ScenarioInfo,
  ScenarioKind,
  ShipPhase,
  Tuning,
  Vehicle,
} from "./types";

/** `launch` = the opening sequence (illustration) before the simulated flight. */
export type Mode = "editing" | "launch" | "flying" | "done";
/** What the opening sequence shows: the full stack lifting off and staging, or only a title card. */
export type Intro = "stack" | "card";
export type Noise = "off" | "low" | "high";
export type CameraMode = "director" | "chase" | "track" | "onboard" | "tower" | "orbit";

export interface Start {
  position: Vec3;
  velocity: Vec3;
}

/** What the HUD shows that changes rarely (per-frame numbers live in `shipLive`). */
export interface FlightView {
  phase: ShipPhase;
  engines: EngineState[];
  guide: "plan" | "none" | "brake";
  legEngines: number;
  outcome: Outcome | null;
  reason: string | null;
}

/** Everything a visitor can change about the vehicle and its surroundings. */
export interface Options {
  /** Landing propellant, t (null = the scenario's own). */
  propellant: number | null;
  /** Landing engines that never light. */
  failed: number[];
  gimbal: number | null;
  throttleMin: number | null;
  isp: number | null;
  ignition: number | null;
  /** Air-density multiplier. */
  density: number;
  /** Steady wind, m/s, blowing toward `windDir` (degrees from +x toward +y). */
  windSpeed: number;
  windDir: number;
  gustSpeed: number;
  gustDir: number;
  /** Throttle lag, s. */
  lag: number;
  noise: Noise;
  /** Fixed random seed, or null for a new one each flight. */
  seed: number | null;
}

export interface ViewOptions {
  camera: CameraMode;
  speed: number;
  paused: boolean;
  /** The blue "analysis" view: the vehicle as a blueprint, its forces, and each part's calculations. */
  analysis: boolean;
  /** The part open in the analysis view (`lib/ship/parts.ts`), or null for the whole vehicle. */
  part: string | null;
  /** The keyboard help sheet. */
  help: boolean;
  /** The analysis view replays the recorded instant at this sim time, s (null: live). */
  scrubT: number | null;
  voice: boolean;
  /** Engine, wind and impact sound. */
  sound: boolean;
  plan: boolean;
  trail: boolean;
  preview: boolean;
}

export const DEFAULT_OPTIONS: Options = {
  propellant: null,
  failed: [],
  gimbal: null,
  throttleMin: null,
  isp: null,
  ignition: null,
  density: 1,
  windSpeed: 0,
  windDir: 180,
  gustSpeed: 15,
  gustDir: 180,
  lag: 0.3,
  noise: "low",
  seed: null,
};

export const DEFAULT_VIEW: ViewOptions = {
  camera: "director",
  speed: 1,
  paused: false,
  analysis: false,
  part: null,
  help: false,
  scrubT: null,
  voice: true,
  sound: true,
  plan: true,
  trail: true,
  preview: true,
};

export const SPEEDS = [0.25, 0.5, 1, 2, 4, 8] as const;

type Range = [number, number];
interface Limits {
  z: Range;
  x: Range;
  y: Range;
  vz: Range;
  vx: Range;
  vy: Range;
}

/** Allowed start box per flight kind (m, m/s). */
export const START_LIMITS: Record<ScenarioKind, Limits> = {
  earth: { z: [2500, 80_000], x: [-1500, 3500], y: [0, 0], vz: [-2600, -40], vx: [-150, 100], vy: [0, 0] },
  booster: { z: [5000, 20_000], x: [-800, 1400], y: [-500, 500], vz: [-600, -250], vx: [-60, 60], vy: [-30, 30] },
};

export interface Preset {
  name: string;
  note: string;
  start: Start;
}

/** Named starts; the first is the default. All were flown in the Rust tests or probes. */
export const PRESETS: Record<ScenarioKind, Preset[]> = {
  earth: [
    { name: "Belly flop", note: "5 km, subsonic", start: { position: [700, 0, 5000], velocity: [-25, 0, -75] } },
    { name: "High drop", note: "25 km", start: { position: [2500, 0, 25_000], velocity: [-60, 0, -250] } },
    { name: "Entry", note: "80 km, Mach 7", start: { position: [3000, 0, 80_000], velocity: [-100, 0, -2500] } },
  ],
  booster: [
    { name: "Return", note: "14 km, Mach 1.4", start: { position: [900, 0, 14_000], velocity: [-30, 0, -420] } },
    { name: "Fast return", note: "20 km, Mach 1.8", start: { position: [900, 0, 20_000], velocity: [-25, 0, -550] } },
    { name: "Late", note: "6 km", start: { position: [300, 0, 6000], velocity: [0, 0, -300] } },
  ],
};

const clamp = (v: number, [lo, hi]: Range) => Math.min(hi, Math.max(lo, v));

/** Keep a start inside the kind's box. */
export function clampStart(kind: ScenarioKind, s: Start): Start {
  const l = START_LIMITS[kind];
  return {
    position: [clamp(s.position[0], l.x), clamp(s.position[1], l.y), clamp(s.position[2], l.z)],
    velocity: [clamp(s.velocity[0], l.vx), clamp(s.velocity[1], l.vy), clamp(s.velocity[2], l.vz)],
  };
}

/** The flight kind for a vehicle. */
export function kindOf(vehicle: Vehicle): ScenarioKind {
  return vehicle === "booster" ? "booster" : "earth";
}

const deg = Math.PI / 180;

/** The config Rust takes, from the page's state. */
export function configOf(s: Pick<ShipState, "scenario" | "start" | "options">): FlightConfig {
  const o = s.options;
  const tuning: Tuning = { density_scale: o.density };
  if (o.gimbal !== null) tuning.gimbal_max_deg = o.gimbal;
  if (o.throttleMin !== null) tuning.throttle_min = o.throttleMin;
  if (o.isp !== null) tuning.isp = o.isp;
  if (o.ignition !== null) tuning.ignition_time = o.ignition;
  const config: FlightConfig = {
    scenario: s.scenario,
    position: s.start.position,
    velocity: s.start.velocity,
    lag: o.lag,
    noise: o.noise,
    tuning,
    wind: [o.windSpeed * Math.cos(o.windDir * deg), o.windSpeed * Math.sin(o.windDir * deg)],
    gust_speed: o.gustSpeed,
    failed: o.failed,
  };
  if (o.propellant !== null) config.propellant = o.propellant * 1000;
  return config;
}

export interface ShipState {
  vehicle: Vehicle;
  /** `kindOf(vehicle)`. */
  scenario: ScenarioKind;
  info: Partial<Record<ScenarioKind, ScenarioInfo>>;
  start: Start;
  preset: number;
  options: Options;
  view: ViewOptions;
  mode: Mode;
  /** The opening sequence being played, and when it began (`performance.now()`). */
  intro: Intro;
  launchAt: number;
  /** The booster has been caught and stands beside the pad while the ship returns. */
  parked: boolean;
  preview: Preview | null;
  previewing: boolean;
  flight: FlightView | null;
  error: string | null;
  setInfo(kind: ScenarioKind, info: ScenarioInfo): void;
  setVehicle(v: Vehicle): void;
  setStart(start: Start): void;
  setPreset(i: number): void;
  setOptions(patch: Partial<Options>): void;
  toggleFailed(engine: number): void;
  resetOptions(): void;
  setView(patch: Partial<ViewOptions>): void;
  setPreview(p: Preview | null, previewing?: boolean): void;
  fly(): void;
  /** The opening sequence is over (or skipped): the simulated flight begins. */
  beginFlight(): void;
  /** After a booster catch: fly the ship's return to the same tower. */
  continueWithShip(): void;
  reset(): void;
  finish(): void;
  setFlight(v: FlightView): void;
  setError(message: string | null): void;
}

function fresh(kind: ScenarioKind): Pick<ShipState, "scenario" | "start" | "preset" | "mode" | "preview" | "flight" | "options"> {
  return {
    scenario: kind,
    start: clampStart(kind, PRESETS[kind][0].start),
    preset: 0,
    mode: "editing",
    preview: null,
    flight: null,
    // Engine failures and vehicle numbers do not carry over to another vehicle.
    options: { ...DEFAULT_OPTIONS },
  };
}

export const useShip = create<ShipState>((set, get) => ({
  vehicle: "booster",
  info: {},
  view: { ...DEFAULT_VIEW },
  previewing: false,
  error: null,
  intro: "stack",
  launchAt: 0,
  parked: false,
  ...fresh("booster"),
  setInfo(kind, info) {
    set({ info: { ...get().info, [kind]: info } });
  },
  setVehicle(vehicle) {
    const s = get();
    if (vehicle === s.vehicle || s.mode === "flying") return;
    set({ vehicle, ...fresh(kindOf(vehicle)), options: { ...s.options, ...carried(s.options) } });
  },
  setStart(start) {
    if (get().mode !== "editing") return;
    set({ start: clampStart(get().scenario, start), preset: -1 });
  },
  setPreset(i) {
    const s = get();
    const p = PRESETS[s.scenario][i];
    if (!p || s.mode !== "editing") return;
    set({ start: clampStart(s.scenario, p.start), preset: i });
  },
  setOptions(patch) {
    set({ options: { ...get().options, ...patch } });
  },
  toggleFailed(engine) {
    const o = get().options;
    const failed = o.failed.includes(engine) ? o.failed.filter((e) => e !== engine) : [...o.failed, engine].sort((a, b) => a - b);
    set({ options: { ...o, failed } });
  },
  resetOptions() {
    set({ options: { ...DEFAULT_OPTIONS } });
  },
  setView(patch) {
    const s = get();
    const view = { ...s.view, ...patch };
    // Pausing and the analysis view belong to a flight.
    if (s.mode === "editing" || s.mode === "launch") {
      view.paused = false;
      view.analysis = false;
    }
    // A part is open, and a recorded instant replayed, only inside the analysis view.
    if (!view.analysis) {
      view.part = null;
      view.scrubT = null;
    }
    // Replaying needs the flight to stand still.
    if (s.mode === "flying" && !view.paused) view.scrubT = null;
    set({ view });
  },
  setPreview(preview, previewing = false) {
    set({ preview, previewing });
  },
  fly() {
    const s = get();
    if (s.mode !== "editing") return;
    set({ mode: "launch", intro: "stack", launchAt: now(), parked: false, flight: null, view: { ...s.view, paused: false } });
  },
  beginFlight() {
    if (get().mode === "launch") set({ mode: "flying", flight: null });
  },
  continueWithShip() {
    const s = get();
    if (s.mode !== "done" || s.scenario !== "booster" || s.flight?.outcome !== "caught") return;
    set({
      vehicle: "ship",
      ...fresh("earth"),
      options: { ...s.options, ...carried(s.options) },
      mode: "launch",
      intro: "card",
      launchAt: now(),
      parked: true,
      view: { ...s.view, paused: false, analysis: false, part: null },
    });
  },
  reset() {
    set({ mode: "editing", flight: null, parked: false, view: { ...get().view, paused: false, analysis: false, part: null } });
  },
  finish() {
    if (get().mode === "flying") set({ mode: "done" });
  },
  setFlight(flight) {
    set({ flight });
  },
  setError(error) {
    set({ error });
  },
}));

/** The analysis view is showing (it belongs to a flight, in progress or over). */
export const analysisOn = (s: Pick<ShipState, "view" | "mode">) => s.view.analysis && (s.mode === "flying" || s.mode === "done");

const now = () => (typeof performance !== "undefined" ? performance.now() : 0);

/** Options that make sense for any vehicle (environment, sensors). */
function carried(o: Options): Partial<Options> {
  return {
    density: o.density,
    windSpeed: o.windSpeed,
    windDir: o.windDir,
    gustSpeed: o.gustSpeed,
    gustDir: o.gustDir,
    lag: o.lag,
    noise: o.noise,
    seed: o.seed,
    propellant: null,
    failed: [],
    gimbal: null,
    throttleMin: null,
    isp: null,
    ignition: null,
  };
}

/** Labels for the phase banner and timeline. */
export const PHASE_LABEL: Record<ShipPhase, string> = {
  "belly-flop": "Belly flop",
  flip: "Flip",
  descent: "Grid-fin descent",
  startup: "Engine start-up",
  "landing-burn": "Landing burn",
  terminal: "Final approach",
  done: "Complete",
};

/** The phases a kind goes through, in order (for the timeline). */
export function phasesOf(kind: ScenarioKind): ShipPhase[] {
  return kind === "booster"
    ? ["descent", "startup", "landing-burn", "terminal", "done"]
    : ["belly-flop", "flip", "landing-burn", "terminal", "done"];
}

export const VEHICLE_NAME: Record<Vehicle, string> = { ship: "Ship", booster: "Booster" };

/** The verdict line after the flight. */
export function verdict(kind: ScenarioKind, outcome: Outcome | null, reason: string | null): string {
  if (outcome === "caught") return kind === "booster" ? "Booster caught by the tower" : "Ship caught by the tower";
  if (outcome === "crashed") return `Lost: ${reason ?? "crashed"}`;
  return "";
}

import { create } from "zustand";
import type { Vec3 } from "./frames";
import type { EngineState, Outcome, Preview, ScenarioInfo, ScenarioKind, ShipPhase } from "./types";

export type Mode = "editing" | "flying" | "done";
export type Noise = "off" | "low" | "high";
export type CameraMode = "chase" | "tower" | "orbit";

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

export interface ShipState {
  scenario: ScenarioKind;
  info: Partial<Record<ScenarioKind, ScenarioInfo>>;
  start: Start;
  mode: Mode;
  preview: Preview | null;
  previewing: boolean;
  speed: number;
  lag: number;
  noise: Noise;
  camera: CameraMode;
  view: FlightView | null;
  error: string | null;
  setInfo(kind: ScenarioKind, info: ScenarioInfo): void;
  setScenario(kind: ScenarioKind): void;
  setStart(start: Start): void;
  setPreview(p: Preview | null, previewing?: boolean): void;
  land(): void;
  reset(): void;
  finish(): void;
  setView(v: FlightView): void;
  set(patch: Partial<Pick<ShipState, "speed" | "lag" | "noise" | "camera" | "error">>): void;
}

/** Allowed start box per scenario: altitude, downrange, speeds (m, m/s). */
export const START_LIMITS: Record<
  ScenarioKind,
  { z: [number, number]; x: [number, number]; vz: [number, number]; vx: [number, number] }
> = {
  earth: { z: [2500, 9000], x: [-1500, 2500], vz: [-110, -40], vx: [-60, 60] },
  mars: { z: [10_000, 18_000], x: [-3000, 3000], vz: [-500, -350], vx: [-80, 80] },
};

const clamp = (v: number, [lo, hi]: [number, number]) => Math.min(hi, Math.max(lo, v));

/** Keep a start inside the scenario's box (and in the x–z plane). */
export function clampStart(kind: ScenarioKind, s: Start): Start {
  const l = START_LIMITS[kind];
  return {
    position: [clamp(s.position[0], l.x), 0, clamp(s.position[2], l.z)],
    velocity: [clamp(s.velocity[0], l.vx), 0, clamp(s.velocity[2], l.vz)],
  };
}

const DEFAULT_START: Record<ScenarioKind, Start> = {
  earth: { position: [700, 0, 5000], velocity: [-25, 0, -75] },
  mars: { position: [1500, 0, 14_000], velocity: [-40, 0, -450] },
};

export const useShip = create<ShipState>((set, get) => ({
  scenario: "earth",
  info: {},
  start: DEFAULT_START.earth,
  mode: "editing",
  preview: null,
  previewing: false,
  speed: 1,
  lag: 0.3,
  noise: "low",
  camera: "chase",
  view: null,
  error: null,
  setInfo(kind, info) {
    set({ info: { ...get().info, [kind]: info } });
  },
  setScenario(kind) {
    if (kind === get().scenario || get().mode === "flying") return;
    set({ scenario: kind, start: clampStart(kind, DEFAULT_START[kind]), mode: "editing", preview: null, view: null });
  },
  setStart(start) {
    if (get().mode !== "editing") return;
    set({ start: clampStart(get().scenario, start) });
  },
  setPreview(preview, previewing = false) {
    set({ preview, previewing });
  },
  land() {
    if (get().mode === "editing") set({ mode: "flying", view: null });
  },
  reset() {
    set({ mode: "editing", view: null });
  },
  finish() {
    if (get().mode === "flying") set({ mode: "done" });
  },
  setView(view) {
    set({ view });
  },
  set(patch) {
    set(patch);
  },
}));

/** Labels for the phase banner. */
export const PHASE_LABEL: Record<ShipPhase, string> = {
  "belly-flop": "Belly flop",
  flip: "Flip",
  "landing-burn": "Landing burn",
  terminal: "Final approach",
  done: "Done",
};

/** The verdict line after the flight. */
export function verdict(kind: ScenarioKind, outcome: Outcome | null, reason: string | null): string {
  if (outcome === "caught") return "Caught by the tower";
  if (outcome === "landed") return kind === "mars" ? "Landed on Mars" : "Landed";
  if (outcome === "crashed") return `Lost: ${reason ?? "crashed"}`;
  return "";
}

import { create } from "zustand";
import type { CameraMode } from "./camera";
import { initialState, reduce, type Action, type AppState } from "./state";
import type { Vehicle } from "./types";

interface Store extends AppState {
  dispatch(action: Action): void;
  /** Solve time of the last accepted plan, ms. */
  lastSolveMs: number | null;
  /** Vehicle limits from the Rust side (for drawing bands and the cone). */
  vehicle: Vehicle | null;
  /** Camera chosen by the visitor; null = automatic. */
  cameraOverride: CameraMode | null;
  /** True while the visitor drags a handle (orbit controls pause). */
  dragging: boolean;
  setSolveMs(ms: number): void;
  setVehicle(v: Vehicle): void;
  setCameraOverride(mode: CameraMode | null): void;
  setDragging(on: boolean): void;
}

/** One store for the page: a thin wrapper around the pure reducer. */
export const useApp = create<Store>()((set) => ({
  ...initialState,
  lastSolveMs: null,
  vehicle: null,
  cameraOverride: null,
  dragging: false,
  dispatch: (action) => set((s) => reduce(s, action)),
  setSolveMs: (ms) => set({ lastSolveMs: ms }),
  setVehicle: (v) => set({ vehicle: v }),
  setCameraOverride: (mode) => set({ cameraOverride: mode }),
  setDragging: (on) => set({ dragging: on }),
}));

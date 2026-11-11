import { create } from "zustand";
import { initialState, reduce, type Action, type AppState } from "./state";
import type { Vehicle } from "./types";

interface Store extends AppState {
  dispatch(action: Action): void;
  /** Solve time of the last accepted plan, ms. */
  lastSolveMs: number | null;
  /** Vehicle limits from the Rust side (for drawing bands and the cone). */
  vehicle: Vehicle | null;
  setSolveMs(ms: number): void;
  setVehicle(v: Vehicle): void;
}

/** One store for the page: a thin wrapper around the pure reducer. */
export const useApp = create<Store>()((set) => ({
  ...initialState,
  lastSolveMs: null,
  vehicle: null,
  dispatch: (action) => set((s) => reduce(s, action)),
  setSolveMs: (ms) => set({ lastSolveMs: ms }),
  setVehicle: (v) => set({ vehicle: v }),
}));
